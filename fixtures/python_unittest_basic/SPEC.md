# Fixture: python_unittest_basic

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a return value and a
`unittest.TestCase` test method calls the changed owner.

## When

```bash
cargo xtask fixtures python_unittest_basic
```

## Then

The Python preview adapter finds the function owner, recognises the
`unittest.TestCase` method as a test fact, and emits preview-labeled
evidence.

## Must Not

- Run `unittest`.
- Treat unittest assertions as strong evidence before the assertion
  extraction sub-slice.
