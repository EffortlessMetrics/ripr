# Fixture: rust_proptest_quickcheck_tests

Spec: RIPR-SPEC-0001

## Given

A Rust crate whose only discriminating tests live inside property-test
macros:

- `proptest! { #[test] fn gate_threshold(x in 0u32..100) { prop_assert_eq!(gate(x), x > 10); } }`
- an unmarked `proptest!` helper in the same block
- `quickcheck! { fn qc_gate(x: u32) -> bool { gate(x) == (x > 10) } }`

The changed owner is `gate`, whose predicate is `x > 10`.

## When

```bash
cargo xtask fixtures rust_proptest_quickcheck_tests
```

or:

```bash
ripr check --root fixtures/rust_proptest_quickcheck_tests/input --diff fixtures/rust_proptest_quickcheck_tests/diff.patch --mode fast
```

The diff rewrites `x > 10` to `x >= 10`.

## Then

The parser indexes the `#[test]` proptest fn and the quickcheck fn with
their real lines, the `gate` call, and the `prop_assert_eq!` oracle. The
finding relates `gate_threshold` (and `qc_gate`). The unmarked helper is
not a related test.

## Must Not

- Promote `exposed` from a property-macro block that has no owner call
  and no oracle.
- Treat the unmarked `proptest!` fn as an executable test.
- Invent tests from comments, strings, or lookalike macros.
