# Golden Output Changes

## Pending — strong_error_oracle (1)

Reason:
RIPR-SPEC-0002: add negative and metamorphic evidence-first fixture baseline

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — strong_error_oracle (2)

Reason:
RIPR-SPEC-0026 output(language): RustAdapter tags each Finding with language=rust; check.json gains the additive optional language field

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — strong_error_oracle (3)

Reason:
schema 0.2: dedup assertion text into finding-level assertion_texts map (#1035)

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — strong_error_oracle (4)

Reason:
content-addressed-probe-ids-#1053

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — strong_error_oracle (5)

Reason:
platform-stable content-addressed ids (#1053): normalize owner path separators in fp8

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — strong_error_oracle (6)

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — strong_error_oracle (7)

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — strong_error_oracle (8)

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — strong_error_oracle (9)

Reason:
add relation_reason and relation_confidence fields to related_test JSON output

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — strong_error_oracle (10)

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — strong_error_oracle (11)

Reason:
add human-full golden for exhaustive evidence-promotion projection while default human stays bounded

Command:
cargo xtask goldens check

Updated:
- `expected/human-full.txt`

## Pending — strong_error_oracle (12)

Reason:
Parser-backed reveal analysis avoids confirming call effects from argument-only token matches (#1453)

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (13)

Reason:
restrict CallDeletion probes to standalone call statements; refresh affected goldens and record intentional output changes

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (14)

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (15)

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (16)

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (17)

Reason:
RIPR-SPEC-0076: raise exposed default severity from info to warning so the strongest finding class is not quieter than weaker classes

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (18)

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (19)

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (20)

Reason:
RIPR-SPEC-0151: rebless check JSON for the additive source_currentness field; classifications, stages, confidence, counts, and recorded coordinates remain unchanged.

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`

## Pending — strong_error_oracle (21)

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (22)

Reason:
RIPR-SPEC-0001: #3161 PR-B complete direct error witness gate

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (23)

Reason:
RIPR-SPEC-0001: suppress repair guidance when exact error oracle exists

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (24)

Reason:
RIPR-SPEC-0001: retain repair guidance for unaligned direct sink

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (25)

Reason:
RIPR-SPEC-0001: exact owner-bound error witness establishes the changed error variant path

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (26)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (27)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (28)

Reason:
RIPR-SPEC-0122: exposed findings default to info severity

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (29)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (30)

Reason:
RIPR-SPEC-0122: #4321 additive per-finding id lines in human-full (drill-in identifiers)

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
## Pending — strong_error_oracle (31)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/human.txt`
## Pending — strong_error_oracle (30)

Reason:
RIPR-SPEC-0122: #4321 additive per-finding id lines in human-full (drill-in identifiers)

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — strong_error_oracle (32)

Reason:
RIPR-SPEC-0122: #4322 summary header names all seven classes with canonical tokens and a shown/total denominator

Command:
`cargo xtask goldens bless strong_error_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`