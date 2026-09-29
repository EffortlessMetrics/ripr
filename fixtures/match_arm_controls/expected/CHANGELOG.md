# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0164: computed-value, guard, bare-identifier, and char-scrutinee match variants stay weakly_exposed with zero hop provenance

Command:
`cargo xtask goldens bless match_arm_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — match_arm_controls (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless match_arm_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — match_arm_controls (3)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless match_arm_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — match_arm_controls (4)

Reason:
RIPR-SPEC-0122: discriminator evidence line no longer says yes on findings that are not exposed

Command:
`cargo xtask goldens bless match_arm_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — match_arm_controls (5)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless match_arm_controls --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
