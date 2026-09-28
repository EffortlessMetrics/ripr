# Fixture: typescript_mocked_module_limit

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

A Jest-style test asserts the result with an exact-value matcher
(`expect(...).toBe(90)`), but the test file ALSO mocks the `./api`
module via `vi.mock("./api")` at the top of the file.

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
cargo xtask fixtures typescript_mocked_module_limit
```

or:

```bash
ripr check \
  --root fixtures/typescript_mocked_module_limit/input \
  --diff fixtures/typescript_mocked_module_limit/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `applyDiscount` owner in `src/discount.ts`,
- finds the `test('at threshold discounts', ...)` call in
  `tests/discount.test.ts` that references `applyDiscount(`,
- extracts the strong `toBe` oracle and would otherwise classify
  this finding as `exposed`,
- detects the syntactic `vi.mock("./api")` call in the same test
  file and surfaces it as the `mocked_module` static-limit per
  RIPR-SPEC-0026, in both `evidence` and `missing` text on the
  finding.

The classifier still reaches `Exposed` because the related test
carries a strong oracle, but the static-limit text makes it
explicit that the adapter cannot resolve what the mocked module
substitutes — the user can decide whether the assertion still
covers the changed behavior despite the mock.

The finding carries `language = "typescript"` and
`language_status = "preview"` per RIPR-SPEC-0026.

## Must Not

- Suppress the strong-oracle classification just because a mock is
  present; the limit is reported, not used to downgrade.
- Claim the adapter has resolved the mocked module substitution.
- Use mutation-runtime outcome vocabulary reserved for real
  mutation execution.
