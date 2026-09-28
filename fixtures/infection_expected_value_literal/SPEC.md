# Fixture: infection_expected_value_literal

Spec: RIPR-SPEC-0001

## Given

Two changed threshold predicates in one crate, each reached by a test that
calls the changed owner:

- `fragile_fee`: `weight_grams > 2_000` becomes `>= 2_000`. The related test
  calls `fragile_fee(weight)` with a computed weight
  (`parcel_weight("crate")`, not a literal) and, in the same test, asserts
  `assert_eq!(tax_bps("EU"), 2000)`. The boundary literal `2000` appears only
  as the expected value of an assertion on another function.
- `bulky_fee`: `volume_litres > 50` becomes `>= 50`. The related test binds
  `let volume = 50;` and calls `bulky_fee(volume)`.

The diff has no blank context lines (`git diff --check` hygiene).

## When

```bash
cargo xtask fixtures infection_expected_value_literal
```

or:

```bash
ripr check --root fixtures/infection_expected_value_literal/input \
           --diff fixtures/infection_expected_value_literal/diff.patch \
           --mode fast
```

## Then

- `fragile_fee` is `weakly_exposed` with `infection weak`: the evidence says
  the boundary literal `2000` appears only outside the changed owner's inputs
  (for example as an expected value) and that no test input at the changed
  boundary was detected.
- `bulky_fee` stays `exposed`: the let-bound literal `50` is the owner's input
  (`volume_litres = 50`), so the related test is credited with an input at the
  changed boundary.

## Must Not

- Credit a boundary-matching literal as infection evidence when it is an
  assertion's expected value rather than a value passed into the changed
  owner.
- Report `fragile_fee` as `exposed`.
- Drop the let-bound owner input for `bulky_fee`.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
