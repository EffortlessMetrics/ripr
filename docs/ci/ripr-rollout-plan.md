# `ripr` MSRV 1.95 Rollout Plan

`ripr` is the reference implementation for the Rust estate: MSRV 1.95, panic-free production
and tests, strict AST/string/indexing rails, semantic TOML policy receipts, LEM-aware CI, and
`ripr` self-dogfood.

This document is the anchor. Every later PR in the rollout stack references it to stay on track.

## Purpose

The rollout is a ratchet, not a rebuild. Current state is already strong:

- `crates/ripr` + `xtask` Rust 2024 workspace
- `unsafe_code = "forbid"` workspace-wide
- Extensive panic-family lint profile
- CI with synchronize-only cancellation and main-only cache saves
- `policy/clippy-lints.toml` tracking planned 1.94/1.95 lint flips
- `policy/non-rust-allowlist.toml` as canonical TOML exception ledger
- SARIF/advisory `ripr` self-dogfood in CI

Target state adds: MSRV 1.95, canonical schema-0.3 no-panic allowlist, active 1.95 lint set,
LEM-numeric PR planning, VS Code lane routing, `ripr` soft gate (after calibration).

See [`docs/ci/current-state.md`](current-state.md) for the per-dimension baseline.

## PR Sequence

| PR | Title | Type | Depends on |
|---:|-------|------|------------|
| 00 | docs(policy): document ripr MSRV 1.95 policy rollout | docs | — |
| 01 | policy(msrv): audit Rust 1.95 compatibility | docs/audit | 00 |
| 02 | policy(msrv): move ripr to Rust 1.95 | code+config | 01 |
| 03 | policy(clippy): promote planned Rust 1.95 lints | config+code | 02 |
| 04 | policy(panic): make schema 0.3 no-panic allowlist canonical | code | 00 |
| 05 | policy(panic): remove or receipt test panic debt | code | 04 |
| 06 | policy(clippy): require expect-with-reason suppressions | code | 05 |
| 07 | policy(clippy): activate AST slicing and indexing rails | code | 06 |
| 08 | testing: add fallible test helpers | code | 05 |
| 09 | docs(ci): document current PR Plan and budget path | docs | 00 |
| 10 | ci(plan): implement numeric LEM PR Plan | code | 09 |
| 11 | ci(plan): add advisory PR Plan workflow | workflow | 10 |
| 12 | ci(telemetry): emit CI actuals | code+workflow | 11 |
| 13 | ci(vscode): route extension lane by extension risk | workflow | 00 |
| 14 | ci(ripr): add self-dogfood advisory lane | workflow | 03 |
| 15 | ci(budget): add soft LEM guard | code+workflow | 12 |
| 16 | ci(metrics): scaffold learned LEM estimates | code | 15 |
| 17 | ci(ripr): implement acknowledgeable soft gate | code+workflow | 14 + calibration data |
| 18 | policy(test): add fallible assertion campaign (optional) | code | 08 |

## Natural Stacks

```
00 → 01 → 02 → 03        (MSRV ratchet)
04 → 05 → 06 → 07 → 08   (panic-free + suppression rails)
09 → 10 → 11 → 12 → 15 → 16  (CI economics)
14 → 17                   (ripr self-gate after calibration)
```

Independent (no blocking dependencies):

```
13  (VS Code lane routing — path-gated, does not require MSRV)
18  (optional fallible assertion campaign)
```

## Hard Rules

These apply to every PR in the stack:

- Do not weaken the `ripr` product contract.
- Do not use runtime mutation terms (`killed`, `survived`) outside explicit runtime calibration reports.
- Do not add Clippy test carveouts (`allow-unwrap-in-tests`, etc.).
- Do not add bare `#[allow(...)]` without reason.
- Do not weaken `unsafe_code = "forbid"`.
- Do not hide panic debt by lowering lint levels globally.
- Do not make `ripr` findings blocking until advisory data exists.
- Do not hard-enforce learned LEM budgets before `ci-actuals.json` data has accumulated.
- Do not combine docs path, MSRV bump, panic debt cleanup, CI routing, and soft-gate into one PR.

## Merge Policy

| Condition | Required before merge |
|-----------|----------------------|
| Any PR | `cargo fmt --check`, `cargo check`, `cargo clippy -D warnings`, `cargo test` |
| MSRV bump | `cargo +1.95.0 check/clippy/test` clean or blockers documented |
| Lint policy | `cargo xtask check-lint-policy` passes |
| No-panic | `cargo xtask check-no-panic-family` and `cargo xtask no-panic propose` pass |
| CI workflow | `cargo xtask check-workflows` passes |
| ripr findings not blocking | Advisory data must exist (≥ 2 weeks of `ci-actuals.json`) |
| Learned budgets not enforced | `ci-actuals.json` must have accumulated history |

Bot quota/rate-limit notices are non-actionable noise. Stale comments against old commits are
stale after verifying current HEAD. Only fix actionable comments against the current diff.

## What "Done" Looks Like

```
workspace.package.rust-version = "1.95"
unsafe_code = "forbid"
panic-family lints: deny
indexing_slicing: deny
string_slice: deny
no clippy.toml test carveouts
policy/clippy-lints.toml msrv = "1.95"
planned 1.94/1.95 lints: active or explicitly retained with reason
policy/no-panic-allowlist.toml: canonical schema 0.3
.ripr/no-panic-allowlist.toml: retired or compatibility-only
cargo xtask check-no-panic-family: reads policy/no-panic-allowlist.toml
test unwrap/expect/panic debt: removed or short-expiry receipted
policy/non-rust-allowlist.toml: canonical and checked
bare #[allow(...)]: rejected
#[expect(...)]: requires reason
ci-plan.json: emitted per PR
ci-actuals.json: emitted per run
VS Code lane: path/label/main-gated
ripr self-dogfood: advisory artifacts uploaded
soft budget guard: warns at elevated, enforces only hard ceiling
ripr soft gate: deferred until calibration data exists
```

## PR Review Loop

For each PR:

1. Open as draft.
2. Include: purpose, LEM impact, workflows touched, branch-protection impact, failure mode caught,
   cheaper signal considered, rollback path, commands run.
3. Read all bot/reviewer comments.
4. Fix actionable comments against current HEAD.
5. Re-run relevant checks.
6. Mark ready only after self-review.
7. Merge when required checks are green and actionable feedback is resolved.
8. Rebase dependent PRs after each merge.

## See Also

- [`docs/ci/current-state.md`](current-state.md) — per-dimension baseline snapshot
- [`docs/ci/cost-and-verification-policy.md`](cost-and-verification-policy.md) — economics framing
- [`docs/ci/lem-budgeting.md`](lem-budgeting.md) — LEM bands and $1/PR ceiling
- [`docs/ci/labels.md`](labels.md) — label catalog and CI effects
- [`docs/ci/verification-ladder.md`](verification-ladder.md) — where `ripr` fits
- [`docs/CLIPPY_POLICY.md`](../CLIPPY_POLICY.md) — dual-rail lint design
- [`docs/NO_PANIC_POLICY.md`](../NO_PANIC_POLICY.md) — panic-free policy
- [`docs/POLICY_ALLOWLISTS.md`](../POLICY_ALLOWLISTS.md) — allowlist system
