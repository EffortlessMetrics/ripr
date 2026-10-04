# Fixture: typescript_witness_nested_expression

Spec: RIPR-SPEC-0027

## Given

The discount predicate changes from `total > 100` to `total >= 100`.

The related test wraps the boundary literal in a larger expression:
`applyDiscount(price + 100)` with `price = 60`, so the effective input
is `160` — not the changed boundary `100`. The token-split containment
the witness previously used would credit the contained `100` token
(issue #4102 shape 2).

## When

```bash
cargo xtask fixtures typescript_witness_nested_expression
```

or:

```bash
ripr check \
  --root fixtures/typescript_witness_nested_expression/input \
  --diff fixtures/typescript_witness_nested_expression/diff.patch \
  --mode fast --format json
```

## Then

The boundary witness must not credit a literal contained inside a
larger argument expression: only an argument that stands alone as the
literal value (or an object-literal field pinning the comparison
operand to it) can witness. The classification downgrades to
`weakly_exposed` and names the missing discriminator.

## Must Not

- Treat `price + 100` as carrying the boundary value `100`.
- Classify the changed predicate as `exposed` from nested containment.
- Claim the effective input is statically known.
