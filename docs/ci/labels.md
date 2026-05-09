# CI Labels

Labels allow contributors and maintainers to change lane selection, acknowledge overruns, and
control gate behavior without modifying workflow files. The authoritative ledger lives in
`policy/ci-budget.toml` `[[label]]` entries.

## Label Catalog

| Label | Effect | Budget effect |
|-------|--------|--------------|
| `full-ci` | Run all required, advisory, and release lanes | `release` band; suppresses budget ceiling warning |
| `release-check` | Run release-readiness lanes (`cargo package`, VSIX, server archive, publish dry-run) | `release` band |
| `vscode` | Force the VS Code extension lane even without editor path changes | `large`/`elevated` band |
| `coverage` | Force the coverage lane | `large`/`elevated` band |
| `clippy-future` | Run the advisory future-Clippy lane (candidate lints for next MSRV) | `medium`/`default` band |
| `ripr-waive` | Acknowledge advisory `ripr` static-exposure findings for this PR | budget-neutral |
| `ci-budget-ack` | Acknowledge that this PR intentionally exceeds the expected LEM band | budget-neutral |

## When to Use Each Label

**`full-ci`** — When you need complete end-to-end proof before merging. Use for changes
that affect multiple surfaces simultaneously or for anything that touches release
infrastructure.

**`release-check`** — When the PR prepares or validates a release. Automatically runs
`cargo package`, `cargo publish --dry-run`, VSIX packaging, and server archive checks.

**`vscode`** — When you change the VS Code extension and want to force the extension lane
on a PR that otherwise would not trigger it by path matching. After PR 13 lands, path-
matched extension PRs run this lane automatically; this label is for explicit overrides.

**`coverage`** — When coverage data matters for this PR's review. Coverage is advisory by
default; this label forces it on.

**`clippy-future`** — When you want to check a PR against the advisory future-Clippy lane
(lints planned for the next MSRV). Useful when advancing a planned lint to active status.

**`ripr-waive`** — When `ripr` self-dogfood flags a finding that you have reviewed and
determined is an acceptable gap (e.g., the behavior is covered by a higher-level test or an
integration test outside the diff). Requires a written note in the PR body explaining the
waiver. Does not apply when `full-ci` is present.

**`ci-budget-ack`** — When this PR genuinely needs more CI than the default band allows and
you have reviewed the cost. Typical for cross-surface PRs. The PR body should explain why
the elevated budget is justified.

## Label Interaction Rules

- `full-ci` supersedes `ripr-waive` within the same PR (full-ci implies complete proof).
- `ripr-waive` does not skip CI; it only acknowledges the advisory finding.
- `ci-budget-ack` does not change lane selection; it only acknowledges the forecast overrun.
- Multiple labels can be applied; the union of their effects applies.

## Advisory Status (2026-05-09)

The soft budget guard is currently `"off"` (`policy/ci-budget.toml`,
`[defaults].budget_guard = "off"`). Labels are parsed and recorded but do not enforce.
The guard will enforce after `ci-actuals.json` data has accumulated (PR 15).

## See Also

- [`policy/ci-budget.toml`](../../policy/ci-budget.toml) — authoritative label definitions
- [`docs/ci/budget-guard.md`](budget-guard.md) — guard behavior matrix
- [`docs/ci/lem-budgeting.md`](lem-budgeting.md) — LEM bands and cost estimates
