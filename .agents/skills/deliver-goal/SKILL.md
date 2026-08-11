---
name: deliver-goal
description: Carry a high-level repository outcome through the distinct PR-sized claims needed to satisfy it. Use when the user states an end state rather than one already-scoped issue or PR.
---

# Useful result

The original goal is preserved, its current interpretation and acceptance predicates are explicit, every required claim is either delivered, in flight, externally blocked, or honestly not established, and the goal is closed only when the end state exists.

# Procedure

1. Preserve the user's goal verbatim. Do not replace it with the first plausible issue.
2. Record the current interpretation:
   - desired end state;
   - constraints and maturity boundary;
   - non-goals;
   - material assumptions;
   - unresolved owner decisions;
   - acceptance predicates.
3. Reconstruct current truth from current `main`, GitHub issues and PRs, required checks, controlling specs/ADRs/policies, and the owning production path.
4. Reconcile existing work by claim identity:
   - resume an equivalent existing PR;
   - reuse or update an existing issue;
   - respect an explicit prerequisite;
   - do not infer ownership from nearby files, crates, or symbols.
5. Select one coherent required claim whose delivery would move a goal predicate. Use `deliver-pr`.
6. When a PR reaches a remote-owned state such as required CI, external review, auto-merge, or merge queue, retain it as in flight and advance another distinct required claim when useful.
7. Revisit in-flight work only after a material transition: a finding, failed required check, changed head, concrete conflict, changed prerequisite, merge, or closure.
8. After every merge or deliberate closure, reconcile the issue, remaining acceptance, parent goal, and any generated evidence.
9. Re-evaluate every goal predicate as one of:
   - `pass`;
   - `failed`;
   - `limited`;
   - `not_applicable`;
   - `not_established`.
10. Stop only when the goal is satisfied, every remaining required claim shares a genuine external blocker, a material non-derivable owner decision remains, or the result is honestly not established.

# Decision law

The existence of several reasonable engineering choices does not require escalation. Research the governing sources, choose the strongest reversible option, document the rationale, and proceed. Return `NEEDS_OWNER_DECISION` only when materially different viable outcomes remain after safe research and reversible engineering are exhausted.

# Release-scope law

When a release is pinned, the reviewed immutable pin receipt is the sole membership authority; qualification, source preflight, and finalization consume its exact ref, ancestry, ordered SHA digest, PR dispositions, and manifests unchanged. Ordinary `main` or swarm movement never repins or changes membership. Repin only after a release-invalidating exact-candidate qualification or source-preflight failure, with an explicit superseding receipt. Never close, draft, lock, relabel, retarget, or otherwise mutate an unrelated PR to freeze that scope; unrelated PRs stay open and may evolve. Later merges do not retarget the pinned release. Close only the selected PR for its own evidence-backed terminal disposition—never close it now to reopen it after release.

# Concurrency law

- Many distinct claims may be in flight.
- One claim normally has one current candidate.
- One writer mutates a candidate branch or worktree at a time.
- Readers, researchers, and reviewers may inspect the candidate when they improve evidence or elapsed time.
- Do not monitor sibling implementations, reserve files, or build overlap maps.
- Check other work only for the same claim, an explicit prerequisite, or a concrete Git/integration conflict.

# Useful fan-out

Use focused read-only agents only when they change the evidence or context, for example:

- repository and authority mapping;
- external semantic research;
- test-oracle challenge;
- security, privacy, compatibility, or product review.

The root owns synthesis. Subagent reports are leads until verified against artifacts.

# Valid exits

- `GOAL_SATISFIED`
- `GOAL_PARTIAL`
- `GOAL_IN_FLIGHT`
- `EXTERNAL_BLOCKER`
- `NEEDS_OWNER_DECISION`
- `NOT_ESTABLISHED`

A waiting PR is normally `GOAL_IN_FLIGHT`, not `EXTERNAL_BLOCKER`. “No more issues found” is never equivalent to `GOAL_SATISFIED`.
