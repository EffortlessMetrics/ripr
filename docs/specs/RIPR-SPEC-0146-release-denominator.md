# RIPR-SPEC-0146: Release supplemental denominator ledger

Status: accepted

Issue: #2768

Related release-control work: #2767, #1609, #2379

## Intent

The supplemental denominator report records every commit in a captured
first-parent release range and gives each record an explicit reviewed
disposition. It is a reconciliation aid for the 0.11 release process, not a
candidate qualification or publication decision.

## Input contract

`release-denominator --input <ledger.json>` accepts a versioned
`release_denominator_snapshot` containing the historical base, candidate ref
and SHA, ordered range commits, candidate-tree commits, and one record per
range commit. Each record carries release disposition, ownership, tree state,
review/proof references, and source-survivor or swarm-exclusion context.

The accepted dispositions are `include_product`,
`include_release_infrastructure`, `include_control_or_honesty`,
`structural_no_semantic_delta`, `candidate_only_exclusion`,
`source_only_followup`, `safe_defer_post_0_11`, and
`operator_decision_required`. The report uses `ready` or
`reconcile_required`; it never claims that a candidate is qualified, published,
or released.

`--live --input <ledger.json>` additionally compares the captured candidate
SHA, first-parent range, and candidate-tree commit set with bounded live Git
observations. It does not replace the reviewed ledger.

## Validation

Validation fails closed for missing, duplicate, out-of-range, or wrongly
ordered records; range/tree disagreement; disposition/tree-state mismatch;
unresolved operator decisions in a final ledger; and live observation drift.
JSON and Markdown are rendered from the same normalized report and carry the
same authority boundary and limitation statements.

## Outputs

The command writes:

- `target/ripr/reports/release-denominator.json`
- `target/ripr/reports/release-denominator.md`

The report includes stable range, candidate-tree, and record-set digests,
record counts, disposition/tree-state counts, reconciliation reasons, and the
next action. Provisional output identifies the missing final candidate
decision; final output is ready only after every record is reconciled.

## Acceptance and proof map

Acceptance is limited to a deterministic supplemental denominator ledger and a
shared fail-closed validator. It does not close the final candidate decision
tracked by #1609 or the dependent release-editor lane #2769.

The implementation and fixture contract are mapped in `.ripr/traceability.toml`
under `RIPR-SPEC-0146`. Focused proof is provided by the nine tests named there
and the complete/reconcile-required fixtures under
`fixtures/release_denominator/`; hosted CI is the authoritative execution
proof for this PR.

## Problem

The release denominator currently exists as issue prose and can drift as
development commits land. Reconstructing it at freeze time without an ordered,
reviewed record set risks missing commits, double-counting work, or confusing
candidate-tree presence with release disposition.

## Behavior

The command normalizes a captured first-parent range, validates exactly one
record per commit, reconciles every record with the candidate tree, and emits
stable digests plus actionable reconciliation reasons. Live mode compares the
captured observations with bounded Git facts. Any disagreement produces
`reconcile_required`.

## Required Evidence

- a complete provisional ledger fixture;
- a negative fixture with unresolved, duplicate, and tree-mismatch evidence;
- focused tests for each fail-closed validation family;
- one normalized DTO rendered as both JSON and Markdown;
- traceability from this spec to tests, fixtures, code, outputs, and metrics.

## Non-Goals

- no candidate construction or qualification;
- no merge, issue closure, tag, publication, signing, or source integration;
- no replacement for Git history or GitHub issue/PR state;
- no automatic product-priority or release-owner decision.

## Acceptance Examples

The complete provisional fixture is `ready` and has stable range, tree, and
record-set digests. Removing a record, duplicating a record, changing order,
adding an out-of-range record, changing tree membership, retaining an operator
decision in a final ledger, or disagreeing with live observations produces
`reconcile_required`.

## Test Mapping

The nine focused tests listed in `.ripr/traceability.toml` cover deterministic
normalization, missing/duplicate/out-of-range/order/tree failures, final
operator decisions, live drift, and JSON/Markdown claim-boundary parity.

## Implementation Mapping

The production implementation is `xtask/src/reports/release_denominator.rs`.
CLI parsing, dispatch, and manifest-only fixture registration are mapped in
`xtask/src/command.rs`, `xtask/src/dispatch.rs`, and
`xtask/src/reports/fixtures.rs`. The report contract is documented in
`docs/OUTPUT_SCHEMA.md`.

## Metrics

The report exposes `release_denominator_ready` and
`release_denominator_reconcile_required` as the status metrics, with ordered
range, candidate-tree, and record-set digests retained for reconciliation.
