# Fixture: python_src_layout_package_import (src-layout module identity)

Spec: RIPR-SPEC-0028

## Given

A PyPA **src layout** project: production code lives under `src/pricing/`, the
import root is `src/` (an installed package, or pytest `pythonpath = ["src"]`),
so tests import the package by its name, not by the repository path. The
fixture deliberately carries no packaging config: ripr derives the import root
from the `src` directory alone:

```python
# tests/test_discounts.py
from pricing.discounts import bulk_discount

def test_bulk_discount_large_order():
    assert bulk_discount(101) == 0.15
```

The diff changes the boundary of two same-named free functions:

- `src/pricing/discounts.py::bulk_discount` — the module the test imports;
- `src/pricing/legacy.py::bulk_discount` — a different module no test imports.

## When

```bash
cargo xtask fixtures python_src_layout_package_import
```

## Then

- `src/pricing/discounts.py:2` is `exposed` with `oracle_alignment = direct`:
  `from pricing.discounts import bulk_discount` is a real import of the owner's
  module once the `src` import root is stripped (`src.pricing.discounts` →
  `pricing.discounts`), so the strong exact-value oracle carries free-function
  module identity, exactly as in the equivalent flat layout.
- `src/pricing/legacy.py:2` stays `weakly_exposed` with
  `alignment_reason = strong_oracle_observes_different_sink`: the test imports
  `bulk_discount` from `pricing.discounts`, not `pricing.legacy`, so the
  same-named call is token coincidence, not identity.

## Must Not

- Report the src-layout owner as `strong_oracle_observes_different_sink` when a
  strong test imports it through its package name (the repair loop could never
  close: the card would ask to strengthen a test that already pins the value).
- Credit a same-named function from a different module, a bare file stem, or an
  arbitrary path suffix as module identity; only an exact dotted module path
  (repository-relative, or below a `src` import root) counts.
- Depend on packaging config or run any Python runtime; static preview evidence only.
