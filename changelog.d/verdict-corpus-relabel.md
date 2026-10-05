<!-- section: Added -->
- Verdict corpus: `cargo xtask verdict-corpus relabel` replays a case's
  mutants against its own `cargo test` command, on the toolchain it was
  labeled on, and fails when an outcome, failing test, or derived truth
  drifts from the label, a mutant does not compile or changes nothing,
  repeated runs disagree, or the labeled toolchain is missing.
  `--sample <n> --seed <s>` picks a deterministic subset. Every rewrite
  mutant now carries `mutated_line`, the exact anchor line it replays, and
  `failing_test` must be one test name. Replays of all 203 cases
  reproduced every label; the bytesize master@66a3715 (2.7.0) cases now
  skip a quickcheck property that fails on random inputs regardless of the
  edit. Replays run cargo offline and refuse test commands or checkout
  symlinks that would leave the run-owned tree.
