# Handoff: Campaign 8 Closeout

Date: 2026-05-07
Branch / PR: `runtime-calibration-closeout`
Latest merged PR: #420 `fixtures: add runtime calibration agreement sample`

## Current Work Item

`campaign/runtime-calibration-closeout`

Campaign 8 expanded the calibration fixture lane with supplied runtime data.
The closeout boundary is docs/manifest only: no analyzer, output schema, LSP,
SARIF, badge, VS Code, calibration importer, or public API behavior changes are
part of this closeout.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Calibration fixtures cover static gaps with runtime signals and static gaps without runtime signals | #420; `fixtures/boundary_gap/calibration/runtime-fixtures-v1/repo-exposure.json`; checked `mutation-calibration.{json,md}` reports. |
| Calibration fixtures cover runtime signals without static gaps, ambiguous file/line joins, unmatched runtime data, and static seams without runtime data | #420; `crates/ripr/tests/cli_smoke.rs::calibration_runtime_fixture_matches_checked_reports` asserts the agreement, ambiguity, unmatched, static-only, and join-method buckets. |
| Runtime artifacts are supplied inputs or generated outputs | #420 stores supplied `runtime-mutants.json` plus checked calibration output; no command runs cargo-mutants. |
| Operator and docs keep calibration optional | `fixtures/CALIBRATION_CORPUS.md`, README capability snapshots, and `docs/specs/RIPR-SPEC-0006-mutation-calibration.md` frame calibration as imported runtime context. |
| Static vocabulary remains conservative | `cargo xtask check-static-language`; runtime vocabulary is allowlisted only for explicit calibration reports. |
| Manifest points at the next real lane | `.ripr/goals/active.toml` now points at Campaign 9 `hot-sidecar-latency`, first ready item `cache/current-latency-audit`. |

## PR Chain

- #420 `fixtures: add runtime calibration agreement sample`
- `campaign/runtime-calibration-closeout`

## Verification Run

Closeout proof before opening this PR:

```bash
cargo test -p ripr calibration
cargo xtask mutation-calibration fixtures/boundary_gap/input --mutants-json fixtures/boundary_gap/calibration/runtime-fixtures-v1/runtime-mutants.json --repo-exposure-json fixtures/boundary_gap/calibration/runtime-fixtures-v1/repo-exposure.json
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-fixture-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo test --workspace
cargo xtask check-pr
git diff --check
```

## Next Work Item

`cache/current-latency-audit`

Measure the current repo seam cache behavior, operator report generation, and
saved-workspace LSP refresh proof before changing cache semantics. This is an
audit/proof PR, not a cache rewrite.

## Open Decisions

- Decide which latency evidence belongs in generated reports before any
  persistent or in-memory cache changes.
- Decide whether the existing repo seam cache is sufficient for operator paths,
  and separately whether saved-workspace editor refresh needs additional
  warm-path reuse.

## What Not To Do

- Do not add new LSP features, unsaved-buffer overlays, CodeLens, inlay hints,
  or semantic tokens in the latency audit.
- Do not cache rendered JSON, Markdown, diagnostics, hover text, or agent
  packets.
- Do not make runtime calibration required for pilot, LSP, SARIF, badge, or CI
  paths.
- Do not split `ripr` into multiple public crates.
