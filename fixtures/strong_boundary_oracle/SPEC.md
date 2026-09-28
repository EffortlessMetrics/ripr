# Fixture: strong_boundary_oracle

Spec: RIPR-SPEC-0002

## Given

Production code changes the discount predicate from `>` to `>=`, and related
tests include an exact assertion for `amount == discount_threshold`.

## When

```bash
cargo xtask fixtures strong_boundary_oracle
```

or:

```bash
ripr check --root fixtures/strong_boundary_oracle/input --diff fixtures/strong_boundary_oracle/diff.patch --mode fast
```

## Then

`ripr` should report stronger evidence than `boundary_gap` because the equality
boundary discriminator is present.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Report the equality boundary as missing.
- Degrade the exact boundary assertion into a broad or smoke oracle.
