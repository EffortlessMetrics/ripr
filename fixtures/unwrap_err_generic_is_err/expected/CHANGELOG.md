# Golden Output Changes

## Pending — unwrap_err_generic_is_err (1)

Reason:
RIPR-SPEC-0106: initial golden for generic is_err stays weakly_exposed

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_generic_is_err (2)

Reason:
RIPR-SPEC-0107: error_path requires a variant-observing oracle; sibling/broad oracle no longer promotes exposed. The generic assert!(err.to_string().contains("error")) oracle has no variant token matching CalcError::Negative, so discriminate message correctly changes to observation_unverified. Classification stays weakly_exposed.

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_generic_is_err (3)

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_generic_is_err (4)

Reason:
restrict CallDeletion probes to standalone call statements; refresh affected goldens and record intentional output changes

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_generic_is_err (5)

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_generic_is_err (6)

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_generic_is_err (7)

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_generic_is_err (8)

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_generic_is_err (9)

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_generic_is_err (10)

Reason:
RIPR-SPEC-0023: classification hint added to digest (#2614)

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_generic_is_err (11)

Reason:
RIPR-SPEC-0122: render the missing-discriminator value without restating the label

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_generic_is_err (12)

Reason:
RIPR-SPEC-0151: rebless check JSON for the additive source_currentness field; classifications, stages, confidence, counts, and recorded coordinates remain unchanged.

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`

## Pending — unwrap_err_generic_is_err (13)

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_generic_is_err (14)

Reason:
RIPR-SPEC-0001: #3161 PR-B production witness gate upgrades complete direct-flow propagate evidence and fails incomplete flows closed

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_generic_is_err (15)

Reason:
RIPR-SPEC-0106: Err-construction return-value guidance names the missing reaching input and exact error variant instead of the false broad-assertion claim (PR review XQFi)

Command:
`cargo xtask goldens bless unwrap_err_generic_is_err --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
