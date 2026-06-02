# Fixture: python_no_config_repair_gap

Spec: RIPR-SPEC-0028

## Given

A Python project has a `pyproject.toml`, production source under `src/`, and
pytest tests under `tests/`, but intentionally has no `ripr.toml`.

The changed production owner is a direct predicate-boundary repair case, and
the related pytest test calls the owner but only asserts broad success.

## When

```bash
cargo xtask fixtures python_no_config_repair_gap
```

or:

```bash
ripr check \
  --root fixtures/python_no_config_repair_gap/input \
  --diff fixtures/python_no_config_repair_gap/diff.patch \
  --mode fast
```

## Then

The Python repair-routing surface:

- detects the Python project from `pyproject.toml` without explicit
  configuration,
- maps the diff to `python:src/promotions.py::trial_extension`,
- identifies the missing boundary discriminator `days == minimum_days`,
- emits a bounded repair card with `tests/test_promotions.py` as the suggested
  test file,
- keeps the verify command focused on the related pytest test.

## Must Not

- Require `ripr.toml` for this Python-only project.
- Execute pytest.
- Import the Python project.
- Authorize production-code edits from the repair packet.
- Treat this fixture as a stable Python support claim.
