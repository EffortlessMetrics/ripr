---
name: finish-pr
description: Publish or resume one PR, address current review and CI evidence, arm merge when ready, and reconcile after merge. Use when a coherent candidate exists or a PR already owns the claim.
---

# Useful result

The selected PR has an exact current head, all substantive findings repaired or evidence-refuted, required proof current for affected seams, remote waits represented honestly, and issue state reconciled after merge or deliberate closure.

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
4. Classify every finding as:
   - valid source defect;
   - test or oracle defect;
   - stale/obsolete;
   - incorrect finding;
   - infrastructure or instrument failure;
   - not established.
5. Repair valid findings through the same candidate. For an incorrect finding, reply with source-backed evidence. Resolve only after the reply or repair exists.
6. Refresh only proof and review dimensions affected by the repair.
7. Distinguish candidate-head proof from integration proof:
   - the PR head is the implementation/review subject;
   - current `main` or queued predecessors are the integration basis;
   - the squash/merge-group result is the combined-tree subject.
8. Do not update a behind-only branch. Reconcile only an actual conflict, explicit stack change, material prerequisite change, or failed combined-tree proof.
9. When GitHub owns the next transition, return an in-flight result instead of polling unchanged state. Auto-merge may be armed when the exact reviewed head is ready.
10. After merge, verify `main`, update issue acceptance, parent state, generated evidence, and any residual work. Close only acceptance-complete issues.
11. After deliberate closure or supersession, record the winning candidate and preserved residual work.

# Review law

A differently named agent is not automatically independent. Use another reviewer when it changes the evidence, oracle, context, tools, or failure perspective. The accountable root verifies and integrates the result.

Quota, unavailable, skipped, failed, or stale review-provider output is missing review, not a clean review.

# Valid exits

- `PR_MERGED`
- `PR_IN_FLIGHT`
- `AUTO_MERGE_ARMED`
- `WAITING_REQUIRED_CHECKS`
- `WAITING_EXTERNAL_REVIEW`
- `WAITING_INTEGRATION_PROOF`
- `PR_CLOSED_WITH_DISPOSITION`
- `REPAIR_REQUIRED`
- `INSTRUMENT_FAILURE`
- `INFRASTRUCTURE_FAILURE`
- `EXTERNAL_BLOCKER`
- `NOT_ESTABLISHED`
