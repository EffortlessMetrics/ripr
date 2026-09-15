# Gate baseline fallback disclosure corpus

Adversarial `baseline-check` scenarios for issue #1934 (RIPR-SPEC-0014 §
Baseline Comparison). Each scenario directory holds a `pr-guidance.json` input,
a `baseline.json` input, and the checked `gate-decision.json` /
`gate-decision.md` outputs rendered by `ripr gate evaluate --mode
baseline-check`.

The corpus is exercised by
`crates/ripr/src/output/gate.rs::tests::baseline_fallback_disclosure_fixture_matrix_matches_checked_outputs`.

Scenarios:

- `fallback-only-new-canonical` — same path/line/static_class as a stale
  baseline entry but a genuinely different canonical gap: the fallback-only
  match keeps `is_baseline_new = false` during the compatibility window and
  must be disclosed (warning + `baseline_match_kind`).
- `line-moved-canonical-match` — the line moved but the canonical identity is
  unchanged: a canonical match, so no warning and no `baseline_match_kind`.
- `two-gaps-one-line-same-class` — two gaps share one line and static class:
  the canonical match stays clean; the fallback-only collision is disclosed.
- `missing-canonical-identity` — the candidate carries no canonical or seam
  identity: the fallback-only match is disclosed.
- `mixed-canonical-and-legacy-entries` — a baseline mixing canonical and
  legacy entries: only the legacy fallback match is disclosed.
