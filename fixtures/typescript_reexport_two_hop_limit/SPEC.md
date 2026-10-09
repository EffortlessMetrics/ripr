# Fixture: typescript_reexport_two_hop_limit

Spec: RIPR-SPEC-0095

## Given

A TypeScript preview workspace changes `isRawNetworkError` in `src/util.ts`.

The re-export chain is TWO hops:

```
test/util.test.ts  →  import from src/index.ts
src/index.ts       →  export { isRawNetworkError } from './errors'   (hop 1)
src/errors.ts      →  export { isRawNetworkError } from './util'     (hop 2)
src/util.ts        →  export function isRawNetworkError(...)          (owner)
```

The test imports from `src/index.ts`.

## When

```bash
cargo xtask fixtures typescript_reexport_two_hop_limit
```

## Then

The TypeScript preview adapter:

- follows the chain hop by hop (`index.ts → errors.ts → util.ts`), which is
  within the bounded re-export hop limit (`MAX_REEXPORT_HOPS`, 4 hops);
- lands on the changed owner's own export in `util.ts` under the owner's name;
- credits the test with `relation_reason: re_export_chain_followed`
  (`relation_confidence: medium`), so the finding is `exposed`.

## Purpose

This fixture was originally the two-hop fail-closed control of the single-hop
slice. Bounded chain tracing now credits it: every hop is an explicit,
in-repo named re-export that ends at the owner's own export. The fail-closed
bound moved to chains longer than `MAX_REEXPORT_HOPS`, cycles, ambiguous star
exports, and star hops to modules that do not export the name — covered by the
`reexport_chain_tests` unit tests in
`crates/ripr/src/analysis/language/typescript/tests/reexport_chain_tests.rs`.

## Must Not

- Credit the test when any hop resolves to a module other than the owner file.
- Follow chains beyond `MAX_REEXPORT_HOPS` hops.
