# Targeted rerun benchmark receipt

Recorded for `RIPR-SPEC-0123` on current main commit `5aecff41c698d76b3bf2d68515f31a2037ae93db`.

Command:

```text
cargo xtask targeted-rerun-benchmark \
  --root benchmarks/targeted_rerun_benchmark/input \
  --changed-test tests/targeted.rs \
  --samples 5 \
  --timeout-ms 300000
```

Runner: `local-windows-x86_64`
Analyzer: `ripr 0.10.0`
Cache: isolated `RIPR_CACHE_DIR`, reset between cold and invalidation samples

Measured receipt (`target/ripr/reports/targeted-rerun-benchmark.json`):

| Measure | Result |
| --- | ---: |
| Cold full p50 | 1,512 ms |
| Cold full p95 | 1,667 ms |
| Warm targeted p50 | 228 ms |
| Warm targeted p95 | 918 ms |
| Cold-to-warm p50 speedup | 6.6316x |
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

The receipt was refreshed after the targeted-rerun graph-provenance and
CallDeletion analyzer merges. Parity remains `matched`, the explicit
cache-reset invalidation remains `recomputed_file_facts`, and the registered
thresholds remain satisfied for this five-sample current-main run.
