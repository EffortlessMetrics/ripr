# Fixture: boundary_operand_unresolved

Spec: RIPR-SPEC-0158

Related: #1429; family RIPR-SPEC-0157 (binding-predicate equality boundary).

## Given

The historical #1429 shape: the diff adds an `end == start` equality arm
whose operands are computed locals (`rest.rfind(close)?` and
`open.len_utf8()`) the bounded evaluator cannot resolve. The suite already
contains the equality test (`empty_body_equality_case`: `quote_body("[]",
'[', ']') == Some("")`), but static evaluation cannot connect that input
through `rfind`/`len_utf8` to the boundary.

## When

```bash
cargo xtask fixtures boundary_operand_unresolved
```

## Then

Predicate findings on the equality boundary keep class `weakly_exposed`
(the gap stays visible) and carry the typed static limitation
`rust_value_propagation_unresolved` instead of a prescription to add a
boundary test the suite may already contain:

- `recommended_next_step` names the limitation and defers to real
  mutation testing; it never instructs adding `end == start` coverage.
- the missing-discriminator reason keeps the `unknown` listing and
  appends the earliest unsupported producer edge
  (`boundary operand value unresolved`).
- no finding in this fixture is `exposed`: unresolved propagation is
  never converted into static proof.

## Must Not

- Prescribe a specific boundary input (`Add boundary tests for below,
  equal, and above ...`) for an unconfirmed discriminator.
- Promote to `exposed` or demote the class to hide the gap.
- Emit the generic `changed syntax is not mapped to a high-confidence
  probe family` limitation for this supported predicate shape.

## Paired control

`boundary_gap` (direct-parameter operands, genuinely missing equality
row) keeps the plain absence claim and its satisfiable bounded repair.
