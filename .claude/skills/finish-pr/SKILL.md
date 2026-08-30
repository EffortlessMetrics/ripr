---
name: finish-pr
description: Publish or resume one PR, require substantive exact-head review, address current review and CI evidence, arm merge when ready, and reconcile after merge. Use when a coherent candidate exists or a PR already owns the claim.
---

# Result

The selected PR has an exact current candidate, a current `review-pr` disposition, every substantive finding repaired or evidence-refuted, proof current for affected seams, honest remote-wait state, and accurate post-merge issue reconciliation.

# Route markers

- `review_route:finish_pr_requires_review_ready`
- `review_route:finish_pr_resolves_repaired_threads`

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
7. Resolve every thread that now carries a landed repair or a source-backed reply, as merge preparation. Unresolved threads block the merge independently of review sufficiency, so a candidate can be `REVIEW_READY` with every required check green and still refuse to merge. This does not weaken step 6: a thread with neither a repair nor a reply stays open, and resolving to clear a blocker rather than because the finding is addressed is a false-confidence action. Read the thread state back after acting, and treat a rejected call or a still-unresolved thread as blocking rather than assuming the request succeeded. The reply and the resolve are separate operations that fail separately: a batch that posts no reply but still resolves leaves the thread closed with the evidence missing, which reads as addressed and is not. Order them accordingly — confirm the reply exists before resolving, and leave the thread open when that confirmation fails, so a lost reply cannot close an unrepaired finding.
8. Refresh only proof and review dimensions affected by the repair, then obtain a current `review-pr` disposition for the exact head.
9. Keep three subjects separate:
   - PR head: implementation and review;
   - integration basis: current base or queue predecessors;
   - squash/merge-group result: combined-tree interaction.
10. Do not update a behind-only branch. Reconcile only an actual conflict, explicit stack change, material prerequisite change, or failed integration proof.
11. Do not treat green CI, zero unresolved threads, or unavailable automated reviewers as substantive review. Do not infer a human approval dependency from `mergeStateStatus: BLOCKED`; identify the exact active rule, required actor, unsatisfied requirement, and evidence source. Read **both** authorities, because they can disagree: classic branch protection (`repos/{owner}/{repo}/branches/{branch}/protection`) and repository rulesets (`repos/{owner}/{repo}/rules/branches/{branch}`, then the named ruleset). A requirement reported as disabled in branch protection may still be enforced by an active ruleset — `required_conversation_resolution: false` alongside a ruleset `required_review_thread_resolution: true` is a real configuration on this repository.
12. When GitHub owns the next transition, yield instead of polling unchanged state. Arm auto-merge only for the exact published head when `review-pr` says `REVIEW_READY` and required proof is current.
13. After merge, verify current `main`, update issue and parent acceptance, refresh generated evidence, and close only acceptance-complete issues.
14. After deliberate closure or supersession, name the winning candidate and preserve residual work.

# Release-scope law

For a pinned release, treat the reviewed immutable pin receipt as the sole membership authority: qualification, source preflight, and finalization consume its exact ref, ancestry, ordered SHA digest, PR dispositions, and manifests unchanged. Ordinary `main` or swarm movement never repins or changes membership; repin only after a release-invalidating exact-candidate qualification or source-preflight failure, with an explicit superseding receipt. Do not close, draft, lock, relabel, retarget, or otherwise mutate unrelated PRs to freeze scope; they remain open and may evolve, and post-pin merges do not retarget the release. Close only this selected PR for its own evidence-backed terminal disposition, never close-now/reopen-after-release.

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
