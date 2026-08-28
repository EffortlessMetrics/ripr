# Fixture: assertion_shaped_oracle_cfg_test

Spec: RIPR-SPEC-0133

## Given

An assertion-shaped oracle helper lives in a production file behind
`#[cfg(test)]` (`src/lib.rs`, `mod tests`). Its body is dominated by
`assert!`/`assert_eq!` calls and its only caller is a `#[test]` in the same
module.

The diff changes a predicate computed inside the helper:

```rust
let changed = if normalized.len() >= path.len() { 1 } else { 0 };
```

to:

```rust
let changed = if normalized.len() > path.len() { 1 } else { 0 };
```

## When

```bash
cargo xtask fixtures assertion_shaped_oracle_cfg_test
```

or:

```bash
ripr check --root fixtures/assertion_shaped_oracle_cfg_test/input --diff fixtures/assertion_shaped_oracle_cfg_test/diff.patch --mode fast
```

## Then

`ripr` classifies the change `weakly_exposed` but reframes the
`recommended_next_step`: it must NOT say "Replace broad assertions with exact
equality" (the hawk complaint — exact equality is unavailable for a boolean
invariant and an exact-equality assertion already exists in the same helper).
The guidance instead advises tightening the loosest assertion, and the finding
evidence includes `owner_shape: assertion_shaped (...)`.

## Must Not

- Change the exposure class because the owner is assertion-shaped.
- Emit the standard code-under-test guidance for this owner.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
