# Fixture: weak_error_oracle_assert_matches

Spec: RIPR-SPEC-0002

## Given

This is an oracle-shape variant of `weak_error_oracle`: the related test uses an
`assert_matches!` exact error variant assertion instead of broad `is_err()`.

## When

```bash
cargo xtask fixtures weak_error_oracle_assert_matches
```

or:

```bash
ripr check --root fixtures/weak_error_oracle_assert_matches/input --diff fixtures/weak_error_oracle_assert_matches/diff.patch --mode fast
```

## Then

`ripr` should recognize the exact error variant oracle and avoid reporting the
variant discriminator as missing.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Treat `assert_matches!(..., Err(AuthError::RevokedToken))` as a broad error
  assertion.
- Report `AuthError::RevokedToken` as missing.
