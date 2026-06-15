# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0106: initial golden for generic is_err stays weakly_exposed

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0107: error_path requires a variant-observing oracle; sibling/broad oracle no longer promotes exposed. The generic assert!(err.to_string().contains("error")) oracle has no variant token matching CalcError::Negative, so discriminate message correctly changes to observation_unverified. Classification stays weakly_exposed.

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
