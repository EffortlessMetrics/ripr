# Rust 1.95 / 0.6.0 Release Shaping

This document started as the 0.5.1 quality hardening anchor. The accumulated
scope is now a 0.6.0 feature and experience release, so the release identity is
0.6.0 while the original Rust 1.95 quality rails remain part of the proof trail.
It records the current state, the target state, the PR ladder, and the rules
each PR must follow.

The MSRV bump to Rust 1.95 already landed in 0.5.0. This rollout is **not** another MSRV
bump. It is about documenting the current state, hardening policy rails, shaping
the 0.6.0 release, and keeping the expanded user-visible scope honest.

## Current / target state

| Layer | Current | Target | Status |
| --- | --- | --- | --- |
| Edition | 2024 | 2024 | done |
| MSRV | 1.95 | 1.95 | done |
| Toolchain | 1.95.0 | 1.95.0 | done |
| Published version | 0.5.0 | 0.6.0 | planned |
| Clippy policy | strict profile active | complete ledger/checker alignment | partial |
| No-panic | allowlist present (schema 0.3) | exact counted allowlist + baseline/no-new-debt | partial |
| Non-Rust policy | allowlist present | companion ledgers + strict mode | partial |
| CI | broad `xtask` checks | risk-routed, receipt-backed, release-ready | partial |
| Coverage | advisory workflow exists | advisory/routed with receipt boundary | partial |
| Mutation | not default PR replacement | targeted/nightly/release lanes | planned |
| Release workflow | dry-run on-demand | explicit readiness workflow | todo |

## What 0.6.0 contains

The 0.6.0 release bundles:

- No-panic allowlist exact-identity hardening (path + family + selector + snippet + count).
- Clippy ledger and checker alignment for all active lints.
- Companion file-policy ledgers for generated, executable, dependency, workflow, process, and
  network surfaces.
- Evidence lane tuning: mutation stays targeted/nightly/release, not a default PR tax.
- Targeted Rust 1.95 API cleanup in evidence and report builders.
- Release readiness workflow and dry-run proof.
- Policy operations, history, promotion packets, preview-promotion packets, and
  advisory generated-CI projection.
- Preview TypeScript/Python visibility that remains advisory and non-gating by
  default.
- Repo-operations cockpit, PR-ready packet, command mutability catalog, PR
  triage disposition, generated-evidence discipline, and suggested fixes.
- Editor first-run/status and first-useful-action polish.

## Why 0.6.0

The MSRV bump landed in 0.5.0, and the original plan was patch-level quality
hardening. The landed scope now includes user-visible editor, CI, preview
language, policy-operations, and repo-operations surfaces. 0.6.0 is the correct
release identity for that accumulated experience work.

## Rust 1.95 value for `ripr`

| Rust 1.95 item | `ripr` use |
| --- | --- |
| `if let` guards | AST/seam classification, evidence-record selection, report routing, baseline/gate matching. |
| Atomic `update` / `try_update` | Future telemetry counters, server/session status, advisory run counters. |
| `cfg_select!` | Generated CI/platform snippets, VS Code server resolution, path differences. |
| `cold_path` | Invalid evidence packet, corrupted baseline, malformed policy, missing input, blocked GitHub comment plan. |
| 1.95 Clippy lints | `manual_checked_ops`, `manual_take`, `duration_suboptimal_units`, `same_length_and_capacity`. |

## PR ladder

| PR | Branch | Title | Type |
| --- | --- | --- | --- |
| 1 | `docs/rust-1.95-quality-rollout` | docs(policy): map Rust 1.95 and 0.6.0 release shaping | docs only |
| 2 | `policy/rust-1.95-consistency-audit` | policy(rust): verify Rust 1.95 consistency | audit |
| 3 | `policy/no-panic-exact-identity` | policy(panic): harden no-panic allowlist identity | hardening |
| 4 | `policy/no-panic-baseline` | policy(panic): add exact no-panic baseline and no-new-debt mode | hardening |
| 5 | `policy/no-panic-reporting` | policy(panic): improve no-panic report diagnostics | UX |
| 6 | `policy/clippy-ledger-alignment` | policy(clippy): align active lint ledger with workspace lints | alignment |
| 7 | `policy/disallowed-fields-protected-seams` | policy(clippy): configure protected seam field bans | planned |
| 8 | `policy/file-companion-ledgers` | policy(files): add companion allowlists for generated and risky surfaces | hardening |
| 9 | `ci/evidence-lane-tuning` | ci: tune coverage, mutation, and release evidence lanes | CI |
| 10 | `release/workflow-readiness` | release: add 0.6.0 release readiness workflow | release |
| 11 | `refactor/rust-1.95-api-cleanups` | refactor: use Rust 1.95 APIs in evidence and report builders | cleanup |
| 12 | `policy/first-burndown` | policy: burn down first no-panic and Clippy debt | cleanup |
| 13 | `release/0.6.0-prep` | release: prepare 0.6.0 | release |
| 14 | `release/0.6.0-dry-run` | release: validate 0.6.0 publish readiness | release |

## Acceptance gates per PR

Every PR in this ladder must pass:

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask check-pr
git diff --check
```

Plus, PR-specific gates:

| PR | Extra gates |
| --- | --- |
| 1 | `cargo xtask check-doc-index`, `cargo xtask markdown-links` |
| 3 | `cargo test -p xtask no_panic`, `cargo xtask check-no-panic-family` |
| 4 | `cargo xtask no-panic baseline --reset` (this PR only) |
| 5 | `test -f target/ripr/reports/no-panic.md` |
| 6 | `cargo xtask check-lint-policy`, `cargo xtask check-allow-attributes` |
| 7 | (audit only if no protected fields are ready) |
| 8 | `cargo xtask check-file-policy`, `cargo xtask check-executable-files`, `cargo xtask check-workflows`, `cargo xtask check-generated`, `cargo xtask check-dependencies`, `cargo xtask check-process-policy`, `cargo xtask check-network-policy` |
| 9 | `cargo xtask policy-report` |
| 10 | `cargo xtask release-readiness --version 0.6.0 \|\| true`, `cargo package -p ripr --list`, `cargo publish -p ripr --dry-run` |
| 11 | `cargo xtask dogfood` |
| 12 | `cargo xtask check-no-panic-family`, `cargo xtask policy-report` |
| 13 | `cargo xtask release-readiness --version 0.6.0`, `cargo package -p ripr --list`, `cargo publish -p ripr --dry-run` |
| 14 | same as 13 |

## Hard rules for every PR in this stack

- Do not weaken the `ripr` product contract.
- Do not make `ripr` findings blocking until advisory data exists.
- Do not use runtime mutation terms (`killed`, `survived`) outside explicit runtime mutation
  calibration reports.
- Do not add Clippy test carveouts.
- Do not add bare `#[allow(...)]` without a `policy/clippy-exceptions.toml` entry.
- Do not weaken `unsafe_code = "forbid"`.
- Do not make mutation a default ordinary-PR tax.
- Do not make inline comments default-on.
- Do not change the MSRV; it is already 1.95.
- Do not reset the no-panic baseline except in PR 4.
- Do not combine docs, MSRV, panic debt, CI routing, and soft gate into one PR.

## Evidence-lane doctrine

`ripr` is not a replacement for mutation testing. It is the PR-time static exposure filter.
Mutation remains runtime evidence for targeted, nightly, and release lanes.

See `docs/ci/ripr-mutation-boundary.md` for the boundary doctrine and
`docs/ci/test-evidence-lanes.md` for the lane split.

## Bot / CI loop

For every PR:

1. Identify the first failing command from CI logs.
2. Reproduce locally.
3. Fix only that failure.
4. Re-run the matching local gate.
5. Push.
6. Check bot comments again.

If a bot comments:

| Comment type | Action |
| --- | --- |
| Real defect | Fix it. |
| False positive | Reply with evidence. |
| Style-only, cheap, in scope | Fix it. |
| Out of scope | Defer with a follow-up reference. |
| Stale against old commit | Verify HEAD and mark stale. |

## Required self-review checklist

Before marking a PR ready:

```markdown
## Self-review
- Scope matches PR title:
- Files touched are expected:
- No unrelated cleanup:
- MSRV was not changed accidentally:
- Policy changes are intentional:
- No Clippy test carveouts added:
- No bare `#[allow(clippy::...)]` added:
- No-panic baseline handling is scoped:
- Non-Rust allowlist changes are narrow:
- Evidence lane boundary preserved:
- Release proof preserved:
- Local validation:
- CI status:
- Bot comments addressed:
- Follow-ups:
```

## See also

- [`docs/ci/ripr-rollout-plan.md`](ripr-rollout-plan.md) — MSRV 1.95 ratchet PR stack (PRs 00–18).
- [`docs/ci/test-evidence-lanes.md`](test-evidence-lanes.md) — lane split doctrine.
- [`docs/ci/ripr-mutation-boundary.md`](ripr-mutation-boundary.md) — mutation boundary.
- [`docs/CLIPPY_POLICY.md`](../CLIPPY_POLICY.md) — Clippy dual-rail policy.
- [`docs/NO_PANIC_POLICY.md`](../NO_PANIC_POLICY.md) — no-panic dual-rail policy.
- [`docs/FILE_POLICY.md`](../FILE_POLICY.md) — non-Rust file policy.
- [`docs/POLICY_ALLOWLISTS.md`](../POLICY_ALLOWLISTS.md) — all allowlists index.
- [`docs/ci/current-state.md`](current-state.md) — current CI implementation state.
- [`docs/ci/verification-ladder.md`](verification-ladder.md) — PR verification ladder.
