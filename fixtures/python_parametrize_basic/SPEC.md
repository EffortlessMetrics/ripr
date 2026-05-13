# Fixture: python_parametrize_basic

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a predicate and a pytest
`@pytest.mark.parametrize` test calls the owner.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_parametrize_basic
```

or:

```bash
ripr check \
  --root fixtures/python_parametrize_basic/input \
  --diff fixtures/python_parametrize_basic/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- recognises `@pytest.mark.parametrize` syntactically,
- records the parametrized test as related to the changed owner,
- emits preview language metadata and `owner_kind = "function"`,
- keeps the finding `weakly_exposed` until assertion extraction lands.

## Must Not

- Execute pytest.
- Resolve import graphs.
- Promote preview evidence into gate authority.
