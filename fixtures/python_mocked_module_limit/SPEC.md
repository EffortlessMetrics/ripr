# Fixture: python_mocked_module_limit

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a return call, and a related pytest test
uses `unittest.mock.patch(...)` syntax.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_mocked_module_limit
```

or:

```bash
ripr check \
  --root fixtures/python_mocked_module_limit/input \
  --diff fixtures/python_mocked_module_limit/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `fetch_total` function owner,
- relates the patch-decorated pytest test,
- emits `static_limit_kind = "mocked_module"`,
- keeps the finding preview/advisory.

## Must Not

- Resolve mock substitution semantics.
- Execute pytest.
- Hide the mocked-module limit from output.
