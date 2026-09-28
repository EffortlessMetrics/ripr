# Fixture: rust_dep_edge_cross_crate_ambiguity

Spec: RIPR-SPEC-0172

## Given

The RIPR-SPEC-0172 ambiguity shape with the diff on the OTHER crate: `edge_a`
and `edge_b` both define `pub fn score`, `edge_c` path-depends on exactly one
of them (`edge_a`, transitively reaching `edge_b` through `edge_a`), and
`edge_c`'s integration test imports `edge_a::score` at file level and asserts
that owner's return value exactly. The diff changes `edge_b::score`'s return
expression, so the probe's owner is the crate the test crate has no dependency
edge to: the test's strong oracle is tied to the other crate's same-named
owner.

## When

```bash
cargo xtask fixtures rust_dep_edge_cross_crate_ambiguity
```

or:

```bash
ripr check --root fixtures/rust_dep_edge_cross_crate_ambiguity/input --diff fixtures/rust_dep_edge_cross_crate_ambiguity/diff.patch --mode fast
```

## Then

`ripr` must keep `edge_c`'s test uncredited for the `edge_b::score` probe: the
test's package declares no forward path dependency on the owner's manifest
(its captured callable edge points at `edge_a`, whose same-named definition is
an equally plausible competing candidate for the bare call), so the
RIPR-SPEC-0172 boundaries refuse the admit and the finding stays
non-promoted with no related test evidence.

## Must Not

- Credit `edge_c`'s `edge_a`-tied oracle as related evidence for the
  `edge_b::score` probe.
- Admit through the transitive `edge_c -> edge_a -> edge_b` route; only a
  direct forward declaration participates.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
