# Fixture: python_same_stem_sibling_owner_not_related (a same-stem test must reference the owner)

Spec: RIPR-SPEC-0028

## Given

A flat-layout Python project: `pricing.py` sits at the repository root and
`tests/test_pricing.py` holds its pytest tests. Both tests call only
`discounted_total`:

```python
# tests/test_pricing.py
from pricing import discounted_total

def test_no_discount_below_threshold():
    assert discounted_total(5000) == 5000

def test_discounts_far_above_threshold():
    assert discounted_total(20000) == 18000
```

The diff changes the `discounted_total` threshold comparison and adds a new
`loyalty_price` function that no test references.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_same_stem_sibling_owner_not_related
```

or:

```bash
ripr check \
  --root fixtures/python_same_stem_sibling_owner_not_related/input \
  --diff fixtures/python_same_stem_sibling_owner_not_related/diff.patch \
  --mode fast
```

## Then

- The `discounted_total` change keeps both tests as direct `syntactic_call`
  relations.
- Every `loyalty_price` line reads `no_static_path` with reach `no`,
  `0 related Python test(s) found for owner \`loyalty_price\``, and
  `No Python test references \`loyalty_price(\``. The shared file stem
  (`test_pricing` / `pricing`) does not relate a test that never references the
  owner.

## Must Not

- Relate the `discounted_total` tests to `loyalty_price` through the file stem,
  the test name, or a fixture name.
- Report `loyalty_price` as `weakly_exposed` with heuristic Python test links.
- Run any Python runtime; static preview evidence only.
