# Golden Output Changes

## Pending — split_test_boundary_oracle (1)

Reason:
RIPR-SPEC-0186: split-test boundary input and far oracle must not read exposed; names same_test_pairing_missing

Command:
`cargo xtask goldens bless split_test_boundary_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — split_test_boundary_oracle (2)

Reason:
RIPR-SPEC-0186: refresh human analysis-outcome line to findings-below wording from #4777; pairing class unchanged

Command:
`cargo xtask goldens bless split_test_boundary_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — split_test_boundary_oracle (3)

Reason:
RIPR-SPEC-0122: bounded human surfaces disclose their windows - related-test and observed-value caps, digest missing-discriminator and related-test totals, Hidden block names omitted findings by file:line (class) with the all-base-side distinction (#4320); RIPR-SPEC-0152: all-base-side runs name base-side evidence instead of a lower-priority framing

Command:
`cargo xtask goldens bless split_test_boundary_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — split_test_boundary_oracle (4)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless split_test_boundary_oracle --reason "..."`

Updated:
- `expected/human.txt`
