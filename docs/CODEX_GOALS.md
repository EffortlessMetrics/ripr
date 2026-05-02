# Codex Goals

Codex `/goal` is the autonomous campaign runner for `ripr`.

A Codex goal is not one PR. A Codex goal is a long-running implementation
campaign that may create many scoped PRs, blocked reports, receipts, and
planning updates until the campaign end state is met.

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

If a work item requires review or merge before the next item can safely be based
on `main`, Codex should open the PR, write a campaign handoff, and stop or move
only to an independent item.

Do not silently build multiple dependent PRs on an unmerged branch unless the
campaign manifest marks those work items as stackable.

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
- merge/review boundaries for non-stackable work items
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
cargo xtask check-campaign
```

Blocking a work item through `xtask` is planned but not implemented yet:

```bash
cargo xtask goals block <work-item-id> --reason "..."
```
