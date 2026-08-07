---
name: deliver-pr
description: Carry one coherent issue or claim from current repository truth through proof, implementation, substantive current-head review, merge, and reconciliation. Use for a selected PR-sized lane or an existing PR.
---

# Result

One coherent claim has one current candidate, current evidence and review for affected seams, an accurate GitHub PR state, and an issue disposition that distinguishes delivered from residual work.

# Route markers

- `review_route:deliver_pr_to_review_pr`

# Workflow

1. Hydrate the claim from the issue, accepted artifacts, current source, and any existing PR.
2. Search for an equivalent PR before creating a branch. Continue the existing candidate when one owns the same claim.
3. Find the earliest absent or stale judgment:
   - premise and issue quality;
   - proof design;
   - implementation;
   - test hardening;
   - simplification;
   - candidate challenge;
   - substantive current-head review;
   - review repair;
   - integration proof;
   - reconciliation.
4. Enter there. Do not recreate completed stages because this Claude session arrived later.
5. Use `prepare-issue` when the premise or acceptance is missing or wrong.
6. Use `prepare-proof` when the oracle is absent, weak, or disconnected from production.
7. Use `build-candidate` to implement or repair the current candidate and materialize an exact committed head.
8. Run `review-pr` on that exact head. Before publication, retain unavailable remote checks, artifacts, and external review as `REVIEW_INCOMPLETE` rather than pass.
9. Use `finish-pr` to publish or resume the exact candidate when no equivalent PR exists.
10. Re-enter `review-pr` on the exact published PR head after remote evidence is available. Green CI, zero unresolved threads, and unavailable reviewers do not establish substantive review.
11. Route `REPAIR_REQUIRED` through the same candidate and refresh only affected proof/review dimensions.
12. Only a current `REVIEW_READY` PR head may enter `finish-pr` merge convergence. Incomplete or blocking review remains draft or durably in flight.
13. After merge or deliberate closure, verify current `main`, update delivered and remaining acceptance, update parents, and remove the completed worktree.

# Candidate boundary

- One coherent claim normally has one branch, worktree, candidate, and PR.
- Do not ask several agents to build rival versions of the same implementation merely to create parallel work.
- Separate workers may contribute genuinely disjoint pieces only through one integrating candidate owner.
- Do not inspect sibling implementations or reserve overlapping files, crates, or semantic surfaces.
- A behind-only branch stays untouched.
- Reconcile only an actual conflict, changed prerequisite, failed combined-tree proof, or applicable repository rule.

# Currentness

Review and proof currentness are dimensional:

- production implementation;
- test stimulus;
- test oracle;
- public claim;
- generated relationships;
- conflict resolution;
- integration basis;
- candidate head identity.

Refresh only what the latest edit changed. Unrelated movement on `main` does not invalidate candidate review. Reading comments and check status is remote triage; it does not replace `review-pr`'s semantic owner, oracle, contract-parity, platform, and exact-head evidence inspection.

# Subagents

Use focused subagents for repository mapping, correctness, test-oracle, security/privacy, compatibility, product, or platform review when they change the detection surface. Keep mutation serialized through one candidate owner. Verify every subagent conclusion against the cited artifacts.

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
- `EXTERNAL_BLOCKER`
- `NEEDS_OWNER_DECISION`
- `NOT_ESTABLISHED`

Do not poll unchanged GitHub state. Yield the PR to the outer goal loop while remote systems own the next transition.
