# Fixture: rust_dep_edge_cross_crate_positive

Spec: RIPR-SPEC-0172

## Given

A Cargo workspace where `edge_a` and `edge_b` both define `pub fn score`, so the
owner name is ambiguous across crates, and `edge_c` path-depends on exactly one
of them (`edge_a`). `edge_c`'s integration test imports `edge_a::score` at file
level and asserts the changed return value exactly, while `edge_b`'s own smoke
test calls its same-named local definition. The diff changes `edge_a::score`'s
return expression, so the probe's owner is the crate the test crate is
edge-linked to. The forward `edge_c -> edge_a` dependency declaration and the
file-level import are the identity evidence RIPR-SPEC-0172's admit requires.

## When

```bash
cargo xtask fixtures rust_dep_edge_cross_crate_positive
```

or:

```bash
ripr check --root fixtures/rust_dep_edge_cross_crate_positive/input --diff fixtures/rust_dep_edge_cross_crate_positive/diff.patch --mode fast
```

## Then

`ripr` should credit `edge_c`'s edge-linked dependent test as related evidence
for the `edge_a::score` probe (cross-crate ambiguous-name admit through one
captured callable forward path-dependency edge with a file-level `use`), and
the strong exact-value oracle observing the changed sink classifies the finding
`exposed`. `edge_b`'s smoke test must stay filtered: its own package defines a
same-named `score`, so the bare call is a local shadow, not identity evidence.

## Must Not

- Credit `edge_b`'s same-named smoke test as related evidence for the
  `edge_a::score` probe.
- Admit on the owner-name match alone without the captured callable edge and
  import evidence.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
