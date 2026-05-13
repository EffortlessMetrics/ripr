# Fixture: python_unittest_assertions

Spec: RIPR-SPEC-0028

## Given

Related `unittest.TestCase` methods reach changed Python owners through
`self.assertEqual(...)` and `self.assertRaises(...)`.

## When

```bash
cargo xtask fixtures python_unittest_assertions
```

## Then

The Python preview adapter records strong exact-value and exact-error oracle
evidence.

## Must Not

- Run unittest.
- Claim parity with Rust evidence.
- Add editor routing or policy behavior.
