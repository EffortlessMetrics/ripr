# Fixture: typescript_reexport_no_false_credit

Spec: RIPR-SPEC-0095

## Given

A TypeScript preview workspace changes `isRawNetworkError` in `src/util.ts`.

There is ALSO `src/other.ts` that defines a DIFFERENT function with the same name.

The barrel file `src/index.ts` re-exports from `other.ts`, NOT from `util.ts`:

```typescript
export { isRawNetworkError } from './other';
```

A test imports `isRawNetworkError` from the barrel and calls it.

## When

```bash
cargo xtask fixtures typescript_reexport_no_false_credit
```

## Then

The TypeScript preview adapter:

- resolves the single-hop chain (`test → index.ts → other.ts`) and finds
  that `other.ts` is NOT the changed owner file (`util.ts`);
- does NOT credit the test (stays `no_static_path`, 0 related tests);
- does NOT emit `relation_reason: re_export_chain_followed` for this test.

## Purpose

This is the NO-FALSE-CREDIT control for RIPR-SPEC-0095. It proves that
single-hop name matching is owner-file-bound: the same exported name from a
different source file does not produce a false credit.

## Must Not

- Credit the test even though the name `isRawNetworkError` matches.
- Emit `re_export_chain_followed` when the chain leads to a different file.
