# Fixture: python_named_constant_boundary_repair_gap

Spec: RIPR-SPEC-0028

## Given

A Python production function compares against a module-level named constant
and changes the predicate from:

```python
amount > DISCOUNT_THRESHOLD
```

to:

```python
amount >= DISCOUNT_THRESHOLD
```

with `DISCOUNT_THRESHOLD = 10_000` bound once at module scope and never
rebound. Two strong pytest tests call the owner with literal amounts away from
the boundary (`5_000`, `20_000`). This is the onboarding Python pricing sample
(#4227).

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_named_constant_boundary_repair_gap
```

## Then

The Python preview adapter:

- resolves `DISCOUNT_THRESHOLD` to its literal `10000` (module constant at
  line 1),
- classifies the changed predicate as `weakly_exposed` because no strong
  related call places `amount` at `10000`,
- names `amount == DISCOUNT_THRESHOLD` as the missing discriminator, with the
  constant's value and the observed `amount` values in its reason,
- emits a Python repair card whose boundary rows are `DISCOUNT_THRESHOLD - 1`,
  `DISCOUNT_THRESHOLD` and `DISCOUNT_THRESHOLD + 1`.

## Must Not

- Run pytest or import the Python project.
- Invent expected output values for the boundary rows.
- Resolve a constant that the module can rebind
  (`python_rebound_constant_boundary_limit` is the negative case).
