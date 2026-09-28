# Fixture: unwrap_err_variant_positive

Spec: RIPR-SPEC-0106

## Given

A function with `return Err(CalcError::Negative)` (the changed seam) and a
dedicated same-module test that binds the error via `unwrap_err()` and then
asserts the exact variant:

```rust
let err = compute(-1).unwrap_err();
assert_eq!(err, CalcError::Negative);
```

## When

```bash
cargo xtask fixtures unwrap_err_variant_positive
```

or:

```bash
ripr check --root fixtures/unwrap_err_variant_positive/input \
           --diff fixtures/unwrap_err_variant_positive/diff.patch --mode fast
```

## Then

`ripr` should recognize the `unwrap_err`-bound error oracle as an
`ExactErrorVariant` assertion and credit the `error_path` seam with
`exposed` / `strongly_gripped` (discriminator state yes).

The `return_value` seam shares the same line but asserts an `ExactValue` oracle
(the return type is `Result<i32, CalcError>`); it must also be credited.

## Must Not

- Report `CalcError::Negative` as a missing discriminator.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Credit the `error_path` seam when the assertion pins a DIFFERENT variant
  (the sibling-variant guard in fixture `unwrap_err_sibling_variant` tests this).
