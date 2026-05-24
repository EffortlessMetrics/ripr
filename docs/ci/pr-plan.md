# PR Plan

The PR Plan workflow (`.github/workflows/pr-plan.yml`) is the LEM forecast
lane. It runs on opened, synchronized, reopened, labeled, and unlabeled pull
requests as an **advisory** signal: it does not block merges, and it does not
gate any required check.

## What it should do (target state)

Given the diff between the PR and its base, the PR Plan should:

1. Read the changed-file list.
2. Match those paths to risk packs in `policy/ci-risk-packs.toml`.
3. Resolve the union of `lanes` across matched packs.
4. Look up `base_lem` per lane in `policy/ci-lane-whitelist.toml`.
5. Apply runner multipliers from `policy/ci-budget.toml`.
6. Emit `target/ci/ci-plan.json` and a step-summary table.

The forecast is intentionally *coarse*: per-lane base LEM x runner multiplier,
summed across selected lanes. Once `ci-actuals.json` data exists, the forecast
can be replaced by learned per-lane estimates.

## What this PR ships

This PR ships:

- The workflow YAML at `.github/workflows/pr-plan.yml`.
- A budget entry in `policy/workflow_allowlist.txt`.
- This document.

The job currently emits a structural advisory output (changed-file list,
ledger-presence check, step-summary placeholder). It uploads the changed-file
artifact only on failure or when the PR is labeled `full-ci`, so ordinary PRs
keep the advisory signal without paying for routine artifact retention. The
forecast logic is deferred to the follow-up PR that adds `cargo xtask ci plan`.

## Output schema (target)

```json
{
  "schema_version": 1,
  "repo": "ripr",
  "base": "<base SHA>",
  "head": "<head SHA>",
  "labels": ["full-ci", "ripr-waive"],
  "risk_packs": ["analysis_engine", "policy_xtask"],
  "selected_lanes": [
    { "id": "rust_fast_gate", "base_lem": 8, "runner_multiplier": 1.0 },
    { "id": "check_no_panic_family", "base_lem": 1, "runner_multiplier": 1.0 }
  ],
  "estimated_lem": 9,
  "band": "small",
  "warnings": []
}
```

`band` is one of `small | medium | large | release` (the band ids from
`[[budget_band]]` in `policy/ci-budget.toml`, also documented in
`docs/CI.md`'s Verification Economics section).

`warnings` is filled by the soft budget guard when the forecast crosses an
advisory threshold.

## Posture

| Stage                                    | Posture                |
| ---------------------------------------- | ---------------------- |
| Initial workflow                         | structural advisory    |
| Follow-up: `xtask ci plan` wired         | numeric advisory       |
| `ci-actuals.json` upload                 | numeric advisory       |
| Soft budget guard                        | warn / fail-on-ceiling |

## Override and acknowledgement

Labels documented in `docs/CI.md` (labels section):

- `full-ci` expects release-band forecast; the guard suppresses the
  warning.
- `release-check` uses the same release-band mapping as `full-ci` and runs
  release readiness lanes.
- `ci-budget-ack` records that the author acknowledges elevated forecast (no
  budget effect).

## Why advisory first

A blocking forecast on day one would either be too loose to catch real
overspend or too tight and reject ordinary PRs. Running the forecast in
advisory mode for at least two weeks gives the learned-estimate path
enough data to base a credible threshold on, and lets reviewers calibrate
which warnings they ignore.
