# Golden Output Changes

## Pending

Reason:
pin residual method-owner false-exposed: imported+constructed owner class but asserted method runs on an unrelated receiver; receiver identity required

Command:
`cargo xtask goldens bless python_adversarial_method_owner_imported_unbound_receiver --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless python_adversarial_method_owner_imported_unbound_receiver --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
graduate 4 python adversarial fixtures into honesty corpus (RIPR-SPEC-0108, #1945): add expected/human-full.txt required by the evidence-promotion-honesty gate

Command:
`cargo xtask goldens bless python_adversarial_method_owner_imported_unbound_receiver --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless python_adversarial_method_owner_imported_unbound_receiver --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless python_adversarial_method_owner_imported_unbound_receiver --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
