# CI Current State

This document records the current (as of 2026-05-09) implementation state of
the CI economics system. It is the honest answer to "what actually runs today?"
as distinct from the target design in `docs/CI.md`.

## What is implemented

### Cancellation and cache posture

- PR synchronize events cancel previous runs (correct).
- Cache saves happen only on `main` (correct).
- Release-surface checks gate on push/main or explicit labels (correct).

### Policy gates (all blocking on relevant PRs)

- `cargo fmt --check`
- `cargo check --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo xtask check-no-panic-family`
- `cargo xtask check-allow-attributes`
- `cargo xtask check-static-language`
- `cargo xtask check-file-policy`
- `cargo xtask check-executable-files`
- `cargo xtask check-workflows`
- `cargo xtask check-spec-format`
- `cargo xtask check-fixture-contracts`
- `cargo xtask check-traceability`
- `cargo xtask check-capabilities`
- `cargo xtask check-workspace-shape`
- `cargo xtask check-architecture`
- `cargo xtask check-public-api`
- `cargo xtask check-output-contracts`
- `cargo xtask check-doc-index`
- `cargo xtask check-dependencies`
- `cargo xtask check-supply-chain`

### Advisory lanes (exist, non-blocking)

- `ripr` self-dogfood (advisory only; not a gate).
- Coverage via `cargo-llvm-cov` (advisory; Codecov status is informational).
- Test Analytics.

### On-demand lanes (label or main)

- `cargo package -p ripr --list`
- `cargo publish -p ripr --dry-run`
- VSIX packaging and e2e (currently runs on every PR — see gap below).

## Gaps vs target state

| Gap | Target PR | Impact |
| --- | --- | --- |
| No numeric PR Plan (`ci-plan.json`) | PR 10 | No LEM forecast before lanes run. |
| No `ci-actuals.json` emission | PR 12 | No forecast→actuals loop. |
| VS Code e2e runs on every PR | PR 13 | Pays for Node+xvfb on unrelated Rust PRs. |
| `ripr` self-dogfood is advisory but no LEM tracking | PR 14 | Cannot measure cost of self-verification. |
| No soft budget guard | PR 15 | No warning when PRs exceed budget bands. |
| `indexing_slicing` / `string_slice` are not active | PR 07 | Missing per-call receipts for parser/diff bounded indexing and slicing. |

## Policy files that exist but are not yet fully enforced

- `policy/ci-budget.toml` — `policy_state = "advisory-ledger"`, `enforcement = "none"`.
- `policy/ci-lane-whitelist.toml` — defined but not read by a running planner yet.
- `policy/ci-risk-packs.toml` — defined but not read by a running planner yet.

None of these represent broken invariants. They are correct drafts waiting for
the matching xtask implementation.

## Compatibility mirrors

- `.ripr/no-panic-allowlist.toml` — legacy schema 0.2 compatibility mirror;
  the canonical checker reads `policy/no-panic-allowlist.toml`.

## MSRV state

- Current `workspace.package.rust-version`: `1.95`
- Current `rust-toolchain.toml` channel: `1.95.0`
- Target: `1.95`
- Rust 1.95 compatibility audit: pass on 2026-05-09; see
  [Rust 1.95 compatibility audit](msrv-1.95-audit.md).
- PR 03 promoted clean Rust 1.94/1.95 lints:
  `same_length_and_capacity`, `manual_ilog2`, `needless_type_cast`,
  `decimal_bitwise_operands`, `manual_checked_ops`, `manual_take`,
  `duration_suboptimal_units`, and `unnecessary_trailing_comma`.
- Planned lints retained with explicit blockers:
  `disallowed_fields` needs a reviewed `clippy.toml` protected-seam config,
  `manual_pop_if` is not recognized by Rust 1.95.0 Clippy, and
  `indexing_slicing` / `string_slice` need per-call receipts.
