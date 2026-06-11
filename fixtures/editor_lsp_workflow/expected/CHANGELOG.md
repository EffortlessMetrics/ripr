# Fixture Changelog: editor_lsp_workflow

## 2026-05-10

- Added the canonical Lane 3 editor/LSP workflow fixture. It reuses the
  boundary-gap behavior while pinning the saved-workspace editor projection:
  diagnostics, hover, code actions, first-useful-action status, stale refresh
  guidance, and static-only limits.

## Pending

Reason:
RIPR-SPEC-0026 output(language): RustAdapter tags each Finding with language=rust; check.json gains the additive optional language field

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
audit LSP code-action titles: seam->test gap, analysis->Refresh Analysis

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/lsp-code-actions.json`

## Pending

Reason:
schema 0.2: dedup assertion text into finding-level assertion_texts map (#1035)

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
content-addressed-probe-ids-#1053

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
platform-stable content-addressed ids (#1053): normalize owner path separators in fp8

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
