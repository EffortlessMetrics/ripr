# Fixture: typescript_witness_receiver_qualified

Spec: RIPR-SPEC-0027

## Given

The discount predicate changes from `total > 100` to `total >= 100`.

The test declares a local object carrying a same-named method
(`const pricing = { applyDiscount: (t) => t * 2 }`) and asserts
`expect(pricing.applyDiscount(100)).toBe(200)`. A real owner call is
present elsewhere in the body but off the boundary
(`applyDiscount(150)`). The receiver-qualified call resolves to the
local shadow, not the changed owner (issue #4102 shape 3).

## When

```bash
cargo xtask fixtures typescript_witness_receiver_qualified
```

or:

```bash
ripr check \
  --root fixtures/typescript_witness_receiver_qualified/input \
  --diff fixtures/typescript_witness_receiver_qualified/diff.patch \
  --mode fast --format json
```

## Then

A receiver-qualified same-name call witnesses only when the receiver
binds to the owner's own module (a namespace import of the owner file).
`pricing` is a locally declared object, so the assertion never
witnesses; the finding stays at `weakly_exposed`.

## Must Not

- Credit `pricing.applyDiscount(100)` as an owner call at the boundary.
- Classify the changed predicate as `exposed`.
- Confuse this receiver with the namespace-import control
  (`w15`-shaped `import * as pricing`), which still witnesses.
