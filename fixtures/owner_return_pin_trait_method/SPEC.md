# Fixture: owner_return_pin_trait_method

Spec: RIPR-SPEC-0197

## Given

The tokio-rs/bytes 7930d93 shape. A trait default method's tail changes to
sign-extend its result:

```rust
fn try_get_int(&mut self, nbytes: usize) -> Result<i64, TryGetError> {
    Ok(sign_extend(self.try_get_uint(nbytes)?, nbytes))
}
```

The workspace implements the trait for `&[u8]`, and nothing overrides
`try_get_int`. The test file imports the trait and pins the return value
through a method call on a byte-slice receiver:

```rust
let mut a = &[0xff, 0xff, 0xff][..];
assert_eq!(a.try_get_int(3), Ok(-1));
```

A second test pins only the `Err(..)` that the `?` exit returns.

## When

```bash
cargo xtask fixtures owner_return_pin_trait_method
```

or:

```bash
ripr check --root fixtures/owner_return_pin_trait_method/input \
           --diff fixtures/owner_return_pin_trait_method/diff.patch --mode fast
```

## Then

`ripr` must emit `exposed` for the `return_value` probe. The confirming
oracle is the `Ok(-1)` pin: the assertion's operand is a complete call that
names the owner (the receiver is bound to `&[u8]`, which dispatches to the
trait default), and the changed tail is the owner's only `Ok(..)`.

## Must Not

- Confirm the probe through the `Err(TryGetError { .. })` assertion alone:
  that input leaves through `?` and never builds the changed `Ok(..)`.
- Use mutation-runtime outcome vocabulary.
