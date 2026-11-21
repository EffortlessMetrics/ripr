# Fixture: python_mock_assert_called

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a call argument, and a pytest test calls
the owner and checks a mock interaction with `assert_called_once_with(...)`.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_mock_assert_called
```

or:

```bash
ripr check \
  --root fixtures/python_mock_assert_called/input \
  --diff fixtures/python_mock_assert_called/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `send_receipt` function owner,
- classifies the changed callback line as a `side_effect` / `effect` probe,
- finds the pytest `test_send_receipt_notifies_callback` related test,
- records `assert_called_once_with(...)` as `mock_expectation` / `medium`,
- emits Python preview metadata,
- keeps the finding `weakly_exposed` because mock-call evidence is not an
  exact-value discriminator.

## Must Not

- Inspect mock framework runtime state.
- Execute pytest.
- Treat the mock expectation as runtime proof.
