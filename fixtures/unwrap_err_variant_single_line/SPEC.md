# Fixture: unwrap_err_variant_single_line

Spec: RIPR-SPEC-0106

## Given

A function whose changed seam is `return Err(ParseError::TooLong(name.len()))`
(a tuple-payload error variant) and a dedicated test that binds the error via
`unwrap_err()` and asserts the exact variant — written on a **single line**, the
way an un-`rustfmt`'d or macro-generated test body appears:

```rust
fn too_long_name_rejected_with_exact_variant() { let err = validate("aaaaaaaaaaaa").unwrap_err(); assert_eq!(err, ParseError::TooLong(12)); }
```

## When

```bash
cargo xtask fixtures unwrap_err_variant_single_line
```

or:

```bash
ripr check --root fixtures/unwrap_err_variant_single_line/input \
           --diff fixtures/unwrap_err_variant_single_line/diff.patch --mode fast
```

## Then

`ripr` should detect the `unwrap_err`-bound exact-variant oracle even though the
`let` binding does not begin a source line (it sits mid-line after `{`). The
`error_path` seam is credited `exposed` with discriminator state `yes`, and
`ParseError::TooLong` is **not** listed as a missing discriminator.

Binding detection is statement-oriented, not line-oriented, so a discriminated
seam never carries a contradictory `missing_discriminators` entry purely because
the test body was not split onto multiple lines.

## Must Not

- List `ParseError::TooLong` as a missing discriminator while the same seam is
  reported `exposed` / discriminate `yes` (the self-contradiction this guards).
- Require multi-line test formatting for the `unwrap_err` variant binding to be
  recognized.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.

Claim boundaries for this fixture remain governed by the canonical ledger in
[support tiers](../../docs/status/SUPPORT_TIERS.md).
