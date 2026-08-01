# Agent Workflows

This repository supports high-level goal delivery through two complete provider
sets:

```text
Codex:  AGENTS.md + .agents/skills/**
Claude: CLAUDE.md + .claude/skills/**
```

Select the provider's own skill file. Do not route one provider through the
other provider's files.

The normal outer flow is:

```text
high-level goal
→ deliver-goal
→ one distinct PR claim
→ deliver-pr
→ prepare-issue / prepare-proof / build-candidate / finish-pr as needed
→ merge or durable in-flight state
→ reconcile
→ continue until the actual goal is satisfied
```

---

## Starting from a high-level goal

Use `deliver-goal` when the user states an outcome rather than one scoped issue.

1. Preserve the goal verbatim.
2. State the current interpretation, constraints, non-goals, assumptions, and
   acceptance predicates.
3. Read current `main`, GitHub issues and PRs, required checks, and the governing
   product, architecture, policy, spec, and production paths.
4. Identify the distinct claims required to satisfy the predicates.
5. Resume an equivalent existing PR or issue before creating new work.
6. Advance one coherent claim with `deliver-pr`.
7. When a PR reaches CI, external review, auto-merge, or merge queue, leave it
   in flight and advance another distinct required claim when useful.
8. Reconcile after every merge or deliberate closure.
9. Stop only when predicates pass, a real external blocker covers all remaining
   work, a material owner decision remains, or the result is not established.

Do not substitute “finish this issue” for the larger requested end state unless
that issue's acceptance actually equals the goal.

---

## Starting from an issue

Use `deliver-pr`.

1. Read the issue and every linked governing artifact.
2. Verify the premise against current source and the real consumer.
3. Search for an equivalent existing PR.
4. Enter at the earliest missing or stale judgment:
   - issue/premise;
   - proof;
   - implementation;
   - test hardening;
   - simplification;
   - candidate review;
   - review repair;
   - integration proof;
   - reconciliation.
5. Continue through publication, review, CI, merge, and reconciliation unless a
   real stop condition exists.

Filing or correcting the issue is not a reason to stop when implementation was
the requested job.

---

## Starting from an existing PR

Use `deliver-pr` or `finish-pr` depending on candidate maturity.

- Read the complete current-head diff and PR body.
- Read every current review thread and required check.
- Verify findings against source and behavior.
- Repair valid findings through the same candidate.
- Refute invalid findings with evidence.
- Resolve only after repair or reply.
- Refresh only affected proof and review dimensions.
- Leave a behind-only branch alone.
- Reconcile a real conflict or failed integration proof when it occurs.
- Yield when GitHub owns the next event instead of polling unchanged state.

A PR waiting on CI is still useful in-flight work. It does not normally block a
larger goal.

---

## The six public skills

### `deliver-goal`

Owns the original goal, acceptance predicates, distinct required claims,
in-flight PRs, and final satisfaction judgment.

### `deliver-pr`

Owns one coherent claim from current premise to merge or deliberate closure.

### `prepare-issue`

Researches or corrects the issue, semantic owner, acceptance, dependencies, and
non-goals, then returns to delivery.

### `prepare-proof`

Designs positive, negative, production-path, currentness, and claim-boundary
proof before or during implementation.

### `build-candidate`

Implements, tests, simplifies, challenges, and repairs one current candidate.

### `finish-pr`

Publishes or resumes the PR, repairs review and CI findings, yields remote waits,
merges the exact ready candidate, and reconciles repository state.

Each provider's file contains the complete procedure and valid outcomes.

---

## Claim and candidate rules

```text
many distinct claims may be in flight
one claim normally has one current candidate
one writer mutates that candidate branch/worktree at a time
```

Agents do not inspect sibling worktrees, reserve files or crates, maintain
an overlap ledger, or watch sibling implementations.

Before creating work, check only for:

- an equivalent PR for the same claim;
- an explicit prerequisite;
- a superseding implementation.

During integration, react only to:

- a concrete Git conflict;
- a changed explicit prerequisite;
- a failed combined-tree proof;
- a repository rule that genuinely applies to this candidate.

The later lane owns its own conflict repair and affected re-proof.

---

## Subagents

The accountable root may execute directly or use focused provider-native
subagents.

Useful subagent questions include:

- Where is the semantic owner and production consumer?
- What current issue or PR already owns this claim?
- What is the strongest counterexample to the proposed proof?
- Does the test reach the real production branch?
- What security, privacy, compatibility, or product boundary is at risk?

Subagents are normally read-only. They return evidence-backed findings, not
lifecycle authority. The root verifies their citations, resolves contradictions,
and integrates one candidate.

Do not create one permanent actor for every judgment pass. Research, adversarial
challenge, implementation, test hardening, simplification, and formal review are
passes; the same accountable root may perform several of them.

---

## Decisions and escalation

Make the strongest source-backed reversible decision and proceed.

Escalate only when:

- materially different viable outcomes remain after research and safe
  experiments;
- the choice changes external commitment, destructive action, exposure, or a
  non-derivable product preference;
- credentials or permissions are genuinely unavailable;
- the selected claim exceeds its accepted authority boundary.

Do not stop merely because design judgment exists.

---

## Local proof and precommit

Run focused proof while developing. Before publication, run:

```bash
cargo xtask precommit
```

`precommit` is the authoritative local shift-left entry point. It must preserve
repository policy checks and select Rust linting from the actual local change
set. Full workspace and release qualification remain separate fixed-candidate
steps.

Do not use broad post-edit hooks that run workspace-wide Clippy or tests while
the code is intentionally incomplete. Hooks may invoke canonical repository
commands at explicit lifecycle points; they do not own policy.

For changed Rust, “on-diff Clippy” means compiling the complete impacted package
and relevant targets, not parsing changed lines without crate context.

Run one Cargo command at a time per candidate worktree. Lock contention or a
runner failure is infrastructure state, not source failure.

---

## PR currentness and merge

Keep separate:

- candidate head;
- integration basis;
- squash or merge-group result.

Do not rebase because `main` moved. Reconcile only real conflicts or interactions.

Useful remote-owned outcomes include:

```text
PR_IN_FLIGHT
AUTO_MERGE_ARMED
WAITING_REQUIRED_CHECKS
WAITING_EXTERNAL_REVIEW
WAITING_INTEGRATION_PROOF
```

A substantive review finding, required failure, head change, conflict, changed
prerequisite, merge, or closure is a material transition. Unchanged remote state
is not.

After merge:

1. verify current `main`;
2. update issue acceptance as delivered, partial, blocked, or residual;
3. update parent goals or campaigns;
4. refresh generated evidence where required;
5. close only acceptance-complete issues;
6. remove the worktree and stale branch.

---

## Durable sources

Resume from artifacts, not model memory:

| Artifact | Use |
| --- | --- |
| GitHub issue | Claim, acceptance, owner, dependencies, residual work |
| GitHub PR | Current candidate, review, CI, integration state |
| `docs/ROADMAP.md` | Product direction |
| `docs/IMPLEMENTATION_PLAN.md` | Current implementation direction |
| `docs/IMPLEMENTATION_CAMPAIGNS.md` | Historical and multi-PR context, not a global active queue |
| `.allow/spec-system/slices/` | PR-sized claim boundaries |
| `docs/specs/` and `.ripr/traceability.toml` | Spec-test-code relationships |
| `docs/LEARNINGS.md` | Durable failure modes and hidden invariants |
| Provider skill roots | Executable procedure for the current provider |

No `.ripr/goals/active.toml`, current-writer file, stage file, or agent-liveness
record selects ordinary work.

---

## Honest stopping conditions

Valid terminal or yielding conditions are:

- goal satisfied;
- PR merged;
- PR durably in flight while GitHub owns the next event;
- real external blocker;
- material owner decision;
- deliberate closure or supersession with residual work preserved;
- not established.

Invalid stopping conditions include:

- an issue was filed;
- one plausible PR was opened;
- CI is merely still running;
- another branch is behind;
- a different design was conceivable;
- no more issues were found;
- an automated reviewer was unavailable.
