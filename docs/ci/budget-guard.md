# Soft Budget Guard

The soft budget guard is the lane that turns the LEM forecast into an
acknowledgeable signal. It runs after PR Plan (PR 07) and reads the
forecast emitted there.

## Behavior matrix

| Estimated LEM | Behavior                                                  |
| ------------: | --------------------------------------------------------- |
|          0–35 | green (no warning)                                        |
|         36–75 | warning in PR step summary                                |
|        76–125 | high warning; suggest applying `ci-budget-ack` label      |
|          >125 | fail unless `full-ci` or `ci-budget-override` is present  |

Bands and exact LEM thresholds come from `policy/ci-budget.toml`:

```toml
[budget]
preferred_default_lem = 25
default_limit_lem     = 35
elevated_limit_lem    = 75
hard_limit_lem        = 125
```

## Override and acknowledgement labels

Defined in `policy/ci-budget.toml` `[labels]`:

| Label                  | Effect on the guard                                      |
| ---------------------- | -------------------------------------------------------- |
| `full-ci`              | Suppresses warning and bypasses over-ceiling fail.       |
| `ci-budget-ack`        | Demotes the warning at the elevated/high band.           |
| `ci-budget-override`   | Bypasses the over-ceiling fail (>125 LEM).               |

Override labels are intentionally noisy in PR summaries so reviewers can
see them on the timeline.

## When this PR's logic activates

The guard ships with these guards explicitly conditioned on data:

- It refuses to fail builds until `ci-actuals.json` data has been
  uploaded for at least 14 days.
- Until then, the guard runs in **warn-only** mode: every band emits the
  step-summary line, but the workflow conclusion is always success.
- After the 14-day calibration window, the over-ceiling fail activates
  with the override labels above as escape hatches.

This prevents the guard from misfiring on the first day. The doctrine
(in `docs/ci/cost-and-verification-policy.md`) is "Do not hard-enforce
learned LEM budgets before `ci-actuals.json` data exists" — the guard
respects that by self-disabling until the data exists.

## Failure mode caught

Without the guard, expensive lanes can be added silently and the cost
surfaces only in the monthly bill. The guard puts that cost in front of
the author and reviewers at PR-time.

## Failure mode the guard does not solve

The guard only sees lanes that PR Plan selected. If a workflow runs
outside the PR Plan whitelist (`policy/ci-lane-whitelist.toml`), the
guard cannot see it. The lane whitelist lint (PR 03) is the
complementary enforcement that makes sure new lanes are accounted for.

## Implementation posture

- **PR 12 (this PR)**: schema and behavior contract documented. The
  workflow / xtask wiring lands once PR 07 (PR Plan) and PR 11
  (ci-actuals) have produced an artifact pipeline.
- **Follow-up PR**: `cargo xtask ci budget-guard --plan
  target/ci/ci-plan.json --actuals target/ci/ci-actuals.json
  --budget policy/ci-budget.toml --labels-json "$LABELS_JSON"`,
  emitting an exit code that maps to the band.

## See also

- `docs/ci/lem-budgeting.md`
- `docs/ci/pr-plan.md`
- `docs/ci/ci-actuals.md`
- `docs/ci/labels.md`
- `policy/ci-budget.toml`
