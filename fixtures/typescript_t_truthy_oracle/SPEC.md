# Fixture: typescript_t_truthy_oracle

Spec: RIPR-SPEC-0085
Spec: RIPR-SPEC-0108

## Given

A TypeScript production module changes a return expression in `score(...)`.
The related test reaches the owner but uses only a truthiness assertion:

```ts
t.truthy(actual)
```

The fixture workspace enables the TypeScript preview adapter through
`ripr.toml`.

## When

```bash
cargo xtask fixtures typescript_t_truthy_oracle
```

or:

```bash
ripr check \
  --root fixtures/typescript_t_truthy_oracle/input \
  --diff fixtures/typescript_t_truthy_oracle/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `score` owner in `src/score.ts`,
- finds the related execution-context test in `tests/score.test.ts`,
- classifies `t.truthy(...)` as smoke-only evidence,
- keeps the finding non-promoted and non-packet-ready.

## Must Not

- Treat truthiness as exact-value evidence.
- Promote the finding to `exposed`.
- Emit verify or receipt handoff authority for this incomplete packet.
