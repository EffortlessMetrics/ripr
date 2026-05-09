# Handoff: Campaign 22 Closeout

Date: 2026-05-09
Branch / PR: `campaign-first-useful-action-closeout` / pending at authoring
Latest merged PR: #644 `lsp: harden first useful action status projection` (commit `c38d969`)

## Current Work Item

`campaign/first-useful-action-closeout`

Campaign 22 compresses existing RIPR evidence into one advisory first useful
action for developers, reviewers, and coding agents:

```text
editor, PR, CI, receipt, baseline, ledger, assistant-proof, optional gate,
coverage/grip, and staleness inputs
-> deterministic read-only first-action routing
-> first-useful-action.{json,md}
-> advisory CI summary and artifact projection
-> optional editor status projection
-> verify command and receipt path
```

The campaign did not change analyzer behavior, recommendation ranking, gate
policy semantics, generated workflow pass/fail authority, source-edit behavior,
generated-test behavior, provider calls, mutation execution, public crate
shape, release posture, or security posture.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Campaign opened explicitly | Campaign 22 is recorded as `first-useful-action` in `.ripr/goals/active.toml` and `docs/IMPLEMENTATION_CAMPAIGNS.md`, with work items for spec, fixtures, report producer, CI projection, editor projection, workflow docs, dogfood receipts, and closeout. |
| Report contract exists | `docs/specs/RIPR-SPEC-0020-first-useful-action-report.md` defines the read-only report contract, bounded status and action vocabularies, inputs, routing priorities, JSON/Markdown shape, advisory limits, and non-goals. |
| Output schema is documented | `docs/OUTPUT_SCHEMA.md` documents `first-useful-action.{json,md}`, status vocabulary, action vocabulary, command fields, evidence fields, warnings, fallback states, generated-CI behavior, editor projection, and dogfood receipt shape. |
| Deterministic routing fixtures exist | `fixtures/boundary_gap/expected/first-useful-action/` pins actionable, stale, missing-required-artifact, baseline-only, acknowledged, waived, suppressed, no-actionable-seam, already-improved, and unchanged-after-attempt JSON/Markdown outputs. |
| Report producer exists | `ripr first-action` and `crates/ripr/src/output/first_useful_action.rs` build JSON and Markdown reports from explicit existing artifact paths. The producer requires at least one explicit input and does not rerun hidden analysis. |
| Producer tests pin routing | `cargo test -p ripr first_useful_action` covers fixture-matched actionable and unchanged-after-attempt outputs plus stale editor context and missing assistant-proof routes. |
| Public CLI writes reports | `crates/ripr/tests/cli_smoke.rs::first_action_cli_writes_actionable_report` validates the public command writes first-action JSON and Markdown. |
| Generated CI projection exists | Generated GitHub workflows run `ripr first-action` only when explicit upstream report inputs are present, upload `first-useful-action.{json,md}`, and append a First Useful Action summary when the report exists. |
| Generated CI remains advisory | The generated workflow preserves non-blocking report posture and leaves gate decisions as the explicit pass/fail authority. |
| Editor projection exists | `editors/vscode/src/client.ts` reads an existing workspace-matched `target/ripr/reports/first-useful-action.json` and projects it through status and `ripr: Show Status`. |
| Editor projection stays projection-only | VS Code tests cover existing-report projection, workspace-root mismatch fail-closed behavior, and stale saved-workspace status visibility without shelling out to `ripr first-action`, adding diagnostics, or adding editor decorations. |
| User workflow docs exist | `docs/FIRST_USEFUL_ACTION_WORKFLOW.md` explains GitHub and editor entry points, status meanings, developer/reviewer/agent actions, verify and receipt flow, fallback states, advisory CI, and gate boundaries. |
| Dogfood receipts exist | `cargo xtask dogfood` checks first-action receipts for actionable, baseline-only, stale, missing-required-artifact, unchanged-after-attempt, and no-actionable-seam cases; `docs/handoffs/2026-05-09-first-useful-action-receipts.md` records the cases and boundaries. |
| Capability and traceability records are current | `docs/CAPABILITY_MATRIX.md`, `metrics/capabilities.toml`, and `.ripr/traceability.toml` link RIPR-SPEC-0020 to the spec, fixtures, code, tests, docs, dogfood receipts, and closeout handoff. |
| Static evidence vocabulary boundaries are preserved | Spec, schema docs, workflow docs, fixture README, dogfood receipt, and closeout state that first-action routing is advisory static evidence and does not prove runtime adequacy or run mutation testing. |

## PR Chain

- #633 `campaign: open first useful action`
- #634 `spec: define first useful action report`
- #635 `fixtures: add first useful action corpus`
- #636 `report: add first useful action`
- #638 `ci: surface first useful action`
- #643 `lsp: project first useful action in status`
- #644 `lsp: harden first useful action status projection`
- #645 `docs: document first useful action workflow`
- #647 `dogfood: add first useful action receipts`
- `campaign/first-useful-action-closeout`

## Verification Run

Closeout proof before opening this PR:

```bash
cargo test -p ripr first_useful_action
cargo test -p ripr init_generated_github_workflow
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask dogfood
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Next Work Item

No ready work item remains in `.ripr/goals/active.toml` from Campaign 22 after
this closeout.

Choose the next campaign explicitly before opening another product lane.
Likely future work should be separated by lane:

- Assistant Loop Health, if the next product risk is measuring proof packet
  completeness, missing inputs, repeated warnings, and stuck receipt states;
- Evidence Spine Stabilization, if the next product risk is making every
  downstream surface consume one typed evidence record and movement contract;
- editor/LSP hardening, if the next product risk is status, command payload, or
  hover reliability in real workspaces;
- multi-repo adoption or installed-user polish, if the next product risk is
  trust and habit formation outside the RIPR repo.

Those should not be folded into Campaign 22 closeout.

## What Not To Do

- Do not make generated workflows blocking by default.
- Do not make first-action reports the gate authority.
- Do not claim runtime mutation outcomes from static evidence.
- Do not run cargo-mutants or any mutation engine from first-action workflows.
- Do not rerun hidden analysis or discover artifacts implicitly in the report
  producer.
- Do not move analyzer identity, recommendation ranking, gate policy semantics,
  or editor behavior into closeout work.
- Do not generate tests, edit source, post inline comments, or call LLM
  providers from the first-action report by default.
