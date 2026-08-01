---
name: build-candidate
description: Build, harden, simplify, and challenge one current candidate for a coherent claim. Use after proof design or when an existing candidate needs repair before PR convergence.
---

# Result

One candidate implements the claim, carries discriminating evidence, extends the correct semantic owner, contains no unnecessary parallel authority, and is ready for publication or exact-head review.

# Workflow

1. Confirm the controlling issue, claim boundary, current branch/worktree, and governing sources.
2. Read the owning production path and nearby proof before editing.
3. Implement the smallest coherent change that satisfies the claim. Extend existing authorities instead of creating another validator, owner, or route.
4. Run focused proof early. Classify failures as source, proof/oracle, instrument, environment, or not established before choosing the repair.
5. Improve the test suite:
   - add discriminating negative or alternate cases;
   - validate fixture setup and intended subject;
   - add currentness and identity checks where material;
   - preserve explicit unknown and limitation states.
6. Simplify:
   - remove scaffolding and dead branches;
   - remove public placeholders and panic/todo paths;
   - collapse duplicated decisions into the owning layer;
   - keep one acceptance and rollback boundary.
7. Challenge the candidate from fresh perspectives:
   - authority and architecture;
   - correctness and edge cases;
   - test grip;
   - security, privacy, compatibility, and performance where relevant;
   - user-facing claim honesty.
8. Repair accepted findings through the same candidate.
9. Run `cargo xtask precommit`, focused tests, and changed-surface gates. Report missing or infrastructure-limited evidence without converting it to pass.
10. Route the coherent candidate to `finish-pr`.

# Mutation boundary

- One claim normally has one current candidate.
- One writer mutates the candidate branch or worktree at a time.
- Claude subagents may research or review read-only; one lead integrates accepted repairs.
- Do not create competing implementations merely to use parallel capacity.
- Do not scan sibling lanes for overlap or reserve surfaces.
- This lane owns focused conflict resolution and re-proof only after a real conflict or integration failure exists.

# Decisions

Make and document reasonable reversible engineering decisions. Escalate only when materially different viable outcomes remain after current-source research and safe implementation experiments.

# Valid outcomes

- `CANDIDATE_READY`
- `CANDIDATE_REPAIRED`
- `PROOF_NEEDS_REPAIR`
- `PLAN_OR_ISSUE_NEEDS_REPAIR`
- `INSTRUMENT_FAILURE`
- `INFRASTRUCTURE_FAILURE`
- `EXTERNAL_BLOCKER`
- `NOT_ESTABLISHED`
