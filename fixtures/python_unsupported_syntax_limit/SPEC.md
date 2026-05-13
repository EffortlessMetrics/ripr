# Fixture: python_unsupported_syntax_limit

Spec: RIPR-SPEC-0028

## Given

A Python owner changes a generator `yield` expression with a direct related
pytest assertion.

The fixture workspace enables the Python preview adapter explicitly.

## When

```bash
cargo xtask fixtures python_unsupported_syntax_limit
```

or:

```bash
ripr check \
  --root fixtures/python_unsupported_syntax_limit/input \
  --diff fixtures/python_unsupported_syntax_limit/diff.patch \
  --mode fast
```

## Then

The Python preview finding emits `static_limit_kind = "unsupported_syntax"`.

## Must Not

- Model generator runtime behavior.
- Hide the preview limitation behind a normal action recommendation.
