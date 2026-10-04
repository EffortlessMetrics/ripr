# Fixture: ts_predicate_boundary_overcredit

Spec: RIPR-SPEC-0027

## Given

A TypeScript owner `applyDiscount(total)` changes its predicate boundary
(`total > 100` becomes `total >= 100`). The related test calls
`applyDiscount(150, 100)` and asserts an exact value — but the boundary
literal `100` sits in a SECOND argument position the single-parameter owner
never reads (issue #4102 shape 1: dead argument position).

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
ripr check \
  --root fixtures/ts_predicate_boundary_overcredit/input \
  --diff fixtures/ts_predicate_boundary_overcredit/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `applyDiscount` owner with parameter facts (`total`) in
  `src/pricing.ts`,
- finds the related test in `tests/pricing.test.ts`,
- refuses to witness the changed boundary `total == 100` from the dead
  argument position (`applyDiscount(150, 100)`),
- keeps the finding `weakly_exposed` and names the boundary discriminator
  `total == 100` as missing proof.

## Must Not

- Promote the finding to `exposed` on a boundary literal parked in an
  argument the owner never reads.
- Treat argument-position liveness as optional when owner parameter facts
  are available.
