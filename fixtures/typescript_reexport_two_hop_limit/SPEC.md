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

- follows only ONE hop from the test's import source (`index.ts → errors.ts`);
- finds that `errors.ts` is NOT the changed owner file (`util.ts`);
- does NOT credit the test (stays `no_static_path`, 0 related tests);
- does NOT follow the second hop (`errors.ts → util.ts`);
- remains fail-closed on chains deeper than one hop.

## Purpose

This is the TWO-HOP control for RIPR-SPEC-0095. It proves that ripr is
bounded to single-hop re-export tracing; deeper transitive chains stay
honestly uncredited to avoid false credits from long re-export chains.

## Must Not

- Credit the test even though the two-hop chain DOES eventually reach the owner.
- Follow transitive chains beyond one explicit in-source hop.

## Accepted Limitation

Two-hop and deeper chains remain uncredited in this release. Single-hop is the
safe, bounded first slice. Users with deeper chains can work around this by
importing one hop closer to the owner (from `errors.ts` instead of `index.ts`).
