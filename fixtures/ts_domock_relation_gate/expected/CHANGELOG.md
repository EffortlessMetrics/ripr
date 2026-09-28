# Golden Output Changes

## Pending — ts_domock_relation_gate (1)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless ts_domock_relation_gate --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
