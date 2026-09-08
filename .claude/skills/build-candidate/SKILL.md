---
name: build-candidate
description: Build, harden, simplify, and challenge one current candidate for a coherent claim. Use after proof design or when an existing candidate needs repair before substantive PR review. Establish an inherited baseline floor before mutation, drive a discriminating control before the production repair, and promote only exact-head evidence.
---

# Result

One candidate implements the claim, carries discriminating evidence, extends the correct semantic owner, contains no unnecessary parallel authority, separates inherited failures from candidate failures, and is ready for exact-head `review-pr` inspection.

# Route markers

- `review_route:build_candidate_to_review_pr`
- `review_route:repair_returns_to_same_candidate`

# Workflow

1. Confirm the controlling issue, claim boundary, current branch/worktree, exact base SHA, semantic owner, and governing sources.
2. Establish the inherited baseline floor before the first mutation:
   - run `cargo xtask worktree doctor` and read its report;
   - require `git merge-base origin/main HEAD` to equal the recorded base SHA before `check-fast` may support base attribution. If it does not, reconcile history and record the resulting accepted base, or run base-aware gates directly; do not cite `check-fast` as base-authoritative;
   - on a new candidate, run `cargo xtask check-fast` before editing and bind the result to the recorded base SHA. This is a cheap policy floor, not a claim that every full gate or package compiles;
   - independently require `git diff --name-only origin/main...HEAD` to succeed before citing `check-fast`. Zero changed paths are expected only when `HEAD` is the recorded base. The strict command fail-closes selector loss, while the independent diff remains the denominator receipt and protects against stale or direct helper use; an unverified zero-path result is `INSTRUMENT_FAILURE`, not pass;
   - before editing a Rust or repo-control candidate, run the narrowest `cargo check` that covers the semantic owner on the exact base (`cargo check -p <package> --all-targets` by default; widen to `--workspace --all-targets` for shared manifests, workspace policy, or cross-package behavior). For a genuinely non-Rust-only candidate, record the omitted compile dimension explicitly. A zero-diff `check-fast` result is not compile evidence;
   - when resuming an already-mutated candidate, run the same diagnostics immediately and reproduce any apparent inherited failure in a clean isolated worktree at the exact base before attributing it;
   - run full `cargo xtask precommit` in that isolated exact-base worktree only when the selected claim is a baseline repair or a later candidate failure needs an authoritative comparison;
   - route a real base failure separately unless the selected claim is the baseline repair. Do not absorb unrelated drift into this candidate.
3. Read the owning production path, nearby proof, fixtures, and the strongest known-wrong or boundary case before editing production code.
4. Put a discriminating control ahead of the implementation:
   - add or strengthen the smallest test, fixture, or artifact assertion that should reject the missing behavior;
   - observe it fail for the intended reason against the pre-repair state;
   - for an existing implementation, bind the failure to its parent/pre-repair head or use a reversible negative mutation;
   - if no safe failing observation is available, report that proof dimension as `NOT_ESTABLISHED` rather than inventing a red/green sequence.
5. Implement the smallest coherent change that satisfies the claim. Extend existing authorities instead of creating another validator, owner, or route.
6. Rerun the focused control immediately after each coherent edit. Classify failures as source, proof/oracle, instrument, environment, or not established before choosing the repair, and stop broadening when the control no longer discriminates the selected claim.
7. Improve the test suite:
   - add discriminating negative or alternate cases;
   - validate fixture setup and intended subject;
   - add currentness and identity checks where material;
   - preserve explicit unknown and limitation states;
   - exercise rendered/public behavior where source-text checks could remain green without changing the real route.
8. Simplify:
   - remove scaffolding and dead branches;
   - remove public placeholders and panic/todo paths;
   - collapse duplicated decisions into the owning layer;
   - keep one acceptance and rollback boundary.
9. Challenge the candidate from fresh perspectives:
   - authority, provenance, and architecture;
   - correctness, failure paths, rollback, transaction boundaries, replay, and concurrency;
   - test stimulus and oracle grip;
   - runtime/schema/docs/help/output parity;
   - security, privacy, platform, packaging, compatibility, performance, and user-facing claim honesty where relevant.
10. Repair accepted findings through the same candidate.
11. Commit the coherent candidate so broad verification and review bind to one exact Git object. An uncommitted worktree cannot receive an exact-head disposition.
12. Run `cargo xtask check-fast` again on that exact committed head as the first broad candidate gate, then run `cargo xtask precommit`, focused tests, and changed-surface gates. Before citing `check-fast`, confirm that the accepted merge-base still equals the recorded base SHA, then compare the independently resolved committed path set with `check-fast-selector.md` and the ran/skipped categories in `check-fast.md`; a base mismatch, unexpected zero, omitted path category, or selector failure is `INSTRUMENT_FAILURE`. Reconcile history or run the affected base-aware gates directly, and do not convert that partial route to pass. Read the emitted reports, not only the exit code. If repair changes the head, recommit and rerun the affected dimensions before review.
13. Route that committed head to `review-pr`. Candidate challenge during implementation is not the final substantive PR review, and green checks or an empty thread list do not replace it.
14. Before PR publication, unavailable hosted checks, artifacts, and external review normally produce `REVIEW_INCOMPLETE`. Route the exact candidate to `finish-pr` for publication and re-enter `review-pr` on the published head before merge convergence.
15. When `review-pr` returns `REPAIR_REQUIRED`, repair the same candidate, recommit, and refresh only affected proof/review dimensions before reviewing again.

# Mutation boundary

- One claim normally has one current candidate.
- One writer mutates the candidate branch or worktree at a time.
- Claude subagents may research or review read-only; one lead integrates accepted repairs.
- Inherited failures are not candidate failures until they reproduce against the recorded base.
- A test added after the implementation is not discriminating evidence by itself; bind it to an observed known-wrong state.
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
