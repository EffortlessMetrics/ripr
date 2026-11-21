# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0133/#3213: the changed predicate is inside an inline `#[cfg(test)]` assertion helper. Under the producer-owned source-role contract, the helper remains evidence but does not create a production finding; regenerate the zero-finding check and human outputs.

Command:
`cargo xtask goldens bless assertion_shaped_oracle_cfg_test --reason "RIPR-SPEC-0133/#3213: inline cfg(test) helper is evidence, not a production subject"`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0133: assertion-shaped owner guidance fixtures (new fixture)

Command:
`cargo xtask goldens bless assertion_shaped_oracle_cfg_test --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless assertion_shaped_oracle_cfg_test --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless assertion_shaped_oracle_cfg_test --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless assertion_shaped_oracle_cfg_test --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless assertion_shaped_oracle_cfg_test --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless assertion_shaped_oracle_cfg_test --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0023: classification hint added to digest (#2614)

Command:
`cargo xtask goldens bless assertion_shaped_oracle_cfg_test --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0122: render the missing-discriminator value without restating the label

Command:
`cargo xtask goldens bless assertion_shaped_oracle_cfg_test --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless assertion_shaped_oracle_cfg_test --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
