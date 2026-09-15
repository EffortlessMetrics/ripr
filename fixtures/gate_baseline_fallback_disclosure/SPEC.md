# Fixture: gate_baseline_fallback_disclosure

Spec: RIPR-SPEC-0014

## Given

A `baseline-check` gate run whose explicit baseline contains legacy
`path:line:static_class` fallback identities, and current PR guidance whose
candidates share an anchor with those entries. The `expected/gate-baseline/`
corpus holds five adversarial scenarios (issue #1934): a fallback-only match
for a genuinely new canonical gap, a line-moved candidate whose canonical
identity still matches, two gaps on one line with the same static class, a
candidate with no canonical identity, and a baseline mixing canonical and
legacy entries. The top-level check golden uses a comment-only diff so the
fixture stays a cheap BDD control for the corpus contract.

## When

```bash
cargo xtask fixtures gate_baseline_fallback_disclosure
```

or:

```bash
ripr check --root fixtures/gate_baseline_fallback_disclosure/input --diff fixtures/gate_baseline_fallback_disclosure/diff.patch --mode fast
```

The gate scenarios are exercised by
`crates/ripr/src/output/gate.rs::tests::baseline_fallback_disclosure_fixture_matrix_matches_checked_outputs`,
which renders `ripr gate evaluate --mode baseline-check` for each scenario and
compares against the checked-in `gate-decision.json` / `gate-decision.md`.

## Then

- Every fallback-only baseline match keeps `is_baseline_new = false` during
  the legacy compatibility window, emits a report-level warning naming the
  candidate and the matched legacy identity, and records
  `baseline_match_kind: "legacy_path_line_class"` on the decision payload.
- Canonical identity matches record no `baseline_match_kind` and emit no
  fallback warning.
- `ripr check` reports no changed behavior for the comment-only diff.

## Must Not

- Treat a fallback-only match as a canonical identity match, or let it stay
  silent.
- Flip a fallback-only match to `is_baseline_new = true` (the deferred
  deprecation question).
- Use mutation-runtime outcome vocabulary reserved for real mutation
  execution.
- Present a legacy compatibility match as reviewed canonical debt.
