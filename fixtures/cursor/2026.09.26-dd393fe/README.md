# Cursor CLI 2026.09.26-dd393fe

Anonymized records captured from two authenticated normal CLI print-mode runs on
2026-09-29, including resume, a real WebFetch, and Plan mode CreatePlan. Session,
workspace, and call identifiers have been replaced. The stream fixture selects
initialization and relevant tool events; the durable fixture preserves message
record shapes. No credentials or private repository contents are included.

The durable JSONL projection contains tool inputs but no results or native call
IDs. It cannot establish successful fetches, titles, approval, or execution.
The stream includes explicit successful results but is only available when the
caller saves `--print --output-format stream-json` output.

Source inspection: installed release `7923.index.js` TranscriptStore projection
and `9404.index.js` CLI transcript writer. Structured print output is documented
at https://cursor.com/docs/cli/reference/output-format. Interactive TUI behavior
was not exercised by these fixtures.

Cursor does not expose a stable Plan document ID in these records. Exact Plan
Markdown is content-addressed across the two sources, while native stream call
IDs remain provenance. A changed body becomes a separate observed document;
no revision lineage or approval is inferred. The `todos` on CreatePlan are task
definitions, not proof of later task transitions.
