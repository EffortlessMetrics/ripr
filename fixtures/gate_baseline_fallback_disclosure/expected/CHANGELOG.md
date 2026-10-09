# Golden Output Changes

## Pending

Reason:
PR review round 1: the gate `subject` block now covers every CLI-supplied
decision-changing input, so the baseline scenario goldens gain a `baseline`
content-hash entry beside `pr_guidance`. Decisions, warnings, and Markdown
are unchanged; `analyzer_version` stays pinned.

Command:
`RIPR_UPDATE_FIXTURES=1 cargo test -p ripr --lib -- baseline_fallback_disclosure_fixture_matrix_matches_checked_outputs`

Updated:
- `expected/gate-baseline/*/gate-decision.json` (5 scenarios)

# Golden Output Changes

## Pending

Reason:
#5263: gate-decision.json now records an additive `subject` identity block
(build `analyzer_version`, per-input sha256 content hashes, and the copied
producer receipt). Decisions, warnings, `baseline_match_kind` disclosure,
status and Markdown are unchanged; `analyzer_version` is pinned to a fixed
placeholder in goldens with a shape check on the renderer's own stamp.

Command:
`RIPR_UPDATE_FIXTURES=1 cargo test -p ripr --lib -- baseline_fallback_disclosure_fixture_matrix_matches_checked_outputs`

Updated:
- `expected/gate-baseline/*/gate-decision.json` (5 scenarios)

# Golden Output Changes

## Pending

Reason:
#3906 (F60-14, F60-2(c)): gate-decision Markdown leads a carried repair start with the after-phase step and labels verify and receipt as the manual alternative that names its prerequisites. JSON is unchanged.

Command:
`RIPR_UPDATE_FIXTURES=1 cargo test -p ripr --lib -- baseline_fallback_disclosure_fixture_matrix_matches_checked_outputs`

Updated:
- `expected/gate-baseline/*/gate-decision.md`

## Pending

Reason:
new fixture for issue #1934 baseline fallback disclosure corpus; comment-only diff keeps the check golden minimal

Command:
`cargo xtask goldens bless gate_baseline_fallback_disclosure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless gate_baseline_fallback_disclosure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless gate_baseline_fallback_disclosure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless gate_baseline_fallback_disclosure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — gate_baseline_fallback_disclosure (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless gate_baseline_fallback_disclosure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — gate_baseline_fallback_disclosure (3)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless gate_baseline_fallback_disclosure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — gate_baseline_fallback_disclosure (4)

Reason:
RIPR-SPEC-0122: #4322 summary header names all seven classes with canonical tokens and a shown/total denominator

Command:
`cargo xtask goldens bless gate_baseline_fallback_disclosure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
