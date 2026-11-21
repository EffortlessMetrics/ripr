# Fixture: typescript_t_wrong_receiver_no_oracle

Spec: RIPR-SPEC-0085
Spec: RIPR-SPEC-0108

## Given

A TypeScript production module changes a return expression in `score(...)`.
The related test reaches the owner, but the apparent `is(...)` assertion is on a
helper object, not on the test callback execution context:

```ts
helper.is(actual, expected)
```

The fixture workspace enables the TypeScript preview adapter through
`ripr.toml`.

## When

```bash
cargo xtask fixtures typescript_t_wrong_receiver_no_oracle
```

or:

```bash
ripr check \
  --root fixtures/typescript_t_wrong_receiver_no_oracle/input \
  --diff fixtures/typescript_t_wrong_receiver_no_oracle/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `score` owner in `src/score.ts`,
- finds a related test by owner call,
- does not credit `helper.is(...)` as an execution-context assertion,
- keeps the finding non-promoted and non-packet-ready.

## Must Not

- Credit wrong-receiver `helper.is(...)` as `t.is(...)`.
- Promote the finding to `exposed`.
- Emit a repair packet or receipt command.
