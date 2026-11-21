# Fixture: python_model_field_repair_gap

Spec: RIPR-SPEC-0028

## Given

A Python function returns a simple model-like object constructor with a keyword
field, and a pytest test calls the function but only checks object truthiness.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_model_field_repair_gap
```

or:

```bash
ripr check \
  --root fixtures/python_model_field_repair_gap/input \
  --diff fixtures/python_model_field_repair_gap/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `build_user` function owner,
- classifies the changed returned constructor keyword as a
  `field_construction` / `value` probe,
- keeps the broad pytest assertion weak,
- emits a field/object missing discriminator for `result.active == True`,
- emits a repair card that recommends a returned-object assertion shaped like
  `assert result.active == True`,
- adds a stop condition for cases where the returned object does not expose the
  constructor keyword as a public field or attribute.

## Must Not

- Execute pytest.
- Import the dataclass or infer runtime constructor semantics.
- Generate a passing test.
- Authorize production-code edits.
