# RIPR-SPEC-0109: Confidence Min-Cap by Weakest Stage

Status: accepted

Owner: product / swarm

Created: 2026-06-14

Linked issues:

- #1219 (part D — confidence-display honesty)

Linked PRs:

- None yet

Support-tier impact:

- Confidence display only: no classifier behavior change; no new output field;
  no schema bump; no version bump. The `confidence` field is an existing f32 in
  the JSON output. Values for `*_unknown` findings drop toward their honest
  ceiling; `exposed` and `weakly_exposed` findings with all-Yes/Medium stages
  remain at their current scores (unchanged). Tier labels and claim boundaries
  remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Single function change (`confidence_score` in
  `crates/ripr/src/analysis/classify/decision.rs`).
- No new crates, binaries, LSP servers, output schema fields, or classifier
  behavior changes.
- Seven fixtures re-blessed (confidence numbers only; classifications
  byte-identical).

## Problem

### The residual after #1232

#1232 closed the exposed-inflation bug: `classify` now short-circuits any
`Unknown`/`Opaque` infect or propagate stage to `*_unknown` before reaching the
`Exposed` arm, so no `exposed` finding can carry an unproven stage.

The residual is a display-only honesty gap in `confidence_score`. That function
computes the aggregate score purely from per-stage `StageState` weights
(`Yes=0.20, Weak=0.12, Unknown=0.07, ...`) and never reads
`StageEvidence.confidence` (the `Confidence` enum). As a result:

- A `propagation_unknown` finding with `propagate.state=Unknown,
  propagate.confidence=Low` reports an aggregate confidence of `0.79` — because
  the four other stages are `Yes/Weak` and their state weights dominate.
- A `infection_unknown` finding with `infect.confidence=Low` can report `0.87`
  for the same reason.
- An `*_unknown` finding with `infect.confidence=Unknown,
  propagate.confidence=Low` can report `0.61` while its stages are explicitly
  flagged at `Unknown`/`Low`.

None of these are false classifications (the class is correct). The headline
confidence number overstates certainty relative to ripr's own per-stage markers.

### This is NOT a fail-open

The confidence field is advisory — it is not used by `classify`. No finding is
promoted or demoted. This spec closes a display-only honesty gap.

## Behavior

### The min-cap

Apply a ceiling to the aggregate `confidence_score` determined by the weakest
contributing stage's per-stage `Confidence` marker, **after** all existing score
arithmetic (including the `+0.15` bump for `NoStaticPath`/`ReachableUnrevealed`).
The cap can only lower the score, never raise it.

**Ceilings:**

| `Confidence` variant | Ceiling |
|---|---|
| `High` | `1.0` (no cap) |
| `Medium` | `1.0` (no cap) |
| `Low` | `0.66` |
| `Unknown` | `0.50` |

`High` and `Medium` both ceiling at `1.0` so genuine all-Yes/Medium exposures
are not suppressed.

**Algorithm:**

```
cap = min over {reach, infect, propagate, observe, discriminate} of ceiling(stage.confidence)
return min(existing_score, cap)
```

No other changes: `classify` is untouched, no new stage states, no schema bump.

### Implementation

Single new private function `confidence_ceiling(c: &Confidence) -> f32` and a
one-line application of `f32::min(cap)` at the end of `confidence_score` in
`crates/ripr/src/analysis/classify/decision.rs`. The `Confidence` enum is
reused from `crate::domain::evidence` — not forked.

## Controls

Three unit tests in `decision.rs`:

**(a) Genuine exposure unchanged** — all five stages `Yes`/`Medium` →
`confidence_score == 1.0`. The cap is `1.0` (all Medium → `1.0` ceiling), so
the score is unchanged. This is the must-not-over-correct guard.

**(b) Unknown stage capped** — propagate stage `Unknown`/`Low` with four other
stages `Yes`/`Weak`/`Medium` → `confidence_score ≤ 0.66`. The `Low` ceiling
fires. Class stays `PropagationUnknown` (verified via `classify`).

**(c) #1232 no-regression** — `classify` called with `infect.state=Unknown`
(Low confidence) → `InfectionUnknown`; with `propagate.state=Unknown` (Low
confidence) → `PropagationUnknown`. Neither classification changes. Only the
numeric confidence output changes.

## Golden Re-Bless Summary

Seven fixtures re-blessed (confidence number only; `classification` field
byte-identical in all cases):

| Fixture | Before | After | Classification (unchanged) |
|---|---|---|---|
| `infect_value_returned` (static_unknown finding) | `0.61` | `0.50` | `static_unknown` |
| `infect_wildcard_discard` (propagation_unknown finding) | `0.79` | `0.66` | `propagation_unknown` |
| `observation_unverified_field_construction` | `0.79` | `0.66` | `propagation_unknown` |
| `observation_verified_field_construction` | `0.87` | `0.66` | `propagation_unknown` |
| `observation_verified_return_value` (static_unknown finding) | `0.61` | `0.50` | `static_unknown` |
| `opaque_fixture_builder` | `0.87` | `0.66` | `infection_unknown` |
| `propagate_swallowed_ok` | `0.79` | `0.66` | `propagation_unknown` |

The `exposed` and `weakly_exposed` findings in mixed fixtures (e.g.
`infect_value_returned` contains both an `exposed` and a `weakly_exposed`
finding) are unaffected — their stages carry `Medium` confidence and the cap is
`1.0`.

## Required Evidence

### Unit tests (decision.rs)

Three new controls added to
`crates/ripr/src/analysis/classify/decision.rs::tests`:

| Test | Control |
|---|---|
| `confidence_score_genuine_exposure_all_yes_medium_is_not_capped` | (a) must-not-over-correct |
| `confidence_score_low_propagate_confidence_caps_score_to_0_66` | (b) Low stage capped |
| `confidence_score_cap_does_not_change_classification` | (c) #1232 no-regression |

### Re-blessed fixtures (×7)

| Fixture | Before | After | Classification |
|---|---|---|---|
| `infect_value_returned` (static_unknown) | `0.61` | `0.50` | `static_unknown` (unchanged) |
| `infect_wildcard_discard` (propagation_unknown) | `0.79` | `0.66` | `propagation_unknown` (unchanged) |
| `observation_unverified_field_construction` | `0.79` | `0.66` | `propagation_unknown` (unchanged) |
| `observation_verified_field_construction` | `0.87` | `0.66` | `propagation_unknown` (unchanged) |
| `observation_verified_return_value` (static_unknown) | `0.61` | `0.50` | `static_unknown` (unchanged) |
| `opaque_fixture_builder` | `0.87` | `0.66` | `infection_unknown` (unchanged) |
| `propagate_swallowed_ok` | `0.79` | `0.66` | `propagation_unknown` (unchanged) |

## Non-Goals

- Does NOT change any classification (`classify` is untouched).
- Does NOT add new `StageState`, `Confidence`, or `ExposureClass` variants.
- Does NOT bump `schema_version` or crate version (confidence is an existing
  float field; additive semantics do not require a schema bump).
- Does NOT re-open the exposed-inflation question (closed by #1232).
- Does NOT touch release workflows or publish the crate.
- Does NOT run mutants.
- Static-language clean: `confidence_ceiling` is a private helper; no new
  static-output vocabulary.

## Acceptance

1. `cargo xtask goldens check` passes (re-blessed).
2. `cargo xtask fixtures` passes (no new fixture drift).
3. `cargo test -p ripr confidence_score` — all four tests pass (1 pre-existing
   + 3 new controls).
4. `cargo xtask check-evidence-promotion-honesty` passes — no charter fixture's
   `classification` changed; confidence-only drift does not trip this gate.
5. Full check-* gate loop passes (check-static-language, check-output-contracts,
   check-traceability, check-spec-format, check-spec-numbering, check-doc-index,
   check-support-tiers, etc.).
6. Behavioral: `ripr check --diff <exposed-diff> --json` → `classification:
   exposed, confidence: 1.0`; `ripr check --diff <unknown-propagate-diff>
   --json` → `classification: propagation_unknown, confidence: ≤ 0.66`.

## Acceptance Examples

### Genuine exposure — confidence unchanged

```
classification: exposed
confidence: 1.0
```

All stages `Yes`/`Medium`; cap = `1.0`; score unchanged.

### Unknown-propagate finding — confidence capped

```
classification: propagation_unknown
confidence: 0.66
```

`propagate.state=Unknown, propagate.confidence=Low`; raw score `0.79`; cap
`0.66`; classification unchanged.

### Over-correct guard — must not fire for exposed

```
cargo test -p ripr confidence_score_genuine_exposure_all_yes_medium_is_not_capped
test analysis::classify::decision::tests::confidence_score_genuine_exposure_all_yes_medium_is_not_capped ... ok
```

## Test Mapping

| Test | Spec control |
|---|---|
| `confidence_score_genuine_exposure_all_yes_medium_is_not_capped` | Control (a): must-not-over-correct |
| `confidence_score_low_propagate_confidence_caps_score_to_0_66` | Control (b): Low stage capped |
| `confidence_score_cap_does_not_change_classification` | Control (c): #1232 no-regression |
| `confidence_score_handles_opaque_no_and_not_applicable_stage_states` | Pre-existing: all Medium → no cap |

## Implementation Mapping

| Component | Location |
|---|---|
| `confidence_ceiling` helper | `crates/ripr/src/analysis/classify/decision.rs` |
| `confidence_score` (cap applied) | `crates/ripr/src/analysis/classify/decision.rs` |
| Unit tests (3 new controls) | `crates/ripr/src/analysis/classify/decision.rs` |
| Re-blessed fixtures (×7) | `fixtures/*/expected/check.json` + `human.txt` |

## Metrics

- `confidence_min_cap_fixtures_reblessed`: 7 (number of fixtures whose
  confidence dropped due to the cap)
