# Golden Output Changes

## Pending — guarded_result_match_conditional_failure (1)

Reason:
RIPR-SPEC-0175: #3731 review fix F4 — bounded depth-0 terminal grammar; new fixture pins that conditional panics, unrelated unwraps, and closure-nested markers never credit (no_static_path)

Command:
`cargo xtask goldens bless guarded_result_match_conditional_failure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
