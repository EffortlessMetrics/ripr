# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0097: new fixture for exact toThrow payload oracle upgrade

Command:
`cargo xtask goldens bless typescript_tothrow_exact_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0104: predicate-boundary seam observed only by a toThrow error oracle now correctly weakly_exposed/missing_target_shape (error oracle does not discriminate the predicate change; needs a whitespace-input test). Corrects a latent oracle cross-talk fake-clean from the #1234 golden; oracle_kind stays exact_error_variant.

Command:
`cargo xtask goldens bless typescript_tothrow_exact_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless typescript_tothrow_exact_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless typescript_tothrow_exact_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless typescript_tothrow_exact_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless typescript_tothrow_exact_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
show preview language metadata in human finding digests

Command:
`cargo xtask goldens bless typescript_tothrow_exact_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
