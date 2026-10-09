# Fixture: owner_return_pin_macro_return

Spec: RIPR-SPEC-0197

## Given

`weight(input)` changes from `input * 2` to `input * 3`. The `macro_return`
control contains an assertion that is skipped despite a syntactically
matching owner call. This is an independent-review counterexample for the
same owner-pin execution admission boundary.

## When

`cargo xtask fixtures owner_return_pin_macro_return` analyzes the fixture.
`cargo test -p ripr --test owner_pin_execution` compiles the actual test
against correct and deliberately wrong library implementations.

## Then

One return-value finding remains `reachable_unrevealed`. Both library variants
compile and pass exactly one test; the wrong value is never compared.

## Must Not

- Treat a same-named call outside a closure binding's live scope as its invocation.
- Ignore escapes hidden in opaque macro operands or unknown macro expansion.
- Count compilation failure or zero subjects as a behavioral red witness.
