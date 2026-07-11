# Targeted rerun benchmark receipt

Recorded for `RIPR-SPEC-0123` on commit `b46f828fc929c5e845d44f6c3e654f54876f3da1`.

Command:

```text
cargo xtask targeted-rerun-benchmark \
  --root benchmarks/targeted_rerun_benchmark/input \
  --changed-test tests/targeted.rs \
  --samples 5 \
  --timeout-ms 120000
```

Runner: `local-windows-x86_64`  
Analyzer: `ripr 0.10.0`  
Cache: isolated `RIPR_CACHE_DIR`, reset between cold and invalidation samples

Measured receipt (`target/ripr/reports/targeted-rerun-benchmark.json`):

| Measure | Result |
| --- | ---: |
| Cold full p50 | 1,207 ms |
| Cold full p95 | 1,713 ms |
| Warm targeted p50 | 204 ms |
| Warm targeted p95 | 205 ms |
| Cold-to-warm p50 speedup | 5.9167x |
| Warm p50 target | 30,000 ms |
| Parity | `matched` |
| Explicit invalidation | `recomputed_file_facts` after cache reset |
| Receipt status | `pass` |

This is a named benchmark for the committed fixture, revision, configuration,
and runner class. It does not claim universal latency, runtime mutation
behavior, correctness, or coverage adequacy. Targeted receipts now attribute
owned file-content, workspace manifest, lockfile, toolchain, configuration,
feature, policy, and explicit selector-ledger changes when those fingerprints
are available; unsupported or unavailable inputs remain explicitly named.
