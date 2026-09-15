# Fixture: python_adversarial_dict_field_sibling_key (false-exposed guard — dict change must be observed at the changed key)

Spec: RIPR-SPEC-0028

## Given

An adversarial **sibling-key** dict-literal change. `build_config` changes only the
`port` value (`8080` -> `9090`); the only strong related oracle observes the
**unchanged sibling key** `host`, never the changed `port`:

```python
# changed owner: build_config  (delta: the "port" value, 8080 -> 9090)
def build_config():
    return {"host": "localhost", "port": 9090}
```

```python
# the only related test — observes the UNCHANGED sibling key `host`
from src.conf import build_config

def test_host():
    assert build_config()["host"] == "localhost"
```

The oracle reaches and calls the owner, so a naive `direct` alignment (a strong
oracle observes the owner name) would credit `exposed`. But the change is localized
to the `port` key, which the test never observes — it asserts on `host`, which is
identical under both the old and new dict. The test cannot notice if the `port`
change were wrong.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr does **not** classify the change `exposed`. For a dict-literal
field-construction change, the credit requires a strong oracle to observe the
**changed element** — a subscript / `.get(...)` of the changed key, the changed
value literal, or a whole-collection comparison — not a sibling key or an aggregate
like `len(...)`. Here the only oracle observes the sibling `host`, so the change is
treated as observing a different sink. Companion unit tests pin the inverse: an
oracle that observes the changed key's value (`["port"] == 9090`) or the whole dict
stays `exposed`.

## Must Not

- Credit `exposed` when the only strong oracle observes a sibling dict key (or an
  aggregate) of a localized dict-literal change.
- Regress dict/field fixtures whose oracle observes the changed key or the whole
  result (e.g. `python_field_assignment_shape`, `python_model_field_repair_gap`).
- Run any Python runtime; static preview evidence only.
