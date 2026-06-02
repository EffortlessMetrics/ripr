# Start-Here Surface Convergence Dogfood Receipts

Date: 2026-05-22

Campaign: Start-Here Surface Convergence

Work item: `dogfood/external-style-start-here-receipts`

Branch: `dogfood-external-style-start-here-receipts`

## Scope

This receipt records external-style start-here evidence for the converged
repair path. The checked rows come from `cargo xtask first-pr` and
`cargo xtask dogfood`.

The receipts cover normal Rust shapes and fail-closed states without changing
analyzer truth, generated CI defaults, preview-language policy, source files,
generated tests, provider calls, mutation execution, branch protection, gate
authority, or merge approval.

## Checked Cases

| Case | Command / checked source | Artifacts | State labels | Receipt outcome | Known limits |
| --- | --- | --- | --- | --- | --- |
| Rust workspace missing artifacts | `cargo xtask first-pr` from the repo root | `target/ripr/reports/start-here.json`, `target/ripr/reports/start-here.md` | `missing_artifact`, output state `missing_artifacts` | safe next action is regeneration; receipt path is `not_applicable` | advisory packet only; composes explicit artifacts and does not run hidden analysis |
| Small Rust crate boundary gap | `cargo xtask dogfood` first successful PR receipts | `fixtures/first_successful_pr/boundary-gap/expected/start-here.json`, `fixtures/first_successful_pr/boundary-gap/expected/start-here.md` | `actionable`, `top_gap`, output state `actionable_gap`, receipt state `receipt_missing` | verify command is `cargo xtask fixtures boundary_gap`; row errors: none | static advisory evidence only; no runtime, coverage, mutation, gate, or merge claim |
| Rust output/golden gap | `cargo xtask dogfood` first successful PR receipts | `fixtures/first_successful_pr/output-contract-gap/expected/start-here.json`, `fixtures/first_successful_pr/output-contract-gap/expected/start-here.md` | `actionable`, `top_gap`, output state `actionable_gap`, receipt state `receipt_missing` | verify command is `cargo xtask goldens check`; row errors: none | output proof target is explicit; no source edit or generated test from RIPR |
| No-action diff | `cargo xtask dogfood` first successful PR receipts | `fixtures/first_successful_pr/empty-diff/expected/start-here.json`, `fixtures/first_successful_pr/empty-diff/expected/start-here.md` | `no_action`, `empty_diff` | no repair, verify, or receipt command is emitted; row errors: none | no-action is not a quality claim |
| Stale first-useful action | `cargo xtask dogfood` first useful action receipts | `fixtures/boundary_gap/expected/first-useful-action/stale/first-useful-action.json`, `fixtures/boundary_gap/expected/first-useful-action/stale/first-useful-action.md` | `stale`, action `refresh_evidence` | selected seam remains visible; static movement is `unknown`; row errors: none | stale artifacts require refresh before repair claims |
| Missing required artifact | `cargo xtask dogfood` first useful action receipts | `fixtures/boundary_gap/expected/first-useful-action/missing-required-artifact/first-useful-action.json`, `fixtures/boundary_gap/expected/first-useful-action/missing-required-artifact/first-useful-action.md` | `missing_required_artifact`, action `generate_missing_artifact` | no selected seam; row errors: none | missing artifacts do not imply no work is needed |
| Blocked start-here artifact | `cargo xtask dogfood` first successful PR receipts | `fixtures/first_successful_pr/blocked-ledger/expected/start-here.json`, `fixtures/first_successful_pr/blocked-ledger/expected/start-here.md` | `blocked`, `blocked_artifact` | regeneration command is preserved; row errors: none | blocked packet stays advisory and points to explicit regeneration |
| Disabled preview language | `cargo xtask dogfood` language preview and editor cockpit receipts | `target/ripr/dogfood/language-preview/python_disabled/check.json`, `fixtures/editor_gap_cockpit/disabled_language/expected/*` | `python_disabled`, `disabled_language`, language status `preview` | zero preview findings and refresh-only editor action; row errors: none | preview adapters remain opt-in; disabled preview evidence is not promoted |
| Preview-limited TypeScript evidence | `cargo xtask dogfood` language preview and editor cockpit receipts | `target/ripr/dogfood/language-preview/typescript_mocked_module_limit/check.json`, `fixtures/editor_gap_cockpit/typescript_preview_static_limit/expected/*` | `typescript`, language status `preview`, static limit `mocked_module` | preview metadata and static-limit note are present; row errors: none | preview evidence is advisory and static-limit bounded |
| Preview-limited Python evidence | `cargo xtask dogfood` language preview and editor cockpit receipts | `target/ripr/dogfood/language-preview/python_missing_import_graph_limit/check.json`, `fixtures/editor_gap_cockpit/python_preview_static_limit/expected/*` | `python`, language status `preview`, static limit `missing_import_graph` | preview metadata and static-limit note are present; row errors: none | preview evidence is advisory and static-limit bounded |
| Malformed first-pr packet | `cargo xtask dogfood` editor first-pr bridge receipts | `fixtures/editor_first_pr_bridge/packet_malformed/expected/first-pr-status.json`, `fixtures/editor_first_pr_bridge/packet_malformed/expected/lsp-code-actions.json` | `malformed`, fail closed | only regeneration guidance and refresh remain safe; row errors: none | malformed packet suppresses packet-derived repair actions |
| Missing first-pr packet | `cargo xtask dogfood` editor first-pr bridge receipts | `fixtures/editor_first_pr_bridge/packet_missing/expected/first-pr-status.json`, `fixtures/editor_first_pr_bridge/packet_missing/expected/lsp-code-actions.json` | `missing`, fail closed | only regeneration guidance and refresh remain safe; row errors: none | missing packet does not create repair authority |
| Receipt movement visible | `cargo xtask dogfood` editor first-pr bridge receipts | `fixtures/editor_first_pr_bridge/receipt_improved_packet_ready/expected/*`, `fixtures/editor_first_pr_bridge/receipt_unchanged_packet_ready/expected/*` | `top_repairable_gap`, receipt movement `improved` or `unchanged` | movement is visible next to the packet; row errors: none | receipt movement is static evidence only and not PR readiness |

## Artifact Paths

The committed evidence paths are fixture-backed:

```text
fixtures/first_successful_pr/*/expected/start-here.json
fixtures/first_successful_pr/*/expected/start-here.md
fixtures/boundary_gap/expected/first-useful-action/*/first-useful-action.json
fixtures/boundary_gap/expected/first-useful-action/*/first-useful-action.md
fixtures/editor_gap_cockpit/*/expected/*
fixtures/editor_first_pr_bridge/*/expected/*
```

The local dogfood run also writes advisory receipts to:

```text
target/ripr/reports/start-here.json
target/ripr/reports/start-here.md
target/ripr/reports/dogfood.json
target/ripr/reports/dogfood.md
target/ripr/dogfood/language-preview/<case>/check.json
target/ripr/dogfood/language-preview/<case>/human.txt
```

## Validation

```bash
cargo xtask first-pr
cargo xtask dogfood
```

Result: both commands exited successfully at authoring.

`cargo xtask dogfood` is advisory and currently renders report status `warn`
because one existing generated-CI cockpit row records an advisory regeneration
command-count warning. The start-here, first-useful-action, language preview,
editor cockpit, and editor first-pr rows listed above reported no row errors.

## Limits

- Static advisory evidence only.
- No runtime proof, coverage adequacy, mutation confirmation, gate approval, or
  merge approval.
- No source edits or generated tests from RIPR.
- No provider or model calls.
- No preview-language promotion.
- No generated CI blocking change.
- No hidden analysis rerun beyond the explicit commands.

## Next Work Item

`campaign/start-here-surface-convergence-closeout`

Close only after the campaign maps the proposal, spec, ADR, work items,
receipts, validation commands, remaining limits, and claim boundaries to the
merged artifacts.
