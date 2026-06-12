# Fixture: typescript_limitation_custom_matcher

Spec: RIPR-SPEC-0085

## Given

A TypeScript production module `src/pricing.ts` exports `computePrice` and
changes its guard predicate from `base > 0` to `base >= 0`. A test file
`tests/pricing.test.ts` calls `computePrice` and asserts the result with a
custom Jest matcher `toBeWithinRange` that is NOT in `oracle.rs`'s recognised
matcher set. The oxc AST parser sees the real `expect(price).toBeWithinRange(...)`
call expression and returns `OracleKind::Unknown` with matcher name
`"toBeWithinRange"`. This is a real AST-evidence producer for the named
limitation `typescript_custom_matcher_unresolved`.

## When

```bash
cargo xtask fixtures typescript_limitation_custom_matcher
```

or:

```bash
ripr check \
  --root fixtures/typescript_limitation_custom_matcher/input \
  --diff fixtures/typescript_limitation_custom_matcher/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `computePrice` owner in `src/pricing.ts`,
- finds the `test("price is in expected range", ...)` call in
  `tests/pricing.test.ts` that references `computePrice(`,
- extracts the `toBeWithinRange` matcher via the oxc-parsed AST and
  maps it to `OracleKind::Unknown` because it is not in the
  recognised matcher table,
- emits the additive evidence line
  `typescript_limitation: typescript_custom_matcher_unresolved`
  with `sample_source = tests/pricing.test.ts:<line>`,
  `why_not_actionable` naming the unrecognised matcher, and
  `repair_route: analysis/typescript-custom-matcher-resolution`.

The existing `static_limit_kind` field is NOT changed. `repair_packet_ready`
remains `false`. `language_status` remains `preview`.

## Must Not

- Add `typescript_custom_matcher_unresolved` without a real AST producer.
- Change `static_limit_kind`, `repair_packet_ready`, or `language_status`.
- Emit the limitation for recognised matchers (`toBe`, `toEqual`, etc.).
