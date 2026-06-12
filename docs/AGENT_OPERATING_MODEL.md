# Agent Operating Model

This document captures the orchestration operating model proven across `ripr`'s
campaign-driven development. It is durable repo guidance for any agent — Codex,
Claude Code, Cursor, or a generic runner — working in this repository.

For the mechanics of starting work (roadmap, campaign manifests, scoped-PR
evidence bar), see [Agent workflows](AGENT_WORKFLOWS.md). For the PR shape/check
loop, see [PR automation](PR_AUTOMATION.md). For the campaign model, see
[Codex Goals](CODEX_GOALS.md) and [Scoped PR contract](SCOPED_PR_CONTRACT.md).

---

## The Unit of Work

The unit of work is one scoped PR: one production behavior plus its complete
evidence package.

```text
spec -> test or fixture -> code -> output contract -> metric -> traceability
```

The pipeline per PR:

```text
scout (read-only inventory)
-> adversarial spec / issue
-> builder (implements)
-> verify-myself
-> green PR
-> manual merge when CI green
```

Auto-merge is disabled. Merge the PR after CI turns green; do not wait for an
approval gate unless `stackable = false` or a `blocked_by` dependency applies.

Large fixture, golden, spec, docs, ADR, and traceability diffs are welcome when
they make one production behavior reviewable. A small code diff is not
acceptable if it changes multiple contracts without a spec-test-code trail.

---

## Agent Economics: Right Task, Right Agent, Right Cost

Route work by cost. The dominant failure mode is burning expensive capacity on
tasks a cheaper agent could have resolved first.

### Cheap read-only scouts (Haiku-class, Explore-style)

Use for:

- repo / changed-surface inventory
- issue, PR, and CI-log summaries
- schema and spec surface mapping
- claim checks and assumption verification
- cleanup audits

Scouts return structured tables — item, files touched, claim made, evidence
found, missing proof, risk, next action — not prose. Run scouts **before**
expensive builds to find the cleanest reuse path and catch wrong assumptions
cheaply (shift-left).

### Cheap adversarial second pass

Run a second cheap agent that assumes the first report is wrong, before:

- CI-routing changes
- release claims or changelog assertions
- LSP / agent-packet contract changes
- PR close or supersession decisions

The second pass returns only concrete discrepancies with file or PR references.

### Implementation-grade agents (Sonnet-class)

Use for:

- code changes, test and fixture updates
- conflict resolution
- schema / doc / report alignment
- packet design
- release judgment
- turning scout inventories into coherent PRs with a bounded plan and small
  working set

### Broad parallel workflows (Ultracode-style fanouts)

Use only for queue-scale uncertainty:

- open-PR reconciliation audits
- CI-failure taxonomies
- spec-surface inventories
- cross-repo contract reviews

Do not use for narrow edits. A broad fanout on a one-file change is an
orchestration failure.

### Top-tier judgment (Opus-class)

Escalate only for:

- high-risk release, security, or architecture decisions
- after two failed correction cycles

Using top-tier capacity to discover which files changed is an orchestration
failure.

---

## Verify-Don't-Trust

A confident, detailed builder report is not evidence. "Plausible-but-wrong" is
the dominant failure mode.

Rules:

- `cargo check` / `cargo build` is ground truth. IDE diagnostics are stale
  mid-edit snapshots — ignore them and run the compiler.
- Verify the claim with a behavioral repro of the effect, not just that output
  printed (a weak oracle on your own work).
- Mirror the full CI gate set locally before pushing:
  `cargo test -p xtask policy_checker_facade_runs_current_repo_checks`
  (the policy-checker facade). Cherry-picking individual gates misses some —
  for example, `check-generated` and `check-static-language`.
- Watch your own measurement: a `cmd | head; echo $?` pipe reports the pipe's
  exit code, not the command's.

The policy-checker facade test is the local proxy for the full CI lane. Run it
and inspect failures before pushing.

---

## File-Issue-First + Scoped-PR Contract

File or update the issue before building. Filing is cheap to verify before it
is expensive to build.

PR scope rules:

- One production behavior per PR.
- Do not bundle schema changes with analyzer rewrites.
- Do not mix LSP/UI changes with classifier changes.
- Do not mix cleanup with behavior changes.
- Large fixture/golden/spec/docs diffs are welcome when they make one behavior
  reviewable.

Make the production delta, evidence delta, acceptance criterion, and non-goals
explicit in PRs and planning docs. The [Scoped PR contract](SCOPED_PR_CONTRACT.md)
defines the required fields.

---

## Why Constraints Equal Autonomy

The conservative-language gate, scoped-PR contract, traceability check, and the
facade test are machine-checkable doctrine. That is what lets PRs ship without a
human reading each line.

```text
more well-designed constraints = more delegable autonomy
```

The gates are the trust substrate, not overhead. Bypassing or weakening them
reduces autonomy — it does not accelerate it.

Specifically:

- `check-static-language` enforces exposure vocabulary and prevents runtime
  mutation-testing claims from leaking into static output.
- The scoped-PR contract prevents scope creep that defeats reviewability.
- `check-traceability` keeps spec → test → code chains intact.
- The facade test mirrors the full CI lane locally, so a push doesn't surprise CI.

---

## CI and Cleanup Hygiene

### CI watching

Use the cost-aware adaptive poller (the `ci-watch` skill), not `gh run watch`.
`gh run watch` polls every 3 seconds and consumes the authenticated rate limit
across long runs.

### Merge serialization

Merges serialize under the up-to-date-branch rule. When two PRs are ready,
merge the higher-cost lane's PR first; the lower-cost lane rebases after.

### Cleanup after every pass

Every session should end with:

- worktrees removed (`cargo xtask worktree doctor` reports stale ones)
- stale branches deleted (branches whose remote is gone)
- `target/ripr/` cache growth trimmed (ad-hoc large JSON outputs removed)
- temp files and generated artifacts cleared
- `.claude/worktrees/` nested repos are gitignored; `xtask should_skip_path`
  skips them in policy checks

---

## Long-Context Resumption

The repo is organized so agents resume from artifacts, not chat history. When
picking up unfamiliar work:

| Artifact | Purpose |
| --- | --- |
| `docs/ROADMAP.md` | Direction and checkpoints |
| `docs/IMPLEMENTATION_PLAN.md` | Next scoped PR |
| `docs/IMPLEMENTATION_CAMPAIGNS.md` | Active multi-PR campaigns |
| `.ripr/goals/active.toml` | Machine-readable campaign state |
| `docs/CAPABILITY_MATRIX.md` | Current capability status per area |
| `docs/PR_AUTOMATION.md` | Shape / check / guide model |
| `docs/specs/` + `.ripr/traceability.toml` | Spec → test → code map |
| `docs/LEARNINGS.md` | Durable knowledge; update when something new is learned |
| `AGENTS.md` | Terse rules of engagement (read once at session start) |

Update `docs/LEARNINGS.md` when you discover something that should survive
the session: a failure mode, a performance constraint, a hidden invariant, or
a clarification that would have saved time.

Choose the smallest vertical slice with one production delta and one evidence
package. Do not infer ready work from chat history when the campaign manifest
shows only blocked work.
