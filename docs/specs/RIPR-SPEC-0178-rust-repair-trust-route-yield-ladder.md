# RIPR-SPEC-0178: Rust repair-trust route-yield ladder

Status: proposed

Owner: product / swarm

Created: 2026-09-29

Linked issues:

- [#4570](https://github.com/EffortlessMetrics/ripr-swarm/issues/4570) implements the reporting ladder
- [#3076](https://github.com/EffortlessMetrics/ripr-swarm/issues/3076) names the parent reporting contract

Support-tier impact:

- No support-tier or 0.11 promotion. The report remains `limited` while eligible
  attempts are below the governed threshold. Route yield is a denominator
  disclosure, not ordinary-user success.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`. No new policy ledger, corpus, or command.

## Problem

The governed Rust repair-trust reporter already validates
`metrics/rust-repair-trust/corpus.json` and preserves a zero eligible-attempt
denominator. It did not derive a route-yield ladder from typed observations, so
three observations with no complete route could disappear behind attempt-count
`N/A` or be misread as attempts.

## Behavior

`cargo xtask rust-repair-trust-report` continues to own corpus validation and
the JSON/Markdown report. It extends the existing observation rows with an
optional `route` object and derives, from rows only:

```text
selected opportunities
→ analysis completed
→ canonical behavior/gap identified
→ complete route admitted
→ attempt authorized/eligible
→ attempt started
→ attempt finished
→ static improved/closed
```

Counting rules:

- Unique selected opportunities are `(opportunity identity, cohort)`. Explicit
  `route.opportunity_id` wins; otherwise identity is repository + analyzed head
  + `canonical_candidate_id`. Missing `cohort_id` is the historical cohort.
  Eligible cases never mint an `attempt:` opportunity key. Unlinked cases bind
  to repository + analyzed head + `canonical_gap_id` in the historical cohort.
- A `repair_attempt_id` binds a case only when repository and analyzed head
  match. Cross-identity links are validation errors and cannot complete the
  naming observation.
- CLI and editor rows that share opportunity and cohort are channel evidence,
  not two opportunities. Focused-test execution is counted once per selected
  opportunity; conflicting channel results fail closed to `failed`.
- Distinct `cohort_id` values for the same opportunity remain separate
  experiments; later success does not rewrite the earlier result.
- A complete route requires `complete_route_admitted: true` and
  `canonical_eligibility` other than `rejected`. Packet-ready does not override
  a rejected eligibility.
- Eligible attempts remain counted cases. Observations are not attempts.
- Explicit downstream-true/upstream-false stage flags are rejected and do not
  enter trusted counts. Missing upstream facts stay unobserved rather than
  being manufactured. Ladder `static improved/closed` requires a finished
  attempt.
- Repair success uses improved-or-closed eligible attempts. When that
  denominator is zero the status is `not_measurable`, never `0%` or `100%`.
- Route yield uses complete routes over selected opportunities. Zero
  opportunities is `not_measurable`; zero complete routes over N observations
  is measured `0/N`.
- Completion without hidden help uses only complete routes with observed
  `artifact_archaeology`. Omitted archaeology is unknown, not help-free; the
  rate is `not_measurable` until at least one complete route records the
  field.
- Historical rows without `route` keep unknown finer stops as `not_observed`.
  The only exact legacy mapping is `analysis_timeout`.
  `static_limitation_no_repair_packet` is not relabelled as a missing
  discriminator or unsafe target.
- Hand-edited `route_yield`, `repair_success`, or `route_ladder` keys on the
  corpus cannot enter trusted counts.
- JSON and Markdown share one derived representation.

## Required Evidence

- Public reporter entry `rust_repair_trust_report_value_at` over temporary
  corpora, not only in-memory DTO construction.
- Discriminating controls for empty observations, three unrouted observations,
  live-corpus unique observations, complete route without attempt
  authorization, failed focused test plus improved static movement, CLI/editor
  channel pairing, distinct analyzer cohorts, timeout without a minted gap id,
  legacy unknown stops, packet-ready eligibility rejection, and duplicate or
  hand-edited totals.
- Live `metrics/rust-repair-trust/corpus.json` remains schema-valid with zero
  eligible attempts.

## Non-Goals

- No new corpus, attempt lifecycle, dashboard, or analysis engine.
- No #3074 observation collection, #3075 first attempt, #1702 usability
  pilot, #1560 threshold closeout, or #4572/#4575/#4576/#4578/#4585 work.
- No support-tier promotion, 0.11 release-scope change, network collector, or
  consumer edit.

## Acceptance Examples

1. Empty observations: route yield and repair success are `not_measurable`.
2. Three observations, no complete route: route yield `0/3`, repair success
   `not_measurable`, eligible attempts remain 0.
3. One complete route with `attempt_authorized: false`: route yield `1/1`,
   zero eligible or started attempts, repair success `not_measurable`.
4. One eligible attempt with `movement: improved` and `verification_result:
   failed`: both facts remain visible and separate.
5. CLI and editor rows sharing `opportunity_id`: two channel observations, one
   selected opportunity.
6. Same opportunity, two `cohort_id` values, only the later complete: route
   yield `1/2`, original incomplete result retained.
7. Timeout with `unit: repository_observation`: counted without a `gap:` id.
8. Legacy `static_limitation_no_repair_packet` without `route`: earliest stop
   `not_observed`.
9. `packet_ready: true` and `canonical_eligibility: rejected`: not a complete
   route.
10. Duplicate observation ids or a hand-edited corpus `route_yield` object:
    validation errors; trusted numerators still come from rows.

## Test Mapping

- `xtask/src/reports/rust_repair_trust.rs::tests::empty_observations_keep_route_yield_and_repair_success_unmeasurable`
- `xtask/src/reports/rust_repair_trust.rs::tests::three_observations_without_a_complete_route_are_zero_of_three_not_attempts`
- `xtask/src/reports/rust_repair_trust.rs::tests::three_observations_do_not_inherit_exclusion_rows_as_the_route_denominator`
- `xtask/src/reports/rust_repair_trust.rs::tests::live_corpus_counts_unique_observations_not_attempts_for_route_yield`
- `xtask/src/reports/rust_repair_trust.rs::tests::one_complete_route_without_edit_authorization_is_not_an_attempt`
- `xtask/src/reports/rust_repair_trust.rs::tests::failed_focused_test_and_improved_static_evidence_stay_separate`
- `xtask/src/reports/rust_repair_trust.rs::tests::cli_and_editor_observations_of_one_opportunity_do_not_inflate_route_yield`
- `xtask/src/reports/rust_repair_trust.rs::tests::analyzer_rerun_keeps_distinct_cohorts_and_the_original_result`
- `xtask/src/reports/rust_repair_trust.rs::tests::timeout_before_item_discovery_does_not_mint_a_gap_id`
- `xtask/src/reports/rust_repair_trust.rs::tests::legacy_static_limitation_is_not_relabelled_into_a_precise_stop`
- `xtask/src/reports/rust_repair_trust.rs::tests::packet_ready_rejected_by_canonical_eligibility_is_not_a_complete_route`
- `xtask/src/reports/rust_repair_trust.rs::tests::duplicate_ids_and_hand_edited_totals_cannot_enter_trusted_success_counts`
- `xtask/src/reports/rust_repair_trust.rs::tests::timeout_reason_normalizes_exactly_and_does_not_require_a_gap`
- `xtask/src/reports/rust_repair_trust.rs::tests::unlinked_repeat_attempts_do_not_mint_attempt_id_opportunities`
- `xtask/src/reports/rust_repair_trust.rs::tests::cross_repository_attempt_link_cannot_complete_the_wrong_opportunity`
- `xtask/src/reports/rust_repair_trust.rs::tests::finished_attempt_without_a_start_cannot_enter_the_trusted_ladder`
- `xtask/src/reports/rust_repair_trust.rs::tests::improved_static_movement_without_a_finished_attempt_is_not_ladder_success`
- `xtask/src/reports/rust_repair_trust.rs::tests::omitted_archaeology_cannot_count_as_help_free_completion`
- `xtask/src/reports/rust_repair_trust.rs::tests::observed_absent_archaeology_is_help_free_completion`
- `xtask/src/reports/rust_repair_trust.rs::tests::conflicting_channel_focused_tests_fail_closed_to_failed`

## Implementation Mapping

- `xtask/src/reports/rust_repair_trust.rs` — validator, ladder derivation,
  JSON/Markdown report
- `schemas/ripr/rust-repair-trust-corpus.schema.json` — optional observation
  `route` object and `selected_opportunity` classification
- `metrics/rust-repair-trust/corpus.json` — unchanged live rows; historical
  observations omit `route`

## Metrics

- `rust_repair_trust_route_yield_numerator`
- `rust_repair_trust_route_yield_denominator`
- `rust_repair_trust_repair_success_status`
