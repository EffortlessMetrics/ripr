---
name: Design plan
about: Pre-implementation design plan for a scoped work item. Closed by a PR via `Closes #N`.
title: '<work-item-id>: <one-line goal>'
labels: ['design-plan', 'llm-context']
assignees: []
---

## Source of truth

- Active campaign:
- Work item id (matches `.ripr/goals/active.toml`):
- Branch (will be cut from `main`):
- Latest merged PR on `main` at planning time:
- Follow-up / next-blocked work item:

## Step 0 — premise check

Before editing any code, the executor MUST verify the facts above and the
assumptions below. If any check fails, **stop and report**, do not silently
adapt — a failed premise means the plan is rendered against a stale tree and
will diverge from current `main`.

- [ ] Local `main` is up to date (`git fetch origin && git status` shows no
      lag versus `origin/main`).
- [ ] The work item id appears in `.ripr/goals/active.toml` with
      `status = "ready"` (or `status = "active"`) and is not blocked.
- [ ] The "latest merged PR" listed above matches `git log -1 origin/main
      --oneline` at the time of cut.
- [ ] Any source files / line numbers cited below still exist and contain
      the cited code (paths drift when files are renamed or split).
- [ ] Any external schemas referenced (badge JSON, output JSON, capability
      manifest, etc.) match the current crate output — paste the actual JSON
      in this issue if the planner only paraphrased it.
- [ ] The branch name is unused locally and on the remote (`git branch -a`).

When in doubt, re-derive the fact from the repo rather than trusting the plan.

## Goal

What is the one production behavior, public contract, or architectural seam
this PR changes? One paragraph.

## Production delta

The minimum, exact change in user-visible behavior. Everything else (tests,
docs, fixtures, metadata flips) is evidence/support, not production delta.

## Evidence/support delta

Tests, fixtures, docs, ADRs, learnings, status flips, capability metadata,
golden outputs that ship with the production delta to make it reviewable.

## Plan

### Inputs

What existing artifacts feed the change (prior PRs, current schemas,
existing helpers, existing fixture diffs)?

### Outputs

What new files / modified surfaces does the change produce?

### Required behavior

Step-by-step description of the runtime / build / CI behavior the change
introduces. Include exact CLI flags, exact file paths, exact log lines if
they're load-bearing for review.

## Acceptance criteria

A small, testable list of conditions that, when all true, make the PR
mergeable. Prefer "this exact xtask command exits 0" over prose.

- [ ]
- [ ]
- [ ]

## Non-goals

Explicit list of things this PR does **not** do. Important: a non-goal here
is a directive to the executor. If implementation pressure pushes toward a
non-goal, stop and split the PR.

## Required tests

Tests that must exist after this PR merges, with the exact test name and
location (`<crate>/<file>::tests::<name>`). For each, note the failure mode
the test pins — so a future reader knows what regression each test catches.

## Verification commands

The exact shell commands to run locally to gate the PR. Prefer `cargo xtask
check-pr` plus any task-specific commands; avoid prose like "run the usual
gates."

```bash

```

## Claude execution directive

Imperative-mode instructions for the executor:

- Implementation surface (which files / functions get touched).
- Implementation rules (what helpers to use, what to avoid).
- Subagent decomposition if appropriate (inline haiku, manual worktree, or
  main thread only).
- Hard constraints (don't change public API, don't bump policy file Y, don't
  add dependencies to crate Z).

## Review map

Structural index for the downstream LLM reviewer (CodeRabbit, ChatGPT, future
Claude session). One row per material change site:

| File | Line range | What | Why |
|---|---|---|---|
|  |  |  |  |
