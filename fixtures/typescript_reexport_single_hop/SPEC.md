# Fixture: typescript_reexport_single_hop

Spec: RIPR-SPEC-0095

## Given

A TypeScript preview workspace changes function `isRawNetworkError` in `src/util.ts`.
A barrel file `src/index.ts` re-exports it:

```typescript
export { isRawNetworkError } from './util';
```

A test in `test/util.test.ts` imports from the barrel:

```typescript
import { isRawNetworkError } from '../src/index';
```

The test calls `isRawNetworkError(...)` and asserts on the result with `toBe`.

## When

```bash
cargo xtask fixtures typescript_reexport_single_hop
```

or:

```bash
ripr check \
  --root fixtures/typescript_reexport_single_hop/input \
  --diff fixtures/typescript_reexport_single_hop/diff.patch
```

## Then

The TypeScript preview adapter:

- resolves the single-hop re-export chain (`test → index.ts → util.ts`) and
  credits both tests;
- emits `relation_reason: re_export_chain_followed` for each credited test;
- emits `relation_confidence: medium` (explicit in-source but one level of
  indirection);
- classifies the finding as `exposed` (strong `toBe` oracle);
- stays syntax-first (no tsc, no package graph, no runtime).

## Must Not

- Credit a test that imports through two or more intermediate hops (fail-closed).
- Credit a test whose single-hop chain leads to a DIFFERENT source file
  (no false credit on name collision).
- Resolve node_modules, non-relative specifiers, or star re-exports.
