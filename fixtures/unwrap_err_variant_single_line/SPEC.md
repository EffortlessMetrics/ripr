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
`error_path` seam carries discriminator state `yes`, and `ParseError::TooLong`
is **not** listed as a missing discriminator.

Because the complete owner-bound propagation witness cannot be established for
this tuple-payload variant (the changed payload `name.len()` does not resolve to
the `Result::Err(ParseError::TooLong)` sink identity), the seam stays
`weakly_exposed` rather than `exposed`: reach plus a strong oracle never credits
`exposed` without an established propagation path (RIPR-SPEC-0096 fail-closed;
`#3161` PR-B witness contract). The exact-variant oracle still suppresses the
assertion-repair guidance on both seams.

Binding detection is statement-oriented, not line-oriented, so a discriminated
seam never carries a contradictory `missing_discriminators` entry purely because
the test body was not split onto multiple lines.

## Must Not

- List `ParseError::TooLong` as a missing discriminator while the same seam
  discriminates `yes` (the self-contradiction this guards).
- Require multi-line test formatting for the `unwrap_err` variant binding to be
  recognized.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.

Claim boundaries for this fixture remain governed by the canonical ledger in
[support tiers](../../docs/status/SUPPORT_TIERS.md).
