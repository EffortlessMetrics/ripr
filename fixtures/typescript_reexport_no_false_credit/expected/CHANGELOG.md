# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0095 no-false-credit control: src/index.ts re-exports isRawNetworkError from src/other.ts (NOT the changed owner in src/util.ts); single-hop trace resolves to a different source file, so the test correctly stays uncredited (no_static_path, 0 related tests)

Command:
`cargo xtask goldens bless typescript_reexport_no_false_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0094: additive relation_reason+relation_confidence fields now populated in TS related_tests; new fixtures: typescript_reexport_single_hop (credit), typescript_reexport_no_false_credit (no-false-credit control), typescript_reexport_two_hop_limit (two-hop control); initial golden bless

Command:
`cargo xtask goldens bless typescript_reexport_no_false_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: honest no-static-path messaging (RIPR-SPEC-0113)

Command:
`cargo xtask goldens bless typescript_reexport_no_false_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
P2: all-no-path scope counts

Command:
`cargo xtask goldens bless typescript_reexport_no_false_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless typescript_reexport_no_false_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless typescript_reexport_no_false_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless typescript_reexport_no_false_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless typescript_reexport_no_false_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
show preview language metadata in human finding digests

Command:
`cargo xtask goldens bless typescript_reexport_no_false_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless typescript_reexport_no_false_credit --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
