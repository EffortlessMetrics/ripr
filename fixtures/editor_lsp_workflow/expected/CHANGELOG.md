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

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0023: classification hint added to digest (#2614)

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0122: render the missing-discriminator value without restating the label

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0151: rebless check JSON for the additive source_currentness field; classifications, stages, confidence, counts, and recorded coordinates remain unchanged.

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — editor_lsp_workflow (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — editor_lsp_workflow (3)

Reason:
#3906: the repair-eligible seam's editor projection now leads the agent loop with the repair start (`Start repair: copy repair command`, `ripr.copyAgentRepairCommand`, `ripr agent repair --root . --seam-id 67fc764ba37d77bd --phase before`) and the hover names it first; every other action and hover line is unchanged.

Command:
Hand-curated editor projection; the matching producer output is pinned by `cargo test -p ripr --lib lsp::tests::boundary_gap_lsp_code_actions_match_fixture_expectation`.

Updated:
- `expected/lsp-code-actions.json`
- `expected/lsp-hover.md`

## Pending — editor_lsp_workflow (4)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — editor_lsp_workflow (5)

Reason:
RIPR-SPEC-0122: discriminator evidence line no longer says yes on findings that are not exposed

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — editor_lsp_workflow (6)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — editor_lsp_workflow (7)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — editor_lsp_workflow (8)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless editor_lsp_workflow --reason "..."`

Updated:
- `expected/human.txt`
