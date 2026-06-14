# Fixture: observation_unverified_return_value

Spec: RIPR-SPEC-0094

## Given

Production code changes the expression returned by a function from
`base + 1` to `base * 2`:

```rust
pub fn compute_score(base: i32) -> i32 {
    base * 2
}
```

The only related test calls `compute_score` and asserts a broad relational
property:

```rust
#[test]
fn result_is_positive() {
    assert!(compute_score(3) > 0);
}
```

The assertion text contains no identifier token from the probe expression
`base * 2` that does not also appear in the test call — specifically, `base`
does not appear in the assertion. There is no `token_match`.

## When

```bash
cargo xtask fixtures observation_unverified_return_value
```

or:

```bash
ripr check --root fixtures/observation_unverified_return_value/input \
           --diff fixtures/observation_unverified_return_value/diff.patch --mode fast
```

## Then

`ripr` must emit `weakly_exposed` (not `exposed`) for the `base * 2` probe.
The discriminate stage must be `weak` with reason `observation_unverified`,
and confidence must be less than 1.0.

This fixture is the **before/escape-hatch proof** for
[RIPR-SPEC-0094](../../docs/specs/RIPR-SPEC-0094-observation-unverified-guard-generalization.md):
before this fix, the `ReturnValue` family used the single-assertion escape
hatch (`assertion_count == 1`) without a `token_match` check, producing
`exposed`/confidence-1.00 — a false-actionable claim.

## Must Not

- Report this probe as `exposed` when no assertion text references the changed expression.
- Use mutation-runtime outcome vocabulary.
- Report `reachable_unrevealed` (the test IS reachable and DOES have an oracle).
