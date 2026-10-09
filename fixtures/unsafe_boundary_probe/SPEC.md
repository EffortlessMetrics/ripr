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
boundary keeps its ordinary `static_unknown` probe; and `read_first`'s
changed statement renders its own ordinary `static_unknown` finding whose
expression is the changed statement — not an `unsafe block` boundary
projection — so a boundary probe wrongly attached to the shared edge line
would flip the recorded expression and fail the golden.

## Must Not

- Promote reach plus an oracle to `exposed` for the boundary probe; the
  `static_unknown` family is never credited from oracle logic. With no test
  reaching the owner, each finding is `no_static_path`.
- Attach an `unsafe block` boundary projection to lines outside a boundary or
  to a boundary edge line shared with outside code.
- Suppress the ordinary probes beside the boundary context: the changed
  binding must retain its ordinary `static_unknown` subject.
- Change the output schema; the probe id and family strings are the already
  registered `static_unknown` values.

## Causal qualification

The input is the diff's head side, including `total = 1`, `offset + 1`, and
`pointer.add(0)`. The original expectation predates the distinction between
a type annotation and record construction: `let mut total: u8 = 1` is not a
field construction. The shared simple-binding parser does not admit a
`name: Type` annotation.
Independently, the loop reassigns `total` before its predicate use, which also
prevents direct initializer retargeting. The scalar change remains visible
as `static_unknown`. Separate controls retain unknown for either limit,
while an unannotated binding without reassignment permits predicate
retargeting. Original golden files are retained for review, not used as authority for the obsolete
field-construction expectation.

Unsafe blocks execute in the owner's scope: their writes must end initializer
liveness. An `unsafe fn` item has its own bindings. The unannotated fixture
control distinguishes these scopes and prevents treating a loop write as
nested-item content. Compact and line-broken unsafe blocks retain the same
write/refusal rule.
