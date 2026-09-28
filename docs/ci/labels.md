# CI Labels

Labels are the operator interface for the CI economics system. They modify lane
selection, budget acknowledgement, and gate behavior when the matching workflow
or planner wiring exists. The registry below mirrors the documented policy
vocabulary; the next section separates current wiring from target behavior.

## Label registry

| Label | Effect |
| --- | --- |
| `full-ci` | Runs the currently wired broad advisory, editor, MSRV, and release-surface proof. Maps forecast to release band; suppresses budget warnings. |
| `release-check` | Runs the release-surface proof without opting into every `full-ci` lane: package list, publish dry-run, and release-readiness. |
| `ci-budget-ack` | Acknowledges an over-budget forecast at the `large` band. Budget-neutral; does not run additional lanes. |
| `vscode` | Target label for forcing the VS Code extension lane on PRs that do not touch `editors/vscode/` but need it. |
| `coverage` | Runs the advisory coverage lane for the labeled pull request. `full-ci` also selects `coverage`. |
| `clippy-future` | Runs future or candidate Clippy lint lanes in advisory mode. |
| `ripr-waive` | Target label for acknowledging a `ripr` soft-gate finding for this PR. Requires a written reason in the PR body. |

### Status labels (`status/*`)

The `status/*` set is the closed status vocabulary used by the status-comment
verification contract (`AGENTS.md` § "Status-comment verification contract").
A status label is the primary status signal; a status comment is secondary and
must not contradict the label. See `AGENTS.md` for the full contract — this
table only documents the label meanings.

| Label | Meaning |
| --- | --- |
| `status/done-open` | Delivered; the issue is intentionally kept open. |
| `status/blocked-upstream` | Blocked on `cargo-allow` or an external repository. |
| `status/blocked-repo` | Blocked on a tracked in-repo item (PR, issue, or code). |
| `status/needs-work` | Actionable and not started. Do **not** use it for partially landed work; that understates delivery and invites duplicate implementation. |
| `status/mis-scoped` | Re-scope, close, or supersede — the issue is the wrong scope or stale. |
| `status/partial` | A bounded portion has merged to `main` (or another authoritative repository) and the residual acceptance plus next owner are recorded. Apply only when a merged deliverable exists — never merely planned. |

These labels do not modify CI lane selection. They are consumed by agents and
reviewers as the durable campaign-record signal; the open-issue list is the
campaign record and a low-truth status comment buries substantive signal.

## When labels take effect

Current behavior:

- `release-check` activates Perl and release-surface proof.
- `full-ci` activates Perl/release proof, the named MSRV proof, VS Code
  integration, `coverage`, Test Analytics, and future-Clippy advisory proof.
- `coverage` activates the advisory coverage workflow.
- `clippy-future` activates the future-Clippy advisory workflow.
- `vscode`, `ci-budget-ack`, and `ripr-waive` remain documented policy
  vocabulary that is not yet wired to every target planner behavior; see
  [`current-state.md`](current-state.md).

The workflows that consume these labels subscribe to pull-request label events,
so applying or removing a wired label starts a new selection run. A new commit
is not required merely to make `coverage`, `full-ci`, `release-check`, or
`clippy-future` take effect.

Target behavior: the PR Plan step runs first and emits a `ci-plan.json` that
includes the resolved label set. Subsequent lane jobs read the plan to decide
whether to run.

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
