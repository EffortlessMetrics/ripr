# Dogfooding

Dogfooding means using `ripr` on this repository to keep the product honest. It
should produce focused evidence, not broad self-analysis dashboards.

## Current Useful Commands

```bash
cargo run -p ripr -- --version
cargo run -p ripr -- doctor
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff --json
cargo run -p ripr -- explain --diff crates/ripr/examples/sample/example.diff probe:crates_ripr_examples_sample_src_lib.rs:21:error_path
cargo run -p ripr -- context --diff crates/ripr/examples/sample/example.diff --at probe:crates_ripr_examples_sample_src_lib.rs:21:error_path --json
```

## Dogfooding Rules

- Prefer sample diffs and fixtures over broad repository scans.
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
