# Fixture: binding_value_fail_closed

Spec: RIPR-SPEC-0158

## Given

Three changed `let` initializers the evaluator must refuse:
`map_or_else` (unsupported form), a non-identity `map_or` closure, and
a needle that is not an exact input (a third parameter with no observed
literal row).

## When

```bash
cargo xtask fixtures binding_value_fail_closed
```

## Then

No exact boundary observation occurs even though every function has an
exact-input test call. The `map_or` variants retarget and keep the
honest missing-discriminator weakness (`observed end values: unknown`)
because the evaluator refuses their chains (non-identity closure,
dynamic needle); the `map_or_else` shape does not retarget at all —
its `||` routes through the pre-existing parser predicate shape — and
equally produces no exact value. No `infection yes` boundary fact
appears for any of the three shapes.

## Must Not

- Evaluate `map_or_else`, a non-identity closure, or a dynamic needle.
- Promote an unevaluated chain to an exact value by token resemblance.
