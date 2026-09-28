# Fixture: ts_domock_relation_gate

Spec: RIPR-SPEC-0026

## Given

A TypeScript owner `applyDiscount(amount, threshold)` changes its predicate
boundary (`amount > threshold` becomes `amount >= threshold`). The related
test registers `jest.doMock("../src/pricing")` — the non-hoisted mock
variant both Jest and Vitest support (issue #4103 shape 2). Unlike `mock`,
`doMock` does not hoist: it affects only modules loaded after the call, so
the file's static `import` of the owner still executes the real code. The
adapter cannot statically prove which module instance the observed call
reaches, so collecting the registration and refusing owner-call credit is
the conservative treatment; without collecting `doMock` registrations the
extractor would miss the owner-module mock registration entirely and the
exact-value assertion would be credited as real boundary evidence.

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
ripr check \
  --root fixtures/ts_domock_relation_gate/input \
  --diff fixtures/ts_domock_relation_gate/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- collects the `jest.doMock("../src/pricing")` registration into
  `mocks_in_file` like a `jest.mock(...)` call,
- refuses the trusted owner-call relation (a `doMock` registration of the
  owner module is conservatively refused evidence: the adapter cannot prove
  the observed call executes the changed code rather than a mocked
  module instance loaded after the registration),
- surfaces the `mocked_module` static limit with the named limitation
  `typescript_mock_only_observer`,
- keeps the finding `weakly_exposed` and advisory.

## Must Not

- Promote the finding to `exposed` on an assertion that observes a mocked
  substitution.
- Treat `jest.doMock(...)` / `vi.doMock(...)` as invisible to owner-module
  mock detection.
