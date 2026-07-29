# Golden Output Changes

## Pending

Reason:
issue #1983: initial golden for TypeScript false-promotion corpus charter fixture (RIPR-SPEC-0108)

Command:
`cargo xtask goldens bless typescript_adversarial_hoc_wrapped_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless typescript_adversarial_hoc_wrapped_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless typescript_adversarial_hoc_wrapped_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
show preview language metadata in human finding digests

Command:
`cargo xtask goldens bless typescript_adversarial_hoc_wrapped_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless typescript_adversarial_hoc_wrapped_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
