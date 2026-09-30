# Golden Output Changes

## Pending — constant_declaration_probe_reconciliation (1)

Reason:
RIPR-SPEC-0046: #3719 reconciliation — constant declarations classify static_unknown (no call_deletion/field_construction) and the canonical alignment records the config_or_policy_constant limitation with unknown flow

Command:
`cargo xtask goldens bless constant_declaration_probe_reconciliation --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — constant_declaration_probe_reconciliation (2)

Reason:
RIPR-SPEC-0046: fixture observer moved to cfg-test in src; constant classification unchanged (static_unknown only)

Command:
`cargo xtask goldens bless constant_declaration_probe_reconciliation --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — constant_declaration_probe_reconciliation (3)

Reason:
RIPR-SPEC-0046: add initializer-variant constants for declaration-gate coverage

Command:
`cargo xtask goldens bless constant_declaration_probe_reconciliation --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — constant_declaration_probe_reconciliation (4)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless constant_declaration_probe_reconciliation --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — constant_declaration_probe_reconciliation (5)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless constant_declaration_probe_reconciliation --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — constant_declaration_probe_reconciliation (6)

Reason:
RIPR-SPEC-0122: unreached static_unknown next step hedges macro and integration reach

Command:
`cargo xtask goldens bless constant_declaration_probe_reconciliation --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — constant_declaration_probe_reconciliation (7)

Reason:
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy; stop reasons carry a gloss; boxed-wrapper limitation text has no whitespace runs (#4323)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless constant_declaration_probe_reconciliation --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — constant_declaration_probe_reconciliation (8)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless constant_declaration_probe_reconciliation --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — constant_declaration_probe_reconciliation (9)

Reason:
RIPR-SPEC-0122: #4321 additive per-finding id lines in human-full (drill-in identifiers)

Command:
`cargo xtask goldens bless constant_declaration_probe_reconciliation --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
## Pending — constant_declaration_probe_reconciliation (10)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless constant_declaration_probe_reconciliation --reason "..."`

Updated:
- `expected/human.txt`