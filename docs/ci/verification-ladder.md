# Verification Ladder

The verification ladder records the ordered set of checks a PR passes through
on the way from draft to merge. The ladder is designed so cheap checks run
first and expensive checks are only paid for when needed.

## Ladder steps

### Step 1 — Precommit (local, free)

```bash
cargo xtask precommit
```

Runs: `cargo fmt --check`, `check-static-language`, `check-no-panic-family`,
`check-allow-attributes`, `check-local-context`, `check-file-policy`,
`check-executable-files`, `check-workflows`, `check-spec-format`,
`check-fixture-contracts`, `check-traceability`, `check-capabilities`,
`check-workspace-shape`, `check-architecture`, `check-public-api`,
`check-output-contracts`, `check-doc-index`, `check-readme-state`,
`markdown-links`, `check-campaign`, `check-pr-shape`, `check-generated`,
`check-badge-diff-policy`, `check-generated-clean`.

Cost: ~0 LEM (local only, no CI runners).

### Step 2 — PR Plan (advisory, always runs on PR)

Current behavior: writes the changed-file list to
`target/ripr/reports/pr-plan-changes.txt`, checks that the CI economics
ledgers exist, and writes a step-summary placeholder. It does not block and
does not comment. The numeric changed-file → risk-pack → lane → LEM forecast
and `target/ci/ci-plan.json` output are target behavior for a follow-up PR.

Cost: 1–2 LEM.

### Step 3 — Required gates (blocking on relevant PRs)

- `cargo check --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `cargo test --workspace`
- Policy gates: `check-no-panic-family`, `check-allow-attributes`,
  `check-file-policy`, `check-workflows`, `check-static-language`,
  `check-output-contracts`, `check-doc-index`, `check-workspace-shape`,
  `check-architecture`, `check-public-api`, `check-dependencies`,
  `check-supply-chain`.

Cost: ~8–18 LEM for a typical Rust PR.

### Step 4 — Advisory lanes (non-blocking)

- Coverage report.
- `ripr` self-dogfood (advisory output; not a gate until calibrated).
- Test Analytics baseline delta.
- SARIF upload.

Cost: variable. Target behavior captures observed cost in `ci-actuals.json`.
Current behavior: advisory lanes run, but per-lane `target/ci/ci-actuals.json`
emission is not implemented yet.

### Step 5 — On-demand / release (label or main only)

- `cargo package -p ripr --list`
- `cargo publish -p ripr --dry-run`
- VSIX packaging and e2e when the relevant workflow lane is wired.
- Release readiness checks when the relevant workflow lane is wired.
- Server archive checks when the relevant workflow lane is wired.

Triggered by: `full-ci`, `release-check`, push to `main`, manual dispatch.

Cost: 30–80 LEM.

## Soft gate (future, after calibration)

The `ripr` soft gate sits between steps 3 and 4 once advisory data shows the
false-positive rate is acceptable. It is acknowledgeable with `ripr-waive` and
never permanently blocks a merge.

See `docs/ci/ripr-soft-gate.md` for trigger criteria and acknowledgement rules.

## VS Code lane

The VS Code e2e lane runs at step 5 posture even though it is not release
proof.

Current behavior: the legacy VS Code CI job runs on pushes to `main` or
`master`, manual dispatches, and pull requests labeled `full-ci`. It is not yet
path-gated and is not wired to the `vscode` label.

Target behavior: it runs when:

- `editors/vscode/**` changed, or
- `vscode` or `full-ci` label is applied, or
- push to `main`.

In the target model, it does not run for unrelated Rust analysis changes. This
keeps the typical Rust PR away from Node setup, `xvfb`, and browser e2e
overhead.

## Reading the LEM column

Target behavior: every lane job emits a `ci-actuals.json` record so the
forecast can be compared against observed cost. Once history accumulates, the
PR Plan planner replaces static base-LEM estimates with learned p50 actuals.
Today, `ci-actuals.json` is still planned; use
[`current-state.md`](current-state.md) for the implementation status.

See `docs/ci/lem-budgeting.md` for the LEM definition and band table.
