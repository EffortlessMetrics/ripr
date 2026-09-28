# Soft Budget Guard

The soft budget guard reads the `[[budget_band]]` ledger in
`policy/ci-budget.toml` and the LEM forecast emitted by PR Plan, and
turns the overrun into an acknowledgeable signal.

## Behavior matrix

The bands are exactly those declared in `policy/ci-budget.toml`:

| Band      | LEM range | Posture                | Documented description                                       |
| --------- | --------- | ---------------------- | ------------------------------------------------------------ |
| `small`   |     0-5   | `required`             | Docs, policy metadata, or focused code checks.               |
| `medium`  |    6-20   | `required`             | Ordinary product PR with Rust and policy gates.              |
| `large`   |   21-60   | `advisory`             | Multi-surface PR, extension checks, or broad evidence.       |
| `release` |     61+   | `on_demand_release`    | Explicit `release-check` or `full-ci` proof.                 |

When PR Plan's forecast lands in `small` or `medium`, the guard is
silent. When it lands in `large`, the guard emits an advisory step-
summary line. When it lands in `release` without a release-bearing
label, the guard escalates the warning and asks the author to apply
`ci-budget-ack` (or `release-check` / `full-ci` if the PR really is
release-bearing).

The guard never fails the workflow until the overall ledger flips
`policy_state` from `advisory-ledger` to an enforcing state. Today
`policy_state = "advisory-ledger"`, `enforcement = "none"`, and
`[defaults].budget_guard = "off"`, so any implementation must stay
warn-only and exit successfully.

## Override and acknowledgement labels

Every label is defined in `policy/ci-budget.toml` `[[label]]` entries:

| Label             | Effect on the guard                                                |
| ----------------- | ------------------------------------------------------------------ |
| `full-ci`         | Maps the forecast to the `release` band; suppresses the warning.   |
| `release-check`   | Maps the forecast to the `release` band; runs currently wired release-surface proof. |
| `ci-budget-ack`   | Acknowledges the overrun at the `large` band; budget-neutral.      |
| `vscode`          | Maps to `large` band when an editor lane is forced on.             |
| `coverage`        | Maps to `large` band when coverage is forced on.                   |
| `clippy-future`   | Maps to `medium` band when the future-Clippy advisory lane is on.  |
| `ripr-waive`      | Acknowledges advisory `ripr` findings; budget-neutral.             |

## Calibration constraint

Hard enforcement remains intentionally off until `ci-actuals.json` data
has accumulated for at least 14 days on the lanes the guard meters
against. The `ci-budget.toml` ledger encodes this with
`[defaults].actuals_required = false` and `[defaults].budget_guard =
"off"`. The guard must not enforce until those defaults flip; see
`docs/CI.md`'s Verification Economics section for the doctrine.

## Failure mode caught

Without the guard, expensive lanes can be added silently and the cost
surfaces only in the monthly bill. The guard puts that cost in front of
the author and reviewers at PR-time as soon as the actuals data is
available.

## Failure mode the guard does not solve

The guard only sees lanes that PR Plan selected. If a workflow runs
outside the lane whitelist (`policy/ci-lane-whitelist.toml`), the guard
cannot see it. The CI lane whitelist checker (already on `main` via
`xtask`) is the complementary enforcement that makes sure new lanes are
accounted for.

## Implementation posture

- **Current state**: schema and behavior contract documented against
  the merged `policy/ci-budget.toml` shape. **No enforcement code yet.**
- **Follow-up PR**: `cargo xtask ci budget-guard --plan
  target/ci/ci-plan.json --actuals target/ci/ci-actuals.json
  --budget policy/ci-budget.toml --labels-json "$LABELS_JSON"`,
  emitting an exit code that maps to the band (and stays advisory until
  the ledger's `[defaults].budget_guard` flips).

## See also

- `docs/CI.md` - Verification Economics policy.
- `policy/ci-budget.toml` - `[[budget_band]]` and `[[label]]` ledgers.
- `policy/ci-lane-whitelist.toml` - lane registry.
- `policy/ci-risk-packs.toml` - changed-paths to lane-set mapping.
