# Golden Output Changes

## Pending — typescript_witness_dead_expected (1)

Reason:
RIPR-SPEC-0027 boundary witness guard #4102: this shape must not witness the changed boundary (issue #4102); goldens capture the fail-closed classification

Command:
`cargo xtask goldens bless typescript_witness_dead_expected --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_witness_dead_expected (2)

Reason:
RIPR-SPEC-0122 (#4216): TS/JS preview safe next action is terminal for a closed repair packet (quotes the validator's why_not_actionable) and says no repair for an exposed finding

Command:
`cargo xtask goldens bless typescript_witness_dead_expected --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_witness_dead_expected (3)

Reason:
RIPR-SPEC-0122 (#4216 review): closed-packet TS/JS safe action bounds the quoted reason, drops the causal 'so', and asks unknown-class findings for a manual check

Command:
`cargo xtask goldens bless typescript_witness_dead_expected --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_witness_dead_expected (4)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries
RIPR-SPEC-0087: additive typescript_boundary_input evidence line for a read-only parameter boundary; packet readiness unchanged

Command:
`cargo xtask goldens bless typescript_witness_dead_expected --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
