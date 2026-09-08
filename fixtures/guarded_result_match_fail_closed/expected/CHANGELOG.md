# Golden Output Changes

## Pending — guarded_result_match_fail_closed (1)

Reason:
RIPR-SPEC-0174: new fail-closed battery fixture; wrong-owner, variable-bound, shadowed, message-only, and swallowed Err guards never emit the guarded_result_match oracle and every finding stays below exposed (#3709)

Command:
`cargo xtask goldens bless guarded_result_match_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
