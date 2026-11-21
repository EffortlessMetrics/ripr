# Fixture: observation_unverified_side_effect

Spec: RIPR-SPEC-0094

## Given

Production code changes a `process_order` function to pass `order_id` as the
payload to `notifier.send` (instead of a static string):

```rust
pub fn process_order(notifier: &Notifier, order_id: &str) -> bool {
    notifier.send(order_id)   // ← was notifier.send("static_msg")
}
```

The only related test exercises `process_order` and asserts a broad boolean
result without referencing the specific `order_id` argument:

```rust
#[test]
fn process_order_succeeds() {
    let notifier = Notifier;
    let result = process_order(&notifier, "order-1");
    assert!(result);
}
```

The assertion text `assert!(result)` contains no identifier token from the
`SideEffect` probe expression `notifier.send(order_id)` — "order_id", "notifier",
and "send" are all absent. There is no `token_match`.

## When

```bash
cargo xtask fixtures observation_unverified_side_effect
```

or:

```bash
ripr check --root fixtures/observation_unverified_side_effect/input \
           --diff fixtures/observation_unverified_side_effect/diff.patch --mode fast
```

## Then

`ripr` must emit `weakly_exposed` (not `exposed`) for the `SideEffect` probe.
The discriminate stage must be `weak` with reason `observation_unverified`,
and confidence must be less than 1.0.

This fixture is the **before/escape-hatch proof** for the SideEffect family
in [RIPR-SPEC-0094](../../docs/specs/RIPR-SPEC-0094-observation-unverified-guard-generalization.md):
before this fix, a single broad `assert!(result)` would be credited as
discriminating the specific `order_id` payload change via the
`assertion_count == 1` escape hatch — a false-actionable claim.

## Must Not

- Report `exposed` when no assertion text references any token from the changed send call.
- Use mutation-runtime outcome vocabulary.
- Report `reachable_unrevealed` (the test IS reachable and DOES have an assertion).
