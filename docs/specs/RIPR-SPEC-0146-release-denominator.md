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
`candidate_selection` object carries the #2766/#2871 selected-claim authority;
claim references are rejected when that authority is absent or does not name
the claim. A structurally invalid selected-claim authority also rejects its
claim references; readiness state remains owned by the shared candidate
control evaluator.

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
captured census of `c86807ec..c30a2683` observed on 2026-08-03. It carries 234
first-parent records, 219 records present in the retained provisional tree,
and 15 records with `candidate_tree_state_pending`; all 234 records remain
`operator_decision_required`. Its fixed provisional review cutoff is
`fcbb30a7cf6a37027fa377abafb617632b2e6f57`; later rows are retained as
observed delta, not silently excluded. Every record has replayable GitHub
capture status and typed authority, but those references remain unreviewed
until #2832 adjudication. The fixture pins range digest
`sha256:b85b8314b5f738335ae63220fe5f0ea8ef4e6e1892124eea148ea49181168501`,
candidate-tree digest
`sha256:c1b3675b6b98f609343f35711898e805a6ad27577c8f9b351ae53718b91082ae`,
and record-set digest
`sha256:172ef3d76ae3db47b8f7abedae9151ce971d3941b5a9eeb18b4c824d25c9530d`.
It is a fixed, captured input to substantive #2832 review, not a final
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

## Acceptance and proof map

Acceptance is limited to a deterministic supplemental denominator ledger and a
shared fail-closed validator. It does not close the final candidate decision
tracked by #1609 or the dependent release-editor lane #2769.

The implementation and fixture contract are mapped in `.ripr/traceability.toml`
under `RIPR-SPEC-0146`. Focused proof is provided by the twenty-nine tests named there
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

The twenty-nine focused tests listed in `.ripr/traceability.toml` cover deterministic
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
