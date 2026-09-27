# Packaging validation

The packaging change is separate from the plugin implementation. It leaves the shared release preparer, signing service, permissions, pins, allowlist, and source-only publication in place. The existing Securefix autofix request carries version synchronization.

On macOS arm64, `mise run lint`, `mise run test`, and `mise run build` passed with version checks enabled. Temporary fixtures verified stable and prerelease synchronization, read-only check mode, invalid-version rejection, no partial changes for a missing destination record, and unchanged dependency/minimum-Herdr versions.

`bash scripts/package.sh` built the locked `aarch64-apple-darwin` release binary and archive. SHA-256 matched its checksum file. Archive inspection confirmed a single top-level directory and the binary, manifest, license, installation script, and OpenCode runtime files. No database, node_modules, or Cargo target tree was included.

The archive was extracted to a temporary directory. Its executable reported `herdr-infobox 0.0.0`, matching the packaged authority and manifest. The packaged installer succeeded without a Rust project, and the extracted binary returned doctor JSON using fresh temporary state.

The packaging diff received independent correctness and ponytail review without blockers. Other target triples, server asset publication, and GitHub App installation coverage remain unverified. No release workflow was dispatched, release published, live hook edited, or real agent session started.
