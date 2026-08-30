# Fixture: typescript_tape_equal_oracle

Spec: RIPR-SPEC-0085
Spec: RIPR-SPEC-0108

## Given

A TypeScript production module changes a return expression in `score(...)`.
The related test uses the execution-context assertion shape:

```ts
t.equal(actual, expected)
```

The fixture workspace enables the TypeScript preview adapter through
`ripr.toml`.

## When

```bash
cargo xtask fixtures typescript_tape_equal_oracle
```

or:

```bash
ripr check \
  --root fixtures/typescript_tape_equal_oracle/input \
  --diff fixtures/typescript_tape_equal_oracle/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `score` owner in `src/score.ts`,
- finds the related execution-context test in `tests/score.test.ts`,
- classifies `t.equal(...)` as `exact_value` with strong strength,
- classifies the finding as `exposed` while keeping TypeScript preview
  advisory-only and non-packet-ready.

## Must Not

- Treat `t.equal(...)` as an unknown matcher.
- Emit a repair packet for an already observed preview finding.
- Claim runtime, gate, or support-tier authority.
