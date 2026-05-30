# Fixture: python_opaque_custom_helper_limit

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a simple predicate boundary, and a pytest
test reaches the owner through a direct call but observes the result only
through a custom assertion helper.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_opaque_custom_helper_limit
```

or:

```bash
ripr check \
  --root fixtures/python_opaque_custom_helper_limit/input \
  --diff fixtures/python_opaque_custom_helper_limit/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `apply_discount` owner,
- records the direct pytest relation and unknown custom-helper oracle shape,
- emits `static_limit_kind = "opaque_custom_assertion_helper"`,
- keeps the finding `static_unknown`,
- omits canonical repair-gap identity, missing discriminator facts, repair
  cards, and verify-command placement.

## Must Not

- Inspect or execute the custom assertion helper body.
- Route the finding into an agent packet.
- Claim the helper supplies a bounded discriminator for the changed boundary.
