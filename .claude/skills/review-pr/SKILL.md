---
name: review-pr
description: Perform a substantive exact-head review of one pull request or current candidate before merge convergence. Use after candidate hardening, after a material head change, or when review quality/currentness is uncertain.
---

# Result

The exact current PR head has a source-backed review disposition. Blocking defects, suggestions, stale or refuted findings, missing proof/review, and infrastructure or instrument failures remain distinct. The review determines whether the candidate may enter merge convergence; GitHub state and green CI are evidence inputs, not semantic acceptance.

# Contract markers

These stable IDs declare the load-bearing semantics without forcing Claude and
Codex to use identical prose or choreography. `cargo xtask check-agent-skills`
validates the closed marker set; normal review still judges whether the written
procedure and actual behavior earn those declarations.

- `review_contract:exact_head_binding`
- `review_contract:non_mutating_inspection_workspace`
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

An exact head is a committed Git object. Uncommitted edits cannot receive an
exact-head review disposition.

The same candidate may receive two deliberate passes:

1. **Pre-publication candidate review.** Bind the committed branch head and
   inspect source, proof, and repository contracts. Remote checks, artifacts,
   and external review that do not exist yet remain missing, so this normally
   exits `REVIEW_INCOMPLETE` rather than `REVIEW_READY`.
2. **Published merge review.** Bind the exact PR head, inspect current hosted
   jobs, artifacts, comments, and integration evidence, and refresh only the
   dimensions changed since the candidate pass. Only this published-head pass
   may emit `REVIEW_READY` for merge convergence.

Publication adds remote evidence; it does not erase an earlier valid source and
oracle inspection.

# Inspection workspace

Review is a read of a committed Git object. Binding an exact head must not mutate
a checkout the reviewer does not own.

Prefer routes that materialize nothing:

```bash
gh pr view <n> --json headRefOid,baseRefName,body,files
gh pr diff <n>
git fetch origin pull/<n>/head          # updates FETCH_HEAD only
git show <sha>:<path>
git diff <base>...<sha> -- <path>
```

When the review genuinely needs a materialized tree — exercising the real CLI,
LSP, packaging, or generated output under `review_contract:rendered_behavior` —
take a dedicated worktree or clone and work only inside it:

```bash
git worktree add --detach <own-path> <sha>
```

Never run `git checkout`, `git switch`, `git reset`, or any other HEAD-moving
command in a shared root checkout or a sibling candidate worktree. Those belong
to their writer, and a clean tree at the moment you look is not evidence that
they are idle. Remove a review-created worktree when the pass ends.

This is the reader half of the operating law in `CLAUDE.md`: one writer mutates
each candidate branch or worktree, while readers, researchers, reviewers, and
tools inspect it. It adds no reservation, lease, lock, or sibling-monitoring
surface, and none may be introduced to satisfy it.

# When to enter

Use `review-pr` when:

- `build-candidate` has produced a candidate that appears ready;
- an existing PR lacks a substantive review on its current head;
- a production, oracle, public-claim, generated, conflict, or integration edit changed review currentness;
- automated review and CI results need an accountable synthesis;
- a prior inspection covered another head or only part of the diff.

The lead Claude context may perform the pass directly. Delegate only when another context changes the available evidence, oracle, tools, threat model, platform access, or failure perspective. The lead verifies the returned evidence and owns the final disposition.

# Workflow

1. **Bind the subject.** Retain repository, PR or branch, exact `reviewed_head_sha`, base and integration basis, issue/claim boundary, acceptance, non-goals, and review time. Re-read the head before posting. A moved head invalidates affected review dimensions. Bind it from an inspection workspace you own; see **Inspection workspace**.
2. **Read the whole current-head change.** Inspect the complete diff, changed-file inventory, PR body, governing issue/spec/ADR/policy, semantic owner, and real consumers. Trace changed data and decisions into rendered/public behavior rather than stopping at the edited function.
3. **Create a claim/evidence map.** For each material behavior or public claim, name the owner, consumer, positive evidence, discriminating alternate/negative evidence, and explicit limitation or non-claim.
4. **Review the applicable lanes.** Explain why any load-bearing lane is not applicable.
   - **Authority/provenance:** semantic ownership, producer authenticity, identity, currentness, source-of-truth direction, compatibility authority.
   - **Correctness/survivability:** failure paths, rollback, cleanup, partial publication, transaction boundaries, replay, stale input, TOCTOU, races, concurrency, idempotence, recovery.
   - **Stimulus/oracle grip:** fixture construction, nonempty intended subject, production-path reachability, and whether the old or wrong behavior can still satisfy the test.
   - **Rendered behavior:** actual CLI, API, LSP, help, generated, or output surface where practical. Source-text substring tests are not rendered-behavior proof.
   - **Contract parity:** runtime validation, schema, docs, help, output contracts, compatibility history, generated relationships, and support claims agree.
   - **Platform/package/security:** platform branches, filesystem/process semantics, package contents, permissions, trust, secret, and network boundaries when engaged.
   - **Exact-head CI/receipts:** required and advisory checks, actual jobs, denominators, skipped lanes, failures, artifacts, reports, and head identity. Inspect the failing step or receipt; aggregate status is not the cause.
5. **Challenge load-bearing claims.** Use a counterexample, mutation/removal experiment, deliberately wrong implementation, alternate case, or record why such a challenge is not practical. A green test that cannot distinguish the old behavior is an oracle defect.
6. **Classify results without collapsing them:**
   - blocking source/contract defect;
   - blocking test/oracle defect;
   - blocking missing platform/integration proof;
   - non-blocking suggestion;
   - stale or refuted finding with evidence;
   - missing review/proof;
   - instrument failure;
   - infrastructure failure;
   - not established.
7. **Post the inspection record on the exact head.** A clean result documents inspected surfaces, risks, invariants, validation, residual assumptions, missing evidence, and disposition. `LGTM` is not a review record.
8. **Handle author self-review honestly.** GitHub does not allow the author to request changes on their own PR. Submit a `COMMENT` review and state whether the current head is blocking or review-ready. Do not treat the platform limitation as an approval.
9. **Repair through one candidate.** Return valid findings to `build-candidate`; reply to incorrect findings with source-backed evidence; resolve only after repair or reply. Re-enter `review-pr` for dimensions changed by the repair.
10. **Route the disposition.** Only `REVIEW_READY` proceeds to `finish-pr`. Other outcomes retain the candidate as draft, repair-required, incomplete, or durably in flight.

# False-confidence rules

- Zero unresolved threads does not mean the current head was reviewed.
- Green required CI is not semantic correctness.
- Quota-limited, unavailable, skipped, failed, or stale reviewer output is missing review.
- Zero intended subjects is `not_established`, even when a command exits zero.
- Per-file atomic writes are not a whole-attempt transaction.
- Hashes bind bytes, not their claimed producer, invocation, seam, or repository state.
- Raw source-string checks do not prove rendered command/help/output behavior.
- PR prose and docs cannot strengthen runtime/schema authority.
- `mergeStateStatus: BLOCKED` does not identify a human approval dependency.
- A clean working tree in someone else's checkout is not permission to move its HEAD.

# Required review record

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

Lead with actionable findings in severity order. Include file/symbol or contract locations and the concrete failure mode. Put synthesis after findings rather than using a summary to bury them.

# Currentness dimensions

Track:

- production implementation;
- test stimulus;
- test oracle;
- public claim;
- generated relationships;
- conflict resolution;
- integration basis;
- candidate head identity.

Refresh only dimensions changed by a later edit. Unrelated `main` movement does not invalidate the review.

# Valid outcomes

- `REVIEW_READY`
- `REPAIR_REQUIRED`
- `REVIEW_INCOMPLETE`
- `INSTRUMENT_FAILURE`
- `INFRASTRUCTURE_FAILURE`
- `EXTERNAL_BLOCKER`
- `NOT_ESTABLISHED`
