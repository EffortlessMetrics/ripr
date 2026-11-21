# Fixture: observation_unverified_call_deletion

Spec: RIPR-SPEC-0094

## Given

Production code changes a `store_result` function to use `"result_key"` as the
cache key (instead of `"old_key"`):

```rust
pub fn store_result(cache: &Cache, result: i32) {
    cache.insert("result_key", result);   // ← was cache.insert("old_key", result)
}
```

The only related test calls `store_result` with a trivial assertion that does
not reference `"result_key"` or any other token from the changed call
expression:

```rust
#[test]
fn store_result_runs_without_panic() {
    let cache = Cache;
    store_result(&cache, 42);
    assert!(true);
}
```

The assertion text `assert!(true)` contains no token from the `CallDeletion`
probe expression `cache.insert("result_key", result)` that is specific to this
call site. There is no `token_match` beyond the generic "assert" prefix.

## When

```bash
cargo xtask fixtures observation_unverified_call_deletion
```

or:

```bash
ripr check --root fixtures/observation_unverified_call_deletion/input \
           --diff fixtures/observation_unverified_call_deletion/diff.patch --mode fast
```

## Then

`ripr` must emit `weakly_exposed` (not `exposed`) for the `CallDeletion` probe.
The discriminate stage must be `weak` with reason `observation_unverified`,
and confidence must be less than 1.0.

This fixture is the **before/escape-hatch proof** for the CallDeletion family
in [RIPR-SPEC-0094](../../docs/specs/RIPR-SPEC-0094-observation-unverified-guard-generalization.md):
before this fix, `oracle_matches_family(CallDeletion, ...)` fired on any
assertion text containing "assert", so `assert!(true)` was credited as
discriminating the specific `"result_key"` change — a false-actionable claim.

## Must Not

- Report `exposed` when no assertion text references any token from the changed cache.insert call.
- Use mutation-runtime outcome vocabulary.
- Report `reachable_unrevealed` (the test IS reachable and DOES have an assertion).
