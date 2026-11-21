# Fixture: python_fixture_name_relation

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a return value, and a pytest test in a
different file uses a fixture parameter matching the owner file stem but does
not call the changed owner.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_fixture_name_relation
```

or:

```bash
ripr check \
  --root fixtures/python_fixture_name_relation/input \
  --diff fixtures/python_fixture_name_relation/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `calculate_fee` function owner,
- relates the pytest test through conservative fixture-name proximity,
- marks the relation as uncertain,
- keeps the related test oracle unknown,
- emits Python preview metadata.

## Must Not

- Treat fixture-name proximity as runtime proof.
- Upgrade the relation to `exposed`.
- Execute pytest.
