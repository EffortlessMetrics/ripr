# Fixture: python_parametrized_boundary_repair_gap

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a threshold predicate from `>` to `>=`.

A related pytest test reaches the changed owner but uses only broad smoke
evidence. The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_parametrized_boundary_repair_gap
```

or:

```bash
ripr check \
  --root fixtures/python_parametrized_boundary_repair_gap/input \
  --diff fixtures/python_parametrized_boundary_repair_gap/diff.patch \
  --mode fast
```

## Then

The Python repair card:

- keeps `amount == threshold` as the smallest useful equality-boundary repair,
- suggests an optional pytest parameterized shape with below/equal/above rows,
- labels expected values as project/domain-specific instead of inventing them,
- adds a stop condition when below/above expected values are unclear.

## Must Not

- Execute pytest or import the Python project.
- Claim that parameterized rows are required for gap closure.
- Invent expected output values for the below/equal/above cases.
