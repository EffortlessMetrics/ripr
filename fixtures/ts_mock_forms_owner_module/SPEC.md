# Fixture: ts_mock_forms_owner_module

Spec: RIPR-SPEC-0026

## Given

A TypeScript owner `applyDiscount(amount, threshold)` changes its predicate
boundary (`amount > threshold` becomes `amount >= threshold`). The related
test mocks the owner module through two forms the owner-module mock guard
used to miss (issue #4294): the runner object is a renamed import
(`import { vi as v } from "vitest"`), and the specifier is Vitest's typed
`import("../src/pricing")` form. The factory replaces `applyDiscount`, so the
exact-value assertion observes the mock, not the changed boundary.

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
ripr check \
  --root fixtures/ts_mock_forms_owner_module/input \
  --diff fixtures/ts_mock_forms_owner_module/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- resolves `v` to the Vitest runner object and reads the typed
  `import("../src/pricing")` specifier into `mocks_in_file`,
- refuses the trusted owner-call relation,
- surfaces the `mocked_module` static limit with the named limitation
  `typescript_mock_only_observer`,
- keeps the finding `weakly_exposed` and advisory.

## Must Not

- Promote the finding to `exposed` on an assertion that observes a mocked
  substitution.
- Treat a renamed runner import or a typed `import(...)` specifier as
  invisible to owner-module mock detection.
