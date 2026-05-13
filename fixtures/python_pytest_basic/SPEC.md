# Fixture: python_pytest_basic

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a discount boundary and a pytest-style
test function calls the changed owner.

The fixture enables the Python preview adapter:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_pytest_basic
```

## Then

The Python preview adapter:

- finds the `apply_discount` owner in `src/pricing.py`,
- finds the pytest `test_*` function in `tests/test_pricing.py`,
- marks the finding with `language = "python"` and
  `language_status = "preview"`,
- reports `owner_kind: function` in evidence,
- classifies the finding as `weakly_exposed` because assertion extraction
  lands in a later sub-slice.

## Must Not

- Add VS Code selectors or LSP routing.
- Claim parity with Rust evidence.
- Run Python tests or import Python modules.
