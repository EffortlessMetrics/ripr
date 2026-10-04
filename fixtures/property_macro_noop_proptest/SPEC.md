# Fixture: property_macro_noop_proptest

Spec: RIPR-SPEC-0001

## Given

A changed threshold owner and a locally defined no-op property macro.

## When

The public analyzer reads the `>` to `>=` diff.
`crates/ripr/tests/property_macro_quarantine.rs` requires these exact source
bytes, then compiles and runs both correct and wrong owner variants.

## Then

One predicate finding remains `no_static_path`, with no oracle and an explicit macro
limitation. Human guidance is static-limited and does not request a new test.
The runtime harness collects 0 tests and returns success on both owner
variants. This is a deliberately non-discriminating negative, not an adequacy
claim. The ordinary assertion positive fails against the wrong owner.

## Must Not

- Promote macro spelling into strong evidence or `exposed`
- Treat zero-test success or a compiler error as useful test execution
- Erase the named limitation or offer unsupported new-test work
