# Fixture: observation_unverified_field_construction

Spec: RIPR-SPEC-0094

## Given

Production code changes a struct field initializer from `retries: 1` to
`retries` (a variable binding resolving to 3) in a `Config` struct:

```rust
pub fn default_config() -> Config {
    let retries = 3;
    Config {
        timeout_secs: 30,
        retries,          // ← was retries: 1
    }
}
```

The only related test exercises `default_config` and asserts on the
`timeout_secs` field, not on `retries`:

```rust
#[test]
fn config_exists() {
    let cfg = default_config();
    assert!(cfg.timeout_secs > 0);
}
```

The assertion text contains no identifier token from the `FieldConstruction`
probe expression `retries: 1,` — specifically, "retries" does not appear in
`assert!(cfg.timeout_secs > 0)`. There is no `token_match`.

## When

```bash
cargo xtask fixtures observation_unverified_field_construction
```

or:

```bash
ripr check --root fixtures/observation_unverified_field_construction/input \
           --diff fixtures/observation_unverified_field_construction/diff.patch --mode fast
```

## Then

`ripr` must emit `weakly_exposed` (not `exposed`) for the `retries: 1,`
FieldConstruction probe. The discriminate stage must be `weak` with reason
`observation_unverified`, and confidence must be less than 1.0.

This fixture is the **before/escape-hatch proof** for the FieldConstruction
family in
[RIPR-SPEC-0094](../../docs/specs/RIPR-SPEC-0094-observation-unverified-guard-generalization.md):
before this fix, a single-assertion test that reached `default_config` would
be credited with discriminating the changed `retries` field via the
`assertion_count == 1` escape hatch even without referencing `retries`.

## Must Not

- Report the `retries` field probe as `exposed` when no assertion references it.
- Use mutation-runtime outcome vocabulary.
- Report `reachable_unrevealed` (the test IS reachable and DOES observe a field).
