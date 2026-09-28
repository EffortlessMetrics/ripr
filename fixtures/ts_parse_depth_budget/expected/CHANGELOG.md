# Golden Output Changes

## Pending — ts_parse_depth_budget (1)

Reason:
RIPR-SPEC-0177: new fixture for the TypeScript parse-depth budget (#4101); initial golden for the typed expression_nesting_budget refusal and surviving classification

Command:
`cargo xtask goldens bless ts_parse_depth_budget --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_parse_depth_budget (2)

Reason:
RIPR-SPEC-0122 (#4216): TS/JS preview safe next action is terminal for a closed repair packet (quotes the validator's why_not_actionable) and says no repair for an exposed finding

Command:
`cargo xtask goldens bless ts_parse_depth_budget --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_parse_depth_budget (3)

Reason:
RIPR-SPEC-0122 (#4216 review): closed-packet TS/JS safe action bounds the quoted reason, drops the causal 'so', and asks unknown-class findings for a manual check

Command:
`cargo xtask goldens bless ts_parse_depth_budget --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_parse_depth_budget (4)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless ts_parse_depth_budget --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — ts_parse_depth_budget (5)

Reason:
RIPR-SPEC-0122: partial outcome with findings no longer claims zero findings; the analyzed-scope caveat names the finding count

Command:
`cargo xtask goldens bless ts_parse_depth_budget --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
