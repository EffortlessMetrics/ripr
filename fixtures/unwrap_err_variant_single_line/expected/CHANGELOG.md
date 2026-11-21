# Golden Output Changes

## Pending — unwrap_err_variant_single_line (1)

Reason:
new fixture: single-line unwrap_err variant binding (#1255 regression guard)

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (2)

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (3)

Reason:
Parser-backed reveal analysis avoids confirming call effects from argument-only token matches (#1453)

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (4)

Reason:
changed semantic heads use canonical parser expressions while exact error-family classifications stay stable

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (5)

Reason:
restrict CallDeletion probes to standalone call statements; refresh affected goldens and record intentional output changes

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (6)

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (7)

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (8)

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (9)

Reason:
RIPR-SPEC-0076: raise exposed default severity from info to warning so the strongest finding class is not quieter than weaker classes

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (10)

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (11)

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (12)

Reason:
RIPR-SPEC-0151: rebless check JSON for the additive source_currentness field; classifications, stages, confidence, counts, and recorded coordinates remain unchanged.

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`

## Pending — unwrap_err_variant_single_line (13)

Reason:
RIPR-SPEC-0158: the additive per-value provenance field now surfaces the evaluation chains and call sources the line-keyed assertion_texts map dropped (deferred #3295 follow-up)

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (14)

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (15)

Reason:
RIPR-SPEC-0001: #3161 PR-B exact complete error witness

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (16)

Reason:
RIPR-SPEC-0001: suppress repair guidance when exact error oracle exists

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (17)

Reason:
RIPR-SPEC-0001: retain repair guidance for unaligned direct sink

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (18)

Reason:
RIPR-SPEC-0001: fail-closed witness summary wording for incomplete single-line error variant flow

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unwrap_err_variant_single_line (19)

Reason:
RIPR-SPEC-0106: error-variant sink alignment now credits the exact-variant oracle on the return-value companion probe, so the false broad-assertion repair guidance is suppressed (PR review WCtF); Err-construction return-value guidance names the missing reaching input and exact variant (review XQFi)

Command:
`cargo xtask goldens bless unwrap_err_variant_single_line --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
