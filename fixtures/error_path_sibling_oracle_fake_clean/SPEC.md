# Fixture: error_path_sibling_oracle_fake_clean

Spec: RIPR-SPEC-0107

## Given

Production code changes the exact error variant returned for empty input from
`ParseError::TooShort` to `ParseError::TooLong`. A related test covers the
happy path with a sibling `ExactValue` oracle:

```rust
assert_eq!(validate_or_default("hello"), Ok("valid"));
```

This test does not observe the error path at all — it can only reach the
`Ok("valid")` branch. No variant-pinning oracle (`assert_eq!(err,
ParseError::TooLong)`, `assert!(matches!(err, ParseError::TooLong))`) exists.

## When

```bash
cargo xtask fixtures error_path_sibling_oracle_fake_clean
```

or:

```bash
ripr check --root fixtures/error_path_sibling_oracle_fake_clean/input --diff fixtures/error_path_sibling_oracle_fake_clean/diff.patch --mode fast
```

## Then

The `error_path` probe for `return Err(ParseError::TooLong)` reports
`weakly_exposed` (NOT `exposed`). The discriminate stage emits
`observation_unverified`: the sibling exact-value oracle on the happy-path
return value cannot confirm the changed error variant is specifically observed.
This is the fake-clean removed by RIPR-SPEC-0107.

## Must Not

- Classify the `error_path` probe as `exposed` when no variant-observing oracle
  exists (the bug being fixed).
- Treat a sibling `ExactValue` oracle on a non-error return path as confirming
  the changed error variant.
