# Fixture: ts_predicate_boundary_const_result_local

Spec: RIPR-SPEC-0027

## Given

A TypeScript owner `applyDiscount(total)` changes its predicate boundary
(`total > 100` becomes `total >= 100`). The related test uses the canonical
local-binding idiom — `const result = applyDiscount(100);` followed by
`expect(result).toBe(0.9)` — so the `expect(<expr>)` argument is a bare
local whose single `const` initializer IS the owner call at the boundary
input (issue #4104 E1).

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
ripr check \
  --root fixtures/ts_predicate_boundary_const_result_local/input \
  --diff fixtures/ts_predicate_boundary_const_result_local/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `applyDiscount` owner with parameter facts (`total`) in
  `src/pricing.ts`,
- finds the related test in `tests/pricing.test.ts` (direct owner call
  through the local binding's initializer),
- resolves the one-hop `const result = applyDiscount(100)` binding and
  witnesses the changed boundary `total == 100` through the initializer's
  arguments,
- keeps the finding `exposed` with no missing boundary discriminator.

## Must Not

- Credit a local whose initializer is not the owner call itself (a wrapper
  around the call, or a derivation like `applyDiscount(100) * 2`).
- Credit a `let` binding that is reassigned after the owner call, or a name
  declared more than once in the test body.
- Credit a boundary literal parked in an argument position the owner never
  reads (`applyDiscount(150, 100)` behind the binding).
- Weaken the direct-call boundary witness (`expect(applyDiscount(100))`) in
  any way.
