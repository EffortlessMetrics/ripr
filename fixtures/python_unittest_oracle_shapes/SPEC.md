# Fixture: python_unittest_oracle_shapes

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a returned output string, and a
`unittest.TestCase` test calls the owner while observing the output through
`self.assertIn(...)`.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_unittest_oracle_shapes
```

or:

```bash
ripr check \
  --root fixtures/python_unittest_oracle_shapes/input \
  --diff fixtures/python_unittest_oracle_shapes/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `warn_coupon` function owner,
- finds the `NotificationTests.test_warn_coupon_output_message` unittest test,
- records `self.assertIn(...)` as `relational_check` / `weak`,
- records the unittest output assertion shape from `result.output`,
- emits a unittest verify command using `python -m unittest`,
- emits Python preview metadata.

## Must Not

- Run unittest or import the Python module.
- Treat output/log containment as an exact-value oracle.
- Emit repair cards or agent packets in this preview fact slice.
