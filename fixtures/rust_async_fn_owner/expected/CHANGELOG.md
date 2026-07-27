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
