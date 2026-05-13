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
`check-generated-clean`.

Cost: ~0 LEM (local only, no CI runners).

### Step 2 — PR Plan (advisory, always runs on PR)

Runs the changed-file → risk-pack → lane → LEM forecast. Emits `ci-plan.json`
and a step summary. Does not block. Does not comment.

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

Cost: variable, captured in `ci-actuals.json`.

### Step 5 — On-demand / release (label or main only)

- `cargo package -p ripr --list`
- `cargo publish -p ripr --dry-run`
- VSIX packaging and e2e.
- Release readiness checks.
- Server archive checks.

Triggered by: `full-ci`, `release-check`, push to `main`, manual dispatch.

Cost: 30–80 LEM.

## Soft gate (future, after calibration)

The `ripr` soft gate sits between steps 3 and 4 once advisory data shows the
false-positive rate is acceptable. It is acknowledgeable with `ripr-waive` and
never permanently blocks a merge.

See `docs/ci/ripr-soft-gate.md` for trigger criteria and acknowledgement rules.

## VS Code lane

The VS Code e2e lane runs at step 5 posture even though it is not release
proof. It runs when:

- `editors/vscode/**` changed, or
- `vscode` or `full-ci` label is applied, or
- push to `main`.

It does not run for unrelated Rust analysis changes. This keeps the typical
Rust PR away from Node setup, `xvfb`, and browser e2e overhead.

## Reading the LEM column

Every lane job emits a `ci-actuals.json` record so the forecast can be compared
against observed cost. Once history accumulates, the PR Plan planner replaces
static base-LEM estimates with learned p50 actuals.

See `docs/ci/lem-budgeting.md` for the LEM definition and band table.
