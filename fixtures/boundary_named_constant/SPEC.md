# Fixture: boundary_named_constant

Spec: RIPR-SPEC-0001

## Given

Three changed threshold predicates (`>` becomes `>=`) whose boundary is a
named constant rather than a literal:

- `discounted_total`: `amount >= DISCOUNT_THRESHOLD`, where
  `pub const DISCOUNT_THRESHOLD: u64 = 10_000;` is declared in the owner's
  file. A related test calls `discounted_total(10_000)`.
- `bulk_rate`: `items >= BULK_ITEMS`, where
  `pub const BULK_ITEMS: u32 = 5 * 2;` has a computed initializer. A related
  test calls `bulk_rate(parcels::BULK_ITEMS)`.
- `shipping`: `amount >= FREE_SHIPPING`, where `FREE_SHIPPING` is declared
  in `src/config.rs` and imported into the owner's file. A related test calls
  `shipping(5_000)`.

The diff has no blank context lines (`git diff --check` hygiene).

## When

```bash
cargo xtask fixtures boundary_named_constant
```

or:

```bash
ripr check --root fixtures/boundary_named_constant/input \
           --diff fixtures/boundary_named_constant/diff.patch \
           --mode fast
```

## Then

- `discounted_total` is `exposed`: the same-file integer constant resolves
  to `10_000`, so the related test's `10_000` input is at the equality
  boundary.
- `bulk_rate` is `exposed`: the constant's value is not a plain literal, but
  the test argument names the constant itself, which is the boundary value by
  identity.
- `shipping` is `infection_unknown`: the constant is not declared in the
  owner's file, so ripr says it cannot see the constant's value and does not
  name a missing discriminator it could never confirm.

## Must Not

- Leave a boundary test at a same-file integer constant's value reported as a
  missing equality-boundary discriminator.
- Ignore a test argument that names the boundary constant.
- Ask for a missing discriminator on a constant whose value ripr cannot see.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
