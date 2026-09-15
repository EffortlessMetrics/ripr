# Fixture: unsafe_boundary_probe

Spec: RIPR-SPEC-0168

## Given

The #3536 unsafe-boundary shape, with every control inside the analyzed diff:
the binding `total` changes outside the boundary while a conditional
predicate observes it, the `sum_bytes` loop body changes strictly inside an
explicit `unsafe {}` block, and `read_first`'s single-line unsafe statement
(a shared edge line) changes as a negative control.

## When

```bash
cargo xtask fixtures unsafe_boundary_probe
```

## Then

Three findings render. The interior boundary line projects the parser-backed
`unsafe_boundary` probe (expression `unsafe block`, family `static_unknown`)
deduplicated by the boundary's byte identity; the changed binding outside the
boundary keeps its ordinary `field_construction` probe; and `read_first`'s
changed statement renders its own ordinary `static_unknown` finding whose
expression is the changed statement — not an `unsafe block` boundary
projection — so a boundary probe wrongly attached to the shared edge line
would flip the recorded expression and fail the golden.

## Must Not

- Promote reach plus an oracle to `exposed` for the boundary probe; the
  `static_unknown` family short-circuits before any reach or oracle logic.
- Attach an `unsafe block` boundary projection to lines outside a boundary or
  to a boundary edge line shared with outside code.
- Suppress the ordinary probes beside the boundary context: the golden pins
  the `field_construction` probe for the changed binding.
- Change the output schema; the probe id and family strings are the already
  registered `static_unknown` values.
