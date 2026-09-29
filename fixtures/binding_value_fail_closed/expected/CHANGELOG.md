# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0158: new fixture pinning the bounded value-transfer behavior (family matrix observes exact boundaries; unsupported chains fail closed by name)

Command:
`cargo xtask goldens bless binding_value_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0158: review round 2 — operand resolution hoisted per probe (provenance text unchanged in content but regenerated), the starts_with hunk made a real behavior change, and quote-aware splitting/char escapes/dedup refinements

Command:
`cargo xtask goldens bless binding_value_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0158: scoped re-bless for this fixture only - exact operands compare canonical renderings directly and the literal-case provenance renders explicitly

Command:
`cargo xtask goldens bless binding_value_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless binding_value_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — binding_value_fail_closed (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless binding_value_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — binding_value_fail_closed (3)

Reason:
RIPR-SPEC-0122: discriminator evidence line no longer says yes on findings that are not exposed

Command:
`cargo xtask goldens bless binding_value_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — binding_value_fail_closed (4)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless binding_value_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — binding_value_fail_closed (5)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless binding_value_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
