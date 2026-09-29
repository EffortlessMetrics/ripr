# Fixture: typescript_reexport_barrel_other_name

Spec: RIPR-SPEC-0095

## Given

A TypeScript preview workspace changes `applyDiscount` in `src/a.ts`.

The barrel `src/index.ts` forwards both modules:

```typescript
export { applyDiscount } from './a';
export * from './b';
```

The only test imports `formatCents` (from `src/b.ts`) through the directory
specifier `../src` and never imports or calls `applyDiscount`.

## When

```bash
cargo xtask fixtures typescript_reexport_barrel_other_name
```

## Then

The TypeScript preview adapter:

- resolves `../src` to `src/index.ts` and the imported name `formatCents`
  through the star hop to `src/b.ts`, which is not the changed owner;
- does NOT credit the test for `applyDiscount` (stays `no_static_path`,
  0 related tests).

## Purpose

This is the barrel NEGATIVE control for bounded re-export tracing: sharing a
barrel with the changed owner is not a relation. Only the name the test
imports, resolved through the chain, can reach the owner.

## Must Not

- Credit a test that imports an unrelated name from the same barrel.
- Emit `re_export_chain_followed` for `applyDiscount`.
