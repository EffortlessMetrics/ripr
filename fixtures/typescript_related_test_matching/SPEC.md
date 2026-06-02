# Fixture: typescript_related_test_matching

Spec: RIPR-SPEC-0027

## Given

A TypeScript preview workspace changes a function owner in `src/pricing.ts`.
The related Jest/Vitest tests call the owner through:

- a named import alias;
- a namespace import member call.

The same test file also contains false-match shapes:

- a named import from an unrelated source file;
- a direct owner-name import from an unrelated source file;
- a type-only import alias;
- an arbitrary object method named like the owner;
- string and comment mentions shaped like calls.

The repository opts into TypeScript preview analysis.

## When

```bash
cargo xtask fixtures typescript_related_test_matching
```

or:

```bash
ripr check \
  --root fixtures/typescript_related_test_matching/input \
  --diff fixtures/typescript_related_test_matching/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- relates the alias and namespace import calls to `applyDiscount`;
- keeps unrelated imports, type-only imports, object methods, comments, and
  strings out of related-test evidence;
- preserves `language = "typescript"` and `language_status = "preview"`;
- remains syntax-first and advisory.

## Must Not

- Resolve a package graph, invoke `tsc`, or run Jest/Vitest.
- Treat arbitrary object methods or prose mentions as related tests.
- Promote TypeScript preview evidence into gates, badges, baselines, or RIPR
  Zero.
