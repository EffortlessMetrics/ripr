# Fixture: tail_comparison_boundary

Spec: RIPR-SPEC-0001

## Given

Two changed threshold predicates in one crate, each the whole tail
expression of a `-> bool` owner (no `if`, no `let`, no `return`):

- `ships_free`: `items > 10` becomes `items >= 10`. Tests reach it only
  through the `shipping(items)` wrapper, with exact assertions at 20 and 2
  items. No test uses 10 items.
- `earns_gift`: `items > 5` becomes `items >= 5`. The test calls
  `earns_gift(5)` and `earns_gift(4)` with exact assertions.

The diff has no blank context lines (`git diff --check` hygiene).

## When

```bash
cargo xtask fixtures tail_comparison_boundary
```

or:

```bash
ripr check --root fixtures/tail_comparison_boundary/input \
           --diff fixtures/tail_comparison_boundary/diff.patch \
           --mode fast
```

## Then

- The changed comparison is the owner's returned value, so each predicate
  probe carries `propagation yes` to the returned value instead of
  `propagation_unknown`.
- `ships_free` is the start-here gap: `weakly_exposed`, missing
  discriminator `items == 10` (observed `items` values 2 and 20 through the
  wrapper), next step the boundary-test guidance.
- `earns_gift`'s predicate probe is `exposed`: a related test input sits on
  the changed boundary (`items == 5`).

## Must Not

- Report a returned tail comparison as `propagation_unknown`.
- Hide the missing `items == 10` discriminator behind the generic
  broad-assertion advice at the start-here gap.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
