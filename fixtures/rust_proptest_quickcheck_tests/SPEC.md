# Fixture: rust_proptest_quickcheck_tests

Spec: RIPR-SPEC-0001

## Given

Syntax-only Rust source with `proptest!` and `quickcheck!` token trees mentioning
`gate`. This fixture intentionally has no property-framework dependencies and
is not a runnable test suite. Macro spelling does not establish the definition,
expansion, collected tests or execution.

## When

`cargo xtask fixtures rust_proptest_quickcheck_tests` analyzes the `>` to `>=`
change through the public check pipeline.

## Then

No inner function is invented as a collected test and no `prop_assert*` spelling
receives an oracle. The finding names `rust_macro_reach_unresolved` and the
opaque source invocation, stays below `exposed`, and directs the reader to
inspect the macro and existing tests instead of treating missing static reach
as evidence that another test is needed.

## Discriminating proof

`crates/ripr/tests/property_macro_quarantine.rs` compares public static output
with compiled standalone runtime subjects: ordinary assertions distinguish the
correct and wrong owner bodies; a no-op property assertion passes both; no-op
`proptest!` / `quickcheck!` wrappers collect zero tests. Zero-test success is an
explicit negative control, never a test-adequacy claim.

Genuine framework runtime controls are separate from this syntax-only golden.
Their execution does not establish static macro provenance or restore support.

## Must Not

- Mint tests, owner reach or strong oracles from unresolved macro names
- Lose the ordinary executed-assertion positive
- Read comments, strings or lookalike macros as property invocations
- Allocate a file-sized overlay or parse an otherwise unnecessary second file
