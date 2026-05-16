# Merge Freshness And Watcher Policy

This document defines how maintainers and agents should watch a PR from "checks
are running" to "safe to merge" without turning merge readiness into repeated
manual polling or unnecessary CI restarts.

The policy is advisory. It does not change branch protection, auto-merge,
merge-queue settings, required checks, workflows, comments, approvals, or branch
updates.

## Standard Packet

Start with the repo-ops packets:

```bash
cargo xtask cockpit
cargo xtask gh-pr-status --pr <number>
```

`cockpit` gives the board-level state. `gh-pr-status --pr <number>` gives the
per-PR merge-readiness packet, including merge state, behind-main state,
required check status when GitHub exposes it, review state, Droid status, and
the suggested next action.

If GitHub CLI's GraphQL path is rate-limited or stale, use a read-only REST
fallback for inspection:

```bash
gh api repos/EffortlessMetrics/ripr/pulls/<number>
gh api repos/EffortlessMetrics/ripr/commits/<head-sha>/check-runs
```

Do not use the fallback to mutate the PR. It is only a status source.

## Watch Cadence

Avoid short-loop polling. It wastes rate limit, makes logs noisy, and rarely
changes the next safe action.

Use this cadence unless a maintainer gives a stricter one:

- broad Rust or workspace jobs running: wait two to four minutes between checks
- one final required job running: wait about one minute between checks
- GitHub reports unknown mergeability: wait long enough for GitHub to recompute,
  then refresh once
- no checks are changing after repeated reads: stop and inspect the failed,
  queued, or missing context instead of continuing to poll

Do not poll every few seconds unless you are debugging the status tool itself.

## Safe Next Actions

Use the packet's recommendation as the default, then apply these rules:

| Action | Use when | Do not use when |
| --- | --- | --- |
| `wait` | required checks are pending and the branch is otherwise current | a required check already failed or GitHub says the branch cannot merge |
| `inspect failure` | a required check failed, was cancelled unexpectedly, or is missing | the only issue is that optional advisory checks are still running |
| `refresh branch` | the PR is behind main and strict freshness is required before merge | long required jobs are near completion and their result is still useful |
| `merge` | required checks pass, review requirements are satisfied, and mergeability is clean | the branch changed externally and has not been re-read |
| `stop for judgment` | auto-merge settings, branch protection, required review, or duplicate-PR selection is unclear | the issue is deterministic cleanup that the repo already reports |

`refresh branch` means update the PR branch by the repo's normal flow. It does
not mean force-push over remote work.

## Branch Freshness

When `main` moves while checks are running, do not restart CI reflexively. Let
active required jobs finish if their result will still be useful for repair.
Refresh only when strict freshness has invalidated the run, GitHub reports the
branch behind and blocked, or the current run is already cancelled or stale.

Before pushing your own branch update:

1. Fetch current `origin/main`.
2. Re-read the remote PR head.
3. If someone else updated the branch, inspect and fast-forward or merge it
   locally before editing.
4. Rerun the narrow local validation that matches your update.
5. Push only your reviewed branch state.

Never force-push over external branch movement unless the maintainer explicitly
asks for that operation.

## Droid And Advisory Checks

Droid and other advisory checks are review signals unless branch protection or a
repo policy makes them required for the PR. Do not block a ready PR only because
an advisory check is still running. Do inspect advisory failures when they touch
the changed surface or identify a concrete regression risk.

If the required-context list is unavailable, treat the hosted branch-protection
state as the source of truth and record the uncertainty in the PR packet.

## Merge Execution

Prefer merging only after `gh-pr-status --pr <number>` or an equivalent REST
inspection says the branch is clean enough to merge.

If a local detached or side worktree cannot merge a PR because the branch is
checked out elsewhere, merge from the primary checkout or use an explicit
GitHub-hosted merge path after re-reading the remote PR state. Do not move local
branch refs to work around that limitation.

Auto-merge and merge queue may be used only when they are already enabled for
the repository and the operator has authority to use them. This document does
not grant permission to change those settings.

## Cleanup

After a PR branch is merged or abandoned, clean only the worktree created for
that task:

```bash
cargo clean
git worktree remove <task-worktree>
```

Do not remove unrelated worktrees, delete remote branches, or clean shared
build directories unless the maintainer explicitly asks.
