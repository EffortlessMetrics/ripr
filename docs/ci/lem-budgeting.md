# LEM Budgeting

**Local Evidence Minutes** (LEM) is the planning unit for CI cost in this repo.

One LEM is approximately one minute of hosted CI time on one normal GitHub
runner, including setup, toolchain/cache work, command runtime, report writing,
and artifact upload for that lane.

LEM is intentionally approximate until `target/ci/ci-actuals.json` exists and
accumulates history. PRs should still estimate the order of magnitude so
reviewers can notice when a small docs change starts paying for a release-style
proof.

## Band definitions

| Band | LEM range | Meaning |
| --- | ---: | --- |
| Pennies | 0–12 | Docs, metadata, light policy checks. |
| Default | 13–35 | Ordinary Rust PR. |
| Elevated | 36–75 | Risk-expanded PR (multi-surface, broad evidence). |
| High | 76–125 | Explicitly expensive PR. |
| Over ceiling | >125 | Requires override label. |

Target: ordinary PRs well below $0.50. $1/PR is a hard ceiling, not the design
center.

The bands above are the **planning vocabulary**. The `policy/ci-budget.toml`
file defines the currently-active enforcement posture per band. Until enforcement
is set to anything other than `advisory`, no band triggers a blocking failure.

## LEM estimation

At the time of this document, LEM is estimated statically by the PR Plan
workflow from the risk-pack / lane mapping in:

- `policy/ci-risk-packs.toml` — changed-path → risk-pack mapping
- `policy/ci-lane-whitelist.toml` — risk-pack → lane mapping with base LEM
- `policy/ci-budget.toml` — budget band thresholds and enforcement policy

Once `ci-actuals.json` data exists, the planner reads observed lane history and
replaces the static base LEM with learned estimates:

```text
estimate = max(static_floor, p50_recent_actual × 1.15)
warning  = p90_recent_actual
hard planning = p95_recent_actual
```

See `docs/ci/pr-plan.md` for the forecast schema and `docs/ci/ci-actuals.md`
for the actuals schema.

## LEM-in-PRs

Every PR that changes CI workflows, risk packs, budget policy, or lane selection
should fill the CI Economics section of the PR template:

```text
- LEM impact:
- Workflows touched:
- Branch protection impact:
- Failure mode caught:
- Cheaper signal considered:
- Rollback path:
```

Ordinary PRs that do not affect CI behavior may use `n/a`.
