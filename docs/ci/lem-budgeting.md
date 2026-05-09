# LEM Budgeting

## Definition

**Local Evidence Minute (LEM)**: approximately one minute of hosted CI time on one standard
GitHub runner (`ubuntu-latest`), including setup, toolchain/cache warm-up, command runtime,
report writing, and artifact upload for that lane.

LEM is intentionally approximate. One LEM ≈ one runner-minute on `ubuntu-latest`. Faster
runners (Blacksmith, large GitHub-hosted) carry a multiplier. Until `ci-actuals.json` has
accumulated history, estimates are structural forecasts; they are still useful for order-of-
magnitude comparisons and for catching surprise lane additions.

## Current Band Table (from `policy/ci-budget.toml`)

These are the bands currently implemented in the policy ledger:

| Band | LEM range | Posture | Description |
|------|----------:|---------|-------------|
| `small` | 0–5 | required | Docs, policy metadata, or focused code checks |
| `medium` | 6–20 | required | Ordinary product PR with Rust and policy gates |
| `large` | 21–60 | advisory | Multi-surface PR, extension checks, or broad evidence |
| `release` | 61+ | on-demand | Explicit `release-check` or `full-ci` proof |

The budget guard is currently `"off"` (advisory ledger only). Hard enforcement waits for
`ci-actuals.json` data. See [`docs/ci/budget-guard.md`](budget-guard.md).

## Rollout Target Band Table

The rollout plan refines the bands to give more resolution in the middle range where most
routine PRs land. These will be implemented when the numeric PR Plan (PR 10) and soft budget
guard (PR 15) land:

| Band | LEM | Meaning |
|------|----:|---------|
| Pennies | 0–12 | Docs, metadata, light policy checks |
| Default | 13–35 | Ordinary Rust PR (design center) |
| Elevated | 36–75 | Risk-expanded PR — advisory warning |
| High | 76–125 | Explicit expensive PR — stronger warning, suggest `ci-budget-ack` |
| Over ceiling | >125 | Requires `full-ci` or `ci-budget-override` label |

The mapping between the current `small/medium/large/release` IDs and the target numeric
thresholds will be done in PR 10 (`ci(plan): implement numeric LEM PR Plan`).

## The $0.50 Design Center and $1 Hard Ceiling

Running an ordinary Rust PR—`fmt`, `clippy`, `cargo test`, and a handful of xtask policy
gates—should cost well below $0.50 (≈ 12–35 LEM). That is the design center.

$1 per PR (≈ 125 LEM) is the hard ceiling, not the target. PRs that exceed the ceiling
require an explicit acknowledgement label. PRs that exceed it without a label fail the soft
budget guard once enforcement is on.

## Runner Multipliers (illustrative)

| Runner | Approximate LEM multiplier |
|--------|:--------------------------|
| `ubuntu-latest` (GitHub-hosted) | 1.0× |
| Blacksmith (large) | ~4–6× per minute saved |
| `ubuntu-latest` with warm cache | 0.5–0.7× (cache credit) |

Multipliers are not yet calibrated against actuals. Once `ci-actuals.json` exists, the
planner will use observed per-lane durations rather than structural estimates.

## Label Effects on Budget

| Label | Budget effect |
|-------|--------------|
| `full-ci` | Maps forecast to `release` band; suppresses ceiling warning |
| `release-check` | Same as `full-ci`; runs release-readiness lanes |
| `ci-budget-ack` | Acknowledges overrun at `large`/`elevated`; budget-neutral |
| `vscode` | Adds editor lane cost; maps to `large`/`elevated` band |
| `coverage` | Adds coverage lane; maps to `large`/`elevated` band |
| `ripr-waive` | Acknowledges advisory ripr findings; budget-neutral |
| `clippy-future` | Runs advisory future-Clippy lane; maps to `medium`/`default` band |

Full label definitions live in `policy/ci-budget.toml` `[[label]]` entries.

## Estimating a PR's Cost

A quick hand-estimate before `ci-plan.json` exists:

```
Required: rust (~12 LEM), msrv (~5 LEM), xtask policy gates (~3 LEM)  = ~20 LEM
Advisory: coverage (+8 LEM), ripr self-dogfood (+4 LEM)               = +12 LEM advisory
Extension: vscode compile+e2e (+15 LEM)                                = +15 on vscode PRs
```

So an ordinary Rust PR costs ≈ 20 LEM required + 12 LEM advisory ≈ 32 LEM total: inside the
Default band. A PR with VS Code extension changes adds ~15 LEM. A full release proof can
reach 80–120 LEM.

## See Also

- [`policy/ci-budget.toml`](../../policy/ci-budget.toml) — authoritative band ledger
- [`docs/ci/budget-guard.md`](budget-guard.md) — guard behavior matrix
- [`docs/ci/cost-and-verification-policy.md`](cost-and-verification-policy.md) — why we care
- [`docs/ci/pr-plan.md`](pr-plan.md) — numeric PR plan
- [`docs/CI.md`](../CI.md) — verification economics policy
