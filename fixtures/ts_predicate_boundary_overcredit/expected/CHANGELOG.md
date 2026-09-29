# Golden Output Changes

## Pending — ts_predicate_boundary_overcredit (1)

Reason:
RIPR-SPEC-0122 (#4216): TS/JS preview safe next action is terminal for a closed repair packet (quotes the validator's why_not_actionable) and says no repair for an exposed finding

Command:
`cargo xtask goldens bless ts_predicate_boundary_overcredit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_predicate_boundary_overcredit (2)

Reason:
RIPR-SPEC-0122 (#4216 review): closed-packet TS/JS safe action bounds the quoted reason, drops the causal 'so', and asks unknown-class findings for a manual check

Command:
`cargo xtask goldens bless ts_predicate_boundary_overcredit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_predicate_boundary_overcredit (3)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first
RIPR-SPEC-0087: additive typescript_boundary_input evidence line for a read-only parameter boundary; packet readiness unchanged

Command:
`cargo xtask goldens bless ts_predicate_boundary_overcredit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — ts_predicate_boundary_overcredit (4)

Reason:
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy; stop reasons carry a gloss; boxed-wrapper limitation text has no whitespace runs (#4323)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless ts_predicate_boundary_overcredit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
