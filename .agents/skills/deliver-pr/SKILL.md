---
name: deliver-pr
description: Carry one coherent issue or claim from current repository truth through proof, implementation, review, merge, and reconciliation. Use for a selected PR-sized lane or an existing PR.
---

# Useful result

One coherent claim has one current candidate, current proof and review for the affected seams, a durable GitHub PR state, and an accurate issue disposition after merge or deliberate closure.

# Procedure

1. Hydrate the selected claim from the issue, governing artifacts, current source, and any existing PR.
2. Search for an equivalent existing PR before creating a branch. Reuse the current candidate when one exists.
3. Identify the earliest missing or stale judgment:
   - premise or issue quality;
   - proof design;
   - implementation;
   - test hardening;
   - simplification;
   - candidate review;
   - review-comment repair;
   - integration proof;
   - reconciliation.
4. Enter at that point. Do not recreate completed ceremony merely because this session arrived later.
5. If the issue is missing or materially wrong, use `prepare-issue` and continue.
6. If proof is absent or self-confirming, use `prepare-proof` and continue.
7. Build or repair the one current candidate with `build-candidate`.
8. Publish or resume the PR, then use `finish-pr`.
9. After merge or deliberate closure, verify current `main`, update delivered versus remaining acceptance, update parents, and release the candidate worktree.

# Candidate law

- One coherent claim normally has one branch, worktree, candidate, and PR.
- Do not create rival implementations merely to manufacture parallelism.
- Multiple writers may contribute genuinely disjoint pieces only through one integrating candidate owner.
- Do not inspect sibling worktrees or reserve files, crates, APIs, or semantic surfaces.
- A behind-only branch needs no action.
- Rebase or update only for an actual conflict, changed explicit prerequisite, material combined-tree failure, or repository policy that applies to this candidate.

# Proof and review currentness

Track currentness by dimension rather than treating every SHA change as total invalidation:

- production implementation;
- test stimulus;
- test oracle;
- public claim;
- generated relationships;
- conflict resolution;
- integration basis.

Refresh only the dimensions changed by the latest edit. Unrelated movement on `main` invalidates nothing by itself.

# Useful fan-out

Focused read-only agents may inspect authority, tests, correctness, security, compatibility, or product behavior. One writer integrates accepted repairs. Conflicting reports must be resolved against canonical source and actual behavior before publication.

# Valid exits

- `PR_MERGED`
- `PR_IN_FLIGHT`
- `AUTO_MERGE_ARMED`
- `WAITING_REQUIRED_CHECKS`
- `WAITING_EXTERNAL_REVIEW`
- `WAITING_INTEGRATION_PROOF`
- `PR_CLOSED_WITH_DISPOSITION`
- `EXTERNAL_BLOCKER`
- `NEEDS_OWNER_DECISION`
- `NOT_ESTABLISHED`

A remote-owned wait yields to the goal loop; it does not keep the root polling unchanged state.
