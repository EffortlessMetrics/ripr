# Fixture: typescript_jest_vitest_assertion_facts

Spec: RIPR-SPEC-0027

## Given

A TypeScript preview workspace changes several function-level behaviors and
opts in to the TypeScript-family adapter:

```toml
[languages]
enabled = ["rust", "typescript"]
```

The related Vitest/Jest-style tests cover nested `describe(...)`,
`test.each(...)`, `it.each(...)`, exact-value assertions, async
`resolves`, mock interaction assertions, bounded mock payload assertions,
snapshot assertions, and smoke-only truthiness assertions.

## When

```bash
cargo xtask fixtures typescript_jest_vitest_assertion_facts
```

or:

```bash
ripr check \
  --root fixtures/typescript_jest_vitest_assertion_facts/input \
  --diff fixtures/typescript_jest_vitest_assertion_facts/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- discovers tests nested below `describe(...)` blocks;
- discovers array-form `test.each(...)` and `it.each(...)` calls;
- maps `toStrictEqual`, `toBe`, and `resolves.toBe` to exact-value oracles;
- maps `toHaveBeenCalledWith` to a mock-interaction oracle;
- names literal mock payload evidence when the callee and expected argument are
  syntax-bounded;
- keeps snapshot and smoke-only assertions weak instead of treating them as
  strong discriminators;
- preserves `language = "typescript"`, `language_status = "preview"`, and
  `owner_kind = "function"` on the findings.

## Must Not

- Invoke `tsc`, `tsserver`, Vitest, Jest, a package graph, or mutation runtime.
- Promote TypeScript/JavaScript beyond opt-in preview.
- Treat snapshot or smoke-only assertions as strong repair proof.
- Change default gates, badges, baselines, branch protection, or RIPR Zero.
