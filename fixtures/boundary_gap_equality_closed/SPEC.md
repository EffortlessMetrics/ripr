# Fixture: boundary_gap_equality_closed

Spec: RIPR-SPEC-0001

Related: #1429; paired control `boundary_gap`; limitation case
`boundary_operand_unresolved`.

## Given

The same production change as `boundary_gap` (the discount predicate
moves from `amount > discount_threshold` to
`amount >= discount_threshold`), but the suite already contains the
exact equality assertion (`exact_threshold_discounts`:
`discounted_total(100, 100) == 90`).

## When

```bash
cargo xtask fixtures boundary_gap_equality_closed
```

or:

```bash
ripr check --root fixtures/boundary_gap_equality_closed/input --diff fixtures/boundary_gap_equality_closed/diff.patch --mode fast
```

## Then

The same canonical probe item as `boundary_gap`
(`probe:src_lib.rs:predicate:c80557eb`) is `exposed` instead of
`weakly_exposed`: adding the exact equality discriminator closes the
item the gap fixture leaves open. No boundary-test prescription
remains for this item.

## Must Not

- Report the item as `weakly_exposed` with a missing-equality claim
  once the exact assertion observes the boundary.
- Credit a different probe or a renamed item: the probe id must match
  `boundary_gap` exactly, proving the same item improved.
- Promote any unrelated finding to `exposed` on the strength of this
  assertion.

## Paired control

`boundary_gap` (same diff, equality assertion absent) keeps the
`weakly_exposed` gap and its satisfiable bounded repair. Together the
pair proves the discriminator — not the analyzer's mood — moves the
item.
