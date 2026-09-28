# Fixture: binding_predicate_equality_boundary

Spec: RIPR-SPEC-0157

## Given

The #3215 equality-boundary shape: the diff changes the `end` binding's
initializer (`map_or` default 0 -> 1) while the same function's
`if end == start` predicate observes the boundary, and an exact-value
test reaches the owner from both sides of the boundary.

## When

```bash
cargo xtask fixtures binding_predicate_equality_boundary
```

## Then

The changed binding retargets to its exact predicate use: one
predicate-family probe at the `if end == start` line whose before/after
are the old/new initializers, evidence carrying
`binding_predicate_relation` and the earliest unresolved initializer
operation `.rfind(`, and a normal predicate-shaped classification for
the reachable change.

## Must Not

- Emit the generic `changed syntax is not mapped to a high-confidence
  probe family` limitation for this supported direct binding use.
- Claim the operand values are evaluated or resolved; the limitation
  line names the unresolved edge instead.
- Emit a repair instruction or repair-packet readiness from the
  relation alone.
