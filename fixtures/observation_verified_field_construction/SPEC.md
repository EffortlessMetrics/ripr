# Fixture: observation_verified_field_construction

Spec: RIPR-SPEC-0094

## Given

Production code changes a struct field initializer from `retries: 1` to
`retries` in a `Config` struct:

```rust
pub fn default_config() -> Config {
    let retries = 3;
    Config {
        timeout_secs: 30,
        retries,          // ← was retries: 1
    }
}
```

The related test directly asserts the exact value of the changed `retries`
field:

```rust
#[test]
fn config_has_three_retries() {
    let cfg = default_config();
    assert_eq!(cfg.retries, 3);
}
```

The assertion text `assert_eq!(cfg.retries, 3)` contains `retries`, which is
an identifier token from the `FieldConstruction` probe expression `retries: 1,`.
Therefore `token_match` fires and `observation_unverified` is NOT set.

## When

```bash
cargo xtask fixtures observation_verified_field_construction
```

or:

```bash
ripr check --root fixtures/observation_verified_field_construction/input \
           --diff fixtures/observation_verified_field_construction/diff.patch --mode fast
```

## Then

`ripr` must emit `exposed` for the `retries: 1,` FieldConstruction probe.
The `token_match` on `retries` confirms the assertion references the specific
changed field, so `observation_unverified` must NOT be set and
`discriminate.state` must be `yes`.

This fixture is the **anti-over-correction proof** for the FieldConstruction
family in
[RIPR-SPEC-0094](../../docs/specs/RIPR-SPEC-0094-observation-unverified-guard-generalization.md):
a FieldConstruction probe with a token-matching exact assertion must stay
`exposed`.

## Must Not

- Downgrade a confirmed field observer to `weakly_exposed`.
- Emit `observation_unverified` when the assertion directly references the changed field.
- Use mutation-runtime outcome vocabulary.
