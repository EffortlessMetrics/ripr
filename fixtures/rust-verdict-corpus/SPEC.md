# Fixture Corpus: rust-verdict-corpus

Spec: RIPR-SPEC-0219

## Given

Pinned excerpts of real Rust crates (serde, regex-syntax, semver, hex,
itoa, bytesize) under `subjects/`, byte-identical to their upstream commits
with license files (Rust sources stored as `.rs.txt`), and one-line edits
under `cases/`. Each case is labeled
with what the crate's own test suite discriminates, established by running
the listed mutants of the edited expression against the full pinned
checkout.

## When

`cargo xtask verdict-corpus check` applies each edit to a run-owned copy of
its subject, runs `ripr check --json`, and projects the anchored findings to
one verdict.

## Then

Each case scores as ideal, abstained, false actionable, false exposed, or
false silent against its label; contradictions inside ripr's own output are
counted; and the report must equal `expected/report.json`.

## Must Not

- Run mutation testing, `cargo test`, or network access.
- Treat the rates as a population estimate.
- Edit a retained subject file; a changed byte fails its sha256.

## Refreshing

When a ripr change moves a verdict, `check` fails and names the first
differing line. Read `target/ripr/reports/verdict-corpus/report.md`. A row
marked `changed_since_labeling` must be re-checked against the full pinned
checkout before the expected report is refreshed with
`cargo xtask verdict-corpus report --out fixtures/rust-verdict-corpus/expected`.
