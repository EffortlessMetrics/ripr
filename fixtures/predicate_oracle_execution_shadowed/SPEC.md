# Fixture: predicate_oracle_execution_shadowed

Spec: RIPR-SPEC-0197

## Given

A changed predicate is accompanied by one test in the `shadowed` execution context.
The ErrorPath mutation swallows the reader error; the Predicate mutation excludes
an equality-boundary input. Assertions contain no diagnostic-message evidence.

## When

`cargo xtask fixtures predicate_oracle_execution_shadowed` analyzes the exact diff.
`cargo test -p ripr --test owner_pin_execution equality_oracle_family_matched_static_and_runtime_controls`
compiles and runs the fixture against the correct and deliberately wrong behavior.

## Then

Exactly one selected `predicate` finding reads
`reachable_unrevealed`. The correct implementation
passes exactly one runtime test; the wrong implementation passes that test.
ErrorPath also retains a separate deleted-base static-unknown row; it is not the
selected family and cannot satisfy the assertion.

## Must Not

- Treat a compilation failure or zero-test run as behavioral evidence.
- Credit uncalled, false-branch or shadowed equality as an executed oracle.
- Remove direct or demonstrably invoked positive evidence.
- Describe static evidence as a calibrated protection probability.
