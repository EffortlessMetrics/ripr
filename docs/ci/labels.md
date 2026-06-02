# CI Labels

Labels are the operator interface for the CI economics system. They modify lane
selection, budget acknowledgement, and gate behavior when the matching workflow
or planner wiring exists. The registry below mirrors the documented policy
vocabulary; the next section separates current wiring from target behavior.

## Label registry

| Label | Effect |
| --- | --- |
| `full-ci` | Runs all lanes including release-surface proof. Maps forecast to release band; suppresses budget warnings. |
| `release-check` | Runs the currently wired release-surface proof without opting into every `full-ci` lane. Today that means the legacy Rust package list and publish dry-run steps. |
| `ci-budget-ack` | Acknowledges an over-budget forecast at the `large` band. Budget-neutral; does not run additional lanes. |
| `vscode` | Target label for forcing the VS Code extension lane on PRs that do not touch `editors/vscode/` but need it. |
| `coverage` | Documented target label for future coverage-lane selection; current coverage workflow runs on PRs, pushes, and manual dispatch without reading this label. |
| `clippy-future` | Runs future or candidate Clippy lint lanes in advisory mode. |
| `ripr-waive` | Target label for acknowledging a `ripr` soft-gate finding for this PR. Requires a written reason in the PR body. |

## When labels take effect

Current behavior: labels are read directly by the workflows that already have
label conditions. Today, `release-check` and `full-ci` affect the legacy Rust
workflow package list and publish dry-run steps, `full-ci` affects the legacy
VS Code CI job, and `clippy-future` or `full-ci` activates the future-Clippy
advisory workflow. The `vscode`, `coverage`, `ci-budget-ack`, and
`ripr-waive` labels are documented policy vocabulary, but not all of them are
wired to lane-selection behavior yet; see
[`current-state.md`](current-state.md).

Target behavior: the PR Plan step runs first and emits a `ci-plan.json` that
includes the resolved label set. Subsequent lane jobs read the plan to decide
whether to run.

Applying a label after a workflow run has already started requires a
re-synchronize (push or empty commit) or a manual workflow dispatch to pick up
the new label.

## Label authorization

Any contributor may apply `ci-budget-ack` or `vscode`. Applying `full-ci`,
`release-check`, or `ripr-waive` on a PR should be visible in the PR timeline
so reviewers can verify the override is intentional.

Target behavior: `ripr-waive` must include a written reason in the PR body.
PRs without a reason string for `ripr-waive` fail the
`cargo xtask check-pr-shape` gate once that check is wired to the label
semantics.

## Relationship to budget bands

See `docs/ci/lem-budgeting.md` for band definitions and the enforcement
posture that maps bands to label requirements.

## Adding a new label

New labels require:

1. An entry in `policy/ci-budget.toml` `[[label]]` section.
2. An entry in `.github/settings.yml` so the label exists in the repo.
3. An update to this document.
4. A `cargo xtask check-pr` pass to verify `check-workflows` is still clean.
