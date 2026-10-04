# Scale cliff benchmark

`cargo xtask scale-cliff-benchmark` times cold release `ripr check` and
`ripr pilot` runs as repository size grows and records where they stop being
usable. Each run is bounded by `--timeout-ms` and classified `pass`,
`timeout`, `refused_oversized` (the `RIPR_MAX_DIFF_INDEX_FILES` cap), or
`fail`, so a cliff is a status change in the receipt rather than a missing row.

```bash
# generated single-crate workspaces, one-file diff each
cargo xtask scale-cliff-benchmark --sizes 250,1000,4000,16000

# one real repository and the base revision of the change to analyze
cargo xtask scale-cliff-benchmark --repo /path/to/rust --base HEAD~3 --timeout-ms 900000
```

`--mode` and `--base` apply to `check` only; `pilot` always analyzes the whole
repository. `--sizes` is ignored when `--repo` is given.

The receipt (`target/ripr/reports/scale-cliff-benchmark.{json,md}`) records
wall time, peak RSS (GNU `time -v`, Linux runners only; `null` elsewhere),
check finding counts, and a log-log scaling exponent across synthetic sizes
(about 1 is linear, 2 quadratic). The index cap is raised to 1,000,000 by
default so the run measures analysis cost instead of the refusal; pass
`--index-cap product` to measure the shipped behavior.

## Why synthetic is not enough

A generated corpus has distinct function names and a trivial call graph, so it
bounds file-count cost only. The largest cost found so far depends on the call
graph and cannot be reproduced with this generator; it needs a real repository
with name-colliding code (`--repo`).

## Recorded observations (2026-10-04, `a7a089e`, release build, 4 cores, 15 GB)

One cold run each; single-threaded (about 100% of one core). The 160-crate
row, the real-repo rows and the rust-lang/rust timeout rows (with their RSS) were
measured by hand with an out-of-tree wrapper, not by this command: the generator
builds a single crate, and a timed-out run records no peak RSS. `check` is
`--mode draft` with a small diff and the index cap raised.

| Corpus | Files | check | pilot |
| --- | ---: | --- | --- |
| synthetic, one crate | 1,000 | 0.7 s, 57 MB | 2.0 s, 69 MB |
| synthetic, one crate | 4,000 | 2.7 s, 140 MB | 4.8 s, 168 MB |
| synthetic, one crate | 16,000 | 17 s, 493 MB | 11.6 s, 432 MB |
| synthetic, one crate | 32,000 | 46 s, 939 MB | not run |
| synthetic, 160 crates | 16,000 | 0.5 s, 20 MB (scoped to touched crate) | not run |
| ripgrep | 110 | 0.6 s, 53 MB | not run |
| tokio | 808 | 3.8 s, 191 MB | not run |
| cargo | 1,374 | 4.7 s, 330 MB | not run |
| rust-analyzer | 1,485 | 2.0 s, 198 MB | not run |
| bevy | 1,927 | 1.8 s, 142 MB | not run |
| rust-lang/rust @ `1d9e68013` | 39,123 | see below | 227 s, 2.0 GB |

Synthetic single-crate `check` is superlinear: about 1.2 between 1,000 and
32,000 files overall, rising to about 1.4 between 16,000 and 32,000. Slow but
still usable.

### rust-lang/rust

- Default `check --mode draft` refuses: `diff_scope_oversized: 36646 indexed
  Rust files exceed the RIPR_MAX_DIFF_INDEX_FILES limit (1200)`, exit 2.
- With the cap raised, wall time follows the owners of the
  `no_static_path` findings, not the file or finding count:

  | Diff | check |
  | --- | --- |
  | 1 file, 8 findings (6 `no_static_path`) | 14.8 s, 468 MB |
  | 3 files, 33 findings (11 `no_static_path`) | 20.7 s, 623 MB |
  | 22 files, 99 findings (7 `no_static_path`) | 273 s, 2.05 GB |
  | 5 files, 72 changed lines | timed out at 900 s, 1.18 GB |
  | 140 files | timed out at 1,200 s, 1.36 GB |

- Stack sampling of the 5-file run (30 of 30 samples, symbolized build) sits in
  `ReachSweep::bfs_reaches_owner_uncached`, reached from
  `find_transitive_witness` via `apply_rust_no_static_path_limit`. Per
  `no_static_path` finding the analyzer rebuilds the test and production
  function lists and runs a depth-5 forward breadth-first search from every
  distinct callee of every test; common names (`new`, `get`, `len`) fan out
  across most of the graph.
- `pilot --root` completes after the default 30 s budget fires and retries
  (227 s, 2.0 GB) and analyzes 2,000 of at least 10,000 seams; the
  `repo_seam_limit_applied` limitation discloses this.

### Not measured

Repositories above about 40,000 files other than rust-lang/rust, parallel
runs, Windows and macOS, warm-cache behavior, `deep` mode on rust-lang/rust,
and the 140-file diff to completion.
