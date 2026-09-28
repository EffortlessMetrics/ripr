# Fixture: python_rebound_constant_boundary_limit

Spec: RIPR-SPEC-0028

## Given

The same pricing predicate change as
`python_named_constant_boundary_repair_gap` (`amount > DISCOUNT_THRESHOLD` to
`amount >= DISCOUNT_THRESHOLD`), but a `configure()` function declares
`global DISCOUNT_THRESHOLD` and rebinds it, so the module-level
`DISCOUNT_THRESHOLD = 10_000` is not the only value the name can hold. Two
strong pytest tests call the owner with literal amounts away from `10_000`.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_rebound_constant_boundary_limit
```

## Then

The Python preview adapter:

- does not resolve `DISCOUNT_THRESHOLD`, because a `global` declaration can
  rebind it,
- still fails closed to `weakly_exposed`, stating the unresolved operand,
- names no missing discriminator and emits no Python repair card, so no card
  asks for a test that may already sit on the runtime boundary.

## Must Not

- Treat the first literal binding as the constant's only value.
- Emit `amount == DISCOUNT_THRESHOLD` as a typed repair target.
