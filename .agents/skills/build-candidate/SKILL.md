---
name: build-candidate
description: Build, harden, simplify, and challenge one current candidate for a coherent claim. Use after proof design or when an existing implementation needs repair before PR review and convergence. Establish an inherited baseline floor before mutation, drive a discriminating control before the production repair, and promote only exact-head evidence.
---

# Useful result

One candidate implements the selected claim, carries discriminating proof, respects the semantic owner, has no unnecessary parallel authority, separates inherited failures from candidate failures, and is ready for substantive exact-head review.

# Route markers

- `review_route:build_candidate_to_review_pr`
- `review_route:repair_returns_to_same_candidate`

# Procedure

1. Confirm the issue, claim boundary, current candidate branch/worktree, exact base SHA, semantic owner, and governing sources.
2. Establish the inherited baseline floor before the first mutation:
   - run `cargo xtask worktree doctor` and read its report;
   - require `git merge-base origin/main HEAD` to equal the recorded base SHA before `check-fast` may support base attribution. If it does not, reconcile history and record the resulting accepted base, or run base-aware gates directly; do not cite `check-fast` as base-authoritative;
   - on a new candidate, run `cargo xtask check-fast` before editing and bind the result to the recorded base SHA. This is a cheap policy floor, not a claim that every full gate or package compiles;
   - independently require `git diff --name-only origin/main...HEAD` to succeed before citing `check-fast`. Zero changed paths are expected only when `HEAD` is the recorded base. The strict command fail-closes selector loss, while the independent diff remains the denominator receipt and protects against stale or direct helper use; an unverified zero-path result is `INSTRUMENT_FAILURE`, not pass;
   - before editing a Rust or repo-control candidate, run the narrowest `cargo check` that covers the semantic owner on the exact base (`cargo check -p <package> --all-targets` by default; widen to `--workspace --all-targets` for shared manifests, workspace policy, or cross-package behavior). For a genuinely non-Rust-only candidate, record the omitted compile dimension explicitly. A zero-diff `check-fast` result is not compile evidence;
   - when resuming an already-mutated candidate, run the same diagnostics immediately and reproduce any apparent inherited failure in a clean isolated worktree at the exact base before attributing it;
   - run full `cargo xtask precommit` in that isolated exact-base worktree only when the selected claim is a baseline repair or a later candidate failure needs an authoritative comparison;
   - route a real base failure separately unless the selected claim is the baseline repair. Do not absorb unrelated drift into this candidate.
3. Read the owning production path, nearby tests, fixtures, and the strongest known-wrong or boundary case before editing production code.
4. Put a discriminating control ahead of the implementation:
   - add or strengthen the smallest test, fixture, or artifact assertion that should reject the missing behavior;
   - observe it fail for the intended reason against the pre-repair state;
   - for an existing implementation, bind the failure to its parent/pre-repair head or use a reversible negative mutation;
   - if no safe failing observation is available, report that proof dimension as `NOT_ESTABLISHED` rather than inventing a red/green sequence.
5. Make the smallest coherent implementation that satisfies the claim. Do not create a second validator, owner, or route when an existing authority should be extended.
6. Rerun the focused control immediately after each coherent edit. Treat failures as information about source, proof, instrument, or environment rather than retrying blindly, and stop broadening when the control no longer discriminates the selected claim.
7. Improve the test suite:
   - add the discriminating negative or alternate case;
   - validate fixture setup and nonempty subject;
   - add currentness or identity checks where relevant;
   - preserve fail-closed unknown and limitation states;
   - prove rendered/public behavior when source-text coincidence could pass without the real route changing.
8. Simplify the candidate:
   - remove temporary scaffolding and dead branches;
   - collapse duplicated logic into the owning layer;
   - remove public placeholders and panic/todo paths;
   - keep the PR's acceptance and rollback boundary coherent.
9. Challenge the candidate with fresh criteria:
   - authority, provenance, and architecture;
   - correctness, failure paths, rollback, transaction boundaries, replay, and concurrency;
   - test stimulus and oracle grip;
   - runtime/schema/docs/help/output parity;
   - platform, packaging, process, security, and user-facing claim honesty where relevant.
10. Repair every accepted finding through the same candidate.
11. Commit the coherent candidate so broad verification and review bind to one exact Git object. An uncommitted worktree cannot receive an exact-head disposition.
12. Run `cargo xtask check-fast` again on that exact committed head as the first broad candidate gate, then run `cargo xtask precommit`, the focused tests, and the additional gates required by the changed surface. Before citing `check-fast`, confirm that the accepted merge-base still equals the recorded base SHA, then compare the independently resolved committed path set with `check-fast-selector.md` and the ran/skipped categories in `check-fast.md`; a base mismatch, unexpected zero, omitted path category, or selector failure is `INSTRUMENT_FAILURE`. Reconcile history or run the affected base-aware gates directly, and do not convert that partial route to pass. Read the emitted reports, not only the exit code. If repair changes the head, recommit and rerun the affected dimensions before review.
13. Hand the exact committed head to `review-pr`. Candidate challenge inside the builder is not the final PR review, and green CI or zero review threads cannot replace that pass.
14. Before a PR exists, `review-pr` may return `REVIEW_INCOMPLETE` because remote checks, artifacts, and review evidence are unavailable. Route that exact candidate to `finish-pr` for publication, then re-enter `review-pr` on the published PR head before merge convergence.
15. If `review-pr` returns `REPAIR_REQUIRED`, repair the same candidate, recommit, and refresh only the affected review/proof dimensions before reviewing again.

# Candidate law

- One claim normally has one current candidate.
- One writer mutates the branch/worktree at a time.
- Focused agents may research or review read-only.
- Inherited failures are not candidate failures until they reproduce against the recorded base.
- A test added after the implementation is not discriminating evidence by itself; bind it to an observed known-wrong state.
- Do not create rival candidates merely to produce parallel activity.
- Do not scan sibling lanes for file overlap or reserve surfaces.
- If an earlier PR creates a real conflict, this candidate owns its focused reconciliation and affected re-proof.

# Decision law

Choose and document reasonable reversible decisions. Escalate only when materially different viable outcomes remain after source research and safe implementation experiments.

# Valid exits

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
