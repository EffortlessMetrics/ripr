# Agent Workflows

This repository supports high-level goal delivery through complete provider
sets:

```text
AGENTS consumers: AGENTS.md + .agents/skills/**
Claude:           CLAUDE.md + .claude/skills/**
```

Select the provider's own skill file. Do not route one provider through the
other provider's files. A runtime that selects AGENTS.md and ignores CLAUDE.md
uses the AGENTS route; absence of native skill import does not prevent reading
the applicable procedure directly.

The normal outer flow is:

```text
high-level goal
→ deliver-goal
→ one distinct PR claim
→ deliver-pr
→ prepare-issue / prepare-proof / build-candidate as needed
→ commit one exact candidate head
→ review-pr candidate pass
→ finish-pr publication or PR resumption
→ review-pr published-head pass with remote evidence
→ finish-pr merge convergence and reconciliation
→ merge or durable in-flight state
→ continue until the actual goal is satisfied
```

The pre-publication pass normally exits `REVIEW_INCOMPLETE` when hosted checks,
artifacts, or external review do not yet exist. Only the exact published PR head
may receive `REVIEW_READY` for merge convergence.

## Starting from a high-level goal

Use `deliver-goal` when the user states an outcome rather than one scoped issue.

1. Preserve the user's parent end state, constraints, non-goals, assumptions and
   accepted scope. A session, runtime progress bar or current PR batch does not
   establish a smaller goal.
2. Rehydrate from current user and repository instructions, current source,
   issues/PRs and retained evidence. Summaries and subagent reports are leads,
   not permission or completion authority. Instructions define the task;
   artifacts establish observed results.
3. Identify the distinct claims required by the parent acceptance predicates.
   Resume an equivalent existing issue or PR before creating new work.
4. Advance a ready coherent claim with `deliver-pr`; use independent candidate
   worktrees and implementation agents when useful and available.
5. When a PR reaches CI, review, auto-merge or merge queue, retain it in flight
   and advance another ready claim. A build blocks its own transition, not the
   whole goal.
6. Reconcile after merge or deliberate closure, then select the next ready
   claim. A reconciliation document accompanies delivery; it does not replace it.
7. Evaluate the actual parent, including missing or stale evidence. Unknown
   evidence calls for investigation while useful authorized work remains.
8. Stop only when the parent is satisfied, the user stops the work, a material
   non-derivable owner decision remains, or no authorized executable work remains
   because of named capability, prerequisite or authorization boundaries.
   All-useful-work-waiting is `GOAL_IN_FLIGHT`, not completion.

Runtime goal text contains durable end conditions, not volatile PR numbers or
worker names. Replace stale objective text with a successor preserving the
parent; do not ask the user to dictate a recoverable goal again.

When progress counts are useful, count nonoverlapping accepted parent predicates
against the complete selected denominator. Do not count both umbrellas and
children, use PR count as release progress, invent percentages, or infer time to
cut from a percentage. Scope changes need their governing ruling or evidence.

A local commit is useful unpublished evidence, an open PR is in flight, and a
merged PR is landed implementation. Candidate selection, qualification, source
integration, ship authorization, publication and public verification are later
separate judgments. A completed subgoal never closes its parent automatically.
An explicitly requested read-only analysis may finish with its requested report;
do not impose a PR requirement on that different task.

## Starting from an issue

Use `deliver-pr`.

1. Read the issue and linked governing artifacts.
2. Verify the premise against current source and the real consumer.
3. Search all-state and recently merged PRs for equivalent work.
4. Enter at the earliest missing or stale judgment: premise, proof,
   implementation, hardening, simplification, challenge, substantive review,
   review repair, integration proof or reconciliation.
5. Commit the coherent candidate before exact-head review.
6. Continue through candidate review, publication, published-head review,
   review/CI repair, merge and reconciliation unless a genuine boundary remains.

Filing an issue is not completion when implementation was requested. Do not
recreate completed stages because a new session arrived later. Before resolving
a conflict or opening another PR, check whether upstream already delivered or
superseded the claim. Preserve only the genuine unique residual.

## Starting from an existing PR

Use `deliver-pr`, `review-pr` or `finish-pr` according to candidate maturity.

Read the complete current-head diff, PR body, governing issue, review threads
and required checks. Thread/check inspection is remote triage, not substantive
review. Follow changed behavior into its semantic owner and consumers; challenge
the oracle, failure paths, contracts, platforms and artifact identity.

Repair valid findings on the same candidate; refute incorrect findings with
source-backed evidence. Confirm a repair or reply exists before resolving a
thread, then read back its state. Inspect all pages. A bot's automatic addressed
label is not evidence of repair.

Leave a behind-only branch alone. Reconcile actual conflicts, changed explicit
prerequisites, failed combined-tree proof or a genuinely applicable exact-base
rule. Ordinary swarm PRs use protected squash merge. The controlled
history-preserving source-integration transaction follows its own authority,
not this ordinary merge method.

A PR waiting on CI remains useful in-flight work. Yield to another ready claim
rather than polling unchanged remote state. Revisit after a material event.

## The seven public skills

| Skill | Owns |
|---|---|
| `deliver-goal` | Parent end state, acceptance, ready claims, in-flight work and final satisfaction judgment |
| `deliver-pr` | One coherent acceptance-and-rollback claim from premise to merge/closure and reconciliation |
| `prepare-issue` | Current premise, semantic owner, scope, dependencies, negative acceptance and non-goals |
| `prepare-proof` | Positive and discriminating negative proof, production-path reachability, identity and claim limits |
| `build-candidate` | Implementation, tests, simplification, challenge and repair on one committed candidate |
| `review-pr` | Substantive exact-head inspection with current source, oracle, contract, platform and hosted evidence |
| `finish-pr` | Publication, remote repair, earned merge convergence, acceptance reconciliation and cleanup |

Each provider's skill file contains the full procedure. This page is routing and
shared contract context, not a second implementation of every procedure.

## Exact-head review contract

An exact review subject is a committed Git object. An uncommitted worktree cannot
receive an exact-head disposition. Bind:

```text
reviewed_head_sha
integration_basis
claim_boundary
changed_surfaces_and_semantic_owners_inspected
blocking_findings
non_blocking_suggestions
refuted_or_stale_findings
proof_and_ci_observed
proof_or_review_missing
affected_currentness_dimensions
residual_assumptions_and_non_claims
disposition
```

Review the applicable evidence dimensions:

1. semantic ownership, authority, provenance and identity;
2. failure paths, rollback, cleanup, atomicity, replay, races and concurrency;
3. fixture construction, nonempty intended subjects, production reachability and
   whether old or wrong behavior can still pass;
4. rendered CLI/API/LSP/help/output behavior rather than source coincidence;
5. runtime/schema/docs/generated/output/support parity;
6. platform, packaging, process, trust, security and permissions;
7. exact-head required/advisory jobs, selected/executed subjects, skips, failures,
   reports and artifacts.

Challenge load-bearing claims with a counterexample, alternate case, removal or
wrong-implementation experiment, or an explicit reason it is impractical. A
clean review records what was inspected and what remains unverified. Naked LGTM,
green CI and an empty thread list are not semantic review.

Valid dispositions are `REVIEW_READY`, `REPAIR_REQUIRED`, `REVIEW_INCOMPLETE`,
`INSTRUMENT_FAILURE`, `INFRASTRUCTURE_FAILURE`, `EXTERNAL_BLOCKER` and
`NOT_ESTABLISHED`. Missing hosted evidence prevents merge readiness, not
publication to obtain that evidence.
On the author's own PR, use a `COMMENT` review with explicit disposition; the
platform's author-review constraint is not approval.

Keep these false-confidence boundaries:

- unavailable/quota-limited/skipped/stale reviewers are missing review for that
  provider; an adequate permitted alternative can supply the judgment;
- zero intended subjects is not proof;
- atomic individual writes are not automatically an atomic transaction;
- hashes bind bytes, not their claimed producer or invocation;
- docs and PR prose cannot strengthen runtime/schema authority;
- `mergeStateStatus: BLOCKED` is not a diagnosis of a human approval requirement;
- a structural instruction checker does not prove semantic consistency, actual
  provider loading or model compliance.

## Agents, decisions and candidate ownership

Many distinct claims may be in flight. One coherent claim normally has one
current candidate and one writer. Delegate independent implementation to
separate candidate worktrees; readers/reviewers may inspect without moving or
editing a writer's checkout. Give the delegate the claim, inputs, exact question,
non-goals, write boundary, proof and handback. The root verifies load-bearing
citations and owns integration.

Do not create rival implementations, permanent role rosters, reservations,
overlap maps, sibling monitoring or repository-global orchestration state.
A different persona is not automatically independent; a different oracle,
source, tool, platform or failure perspective can be.

Choose the strongest reversible in-scope option after research. Routine
commits, ordinary pushes, PR creation/updates, review and CI repairs, protected
squash merge and lane cleanup are part of an authorized delivery goal. Do not
ask again merely because main moved or multiple implementations are possible.

Escalate only a non-derivable material choice, expanded scope, changed exposure,
destructive action or genuinely unavailable required capability/authorization.
Settings/rulesets/secrets, shared-history rewrites, durable-evidence deletion and
public release actions need their applicable explicit authorization. A future
publication boundary does not block reversible preparation.

## Local proof and precommit

Read `docs/agent-context/validation.md` for actual-shell detection, native exit
status, background-task ownership and environment-specific proof. The host OS
alone does not determine shell grammar. Stderr noise or a success-looking line
alone is not a terminal result.

Use focused proof during implementation. Before publication, use:

```bash
cargo xtask precommit
```

`precommit` is the authoritative local shift-left entry point. It preserves
policy checks and selects Rust linting from the actual local change set. For
changed Rust, on-diff Clippy compiles the impacted package/targets, not isolated
changed lines.

`check-fast` is a cheaper diff-aware route, not an alias for `precommit` or full
qualification. Independently verify its selector/base and ran/skipped report;
a failed or empty selector does not prove that nothing needed testing.

Full-workspace and release qualification are separate fixed-candidate steps.
Do not front-load every hosted gate onto a local machine merely to publish a
coherent candidate. When local execution is unavailable, retain that gap and use
the available hosted route without inventing a local pass. Preserve every
required merge check and substantive review.

Serialize Cargo operations sharing a candidate worktree, target lock or memory
bottleneck; do not kill unrelated processes. Bind owned background commands to
their original task/driver and logs, not process-name guesses. Hooks are thin
conveniences around canonical commands, not independent policy authorities.

## Currentness, merge and reconciliation

Keep the candidate head, integration basis and squash/merge-group result
separate. Refresh only affected implementation, stimulus, oracle, public-claim,
generated, conflict, integration and head-identity dimensions. Unrelated main
movement is not automatic invalidation.

`finish-pr` publishes coherent `REVIEW_INCOMPLETE` candidates to obtain hosted
evidence. It arms merge only with current published-head `REVIEW_READY`, actual
required proof and addressed material findings. Diagnose branch protection and
active rulesets read-only; never weaken them or use admin bypass to clear a PR.

After merge/closure, verify the actual repository object, reconcile delivered
and remaining issue acceptance, update the parent, preserve residual work,
refresh required generated evidence and remove only lane-created residue.
Closing a child does not close its parent.

## Durable sources

| Artifact | Use |
|---|---|
| GitHub issues | Current claim, acceptance, dependencies, decisions and residual work |
| GitHub PRs, reviews and checks | Published candidate, review, proof and integration state |
| `docs/ROADMAP.md` | Product direction |
| `docs/IMPLEMENTATION_PLAN.md` | Current implementation direction |
| `docs/IMPLEMENTATION_CAMPAIGNS.md` | Historical/multi-PR context, not a global queue |
| `.allow/spec-system/slices/` | PR-local claim boundaries |
| `docs/specs/` and `.ripr/traceability.toml` | Spec-test-code relationships |
| `docs/LEARNINGS.md` | Durable failure modes and invariants |
| Provider roots and skills | Operating instructions and procedure |

No `.ripr/goals/active.toml`, current-writer file, stage file or agent-liveness
record selects ordinary work. Keep status changes evidence-bound and update an
existing owned reconciliation comment rather than repeatedly appending copies.

## Honest stopping conditions

A lane can yield as `PR_IN_FLIGHT`, `WAITING_REQUIRED_CHECKS`,
`WAITING_EXTERNAL_REVIEW` or `WAITING_INTEGRATION_PROOF`; that is not parent
completion. A merged PR or deliberate closure is terminal for that claim only.

The parent stops when it is satisfied, the user stops it, a non-derivable material
decision remains, or no authorized executable work remains behind named
capability/prerequisite/authorization boundaries. Retain the unmet predicates
and exact next transition. `NOT_ESTABLISHED` describes missing evidence, not
permission to stop available investigation.

An issue filed, PR opened, busy compiler, behind-only branch, conceivable
alternative design, unavailable review bot, exhausted local checklist or lack
of newly found issues is not sufficient to complete the parent goal.
