# Fixture: typescript_witness_dead_argument

Spec: RIPR-SPEC-0027

## Given

The discount predicate changes from `total > 100` to `total >= 100`.
The owner `applyDiscount(total: number)` reads one parameter.

The related test passes the boundary literal `100` in a second argument
the single-parameter owner never reads (`applyDiscount(150, 100)`), so
the comparison input is `150` under both behaviors and the expected
value `135` is identical under both behaviors (issue #4102 shape 1).

## When

```bash
cargo xtask fixtures typescript_witness_dead_argument
```

or:

```bash
ripr check \
  --root fixtures/typescript_witness_dead_argument/input \
  --diff fixtures/typescript_witness_dead_argument/diff.patch \
  --mode fast --format json
```

## Then

The boundary witness must not credit this assertion as observing the
changed boundary: the literal sits at an argument position the changed
comparison cannot read, so the classification downgrades to
`weakly_exposed` and the finding names the missing discriminator
(`total == 100`) instead of `already_observed`.

## Must Not

- Classify the changed predicate as `exposed` from an unread argument.
- Claim the assertion already observed the changed sink.
- Promote the static witness to mutation-runtime vocabulary.
