# Golden Output Changes

## Pending — tail_comparison_boundary (1)

Reason:
RIPR-SPEC-0001: new fixture pinning that a comparison which is the whole tail of a -> bool owner propagates to the returned value, so a wrapper-reached threshold names its missing equality boundary (items == 10) and an on-boundary test is exposed

Command:
`cargo xtask goldens bless tail_comparison_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — tail_comparison_boundary (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (#4002); this fixture's golden was blessed on a base before that change, so --diff envelopes now omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed.

Command:
`cargo xtask goldens bless tail_comparison_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — tail_comparison_boundary (3)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless tail_comparison_boundary --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
