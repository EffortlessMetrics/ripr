# Fixture: observation_verified_return_value

Spec: RIPR-SPEC-0094

## Given

Production code changes the return expression of a function from `x + 1` to
`x * SCALE_FACTOR`:

```rust
pub const SCALE_FACTOR: i32 = 2;

pub fn compute_score(x: i32) -> i32 {
    x * SCALE_FACTOR
}
```

The related test asserts the exact expected value referencing `SCALE_FACTOR`
directly:

```rust
#[test]
fn score_uses_scale_factor() {
    assert_eq!(compute_score(3), 3 * SCALE_FACTOR);
}
```

The assertion text contains `SCALE_FACTOR`, which is an identifier token from
the probe expression `x * SCALE_FACTOR`. Therefore `token_match` fires and
`observation_unverified` is NOT set.

## When

```bash
cargo xtask fixtures observation_verified_return_value
```

or:

```bash
ripr check --root fixtures/observation_verified_return_value/input \
           --diff fixtures/observation_verified_return_value/diff.patch --mode fast
```

## Then

`ripr` must emit `exposed` (confidence 1.0) for the `x * SCALE_FACTOR` probe.
The `token_match` on `SCALE_FACTOR` confirms the assertion references the
specific changed expression, so `observation_unverified` must NOT be set and
`discriminate.state` must be `yes`.

This fixture is the **anti-over-correction proof** for
[RIPR-SPEC-0094](../../docs/specs/RIPR-SPEC-0094-observation-unverified-guard-generalization.md):
a ReturnValue probe with a token-matching exact assertion must stay `exposed`.

## Must Not

- Downgrade a confirmed observer to `weakly_exposed`.
- Emit `observation_unverified` when the assertion references the changed expression's token.
- Use mutation-runtime outcome vocabulary.
