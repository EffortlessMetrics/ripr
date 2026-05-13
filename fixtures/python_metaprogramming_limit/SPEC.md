# Fixture: python_metaprogramming_limit

Spec: RIPR-SPEC-0028

## Given

A Python owner changes a metaprogramming expression with a direct related
pytest assertion.

The fixture workspace enables the Python preview adapter explicitly.

## When

```bash
cargo xtask fixtures python_metaprogramming_limit
```

or:

```bash
ripr check \
  --root fixtures/python_metaprogramming_limit/input \
  --diff fixtures/python_metaprogramming_limit/diff.patch \
  --mode fast
```

## Then

The Python preview finding emits `static_limit_kind = "metaprogramming"` and
keeps the output advisory.

## Must Not

- Execute `eval`.
- Infer generated runtime behavior.
