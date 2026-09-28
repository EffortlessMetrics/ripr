# Fixture: typescript_static_limit_taxonomy

Spec: RIPR-SPEC-0027

## Given

A TypeScript/JavaScript preview workspace opts into the adapter:

```toml
[languages]
enabled = ["rust", "typescript"]
```

The changed production lines cover static limits that cannot be turned into
trusted repair guidance with syntax-only TypeScript evidence:

- computed member invocation (`dynamic_dispatch`);
- `Proxy` metaprogramming (`metaprogramming`);
- decorator-modified method behavior (`decorator_indirection`);
- production calls through imported symbols without an import graph
  (`missing_import_graph`).

Existing fixtures cover `mocked_module` and parse-error `unsupported_syntax`.

## When

```bash
cargo xtask fixtures typescript_static_limit_taxonomy
```

or:

```bash
ripr check \
  --root fixtures/typescript_static_limit_taxonomy/input \
  --diff fixtures/typescript_static_limit_taxonomy/diff.patch \
  --mode fast
```

## Then

The TypeScript-family preview adapter emits findings with `language_status =
"preview"` and the corresponding `static_limit_kind` for each changed line.
The limitations appear in both evidence and human-readable missing context.

## Must Not

- Invoke `tsc`, `tsserver`, package graph resolution, Jest/Vitest runtime, or
  provider/model calls.
- Generate tests or edit source files.
- Promote TypeScript/JavaScript out of opt-in preview.
- Give TypeScript/JavaScript preview findings default gate, badge, baseline,
  or RIPR Zero authority.
