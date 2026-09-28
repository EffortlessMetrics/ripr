# Fixture: python_src_layout_repair_gap

Spec: RIPR-SPEC-0028

## Given

A Python project uses an external-repo-style `src/` package layout with a
pyproject file, a production package under `src/shop_service/`, and pytest tests
under `tests/`.

The changed production owner is a direct predicate-boundary repair case, and
the related pytest test calls the owner but only asserts broad success.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_src_layout_repair_gap
```

or:

```bash
ripr check \
  --root fixtures/python_src_layout_repair_gap/input \
  --diff fixtures/python_src_layout_repair_gap/diff.patch \
  --mode fast
```

## Then

The Python repair-routing surface:

- maps the diff to `python:src/shop_service/pricing.py::loyalty_discount`,
- identifies the missing boundary discriminator
  `subtotal == loyalty_threshold`,
- emits a bounded repair card with `tests/test_pricing.py` as the suggested
  test file,
- keeps the verify command focused on the related pytest test.

## Must Not

- Execute pytest.
- Import the `shop_service` package.
- Authorize production-code edits from the repair packet.
- Treat this fixture as a stable Python support claim.
