# Agent Operating Model

This repository is designed for high-discretion Claude and Codex development.
The developer may state a high-level outcome; the accountable root is expected
to research current repository truth, identify the required claims, build and
challenge candidates, carry PRs through substantive review and CI, and continue
until the actual outcome is satisfied.

Claude and Codex use separate complete instruction and skill sets:

```text
Claude: CLAUDE.md + .claude/skills/**
Codex:  AGENTS.md + .agents/skills/**
```

The two sets may intentionally repeat the same repository semantics. Neither is
a generated wrapper around the other provider's files.

For operational entry and exit rules, see [Agent workflows](AGENT_WORKFLOWS.md).
For local and CI validation, see [PR automation](PR_AUTOMATION.md) and
[Verification ladder](ci/verification-ladder.md).

---

## Core law

```text
many distinct claims may be in flight
one current candidate per coherent claim
one writer mutates a candidate branch/worktree at a time
many readers, researchers, reviewers, and tools may inspect it
Git and focused integration proof surface real interactions when they occur
```

Do not coordinate around hypothetical overlap. A lane works its selected claim.
It does not inspect sibling worktrees, reserve files or crates, maintain an
overlap map, or monitor nearby implementations.

Cross-lane attention is normally required only when:

- an equivalent PR already implements the same claim;
- an explicit prerequisite changes materially;
- Git reports a concrete conflict;
- combined-tree proof exposes an actual semantic interaction;
- one lane needs to communicate a material fact through an issue or PR comment.

A behind-only branch needs no action. The affected lane owns its conflict
resolution and focused re-proof when a real interaction appears.

---

## Goal loop

A high-level request is not automatically equivalent to one issue.

The accountable root preserves:

- the goal source;
- the current interpretation;
- constraints and non-goals;
- material assumptions and decisions;
- acceptance predicates;
- the distinct claims currently required to satisfy those predicates.

Each predicate is evaluated as:

```text
pass
failed
limited
not_applicable
not_established
```

“No more issues found” is not `pass`.

The goal loop is:

```text
preserve goal
→ reconstruct current truth
→ select or resume one distinct required claim
→ deliver one PR lane until GitHub or an external dependency owns the next event
→ retain that lane as in flight
→ advance another distinct required claim when useful
→ reconcile merged or deliberately closed work
→ re-evaluate the actual goal
```

One PR waiting on CI or external review does not normally block the goal.

---

## PR loop

The unit of merge is one coherent acceptance-and-rollback claim. PR size is
measured by semantic authority and review boundary, not line count.

```text
current premise
→ discriminating proof
→ implementation
→ test hardening
→ simplification
→ candidate challenge
→ commit one exact candidate head
→ pre-publication review-pr candidate pass
→ publication or PR resumption
→ published-head review-pr with remote evidence
→ review and CI repair
→ refreshed exact-head review for affected dimensions
→ integration proof
→ merge convergence
→ reconciliation
```

The pre-publication pass normally remains `REVIEW_INCOMPLETE` while hosted
checks, artifacts, and external review are absent. The exact published PR head
must receive `REVIEW_READY` before merge convergence.

These are judgment passes, not mandatory identities. The accountable root may
perform several passes directly. Focused provider-native subagents are useful
when they change the evidence, context, oracle, tools, failure perspective,
cost, platform reach, or elapsed time.

A different persona is not automatically independent. Independence is earned
through a different oracle, source, threat model, context, or verification
method where the risk warrants it.

---

## State-aware entry

Agents enter existing work at the earliest missing or stale judgment.

Examples:

- no issue: research and capture the claim, then continue;
- issue ready, no proof: design proof;
- proof ready, no implementation: build candidate;
- existing candidate: harden, simplify, or challenge it;
- committed candidate, no PR: run candidate review, publish with incomplete
  remote evidence, then review the published head;
- candidate appears ready: run substantive exact-head review;
- existing PR with comments: verify, repair or refute, refresh review as needed,
  and continue;
- PR waiting on remote state: yield the lane and advance another distinct claim;
- merged PR: reconcile the issue and parent goal.

Do not recreate completed ceremony merely because a new session arrived later.

---

## Engineering decisions

The existence of alternatives does not itself require escalation.

Default behavior:

```text
inspect current authority
→ research the material alternatives
→ choose the strongest reversible option
→ document the rationale
→ proceed
→ let proof and review correct the candidate
```

Use an owner decision only when materially different viable outcomes remain
after safe research and reversible engineering experiments are exhausted, or
when the choice changes external commitment, destructive action, exposure, or a
non-derivable product preference.

---

## Verify, do not trust

A detailed builder, subagent, automated review, or CI report is not semantic
evidence by itself.

- Verify current source and the retained production path.
- Run the compiler and relevant behavioral proof.
- Prove the effect, not merely that output was printed.
- Assert fixture construction and nonempty parsed subjects before downstream
  claims.
- Treat missing, skipped, unobserved, stale, or incomparable evidence explicitly.
- Distinguish source failure, test/oracle failure, instrument failure,
  infrastructure failure, and not-established state.
- Treat automated review findings as hypotheses: repair valid findings or reply
  with source-backed evidence.
- Quota, unavailable, skipped, failed, or stale review-provider output is
  missing review, not a clean review.
- A green aggregate check does not substitute for missing load-bearing evidence.
- No unresolved threads does not mean the current head received substantive
  review.
- A pass with zero intended subjects is not proof.

RIPR’s actionability rule remains load-bearing: a wrong actionable repair signal
is worse than several missed advisories. Keep actionability fail-closed and use
the shared owning validator.

---

## Proof design

Proof should be designed before or while implementation begins.

A useful proof package contains:

- the positive retained behavior;
- a discriminating negative or alternate case;
- production-path reachability;
- setup validation;
- currentness and artifact identity where material;
- a removal or deliberately wrong implementation experiment when valuable;
- explicit claim limits.

Goldens may encode an incorrect expectation. Where promotion or authority risk
is material, use an independent invariant or corpus in addition to the golden.

Platform-specific branches require platform-capable proof. Process success,
static movement, semantic correctness, mutation adequacy, and merge readiness
remain separate assurance axes.

---

## Candidate ownership

One current candidate normally owns one coherent claim.

Do not create competing implementations merely to consume parallel capacity.
Several workers may contribute genuinely disjoint pieces of one broad candidate
only through one integrating writer and one PR boundary.

The integrating writer:

- preserves the controlling issue and claim boundary;
- owns accepted mutations to the branch/worktree;
- reconciles contradictory research or review;
- keeps the candidate coherent;
- commits and publishes one current head.

Read-only subagents may map authority, challenge tests, research external
semantics, or review correctness/security/privacy/compatibility/platform
behavior. The root verifies and synthesizes their results.

---

## Substantive exact-head review

`review-pr` is the required judgment between candidate hardening and merge
convergence. It is not another permanent actor and does not require a separate
person. It is a distinct evidence pass.

An exact review subject is a committed Git object. Uncommitted worktree state
cannot receive an exact-head disposition.

A candidate may receive two passes:

1. **Pre-publication candidate pass:** source, proof, and repository contracts
   are reviewed on the exact committed branch head. Missing hosted checks,
   artifacts, and external review remain explicit, so the normal result is
   `REVIEW_INCOMPLETE`.
2. **Published merge pass:** the exact PR head is reviewed with current hosted
   jobs, artifacts, comments, and integration evidence. Only this pass may emit
   `REVIEW_READY` for merge convergence.

Bind every review to:

```text
reviewed_head_sha
integration_basis
claim_boundary
review timestamp
```

Read the complete current-head diff and PR body, then follow the change into the
semantic owner and actual consumers. Review applicable lanes:

1. authority, provenance, identity, currentness, producer authenticity, and
   compatibility authority;
2. correctness, failure paths, rollback, cleanup, partial publication,
   transaction boundaries, replay, stale input, TOCTOU, races, and concurrency;
3. fixture setup, intended nonempty subjects, production-path reachability, and
   whether the old or wrong behavior can still pass the test;
4. rendered CLI/API/LSP/help/output behavior rather than raw source-text
   coincidence;
5. runtime validation, schema, docs, generated artifacts, output contracts,
   compatibility history, and support-claim parity;
6. platform, package, process, trust, security, permission, secret, and network
   behavior where engaged;
7. exact-head required/advisory jobs, denominators, skipped lanes, failures,
   reports, receipts, and workflow artifacts.

Challenge each load-bearing claim with a counterexample, alternate case,
mutation/removal experiment, deliberately wrong implementation, or an explicit
reason the experiment is impractical.

A review record distinguishes:

- blocking source/contract defects;
- blocking test/oracle defects;
- blocking missing platform/integration proof;
- non-blocking suggestions;
- refuted or stale findings;
- missing proof or review;
- instrument failure;
- infrastructure failure;
- not established.

A clean review names changed surfaces, semantic owners, risks, invariants,
validation signals, residual assumptions, missing evidence, and exact head.
`LGTM` is not useful evidence.

On the author's own PR, GitHub cannot accept `REQUEST_CHANGES`. Use a `COMMENT`
review with an explicit blocking or review-ready disposition. The platform
constraint is not approval.

False-confidence boundaries:

- green CI does not establish semantic correctness;
- no review threads does not establish review;
- individually atomic writes do not prove a multi-file/directory transaction;
- a digest binds bytes but not their claimed producer or invocation;
- source-string checks do not prove rendered behavior;
- documentation cannot strengthen runtime/schema authority;
- `mergeStateStatus: BLOCKED` is not a causal diagnosis and does not prove a
  human approval requirement.

Only the published-head `REVIEW_READY` disposition proceeds to merge
convergence. `REPAIR_REQUIRED` returns to the same candidate and refreshes
affected review dimensions.

---

## Review and currentness

Keep three subjects distinct:

```text
PR head
  implementation and review subject

integration basis
  current main or queued predecessor set

squash / merge-group result
  combined-tree interaction subject
```

Review and proof currentness are dimensional:

- production implementation;
- test stimulus;
- test oracle;
- public claim;
- generated relationships;
- conflict resolution;
- integration basis;
- candidate head identity.

A test-only hardening push need not invalidate review of untouched production
code. A conflict resolution invalidates review of the conflict seam. Unrelated
movement on `main` invalidates nothing by itself.

Do not rebase merely because a branch is behind. Reconcile when there is an
actual conflict, changed explicit prerequisite, material combined-tree failure,
or an applicable repository requirement.

---

## GitHub-owned waits

Once a coherent candidate is published and GitHub owns the next event, return a
non-terminal result rather than polling unchanged state:

```text
PR_IN_FLIGHT
AUTO_MERGE_ARMED
WAITING_REQUIRED_CHECKS
WAITING_EXTERNAL_REVIEW
WAITING_INTEGRATION_PROOF
```

Resume only after a material transition: substantive review, required failure,
head change, conflict, changed prerequisite, merge, or closure.

Issues, PRs, reviews, checks, and committed artifacts are durable state. Model
threads, subagent lifetimes, and local planning notes are not repository
authority.

---

## Local validation

`cargo xtask precommit` is the authoritative local shift-left entry point.
Run focused proof before it, then run the changed-surface and fixed-candidate
gates required by the PR.

Do not install broad post-edit hooks that run workspace-wide Clippy or tests
while a candidate is intentionally incomplete. Provider hooks, when used, must
remain thin conveniences around canonical repository commands rather than
private policy engines.

One Cargo command at a time per candidate worktree is the default. Do not kill
unrelated Cargo processes. Report lock or capacity failures as infrastructure
state rather than source failure.

---

## Merge and reconciliation

A ready PR has:

- one coherent claim;
- an exact current published candidate;
- a current published-head `REVIEW_READY` disposition;
- all substantive findings repaired or evidence-refuted;
- current proof for affected seams;
- required checks green or an explicit non-ready disposition;
- no unresolved load-bearing conversation;
- accurate limitations and non-claims.

Squash merge the exact reviewed head when ready. After merge:

```text
verify current main
→ update delivered and remaining issue acceptance
→ update parent goal or campaign
→ refresh generated evidence where required
→ close only acceptance-complete issues
→ remove the completed worktree and stale branch
```

Deferred, partial, blocked, or superseded work remains visible with an accurate
disposition. It is not closed merely because it is outside the current release
or goal frontier.

---

## Durable resumption

Resume from current artifacts rather than chat history:

| Artifact | Purpose |
| --- | --- |
| GitHub issues and PRs | Live claim, candidate, review, and merge state |
| `docs/ROADMAP.md` | Product direction and checkpoints |
| `docs/IMPLEMENTATION_PLAN.md` | Current implementation direction |
| `docs/IMPLEMENTATION_CAMPAIGNS.md` | Historical and multi-PR context, not a global selector |
| `.allow/spec-system/slices/` | PR-sized scope and claim boundaries |
| `docs/specs/` + `.ripr/traceability.toml` | Spec → test → code relationships |
| `docs/LEARNINGS.md` | Durable failure modes and hidden invariants |
| `AGENTS.md` / `.agents/skills/**` | Codex operating set |
| `CLAUDE.md` / `.claude/skills/**` | Claude operating set |

Update durable artifacts when the knowledge should survive a session. Do not
create a repository-global active-goal, current-writer, liveness, or stage file.
