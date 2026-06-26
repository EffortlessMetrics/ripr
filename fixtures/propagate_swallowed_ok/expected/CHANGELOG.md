# Golden Output Changes

## Pending

Reason:
initial golden for fix B: swallowed ok tail → propagation_unknown

Command:
`cargo xtask goldens bless propagate_swallowed_ok --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
re-bless after merging origin/main (#1216): confidence 0.87->0.79 refresh; classification propagation_unknown unchanged

Command:
`cargo xtask goldens bless propagate_swallowed_ok --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0109: cap headline confidence by weakest stage's per-stage Confidence (#1219 part D)

Command:
`cargo xtask goldens bless propagate_swallowed_ok --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: honest no-static-path messaging (RIPR-SPEC-0113)

Command:
`cargo xtask goldens bless propagate_swallowed_ok --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Honesty (dogfood anyhow Chain::len): suppress the all-no-static-path note when a finding has reach=yes — a reaching test IS a static test path; the note must not contradict the finding's own reach evidence (RIPR-SPEC-0090)

Command:
`cargo xtask goldens bless propagate_swallowed_ok --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
