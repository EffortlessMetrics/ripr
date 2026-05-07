# CI Actuals

`ci-actuals.json` is the per-job telemetry record that closes the
forecast → actuals → learned-estimate loop described in `docs/CI.md`'s
Verification Economics section.

## Schema

```json
{
  "schema_version": 1,
  "repo": "ripr",
  "sha": "<commit SHA>",
  "workflow": "<workflow file basename>",
  "jobs": [
    {
      "name": "<job id from policy/ci-lane-whitelist.toml>",
      "runner": "ubuntu-latest",
      "estimated_lem": 18,
      "actual_seconds": 420,
      "actual_lem": 7,
      "conclusion": "success",
      "cache_hit": true,
      "risk_packs": ["analysis_engine"]
    }
  ]
}
```

## Field reference

| Field             | Type     | Description                                                  |
| ----------------- | -------- | ------------------------------------------------------------ |
| `schema_version`  | integer  | Bump when the layout changes. Currently `1`.                 |
| `repo`            | string   | Repo short name; `ripr`.                                     |
| `sha`             | string   | Commit SHA the job ran against.                              |
| `workflow`        | string   | Filename of the workflow file.                               |
| `jobs[].name`     | string   | Lane id (matches `policy/ci-lane-whitelist.toml`).           |
| `jobs[].runner`   | string   | Runner class; matches `runner_multipliers` in the budget.    |
| `jobs[].estimated_lem` | integer | Forecast value emitted by PR Plan (PR 07).              |
| `jobs[].actual_seconds` | integer | Wall-clock seconds the job took.                       |
| `jobs[].actual_lem` | integer | `ceil(actual_seconds / 60 * runner_multiplier)`.            |
| `jobs[].conclusion` | string  | One of `success`, `failure`, `cancelled`, `skipped`.         |
| `jobs[].cache_hit`  | boolean | True when the job restored a warm rust-cache.                |
| `jobs[].risk_packs` | array   | Risk pack ids that selected this lane (from PR Plan).        |

## Posture

- **PR 11 (this PR)** establishes the schema as a documented contract.
  Workflows do not yet emit `ci-actuals.json`.
- **Follow-up PR**: wire `cargo xtask ci actuals --workflow <name>
  --json-out target/ci/ci-actuals.json` into each lane and upload the
  artifact. Each lane uploads its own JSON; PR Plan rolls them up.
- **PR 12** consumes the rolled-up actuals to inform the soft budget
  guard.

## Why no enforcement yet

The forecast → actuals → learned-estimate loop only works once a
*distribution* of actuals exists per lane. Until at least 2 weeks of
`ci-actuals.json` data has accumulated, any threshold is guesswork. The
schema is fixed now so every lane that lands later already emits the
right shape.

## Forecast vs. actuals reconciliation

PR Plan emits `estimated_lem`. The job records `actual_lem`. The
follow-up that wires this exposes the delta in the step summary:

```text
rust_fast_gate: estimated 18 LEM, actual 12 LEM (Δ -6)
ripr_self_dogfood: estimated 6 LEM, actual 4 LEM (Δ -2)
```

A persistent positive delta (forecast under-estimates) means the lane's
`base_lem` in the whitelist needs to grow. A persistent negative delta
(forecast over-estimates) is fine — it just leaves slack budget for
unexpected lanes.

## Storage

Actuals are uploaded as artifacts during the lane run, then aggregated
by the PR Plan job into a single `ci-actuals.json` for the run. Long-term
retention happens via the Codecov / Test Analytics path
(`.github/workflows/test-analytics.yml`) once the schema lands; a
dedicated retention path is out of scope for this rollout.

## See also

- `docs/CI.md` — Verification Economics policy (LEM definition, bands).
- `policy/ci-budget.toml` — `[[budget_band]]` and `[[label]]` ledgers.
- `policy/ci-lane-whitelist.toml` — lane registry.
- `policy/ci-risk-packs.toml` — changed-paths → lane-set mapping.
