# Fixture: owner_return_pin_called_closure

Spec: RIPR-SPEC-0197

## Given

The return of `weight(input)` changes from `input * 2` to `input * 3`.
The `called_closure` test is one cell of the matched owner-pin execution corpus.

## When

`cargo xtask fixtures owner_return_pin_called_closure` runs the public analyzer.
`cargo test -p ripr --test owner_pin_execution` separately compiles and runs
these exact fixture sources against both correct and deliberately wrong libraries.

## Then

Exactly one return-value finding reads `exposed`. The runtime test passes
for the correct library. With the wrong library it fails.
Runtime outcomes are independent controls, not claims made by the static analyzer.

## Must Not

- Accept a zero-finding, zero-test or compilation failure as a behavioral witness.
- Credit an uncalled closure or shadowed macro as an owner-return discriminator.
- Disable the direct or directly invoked closure positive controls.
