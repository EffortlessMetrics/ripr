# Golden Output Changes

## Pending

Reason:
document the local-callable-instance limitation (#1172): conservative weakly_exposed pinned; target exposed when relation+oracle-extraction traces local bindings

Command:
`cargo xtask goldens bless python_local_callable_binding --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive related_tests_total (#1204) + relation_reason/confidence (#1207) fields; fixture added by #1205 after those landed

Command:
`cargo xtask goldens bless python_local_callable_binding --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
surface smoke oracle via local_binding relation: corrects the misleading 'no direct test' card to 'strengthen existing smoke assertion'; class stays weakly_exposed (analysis/python-local-callable-instance-alignment)

Command:
`cargo xtask goldens bless python_local_callable_binding --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless python_local_callable_binding --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless python_local_callable_binding --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless python_local_callable_binding --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless python_local_callable_binding --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
show preview language metadata in human finding digests

Command:
`cargo xtask goldens bless python_local_callable_binding --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless python_local_callable_binding --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
