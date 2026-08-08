# Golden CHANGELOG

## Pending

- Initial bless: async fn owner fixture probes whether ripr correctly resolves
  `async fn` owners and `.await`-based test oracles. The analyzer reaches the
  changed predicate, identifies the strong oracle (exact_value), but reports
  `propagation_unknown` because it cannot statically trace `.await` return
  propagation. This is an honest limitation, not an over-credit (#2450 part 2).

## Pending

Reason:
Initial async-fn fixture: probes async owner resolution + propagation_unknown honesty (#2450 part 2)

Command:
`cargo xtask goldens bless rust_async_fn_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
Align input with the added revision (>); the input was the pre-change form (>=), inconsistent with the diff postimage and other fixtures (review P2 #2486)

Command:
`cargo xtask goldens bless rust_async_fn_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless rust_async_fn_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless rust_async_fn_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless rust_async_fn_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless rust_async_fn_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless rust_async_fn_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0023: classification hint added to digest (#2614)

Command:
`cargo xtask goldens bless rust_async_fn_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending

Reason:
RIPR-SPEC-0122: render the missing-discriminator value without restating the label

Command:
`cargo xtask goldens bless rust_async_fn_owner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
