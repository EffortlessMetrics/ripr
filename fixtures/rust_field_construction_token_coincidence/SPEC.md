# Fixture: rust_field_construction_token_coincidence

Spec: RIPR-SPEC-0094

## Given

Production code changes the `retries` initializer of `Config` in
`default_config` from `retries: 1` to `retries`. The related test calls
`default_config()` but asserts only `cfg.timeout_secs`. It also asserts
`fb.retries` on an unrelated `Fallback` value:

```rust
let cfg = default_config();
assert_eq!(cfg.timeout_secs, 30);
let fb = fallback();
assert_eq!(fb.retries, 3);
```

The `retries` token in `fb.retries` coincides with the changed field name,
but no assertion reads the field on the value `default_config()` returns.

## When

```bash
ripr check --root fixtures/rust_field_construction_token_coincidence/input \
           --diff fixtures/rust_field_construction_token_coincidence/diff.patch --mode fast
```

## Then

The `retries` FieldConstruction finding must not be `exposed`. The missing
field-value discriminator stays attached, and discrimination is capped below
`yes` (#4428). This is the negative partner of
`observation_verified_field_construction`, where `cfg.retries` on the owner's
result keeps the finding `exposed`.

## Must Not

- Promote the finding to `exposed` from a same-named field on another value.
- Use mutation-runtime outcome vocabulary.
