# Fixture: python_unresolved_fixture_limit

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a predicate boundary, and a pytest test
reaches the owner through a direct call whose input and expected value come
from a fixture parameter.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_unresolved_fixture_limit
```

or:

```bash
ripr check \
  --root fixtures/python_unresolved_fixture_limit/input \
  --diff fixtures/python_unresolved_fixture_limit/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `apply_discount` owner,
- records the direct pytest relation and the fixture-sourced exact assertion,
- emits `static_limit_kind = "unresolved_pytest_fixture"`,
- keeps the finding `static_unknown`,
- omits canonical repair-gap identity, missing discriminator facts, repair
  cards, and verify-command placement.

## Must Not

- Execute pytest or fixture setup.
- Infer that `discount_case` contains `amount == threshold`.
- Route the finding into an agent packet.
