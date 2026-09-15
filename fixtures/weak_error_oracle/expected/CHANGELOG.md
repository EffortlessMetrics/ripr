# Golden Output Changes

## Pending — weak_error_oracle (1)

Reason:
RIPR-SPEC-0001: baseline current weak error oracle fixture output

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (2)

Reason:
RIPR-SPEC-0001: JSON findings expose stop_reasons for every finding

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (3)

Reason:
Human output formatting: align Discriminate spacing with other RIPR evidence lines.

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (4)

Reason:
RIPR-SPEC-0001: oracle-strength-v2 distinguishes exact, broad, and smoke oracles

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (5)

Reason:
RIPR-SPEC-0001: local delta flow names Result::Err as the visible error variant sink

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (6)

Reason:
RIPR-SPEC-0001: activation modeling names missing exact error variant discriminator

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (7)

Reason:
RIPR-SPEC-0001: evidence-first output renders flow, activation, weakness, and next action

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (8)

Reason:
RIPR-SPEC-0026 output(language): RustAdapter tags each Finding with language=rust; check.json gains the additive optional language field

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (9)

Reason:
schema 0.2: dedup assertion text into finding-level assertion_texts map (#1035)

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (10)

Reason:
content-addressed-probe-ids-#1053

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (11)

Reason:
platform-stable content-addressed ids (#1053): normalize owner path separators in fp8

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (12)

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (13)

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (14)

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (15)

Reason:
add relation_reason and relation_confidence fields to related_test JSON output

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (16)

Reason:
RIPR-SPEC-0094: call_deletion and return_value probes now correctly emit observation_unverified when no assertion token references the changed expression (was: broad-error message); classification stays weakly_exposed, genuine honesty fix per issue #1216

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (17)

Reason:
RIPR-SPEC-0107: error_path requires a variant-observing oracle; sibling/broad oracle no longer promotes exposed. The broad assert!(authenticate("").is_err()) oracle has no variant token (AuthError, RevokedToken are not in is_err() text), so discriminate message correctly changes to observation_unverified. Classification stays weakly_exposed.

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (18)

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — weak_error_oracle (19)

Reason:
add human-full golden for exhaustive evidence-promotion projection while default human stays bounded

Command:
cargo xtask goldens check

Updated:
- `expected/human-full.txt`

## Pending — weak_error_oracle (20)

Reason:
restrict CallDeletion probes to standalone call statements; refresh affected goldens and record intentional output changes

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — weak_error_oracle (21)

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — weak_error_oracle (22)

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — weak_error_oracle (23)

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — weak_error_oracle (24)

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — weak_error_oracle (25)

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — weak_error_oracle (26)

Reason:
RIPR-SPEC-0023: classification hint added to digest (#2614)

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — weak_error_oracle (27)

Reason:
RIPR-SPEC-0122: render the missing-discriminator value without restating the label

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — weak_error_oracle (28)

Reason:
RIPR-SPEC-0151: rebless check JSON for the additive source_currentness field; classifications, stages, confidence, counts, and recorded coordinates remain unchanged.

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`

## Pending — weak_error_oracle (29)

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — weak_error_oracle (30)

Reason:
RIPR-SPEC-0001: #3161 PR-B production witness gate upgrades complete direct-flow propagate evidence and fails incomplete flows closed

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — weak_error_oracle (31)

Reason:
RIPR-SPEC-0001: Err-construction return-value guidance names the missing reaching input and exact error variant instead of the false broad-assertion claim (PR review XQFi)

Command:
`cargo xtask goldens bless weak_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
