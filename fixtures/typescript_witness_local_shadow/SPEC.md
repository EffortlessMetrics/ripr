# Fixture: typescript_witness_local_shadow

Spec: RIPR-SPEC-0027

## Given

The discount predicate changes from `total > 100` to `total >= 100`.

The test body declares its own `function applyDiscount(total): number`
returning `42` and asserts `expect(applyDiscount(100)).toBe(42)`. The
changed owner never executes; the call resolves to the body-local
declaration (issue #4102 shape 4).

## When

```bash
cargo xtask fixtures typescript_witness_local_shadow
```

or:

```bash
ripr check \
  --root fixtures/typescript_witness_local_shadow/input \
  --diff fixtures/typescript_witness_local_shadow/diff.patch \
  --mode fast --format json
```

## Then

The relation and witness layers must treat a test body that declares
the owner's name as calling the shadow, not the imported owner: the
direct-owner-call relation is not credited, the boundary witness is
not reached, and the classification fails closed to `no_static_path`.

## Must Not

- Credit a DirectOwnerCall relation for the shadowed name.
- Classify the changed predicate as `exposed` or `weakly_exposed` from
  a call that never reaches the changed owner.
- Confuse this declaration guard with the import-alias shadow guard it
  extends.
