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

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
restrict CallDeletion probes to standalone call statements; refresh affected goldens and record intentional output changes

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
