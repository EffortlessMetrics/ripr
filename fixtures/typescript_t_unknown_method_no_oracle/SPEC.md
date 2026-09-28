# Fixture: typescript_t_unknown_method_no_oracle

Spec: RIPR-SPEC-0085
Spec: RIPR-SPEC-0108

## Given

A TypeScript production module changes a return expression in `score(...)`.
The related test reaches the owner, but the execution-context method is not in
RIPR's supported assertion vocabulary:

```ts
t.frobnicate(actual, expected)
```

The fixture workspace enables the TypeScript preview adapter through
`ripr.toml`.

## When

```bash
cargo xtask fixtures typescript_t_unknown_method_no_oracle
```

or:

```bash
ripr check \
  --root fixtures/typescript_t_unknown_method_no_oracle/input \
  --diff fixtures/typescript_t_unknown_method_no_oracle/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `score` owner in `src/score.ts`,
- finds a related test by owner call,
- does not credit `t.frobnicate(...)` as an oracle,
- keeps the finding non-promoted and non-packet-ready.

## Must Not

- Infer semantics for an unknown `t.*` method.
- Promote the finding to `exposed`.
- Emit a repair packet or receipt command.
