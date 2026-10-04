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
cargo run -p ripr -- explain --diff crates/ripr/examples/sample/example.diff probe:crates_ripr_examples_sample_src_lib.rs:error_path:c1a03250
cargo run -p ripr -- context --diff crates/ripr/examples/sample/example.diff --at probe:crates_ripr_examples_sample_src_lib.rs:error_path:c1a03250 --json
```

`cargo xtask dogfood` is the stable advisory loop. It runs `ripr check --mode
fast` against checked fixture diffs, writes actual outputs under
`target/ripr/dogfood/`, and writes `target/ripr/reports/dogfood.md` plus
`target/ripr/reports/dogfood.json`. The command exits non-zero whenever the
report status is `warn` (any scenario recorded errors). It also checks repo-local finding-alignment
receipts under `fixtures/finding-alignment-dogfood/` so real RIPR PR examples
preserve the Lane 1 split between raw findings, canonical evidence items, and
actionable canonical gaps. Python repair-routing eval receipts live under
`fixtures/python-real-repo-evals/`; they record curated scratch or real-repo
repair-card, verify, and outcome evidence without promoting Python beyond its
preview/advisory boundary.
Bun UB cross-language witness receipts live under
`fixtures/bun-ub-cross-language-dogfood/`; they record calibrated Blob /
ArrayBuffer TypeScript preview receipts without running Bun or producing repair
packets.

## First-Run Walk

`cargo xtask first-run` replays a new developer's first hour on pinned crates
ripr has not been tuned on (`semver 1.0.23`, `fastrand 2.3.0`,
`bytesize 1.3.0`). Cargo fetches the sources, so the walk adds no network
client. Each crate is cloned from a local `origin` onto a feature branch that
carries one recorded boundary edit. The edit must match exactly once on its
recorded line, so version drift fails the walk instead of changing the subject.

```bash
cargo xtask first-run --install-published       # install ripr from crates.io, then walk
cargo xtask first-run --ripr path/to/ripr       # walk a built binary (a release candidate)
```

Per crate it times `ripr doctor`, `ripr check`, `ripr check --format json`,
`ripr pilot --root .`, the `ripr explain` command `check` printed (run as
printed), `ripr init --ci github`, and a second `ripr doctor`. The report is
written to `target/ripr/first-run/first-run.{json,md}`, plus `first-run-rows.jsonl`: one `first_run_row.v1` object per metric (secs, exit, stdout_lines, friction_count, workflow_lines, verdict) for a scoreboard to gate on. A previous walk's directory is replaced; an `--out` the walk did not create is
refused. `--ripr` defaults to the `ripr` on `PATH`.

The walk observes and does not gate. It flags friction: a nonzero exit, stderr
beyond progress lines, output over a read budget, no next step after `check`, a
step over its time budget, and a generated workflow that is over 1,500 lines or
compiles ripr from source on every run. It records the static verdict class each
release produces for each edit, so a verdict change between releases is visible.
Whether a verdict is correct is a separate question for a hand-labeled corpus,
not this walk.

Run it against the published release and the release candidate, and compare the
two JSON files. It needs registry access and Git.

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
