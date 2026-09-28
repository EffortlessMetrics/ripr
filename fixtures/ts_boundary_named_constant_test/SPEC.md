# Fixture: ts_boundary_named_constant_test

Spec: RIPR-SPEC-0027

## Given

Two TypeScript owners whose changed predicate boundaries name constants, so
the related tests' boundary inputs are constant identifiers rather than
literals (issue #4104 E3, mirroring the Rust
`analysis/value_resolution.rs::named_constant` resolution):

- `applyDiscount`: `total > DISCOUNT_THRESHOLD` becomes
  `total >= DISCOUNT_THRESHOLD`, where `DISCOUNT_THRESHOLD` is
  `export const DISCOUNT_THRESHOLD = 100;` in the owner's own module. The
  related test imports the constant from that module and passes it back:
  `applyDiscount(DISCOUNT_THRESHOLD)`. Entity identity runs through the
  import record to the declaring module; the boundary operand resolves to
  `100` and the argument substitutes to the same value.
- `freeShipping`: `items > 5` becomes `items >= 5` (a literal boundary), and
  the related test declares `const BULK_ITEMS = 5;` in the test body and
  passes it: `freeShipping(BULK_ITEMS)`. A single immutable integer `const`
  in the test body resolves the argument to the boundary value.

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
ripr check \
  --root fixtures/ts_boundary_named_constant_test/input \
  --diff fixtures/ts_boundary_named_constant_test/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds both owners with parameter facts in `src/pricing.ts`,
- finds both related tests (direct owner calls),
- resolves the boundary constant for `applyDiscount` from the owner's own
  module and the argument constant through the import record,
- resolves `BULK_ITEMS` from the single immutable test-body declaration,
- keeps both findings `exposed` with no missing boundary discriminator.

## Must Not

- Resolve a `let`/`var` binding, a computed initializer (`50 * 2`), a
  constant declared more than once, or an off-value constant (the input must
  still equal the changed comparison's boundary value).
- Resolve a constant imported from a module that is not the owner's own
  module (the declaring module is unknown to the adapter; it stays
  fail-closed rather than guessed).
- Credit a test-file module-level constant that is neither in the test body
  nor imported from the owner's module.
