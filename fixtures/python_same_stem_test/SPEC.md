# Fixture: python_same_stem_test

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a return value, and a same-stem pytest
file references the changed owner (`handler = reconcile_total`) without a
syntactic call to it.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_same_stem_test
```

or:

```bash
ripr check \
  --root fixtures/python_same_stem_test/input \
  --diff fixtures/python_same_stem_test/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `reconcile_total` function owner,
- relates the pytest test by same-stem file proximity, because the test
  references the owner (a same-stem test that never references the owner is
  not related; RIPR-SPEC-0028),
- keeps the relation weak and does not promote the exact assertion to a
  strong discriminator,
- emits Python preview metadata.

## Must Not

- Treat file proximity as runtime proof.
- Upgrade same-stem proximity alone to `exposed`.
- Relate a same-stem test that does not reference the changed owner.
- Execute pytest.
