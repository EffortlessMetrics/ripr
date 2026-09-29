# Golden Output Changes

## Pending

Reason:
new fixture for RANK-1 fix: removed-side probes for fn two now correctly land at line 8 (new-file coordinate), not line 6 (old-side coordinate that maps to fn one territory after +2 net delta from hunk 1)

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: honest no-static-path messaging (RIPR-SPEC-0113)

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: all-no-path scope counts

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
changed semantic heads use canonical parser expressions while removed-side targeting stays stable

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0001: wrap long human advisory prose at a fixed 100-column boundary for #2784

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0023: classification hint added to digest (#2614)

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0151: additive per-finding source_currentness disposition (#3280); removed-only Rust probes record the base-side line coordinate

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0151: candidate-line count now counts candidate-current anchors only (base-side coordinates excluded)

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0151: producer slice keeps the recorded coordinate unchanged; the fixture returns to its pre-slice values plus the additive source_currentness field

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0152: preview findings resolve candidate_current from head-side delta evidence; base-deleted findings gain the human revision label and leave actionable projections

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0152: hidden-count denominator keeps non-current findings; mutation candidates and review-comment authority require candidate_current

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — multi_hunk_removed_line_wrong_target (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — multi_hunk_removed_line_wrong_target (3)

Reason:
RIPR-SPEC-0082/RIPR-SPEC-0122 wording owner change (PR #3978): Why lines re-derived from reach/observe stage state, preview notes use language display names with singular file counts, recovery detail lines end with exactly one period; mechanical re-render of unchanged fixture evidence

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — multi_hunk_removed_line_wrong_target (4)

Reason:
RIPR-SPEC-0001: a changed line inside a function no test reaches is no_static_path whatever its probe shape; static_unknown escalate-to-mutation advice no longer stands in for a missing test
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — multi_hunk_removed_line_wrong_target (5)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — multi_hunk_removed_line_wrong_target (6)

Reason:
RIPR-SPEC-0122: unreached static_unknown next step hedges macro and integration reach

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — multi_hunk_removed_line_wrong_target (7)

Reason:
RIPR-SPEC-0094: re-bless after merging main (#4425 observed-value caps); unreached-owner findings read no_static_path and new-function signature lines are not probed (#4428)

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — multi_hunk_removed_line_wrong_target (8)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless multi_hunk_removed_line_wrong_target --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
