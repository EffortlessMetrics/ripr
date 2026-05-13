# Fixture: python_related_test_file_path_proximity

Spec: RIPR-SPEC-0028

## Given

A Python test file path shares a specific owner token, but the test body
does not directly call the owner and the test name does not repeat the
token.

The fixture workspace enables the Python preview adapter explicitly.

## When

```bash
cargo xtask fixtures python_related_test_file_path_proximity
```

or:

```bash
ripr check \
  --root fixtures/python_related_test_file_path_proximity/input \
  --diff fixtures/python_related_test_file_path_proximity/diff.patch \
  --mode fast
```

## Then

The Python preview adapter emits a related-test fact with
`relation_reason = "file_path_proximity"`,
`relation_confidence = "medium"`, and `language = "python"`.

## Must Not

- Treat file proximity as high confidence.
- Cross workspace roots.
