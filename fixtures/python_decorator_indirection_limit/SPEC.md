# Fixture: python_decorator_indirection_limit

Spec: RIPR-SPEC-0028

## Given

A decorated Python owner changes under a direct related pytest assertion.

The fixture workspace enables the Python preview adapter explicitly.

## When

```bash
cargo xtask fixtures python_decorator_indirection_limit
```

or:

```bash
ripr check \
  --root fixtures/python_decorator_indirection_limit/input \
  --diff fixtures/python_decorator_indirection_limit/diff.patch \
  --mode fast
```

## Then

The Python preview finding emits
`static_limit_kind = "decorator_indirection"` and preserves the decorator as
context.

## Must Not

- Execute or expand the decorator.
- Upgrade preview confidence because a direct test exists.
