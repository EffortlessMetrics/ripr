# Fixture: python_related_test_import_overlap

Spec: RIPR-SPEC-0028

## Given

A Python test file imports the changed owner and its test name shares a
specific owner token, but the test body does not directly call the owner.

The fixture workspace enables the Python preview adapter explicitly.

## When

```bash
cargo xtask fixtures python_related_test_import_overlap
```

or:

```bash
ripr check \
  --root fixtures/python_related_test_import_overlap/input \
  --diff fixtures/python_related_test_import_overlap/diff.patch \
  --mode fast
```

## Then

The Python preview adapter emits a related-test fact with
`relation_reason = "import_reference_overlap"`,
`relation_confidence = "medium"`, and `language = "python"`.

## Must Not

- Execute Python imports.
- Resolve the import graph semantically.
