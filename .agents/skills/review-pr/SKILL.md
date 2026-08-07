---
name: review-pr
description: Perform a substantive exact-head review of one pull request or current candidate before merge convergence. Use after candidate hardening, after a material head change, or when review quality/currentness is uncertain.
---

# Useful result

The exact current PR head has an evidence-backed disposition that distinguishes blocking defects, non-blocking suggestions, refuted or stale findings, missing proof/review, and infrastructure or instrument failure. The review establishes whether the candidate may proceed to merge convergence; it does not substitute GitHub state or green CI for semantic acceptance.

# Contract markers

These stable IDs declare the load-bearing review contract without requiring
provider prose symmetry. `cargo xtask check-agent-skills` validates the closed
set; normal review still verifies whether the prose and behavior actually honor
it.

- `review_contract:exact_head_binding`
- `review_contract:semantic_owner_and_consumers`
- `review_contract:wrong_behavior_oracle_challenge`
- `review_contract:rendered_behavior`
- `review_contract:contract_parity`
- `review_contract:platform_relevance`
- `review_contract:exact_head_ci_receipts`
- `review_contract:denominator_honesty`
- `review_contract:mutation_or_removal_challenge`
- `review_contract:no_threads_is_not_review`
- `review_contract:green_ci_is_not_semantic_review`
- `review_contract:clean_review_record_not_lgtm`
- `review_contract:author_self_review_comment`
- `review_contract:review_ready_gate`
- `review_contract:repair_same_candidate`
- `review_contract:blocked_is_not_human_cause`

# Exact-head levels

An exact head is a committed Git object. Uncommitted worktree state cannot
receive an exact-head disposition.

The procedure may run twice on the same candidate lifecycle:

1. **Candidate review before publication.** Bind the committed branch head and
   inspect source, proof, and repository contracts. Hosted checks, artifacts,
   and external review that do not yet exist remain missing evidence, so the
   normal result is `REVIEW_INCOMPLETE`, not `REVIEW_READY`.
2. **Merge review after publication.** Bind the exact published PR head, inspect
   current remote checks, artifacts, comments, and integration evidence, and
   refresh only dimensions changed since the earlier review. Only this complete
   published-head pass may emit `REVIEW_READY` for merge convergence.

Publication does not erase a valid candidate review. It adds remote evidence
that must be inspected before the disposition can advance.

# Entry boundary

Use this procedure when a coherent candidate exists and one of these is true:

- the candidate appears ready for publication or merge convergence;
- an existing PR has no substantive current-head review;
- a material production, test-oracle, public-claim, generated, conflict, or integration edit changed review currentness;
- automated comments or CI results need an accountable synthesis;
- a prior review was anchored to another head or an incomplete diff.

A differently named reviewer is optional. Use another agent only when it changes the evidence, oracle, context, tools, threat model, platform reach, or failure perspective. The accountable root verifies the review and owns the disposition.

# Procedure

1. **Bind the review subject.** Record:
   - repository and PR number or candidate branch;
   - `reviewed_head_sha`;
   - base branch and exact integration basis when known;
   - controlling issue, claim boundary, acceptance, and non-goals;
   - review timestamp.
   Re-read the head before posting the disposition. A moved head makes the prior review stale for affected dimensions.
2. **Read the complete current-head change.** Inspect the full diff, changed-file inventory, PR body, issue, applicable specs/ADRs/policy, and the owning production path. Follow changed values into actual consumers and rendered/public surfaces; do not review the patch in isolation.
3. **Map claims to evidence.** For each material production or public claim, identify:
   - semantic owner;
   - retained consumer or decision path;
   - positive evidence;
   - discriminating negative/alternate evidence;
   - explicit limitation or non-claim.
4. **Run the applicable review lanes.** Do not mechanically apply irrelevant lanes, but explain omissions for load-bearing claims.
   - **Authority and provenance:** semantic ownership, identity, currentness, producer authenticity, source-of-truth direction, and compatibility authority.
   - **Correctness and survivability:** failure paths, rollback, cleanup, partial publication, atomicity, replay, stale input, race/TOCTOU, concurrency, idempotence, and recovery.
   - **Test stimulus and oracle grip:** fixture construction, nonempty intended subjects, production-path reachability, whether the old or wrong behavior can still pass, and whether assertions observe the actual decision rather than nearby shape.
   - **Rendered behavior:** exercise the real CLI/API/LSP/output/help surface where practical. Raw source-text coincidence is not proof that rendered behavior or command routing is correct.
   - **Contract parity:** runtime validation, schemas, docs, help, generated artifacts, compatibility history, output contracts, and support claims must agree. One layer accepting a state another rejects is a finding.
   - **Platform, packaging, process, and security:** inspect platform-specific branches, filesystem/process semantics, package contents, permissions, trust boundaries, and secret/network behavior when the diff engages them.
   - **Exact-head CI and receipts:** inspect required and advisory checks, actual jobs, denominators, skipped lanes, failures, workflow artifacts, and head identity. Read the failing step or retained report; do not use an aggregate status as the explanation.
5. **Challenge every load-bearing claim.** Use at least one counterexample, mutation/removal experiment, deliberately wrong implementation, alternate case, or explicit reason why such an experiment is impractical. A test that cannot distinguish the old or wrong behavior is a test-oracle finding even when it is green.
6. **Classify findings.** Keep these separate:
   - blocking source or contract defect;
   - blocking test/oracle defect;
   - blocking missing platform/integration proof;
   - non-blocking suggestion;
   - refuted or stale finding with evidence;
   - missing review or proof;
   - instrument failure;
   - infrastructure failure;
   - not established.
7. **Post a current-head inspection record.** A clean review is not `LGTM`. Name the inspected surfaces, risks, invariants, validation signals, residual assumptions, missing evidence, and exact disposition.
8. **Respect GitHub's author-review limitation.** On the author's own PR, submit a `COMMENT` review with an explicit blocking or non-blocking disposition. GitHub's refusal to let an author request changes is a platform constraint, not approval and not evidence that the PR is ready.
9. **Repair through the same candidate.** Route valid findings back to `build-candidate`. Reply to incorrect automated findings with source-backed evidence. Resolve a thread only after repair or reply exists. Re-run `review-pr` for the affected currentness dimensions after the head changes.
10. **Hand off only an earned disposition.** `REVIEW_READY` may proceed to `finish-pr`. All other non-terminal results keep the candidate draft/in repair or durably in flight.

# False-confidence prohibitions

- No unresolved threads does not mean substantive review occurred.
- Green required CI does not establish semantic correctness.
- Reviewer quota, unavailability, skip, failure, or stale output is missing review, not clean review.
- A pass with zero intended subjects is `not_established`, not proof.
- Individually atomic file writes do not establish an atomic multi-file or directory transaction.
- Digest binding does not prove the bytes came from the named producer, seam, invocation, or repository state.
- Source-text assertions do not prove rendered help, CLI routing, or public output.
- Documentation and PR prose do not override runtime/schema/code authority.
- `mergeStateStatus: BLOCKED` is not a causal diagnosis and never proves a human approval requirement.

# Required inspection record

```text
reviewed_head_sha:
integration_basis:
claim_boundary:
changed_surfaces_and_semantic_owners_inspected:
blocking_findings:
non_blocking_suggestions:
refuted_or_stale_findings:
proof_and_ci_observed:
proof_or_review_missing:
affected_currentness_dimensions:
residual_assumptions_and_non_claims:
disposition:
```

When findings exist, lead with them in severity order and include file/symbol or contract locations plus the failure mode. Summaries follow the findings; they do not hide them.

# Review currentness

Track at least:

- production implementation;
- test stimulus;
- test oracle;
- public claim;
- generated relationships;
- conflict resolution;
- integration basis;
- candidate head identity.

Refresh only dimensions changed by a later edit. Unrelated movement on `main` invalidates nothing by itself.

# Valid exits

- `REVIEW_READY`
- `REPAIR_REQUIRED`
- `REVIEW_INCOMPLETE`
- `INSTRUMENT_FAILURE`
- `INFRASTRUCTURE_FAILURE`
- `EXTERNAL_BLOCKER`
- `NOT_ESTABLISHED`
