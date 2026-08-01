---
name: finish-pr
description: Publish or resume one PR, address current review and CI evidence, arm merge when ready, and reconcile after merge. Use when a coherent candidate exists or a PR already owns the claim.
---

# Result

The selected PR has an exact current candidate, every substantive finding repaired or evidence-refuted, proof and review current for affected seams, honest remote-wait state, and accurate post-merge issue reconciliation.

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
4. Classify findings as valid source defect, test/oracle defect, stale, incorrect, infrastructure/instrument failure, or not established.
5. Repair valid findings in the same candidate. Refute incorrect findings with source-backed evidence. Resolve only after a repair or reply exists.
6. Refresh only proof and review dimensions affected by the repair.
7. Keep three subjects separate:
   - PR head: implementation and review;
   - integration basis: current base or queue predecessors;
   - squash/merge-group result: combined-tree interaction.
8. Do not update a behind-only branch. Reconcile only an actual conflict, explicit stack change, material prerequisite change, or failed integration proof.
9. When GitHub owns the next transition, yield instead of polling unchanged state. Arm auto-merge only for the exact reviewed candidate when repository policy allows.
10. After merge, verify current `main`, update issue and parent acceptance, refresh generated evidence, and close only acceptance-complete issues.
11. After deliberate closure or supersession, name the winning candidate and preserve residual work.

# Review independence

A different persona is not automatically independent. Use another reviewer when it changes evidence, oracle, context, tools, or failure perspective. The lead Claude context verifies the result and owns integration.

Quota, unavailable, skipped, failed, or stale reviewer output means review is missing for that provider; it is not a clean result.

# Valid outcomes

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
