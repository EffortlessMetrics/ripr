# Fixture: python_related_test_mixed_language_no_cross_route

Spec: RIPR-SPEC-0028

## Given

A Python production owner changes, and a TypeScript test file mentions a
same-named function.

The fixture workspace enables Python and TypeScript preview adapters, but
the changed file is Python.

## When

```bash
cargo xtask fixtures python_related_test_mixed_language_no_cross_route
```

or:

```bash
ripr check \
  --root fixtures/python_related_test_mixed_language_no_cross_route/input \
  --diff fixtures/python_related_test_mixed_language_no_cross_route/diff.patch \
  --mode fast
```

## Then

The Python preview adapter emits no Python related test and does not
cross-route the TypeScript test file into Python evidence.

## Must Not

- Use TypeScript tests as Python related-test evidence.
- Emit a related-test path outside the Python adapter's file set.
