# Fixture: python_unittest_assertions

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a predicate boundary, and a
`unittest.TestCase` test calls the changed owner with `self.assertEqual(...)`.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_unittest_assertions
```

or:

```bash
ripr check \
  --root fixtures/python_unittest_assertions/input \
  --diff fixtures/python_unittest_assertions/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `risk_score` function owner,
- finds the `RiskScoreTests.test_high_risk_score` unittest related test,
- records `self.assertEqual(...)` as `exact_value` / `strong`,
- emits Python preview metadata,
- classifies the changed predicate line as `exposed`.

## Must Not

- Execute unittest.
- Require runtime imports or environment setup.
- Infer more than the syntactic assertion shape.
