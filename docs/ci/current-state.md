# CI Current State

This document records the current implementation state of the CI economics
system as of 2026-08-25. It is the honest answer to “what actually runs today?”
as distinct from the target design in `docs/CI.md`.

## What is implemented

### Required Rust and repository-policy owner

`.github/workflows/routed-rust.yml` classifies docs-only changes, selects one
trusted self-hosted runner when available, falls back to GitHub-hosted when
required, and normalizes the protected `Ripr Rust Small Result` context.

The four Rust implementations delegate to `.github/workflows/rust-gates.yml`.
That reusable workflow is the single executable owner for ordinary required
Rust and repository-policy proof:

```text
cargo fmt --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo xtask precommit
cargo xtask check-evidence-promotion-honesty
cargo xtask check-agent-skills
cargo xtask check-dependencies
cargo xtask check-process-policy
cargo xtask check-network-policy
cargo xtask goldens check
cargo xtask fixtures
```

Formatting runs before nextest installation and hosted cache restoration. A
required failure skips broad advisory report generation. Ordinary successful
PRs do not upload the full report bundle unless `full-ci` is present; failed
runs retain bounded diagnostic evidence.

### Unique CI proof

`.github/workflows/ci.yml` no longer repeats the routed Rust gate table. It owns
only distinct proof:

- Perl feature-adapter check and focused Perl language-analysis tests;
- package list, publish dry-run, and release-readiness proof on `main`,
  `release-check`, or `full-ci`;
- the named duplicate MSRV proof on manual or `full-ci` runs;
- VS Code compile, package, and real-server E2E proof on pushes, manual runs,
  and `full-ci` pull requests.

The routed owner already runs Rust 1.95.0 on every ordinary PR and `main` push,
so the separate MSRV workspace check is not repeated on each push.

### Cancellation and cache posture

- PR synchronize events cancel superseded runs.
- Label events do not cancel an in-progress run.
- Cache saves happen on `main` unless a lane explicitly remains read-only.
- Release-surface checks run on `main` or explicit release labels.

### Advisory lanes

- Coverage runs on `main`, manual dispatch, and pull requests labeled
  `coverage` or `full-ci`.
- Test Analytics runs on `main`, manual dispatch, and pull requests labeled
  `full-ci`.
- Future Clippy runs on `main`, manual dispatch, `clippy-future`, or `full-ci`.
- Source-of-truth, security, review, and `ripr` evidence lanes retain their own
  documented advisory contracts.

Coverage and Test Analytics no longer rebuild the entire workspace on every
ordinary pull request. Their artifacts have five-day retention on these lanes.

## Gaps vs target state

| Gap | Impact |
| --- | --- |
| No numeric PR Plan (`ci-plan.json`) | The structural PR Plan exists, but there is no machine-readable LEM forecast before lane selection. |
| No `ci-actuals.json` emission | There is no forecast-to-actuals feedback loop or measured post-decision waste metric. |
| VS Code is not path-gated or wired to `vscode` | `full-ci` remains the only PR label that runs the editor lane. |
| Advisory workflows do not consume canonical Rust artifacts | Coverage and Test Analytics are selected economically, but selected runs still compile independently. |
| No soft budget guard | CI does not yet warn when the selected proof exceeds its forecast band. |

## Policy files that exist but are not yet fully enforced

- `policy/ci-budget.toml` — advisory budget and label vocabulary.
- `policy/ci-lane-whitelist.toml` — allowed lane and artifact-family registry.
- `policy/ci-risk-packs.toml` — target path-to-risk-pack mapping.

These are reviewable policy inputs. They do not yet constitute a numeric planner
or an enforcement claim.

## Compatibility mirrors

- `.ripr/no-panic-allowlist.toml` is the legacy schema 0.2 compatibility
  mirror; the canonical checker reads `policy/no-panic-allowlist.toml`.
- `cargo xtask check-no-panic-family` reports allowed findings, advisory
  `last_seen` drift, stale entries, unallowed findings, and warnings. Ambiguous
  selector matches and duplicate semantic identities fail until selectors are
  made unique.
- `cargo xtask check-no-panic-family --propose` writes review-only Markdown and
  TOML migration proposals without rewriting the canonical allowlist.

## MSRV state

- Current `workspace.package.rust-version`: `1.95`
- Current `rust-toolchain.toml` channel: `1.95.0`
- Required routed Rust toolchain: `1.95.0`
- Named duplicate MSRV lane: manual or `full-ci`
- Rust 1.95 compatibility audit: pass on 2026-05-09; see
  [Rust 1.95 compatibility audit](msrv-1.95-audit.md).
- Rust 1.95 consistency audit: pass on 2026-05-10; see
  [Rust 1.95 consistency audit](rust-1.95-consistency-audit.md).
