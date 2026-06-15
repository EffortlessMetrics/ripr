# Fixture: propagate_swallowed_ok

Spec: RIPR-SPEC-0096

## Given

Production code changes the multiplier in a call whose result is
immediately swallowed with `.ok()`:

```rust
pub fn apply(&mut self, amount: i32) {
    self.persist(amount * 9).ok();
}
```

The `.ok()` tail converts the `Result` to `Option` and drops it — the
changed value cannot propagate to a directly observable sink.  A test with
a strong exact-value assertion covers the balance observable:

```rust
#[test]
fn apply_changes_balance() {
    let mut ledger = Ledger::new(100);
    ledger.apply(5);
    assert_eq!(ledger.balance(), 145);
}
```

## When

```bash
cargo xtask fixtures propagate_swallowed_ok
```

or:

```bash
ripr check --root fixtures/propagate_swallowed_ok/input --diff fixtures/propagate_swallowed_ok/diff.patch --mode fast
```

## Then

`ripr` must emit `propagation_unknown` (not `exposed`) for the
`self.persist(amount * 9).ok()` probe.  The propagate stage must be
`unknown` because the call-chain tail swallows the return value.

This fixture is the **before/after proof** for fix B introduced in
[RIPR-SPEC-0096](../../docs/specs/RIPR-SPEC-0096-infect-propagate-fail-closed.md):
without the fix this probe was reported `exposed` — a false-actionable
because the swallowed result cannot reach a directly observable sink.

## Must Not

- Use mutation-runtime outcome vocabulary.
- Report `exposed` when the call-chain tail is `.ok()` swallowing the result.
- Downgrade `x.ok().map(f)` (the value continues to flow — must stay exposed).
