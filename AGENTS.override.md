# Codex Repository Instructions

Read `AGENTS.md` at the repository root before modifying the repository. Its
product contract, language rules, architecture, Rust baseline, file policy,
gates, implementation bias, evidence-honesty rules, status and finding
contracts, PR doctrine, merge safety, release boundaries, and cleanup rules
remain authoritative.

This file replaces only the retired **Orchestration Operating Model**, fixed
subagent roles, wave discipline, global edit-cage/resource-reservation model,
and long-context work-selection instructions in `AGENTS.md`.

Codex procedures live under `.agents/skills/**`. Use the narrowest applicable
skill:

```text
high-level outcome  → deliver-goal
selected claim/issue or existing PR → deliver-pr
missing or stale issue premise → prepare-issue
missing or weak oracle → prepare-proof
implementation/hardening → build-candidate
published or existing PR → finish-pr
```

Do not route Codex through `.claude/skills/**`. Claude has its own complete file
set under `CLAUDE.md` and `.claude/skills/**`.

## Operating law

```text
many distinct claims may be in flight
one current candidate per coherent claim
one writer mutates each candidate branch/worktree at a time
readers, researchers, reviewers, and tools may inspect it
Git and focused integration proof surface real interactions when they occur
```

Do not inspect sibling worktrees, reserve files/crates/semantic surfaces,
maintain overlap maps, or monitor sibling implementations. Check other work
only for:

- an equivalent PR implementing the same claim;
- an explicit prerequisite;
- a concrete Git conflict;
- a failed combined-tree proof;
- a material fact communicated through an issue or PR comment.

A behind-only branch needs no update. The affected lane owns its conflict repair
and affected re-proof after a real interaction appears.

## Goal delivery

Preserve the user's original goal, current interpretation, constraints,
non-goals, assumptions, and acceptance predicates. Do not substitute the first
plausible issue for the actual goal.

Evaluate each goal predicate as:

```text
pass
failed
limited
not_applicable
not_established
```

“No more issues found” is not `pass`.

When one PR reaches a remote-owned state such as required CI, external review,
auto-merge, or merge queue, leave it in flight and advance another distinct
required claim when useful. Resume only after a material transition.

## Judgment passes, not permanent actors

Research, adversarial challenge, proof design, implementation, test hardening,
simplification, review, repair, integration proof, and reconciliation are
meaningful passes. They are not mandatory identities.

The accountable root may perform several passes directly. Use built-in or
project-scoped subagents only when they materially improve evidence, context,
tools, failure perspective, cost, or elapsed time. Subagents are normally
read-only; their reports are leads until verified against current artifacts.

Do not create Scout, Adversary, Builder, Verifier, Reviewer, or Cleanup Auditor
as required lifecycle roles. Do not require author/reviewer/integrator to be
different accounts. Independence comes from a different oracle, source,
context, threat model, or verification method where risk warrants it.

## Candidate boundary

One coherent claim normally has one branch, worktree, candidate, and PR. Do not
create rival implementations merely to manufacture parallelism. Disjoint
implementation contributions may be delegated only through one integrating
candidate owner.

Do not use repository-global active-goal, current-writer, current-stage,
liveness, lease, lock, or candidate-frontier files. GitHub issues, PRs, reviews,
checks, and committed evidence are durable state. Model threads and subagent
lifetimes are not.

## Engineering decisions

The existence of alternatives does not require a stop. Research the governing
sources, choose the strongest reversible option, document the rationale, and
proceed. Request an owner decision only when materially different viable
outcomes remain after safe research and reversible engineering experiments, or
when external commitment, destructive action, exposure, or a non-derivable
product preference changes.

## Review and integration currentness

Keep separate:

```text
PR head             implementation and review subject
integration basis   current base or queued predecessors
squash result       combined-tree interaction subject
```

Refresh only the proof and review dimensions affected by a change:
production implementation, test stimulus, oracle, public claim, generated
relationships, conflict resolution, or integration basis. Unrelated movement
on `main` invalidates nothing by itself.

When GitHub owns the next transition, yield one of:

```text
PR_IN_FLIGHT
AUTO_MERGE_ARMED
WAITING_REQUIRED_CHECKS
WAITING_EXTERNAL_REVIEW
WAITING_INTEGRATION_PROOF
```

Do not poll unchanged remote state.

## Local validation

Use focused proof during implementation. Before publication, use:

```bash
cargo xtask precommit
```

`precommit` is the authoritative local shift-left entry point. Do not run broad
workspace Clippy or tests after every edit. Provider hooks, if present, remain
thin conveniences around canonical repository commands and do not own policy.

For Rust, on-diff Clippy means compiling the complete impacted package and
relevant targets, not scanning changed lines without crate context. Run one
Cargo command at a time per candidate worktree. Do not kill unrelated Cargo
processes. Report lock/capacity failure as infrastructure state.

## Merge and reconciliation

Use `finish-pr` for review, CI, integration, and merge. Repair valid findings in
the same candidate; refute incorrect findings with source-backed evidence;
resolve threads only after repair or reply.

After merge:

```text
verify current main
→ update delivered and remaining issue acceptance
→ update parent goal/campaign
→ refresh generated evidence where required
→ close only acceptance-complete issues
→ remove completed worktree and stale branch
```

Deferred, partial, blocked, or superseded work remains visible with an accurate
disposition.

## Explicit exclusions

- no Kiro skill, overlay, lifecycle route, handoff, or provider entry;
- no executor DAG or permanent role roster;
- no candidate tournament for one claim;
- no sibling-lane telemetry or reservation system;
- no mandatory approval pause for ordinary engineering decisions;
- no lifecycle state encoded in issue comments or labels;
- no release, publication, tag, signing, marketplace, or secret operation
  without the existing explicit repository authority.
