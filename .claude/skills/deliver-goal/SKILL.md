---
name: deliver-goal
description: Carry a high-level repository outcome through the distinct PR-sized claims required to satisfy it. Use when the user states an end state rather than one already-scoped issue or PR.
---

# Result

The user's original goal remains visible, its current interpretation and acceptance predicates are explicit, and the repository advances through distinct claims until the end state is satisfied, externally blocked, materially undecidable, or honestly not established.

# Workflow

1. Preserve the goal verbatim. Do not translate it into the first issue you happen to find.
2. State the current interpretation:
   - desired state;
   - constraints and maturity boundary;
   - non-goals;
   - assumptions;
   - unresolved material decisions;
   - acceptance predicates.
3. Reconstruct current truth from current `main`, GitHub issues and PRs, checks, controlling specs/ADRs/policies, and the real production consumer.
4. Reconcile work by claim identity:
   - resume an equivalent PR;
   - update or create the controlling issue;
   - honor explicit prerequisites;
   - ignore sibling file, crate, and symbol overlap unless Git or proof produces a concrete interaction.
5. Select one coherent required claim and use `deliver-pr`.
6. When GitHub owns the next transition—required CI, external review, auto-merge, or merge queue—leave that PR in flight and advance another distinct required claim when useful.
7. Revisit an in-flight PR only after a material event: a substantive finding, failed required check, changed candidate head, concrete conflict, changed prerequisite, merge, or closure.
8. Reconcile issue and parent-goal state after every merge or deliberate closure.
9. Evaluate every predicate as `pass`, `failed`, `limited`, `not_applicable`, or `not_established`.
10. Stop only when the goal is satisfied, all remaining work shares a genuine external blocker, a material non-derivable owner decision remains, or the result is not established.

# Engineering decisions

Do not stop merely because several implementations are possible. Read the sources, choose the strongest reversible option, record the rationale, and proceed. Ask for an owner decision only when materially different viable outcomes remain after safe research and reversible experiments.

# Parallel work

- Many distinct claims may be in flight.
- One claim normally has one current candidate.
- One writer mutates a candidate branch or worktree at a time.
- Claude subagents or Agent Teams may perform focused read-only research, oracle review, or specialist review when they improve evidence, context, cost, or elapsed time.
- Do not run candidate tournaments, inspect sibling worktrees, reserve files, or maintain overlap maps.
- The lead Claude context owns synthesis and verifies all load-bearing claims.

# Valid outcomes

- `GOAL_SATISFIED`
- `GOAL_PARTIAL`
- `GOAL_IN_FLIGHT`
- `EXTERNAL_BLOCKER`
- `NEEDS_OWNER_DECISION`
- `NOT_ESTABLISHED`

A PR waiting on CI or review is normally `GOAL_IN_FLIGHT`, not a blocker. Exhausting known issues is not evidence that the goal is satisfied.
