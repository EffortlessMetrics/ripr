# Fixture: typescript_oracle_helper_gated

Spec: RIPR-SPEC-0085

## Given

A TypeScript production module `src/pricing.ts` exports `computePrice` and
changes its guard predicate from `base > 0` to `base >= 0`. A related test file
`tests/pricing.test.ts` calls `computePrice` only through an assertion-shaped
helper call:

```ts
assertPriceBoundary(computePrice(10, 3), 20);
```

The helper may be a real oracle at runtime, but the syntax-first TypeScript
adapter does not inspect the helper body or infer helper semantics.

## When

```bash
cargo xtask fixtures typescript_oracle_helper_gated
```

or:

```bash
ripr check \
  --root fixtures/typescript_oracle_helper_gated/input \
  --diff fixtures/typescript_oracle_helper_gated/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `computePrice` owner in `src/pricing.ts`,
- finds the related `test("price is checked through helper", ...)` block,
- sees that the helper call wraps the owner call,
- emits the additive evidence line
  `typescript_limitation: typescript_oracle_helper_gated`
  with `sample_source = tests/pricing.test.ts:<line>`,
  `why_not_actionable` naming the helper, and
  `repair_route: analysis/typescript-oracle-helper-resolution`.

The existing `static_limit_kind` field is NOT changed. The finding remains
non-actionable and TypeScript remains preview-only.

## Must Not

- Credit the helper as a strong oracle.
- Inspect or infer the helper implementation.
- Emit `repair_packet_ready: true`.
- Change `static_limit_kind` or `language_status`.
