# LEM Budgeting

LEM (Linux Equivalent Minutes) normalizes CI spend across runner types so
budgets can be reasoned about uniformly.

## Definition

```text
LEM = wall-clock job minutes × runner multiplier
```

A "Linux Equivalent Minute" is one minute on a baseline `ubuntu-latest`
runner. Other runner classes are converted via a multiplier so that a five
minute macOS job is not silently ten times more expensive than a five minute
Linux job.

## Standard budget bands

| Band         |    LEM | Meaning                        |
| ------------ | -----: | ------------------------------ |
| Pennies      |   0–12 | docs / metadata / light checks |
| Default      |  13–35 | ordinary Rust PR               |
| Elevated     |  36–75 | risk-expanded PR               |
| High         | 76–125 | explicit expensive PR          |
| Over ceiling |   >125 | requires override              |

These bands are advisory until `ci-actuals.json` has accumulated enough data
to learn per-lane defaults. Ordinary PRs should stay well below `$0.50` of
spend where possible. `$1` is a ceiling, not a goal.

## Runner multipliers

The canonical multipliers are encoded in `policy/ci-budget.toml`. As of this
document:

| Runner               | Multiplier |
| -------------------- | ---------: |
| `ubuntu-latest`      |        1.0 |
| `ubuntu-24.04`       |        1.0 |
| `windows-latest`     |        2.0 |
| `macos-latest`       |       10.0 |
| Node extension build |        2.0 |
| External AI review   |        4.0 |

Multipliers reflect runner billing weight, not wall-clock differences. The
multiplier applies to the wall-clock minutes the job actually spent, including
queue and setup time.

## Override and acknowledgement labels

| Label                  | Effect                                                |
| ---------------------- | ----------------------------------------------------- |
| `full-ci`              | Run the full deep validation set; expects high LEM.   |
| `ci-budget-ack`        | Author has acknowledged elevated estimate.            |
| `ci-budget-override`   | Bypass the over-ceiling failure for this PR only.     |
| `ripr-waive`           | Acknowledge a `ripr` advisory finding (post PR 14).   |
| `coverage`             | Run coverage lane.                                    |
| `mutation`             | Run mutation calibration lane.                        |
| `vscode`               | Force VS Code lane on a non-extension diff.           |
| `release-check`        | Run release surface lanes (package, publish dry-run). |

These are documented in `docs/ci/labels.md`. The labels are inert until the
PR Plan and budget guard PRs land them in the actual flow.

## Forecast vs. actuals flow

```text
forecast (PR Plan)
  -> actuals (ci-actuals.json)
  -> learned estimates (per lane, per repo)
  -> updated forecasts
```

PR Plan emits a forecast on every PR. `ci-actuals.json` is uploaded by jobs.
Learned estimates feed back into the next forecast. Hard enforcement only
turns on once the learned distribution is stable.
