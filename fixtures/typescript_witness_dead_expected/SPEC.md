# Fixture: typescript_witness_dead_expected

Spec: RIPR-SPEC-0027

## Given

The discount predicate changes from `total > 100` to `total >= 100`.
The changed behavior returns `total * 0.9` at the boundary input
(`90` for input `100`); the unchanged behavior returns `total` (`100`).

The related test asserts `expect(applyDiscount(100)).toBe(999)` — an
expected value matching neither behavior, statically determinable from
the owner's own source (issue #4102 shape 5a).

## When

```bash
cargo xtask fixtures typescript_witness_dead_expected
```

or:

```bash
ripr check \
  --root fixtures/typescript_witness_dead_expected/input \
  --diff fixtures/typescript_witness_dead_expected/diff.patch \
  --mode fast --format json
```

## Then

A dead expected value — one that matches neither the changed
behavior's value at the boundary input nor, when checkable, differs
from the unchanged behavior's — cannot witness. The classification
downgrades to `weakly_exposed` and names the missing discriminator.

## Must Not

- Classify the changed predicate as `exposed` from a dead expectation.
- Claim the assertion observed the changed value pass.
- Attempt the liveness fold when the owner source or branch shapes are
  not statically resolvable (the check fails closed to `None`).
