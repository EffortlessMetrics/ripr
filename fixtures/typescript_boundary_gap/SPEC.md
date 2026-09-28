# Fixture: typescript_boundary_gap

Spec: RIPR-SPEC-0027

## Given

A TypeScript production module changes its discount predicate from:

```ts
amount > threshold
```

to:

```ts
amount >= threshold
```

A single Jest-style test references the function by name but does not
cover the equality boundary.

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml` so the adapter actually runs on this fixture:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
cargo xtask fixtures typescript_boundary_gap
```

or:

```bash
ripr check \
  --root fixtures/typescript_boundary_gap/input \
  --diff fixtures/typescript_boundary_gap/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `applyDiscount` owner in `src/discount.ts`,
- finds the `test('...')` call in `tests/discount.test.ts` that
  references `applyDiscount(`,
- runs the assertion-shape walker (#767) but extracts no
  `expect(...).matcher(...)` chain — the tests use raw control flow
  (`if (result !== ...) throw`) instead — so the strongest oracle is
  `unknown`,
- classifies the changed predicate line as `weakly_exposed` because a
  related test reaches the owner but no strong discriminator is
  observable.

The finding carries `language = "typescript"` and
`language_status = "preview"` per RIPR-SPEC-0026.

This is the weak-oracle baseline. The companion fixture
`typescript_strong_oracle` pins the `exposed` end of the same
gradient by adding an `expect(...).toBe(...)` assertion. Probe-shape
facts (#768) and explicit static-limit reporting (#769) will refine
subsequent goldens; this fixture's current goldens should be re-blessed
when those slices land.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation
  execution.
- Claim the TypeScript adapter has parity with Rust evidence.
- Hide that assertion-shape extraction is preview-deferred.
