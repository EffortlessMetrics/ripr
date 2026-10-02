# Fixture: owner_return_pin_identity_traps

Spec: RIPR-SPEC-0197

## Given

Four changed return values, each with a test whose `assert_eq!` calls
something named like the owner without pinning the owner's changed value:

- `Codec::decode` (an associated function) changes; the test asserts
  `assert_eq!(decode(8), 9)`, a bare call that names the free function
  `decode`.
- The trait default `Reader::next_word` changes; the test's receiver is
  `Fixed`, whose `impl Reader` overrides `next_word`.
- `checked_half`'s tail `Ok(validate(x)? >> 1)` changes; the test asserts
  `assert_eq!(checked_half(-4), Err(HalfError::Negative))`, an input that
  leaves through `?`.
- `scaled` changes; the test binds `let scaled = |value: i32| value * 10;`
  and asserts `assert_eq!(scaled(3), 30)` on its own closure.

## When

```bash
cargo xtask fixtures owner_return_pin_identity_traps
```

or:

```bash
ripr check --root fixtures/owner_return_pin_identity_traps/input \
           --diff fixtures/owner_return_pin_identity_traps/diff.patch --mode fast
```

## Then

No finding reads `exposed`. Each assertion fails one owner-return pin gate
of RIPR-SPEC-0197: call identity (associated versus free, overridden
default, local binding) or the return path (the early-exit input).

## Must Not

- Emit `exposed` for any of the four probes.
- Use mutation-runtime outcome vocabulary.
