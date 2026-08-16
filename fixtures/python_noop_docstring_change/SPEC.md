# Fixture: python_noop_docstring_change (no-op guard — docstring-only change has no behavior delta)

Spec: RIPR-SPEC-0028

## Given

A Python diff whose only change is the **docstring** of `discount` — a
documentation edit with no runtime behavior. The function body and the related
test are unchanged:

```python
# changed owner: src/pricing.py::discount  (docstring-only edit)
def discount(price):
    """Apply the standard discount to a price."""   # was: "Apply a discount."
    return price * 0.8
```

```python
# the only related test — a strong exact-value oracle on the owner's output
from src.pricing import discount

def test_discount():
    assert discount(100) == 80
```

The strong `== 80` oracle reaches and observes the owner's output, so a naive
sink-alignment would credit `exposed`. But the **changed line carries no behavior
delta**: a docstring edit cannot change what `discount` returns, so there is
nothing for a test to discriminate.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr emits **no behavior probe** for the docstring change — a no-op /
equivalent-mutant change (docstring-only, comment-only, or blank-line) has no
behavior delta, so the finding set is empty and nothing is classified `exposed`.
A real body change to the same function (e.g. `* 0.9` -> `* 0.8`) still
classifies normally; this fixture isolates the no-op line.

## Must Not

- Classify a docstring-only (or comment-only, or blank-line) change `exposed`.
- Emit a behavior probe for a changed line that is entirely a bare string literal
  expression statement (a docstring) or a comment.
- Run any Python runtime; static preview evidence only.
