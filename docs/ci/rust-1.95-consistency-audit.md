# Rust 1.95 Consistency Audit

Date: 2026-05-10

Rollout item: PR 2, `policy(rust): verify Rust 1.95 consistency`

## Result

Status: **pass** — all surfaces agree on Rust 1.95. No stale references found that
incorrectly describe the current support floor.

## Surfaces checked

| Surface | Expected | Actual | Status |
| --- | --- | --- | --- |
| `Cargo.toml` `rust-version` | `1.95` | `1.95` | pass |
| `rust-toolchain.toml` channel | `1.95.0` | `1.95.0` | pass |
| `clippy.toml` `msrv` | `1.95` | `1.95` | pass |
| `policy/clippy-lints.toml` `msrv` | `1.95` | `1.95` | pass |
| `README.md` MSRV badge | `1.95` | `1.95` | pass |
| `CLAUDE.md` MSRV text | `1.95` | `1.95` | pass |
| `AGENTS.md` MSRV text | `1.95` | `1.95` | pass |
| `docs/CLIPPY_POLICY.md` | `1.95` | `1.95` | pass |
| `docs/RELEASE.md` `rust-version` | `1.95` | `1.95` | pass |
| `docs/ci/current-state.md` MSRV section | `1.95` | `1.95` | pass |
| `.github/workflows/` toolchain pins | `1.95.0` | `1.95.0` | pass |
| `crates/ripr/Cargo.toml` | `workspace = true` | `workspace = true` | pass |
| `xtask/Cargo.toml` | `workspace = true` | `workspace = true` | pass |
| `editors/vscode/package.json` | no MSRV claim | no MSRV claim | pass |

## Command used

```bash
rg '1\.93|1\.94|MSRV|msrv|rust-version|toolchain' \
  --type md --type toml --type rust \
  --glob '!target/**' --glob '!CHANGELOG.md'
```

## Findings requiring attention

None.

## References to 1.93 / 1.94 and why they are not stale

The search found these occurrences:

| Location | Content | Classification |
| --- | --- | --- |
| `policy/clippy-lints.toml` | `activate_when_msrv = "1.93"` for `indexing_slicing` | Descriptive: lint available since 1.93. MSRV is no longer the blocker; receipts are. See `reason` field. |
| `policy/clippy-lints.toml` | `activate_when_msrv = "1.93"` for `string_slice` | Same as above. |
| `xtask/src/main.rs` | `"rustc 1.93.1\n"` | Synthetic test fixture for output-parser testing. Not a configuration value. |
| `xtask/src/main.rs` | `activate_when_msrv = "1.94"` | Inline test ledger for parser unit tests. Not a configuration value. |
| `xtask/src/main.rs` | `activate_when_msrv = "1.93"` | Same — parser unit test fixture. |
| `CHANGELOG.md` | Various | Historical release notes. Intentionally preserved. |
| `docs/ci/ripr-rollout-plan.md` | Various | Historical rollout plan. Intentionally preserved. |
| `docs/ci/msrv-1.95-audit.md` | Various | Historical audit receipt. Intentionally preserved. |

None of these misstate the current support floor.

## Next step

Proceed to PR 3: `policy/no-panic-exact-identity`.

## See also

- [`docs/ci/msrv-1.95-audit.md`](msrv-1.95-audit.md) — original Rust 1.95 compatibility audit.
- [`docs/ci/current-state.md`](current-state.md) — current CI implementation state.
