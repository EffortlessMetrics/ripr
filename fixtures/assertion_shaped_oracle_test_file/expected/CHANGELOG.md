# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0133: assertion-shaped owner reframes guidance for oracles (new fixture)

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
PR #2170 review: oracle helper input made internally consistent (asserts absence of foreign separator, exact-equality operand avoids phantom Some() return sink); class and reframed guidance unchanged

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0023: classification hint added to digest (#2614)

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0151: rebless check JSON for the additive source_currentness field; classifications, stages, confidence, counts, and recorded coordinates remain unchanged.

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — assertion_shaped_oracle_test_file (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — assertion_shaped_oracle_test_file (3)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — assertion_shaped_oracle_test_file (4)

Reason:
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy; stop reasons carry a gloss; boxed-wrapper limitation text has no whitespace runs (#4323)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless assertion_shaped_oracle_test_file --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
