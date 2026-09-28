# Fixture: unwrap_err_sibling_variant

Spec: RIPR-SPEC-0106

## Given

A function with TWO error returns: `Err(CalcError::Negative)` and
`Err(CalcError::TooLarge)`. The CHANGED seam is `TooLarge`. The ONLY error
test pins the Negative variant:

```rust
let err = compute(-1).unwrap_err();
assert_eq!(err, CalcError::Negative);
```

## When

```bash
cargo xtask fixtures unwrap_err_sibling_variant
```

or:

```bash
ripr check --root fixtures/unwrap_err_sibling_variant/input \
           --diff fixtures/unwrap_err_sibling_variant/diff.patch --mode fast
```

## Then

`ripr` MUST report the `TooLarge` error seam as `weakly_exposed` (NOT
`exposed`). The `Negative`-pinning test does NOT discriminate whether the
`TooLarge` behavior changed. This is the over-credit guard: a sibling
variant assertion must not credit an unrelated variant seam.

## Must Not

- Raise the `error_path` seam for `TooLarge` to `exposed`.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Credit a `CalcError::Negative` assertion as discriminating `CalcError::TooLarge`.
