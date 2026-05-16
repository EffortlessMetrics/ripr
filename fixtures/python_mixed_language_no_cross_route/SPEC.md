# Fixture: python_mixed_language_no_cross_route

Spec: RIPR-SPEC-0028

## Given

A Python production owner changes, the repo enables Python and TypeScript
preview adapters, and only a TypeScript test mentions the Python owner name.

## When

```bash
cargo xtask fixtures python_mixed_language_no_cross_route
```

or:

```bash
ripr check \
  --root fixtures/python_mixed_language_no_cross_route/input \
  --diff fixtures/python_mixed_language_no_cross_route/diff.patch \
  --mode fast
```

## Then

The Python preview adapter emits a Python preview finding with no related
Python test. The TypeScript test is not used as Python related-test evidence.

## Must Not

- Cross-route related tests across languages.
- Treat a TypeScript text mention as Python evidence.
- Add editor routing.
