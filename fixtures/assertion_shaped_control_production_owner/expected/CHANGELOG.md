# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0133: assertion-shaped owner guidance fixtures (new fixture)

Command:
`cargo xtask goldens bless assertion_shaped_control_production_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless assertion_shaped_control_production_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
