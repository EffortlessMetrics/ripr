# Golden Output Changes

## Pending

Reason:
initial golden for fix B control: returned call stays weakly_exposed

Command:
`cargo xtask goldens bless propagate_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
re-bless after merging origin/main (#1216): discriminate message now observation_unverified; classification weakly_exposed unchanged

Command:
`cargo xtask goldens bless propagate_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless propagate_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
changed semantic heads use canonical parser expressions and content-addressed probe identities

Command:
`cargo xtask goldens bless propagate_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
restrict CallDeletion probes to standalone call statements; refresh affected goldens and record intentional output changes

Command:
`cargo xtask goldens bless propagate_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless propagate_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless propagate_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless propagate_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless propagate_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless propagate_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless propagate_value_returned --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
