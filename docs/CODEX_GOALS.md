# Codex Goals

Codex `/goal` is the autonomous campaign runner for `ripr`.

A Codex goal is not one PR. A Codex goal is a long-running implementation
campaign that may create many scoped PRs, blocked reports, receipts, and
planning updates until the campaign end state is met.

For the agent-neutral repository tracking model (proposals, specs, ADRs,
campaigns, work items, the active goal manifest, and handoffs) that any
runner — Codex, Kiro, Claude Code, Cursor, or a generic agent — consumes,
see [Repo tracking model](REPO_TRACKING_MODEL.md).

When a handoff uses generic proof-stack language, Codex should translate it
into RIPR's existing repo artifacts instead of adding a second task system:
proposal/PRD means `docs/proposals/RIPR-PROP-*`, spec means
`docs/specs/RIPR-SPEC-*`, ADR means `docs/adr/`, implementation plan means
`docs/IMPLEMENTATION_PLAN.md`, `docs/IMPLEMENTATION_CAMPAIGNS.md`, or `plans/`,
active goal means `.ripr/goals/active.toml`, policy ledger means `policy/*.toml`
or `.ripr/traceability.toml`, capability claims mean `docs/CAPABILITY_MATRIX.md`
and `metrics/capabilities.toml`, support tiers mean
`docs/status/SUPPORT_TIERS.md`, closeout means `docs/handoffs/`, and durable
learning means `docs/LEARNINGS.md`.

The repository supplies the harness around that loop:

- implementation campaign docs
- scoped PR contract
- `xtask` shape, check, fixture, golden, metrics, and report commands
- fixture and golden conventions
- metrics and capability manifests
- spec-test-code traceability
- PR summaries
- CI report artifacts
- blocked reports

## Vocabulary

Use these terms consistently:

| Term | Meaning |
| --- | --- |
| Codex Goal | Long-running autonomous campaign objective. |
| Campaign | Multi-PR implementation sequence with an objective and end state. |
| Work item | PR-sized unit of progress inside a campaign. |
| Scoped PR | Mergeable review unit with a narrow production delta and evidence package. |
| Receipt | Machine-readable or durable proof of what ran and passed. |
| Blocked report | Durable stop artifact when the agent cannot safely continue. |
| PR summary | Human-readable reviewer packet under `target/ripr/reports/pr-summary.md`. |

Avoid collapsing Codex Goals into PR-sized tasks. Work items and PRs are the
review units inside the campaign; the campaign is the Codex Goal.

The correct model is:

```text
Codex /goal
  = multi-day, multi-PR implementation campaign

Campaign
  = one large objective with an end state

Work item
  = one PR-sized slice inside that campaign

Scoped PR contract
  = the evidence and quality bar for each PR-sized slice
```

## Campaign Progress

A campaign advances through a queue of scoped work items. Each work item should
produce one reviewable PR, one blocked report, or one explicit planning update.

The goal is complete only when the campaign end state is satisfied, not when one
PR is opened.

Codex Goals runs should use repository artifacts instead of chat history:

- [Implementation campaigns](IMPLEMENTATION_CAMPAIGNS.md)
- [Implementation plan](IMPLEMENTATION_PLAN.md)
- [Scoped PR contract](SCOPED_PR_CONTRACT.md)
- [PR automation](PR_AUTOMATION.md)
- `.ripr/goals/active.toml`
- `target/ripr/reports/`

## Multiple PRs

A Codex goal may create multiple scoped PRs in one run only when the work items
are independent or explicitly marked stackable.

If a non-stackable work item must land before the next item can safely be based
on `main`, Codex should finish the current PR first: open or update it, repair
review findings, validate it, merge it when ready, verify `main`, and then move
to the next item.

Do not silently build multiple dependent PRs on an unmerged branch unless the
campaign manifest marks those work items as stackable.

Goal manifests do not carry a special merge-permission field. Merge readiness
comes from ordinary repo policy: branch protection, required checks, draft
state, review comments, current operator direction, scope/risk, and whether the
PR's issues have been reviewed and addressed.

## Stop Conditions

A Codex Goals run should stop or write a blocked report when continuing would
require human judgment or would broaden scope.

Stop for:

- policy exceptions
- architecture boundary exceptions
- dependency additions
- schema or public output contract changes without explicit scope
- golden blessing decisions
- credential, publish, or marketplace decisions
- non-stackable dependency boundaries for dependent work items
- missing acceptance evidence that cannot be produced within the work item

Blocked reports should be written to:

```text
target/ripr/reports/blocked.md
```

They should name:

- active campaign
- work item
- failing command
- blocker
- why continuing would broaden scope or require human judgment
- recommended next action

## Campaign Manifest

The active campaign manifest is:

```text
.ripr/goals/active.toml
```

It is the machine-readable pointer for campaign state. It names the active
campaign, end state, work items, dependencies, stackability, and required
commands.

The current `xtask` goals commands read this manifest:

```bash
cargo xtask goals status
cargo xtask goals next
cargo xtask goals report
```

Validate the manifest with:

```bash
cargo xtask check-goals
```

`check-goals` validates the active execution manifest and the focused tracker
rails around it. `check-campaign` remains a compatibility alias. Focused
tracker manifests must stay outside
`.ripr/goals/active.toml`, done work items must stay tied to proof commands,
and declared source-of-truth paths must point at existing proposal, plan, spec,
receipt, and closeout files. When campaign docs reference a tracker manifest,
the referenced manifest path must exist.

Blocked work items are manifest state, not a separate mutation command. Record
blocked work in the active manifest with `status = "blocked"`,
`blocked_reason`, and `blocked_by` when applicable. `cargo xtask goals next`
surfaces those blocked items and their reasons so agents do not infer ready work
from chat history when the queue is intentionally blocked.
