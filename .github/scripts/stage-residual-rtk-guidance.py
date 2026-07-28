#!/usr/bin/env python3
"""Stage the reviewed RTK and live-authority source port on a disposable branch."""

from pathlib import Path

PATHS = [
    Path("docs/IMPLEMENTATION_CAMPAIGNS.md"),
    Path("docs/RIPR_SWARM_HUMAN_WORKFLOW.md"),
    Path("docs/lanes/LANE_1_RIPR_PLUS_BURNDOWN.md"),
    Path("docs/lanes/LANE_4_PR_CI_REVIEW.md"),
    Path("docs/swarm-development.md"),
    Path("plans/adoption-integration-cleanup/README.md"),
    Path("plans/adoption-integration-cleanup/implementation-plan.md"),
    Path("plans/campaign-27/lane3-editor-preview-routing.md"),
    Path("plans/lane4-pr-ci-review-cockpit/implementation-plan.md"),
    Path("plans/rust-usable-gap-projection/agent-context-route.md"),
    Path("plans/rust-usable-gap-projection/implementation-plan.md"),
]

for path in PATHS:
    text = path.read_text(encoding="utf-8")
    path.write_text(text.replace("rtk ", ""), encoding="utf-8")

path = Path("docs/swarm-development.md")
text = path.read_text(encoding="utf-8")
start = text.index("## Swarm Operator Loop\n")
end = text.index("\n## Runner Posture\n", start)
section = """## Swarm Operator Loop

Use current repo state as the source of truth before starting or reviewing work:

```bash
git fetch origin --prune
git status --short --branch
gh pr list --repo EffortlessMetrics/ripr-swarm --state open
gh pr list --repo EffortlessMetrics/ripr --state open
gh issue list --repo EffortlessMetrics/ripr-swarm --state open --limit 100
```

Treat ordinary development PRs in `EffortlessMetrics/ripr` as source/swarm
drift. Port, redirect, or close them unless they are release, security, or
explicit promotion work.

The retired `.ripr/goals` scheduler is not live execution authority. Do not
continue a closed campaign or infer a successor from chat history. Select work
from repo-owned evidence in this order:

1. open `ripr-swarm` PRs, reviews, and required checks;
2. ordinary source-repo PRs that should be ported or redirected;
3. open issues with explicit ownership and current acceptance criteria, including
   their linked accepted RIPR-SPEC requirements, proposals, ADRs, and plans;
4. historical campaign documents only as context, never as current authorization.

After a PR is selected from live GitHub evidence, consult its matching
`ImplementationSliceV1` under `.allow/spec-system/slices/` to bound that PR's
scope. Slice files do not select work or authorize execution.

If no aligned work is available, leave the trunk clean. Record new routed-runner
proof on #24 or #34 only when there is fresh evidence; otherwise do not create a
make-work campaign.

Every normal swarm slice should finish the same way:

- open a same-repo PR with one clear purpose;
- wait for `Ripr Rust Small Result` and any touched-surface checks;
- merge only when clean and current;
- remove generated residue, isolated targets, and stale local branches or
  worktrees that are no longer needed.
"""
path.write_text(text[:start] + section + text[end:], encoding="utf-8")

path = Path("docs/IMPLEMENTATION_CAMPAIGNS.md")
text = path.read_text(encoding="utf-8")
first_campaign = text.index("## Campaign 1:")
intro = """# Implementation Campaigns

This document preserves historical campaign-level context for Codex Goals and
long-context contributor work. It is not live execution authority. The campaigns
below remain useful for chronology, objectives, and completed work-item context.

Live work selection and ownership come from GitHub issues, pull requests, checks,
reviews, and the local worktree. One PR's scope is its `ImplementationSliceV1`
under `.allow/spec-system/slices/`; normative behavior lives in RIPR-SPEC
requirements. Do not infer current work from a campaign status below.

"""
path.write_text(intro + text[first_campaign:], encoding="utf-8")

path = Path("docs/lanes/LANE_4_PR_CI_REVIEW.md")
text = path.read_text(encoding="utf-8")
text = text.replace(
    "| Active goal manifest | current Codex/Droid execution state | `.ripr/goals/active.toml` or a lane manifest when Lane 4 is active |",
    "| Live work evidence | current ownership, scope, and review state | GitHub issues, PRs, checks, reviews, the local worktree, and the PR-local `ImplementationSliceV1` |",
)
old = """Proposal explains why. Specs define what must be true. ADRs record durable
architecture decisions. Plans sequence PRs. Active manifests tell agents what
to do now. Policy ledgers own authority and exceptions. Closeouts record what
happened and what remains."""
new = """Proposal explains why. Specs define what must be true. ADRs record durable
architecture decisions. Plans sequence bounded work but do not authorize it.
Current GitHub and local-worktree evidence identifies what is live; the PR-local
implementation slice bounds the change. Policy ledgers own authority and
exceptions. Closeouts record what happened and what remains."""
if old not in text:
    raise SystemExit("expected Lane 4 authority paragraph not found")
path.write_text(text.replace(old, new), encoding="utf-8")
