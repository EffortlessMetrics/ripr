# Fixture: python_unittest_basic

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a threshold predicate, and a
`unittest.TestCase` subclass contains a `test_*` method that calls the owner
with a broad boolean assertion.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_unittest_basic
```

or:

```bash
ripr check \
  --root fixtures/python_unittest_basic/input \
  --diff fixtures/python_unittest_basic/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- recognises `risk_score` as a Python function owner,
- recognises `RiskScoreTests.test_risk_score_high` as a unittest test,
- records `self.assertTrue(...)` as `smoke_only` / `smoke`,
- emits preview language metadata and `owner_kind = "function"`,
- classifies the changed line as `weakly_exposed` because smoke evidence is not
  a strong discriminator.

## Must Not

- Execute unittest.
- Require runtime imports or environment setup.
