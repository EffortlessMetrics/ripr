# CI Labels

Labels route PRs to the right CI surface, acknowledge elevated cost, and gate
override paths. They are the user-facing dial that pairs with the LEM
forecast.

## Routing labels

| Label                  | Purpose                                                  |
| ---------------------- | -------------------------------------------------------- |
| `full-ci`              | Force the full deep validation set on this PR.           |
| `release-check`        | Force release surface lanes (package, publish dry-run).  |
| `vscode`               | Force VS Code extension build/test on a non-editor diff. |
| `coverage`             | Run the coverage lane.                                   |
| `mutation`             | Run the runtime mutation calibration lane.               |
| `clippy-future`        | Run the advisory future-Clippy scan.                     |

Routing labels add lanes; they should not remove ordinary required gates.

## Acknowledgement and override labels

| Label                  | Purpose                                                  |
| ---------------------- | -------------------------------------------------------- |
| `ci-budget-ack`        | Author acknowledges elevated LEM (36–75).                |
| `ci-budget-override`   | Bypass over-ceiling fail (>125 LEM).                     |
| `ripr-waive`           | Acknowledge an advisory `ripr` finding on this PR.       |

Override and waive labels are intentionally noisy in PR summaries so reviewers
can see them on the timeline.

## Conventions

- Labels are case-sensitive and must match the `[labels]` block of
  `policy/ci-budget.toml`.
- A bot or workflow reaction to a label should be idempotent.
- Removing an override label on a PR re-applies the original guard on the
  next push.
- Labels do not mutate code or policy — they only reshape which lanes run
  and which warnings escalate.

## How labels appear in PR Plan output

The PR Plan job lists active labels, the resulting lane set, and the
forecast LEM. Acknowledgement labels turn warnings into notices in the step
summary; override labels turn failures into warnings.

## Where labels are defined

- Source of truth: `policy/ci-budget.toml`, `[labels]` table.
- Suggested colors and descriptions: this document.
- Workflow listening: documented per-workflow alongside its lane entry in
  `policy/ci-lane-whitelist.toml`.
