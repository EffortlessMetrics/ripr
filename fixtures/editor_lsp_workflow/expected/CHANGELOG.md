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

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
add relation_reason and relation_confidence fields to related_test JSON output

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
changed semantic heads use canonical parser expressions and content-addressed probe identities

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
