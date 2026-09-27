# Golden Output Changes

## Pending — typescript_witness_dead_argument (1)

Reason:
RIPR-SPEC-0027 boundary witness guard #4102: literal in argument the single-param owner never reads no longer witnesses (was exposed, now weakly_exposed)

Command:
`cargo xtask goldens bless typescript_witness_dead_argument --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_witness_dead_argument (2)

Reason:
RIPR-SPEC-0122 (#4216): TS/JS preview safe next action is terminal for a closed repair packet (quotes the validator's why_not_actionable) and says no repair for an exposed finding

Command:
`cargo xtask goldens bless typescript_witness_dead_argument --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_witness_dead_argument (3)

Reason:
RIPR-SPEC-0122 (#4216 review): closed-packet TS/JS safe action bounds the quoted reason, drops the causal 'so', and asks unknown-class findings for a manual check

Command:
`cargo xtask goldens bless typescript_witness_dead_argument --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
