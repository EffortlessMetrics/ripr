# Fixture: typescript_tothrow_exact_oracle

Spec: RIPR-SPEC-0097

## Given

A TypeScript production function changes an error-path condition. The
related tests each use a different exact-payload form of `toThrow`:

- `toThrow("exact message")` -- string literal
- `toThrow({ code: "EMPTY_INPUT" })` -- all-literal object
- `toThrow(ParseError)` -- PascalCase class reference

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
cargo xtask fixtures typescript_tothrow_exact_oracle
```

or:

```bash
ripr check \
  --root fixtures/typescript_tothrow_exact_oracle/input \
  --diff fixtures/typescript_tothrow_exact_oracle/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `parseUser` owner in `src/parser.ts`,
- finds the related tests in `tests/parser.test.ts`,
- extracts ExactErrorVariant / strong oracle from each test because all
  three `toThrow` forms carry a concrete, in-source exact payload,
- classifies the changed predicate as `exposed` (not `weakly_exposed`)
  because the strongest extracted oracle is `exact_error_variant`.

This is the gradient companion to `typescript_broad_tothrow` (bare
`.toThrow()` stays `weakly_exposed`) and demonstrates the three
exact-payload upgrade paths added by RIPR-SPEC-0097.

## Controls (must NOT flip to exposed)

- `typescript_broad_tothrow`: bare `.toThrow()` with no argument stays
  `weakly_exposed` (BroadError / weak oracle).
- `.toThrow(message)` where `message` is a camelCase identifier stays
  weak (fail-closed: cannot confirm it is a class reference, not a
  variable).
- `.toThrow({ code })` with a shorthand property (non-literal value)
  stays weak (fail-closed: shorthand properties are not all-literal).

## Must Not

- Upgrade bare `.toThrow()` or `.toThrow(dynamicVar)` to ExactErrorVariant.
- Claim runtime mutation confidence or Rust parity.
- Emit an actionable repair packet (TS stays preview-tier advisory).
