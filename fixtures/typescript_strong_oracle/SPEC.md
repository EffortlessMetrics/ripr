# Fixture: typescript_strong_oracle

Spec: RIPR-SPEC-0027

## Given

A TypeScript production module changes its discount predicate from:

```ts
amount > threshold
```

to:

```ts
amount >= threshold
```

A Jest-style test asserts the result of `applyDiscount(...)` with an
exact-value matcher (`expect(...).toBe(...)`).

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml` so the adapter actually runs on this fixture:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
cargo xtask fixtures typescript_strong_oracle
```

or:

```bash
ripr check \
  --root fixtures/typescript_strong_oracle/input \
  --diff fixtures/typescript_strong_oracle/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `applyDiscount` owner in `src/discount.ts`,
- finds the `test('...')` calls in `tests/discount.test.ts` that
  reference `applyDiscount(`,
- extracts the `expect(applyDiscount(...)).toBe(...)` assertion and
  maps `toBe` to an exact-value oracle of `strong` strength (#767),
- classifies the changed predicate line as `exposed` because at least
  one related test carries a strong oracle.

The finding carries `language = "typescript"` and
`language_status = "preview"` per RIPR-SPEC-0026.

This is the strong-oracle gradient companion to
`typescript_boundary_gap`. Together they pin both ends of the current
TypeScript preview classifier: `weakly_exposed` when only raw control
flow asserts and `exposed` when an `expect`-style exact-value oracle
is present.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation
  execution.
- Claim infection or propagation are statically known on the
  TypeScript adapter.
- Report a strength higher than `strong` for `toBe` — `Strong` is the
  ceiling for the current preview adapter.
