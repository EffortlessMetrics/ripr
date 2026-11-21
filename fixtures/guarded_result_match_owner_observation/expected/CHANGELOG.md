# Golden Output Changes

## Pending — guarded_result_match_owner_observation (1)

Reason:
RIPR-SPEC-0174: new fixture for the guarded Result match producer-owned observation (reduced #13162 expect_response shape); the producer-owned seam credits the guarded_result_match oracle and the propagation-complete probe reads exposed (#3709)

Command:
`cargo xtask goldens bless guarded_result_match_owner_observation --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
