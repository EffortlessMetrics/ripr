# Fixture: typescript_negated_t_oracle

Spec: RIPR-SPEC-0085
Spec: RIPR-SPEC-0108

## Given

A TypeScript production module changes its discount predicate from:

```ts
amount > threshold
```

to:

```ts
amount >= threshold
```

The related execution-context tests reach `applyDiscount(...)`, but they use
negated equality assertions:

```ts
t.not(...)
t.notDeepEqual(...)
```

The fixture workspace enables the TypeScript preview adapter via `ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
cargo xtask fixtures typescript_negated_t_oracle
```

or:

```bash
ripr check \
  --root fixtures/typescript_negated_t_oracle/input \
  --diff fixtures/typescript_negated_t_oracle/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `applyDiscount` owner in `src/discount.ts`,
- finds the related execution-context tests in `tests/discount.test.ts`,
- classifies `t.not(...)` and `t.notDeepEqual(...)` as
  `relational_check` with weak strength,
- keeps the finding preview-labeled and `weakly_exposed`.

## Must Not

- Treat negated equality as `exact_value` evidence.
- Promote the finding to `exposed`.
- Claim runtime mutation confidence or TypeScript support-tier promotion.
