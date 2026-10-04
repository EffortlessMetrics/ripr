# RIPR-SPEC-0199: Rust judged-panel analyzer feedback ledger

Status: proposed

Owner: product / swarm

Created: 2026-09-29

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- [#4796](https://github.com/EffortlessMetrics/ripr-swarm/issues/4796) implements the feedback ledger
- [#3806](https://github.com/EffortlessMetrics/ripr-swarm/issues/3806) supplies immutable terminal judgments
- [#4795](https://github.com/EffortlessMetrics/ripr-swarm/issues/4795) owns runtime calibration and is not absorbed
- [#3164](https://github.com/EffortlessMetrics/ripr-swarm/issues/3164) remains the parent programme

Linked PRs:

- None yet

Support-tier impact:

- None. The ledger is a metrics sidecar. Support-tier and capability checks
  must not consume it more strongly than its judged denominator.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- Retain `metrics/rust-judged-behavior-panel/feedback-ledger.json` under the
  existing judged-panel JSON allowlist.
- No analyzer, GitHub, calibration, support, or release mutation.

## Problem

Independently adjudicated Rust failures were not durable, owned, or
regression-tracked. Confirmed false-exposed, false-actionable, under-credit,
wrong-target, and incorrect-limitation families could disappear after a later
repair, or be forced into the wrong defect class by nearby targets, human
prose, or runtime calibration.

## Behavior

`cargo xtask rust-judged-panel check` validates one retained feedback ledger
bound to the #3806 judgment packet by digest. Every terminal judged case has
exactly one row. Failure direction is derived from immutable labels:

```text
false_exposed
false_actionable
incorrect_limitation
static_under_credit
wrong_target (only when no stronger label fires and an exact target is bound)
inconclusive_no_feedback
instrument_or_case_defect
no_confirmed_failure
```

Closed repair states are `open`, `candidate_in_review`,
`repaired_pending_replay`, and `closed_with_replay`. Closure requires a merged
implementation identity and original-case replay. A fixture pass without that
replay stays `repaired_pending_replay`. Wrong-target closure cannot succeed on
a nearby observer identity.

Confirmed defects that cannot retain the exact mechanism as a producer-path
honesty fixture stay `replay_only` with a named materialization or
authorization boundary. The ledger records owner-search receipts and reuses a
focused owner when one exists. It does not create, assign, close, or label
GitHub objects.

`cargo xtask rust-judged-panel feedback [--out] [--check]` derives JSON and
Markdown from one DTO. Reports carry counts by direction, status, reduction,
and owner class. They carry no overall analyzer score. Runtime calibration
status is `#4795` metadata (`not_run` or `unavailable` while that producer's
release-challenge receipts stay unauthorized) and cannot set the static class.

## Non-Goals

- Analyzer family repairs
- Automatic GitHub issue or PR mutation
- Judgment or runtime-result rewrite
- #4795 calibration execution or scorecard
- #3164 parent closure, support promotion, or release membership
- Route-yield denominator merger with #3076
- Honesty-corpus writes from this command

## Required Evidence

- Discriminating tests named `rust_analysis_feedback_*` cover false-exposed vs
  false-actionable, under-credit vs support promotion, wrong-target identity,
  limitation/inconclusive non-defects, duplicate and existing-owner reuse,
  extra or missing rows, anti-hardcode fixture ids, repaired-without-replay,
  stale digest, row-order stability, denominator change on deletion, human
  notes that cannot strengthen a class, and calibration that cannot set the
  static class.
- Production `feedback-ledger.json` validates against the current #3806
  packet.
- JSON and Markdown reports agree on every count, including family and owner
  denominators.

## Inputs

- `metrics/rust-judged-behavior-panel/release-judgments.json`
- `metrics/rust-judged-behavior-panel/feedback-ledger.json`

## Outputs

- Validation on `cargo xtask rust-judged-panel check`
- `target/ripr/rust-judged-panel/feedback/feedback.json`
- `target/ripr/rust-judged-panel/feedback/feedback.md`

## Acceptance Examples

1. A confirmed false-exposed row cannot be classified false-actionable.
2. Closing a repaired row without original-case replay is rejected.
3. Reordered ledger rows emit byte-identical reports.
4. Deleting one unfavorable row changes the defect denominator.
5. Notes claiming false-exposed cannot reclassify an inconclusive judgment.

## Test Mapping

- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_false_exposed_is_not_false_actionable`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_under_credit_is_not_support_promotion`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_wrong_target_requires_exact_identity`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_wrong_target_cannot_close_on_a_nearby_target`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_correct_limitation_is_not_an_analyzer_defect`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_duplicate_owner_is_detected`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_existing_owner_cannot_be_left_unowned`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_incorrect_limitation_cannot_close_as_accepted`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_fixture_hardcoded_to_case_id_is_rejected`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_repaired_fixture_without_original_replay_stays_pending`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_stale_judgment_digest_invalidates_closure`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_reordered_rows_make_byte_stable_reports`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_deleting_an_unfavorable_row_changes_the_denominator`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_human_notes_cannot_strengthen_inconclusive_or_limitation`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_extra_row_is_not_a_judged_case`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_recorded_calibration_belongs_to_4795`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_production_ledger_covers_every_judged_case`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/tests.rs::rust_analysis_feedback_production_reports_carry_the_judged_denominator`

## Implementation Mapping

- `xtask/src/rust_judged_panel/rust_analysis_feedback.rs`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/schema.rs`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/lifecycle.rs`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/owners.rs`
- `xtask/src/rust_judged_panel/rust_analysis_feedback/report.rs`
- `metrics/rust-judged-behavior-panel/feedback-ledger.json`

## Metrics

- `rust_judged_panel_feedback_rows`
- `rust_judged_panel_feedback_analyzer_defects`
- `rust_judged_panel_feedback_unowned`

## Failure Modes

- Stale judgment digest
- Missing row for a judged case
- Human prose or calibration setting static class
- Closure without merged repair and original-case replay
- Nearby-target closure of a wrong-target row
