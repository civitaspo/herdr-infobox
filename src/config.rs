use crate::Result;
use std::{fs, path::PathBuf};
#[derive(Debug, Clone)]
pub struct Paths {
    pub state: PathBuf,
    pub db: PathBuf,
    pub spool: PathBuf,
    pub snapshots: PathBuf,
    pub host_id: String,
}
impl Paths {
    pub fn open(state: Option<PathBuf>) -> Result<Self> {
        let state = state
            .or_else(|| std::env::var_os("HERDR_PLUGIN_STATE_DIR").map(PathBuf::from))
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("XDG_STATE_HOME").unwrap_or_else(|| {
                    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                        .join(".local/state")
                        .into_os_string()
                }))
                .join("herdr-infobox")
            });
        fs::create_dir_all(&state)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&state, fs::Permissions::from_mode(0o700))?;
        }
        let state = fs::canonicalize(state)?;
        let host_path = state.join("host-id");
        let host_id = match fs::read_to_string(&host_path) {
            Ok(id) => id,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                use std::io::Write;
                let id = uuid::Uuid::new_v4().to_string();
                let temporary = state.join(format!(".host-{}.tmp", uuid::Uuid::new_v4()));
                let mut f = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temporary)?;
                f.write_all(id.as_bytes())?;
                f.sync_all()?;
                let published = fs::hard_link(&temporary, &host_path);
                fs::remove_file(&temporary)?;
                match published {
                    Ok(()) => id,
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        fs::read_to_string(&host_path)?
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            Err(e) => return Err(e.into()),
        };
        uuid::Uuid::parse_str(host_id.trim())?;
        let paths = Self {
            db: state.join("infobox.sqlite3"),
            spool: state.join("spool"),
            snapshots: state.join("snapshots"),
            state,
            host_id: host_id.trim().into(),
        };
        fs::create_dir_all(&paths.spool)?;
        fs::create_dir_all(&paths.snapshots)?;
        Ok(paths)
    }
}
