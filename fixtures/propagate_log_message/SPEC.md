# Fixture: propagate_log_message

Spec: RIPR-SPEC-0096

## Given

Production code changes the multiplier in a `log::info!` macro call:

```rust
pub fn report(amount: i32) {
    log::info!("amount is {}", amount * 9);
}
```

`log::` is a capturable sink (a structured logging framework with
subscribers/consumers).  The test only checks that the function does not
panic (smoke oracle):

```rust
#[test]
fn report_does_not_panic() {
    report(5);
}
```

## When

```bash
cargo xtask fixtures propagate_log_message
```

or:

```bash
ripr check --root fixtures/propagate_log_message/input --diff fixtures/propagate_log_message/diff.patch --mode fast
```

## Then

`ripr` must NOT downgrade the `log::info!` probe to `propagation_unknown`.
`log::` sinks are capturable (structured logging frameworks are
statically observable) and must retain their `LogMessage` sink kind.

This fixture is the **control** for fix C (stdout-vs-log sub-case) in
[RIPR-SPEC-0096](../../docs/specs/RIPR-SPEC-0096-infect-propagate-fail-closed.md):
only bare `println!`/`eprintln!` are downgraded, not `log::` or `tracing::`.

## Must Not

- Report `propagation_unknown` for `log::info!` calls.
- Use mutation-runtime outcome vocabulary.
