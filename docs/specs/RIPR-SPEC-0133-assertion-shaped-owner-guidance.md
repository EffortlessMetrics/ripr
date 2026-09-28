# RIPR-SPEC-0133: Assertion-Shaped Owner Guidance

Status: accepted

Owner: product / swarm

Created: 2026-07-21

Linked issues:

- [#2131](https://github.com/EffortlessMetrics/ripr-swarm/issues/2131) -
  guidance for assertion-shaped owners is incoherent (split out of #2130).

Support-tier impact:

- No tier change. This spec changes only the advisory text of the
  `recommended_next_step` field for findings whose changed owner is an
  assertion-shaped helper, plus one additive `Finding.evidence` disclosure
  line on those findings. It does not change any classification, finding-set,
  ExposureClass, probe family, confidence score, `repair_packet_ready`
  authority, or pass/fail behavior.
- The guidance is advisory text; changing its wording is not a schema change.
  No schema_version bump is required.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

## Problem

RIPR's guidance presumes the changed owner is code *under* test. When the
changed owner is itself an assertion routine, the roles invert and the
generated advice inverts with them (issue #2131, observed on
`EffortlessSteven/hawk@a83da49`):

- `no_static_path` said "add a co-located test that reaches and observes the
  changed owner" — asking for a test of the oracle itself (infinite regress).
- `weakly_exposed` said "Replace broad assertions with exact equality" — fired
  on a boolean invariant where exact equality is unavailable, and an
  exact-equality assertion already existed lines earlier in the same function.
- `infection_unknown` suggested teaching ripr about "the fixture/builder" when
  there was none.

## Behavior

### Detection (assertion-shaped owner)

A changed owner is *assertion-shaped* when both rules hold:

1. **Dominance rule**: the owner function's body contains at least one
   assert-family macro (`assert!`, `assert_eq!`, `assert_ne!`,
   `debug_assert!`, `debug_assert_eq!`, `debug_assert_ne!`) or `.expect(`
   call, and every non-declaration (`let`), non-control-flow statement in the
   body is one of those. Statements the text scanner cannot recognize count
   against dominance. `assert_matches!` and snapshot macros are deliberately
   out of scope (they leave an unrecognized statement, which blocks the
   detection).
2. **Caller rule**: no non-test function in the index calls the owner.
   Bare-name call matching can collide with a different same-named function,
   but a collision only *blocks* the reframe — the fail-closed direction.

The rule is explainable in one sentence, shared as the
`ASSERTION_SHAPED_OWNER_REASON` const: "its body is dominated by
assert*/expect calls and nothing outside tests calls it".

### Reframed guidance (class unchanged)

The exposure class is NEVER changed by this detection. Only the
`recommended_next_step` text is reframed for oracle-shaped owners, defined as
`pub(crate) const`s in `crates/ripr/src/domain/classification.rs` next to
`NO_STATIC_PATH_NEXT_STEP`:

- `NoStaticPath` → `ASSERTION_SHAPED_NO_STATIC_PATH_NEXT_STEP`: name that the
  owner is itself an assertion helper, so a test that observes it would be a
  test of the oracle; sharpen its assertions or exercise it indirectly through
  the code it checks.
- `WeaklyExposed` → `ASSERTION_SHAPED_WEAKLY_EXPOSED_NEXT_STEP`: exact-equality
  advice for code under test may not apply (boolean invariants have no exact
  equality); tighten the loosest assertion in the helper.
- `ReachableUnrevealed` → `ASSERTION_SHAPED_REACHABLE_UNREVEALED_NEXT_STEP`:
  the helper is the observation, not the code under observation; ensure at
  least one test calls it over inputs that exercise the changed check.
- `InfectionUnknown` → `ASSERTION_SHAPED_INFECTION_UNKNOWN_NEXT_STEP`: there is
  no fixture/builder for ripr.toml to describe; add a boundary or negative-path
  case for the code this helper checks.
- `Exposed` stays `None`; `PropagationUnknown`/`StaticUnknown` keep the
  standard escalation text ("escalate to real mutation testing" is coherent
  for an oracle).

Each reframed string embeds `ASSERTION_SHAPED_OWNER_REASON` verbatim so the
rule is stated in output; a unit test pins that parity.

### Disclosure

Findings on assertion-shaped owners gain one additive evidence line:

```text
owner_shape: assertion_shaped (its body is dominated by assert*/expect calls and nothing outside tests calls it)
```

so JSON and human projections show why the guidance is phrased for an oracle.
No new JSON field is added; there is no schema_version bump. All output
surfaces continue to consume `Finding.recommended_next_step` through
`output::next_step::reconcile_next_step` — no renderer forks.

### What is NOT changing

- `ExposureClass` values, labels, or severities; classification logic; probe
  generation; confidence scoring.
- `repair_packet_ready` or any actionability authority (the shared validator
  remains the only flip point; this spec touches advisory text only).
- Guidance for owners that are not assertion-shaped.
- Whether test files are analyzed (that is #2130's scoping question; this spec
  stands either way).
- Any JSON schema field names or types; pass/fail exit codes.

## Non-Goals

- Skipping or reclassifying RIPR gap classes for assertion-shaped owners.
- Detecting oracle-shaped code in languages other than Rust.
- Broadening the assert family (e.g. `assert_matches!`, snapshot macros).
- Changing whether `tests/` diffs produce probes.

## Acceptance Examples

1. **Hawk shape**: an assertion helper in a `src/` helper module (mixed
   `assert!` boolean invariant + `assert_eq!` exact equality), called only by
   tests. Guidance is reframed; evidence carries the `owner_shape` line.
   Fixture: `fixtures/assertion_shaped_oracle_test_file`.
2. **`#[cfg(test)]` module helper**: an assertion helper in a production file
   behind `#[cfg(test)]`. Same reframe.
   Fixture: `fixtures/assertion_shaped_oracle_cfg_test`.
3. **Control — normal production owner**: guidance unchanged, no `owner_shape`
   line. Fixture: `fixtures/assertion_shaped_control_production_owner`.
4. **Control — production caller**: an assert-heavy fn with a non-test caller
   is NOT assertion-shaped. Fixture:
   `fixtures/assertion_shaped_control_production_caller`.
5. **Golden blast radius**: no pre-existing fixture changes classification,
   finding set, or guidance; only the four new fixtures are added.
   `cargo xtask goldens check` passes without re-blessing existing fixtures.

## Required Evidence

- `ASSERTION_SHAPED_*` consts defined in `domain/classification.rs`,
  re-exported from `domain/mod.rs`.
- Detector in `analysis/classify/owner_shape.rs`; verdict threaded through
  `ProbeContext.owner_assertion_shaped` from `classifier::classify_probe` (the
  only place the full index is available).
- `decision::recommended_next_step` reframes per class; `PropagationUnknown` /
  `StaticUnknown` / `Exposed` keep standard behavior.
- Four new fixtures blessed with a reason citing this spec.
- `cargo xtask goldens check` clean with zero drift on pre-existing fixtures.
- `cargo xtask fixtures` clean.
- `cargo xtask check-evidence-promotion-honesty` clean (no class flips, so no
  promotion-honesty impact).

## Test Mapping

- `crates/ripr/src/analysis/classify/owner_shape.rs::tests` — dominance rule,
  caller rule, word-boundary, comment/string stripping, fail-closed cases.
- `crates/ripr/src/analysis/classify/decision.rs::tests::recommended_next_step_reframes_guidance_for_assertion_shaped_owners`
  — every class maps to the reframed const or keeps standard behavior.
- `crates/ripr/src/analysis/classify/decision.rs::tests::assertion_shaped_guidance_states_the_rule_and_avoids_test_of_test_advice`
  — each reframed string embeds the one-sentence rule and never asks for a
  co-located test of the oracle.
- `crates/ripr/src/analysis/classifier.rs::tests::given_assertion_shaped_owner_when_classifying_then_guidance_is_reframed_not_reclassified`
  — end-to-end through `classify_probe`: class kept, guidance reframed,
  `owner_shape` evidence line present.
- `crates/ripr/src/analysis/classifier.rs::tests::given_assertion_heavy_owner_with_production_caller_when_classifying_then_guidance_is_standard`
  — the caller rule blocks the reframe end-to-end.

## Implementation Mapping

| Component | Location |
|---|---|
| Guidance consts + shared reason | `crates/ripr/src/domain/classification.rs` |
| Re-export | `crates/ripr/src/domain/mod.rs` |
| Detector | `crates/ripr/src/analysis/classify/owner_shape.rs` |
| Reframed next-step decision | `crates/ripr/src/analysis/classify/decision.rs` |
| Context flag | `crates/ripr/src/analysis/classify/context.rs` |
| Detection call site | `crates/ripr/src/analysis/classifier.rs` |
| Finding build + evidence line | `crates/ripr/src/analysis/classifier/finding.rs` |
| Golden fixtures | `fixtures/assertion_shaped_*/expected/{check.json,human.txt}` |

## CI Proof

- `cargo test -p ripr` — all pass including the new tests.
- `cargo clippy -p ripr --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- `cargo xtask check-static-language` pass (conservative vocabulary only).
- `cargo xtask check-no-panic-family` pass.
- `cargo xtask check-fixture-contracts` pass.
- `cargo xtask check-traceability` pass.
- `cargo xtask check-evidence-promotion-honesty` pass.
- `cargo xtask goldens check` clean (no pre-existing fixture drift).
- `cargo xtask fixtures` clean.
- `cargo xtask dogfood` clean.

## Metrics

- Gate: 0 golden drift on pre-existing fixtures; 4 new fixtures added.
- Gate: 0 classification changes attributable to the detection (guidance-only).
