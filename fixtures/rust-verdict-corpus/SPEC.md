# Fixture Corpus: rust-verdict-corpus

Spec: RIPR-SPEC-0219

## Given

Pinned excerpts of real Rust crates (serde, regex-syntax, semver, hex,
itoa, bytesize, rusqlite, strsim, atuin; semver and bytesize at two pins) under
`subjects/`, byte-identical to their upstream commits
with license files (Rust sources stored as `.rs.txt`), small authored
crates written to fill cells the real crates leave empty (`authored-pricing`,
`authored-ledger` and `authored-config` for verdict and probe-family cells;
`authored-accounts`, `authored-checkout`, `authored-tokens`, `authored-shop`
and `authored-roles` for test shapes other RIPR specs define, each case naming
its specs in its reasoning; one `authored-specNNNN-<k>` crate per acceptance
example of RIPR-SPEC-0225 to 0228, isolated so no other example's test relates
to its owner), and one-line edits under `cases/`. Each case is labeled
with what the crate's own test suite discriminates, established by running
the listed mutants of the edited expression against the full pinned
checkout (for an authored crate, the whole stored crate).

## When

`cargo xtask verdict-corpus check` applies each edit to a run-owned copy of
its subject, runs `ripr check --json`, and projects the anchored findings to
one verdict.

## Then

Each case scores as ideal, abstained, false actionable, false exposed, or
false silent against its label; contradictions inside ripr's own output are
counted; authored rates are reported apart from upstream rates; and the
report must equal `expected/report.json`.

## Must Not

- Run mutation testing, `cargo test`, or network access.
- Treat the rates as a population estimate.
- Edit a retained subject file; a changed byte fails its sha256.

## Refreshing

When a ripr change moves a verdict, `check` fails and names the first
differing line. Read `target/ripr/reports/verdict-corpus/report.md`. A row
marked `changed_since_labeling` must be re-checked against the full pinned
checkout (for an authored crate, the stored crate itself) before the expected report is refreshed with
`cargo xtask verdict-corpus report --out fixtures/rust-verdict-corpus/expected`.
