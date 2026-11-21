# Fixture: infect_wildcard_discard

Spec: RIPR-SPEC-0096

## Given

Production code adds calls whose return values are bound to the wildcard
discard patterns `let _ : i32 = compute_fee(amount * 9)` and
`let _= compute_fee(amount * 9)`. The caller function returns `amount`
unchanged — the changed values cannot reach any observable sink.

A test with a strong exact-value assertion covers `process`:

```rust
#[test]
fn process_returns_amount_unchanged() {
    assert_eq!(process(42), 42);
}
```

## When

```bash
cargo xtask fixtures infect_wildcard_discard
```

or:

```bash
ripr check --root fixtures/infect_wildcard_discard/input --diff fixtures/infect_wildcard_discard/diff.patch --mode fast
```

## Then

`ripr` must emit `infection_unknown` (not `exposed`) for the
both wildcard-discard probes. The infect stage must be
`unknown` with reason "Changed value is bound to a discard pattern; it
cannot infect a sink".

This fixture is the **before/after proof** for fix A introduced in
[RIPR-SPEC-0096](../../docs/specs/RIPR-SPEC-0096-infect-propagate-fail-closed.md):
without the fix this probe was reported `exposed` — a false-actionable
because the discarded value can never reach a sink.

## Must Not

- Use mutation-runtime outcome vocabulary.
- Report `exposed` when the changed value is bound to a wildcard discard.
- Downgrade `let _name = ...` (named bindings) — those may still be used.
