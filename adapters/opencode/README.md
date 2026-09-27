# OpenCode V1 bridge

This bridge observes completed tools through the published V1 plugin contract. It never changes tool arguments, output, permissions, or agent mode. It resolves the native session before assigning a directory. Collection failures are contained.

`createInfobox(binary, state)` accepts absolute paths and returns an OpenCode plugin. `herdr-infobox adapters install opencode` generates a local wrapper with these values. The optional `Infobox` export reads `HERDR_INFOBOX_BIN` and `HERDR_INFOBOX_STATE_DIR`. Both paths are required.

The type-only SDK dependency is pinned in package.json. OpenCode V2 and live V1 delivery remain unverified. See [compatibility](../../docs/compatibility.md).

## Run the bridge checks

Node 24 supports stripping the TypeScript types used by this bridge.

```sh
node adapters/opencode/test.mjs
```

The test mocks the OpenCode API and child spawn. It verifies session directory selection, absolute state arguments, immutable tool output, and failure containment. It does not start OpenCode or consume agent credentials.

For an SDK type check, install the pinned package into a temporary development directory with TypeScript and Node types. Run `tsc --noEmit --strict --skipLibCheck --module nodenext --moduleResolution nodenext --target es2022` against index.ts. These development tools are not runtime dependencies of the Rust executable.

Run the actual collector transport check after building Rust:

```sh
node adapters/opencode/integration.mjs target/debug/herdr-infobox
```

This uses temporary state and a mocked session API, then reads back the persisted reference through the real CLI. It does not start an agent session.
