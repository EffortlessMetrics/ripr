# Fixture: boundary_discriminator

Spec: RIPR-SPEC-0001

## Given

Production code changes the discount predicate from:

```rust
amount > discount_threshold
```

to:

```rust
amount >= discount_threshold
```

Related tests include explicit coverage for `amount == discount_threshold` and
assert the discounted value at that boundary.

## When

```bash
cargo xtask fixtures boundary_discriminator
```

or:

```bash
ripr check --root fixtures/boundary_discriminator/input --diff fixtures/boundary_discriminator/diff.patch --mode fast
```

## Then

`ripr` records the static exposure classification for the changed predicate and
retains evidence that the equality boundary has a concrete discriminator.

The current expected output is a baseline for future analyzer improvements. If a
later PR improves the classification, bless the changed output with a reason
that cites the relevant spec.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Claim the equality boundary is missing when tests assert it directly.
- Hide the observed equality-boundary discriminator evidence.
