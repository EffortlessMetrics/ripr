# Fixture: unwrap_err_generic_is_err

Spec: RIPR-SPEC-0106

## Given

A function with `return Err(CalcError::Negative)` (the changed seam) and a
test that binds via `unwrap_err()` but then only makes a GENERIC assertion
that does not pin the specific variant:

```rust
let err = compute(-1).unwrap_err();
assert!(err.to_string().contains("error"));
```

The binding does not elevate to `ExactErrorVariant` because the subsequent
assertion lacks a named enum variant token.

## When

```bash
cargo xtask fixtures unwrap_err_generic_is_err
```

or:

```bash
ripr check --root fixtures/unwrap_err_generic_is_err/input \
           --diff fixtures/unwrap_err_generic_is_err/diff.patch --mode fast
```

## Then

`ripr` MUST NOT credit the `error_path` seam with `exposed`. The `is_err()`
or generic string-contains assertion does not pin the changed error variant.
The `error_path` seam should remain at most `weakly_exposed`.

## Must Not

- Raise the `error_path` seam for `CalcError::Negative` to `exposed` on the
  basis of a generic assertion.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Treat `assert!(err.to_string().contains(...))` as an `ExactErrorVariant`
  discriminator.
