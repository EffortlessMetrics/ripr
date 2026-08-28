# Fixture: typescript_adversarial_hoc_wrapped_owner (false-exposed guard — higher-order wrapper obscures owner)

Spec: RIPR-SPEC-0108

Corpus case: `ts_hoc_wrapped_owner` in
`fixtures/evidence-promotion-honesty-corpus/corpus.json` (issue #1983,
TypeScript family "decorator/higher-order wrapper obscuring the real owner").

## Given

An adversarial **higher-order wrapper** trap. The changed owner is the inner
function `computeTotal` (src/audit.ts), whose tax multiplier changes from
`1.1` to `1.2`. The only public export is the wrapped alias
`total = withLogging(computeTotal)`, and the only test exercises the WRAPPER,
not the owner, under a strong exact-value oracle:

```ts
// changed owner: computeTotal (not exported directly)
return amount * 1.2;

// the ONLY test — calls the HOC-wrapped alias, never names computeTotal
import { total } from '../src/audit';
expect(total(100)).toBe(120);
```

The tempting wrong relation is wrapper transparency: the test's exact-value
assertion on `total(...)` numerically depends on `computeTotal`, so a
wrapper-unwrapping matcher would credit direct observation. The syntax-first
preview adapter cannot resolve what `withLogging` substitutes, logs, or
forwards, so the wrapper boundary must stay opaque. (The decorator-syntax
variant of this family is already pinned by
`fixtures/typescript_static_limit_taxonomy` — corpus case
`ts_decorator_indirection_limit`.)

## When

`ripr check` analyzes the diff against the TypeScript preview adapter.

## Then

ripr classifies the change `weakly_exposed`. The test never names
`computeTotal`, so no direct or imported owner-call relation fires; the only
link is the same-file-stem proximity heuristic, which is advisory-only and
cannot borrow the wrapper's strong assertion as proof.

**This fixture must NEVER read `exposed`.** Crediting the wrapped call's
oracle as observing the obscured inner owner would be exactly the
identity-over-tokens mistake this corpus exists to pin.

## Must Not

- Resolve the higher-order wrapper `withLogging(computeTotal)` into a direct
  owner call on `computeTotal`.
- Borrow the `total(...)` exact-value oracle as evidence about the inner
  `computeTotal` owner.
- Run any TypeScript runtime; static preview evidence only.
