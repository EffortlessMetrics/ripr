# Fixture: typescript_dynamic_assertion_unresolved

Spec: RIPR-SPEC-0085

## Given

A TypeScript production module changes its clamp predicate. The test
exercises `clamp(...)` with `expect(clamp(...)).toBe(expectedMin)` where
`expectedMin` is a local variable — a non-literal dynamic expression.

The fixture workspace enables the TypeScript preview adapter via `ripr.toml`.

## When

```bash
cargo xtask fixtures typescript_dynamic_assertion_unresolved
```

or:

```bash
ripr check \
  --root fixtures/typescript_dynamic_assertion_unresolved/input \
  --diff fixtures/typescript_dynamic_assertion_unresolved/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter (RIPR-SPEC-0085 §PR5):

- finds the `clamp` owner in `src/clamp.ts`,
- finds the `test(...)` in `tests/clamp.test.ts` with a direct call relation,
- extracts the `expect(clamp(...)).toBe(expectedMin)` assertion where the
  matcher argument `expectedMin` is a variable (non-literal dynamic expression),
- emits `typescript_oracle_observed: clamp(-5, 0, 10)` (the `expect(...)` arg),
- does NOT emit `typescript_oracle_expected` (dynamic arg — cannot resolve),
- emits `typescript_oracle_confidence: medium` (strong oracle with unresolved literal),
- emits `typescript_dynamic_assertion_unresolved` named limitation because
  the `toBe(...)` argument is a dynamic variable — the adapter cannot
  statically establish the expected discriminator value.

## Must Not

- Emit `typescript_oracle_expected` for a dynamic variable argument.
- Claim `repair_packet_ready: true`.
- Change `oracle_kind`, `oracle_strength`, or `static_limit_kind`.
