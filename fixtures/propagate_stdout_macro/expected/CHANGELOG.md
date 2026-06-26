# Golden Output Changes

## Pending

Reason:
initial golden for fix C: println macro → propagation_unknown

Command:
`cargo xtask goldens bless propagate_stdout_macro --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: honest no-static-path messaging (RIPR-SPEC-0113)

Command:
`cargo xtask goldens bless propagate_stdout_macro --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Honesty (dogfood anyhow Chain::len): suppress the all-no-static-path note when a finding has reach=yes — a reaching test IS a static test path; the note must not contradict the finding's own reach evidence (RIPR-SPEC-0090)

Command:
`cargo xtask goldens bless propagate_stdout_macro --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
