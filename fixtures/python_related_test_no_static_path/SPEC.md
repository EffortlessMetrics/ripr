# Fixture: python_related_test_no_static_path

Spec: RIPR-SPEC-0028

## Given

A Python production owner changes, but no Python test has a conservative
related-test signal.

The fixture workspace enables the Python preview adapter explicitly.

## When

```bash
cargo xtask fixtures python_related_test_no_static_path
```

or:

```bash
ripr check \
  --root fixtures/python_related_test_no_static_path/input \
  --diff fixtures/python_related_test_no_static_path/diff.patch \
  --mode fast
```

## Then

The Python preview adapter emits `no_static_path` and no related tests.

## Must Not

- Invent a related test from unrelated Python files.
- Cross-route evidence from another language.
