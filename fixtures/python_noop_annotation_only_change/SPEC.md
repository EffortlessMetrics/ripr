# Fixture: python_noop_annotation_only_change (no-op guard — annotation-only def change has no behavior delta)

Spec: RIPR-SPEC-0028

## Given

A Python diff whose only change is a **parameter type annotation** on `discount`
(`int` -> `str`). Python does not enforce annotations at runtime, so the callable's
behavior is unchanged:

```python
# changed owner: src/pricing.py::discount  (annotation-only: amount: int -> amount: str)
def discount(amount: str) -> int:
    return amount
```

```python
# the only related test — a strong exact-value oracle on the owner's output
from src.pricing import discount

def test_discount_passthrough():
    assert discount(100) == 100
```

The strong `== 100` oracle reaches and observes the owner's output, so a naive
classifier would credit `exposed`. But the changed line touches **only an
annotation**: the runtime signature (parameter name, default, order, async-ness) is
unchanged, so there is no behavior delta for a test to discriminate.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr emits **no behavior probe** for the annotation-only change — the finding set is
empty and nothing is classified `exposed`. A change to a parameter or return
annotation that leaves the runtime signature unchanged is a no-op. A default-VALUE
change on the same header (`size=10` -> `size=20`) is behavioral and still classifies
normally (a companion unit test pins this).

## Must Not

- Classify an annotation-only `def` change `exposed`, or emit a behavior probe for it.
- Suppress a default-value change, a parameter rename/add/remove, a positional-only /
  keyword-only marker move, or an async-ness change (all behavioral, must still
  classify).
- Run any Python runtime; static preview evidence only.
