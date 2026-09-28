# Fixture: mixed_rust_typescript_preview

Spec: RIPR-SPEC-0027

## Given

A mixed Rust plus TypeScript workspace enables the TypeScript preview adapter and
contains both a Rust predicate change and a TypeScript predicate change.

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
cargo xtask fixtures mixed_rust_typescript_preview
```

or:

```bash
ripr check \
  --root fixtures/mixed_rust_typescript_preview/input \
  --diff fixtures/mixed_rust_typescript_preview/diff.patch \
  --mode fast
```

## Then

Rust evidence and TypeScript preview evidence appear in the same check output
without forking schemas. The TypeScript finding remains preview/advisory.

## Must Not

- Let TypeScript preview evidence alter Rust evidence semantics.
- Treat mixed-language preview evidence as a default gate input.
- Promote preview evidence into badges, baselines, or RIPR Zero.
