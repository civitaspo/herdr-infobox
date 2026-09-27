use crate::{Result, config::Paths, model::*};
use futures::executor::block_on;
use turso::{Connection, IntoParams, Row};
macro_rules! params { ($($value:expr),* $(,)?) => { turso::params![$(($value).to_owned()),*] }; }
use std::{path::Path, time::Duration};

pub struct Store {
    conn: Connection,
}
impl Store {
    pub fn open(paths: &Paths) -> Result<Self> {
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(paths.state.join("schema.lock"))?;
        lock.lock()?;
        let store = Self::connect(paths)?;
        let version: i64 = one(&store.conn, "PRAGMA user_version", (), |r| r.get(0))?;
        if version > 1 {
            return Err("Database schema is newer than this binary".into());
        }
        if version == 0 {
            block_on(store.conn.execute_batch(SCHEMA))?;
        }
        Ok(store)
    }
    pub fn open_existing(paths: &Paths) -> Result<Self> {
        if !paths.db.exists() {
            return Err("Run session add or ui before enabling hooks".into());
        }
        let store = Self::connect(paths)?;
        let version: i64 = one(&store.conn, "PRAGMA user_version", (), |r| r.get(0))?;
        if version != 1 {
            return Err("Database migration required; run doctor outside the hook".into());
        }
        Ok(store)
    }
    fn connect(paths: &Paths) -> Result<Self> {
        let path = paths
            .db
            .to_str()
            .ok_or("Turso database path must be UTF-8")?;
        let db = block_on(
            turso::Builder::new_local(path)
                .experimental_multiprocess_wal(true)
                .build(),
        )?;
        let conn = db.connect()?;
        conn.busy_timeout(Duration::from_millis(25))?;
        block_on(conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;"))?;
        Ok(Self { conn })
    }
    pub fn register(&mut self, key: &SessionKey) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        exec(
            &self.conn,
            "INSERT INTO sessions(id,host,provider,native_id,scope,observed_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(host,provider,native_id,scope) DO NOTHING",
            params![
                id,
                key.host_id,
                key.provider.name(),
                key.native_session_id,
                key.agent_scope,
                now()
            ],
        )?;
        self.id_for(key)?
            .ok_or_else(|| "Session registration failed".into())
    }
    pub fn id_for(&self, key: &SessionKey) -> Result<Option<String>> {
        optional(
            &self.conn,
            "SELECT id FROM sessions WHERE host=?1 AND provider=?2 AND native_id=?3 AND scope=?4",
            params![
                key.host_id,
                key.provider.name(),
                key.native_session_id,
                key.agent_scope
            ],
            |r| r.get(0),
        )
    }
    pub fn sessions(&self) -> Result<Vec<SessionSummary>> {
        let sql = "SELECT id,host,provider,native_id,scope,ended FROM sessions ORDER BY observed_at DESC,id";
        let rows = query(&self.conn, sql, (), |r| {
            Ok((
                r.get::<String>(0)?,
                r.get::<String>(1)?,
                r.get::<String>(2)?,
                r.get::<String>(3)?,
                r.get::<String>(4)?,
                r.get::<bool>(5)?,
            ))
        })?;
        rows.into_iter()
            .map(|r| {
                let (id, host_id, provider, native_session_id, agent_scope, ended) = r;
                Ok(SessionSummary {
                    id,
                    key: SessionKey {
                        host_id,
                        provider: parse_provider(&provider)?,
                        native_session_id,
                        agent_scope,
                    },
                    ended,
                })
            })
            .collect()
    }
    pub fn resolve(&self, selector: &str) -> Result<SessionSummary> {
        let matches: Vec<_> = self
            .sessions()?
            .into_iter()
            .filter(|s| {
                s.id == selector
                    || s.id.starts_with(selector)
                    || s.key.native_session_id == selector
                    || format!("{}:{}", s.key.provider.name(), s.key.native_session_id) == selector
            })
            .collect();
        match matches.len() {
            1 => Ok(matches.into_iter().next().unwrap()),
            0 => Err(format!("Unknown session {selector}; use session list").into()),
            _ => Err(format!("Ambiguous session {selector}; use its UUID").into()),
        }
    }
    pub fn commit_batch(&mut self, batch: &EventBatch) -> Result<bool> {
        validate_batch(batch)?;
        atomic(&mut self.conn, |tx| apply_batch(tx, batch))
    }
    pub fn commit_import(&mut self, chunk: &ImportChunk) -> Result<()> {
        if chunk.cursor_before.as_ref().is_some_and(|c| {
            c.session_id != chunk.cursor_after.session_id
                || c.source_path != chunk.cursor_after.source_path
                || c.parser_version != chunk.cursor_after.parser_version
        }) {
            return Err("Cursor identity mismatch".into());
        }
        for batch in &chunk.batches {
            validate_batch(batch)?;
            if self.id_for(&batch.session)?.as_deref() != Some(&chunk.cursor_after.session_id) {
                return Err("Transcript batch belongs to another session".into());
            }
        }
        atomic(&mut self.conn, |tx| {
            let path = path_bytes(&chunk.cursor_after.source_path);
            let old: Option<(String, i64, String)> = optional(
                tx,
                "SELECT identity,offset,parser FROM source_cursors WHERE path=?1 AND session=?2 AND parser=?3",
                params![
                    &path,
                    chunk.cursor_after.session_id,
                    chunk.cursor_after.parser_version
                ],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            let expected = chunk.cursor_before.as_ref().map(|c| {
                (
                    c.file_identity.clone(),
                    c.offset as i64,
                    c.parser_version.clone(),
                )
            });
            if old != expected {
                return Err(
                    "Transcript cursor changed concurrently; retry from current cursor".into(),
                );
            }
            for batch in &chunk.batches {
                apply_batch(tx, batch)?;
            }
            exec(
                tx,
                "INSERT INTO source_cursors(path,identity,offset,parser,session) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(session,path,parser) DO UPDATE SET identity=excluded.identity,offset=excluded.offset,parser=excluded.parser",
                params![
                    path,
                    chunk.cursor_after.file_identity,
                    i64::try_from(chunk.cursor_after.offset)?,
                    chunk.cursor_after.parser_version,
                    chunk.cursor_after.session_id
                ],
            )?;
            Ok(())
        })
    }
    pub fn cursor(
        &self,
        session_id: &str,
        path: &Path,
        parser: &str,
    ) -> Result<Option<SourceCursor>> {
        optional(
            &self.conn,
            "SELECT identity,offset,parser FROM source_cursors WHERE path=?1 AND session=?2 AND parser=?3",
            params![path_bytes(path), session_id, parser],
            |r| {
                Ok(SourceCursor {
                    session_id: session_id.into(),
                    source_path: path.into(),
                    file_identity: r.get(0)?,
                    offset: r.get::<i64>(1)? as u64,
                    parser_version: r.get(2)?,
                })
            },
        )
    }
    pub fn transcript_sources(&self) -> Result<Vec<(SessionSummary, SourceCursor)>> {
        let mut result = Vec::new();
        for session in self.sessions()? {
            let sql = "SELECT path,identity,offset,parser FROM source_cursors WHERE session=?1";
            let cursors = query(&self.conn, sql, params![session.id], |r| {
                Ok(SourceCursor {
                    session_id: session.id.clone(),
                    source_path: bytes_path(r.get(0)?),
                    file_identity: r.get(1)?,
                    offset: r.get::<i64>(2)? as u64,
                    parser_version: r.get(3)?,
                })
            })?;
            for cursor in cursors {
                result.push((session.clone(), cursor));
            }
        }
        Ok(result)
    }
    pub fn pending_paths(&self) -> Result<Vec<PendingPath>> {
        let sql = "SELECT event_id,item,session,path,cwd,relation FROM pending_paths WHERE error IS NULL LIMIT 100";
        query(&self.conn, sql, (), |r| {
            Ok(PendingPath {
                event_id: r.get(0)?,
                index: r.get::<i64>(1)? as usize,
                session_id: r.get(2)?,
                path: bytes_path(r.get(3)?),
                cwd: bytes_path(r.get(4)?),
                relation: r.get(5)?,
            })
        })
    }
    pub fn finish_path(
        &mut self,
        pending: &PendingPath,
        result: &std::result::Result<Discovery, String>,
    ) -> Result<()> {
        match result {
            Ok(found) => {
                self.add_worktree(&pending.session_id, found, &pending.relation)?;
                exec(
                    &self.conn,
                    "DELETE FROM pending_paths WHERE event_id=?1 AND item=?2",
                    params![pending.event_id, pending.index as i64],
                )?;
            }
            Err(reason) => {
                exec(
                    &self.conn,
                    "UPDATE pending_paths SET error=?3 WHERE event_id=?1 AND item=?2",
                    params![pending.event_id, pending.index as i64, reason],
                )?;
            }
        }
        Ok(())
    }
    pub fn add_worktree(
        &mut self,
        session: &str,
        found: &Discovery,
        relation: &str,
    ) -> Result<String> {
        atomic(&mut self.conn, |tx| {
            let host: String = one(
                tx,
                "SELECT host FROM sessions WHERE id=?1",
                params![session],
                |r| r.get(0),
            )?;
            let repo = uuid::Uuid::new_v4().to_string();
            exec(
                tx,
                "INSERT INTO repositories(id,host,common_dir) VALUES(?1,?2,?3) ON CONFLICT(host,common_dir) DO NOTHING",
                params![repo, host, path_bytes(&found.common_dir)],
            )?;
            let repo: String = one(
                tx,
                "SELECT id FROM repositories WHERE host=?1 AND common_dir=?2",
                params![host, path_bytes(&found.common_dir)],
                |r| r.get(0),
            )?;
            let id = uuid::Uuid::new_v4().to_string();
            exec(
                tx,
                "INSERT INTO worktrees(id,repository,root,git_dir,url,branch) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(repository,git_dir,root) DO UPDATE SET url=excluded.url,branch=excluded.branch",
                params![
                    id,
                    repo,
                    path_bytes(&found.root),
                    path_bytes(&found.git_dir),
                    found.github_url,
                    found.branch
                ],
            )?;
            let id: String = one(
                tx,
                "SELECT id FROM worktrees WHERE repository=?1 AND git_dir=?2 AND root=?3",
                params![repo, path_bytes(&found.git_dir), path_bytes(&found.root)],
                |r| r.get(0),
            )?;
            exec(
                tx,
                "INSERT OR IGNORE INTO session_worktrees(session,worktree,relation) VALUES(?1,?2,?3)",
                params![session, id, relation],
            )?;
            Ok(id)
        })
    }
    pub fn view(&self, session: &SessionSummary) -> Result<SessionView> {
        let sid = &session.id;
        let sql = "SELECT DISTINCT w.id,w.repository,w.root,w.git_dir,r.common_dir,w.url,w.branch FROM worktrees w JOIN repositories r ON r.id=w.repository JOIN session_worktrees sw ON sw.worktree=w.id WHERE sw.session=?1 ORDER BY w.root";
        let mut worktrees = query(&self.conn, sql, params![sid], |r| {
            Ok(Worktree {
                id: r.get(0)?,
                repository_id: r.get(1)?,
                root: bytes_path(r.get(2)?),
                git_dir: bytes_path(r.get(3)?),
                common_dir: bytes_path(r.get(4)?),
                github_url: r.get(5)?,
                branch: r.get(6)?,
                relations: vec![],
            })
        })?;
        for w in &mut worktrees {
            w.relations = self.strings(
                "SELECT relation FROM session_worktrees WHERE session=?1 AND worktree=?2",
                sid,
                &w.id,
            )?;
        }
        let sql = "SELECT url,title,failure FROM refs WHERE session=?1 ORDER BY rowid DESC";
        let mut references = query(&self.conn, sql, params![sid], |r| {
            Ok(Reference {
                url: r.get(0)?,
                title: r.get(1)?,
                failure: r.get(2)?,
                relations: vec![],
                sources: vec![],
            })
        })?;
        for r in &mut references {
            r.relations = self.strings(
                "SELECT DISTINCT relation FROM reference_observations WHERE session=?1 AND url=?2",
                sid,
                &r.url,
            )?;
            r.sources = self.strings(
                "SELECT DISTINCT source FROM reference_observations WHERE session=?1 AND url=?2",
                sid,
                &r.url,
            )?;
        }
        let sql = "SELECT r.id,p.plan_key,r.hash,r.markdown,r.path,r.phase,e.state,p.selected_revision=r.id FROM plan_revisions r JOIN plans p ON p.id=r.plan LEFT JOIN plan_executions e ON e.revision=r.id WHERE p.session=?1 ORDER BY r.rowid DESC";
        let rows = query(&self.conn, sql, params![sid], |r| {
            Ok((
                r.get::<String>(0)?,
                r.get::<String>(1)?,
                r.get::<String>(2)?,
                r.get::<String>(3)?,
                r.get::<Option<Vec<u8>>>(4)?,
                r.get::<String>(5)?,
                r.get::<Option<String>>(6)?,
                r.get::<Option<bool>>(7)?.unwrap_or(false),
            ))
        })?;
        let plans = rows
            .into_iter()
            .map(|r| {
                let (id, plan_key, hash, markdown, path, phase, execution, selected) = r;
                Ok(PlanRevision {
                    id,
                    plan_key,
                    hash,
                    markdown,
                    source_path: path.map(bytes_path),
                    phase: serde_json::from_str(&phase)?,
                    execution: execution.map(|s| serde_json::from_str(&s)).transpose()?,
                    selected,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let sql = "SELECT task_id,text,status,parent_id FROM plan_tasks WHERE session=?1 ORDER BY namespace,task_id";
        let tasks = query(&self.conn, sql, params![sid], |r| {
            Ok(Task {
                id: r.get(0)?,
                text: r.get(1)?,
                status: r.get(2)?,
                parent_id: r.get(3)?,
            })
        })?;
        let sql = "SELECT feature,state,reason FROM capabilities WHERE session=?1 ORDER BY feature";
        let capabilities = query(&self.conn, sql, params![sid], |r| {
            Ok(Capability {
                feature: r.get(0)?,
                state: r.get(1)?,
                reason: r.get(2)?,
            })
        })?;
        Ok(SessionView {
            session: self.resolve(&session.id)?,
            worktrees,
            references,
            plans,
            tasks,
            capabilities,
        })
    }
    fn strings(&self, sql: &str, a: &str, b: &str) -> Result<Vec<String>> {
        query(&self.conn, sql, params![a, b], |r| r.get(0))
    }
    pub fn health(&self) -> Result<serde_json::Value> {
        let version: i64 = one(&self.conn, "PRAGMA user_version", (), |r| r.get(0))?;
        let (count, last): (i64, Option<i64>) = one(
            &self.conn,
            "SELECT COUNT(*),MAX(received_at) FROM events",
            (),
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let pending: i64 = one(&self.conn, "SELECT COUNT(*) FROM pending_paths", (), |r| {
            r.get(0)
        })?;
        let cursors: i64 = one(&self.conn, "SELECT COUNT(*) FROM source_cursors", (), |r| {
            r.get(0)
        })?;
        let diagnostics = self
            .diagnostics()?
            .into_iter()
            .map(|(name, _, count)| serde_json::json!({"name":name,"count":count}))
            .collect::<Vec<_>>();
        Ok(
            serde_json::json!({"engine":"turso","engine_version":"0.7.2","multiprocess_wal":"experimental","schema_version":version,"event_count":count,"last_event_at":last,"pending_paths":pending,"transcript_cursors":cursors,"diagnostics":diagnostics}),
        )
    }
    pub fn diagnostic(&self, name: &str, reason: &str) -> Result<()> {
        exec(
            &self.conn,
            "INSERT INTO diagnostics(name,reason,count) VALUES(?1,?2,1) ON CONFLICT(name) DO UPDATE SET reason=excluded.reason,count=count+1",
            params![name, reason],
        )?;
        Ok(())
    }
    pub fn diagnostics(&self) -> Result<Vec<(String, String, i64)>> {
        query(
            &self.conn,
            "SELECT name,reason,count FROM diagnostics",
            (),
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
    }
}
fn exec(conn: &Connection, sql: &str, params: impl IntoParams) -> Result<u64> {
    Ok(block_on(conn.execute(sql, params))?)
}
fn query<T>(
    conn: &Connection,
    sql: &str,
    params: impl IntoParams,
    map: impl Fn(&Row) -> turso::Result<T>,
) -> Result<Vec<T>> {
    block_on(async {
        let mut rows = conn.query(sql, params).await?;
        let mut values = Vec::new();
        while let Some(row) = rows.next().await? {
            values.push(map(&row)?);
        }
        Ok(values)
    })
}
fn optional<T>(
    conn: &Connection,
    sql: &str,
    params: impl IntoParams,
    map: impl Fn(&Row) -> turso::Result<T>,
) -> Result<Option<T>> {
    Ok(query(conn, sql, params, map)?.into_iter().next())
}
fn one<T>(
    conn: &Connection,
    sql: &str,
    params: impl IntoParams,
    map: impl Fn(&Row) -> turso::Result<T>,
) -> Result<T> {
    optional(conn, sql, params, map)?.ok_or_else(|| "Required database row is missing".into())
}
fn atomic<T>(conn: &mut Connection, operation: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    let tx = block_on(
        conn.transaction_with_behavior(turso::transaction::TransactionBehavior::Immediate),
    )?;
    match operation(&tx) {
        Ok(value) => {
            block_on(tx.commit())?;
            Ok(value)
        }
        Err(error) => {
            block_on(tx.rollback())?;
            Err(error)
        }
    }
}

fn validate_batch(batch: &EventBatch) -> Result<()> {
    if batch.schema_version != 1 {
        return Err("Unsupported normalized event schema".into());
    }
    if batch.events.len() > 1000 {
        return Err("Too many observations".into());
    }
    for event in &batch.events {
        if let Observation::Plan { markdown, .. } = event
            && markdown.len() > 1024 * 1024
        {
            return Err("Plan exceeds 1 MiB".into());
        }
    }
    Ok(())
}
fn apply_batch(tx: &Connection, batch: &EventBatch) -> Result<bool> {
    let key = &batch.session;
    let sid: Option<String> = optional(
        tx,
        "SELECT id FROM sessions WHERE host=?1 AND provider=?2 AND native_id=?3 AND scope=?4",
        params![
            key.host_id,
            key.provider.name(),
            key.native_session_id,
            key.agent_scope
        ],
        |r| r.get(0),
    )?;
    let sid = sid.ok_or("Session is not registered; use session add or bind a Herdr pane")?;
    let inserted = exec(
        tx,
        "INSERT OR IGNORE INTO events(session,source,source_id,call_key,source_order,payload,received_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![
            sid,
            batch.source,
            batch.source_event_id,
            batch.native_call_key,
            batch.source_order.map(i64::try_from).transpose()?,
            serde_json::to_string(batch)?,
            now()
        ],
    )?;
    if inserted == 0 {
        return Ok(false);
    }
    let event_id = tx.last_insert_rowid();
    exec(
        tx,
        "UPDATE sessions SET observed_at=?2 WHERE id=?1",
        params![sid, now()],
    )?;
    for (index, event) in batch.events.iter().enumerate() {
        match event {
            Observation::Session {
                parent_native_id,
                ended,
            } => {
                exec(
                    tx,
                    "UPDATE sessions SET parent_native_id=COALESCE(?2,parent_native_id),ended=?3 WHERE id=?1",
                    params![sid, parent_native_id, ended],
                )?;
            }
            Observation::Path {
                path,
                cwd,
                relation,
            } => {
                exec(
                    tx,
                    "INSERT INTO pending_paths(event_id,item,session,path,cwd,relation) VALUES(?1,?2,?3,?4,?5,?6)",
                    params![
                        event_id,
                        index as i64,
                        sid,
                        path_bytes(path),
                        path_bytes(cwd),
                        relation
                    ],
                )?;
            }
            Observation::Reference {
                url,
                title,
                title_source,
                relation,
            } => {
                let url = normalize_url(url)?;
                let priority = match title_source.as_deref().filter(|_| title.is_some()) {
                    Some("manual") => 3,
                    Some("provider_result") => 2,
                    Some("citation_label") => 1,
                    _ => 0,
                };
                exec(
                    tx,
                    "INSERT INTO refs(session,url,title,title_priority) VALUES(?1,?2,?3,?4) ON CONFLICT(session,url) DO UPDATE SET title=CASE WHEN excluded.title IS NOT NULL AND excluded.title_priority>=refs.title_priority THEN excluded.title ELSE refs.title END,title_priority=MAX(refs.title_priority,excluded.title_priority)",
                    params![sid, url, title, priority],
                )?;
                let correlation = batch
                    .native_call_key
                    .as_deref()
                    .unwrap_or(&batch.source_event_id);
                exec(
                    tx,
                    "INSERT OR IGNORE INTO reference_observations(session,url,event_id,source,relation,correlation) VALUES(?1,?2,?3,?4,?5,?6)",
                    params![sid, url, event_id, batch.source, relation, correlation],
                )?;
            }
            Observation::FetchFailed { url, reason } => {
                let url = normalize_url(url)?;
                exec(
                    tx,
                    "INSERT INTO refs(session,url,failure) VALUES(?1,?2,?3) ON CONFLICT(session,url) DO UPDATE SET failure=excluded.failure",
                    params![sid, url, reason],
                )?;
            }
            Observation::Plan {
                plan_key,
                markdown,
                source_path,
                phase,
            } => {
                let plan = uuid::Uuid::new_v4().to_string();
                exec(
                    tx,
                    "INSERT OR IGNORE INTO plans(id,session,plan_key) VALUES(?1,?2,?3)",
                    params![plan, sid, plan_key],
                )?;
                let plan: String = one(
                    tx,
                    "SELECT id FROM plans WHERE session=?1 AND plan_key=?2",
                    params![sid, plan_key],
                    |r| r.get(0),
                )?;
                let hash = content_hash(markdown.as_bytes());
                let phase = serde_json::to_string(phase)?;
                exec(
                    tx,
                    "INSERT INTO plan_revisions(id,plan,hash,markdown,path,phase,event_id) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(plan,hash) DO UPDATE SET phase=CASE WHEN plan_revisions.phase='\"approved\"' OR (plan_revisions.phase='\"proposed\"' AND excluded.phase='\"draft\"') THEN plan_revisions.phase ELSE excluded.phase END",
                    params![
                        uuid::Uuid::new_v4().to_string(),
                        plan,
                        hash,
                        markdown,
                        source_path.as_deref().map(path_bytes),
                        phase,
                        event_id
                    ],
                )?;
            }
            Observation::Execution {
                plan_key,
                revision_hash,
                state,
                evidence,
            } => {
                let revision: String = one(
                    tx,
                    "SELECT r.id FROM plan_revisions r JOIN plans p ON p.id=r.plan WHERE p.session=?1 AND p.plan_key=?2 AND r.hash=?3",
                    params![sid, plan_key, revision_hash],
                    |r| r.get(0),
                )?;
                let existing: Option<(String, Option<i64>)> = optional(
                    tx,
                    "SELECT source,source_order FROM plan_executions WHERE revision=?1",
                    params![revision],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?;
                let allowed = existing.is_none()
                    || evidence == "manual"
                    || existing.as_ref().is_some_and(|(source, order)| {
                        source == &batch.source
                            && order
                                .zip(batch.source_order.map(|v| v as i64))
                                .is_some_and(|(old, new)| new > old)
                    });
                if allowed {
                    let selected: (Option<String>, Option<String>, Option<i64>) = one(
                        tx,
                        "SELECT selected_revision,selected_source,selected_order FROM plans WHERE session=?1 AND plan_key=?2",
                        params![sid, plan_key],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )?;
                    let order = batch.source_order.map(i64::try_from).transpose()?;
                    if selected.0.is_none()
                        || evidence == "manual"
                        || (selected.1.as_deref() == Some(&batch.source)
                            && selected.2.zip(order).is_some_and(|(old, new)| new > old))
                    {
                        exec(
                            tx,
                            "UPDATE plans SET selected_revision=?3,selected_source=?4,selected_order=?5 WHERE session=?1 AND plan_key=?2",
                            params![sid, plan_key, revision, batch.source, order],
                        )?;
                    }
                    exec(
                        tx,
                        "INSERT INTO plan_executions(revision,state,evidence,source,source_order) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(revision) DO UPDATE SET state=excluded.state,evidence=excluded.evidence,source=excluded.source,source_order=excluded.source_order",
                        params![
                            revision,
                            serde_json::to_string(state)?,
                            evidence,
                            batch.source,
                            batch.source_order.map(i64::try_from).transpose()?
                        ],
                    )?;
                }
            }
            Observation::Tasks {
                namespace,
                replace,
                tasks,
            } => {
                if *replace {
                    exec(
                        tx,
                        "DELETE FROM plan_tasks WHERE session=?1 AND namespace=?2",
                        params![sid, namespace],
                    )?;
                }
                for task in tasks {
                    exec(
                        tx,
                        "INSERT INTO plan_tasks(session,namespace,task_id,text,status,parent_id) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(session,namespace,task_id) DO UPDATE SET text=excluded.text,status=excluded.status,parent_id=excluded.parent_id",
                        params![
                            sid,
                            namespace,
                            task.id,
                            task.text,
                            task.status,
                            task.parent_id
                        ],
                    )?;
                }
            }
            Observation::Capability {
                feature,
                state,
                reason,
            } => {
                exec(
                    tx,
                    "INSERT INTO capabilities(session,feature,state,reason) VALUES(?1,?2,?3,?4) ON CONFLICT(session,feature) DO UPDATE SET state=excluded.state,reason=excluded.reason",
                    params![sid, feature, state, reason],
                )?;
            }
        }
    }
    Ok(true)
}
pub fn normalize_url(value: &str) -> Result<String> {
    let url = url::Url::parse(value)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Only HTTP and HTTPS references are supported".into());
    }
    let (_, rest) = value.split_once("://").ok_or("URL requires an authority")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    let credentials = authority
        .rsplit_once('@')
        .map(|(c, _)| format!("{c}@"))
        .unwrap_or_default();
    let host = match url.host().ok_or("URL host missing")? {
        url::Host::Ipv6(host) => format!("[{host}]"),
        host => host.to_string(),
    };
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    Ok(format!(
        "{}://{credentials}{host}{port}{}",
        url.scheme(),
        &rest[end..]
    ))
}
fn parse_provider(value: &str) -> Result<Provider> {
    match value {
        "claude" => Ok(Provider::Claude),
        "opencode" => Ok(Provider::OpenCode),
        "codex" => Ok(Provider::Codex),
        "cursor" => Ok(Provider::Cursor),
        "devin" => Ok(Provider::Devin),
        _ => Err("Unknown stored provider".into()),
    }
}
#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}
#[cfg(unix)]
fn bytes_path(bytes: Vec<u8>) -> std::path::PathBuf {
    use std::os::unix::ffi::OsStringExt;
    std::ffi::OsString::from_vec(bytes).into()
}
const SCHEMA: &str = r#"
BEGIN IMMEDIATE;
CREATE TABLE sessions(id TEXT PRIMARY KEY,host TEXT NOT NULL,provider TEXT NOT NULL,native_id TEXT NOT NULL,scope TEXT NOT NULL,parent_native_id TEXT,ended INTEGER NOT NULL DEFAULT 0,observed_at INTEGER NOT NULL,UNIQUE(host,provider,native_id,scope));
CREATE TABLE events(id INTEGER PRIMARY KEY,session TEXT NOT NULL REFERENCES sessions(id),source TEXT NOT NULL,source_id TEXT NOT NULL,call_key TEXT,source_order INTEGER,payload TEXT NOT NULL,received_at INTEGER NOT NULL,UNIQUE(session,source,source_id));
CREATE TABLE repositories(id TEXT PRIMARY KEY,host TEXT NOT NULL,common_dir BLOB NOT NULL,UNIQUE(host,common_dir));
CREATE TABLE worktrees(id TEXT PRIMARY KEY,repository TEXT NOT NULL REFERENCES repositories(id),root BLOB NOT NULL,git_dir BLOB NOT NULL,url TEXT,branch TEXT,UNIQUE(repository,git_dir,root));
CREATE TABLE session_worktrees(session TEXT REFERENCES sessions(id),worktree TEXT REFERENCES worktrees(id),relation TEXT NOT NULL,PRIMARY KEY(session,worktree,relation));
CREATE TABLE pending_paths(event_id INTEGER REFERENCES events(id),item INTEGER,session TEXT REFERENCES sessions(id),path BLOB,cwd BLOB,relation TEXT,error TEXT,PRIMARY KEY(event_id,item));
CREATE TABLE refs(session TEXT REFERENCES sessions(id),url TEXT,title TEXT,title_priority INTEGER NOT NULL DEFAULT 0,failure TEXT,PRIMARY KEY(session,url));
CREATE TABLE reference_observations(session TEXT,url TEXT,event_id INTEGER REFERENCES events(id),source TEXT,relation TEXT,correlation TEXT,FOREIGN KEY(session,url) REFERENCES refs(session,url),PRIMARY KEY(session,url,source,relation,correlation));
CREATE TABLE plans(id TEXT PRIMARY KEY,session TEXT REFERENCES sessions(id),plan_key TEXT,selected_revision TEXT,selected_source TEXT,selected_order INTEGER,UNIQUE(session,plan_key));
CREATE TABLE plan_revisions(id TEXT PRIMARY KEY,plan TEXT REFERENCES plans(id),hash TEXT,markdown TEXT,path BLOB,phase TEXT,event_id INTEGER REFERENCES events(id),UNIQUE(plan,hash));
CREATE TABLE plan_executions(revision TEXT PRIMARY KEY REFERENCES plan_revisions(id),state TEXT,evidence TEXT,source TEXT,source_order INTEGER);
CREATE TABLE plan_tasks(session TEXT REFERENCES sessions(id),namespace TEXT,task_id TEXT,text TEXT,status TEXT,parent_id TEXT,PRIMARY KEY(session,namespace,task_id));
CREATE TABLE capabilities(session TEXT REFERENCES sessions(id),feature TEXT,state TEXT,reason TEXT,PRIMARY KEY(session,feature));
CREATE TABLE source_cursors(session TEXT REFERENCES sessions(id),path BLOB,identity TEXT,offset INTEGER,parser TEXT,PRIMARY KEY(session,path,parser));
CREATE TABLE diagnostics(name TEXT PRIMARY KEY,reason TEXT,count INTEGER);
PRAGMA user_version=1;
COMMIT;
"#;
