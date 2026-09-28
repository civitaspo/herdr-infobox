# OpenCode 2.0.18 export fixture

Synthetic, anonymized fixture based on release `v2.0.18`, commit `cd9a14a6b688d4021bee381dfd39d2cef9c0f862`. It is not a captured live session. Unused schema fields are omitted.

Sources under that immutable commit in https://github.com/anomalyco/opencode:

- `packages/protocol/src/groups/session.ts`: `experimental.session.export`, response `{data:{info,messages}}`.
- `packages/schema/src/session.ts`, `session-message.ts`, `location.ts`: identity, session locations, message/tool states.
- `packages/core/src/plugin/plan.ts`: Plan mode discusses plans in conversation; switching agents does not record approval.
- `packages/core/src/tool/plugin/webfetch.ts`: URL input, completed fetch semantics, content-type metadata without page title.
- `packages/core/src/tool/plugin/read.ts`, `write.ts`, `edit.ts`: file inputs use `path`.

The decoder deliberately does not translate unknown checklist schemas into Plan documents, infer execution from session outcomes, or parse reasoning text. Live runtime compatibility requires separate verification.
