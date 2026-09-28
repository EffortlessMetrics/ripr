---
name: finish-pr
description: Publish or resume one PR, require substantive exact-head review, address current review and CI evidence, arm merge when ready, and reconcile after merge. Use when a coherent candidate exists or a PR already owns the claim.
---

# Useful result

The selected PR has an exact current head, a current `review-pr` disposition, all substantive findings repaired or evidence-refuted, required proof current for affected seams, remote waits represented honestly, and issue state reconciled after merge or deliberate closure.

# Route markers

- `review_route:finish_pr_requires_review_ready`
- `review_route:finish_pr_resolves_repaired_threads`

# Finish contract markers

- `finish_contract:routine_repo_writes`
- `finish_contract:ordinary_squash_merge`
- `finish_contract:duplicate_recheck`
- `finish_contract:behind_only_no_restack`

# Entry condition

`finish-pr` owns publication, remote review/CI repair, merge, and reconciliation. It does not silently manufacture the substantive review pass.

A committed candidate with `REVIEW_INCOMPLETE` may enter so the procedure can publish the PR and obtain remote evidence. Before arming auto-merge or merging, the exact published PR head must have a current `REVIEW_READY` result from `review-pr`. When the head changes materially, re-enter `review-pr` for affected currentness dimensions.

Routine publication and convergence inside the selected repository claim—push ordinary branch, open/update PR, reply to review, repair CI, resolve addressed threads, arm normal auto-merge, use protected squash merge, and clean lane-created state—do not require another owner approval. Separate authorization remains required for force-push/shared-history rewriting, settings/rulesets/secrets, public tags/releases/publication/signing/credentials, durable-evidence deletion, or work outside the selected goal.

# Procedure

1. Reuse the PR that already owns the claim. Search current source, all-state PRs, recently merged PRs, and the controlling issue immediately before publishing a new PR. Publish a new PR only when no equivalent candidate exists.
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
7. As merge preparation, resolve every thread that now holds a landed repair or a source-backed reply. Unresolved threads are a merge blocker in their own right, separate from whether review was sufficient: a candidate can hold `REVIEW_READY`, pass every required check, and still be refused. This does not soften the previous step. A thread with neither a repair nor a reply stays open, and resolving in order to clear a blocker rather than because the finding is addressed is a false-confidence action. Read each thread's state back after acting; a rejected call or a thread still reported unresolved keeps merge preparation blocked, and neither may be assumed to have succeeded. The reply and the resolve are independent operations that fail independently—confirm the reply exists before issuing the resolve and leave the thread open when that confirmation fails.
8. Refresh only proof and review dimensions affected by the repair, then obtain a new or amended `review-pr` disposition for the exact head.
9. Distinguish candidate-head proof from integration proof:
   - the PR head is the implementation/review subject;
   - current `main` or queued predecessors are the integration basis;
   - the ordinary squash result is the combined-tree subject.
10. Do not update a behind-only branch. Unrelated movement on `main` is normal and does not require restacking, force-pushing, or rerunning unaffected proof. Reconcile only an actual conflict, explicit stack change, material prerequisite change, failed combined-tree proof, or exact-base policy that applies to this candidate.
11. Before spending time on conflict repair, re-run the claim-identity search. When upstream or another PR already delivered the same claim, stop, compare for unique residuals, record the winning implementation, and close or disposition the duplicate rather than resolving a dead candidate.
12. Do not infer review from an empty thread list, reviewer quota/unavailability, or green required checks. Do not infer a human approval requirement from `mergeStateStatus: BLOCKED`; identify the exact rule and evidence source first. Query both authorities, since classic branch protection and repository rulesets can disagree.
13. When GitHub owns the next transition, return an in-flight result instead of polling unchanged state. Auto-merge may be armed only for the exact published head with `REVIEW_READY` and current required proof. The parent goal may advance another independent claim while this PR waits.
14. Merge ordinary `ripr-swarm` development PRs with the repository's protected squash method. Exact history-preserving source-integration transactions are a separate controlled path and follow their governing issue instead of this rule.
15. After merge, verify `main`, update issue acceptance, parent state, generated evidence, and any residual work. A merged PR means implementation landed; it does not by itself complete the parent issue, release phase, or high-level goal.
16. After deliberate closure or supersession, record the winning candidate and preserved residual work.
17. Remove only the lane-created worktree, stale local branch, and temporary residue after the merged or closed disposition is durable.

# Release-scope law

For a pinned release, treat the reviewed immutable pin receipt as the sole membership authority: qualification, source preflight, and finalization consume its exact ref, ancestry, ordered SHA digest, PR dispositions, and manifests unchanged. Ordinary `main` or swarm movement never repins or changes membership; repin only after a release-invalidating exact-candidate qualification or source-preflight failure, with an explicit superseding receipt. Do not close, draft, lock, relabel, retarget, or otherwise mutate unrelated PRs to freeze scope; they remain open and may evolve, and post-pin merges do not retarget the release. Close only this selected PR for its own evidence-backed terminal disposition—never close-now/reopen-after-release.

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
