# Fixture: typescript_owner_kinds

Spec: RIPR-SPEC-0027

## Given

A TypeScript file changes behavior inside several syntax-first owner shapes:

- an exported arrow function assigned to `const`;
- an instance class method;
- a static class method;
- a module-level const initializer.

The repository opts into TypeScript preview analysis.

## When

```bash
cargo xtask fixtures typescript_owner_kinds
```

or:

```bash
ripr check \
  --root fixtures/typescript_owner_kinds/input \
  --diff fixtures/typescript_owner_kinds/diff.patch \
  --mode fast
```

## Then

- Findings carry `language = "typescript"` and `language_status = "preview"`.
- The arrow function finding carries `owner_kind = "arrow_function"`.
- The instance method finding carries `owner_kind = "method"`.
- The static method finding carries `owner_kind = "class_method"`.
- The module initializer finding carries `owner_kind = "module_function"`.

## Must Not

- Treat TypeScript owner facts as typechecker-backed analysis.
- Infer related-test safety for class methods from arbitrary object method
  calls.
- No support-tier, gate, badge, or runtime-test authority changes.
