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
review/proof references, source-survivor or swarm-exclusion context, and
typed `references[]` authority evidence. Each reference records its kind,
number, source, one evidence URL or stable GitHub identity, the observed
commit SHA, review state, and any limitation. The legacy `pr_refs` and
`issue_refs` fields remain compatibility projections derived from
`references[]`; they are not reference authority.

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
ordered records; candidate-tree commit sets that are not subsets of the
captured range; range/tree disagreement; disposition/tree-state mismatch;
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

## Current-main evidence

`fixtures/release_denominator/current-main-provisional.json` is a captured
provisional census of `c86807ec..c30a2683` at `origin/main` as observed on
2026-08-03. It carries 234 first-parent records and 219 candidate-tree
records, with 15 candidate-only exclusions and 15 safe deferrals. The
development-main merges for #2842, #2844, corrective #2858/#2859,
corrective #2862, release-control #2857, corrective #2867, #2870, #2860, #2861,
#2869, #2876, #2873, #2878, and #2872 are retained in
the denominator with current `safe_defer_post_0_11` disposition and
`absent_by_candidate_only_exclusion` tree state; earlier `hold_post_release`
is historical context only. The later corrective commits inherit that
disposition unless #2379 changes the release graph. The fixture pins cutoff
`c30a26831b75051813bfaa3dbd9378096ec6aa82`, range digest
`sha256:b85b8314b5f738335ae63220fe5f0ea8ef4e6e1892124eea148ea49181168501`,
and record-set digest
`sha256:166380b5b8cef061f6d617db089a5070e566e54841bd103c6a962d832881efe0`.
It is range/identity and explicitly reconciled disposition evidence, not a
final release qualification.

## Acceptance and proof map

Acceptance is limited to a deterministic supplemental denominator ledger and a
shared fail-closed validator. It does not close the final candidate decision
tracked by #1609 or the dependent release-editor lane #2769.

The implementation and fixture contract are mapped in `.ripr/traceability.toml`
under `RIPR-SPEC-0146`. Focused proof is provided by the twenty-five tests named there
and the complete/reconcile-required fixtures under
`fixtures/release_denominator/`; hosted CI is the authoritative execution
proof for this PR. The complete fixture pins the #2767/#2788 and #2768/#2790
authority pairs, distinguishes an earlier body PR reference, and covers
deterministic reference ordering, compatibility projection agreement, and
digest sensitivity to changed mappings. Final ledgers reject unreviewed or
legacy-only reference authority.

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

The twenty-five focused tests listed in `.ripr/traceability.toml` cover deterministic
normalization, missing/duplicate/out-of-range/order/tree failures, final
operator decisions, live drift, JSON/Markdown claim-boundary parity, typed
reference authority, compatibility projection mismatch, contradictory
identity, the two known issue/merge-PR pairs, deterministic ordering, changed mappings, final unreviewed
references, malformed reference evidence, numeric compatibility projection
ordering, manual-mapping reasons, reused reference identity, and the current-main
census pinned to the final corrective cutoff, counts, excluded commit
identities, and record-set digest.

## Implementation Mapping

The production implementation is `xtask/src/reports/release_denominator.rs`,
including `ReferenceEvidence` validation and compatibility projection
normalization.
CLI parsing, dispatch, and manifest-only fixture registration are mapped in
`xtask/src/command.rs`, `xtask/src/dispatch.rs`, and
`xtask/src/reports/fixtures.rs`. The report contract is documented in
`docs/OUTPUT_SCHEMA.md`.

## Metrics

The report exposes `release_denominator_ready` and
`release_denominator_reconcile_required` as the status metrics, with ordered
range, candidate-tree, and record-set digests retained for reconciliation.
