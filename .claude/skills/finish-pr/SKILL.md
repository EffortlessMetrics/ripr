---
name: finish-pr
description: Publish or resume one PR, require substantive exact-head review, address current review and CI evidence, arm merge when ready, and reconcile after merge. Use when a coherent candidate exists or a PR already owns the claim.
---

# Result

The selected PR has an exact current candidate, a current `review-pr` disposition, every substantive finding repaired or evidence-refuted, proof current for affected seams, honest remote-wait state, and accurate post-merge issue reconciliation.

# Route markers

- `review_route:finish_pr_requires_review_ready`

# Entry condition

`finish-pr` owns publication, review-comment/CI repair, merge, and reconciliation. It does not infer that substantive review occurred.

A committed candidate with `REVIEW_INCOMPLETE` may enter so the PR can be published and remote evidence can run. Before auto-merge or merge, the exact published head must have `REVIEW_READY` from `review-pr`. Re-enter review for dimensions changed by a later head.

# Workflow

1. Reuse the PR that already owns the claim. Create another only when no equivalent candidate exists.
2. Publish a complete PR body with:
   - production and evidence delta;
   - acceptance matrix;
   - issue/spec/ADR authority;
   - proof actually run;
   - limitations and non-claims;
   - rollback boundary;
   - exact candidate SHA.
3. Read current review submissions, inline threads, required and advisory checks, mergeability, and head identity.
4. Locate the current-head `review-pr` inspection record. If it is absent, stale, covers another head, or merely summarizes comments and CI, run `review-pr` before declaring readiness.
5. Classify findings as valid source defect, test/oracle defect, stale, incorrect, infrastructure/instrument failure, missing review/proof, or not established.
6. Repair valid findings in the same candidate. Refute incorrect findings with source-backed evidence. Resolve only after a repair or reply exists.
7. Refresh only proof and review dimensions affected by the repair, then obtain a current `review-pr` disposition for the exact head.
8. Keep three subjects separate:
   - PR head: implementation and review;
   - integration basis: current base or queue predecessors;
   - squash/merge-group result: combined-tree interaction.
9. Do not update a behind-only branch. Reconcile only an actual conflict, explicit stack change, material prerequisite change, or failed integration proof.
10. Do not treat green CI, zero unresolved threads, or unavailable automated reviewers as substantive review. Do not infer a human approval dependency from `mergeStateStatus: BLOCKED`; identify the exact active rule, required actor, unsatisfied requirement, and evidence source.
11. When GitHub owns the next transition, yield instead of polling unchanged state. Arm auto-merge only for the exact published head when `review-pr` says `REVIEW_READY` and required proof is current.
12. After merge, verify current `main`, update issue and parent acceptance, refresh generated evidence, and close only acceptance-complete issues.
13. After deliberate closure or supersession, name the winning candidate and preserve residual work.

# Review independence

A different persona is not automatically independent. Use another reviewer when it changes evidence, oracle, context, tools, platform access, or failure perspective. The lead Claude context verifies the result and owns integration.

Quota, unavailable, skipped, failed, or stale reviewer output means review is missing for that provider. On the author's own PR, use a `COMMENT` review with an explicit blocking or review-ready disposition because GitHub cannot request changes from the author. That platform limitation is not approval.

# Valid outcomes

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
