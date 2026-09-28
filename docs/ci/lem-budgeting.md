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
| `small` | 0-5 | Docs, policy metadata, or focused code checks. |
| `medium` | 6-20 | Ordinary product PR with Rust and policy gates. |
| `large` | 21-60 | Multi-surface PR, extension checks, or broad evidence. |
| `release` | 61+ | Explicit `release-check` or `full-ci` proof. |

Target: ordinary PRs well below $0.50. $1/PR is a hard ceiling, not the design
center.

The bands above mirror `policy/ci-budget.toml`, which is the machine-readable
ledger and enforcement authority. Today `policy_state = "advisory-ledger"` and
`enforcement = "none"`, so no band triggers a blocking failure.

## LEM estimation

Target behavior: LEM is estimated statically by the PR Plan workflow from the
risk-pack / lane mapping in:

- `policy/ci-risk-packs.toml` — changed-path → risk-pack mapping
- `policy/ci-lane-whitelist.toml` — risk-pack → lane mapping with base LEM
- `policy/ci-budget.toml` — budget band thresholds and enforcement policy

Current behavior: the PR Plan workflow is structural advisory only. It writes
the changed-file list and ledger-presence summary, but it does not yet emit a
numeric LEM forecast or `target/ci/ci-plan.json`; see
[`pr-plan.md`](pr-plan.md) and [`current-state.md`](current-state.md).

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
