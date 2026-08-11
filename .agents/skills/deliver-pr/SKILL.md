---
name: deliver-pr
description: Carry one coherent issue or claim from current repository truth through proof, implementation, substantive current-head review, merge, and reconciliation. Use for a selected PR-sized lane or an existing PR.
---

# Useful result

One coherent claim has one current candidate, current proof and review for the affected seams, a durable GitHub PR state, and an accurate issue disposition after merge or deliberate closure.

# Route markers

- `review_route:deliver_pr_to_review_pr`

# Procedure

1. Hydrate the selected claim from the issue, governing artifacts, current source, and any existing PR.
2. Search for an equivalent existing PR before creating a branch. Reuse the current candidate when one exists.
3. Identify the earliest missing or stale judgment:
   - premise or issue quality;
   - proof design;
   - implementation;
   - test hardening;
   - simplification;
   - candidate challenge;
   - substantive current-head review;
   - review-comment repair;
   - integration proof;
   - reconciliation.
4. Enter at that point. Do not recreate completed ceremony merely because this session arrived later.
5. If the issue is missing or materially wrong, use `prepare-issue` and continue.
6. If proof is absent or self-confirming, use `prepare-proof` and continue.
7. Build or repair the one current candidate with `build-candidate` and materialize an exact committed head.
8. Run `review-pr` on that exact head. Before publication, retain unavailable remote checks, artifacts, and external review as `REVIEW_INCOMPLETE`; do not convert them to pass.
9. Use `finish-pr` to publish or resume the exact candidate when no equivalent PR already exists.
10. Re-enter `review-pr` on the exact published PR head after remote evidence is available. A green check set, empty thread list, or unavailable reviewer does not establish substantive review.
11. Route `REPAIR_REQUIRED` back through the same candidate, then refresh only the affected proof and review dimensions.
12. Only a current `REVIEW_READY` PR head may enter `finish-pr` merge convergence. Explicit incomplete or blocking review states remain draft or durably in flight.
13. After merge or deliberate closure, verify current `main`, update delivered versus remaining acceptance, update parents, and release the candidate worktree.

# Candidate law

- One coherent claim normally has one branch, worktree, candidate, and PR.
- Do not create rival implementations merely to manufacture parallelism.
- Multiple writers may contribute genuinely disjoint pieces only through one integrating candidate owner.
- Do not inspect sibling worktrees or reserve files, crates, APIs, or semantic surfaces.
- A behind-only branch needs no action.
- Rebase or update only for an actual conflict, changed explicit prerequisite, material combined-tree failure, or repository policy that applies to this candidate.

# Release-scope law

- Pin release membership to the exact immutable head SHA, ancestry, and release manifests.
- Treat the reviewed immutable pin receipt as the sole membership authority: qualification, source preflight, and finalization consume it unchanged. Ordinary `main` or swarm movement never repins or changes membership; repin only after a release-invalidating exact-candidate qualification or source-preflight failure, with an explicit superseding receipt.
- Never close, draft, lock, relabel, retarget, or otherwise mutate an unrelated PR to freeze release scope; unrelated PRs stay open and may evolve.
- A post-pin merge does not retarget the release. Close only this selected PR for its own evidence-backed terminal disposition; never close-now/reopen-after-release.

# Proof and review currentness

Track currentness by dimension rather than treating every SHA change as total invalidation:

- production implementation;
- test stimulus;
- test oracle;
- public claim;
- generated relationships;
- conflict resolution;
- integration basis;
- candidate head identity.

Refresh only the dimensions changed by the latest edit. Unrelated movement on `main` invalidates nothing by itself. A current-head review that covered only comments or CI is not a substitute for `review-pr`'s semantic-owner, oracle, contract-parity, platform, and exact-head evidence pass.

# Useful fan-out

Focused read-only agents may inspect authority, tests, correctness, security, compatibility, product behavior, or platform semantics. One writer integrates accepted repairs. Conflicting reports must be resolved against canonical source and actual behavior before publication.

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
- `EXTERNAL_BLOCKER`
- `NEEDS_OWNER_DECISION`
- `NOT_ESTABLISHED`

A remote-owned wait yields to the goal loop; it does not keep the root polling unchanged state.
