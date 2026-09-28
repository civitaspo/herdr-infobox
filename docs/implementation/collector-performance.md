# Collector process timing

On 2026-09-28, the local debug binary with the Turso engine was exercised on macOS 26.7 arm64 using temporary state. No coding agent, credentials, live hooks, or provider runtime participated.

Run the current Rust probe (which builds the native binary):

```sh
mise run benchmark
```

The probe enrolls one synthetic Claude session, launches five warmup collector processes, then measures 100 sequential processes with small synthetic `PreToolUse` WebFetch records. The historical measurements below used the original Python probe. Wall time includes subprocess launch, stdin delivery, decoding, database persistence, and process exit. The p95 uses the nearest-rank observation. All 105 processes returned success with empty stdout and stderr; a fresh `ui --once` process observed all 105 distinct reference URLs.

| Build | Samples | p50 | p95 | Maximum |
| --- | ---: | ---: | ---: | ---: |
| Local debug | 100 | 15.610 ms | 17.838 ms | 32.274 ms |

This is bounded local evidence for the small-hook path, not a release performance claim or provider compatibility test. It does not characterize large payloads, contention, cold filesystem caches, transcript reconciliation, or other hosts. The implementation checks independently cover busy-writer spooling and replay.

The replacement Rust harness was verified on the same host on 2026-09-28:
100 samples, p50 15.970 ms, p95 18.692 ms, maximum 33.475 ms, with all 105
processes silent and all 105 references persisted. These measurements include
reader-thread startup and up to 1 ms of exit-polling overhead. The changed
harness prevents a direct performance comparison with the historical result.
