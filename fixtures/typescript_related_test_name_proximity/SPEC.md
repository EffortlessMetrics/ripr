# Fixture: typescript_related_test_name_proximity

Spec: RIPR-SPEC-0027

## Given

A TypeScript preview workspace changes a function owner in `src/pricing.ts`.
The test corpus has no direct owner call and no resolved runtime import call.
Instead, it offers only heuristic related-test locations:

- a same-stem test file;
- a `describe(...)` block named after the owner;
- a test name token match for the owner.

The corpus also includes partial-token names that mention discounts without the
full `applyDiscount` owner token.

The repository opts into TypeScript preview analysis.

## When

```bash
cargo xtask fixtures typescript_related_test_name_proximity
```

or:

```bash
ripr check \
  --root fixtures/typescript_related_test_name_proximity/input \
  --diff fixtures/typescript_related_test_name_proximity/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- relates same-stem, describe-name, and test-name proximity as uncertain links;
- does not use strong assertions from heuristic-only links as proof;
- keeps partial-token names out of related-test evidence;
- preserves `language = "typescript"` and `language_status = "preview"`;
- remains syntax-first and advisory.

## Must Not

- Resolve a package graph, invoke `tsc`, or run Jest/Vitest.
- Treat heuristic proximity as a complete repair packet or strong proof.
- Treat partial owner tokens such as `discount` as `applyDiscount`.
- Promote TypeScript preview evidence into gates, badges, baselines, or RIPR
  Zero.
