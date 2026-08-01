---
name: build-candidate
description: Build, harden, simplify, and challenge one current candidate for a coherent claim. Use after proof design or when an existing implementation needs repair before PR convergence.
---

# Useful result

One candidate implements the selected claim, carries discriminating proof, respects the semantic owner, has no unnecessary parallel authority, and is ready for publication or current-head review.

# Procedure

1. Confirm the issue, claim boundary, current candidate branch/worktree, and governing sources.
2. Read the owning production path and nearby tests before editing.
3. Make the smallest coherent implementation that satisfies the claim. Do not create a second validator, owner, or route when an existing authority should be extended.
4. Run focused proof early. Treat failures as information about source, proof, instrument, or environment rather than retrying blindly.
5. Improve the test suite:
   - add the discriminating negative or alternate case;
   - validate fixture setup and nonempty subject;
   - add currentness or identity checks where relevant;
   - preserve fail-closed unknown and limitation states.
6. Simplify the candidate:
   - remove temporary scaffolding and dead branches;
   - collapse duplicated logic into the owning layer;
   - remove public placeholders and panic/todo paths;
   - keep the PR's acceptance and rollback boundary coherent.
7. Challenge the candidate with fresh criteria:
   - authority and architecture;
   - correctness and edge cases;
   - test grip;
   - security/privacy/compatibility where relevant;
   - user-facing claim honesty.
8. Repair every accepted finding through the same candidate.
9. Run `cargo xtask precommit`, focused tests, and the additional gates required by the changed surface. Report incomplete or infrastructure-limited evidence honestly.
10. Prepare the candidate for `finish-pr`.

# Candidate law

- One claim normally has one current candidate.
- One writer mutates the branch/worktree at a time.
- Focused agents may research or review read-only.
- Do not create rival candidates merely to produce parallel activity.
- Do not scan sibling lanes for file overlap or reserve surfaces.
- If an earlier PR creates a real conflict, this candidate owns its focused reconciliation and affected re-proof.

# Decision law

Choose and document reasonable reversible decisions. Escalate only when materially different viable outcomes remain after source research and safe implementation experiments.

# Valid exits

- `CANDIDATE_READY`
- `CANDIDATE_REPAIRED`
- `PROOF_NEEDS_REPAIR`
- `PLAN_OR_ISSUE_NEEDS_REPAIR`
- `INSTRUMENT_FAILURE`
- `INFRASTRUCTURE_FAILURE`
- `EXTERNAL_BLOCKER`
- `NOT_ESTABLISHED`
