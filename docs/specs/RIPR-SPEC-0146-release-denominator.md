# RIPR-SPEC-0146: Historical Release Supplemental Denominator Ledger

Status: accepted

Issue: #2768

Related release-control work: #2767, #1609, #2379

## Intent

The supplemental denominator report records every commit in a captured
first-parent release range and gives each record an explicit reviewed
disposition. It is retained as historical reconciliation evidence for the
former C/T release-control model, not an active 0.11.0 candidate or publication
authority. The active release selects the exact live swarm head at the
transaction boundary; the later pin receipt regenerates counts and the ordered
SHA digest from the final pinned heads.

## Input contract

`release-denominator --input <ledger.json>` accepts a versioned historical
`release_denominator_snapshot` containing the historical base, candidate ref
and SHA, the pinned `source.github_repository`, an optional fixed
`provisional_review_cutoff_sha`, ordered range
commits, candidate-tree commits, and one record per range commit. Each record
carries release disposition, ownership, tree state, review/proof references,
source-survivor or swarm-exclusion context, capture status, optional
`claim_refs[]`, and typed `references[]` authority evidence. Each reference
records its kind, number, source, one evidence URL or stable GitHub identity,
the observed commit SHA, review state, and any limitation. The legacy
`pr_refs` and `issue_refs` fields remain compatibility projections derived
from `references[]`; they are not reference authority. An optional
`candidate_selection` object carries the historical #2766/#2871 selected-claim authority;
claim references are rejected when that authority is absent or does not name
the claim. A structurally invalid selected-claim authority also rejects its
claim references; readiness state remains owned by the shared candidate
control evaluator.

Records carrying the accepted execution-surface exclusion also retain the
exact excluded path set and `candidate_only_exclusion_granularity =
"hunk_or_symbol"`; a later candidate materializer must not remove an entire
shared file when retained changes in that file are outside the exclusion.

The accepted dispositions are `include_product`,
`include_release_infrastructure`, `include_control_or_honesty`,
`structural_no_semantic_delta`, `candidate_only_exclusion`,
`source_only_followup`, `safe_defer_post_0_11`, and
`operator_decision_required`. The report uses `ready` or
`reconcile_required`; it never claims that a candidate is qualified, published,
or released.

When a provisional cutoff is present, normalization derives the count of
`operator_decision_required` records through that cutoff from the ordered
records. A supplied candidate-selection count is reconciled against the
derived value; disagreement is not silently accepted.

`--live --input <ledger.json>` additionally compares the captured historical
candidate SHA, first-parent range, and candidate-tree commit set with bounded
live Git observations. It does not replace the reviewed ledger or select the
active live-head release candidate.

`--capture-github --input <ledger.json> --output <capture.json>` reads the
exact ledger range and captures all-state PR merge identities plus PR-body
issue/PR references from the current repository through `gh`. The capture is
bound to the input candidate SHA and range digest and is replayable offline.
Captured references are deliberately `reviewed: false` with an explicit
limitation; GitHub observation is not operator adjudication. PR-body references
are classified as closing references only for explicit, token-bounded closing
keywords; ordinary or negated prose remains a body reference.

`--import-github --input <ledger.json> --capture <capture.json> --output
<ledger.json>` imports only a capture with matching repository, candidate,
and range identity. It rejects missing or duplicate commit records and converts
inherited `safe_defer_post_0_11` rows, plus rows after the fixed provisional
cutoff, into `operator_decision_required` with
`candidate_tree_state_pending`. No blanket post-cutoff exclusion is accepted.

`--apply-adjudication --input <ledger.json> --decisions <adjudication.json>
--output <ledger.json>` applies one reviewed, position-complete adjudication
manifest through the pinned cutoff. Batch and override review references must
use an accepted adjudication prefix (`review:2832:*` for the reviewed prefix,
`review:2825:*` for the post-P development-denominator delta). Each position
receives a
closed disposition, a non-pending candidate-tree state, a batch review receipt,
and a residual non-claim; rows after the cutoff remain untouched. The
adjudicated candidate-tree commit list is projected from the resulting tree
states, while the ledger remains `stage = provisional`.

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

`fixtures/release_denominator/current-main-provisional.json` is the fixed
captured census of `c86807ec..b8b1c9ec` observed on 2026-08-08. After the
reviewed-prefix manifest from issue #2832 (through `fcbb30a7`) and the post-P
delta manifest from issue #2825 (through the selected development cut) are
applied it carries 333
first-parent records, 325 records present in the reviewed provisional tree,
five whole-commit candidate-only dependency exclusions, three reviewed
safe-deferral rows absent from the candidate tree, and zero rows through the
cut with `operator_decision_required`. Its provisional review cutoff stays at
the reviewed prefix `fcbb30a7cf6a37027fa377abafb617632b2e6f57`; the 103
post-cutoff rows through the selected development cut are adjudicated with
`review:2825:*` authority, so the post-provisional review check remains
meaningful. Every record has replayable GitHub
capture status and typed authority; captured references remain unreviewed
authority until separately reviewed, while #2832 and #2825 record the
substantive commit dispositions. The fixture pins range digest
`sha256:857911c214cd10a011ffc54fcf3226b81811a699a6e2364f10c03343c6a969c4`,
candidate-tree digest
`sha256:025963fa597c4f2c068be06cea7576a43589ac49b34014c68119987f5bb59825`,
and record-set digest
`sha256:4d8977b4a50e7697a5632f9e2e127bc83fde05640fddc09356dfc532bb72956a`.
It is a reviewed provisional input to the final candidate decision, not a final
release qualification.

## Cut-relative denominator boundary

The final denominator is relative to a selected development cut `C`, not to
an indefinitely moving `main`. Every commit through `C` must have a reviewed
disposition and candidate-tree state. The ledger must identify whether each
record is present in the candidate tree, which selected claim it satisfies,
whether it is structural/control-only, superseded, explicitly excluded, or
deferred, and what residual issue or truthful non-claim remains. Commits after
`C` belong to a later provisional range or a later candidate; they must not be
added to this candidate's denominator merely because development continues.

The denominator is one reviewed input to the candidate-relative hard-cut
predicate `candidate_required_claims_pending == 0`; the #2766 candidate
selection DTO owns selected-claim closure. This denominator report does not
establish selected-claim satisfaction from generic issue references alone. It
does not impose a repository-wide open-PR or open-issue convergence
requirement.

For a selected cut, `final_cut_authority.record_set_digest` must equal the
normalized report's `record_set_digest`. Post-provisional review credit is
limited to nonblank structured `review:2832:<id>` or `review:2825:<id>`
references; arbitrary,
whitespace-only, or unrelated record strings do not count as adjudication.
When the selected cut equals the provisional cutoff, the post-cutoff record
set is empty and its derived unreviewed count is zero without relying on an
invalid slice.

## Acceptance and proof map

Acceptance is limited to a deterministic supplemental denominator ledger and a
shared fail-closed validator. It does not close the final candidate decision
tracked by #1609 or the dependent release-editor lane #2769.

The implementation and fixture contract are mapped in `.ripr/traceability.toml`
under `RIPR-SPEC-0146`. Focused proof is provided by the thirty tests named there
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

The thirty focused tests listed in `.ripr/traceability.toml` cover deterministic
normalization, missing/duplicate/out-of-range/order/tree failures, final
operator decisions, live drift, JSON/Markdown claim-boundary parity, typed
reference authority, compatibility projection mismatch, contradictory
identity, the two known issue/merge-PR pairs, deterministic ordering, changed mappings, final unreviewed
references, malformed reference evidence, numeric compatibility projection
ordering, manual-mapping reasons, reused reference identity, adjudication
review-ref slug shape, and the current-main
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
