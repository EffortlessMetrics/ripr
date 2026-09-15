# Fixture: python_adversarial_list_element_sibling_index (false-exposed guard — list change observed at the changed index)

Spec: RIPR-SPEC-0028

## Given

A list-literal change at index 1 (`"search"` -> `"browse"`); the only strong related
oracle observes the **unchanged sibling index 0**, never the changed position:

```python
def route_order():
    return ["index", "browse", "detail"]
```

```python
from src.routes import route_order

def test_first():
    assert route_order()[0] == "index"   # sibling index, unchanged
```

The oracle calls the owner, so a naive `direct` alignment would credit `exposed`. But
the change is at index 1, which the test never observes; index 0 is identical under
both the old and new list.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr does **not** classify the change `exposed`. For a list-literal field-construction
change, the credit requires a strong oracle to observe the **changed index** (a
`[1]` subscript), the changed element value, or a whole-collection comparison — not a
sibling index or an aggregate like `len(...)`. A companion unit test pins the inverse
(`[1] == "browse"` stays `exposed`).

## Must Not

- Credit `exposed` when the only strong oracle observes a sibling list index.
- Regress dict behavior (#1297) or the f-string-is-not-a-dict fix (#1298).
- Run any Python runtime; static preview evidence only.
