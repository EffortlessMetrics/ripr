# Fixture: typescript_broad_tothrow

Spec: RIPR-SPEC-0027

## Given

A TypeScript production function changes an error-path condition. The
related test reaches the changed owner, but the assertion is a bare
Jest/Vitest `toThrow()` matcher without a class, message, regex, or
payload.

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
cargo xtask fixtures typescript_broad_tothrow
```

or:

```bash
ripr check \
  --root fixtures/typescript_broad_tothrow/input \
  --diff fixtures/typescript_broad_tothrow/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `parseUser` owner in `src/parser.ts`,
- finds the related test in `tests/parser.test.ts`,
- classifies the bare `toThrow()` assertion as `broad_error` with weak
  strength,
- keeps the finding preview-labeled and advisory.

## Must Not

- Surface bare `toThrow()` as exact error-variant evidence.
- Claim runtime mutation confidence or Rust parity.
- Add VS Code selectors or LSP routing.
