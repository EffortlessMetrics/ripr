# Fixture: typescript_monorepo_package_local

Spec: RIPR-SPEC-0085

## Given

A monorepo workspace with two packages:

- `packages/pkg-a/` — source `src/discount.ts` (changed) + test
  `tests/discount.test.ts` (imports from `../src/discount`).
- `packages/pkg-b/` — its own source and a test that mentions `applyDiscount`
  by name locally (a shadow variable, not an import from pkg-a).

Both packages have their own `package.json` (with Jest framework).  The
monorepo root has a `"workspaces"` field in its root `package.json`.

## When

```bash
cargo xtask fixtures typescript_monorepo_package_local
```

or:

```bash
ripr check \
  --root fixtures/typescript_monorepo_package_local/input \
  --diff fixtures/typescript_monorepo_package_local/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter (RIPR-SPEC-0085 §PR6):

- finds the `applyDiscount` owner in `packages/pkg-a/src/discount.ts`,
- selects the `test(...)` in `packages/pkg-a/tests/discount.test.ts` because
  it imports directly from `../src/discount` and is in the SAME package,
- does NOT select the `test(...)` in `packages/pkg-b/tests/pricing.test.ts`
  because that test is in a different package (`packages/pkg-b`).

## Must Not

- Select cross-package test candidates, even if they mention the owner by name.
- Regress existing single-package fixtures where both source and test have no
  package.json (package_root stays `.`, ownership is unaffected).
- Claim `repair_packet_ready: true`.
