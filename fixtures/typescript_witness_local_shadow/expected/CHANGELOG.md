# Golden Output Changes

## Pending — typescript_witness_local_shadow (1)

Reason:
RIPR-SPEC-0027 boundary witness guard #4102: this shape must not witness the changed boundary (issue #4102); goldens capture the fail-closed classification

Command:
`cargo xtask goldens bless typescript_witness_local_shadow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_witness_local_shadow (2)

Reason:
RIPR-SPEC-0122 (#4216): TS/JS preview safe next action is terminal for a closed repair packet (quotes the validator's why_not_actionable) and says no repair for an exposed finding

Command:
`cargo xtask goldens bless typescript_witness_local_shadow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_witness_local_shadow (3)

Reason:
RIPR-SPEC-0122 (#4216 review): closed-packet TS/JS safe action bounds the quoted reason, drops the causal 'so', and asks unknown-class findings for a manual check

Command:
`cargo xtask goldens bless typescript_witness_local_shadow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_witness_local_shadow (4)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless typescript_witness_local_shadow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_witness_local_shadow (5)

Reason:
RIPR-SPEC-0046 RIPR-SPEC-0047: check JSON now carries a top-level source_subject stamp with the analysis-time content digests of the files a derived gap ledger names (#4544); no finding, classification, or human output changed.
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy; stop reasons carry a gloss; boxed-wrapper limitation text has no whitespace runs (#4323)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless typescript_witness_local_shadow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_witness_local_shadow (6)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless typescript_witness_local_shadow --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — typescript_witness_local_shadow (7)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless typescript_witness_local_shadow --reason "..."`

Updated:
- `expected/human.txt`
