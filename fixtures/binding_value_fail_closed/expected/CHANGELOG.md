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
