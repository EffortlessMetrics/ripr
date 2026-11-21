# Fixture: weak_error_oracle

Spec: RIPR-SPEC-0001

## Given

Production code changes the exact error variant returned for an empty token
from:

```rust
AuthError::EmptyToken
```

to:

```rust
AuthError::RevokedToken
```

The related test reaches the changed path but only asserts:

```rust
assert!(authenticate("").is_err());
```

It does not assert the exact error variant.

## When

```bash
cargo xtask fixtures weak_error_oracle
```

or:

```bash
ripr check --root fixtures/weak_error_oracle/input --diff fixtures/weak_error_oracle/diff.patch --mode fast
```

## Then

`ripr` records the current static exposure classification for the changed error
path and names the missing exact error-variant discriminator.

The current expected output is a baseline for future oracle-strength
improvements. If a later PR improves the classification, bless the changed
output with a reason that cites the relevant spec.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Treat broad `is_err()` coverage as an exact error-variant discriminator.
- Hide that the exact error variant is not asserted.
