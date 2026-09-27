# Collector process timing

On 2026-09-28, the local debug binary with the Turso engine was exercised on macOS 26.7 arm64 using temporary state. No coding agent, credentials, live hooks, or provider runtime participated.

Reproduce after building:

```sh
python3 scripts/benchmark-collector.py
# An optional first argument selects another compiled binary.
```

The probe enrolls one synthetic Claude session, launches five warmup collector processes, then measures 100 sequential processes with small synthetic `PreToolUse` WebFetch records. Wall time includes Python subprocess launch, stdin delivery, decoding, database persistence, and process exit. The p95 uses the nearest-rank observation. All 105 processes returned success with empty stdout and stderr; a fresh `ui --once` process observed all 105 distinct reference URLs.

| Build | Samples | p50 | p95 | Maximum |
| --- | ---: | ---: | ---: | ---: |
| Local debug | 100 | 15.610 ms | 17.838 ms | 32.274 ms |

This is bounded local evidence for the small-hook path, not a release performance claim or provider compatibility test. It does not characterize large payloads, contention, cold filesystem caches, transcript reconciliation, or other hosts. The implementation checks independently cover busy-writer spooling and replay.
