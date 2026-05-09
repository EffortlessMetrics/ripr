# CI Labels

Labels are the operator interface for the CI economics system. They modify lane
selection, budget acknowledgement, and gate behavior.

## Label registry

| Label | Effect |
| --- | --- |
| `full-ci` | Runs all lanes including release-surface proof. Maps forecast to release band; suppresses budget warnings. |
| `release-check` | Same lane mapping as `full-ci`; runs release readiness lanes explicitly. |
| `ci-budget-ack` | Acknowledges an over-budget forecast at the `elevated` band. Budget-neutral; does not run additional lanes. |
| `vscode` | Forces the VS Code extension lane on for PRs that do not touch `editors/vscode/` but need it. |
| `ripr-waive` | Waives a `ripr` soft-gate finding for this PR. Requires a written reason in the PR body. |

## When labels take effect

Labels are read at workflow start. The PR Plan step runs first and emits a
`ci-plan.json` that includes the resolved label set. Subsequent lane jobs read
the plan to decide whether to run.

Applying a label after a workflow run has already started requires a
re-synchronize (push or empty commit) or a manual workflow dispatch to pick up
the new label.

## Label authorization

Any contributor may apply `ci-budget-ack` or `vscode`. Applying `full-ci`,
`release-check`, or `ripr-waive` on a PR should be visible in the PR timeline
so reviewers can verify the override is intentional.

`ripr-waive` must include a written reason in the PR body. PRs without a reason
string for `ripr-waive` fail the `cargo xtask check-pr-shape` gate (once that
check is wired).

## Relationship to budget bands

See `docs/ci/lem-budgeting.md` for band definitions and the enforcement
posture that maps bands to label requirements.

## Adding a new label

New labels require:

1. An entry in `policy/ci-budget.toml` `[[label]]` section.
2. An entry in `.github/settings.yml` so the label exists in the repo.
3. An update to this document.
4. A `cargo xtask check-pr` pass to verify `check-workflows` is still clean.
