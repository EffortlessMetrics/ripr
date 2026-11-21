# Fixture: python_pytest_oracle_shapes

Spec: RIPR-SPEC-0028

## Given

A Python production function changes an output/log side effect, and a pytest
test calls the changed owner while observing the logged output through the
`caplog` fixture.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_pytest_oracle_shapes
```

or:

```bash
ripr check \
  --root fixtures/python_pytest_oracle_shapes/input \
  --diff fixtures/python_pytest_oracle_shapes/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `warn_coupon` function owner,
- finds the pytest `test_warn_coupon_logs_expired_message` related test,
- records the pytest `caplog` fixture parameter,
- records the output assertion shape from `caplog.text`,
- keeps the oracle conservative as `relational_check` / `weak`,
- emits Python preview metadata.

## Must Not

- Run pytest or import the Python module.
- Treat output/log containment as an exact-value oracle.
- Emit repair cards or agent packets in this preview fact slice.
