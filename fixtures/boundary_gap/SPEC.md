# Fixture: boundary_gap

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

Existing tests cover a value below the threshold and a value far above the
threshold. They do not cover `amount == discount_threshold`.

## When

```bash
cargo xtask fixtures boundary_gap
```

or:

```bash
ripr check --root fixtures/boundary_gap/input --diff fixtures/boundary_gap/diff.patch --mode fast
```

## Then

`ripr` records the current static exposure classification for the changed
predicate and names the missing activation evidence around the equality
boundary.

The current expected output is a baseline for future analyzer improvements. If a
later PR improves the classification, bless the changed output with a reason
that cites the relevant spec.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Claim that the code has no tests.
- Hide the missing equality-boundary discriminator.
