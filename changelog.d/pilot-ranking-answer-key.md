<!-- section: Added -->
- `cargo xtask pilot-ranking`: a checked-in answer key for `ripr pilot`'s top
  picks. Five small crates are pinned to exact commits with the cargo-mutants
  outcome of each of their 1,647 viable mutants, so `score` reports top-5 and
  top-10 precision, scored share and distinct functions in seconds without
  rerunning cargo-mutants. A new advisory `ranking` scoreboard lane compares
  the pooled numbers, the confirmed and refuted counts and the pick count with
  a committed baseline nightly, on demand, and when a pull request that
  touches pilot ranking or seam grading leaves draft. Labels must
  come from a full cargo-mutants run, checked against an unfiltered
  `cargo mutants --list`.
