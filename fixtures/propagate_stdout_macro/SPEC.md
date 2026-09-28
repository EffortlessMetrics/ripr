# Fixture: propagate_stdout_macro

Spec: RIPR-SPEC-0096

## Given

Production code changes the multiplier in a `println!` macro call:

```rust
pub fn report(amount: i32) {
    println!("amount is {}", amount * 9);
}
```

`println!` writes to stdout which is not a statically-capturable,
observable sink from ripr's perspective.  The test only checks that the
function does not panic (smoke oracle):

```rust
#[test]
fn report_does_not_panic() {
    report(5);
}
```

## When

```bash
cargo xtask fixtures propagate_stdout_macro
```

or:

```bash
ripr check --root fixtures/propagate_stdout_macro/input --diff fixtures/propagate_stdout_macro/diff.patch --mode fast
```

## Then

`ripr` must emit `propagation_unknown` (not `exposed`) for the
`println!(...)` probe.  Stdout macros are non-capturable and must route to
`FlowSinkKind::Unknown` so propagation is statically unknown.

This fixture is the **before/after proof** for fix C (stdout macro
sub-case) introduced in
[RIPR-SPEC-0096](../../docs/specs/RIPR-SPEC-0096-infect-propagate-fail-closed.md).

## Must Not

- Use mutation-runtime outcome vocabulary.
- Report `exposed` when the only effect is a `println!` or `eprintln!` call.
- Downgrade `log::info!` or `tracing::info!` macros (capturable sink).
