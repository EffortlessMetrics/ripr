---
name: finish-pr
description: Publish or resume one PR, require substantive exact-head review, address current review and CI evidence, arm merge when ready, and reconcile after merge. Use when a coherent candidate exists or a PR already owns the claim.
---

# Useful result

The selected PR has an exact current head, a current `review-pr` disposition, all substantive findings repaired or evidence-refuted, required proof current for affected seams, remote waits represented honestly, and issue state reconciled after merge or deliberate closure.

# Route markers

- `review_route:finish_pr_requires_review_ready`

# Entry condition

`finish-pr` owns publication, remote review/CI repair, merge, and reconciliation. It does not silently manufacture the substantive review pass.

A committed candidate with `REVIEW_INCOMPLETE` may enter so the procedure can publish the PR and obtain remote evidence. Before arming auto-merge or merging, the exact published PR head must have a current `REVIEW_READY` result from `review-pr`. When the head changes materially, re-enter `review-pr` for affected currentness dimensions.

# Procedure

1. Reuse the PR that already owns the claim. Publish a new PR only when no equivalent candidate exists.
2. Write a complete body:
   - production and evidence delta;
   - acceptance matrix;
   - governing issue/spec/ADR;
   - proof actually run;
   - limitations and non-claims;
   - rollback boundary;
   - exact candidate SHA.
3. Inspect current reviews, inline threads, required and advisory checks, mergeability, and head identity.
4. Locate the current-head `review-pr` record. If it is missing, stale, covers only automated comments/CI, or is anchored to another head, run `review-pr` before treating the candidate as merge-ready.
5. Classify every automated or human finding as:
   - valid source defect;
   - test or oracle defect;
   - stale/obsolete;
   - incorrect finding;
   - infrastructure or instrument failure;
   - missing proof/review;
   - not established.
6. Repair valid findings through the same candidate. For an incorrect finding, reply with source-backed evidence. Resolve only after the reply or repair exists.
7. Refresh only proof and review dimensions affected by the repair, then obtain a new or amended `review-pr` disposition for the exact head.
8. Distinguish candidate-head proof from integration proof:
   - the PR head is the implementation/review subject;
   - current `main` or queued predecessors are the integration basis;
   - the squash/merge-group result is the combined-tree subject.
9. Do not update a behind-only branch. Reconcile only an actual conflict, explicit stack change, material prerequisite change, or failed combined-tree proof.
10. Do not infer review from an empty thread list, reviewer quota/unavailability, or green required checks. Do not infer a human approval requirement from `mergeStateStatus: BLOCKED`; identify the exact rule and evidence source first.
11. When GitHub owns the next transition, return an in-flight result instead of polling unchanged state. Auto-merge may be armed only for the exact published head with `REVIEW_READY` and current required proof.
12. After merge, verify `main`, update issue acceptance, parent state, generated evidence, and any residual work. Close only acceptance-complete issues.
13. After deliberate closure or supersession, record the winning candidate and preserved residual work.

# Review law

A differently named agent is not automatically independent. Use another reviewer when it changes the evidence, oracle, context, tools, platform access, or failure perspective. The accountable root verifies and integrates the result.

Quota, unavailable, skipped, failed, or stale review-provider output is missing review, not a clean result. A self-review on the author's PR uses a `COMMENT` event with an explicit disposition because GitHub cannot request changes from the author; that platform constraint is not approval.

# Valid exits

- `PR_MERGED`
- `PR_IN_FLIGHT`
- `AUTO_MERGE_ARMED`
- `WAITING_REQUIRED_CHECKS`
- `WAITING_EXTERNAL_REVIEW`
- `WAITING_INTEGRATION_PROOF`
- `PR_CLOSED_WITH_DISPOSITION`
- `REPAIR_REQUIRED`
- `REVIEW_INCOMPLETE`
- `INSTRUMENT_FAILURE`
- `INFRASTRUCTURE_FAILURE`
- `EXTERNAL_BLOCKER`
- `NOT_ESTABLISHED`
