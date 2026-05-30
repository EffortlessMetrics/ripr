# Fixture: python_property_based_limit

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a predicate boundary, and a pytest test
reaches the owner through a direct call inside a Hypothesis-style
property-based test.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_property_based_limit
```

or:

```bash
ripr check \
  --root fixtures/python_property_based_limit/input \
  --diff fixtures/python_property_based_limit/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `apply_discount` owner,
- records the direct pytest relation and the broad property assertion,
- emits `static_limit_kind = "property_based_test"`,
- keeps the finding `static_unknown`,
- omits canonical repair-gap identity, missing discriminator facts, repair
  cards, and verify-command placement.

## Must Not

- Execute Hypothesis or infer generated example values.
- Assume the generated cases include `amount == threshold`.
- Route the finding into an agent packet.
