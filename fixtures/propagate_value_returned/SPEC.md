# Fixture: propagate_value_returned

Spec: RIPR-SPEC-0096

## Given

Production code changes the multiplier in a call that is directly returned:

```rust
pub fn apply(&mut self, amount: i32) -> Result<(), String> {
    return self.persist(amount * 9);
}
```

The result is returned to the caller — the changed value escapes through
the return path.  A test with an exact assertion covers the returned value:

```rust
#[test]
fn apply_returns_ok_on_success() {
    let mut ledger = Ledger::new(100);
    assert!(ledger.apply(5).is_ok());
}
```

## When

```bash
cargo xtask fixtures propagate_value_returned
```

or:

```bash
ripr check --root fixtures/propagate_value_returned/input --diff fixtures/propagate_value_returned/diff.patch --mode fast
```

## Then

`ripr` must emit `exposed` for the `return self.persist(amount * 9)` probe.
A returned result is NOT swallowed — the value propagates to the caller.
This is the **control** for fix B in
[RIPR-SPEC-0096](../../docs/specs/RIPR-SPEC-0096-infect-propagate-fail-closed.md):
the swallowed-value predicate must NOT fire when the result is returned.

## Must Not

- Classify a directly-returned call as `propagation_unknown`.
- Use mutation-runtime outcome vocabulary.
