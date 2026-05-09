# Current State Snapshot

Dimension-by-dimension baseline as of the PR 00 rollout anchor (2026-05-09).
Each row describes where the repo is today and what the rollout target is.
See [`docs/ci/ripr-rollout-plan.md`](ripr-rollout-plan.md) for the PR that moves each dimension.

## MSRV

| | Today | Target | PR |
|-|-------|--------|-----|
| `workspace.package.rust-version` | `"1.93"` | `"1.95"` | 02 |
| `rust-toolchain.toml channel` | `"1.93.1"` | `"1.95.x"` | 02 |
| `policy/clippy-lints.toml msrv` | `"1.93"` | `"1.95"` | 02 |
| Rust 1.95 compat audit | not run | committed doc | 01 |

## Lint Policy

| | Today | Target | PR |
|-|-------|--------|-----|
| `unsafe_code` | `"forbid"` | `"forbid"` (unchanged) | — |
| Panic-family lints | `deny` (unwrap, expect, panic, todo, unreachable, …) | same | — |
| `indexing_slicing` | deferred (parser/diff bounded slicing) | `"deny"` + `#[expect]` receipts | 07 |
| `string_slice` | deferred (AST-bounded) | `"deny"` + `#[expect]` receipts | 07 |
| Planned 1.94 lints | `[[planned]]` in clippy-lints.toml | `deny`/`warn` in Cargo.toml | 03 |
| Planned 1.95 lints | `[[planned]]` in clippy-lints.toml | `deny`/`warn` in Cargo.toml | 03 |
| Test Clippy carveouts | none | none (maintained) | — |
| Suppression style | `allow_attributes_without_reason = "deny"` | + bare `#[allow]` rejected | 06 |

### Planned lints not yet active (from `policy/clippy-lints.toml`)

Targeting 1.94:
- `same_length_and_capacity` → deny
- `manual_ilog2` → warn
- `needless_type_cast` → warn
- `decimal_bitwise_operands` → warn

Targeting 1.95:
- `disallowed_fields` → deny (requires `clippy.toml` config)
- `manual_checked_ops` → warn
- `manual_take` → warn
- `manual_pop_if` → warn
- `duration_suboptimal_units` → warn
- `unnecessary_trailing_comma` → warn

## No-Panic Allowlist

| | Today | Target | PR |
|-|-------|--------|-----|
| Canonical file | `.ripr/no-panic-allowlist.toml` | `policy/no-panic-allowlist.toml` | 04 |
| Schema | `0.2` | `0.3` | 04 |
| Checker reads | `.ripr/no-panic-allowlist.toml` | `policy/no-panic-allowlist.toml` | 04 |
| `policy/no-panic-allowlist.toml` status | shadow/sample (3 representative entries) | canonical (all entries) | 04 |
| Test panic debt | `classification = "test_only"` entries | removed or short-expiry receipted | 05 |
| Fallible test helpers | ad-hoc | `test_support` module | 08 |

## Source Suppressions

| | Today | Target | PR |
|-|-------|--------|-----|
| `#[allow(...)]` governance | `allow_attributes_without_reason = "deny"` | + `check-allow-attributes` enforces | 06 |
| `#[expect(...)]` without reason | possible (no xtask check) | rejected by xtask check | 06 |
| Policy ID in reason | optional | required for durable exceptions | 06 |
| `.ripr/allow-attributes.txt` | exists (advisory allowlist) | enforced by updated checker | 06 |

## CI Economics

| | Today | Target | PR |
|-|-------|--------|-----|
| PR Plan workflow | structural advisory (file list + placeholder summary) | numeric LEM forecast | 10, 11 |
| `ci-plan.json` | not emitted | emitted per PR | 10 |
| `ci-actuals.json` | not emitted | emitted per run | 12 |
| Soft budget guard | `budget_guard = "off"` | warn at >35 LEM, enforce ceiling >125 | 15 |
| Learned estimates | not implemented | p50-based per lane | 16 |
| Band IDs (current) | `small`/`medium`/`large`/`release` | see `docs/ci/lem-budgeting.md` | 10 |
| VS Code lane routing | runs on every PR | path/label/main-gated | 13 |
| `ripr` self-dogfood | SARIF advisory, no soft gate | advisory + soft gate after calibration | 14, 17 |

## Non-Rust File Policy

| | Today | Target |
|-|-------|--------|
| Canonical allowlist | `policy/non-rust-allowlist.toml` (TOML with owner/surface/classification/reason/covered_by) | same (maintained) |
| xtask check | `cargo xtask check-file-policy` | same |

## Branch Protection and Required Checks

Today: `rust`, `msrv`, and policy xtask gates are required. VS Code runs on every PR.
Target: VS Code lane is path/label/main-gated (PR 13). No new required checks until soft
gate is calibrated. Advisory lanes upload artifacts and do not block.
