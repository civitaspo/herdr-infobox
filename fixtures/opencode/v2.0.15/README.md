# OpenCode 2.0.15 runtime fixture

`export.json` comes from an isolated real session using the released CLI and an
explicit local server. The Plan agent read `sample.txt`, fetched `example.com`,
and proposed a change. The same session resumed with the build agent and
patched the file while infobox was closed. Reconciliation displayed the
repository link, opened reference, proposed Plan, and matching unstaged diff.

Identifiers, local paths, prompts, and assistant prose are anonymized. Provider
state, reasoning, model accounting, and snapshots are omitted. The observed
message and tool envelopes, including terminal `idle` markers, are retained.
No credentials or account information are included.

The official release source is commit
`6f3639d82ed0760091792189b78f8eeb44f699b1`. Its export endpoint, CLI API handler,
session/message schemas, Plan plugin, and webfetch implementation match 2.0.18.
The server protocol adds unrelated pairing endpoints in 2.0.18; the `/api/info`
response consumed here is unchanged. Other versions remain unverified.

Plan approval, execution, page titles, task checklists, Herdr pane controls, and
annotate delivery were not established by this run. `idle.outcome` describes
session execution and must not be interpreted as Plan approval or completion.
