# Fixture: typescript_owner_file_match

Spec: RIPR-SPEC-0027

## Given

A TypeScript workspace has two production files with owners spanning the
same line numbers:

```text
src/a.ts::alphaScore
src/b.ts::betaScore
```

The diff changes `src/b.ts` on a line number that is also inside
`src/a.ts::alphaScore`. Each owner has its own related test.

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
cargo xtask fixtures typescript_owner_file_match
```

or:

```bash
ripr check \
  --root fixtures/typescript_owner_file_match/input \
  --diff fixtures/typescript_owner_file_match/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- filters candidate owners to `src/b.ts` before checking line ranges,
- selects `betaScore` instead of the same-line owner in `src/a.ts`,
- attaches only `tests/b.test.ts` as related evidence,
- emits preview-labeled public artifacts for the changed file.

## Must Not

- Cross-route owner or related-test evidence between TypeScript files.
- Add VS Code selectors or LSP routing.
- Claim TypeScript preview evidence has Rust parity.
