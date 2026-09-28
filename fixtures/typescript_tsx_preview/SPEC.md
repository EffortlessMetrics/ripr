# Fixture: typescript_tsx_preview

Spec: RIPR-SPEC-0027

## Given

A TypeScript React-style `.tsx` production module changes a predicate from
`amount > threshold` to `amount >= threshold`.

The fixture workspace enables the TypeScript preview adapter:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
cargo xtask fixtures typescript_tsx_preview
```

or:

```bash
ripr check \
  --root fixtures/typescript_tsx_preview/input \
  --diff fixtures/typescript_tsx_preview/diff.patch \
  --mode fast
```

## Then

The TypeScript-family adapter parses the `.tsx` source and `.test.tsx`
test file, labels the finding as `language = "typescript"` with
`language_status = "preview"`, and keeps the evidence advisory.

## Must Not

- Treat TSX support as React semantic analysis.
- Claim TypeScript parity with Rust.
- Promote preview evidence into gates, badges, baselines, or RIPR Zero.
