# Handoff: Campaign 7 Closeout

Date: 2026-05-07
Branch / PR: `defaults-first-closeout` / TBD
Latest merged PR: #417 `docs: verify release install paths` (commit `3eca827`)

## Current Work Item

`campaign/defaults-first-closeout`

Campaign 7 made the defaults-first operator path coherent: install RIPR, run a
pilot or explicit fixture scan, inspect the weak seam, copy a targeted-test
brief, rerun static evidence, and read a receipt. The closeout boundary is
docs/manifest only; no analyzer, output schema, LSP, SARIF, badge, VS Code, or
public API behavior changes are part of this closeout.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Built-in defaults and generated `ripr.toml` are conservative | #410 and #411; `docs/CONFIGURATION.md`; `ripr.toml.example`; `crates/ripr/src/config.rs::tests::generated_init_config_matches_builtin_defaults`; `crates/ripr/tests/cli_smoke.rs::doctor_reports_missing_config_defaults`. |
| Fast, normal, and deep mode behavior is clear | #411; `docs/CONFIGURATION.md` mode table; `crates/ripr/src/analysis/workspace/select.rs::tests::operator_mode_tiers_are_pinned_for_defaults_first_adoption`. |
| One operator cockpit joins existing surfaces | #412; `xtask/src/reports/operator.rs`; `docs/OUTPUT_SCHEMA.md` Operator Cockpit Report; `cargo xtask operator-cockpit-report`. |
| GitHub Action uploads useful artifacts and keeps blocking policy opt-in | #413 and #414; `docs/CI.md`; `crates/ripr/src/cli/commands.rs::tests::init_generated_github_workflow_uploads_reports_and_makes_sarif_optional`. |
| Editor install path and existing commands are documented without new LSP features | #409 and #415; `docs/EDITOR_EXTENSION.md`; `docs/SERVER_PROVISIONING.md`; `cargo xtask lsp-cockpit-report`; VS Code CI passed on #417. |
| Public example corpus demonstrates weak seams, targeted briefs, receipts, and optional calibration | #416; `fixtures/EXAMPLE_CORPUS.md`; `fixtures/boundary_gap/calibration/*`; `fixtures/opaque_fixture_builder`; `cargo xtask fixtures`; `cargo xtask goldens check`; `cargo xtask check-fixture-contracts`. |
| Install and release paths are verified enough for a new user to run the loop | #417; `docs/RELEASE.md`; `docs/RELEASE_BINARIES.md`; `docs/SERVER_PROVISIONING.md`; local `cargo install` smoke; public `v0.3.0` release asset and checksum smoke; `npm --prefix editors/vscode run package`. |
| Clean install-to-targeted-test loop is demonstrated | Installed `target/ripr/install-smoke/bin/ripr.exe` ran the boundary-gap agent seam packet, `ripr outcome`, and `ripr calibrate cargo-mutants` commands against `fixtures/boundary_gap`. The receipt showed the new observed value `100`; calibration joined by `seam_id`. |
| Static vocabulary remains conservative | `cargo xtask check-static-language`; static reports use exposure/test-grip vocabulary, while runtime vocabulary stays in explicit calibration reports. |
| Manifest points at the next real lane | `.ripr/goals/active.toml` now points at Campaign 8 `runtime-calibration-fixtures`, first ready item `calibration/runtime-fixtures-v1`. |

## PR Chain

- #409 `vscode: default editor analysis to draft`
- #410 `campaign: pin defaults config baseline`
- #411 `test: pin defaults mode and repo filters`
- #412 `campaign: add operator cockpit report`
- #413 `ci: add defaults-first GitHub Action entrypoint`
- #414 `ci: gate generated SARIF rendering`
- #415 `vscode: document and verify install polish`
- #416 `fixtures: add defaults-first example corpus`
- #417 `docs: verify release install paths`

## Verification Run

Closeout proof before opening this PR:

```bash
target/ripr/install-smoke/bin/ripr.exe check --root fixtures/boundary_gap/input --diff fixtures/boundary_gap/diff.patch --mode ready --format agent-seam-packets-json
target/ripr/install-smoke/bin/ripr.exe outcome --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
target/ripr/install-smoke/bin/ripr.exe calibrate cargo-mutants --mutants-json fixtures/boundary_gap/calibration/runtime-mutants.json --repo-exposure-json fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask check-traceability
cargo xtask check-capabilities
cargo test --workspace
cargo xtask check-pr
```

## Next Work Item

`calibration/runtime-fixtures-v1`

Add controlled calibration fixtures for the main static/runtime agreement
buckets. Keep runtime results as supplied input artifacts. Do not add mutation
execution to RIPR.

## Open Decisions

- Runtime calibration fixture breadth: start with the smallest set that covers
  the agreement buckets already named by `mutation-calibration.{json,md}`.

## What Not To Do

- Do not add unsaved-buffer overlays, CodeLens, inlay hints, semantic tokens, or
  other speculative LSP features as part of calibration fixture work.
- Do not split `ripr` into multiple public crates.
- Do not make RIPR run cargo-mutants. Calibration imports supplied runtime data
  only.
