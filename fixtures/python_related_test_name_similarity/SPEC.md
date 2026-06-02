# Fixture: python_related_test_name_similarity

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a return value, and a pytest test in a
different file names the changed owner but does not call it.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_related_test_name_similarity
```

or:

```bash
ripr check \
  --root fixtures/python_related_test_name_similarity/input \
  --diff fixtures/python_related_test_name_similarity/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `apply_discount` function owner,
- relates the pytest test through conservative test-name similarity,
- marks the relation as uncertain,
- does not promote the unrelated exact assertion to a strong discriminator,
- emits Python preview metadata.

## Must Not

- Treat test-name similarity as runtime proof.
- Upgrade the relation to `exposed`.
- Execute pytest.
