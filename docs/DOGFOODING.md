# Dogfooding

Dogfooding means using `ripr` on this repository to keep the product honest. It
should produce focused evidence, not broad self-analysis dashboards.

## Current Useful Commands

```bash
cargo xtask dogfood
cargo run -p ripr -- --version
cargo run -p ripr -- doctor
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff --json
cargo run -p ripr -- explain --diff crates/ripr/examples/sample/example.diff probe:crates_ripr_examples_sample_src_lib.rs:21:error_path
cargo run -p ripr -- context --diff crates/ripr/examples/sample/example.diff --at probe:crates_ripr_examples_sample_src_lib.rs:21:error_path --json
```

`cargo xtask dogfood` is the stable advisory loop. It runs `ripr check --mode
fast` against checked fixture diffs, writes actual outputs under
`target/ripr/dogfood/`, and writes `target/ripr/reports/dogfood.md` plus
`target/ripr/reports/dogfood.json`. It also checks repo-local finding-alignment
receipts under `fixtures/finding-alignment-dogfood/` so real RIPR PR examples
preserve the Lane 1 split between raw findings, canonical evidence items, and
actionable canonical gaps. Python repair-routing eval receipts live under
`fixtures/python-real-repo-evals/`; they record curated scratch or real-repo
repair-card, verify, and outcome evidence without promoting Python beyond its
preview/advisory boundary.

## Dogfooding Rules

- Prefer sample diffs and fixtures over broad repository scans.
- Treat repo-wide RIPR refreshes as build-heavy on this repo. Use
  `repo-badge-json`, generated receipts, or an explicit gap ledger for ordinary
  summary counts. Use `cargo xtask repo-exposure-summary-report` when local
  planning needs bounded repo exposure counts, and run only one no-ledger repo
  scan at a time. If it emits `basis: "limited_runtime_status"` with
  `runtime_status.downstream_consumable: false`, do not use that artifact as a
  repair queue or badge basis.
- Do not use full `repo-exposure-json` as the normal badge, receipt, top-file,
  or packet-queue input. Full exposure dumps require explicit operator intent
  and cleanup after inspection.
- Raise `RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS` only for the command that needs
  a large-repo cache write, after checking disk headroom.
- When `ripr` finds a real gap in its own code, add a fixture or regression
  test before changing the analyzer.
- Do not use `ripr` findings as blocking CI until the SARIF policy and
  calibration work lands.
- Record useful findings in [Learnings](LEARNINGS.md) when they change how the
  project should be built.

## Planned Dogfood Loop

After the fixture lab and evidence output exist:

```text
make code change
-> run ripr against the diff
-> inspect finding evidence
-> add targeted test or document static_unknown stop reason
-> keep fixture/golden output aligned
```

The goal is to keep the analyzer grounded in real developer workflows while
still respecting the product boundary: static evidence guides, real mutation
confirms later.
