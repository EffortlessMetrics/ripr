# Fixture: observation_verified_call_deletion

Spec: RIPR-SPEC-0094

## Given

Production code changes a `store_result` function to use `"result_key"` as the
cache key:

```rust
pub fn store_result(cache: &mut Cache, result: i32) {
    cache.insert("result_key", result);   // ← was cache.insert("old_key", result)
}
```

The related test exercises `store_result` and then asserts the exact persisted
cache contents with `assert_eq!` — a Strong exact-value observer that names
`result_key` (a token from the probe expression):

```rust
#[test]
fn store_result_inserts_result_key_with_value() {
    let mut cache = Cache::new();
    store_result(&mut cache, 42);
    assert_eq!(cache.inserted, vec!["result_key=42".to_string()]);
}
```

This assertion both (a) token-matches the `CallDeletion` probe expression
`cache.insert("result_key", result)` and (b) reaches Strong oracle strength, so
the discriminate stage is `yes` and the finding is `exposed`.

## When

```bash
cargo xtask fixtures observation_verified_call_deletion
```

or:

```bash
ripr check --root fixtures/observation_verified_call_deletion/input \
           --diff fixtures/observation_verified_call_deletion/diff.patch --mode fast
```

## Then

`ripr` must classify the `CallDeletion` probe as `exposed` and must NOT emit
`observation_unverified`.

This fixture is the **verified-effect control** for
[RIPR-SPEC-0094](../../docs/specs/RIPR-SPEC-0094-observation-unverified-guard-generalization.md):
a CallDeletion probe whose changed behavior is genuinely observed (strong
persisted-state assertion) stays `exposed`, proving the `observation_unverified`
guard does not over-correct legitimately-observed effects.

## Must Not

- Emit `observation_unverified` when the assertion observes the changed effect.
- Downgrade a strong, token-matching effect observer to `weakly_exposed`.
- Use mutation-runtime outcome vocabulary.
