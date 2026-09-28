# Fixture: recursive_controls

Spec: RIPR-SPEC-0165

## Given

Four caller-side equality boundaries over recursive or nested-call
helpers that each break one rule of the bounded evaluation: a true
cycle (`_ => label_cycle(kind)` re-enters the same bound state), a
chain beyond the hop bound (`d -> c -> b -> a` needs four evaluations;
the explicit bound is three — the exhaustive `_ => "other"` arm exists
for compilation and no test row reaches it), a computed nested
argument (`label_computed(fix(kind))`), and a non-unique nested callee
(`resolve("x")` with `resolve` defined in two modules). The diff
changes each comparison constant (`"alpha"` -> `"other"`).

## When

```bash
cargo xtask fixtures recursive_controls
```

## Then

Every probe stays `weakly_exposed`: the bounded context refuses each
variant by rule — a repeated state is a true cycle, the fourth
evaluation exceeds the explicit bound, a computed argument never
binds, and a non-unique callee never resolves.

## Must Not

- Produce any `exposed` finding or any hop-provenance string on any
  of the four probes.
- Treat the refusal as a missing test: the limitation is the
  evaluator's named bound, not absent user proof.
