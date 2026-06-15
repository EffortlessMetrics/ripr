# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0107: initial golden for error_path fake-clean repro fixture; error_path with only sibling ExactValue oracle correctly reports weakly_exposed via observation_unverified

Command:
`cargo xtask goldens bless error_path_sibling_oracle_fake_clean --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
