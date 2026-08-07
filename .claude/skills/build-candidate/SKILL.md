---
name: build-candidate
description: Build, harden, simplify, and challenge one current candidate for a coherent claim. Use after proof design or when an existing candidate needs repair before substantive PR review.
---

# Result

One candidate implements the claim, carries discriminating evidence, extends the correct semantic owner, contains no unnecessary parallel authority, and is ready for exact-head `review-pr` inspection.

# Route markers

- `review_route:build_candidate_to_review_pr`
- `review_route:repair_returns_to_same_candidate`

# Workflow

1. Confirm the controlling issue, claim boundary, current branch/worktree, and governing sources.
2. Read the owning production path and nearby proof before editing.
3. Implement the smallest coherent change that satisfies the claim. Extend existing authorities instead of creating another validator, owner, or route.
4. Run focused proof early. Classify failures as source, proof/oracle, instrument, environment, or not established before choosing the repair.
5. Improve the test suite:
   - add discriminating negative or alternate cases;
   - validate fixture setup and intended subject;
   - add currentness and identity checks where material;
   - preserve explicit unknown and limitation states;
   - exercise rendered/public behavior where source-text checks could remain green without changing the real route.
6. Simplify:
   - remove scaffolding and dead branches;
   - remove public placeholders and panic/todo paths;
   - collapse duplicated decisions into the owning layer;
   - keep one acceptance and rollback boundary.
7. Challenge the candidate from fresh perspectives:
   - authority, provenance, and architecture;
   - correctness, failure paths, rollback, transaction boundaries, replay, and concurrency;
   - test stimulus and oracle grip;
   - runtime/schema/docs/help/output parity;
   - security, privacy, platform, packaging, compatibility, performance, and user-facing claim honesty where relevant.
8. Repair accepted findings through the same candidate.
9. Run `cargo xtask precommit`, focused tests, and changed-surface gates. Report missing or infrastructure-limited evidence without converting it to pass.
10. Commit the coherent candidate so the review subject is one exact Git object. An uncommitted worktree cannot receive an exact-head disposition.
11. Route that committed head to `review-pr`. Candidate challenge during implementation is not the final substantive PR review, and green checks or an empty thread list do not replace it.
12. Before PR publication, unavailable hosted checks, artifacts, and external review normally produce `REVIEW_INCOMPLETE`. Route the exact candidate to `finish-pr` for publication and re-enter `review-pr` on the published head before merge convergence.
13. When `review-pr` returns `REPAIR_REQUIRED`, repair the same candidate, recommit, and refresh only affected proof/review dimensions before reviewing again.

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

- `CANDIDATE_READY_FOR_REVIEW`
- `CANDIDATE_REPAIRED`
- `PROOF_NEEDS_REPAIR`
- `PLAN_OR_ISSUE_NEEDS_REPAIR`
- `REPAIR_REQUIRED`
- `REVIEW_INCOMPLETE`
- `INSTRUMENT_FAILURE`
- `INFRASTRUCTURE_FAILURE`
- `EXTERNAL_BLOCKER`
- `NOT_ESTABLISHED`
