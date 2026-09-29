# RIPR-SPEC-0179: Local result-bound usefulness feedback receipts

Status: proposed

Owner: product-swarm

Created: 2026-09-29

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #4585 — local result-bound usefulness receipts before editor or protocol integration
- #1619 — parent feedback contract (later editor/protocol slices stay out of this leaf)
- #1702 — pilot can record pre-attempt failures and useful limitations
- #4570 — report integration ownership beyond the join
- #4572 — attempt retention (not absorbed)

Linked PRs:

- #4684 — local result-bound usefulness receipts

Support-tier impact:

- None. This spec adds a local ignored artifact and an explicit CLI. It does
  not promote a language, editor surface, gate, or public support claim.
- Claim boundaries remain governed by
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- No new crates, binaries, dependencies, network allowlist rows, process
  spawners, LSP/MCP write tools, or support-tier changes.

## Problem

Objective static movement cannot say why a result was useful, incorrect,
unclear, too expensive, or intentionally left alone. Developers and the #1702
pilot currently reconstruct that judgment from session notes. A current
file/line is not a stable identity for attaching an old judgment to a new
result. Implicit recording on analyze, hover, or packet paths would mix
opinion with analyzer state.

## Behavior

`ripr feedback record` writes one versioned receipt under
`target/ripr/feedback/` bound to an immutable snapshot and optional canonical
item. `ripr feedback export` joins stored receipts onto existing route-quality
rows without creating a second attempt ledger. Recording is explicit: analyze,
hover, and packet paths never record feedback.

The receipt retains:

```text
schema / feedback identity / idempotency key
result snapshot and canonical item, with explicit absence where no gap exists
route digest and optional RepairAttempt/receipt reference
actor kind: human | agent | unknown
review status, separate from actor kind
reason code and optional bounded note
historical/current/mismatched reference state
```

Judgment classes are `useful`, `incorrect`, `unclear`, `expensive`, and
`intentional_no_action`. Closed reason codes map onto those classes. `other`
requires `--note` and `--judgment`. A useful limitation or intentional
no-action may omit a gap id and attempt. File/line is rejected as identity.

Idempotency: the same key and payload produce one record (`already_recorded`
on repeat). A different payload with the same key is a conflict. Notes are
bounded (1024 bytes) and fail closed on known secret patterns; those checks
are best-effort, not a guarantee. Storage uses the shared atomic writer.
Default storage is a local ignored artifact. No network submission.

Recording or aggregating feedback changes none of diagnostic visibility,
analyzer classification, baseline/suppression/waiver policy, gap movement or
closure, support tier, or CI gates. Agent feedback stays agent feedback until
a human review actor is recorded. Helpful feedback does not establish
correctness; a negative opinion does not automatically establish a false
positive. Silence is not a vote.

Export reports objective route-quality counts separately from subjective
usefulness. Unreviewed, stale, unmatched, and missing-feedback states are
counts, not success percentages. `reviewed_human_useful_rate` is null when
no human-reviewed receipts exist.

## Non-Goals

- Editor UI, custom LSP requests, or MCP write tools (#1619 later slices)
- Attempt retention or a second attempt ledger (#4572)
- Reporting ownership beyond joining existing route-quality rows (#4570)
- Suppression proposals, hosted telemetry, model training, or external contact
- New user identity system, 0.11 release membership, or support promotion
- Runtime mutation outcomes or analyzer-correctness claims from opinion

## Required Evidence

- Public help states writes, privacy, exact output, and non-effects.
- JSON receipts use `schema_version` `0.1` and `kind`
  `usefulness_feedback_receipt`; export uses `kind`
  `usefulness_feedback_join`.
- `policy_effects` records unchanged diagnostics, classification, baseline,
  suppressions, gates, and gap closure.
- Discriminating tests cover idempotency, conflict, file/line rejection,
  no-item useful limitation, historical later-attempt binding, mismatch,
  agent versus human review, no fake success rates, join to existing rows,
  path escape, secret-pattern notes, oversized notes, leftover temp files,
  and malformed receipts.

## Inputs

- `--snapshot` (required), optional `--item`, `--route-digest`, `--attempt`,
  `--receipt`, `--actor`, `--review`, `--review-actor`, `--reason`,
  `--judgment`, `--note`, `--idempotency-key`, `--root`, `--json`
- Export: `--root`, optional `--route-quality`, `--out`, `--json`

## Outputs

- `target/ripr/feedback/<idempotency-key>.json`
- Optional join JSON on stdout or `--out`

## Acceptance Examples

1. `ripr feedback record --snapshot snap-1 --reason useful_limitation`
   writes one receipt with no canonical item and exits 0.
2. Repeating the same key and payload returns `already_recorded` and does
   not create a second file.
3. The same key with a different payload exits non-zero with an idempotency
   conflict.
4. `--file` / `--line` exits non-zero; identity remains snapshot plus item.
5. Comparing suppression, baseline, and gate artifacts before and after
   recording shows no mutation.
6. Agent `wrong_discriminator` feedback is counted as agent/unreviewed, not
   human-approved.
7. Export with only unreviewed receipts leaves
   `reviewed_human_useful_rate` null and reports missing-feedback rows as
   counts.
8. Export joins a receipt onto an existing route-quality sample gap id
   without rewriting that row's attempted/improved counts.

## Test Mapping

- `crates/ripr/src/domain/feedback.rs::tests::every_reason_maps_onto_one_of_the_five_judgments`
- `crates/ripr/src/domain/feedback.rs::tests::snapshot_is_required_and_file_line_is_not_identity`
- `crates/ripr/src/domain/feedback.rs::tests::useful_limitation_may_omit_canonical_item`
- `crates/ripr/src/domain/feedback.rs::tests::other_requires_a_note_and_explicit_judgment`
- `crates/ripr/src/domain/feedback.rs::tests::closed_reason_rejects_a_mismatched_judgment`
- `crates/ripr/src/domain/feedback.rs::tests::a_later_attempt_keeps_the_historical_reference`
- `crates/ripr/src/domain/feedback.rs::tests::absent_live_identity_is_current_not_staleness`
- `crates/ripr/src/app/feedback.rs::tests::recording_the_same_key_and_payload_is_idempotent`
- `crates/ripr/src/app/feedback.rs::tests::matching_explicit_judgment_is_idempotent_with_the_derived_class`
- `crates/ripr/src/app/feedback.rs::tests::same_key_with_a_different_payload_is_a_conflict`
- `crates/ripr/src/app/feedback.rs::tests::useful_limitation_does_not_require_an_attempt_or_item`
- `crates/ripr/src/app/feedback.rs::tests::recording_does_not_mutate_policy_or_gate_artifacts`
- `crates/ripr/src/app/feedback.rs::tests::agent_feedback_is_never_counted_as_human_reviewed`
- `crates/ripr/src/app/feedback.rs::tests::unreviewed_and_missing_feedback_do_not_become_success_rates`
- `crates/ripr/src/app/feedback.rs::tests::join_attaches_to_existing_route_quality_rows_without_a_second_ledger`
- `crates/ripr/src/app/feedback.rs::tests::path_escape_and_secret_notes_fail_closed`
- `crates/ripr/src/app/feedback.rs::tests::leftover_temp_files_are_not_loaded_as_receipts`
- `crates/ripr/src/app/feedback.rs::tests::malformed_receipt_fails_closed`
- `crates/ripr/src/app/feedback.rs::tests::production_source_does_not_reach_policy_process_or_network_surfaces`
- `crates/ripr/src/cli/commands/feedback.rs::tests::public_cli_records_idempotently_and_rejects_file_line_identity`
- `crates/ripr/src/cli/commands/feedback.rs::tests::public_cli_export_joins_without_network_or_policy_verbs`
- `crates/ripr/src/cli/commands/feedback.rs::tests::public_cli_rejects_other_without_judgment_and_mismatched_class`
- `crates/ripr/src/cli/commands/feedback.rs::tests::help_states_writes_privacy_output_and_non_effects`
- `crates/ripr/src/output/feedback.rs::tests::receipt_json_round_trips_and_states_non_effects`

## Implementation Mapping

- `crates/ripr/src/domain/feedback.rs` — typed identity, reason taxonomy, and
  reference-state classification
- `crates/ripr/src/app/feedback.rs` — record, load, privacy, join
- `crates/ripr/src/output/feedback.rs` — JSON receipt and join rendering
- `crates/ripr/src/cli/commands/feedback.rs` — `ripr feedback record|export`

## CI Proof

- Focused `cargo test -p ripr` filters for `feedback`
- `cargo xtask precommit` on the candidate

## Metrics

- `usefulness_feedback_receipts_recorded` — count of local receipts
- `usefulness_feedback_join_unmatched` — unmatched receipt count on export

These metrics are advisory counts. They are not success rates, support-tier
promotion evidence, or analyzer correctness.
