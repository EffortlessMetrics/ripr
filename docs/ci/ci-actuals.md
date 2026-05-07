# CI Actuals

`ci-actuals.json` is the per-lane telemetry record that closes the
forecast → actuals → learned-estimate loop described in `docs/CI.md`'s
Verification Economics section. **The canonical schema lives in
`docs/CI.md`'s "CI Actuals" subsection** — this document expands on it
with field-level guidance and emission notes; it does not redefine the
shape.

## Canonical schema (mirrored from `docs/CI.md`)

```json
{
  "schema_version": "0.1",
  "workflow": "ci",
  "job": "rust",
  "status": "success",
  "duration_seconds": 212,
  "runner": "ubuntu-latest",
  "estimated_lem": 8,
  "actual_lem": 9,
  "cache_hit": true
}
```

One record per lane. Multiple lanes upload multiple records; PR Plan
aggregates them into a single artifact for the run.

## Field reference

| Field              | Type    | Description                                                              |
| ------------------ | ------- | ------------------------------------------------------------------------ |
| `schema_version`   | string  | Bump when the layout changes. Currently `"0.1"`.                         |
| `workflow`         | string  | Filename basename of the workflow that emitted the record.               |
| `job`              | string  | Job key inside that workflow.                                            |
| `status`           | string  | One of `success`, `failure`, `cancelled`, `skipped`.                     |
| `duration_seconds` | integer | Wall-clock seconds the job took.                                         |
| `runner`           | string  | GitHub runner label (e.g. `ubuntu-latest`).                              |
| `estimated_lem`    | integer | Forecast emitted by PR Plan at the start of the run.                     |
| `actual_lem`       | integer | Measured LEM. The runner-class multiplier mapping is repo policy; see    |
|                    |         | `policy/ci-budget.toml` once the multiplier table lands. Until then,     |
|                    |         | `actual_lem == ceil(duration_seconds / 60)` for `ubuntu-latest`-class.   |
| `cache_hit`        | boolean | True when the job restored a warm Rust cache.                            |

The runner-class multiplier mapping is intentionally not pinned in the
schema. It is repo policy and will live in `policy/ci-budget.toml` once
the table is materialised. Today, all merged jobs run on `ubuntu-
latest` so the multiplier defaults to `1.0`; expensive runner classes
(Windows, macOS, etc.) are added to the policy file when first used.

## Posture

- **PR 11 (this PR)** establishes the field-level documentation that
  expands on the canonical schema in `docs/CI.md`.
- **Follow-up**: wire each lane to emit `target/ci/ci-actuals.json`
  using the schema above. Each lane uploads its own JSON; PR Plan rolls
  them up.
- The soft budget guard (PR 12) consumes the rolled-up actuals; it
  remains advisory until `[defaults].budget_guard` in
  `policy/ci-budget.toml` flips from `"off"`.

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
ripr-self-dogfood: estimated 6 LEM, actual 4 LEM (Δ -2)
```

A persistent positive delta (forecast under-estimates) means the lane's
expected band needs to grow. A persistent negative delta (forecast
over-estimates) is fine — it just leaves slack budget for unexpected
lanes.

## Storage

Actuals are uploaded as artifacts during the lane run, then aggregated
by the PR Plan job into a single `ci-actuals.json` for the run. Long-
term retention happens via the Codecov / Test Analytics path
(`.github/workflows/test-analytics.yml`) once the schema lands; a
dedicated retention path is out of scope for this rollout.

## See also

- `docs/CI.md` — canonical "CI Actuals" subsection and Verification
  Economics policy.
- `policy/ci-budget.toml` — `[[budget_band]]` and `[[label]]` ledgers.
- `policy/ci-lane-whitelist.toml` — lane registry.
- `policy/ci-risk-packs.toml` — changed-paths → lane-set mapping.
