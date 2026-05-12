# Fixture: typescript_return_value_shape

Spec: RIPR-SPEC-0027

## Given

A TypeScript production module changes its rounding strategy from:

```ts
return Math.floor(amount * 100) / 100;
```

to:

```ts
return Math.round(amount * 100) / 100;
```

A Jest-style test references `roundCents(` and asserts the result with
raw control flow (`if (result !== 1.01) throw`) — no
`expect(...).matcher(...)` chain.

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
cargo xtask fixtures typescript_return_value_shape
```

or:

```bash
ripr check \
  --root fixtures/typescript_return_value_shape/input \
  --diff fixtures/typescript_return_value_shape/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `roundCents` owner in `src/rounding.ts`,
- finds the `test('rounds two decimals', ...)` call in
  `tests/rounding.test.ts` that references `roundCents(`,
- classifies the changed-line probe family as `return_value` with
  delta `value` per the #768 probe-shape classifier (the changed line
  begins with `return ...;`),
- emits no `expect(...)` assertion, so the strongest extracted oracle
  is `unknown`,
- classifies the finding as `weakly_exposed`.

The finding carries `language = "typescript"` and
`language_status = "preview"` per RIPR-SPEC-0026.

## Must Not

- Report `probe.family = predicate` for a top-level `return ...;`
  line — that was the pre-#768 default, now refined.
- Use mutation-runtime outcome vocabulary reserved for real mutation
  execution.
- Claim infection or propagation are statically known on the
  TypeScript adapter.
