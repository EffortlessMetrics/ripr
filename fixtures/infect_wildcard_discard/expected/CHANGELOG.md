# Golden Output Changes

## Pending

Reason:
initial golden for fix A: wildcard-discard → infection_unknown

Command:
`cargo xtask goldens bless infect_wildcard_discard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
re-bless after merging origin/main (#1216): confidence/discriminate-message refresh; classification infection_unknown unchanged

Command:
`cargo xtask goldens bless infect_wildcard_discard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0109: cap headline confidence by weakest stage's per-stage Confidence (#1219 part D)

Command:
`cargo xtask goldens bless infect_wildcard_discard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: honest no-static-path messaging (RIPR-SPEC-0113)

Command:
`cargo xtask goldens bless infect_wildcard_discard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
