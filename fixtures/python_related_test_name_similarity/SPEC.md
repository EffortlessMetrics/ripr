# Fixture: python_related_test_name_similarity

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a return value, and a pytest test in a
different file names the changed owner in its title and references it
(`handler = apply_discount`) but does not call it.

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
- relates the pytest test through conservative test-name similarity, because
  the test references the owner (a title match alone is not a relation;
  RIPR-SPEC-0028),
- marks the relation as uncertain,
- does not promote the unrelated exact assertion to a strong discriminator,
- emits Python preview metadata.

## Must Not

- Treat test-name similarity as runtime proof.
- Upgrade the relation to `exposed`.
- Relate a test whose only mention of the owner is its title.
- Execute pytest.
