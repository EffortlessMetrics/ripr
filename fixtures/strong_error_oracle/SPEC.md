# Fixture: strong_error_oracle

Spec: RIPR-SPEC-0002

## Given

Production code changes the exact error variant for an empty token, and the
related test asserts `Err(AuthError::RevokedToken)` exactly.

## When

```bash
cargo xtask fixtures strong_error_oracle
```

or:

```bash
ripr check --root fixtures/strong_error_oracle/input --diff fixtures/strong_error_oracle/diff.patch --mode fast
```

## Then

`ripr` should report stronger evidence than `weak_error_oracle` because the exact
error variant discriminator is present.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Treat the exact error variant assertion as only a broad error oracle.
- Report `AuthError::RevokedToken` as a missing discriminator.
