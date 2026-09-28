# Fixture: python_adversarial_fstring_length_invariant_aggregate (false-exposed guard — length-invariant f-string change observed only via len)

Spec: RIPR-SPEC-0028

## Given

A length-invariant f-string change: the literal prefix `"OK:"` -> `"NO:"` (equal
length, interpolation `{code}` unchanged), so the output length is identical for any
input. The only strong related oracle observes `len(...)`:

```python
def status_label(code):
    return f"NO:{code}"
```

```python
from src.status import status_label

def test_len():
    assert len(status_label(7)) == 4   # aggregate; length unchanged by the edit
```

The oracle calls the owner, so a naive `direct` alignment would credit `exposed`. But
`len("OK:7") == len("NO:7") == 4`, so the `len(...)` oracle passes identically under
the old and new code — it cannot notice the changed prefix.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr does **not** classify the change `exposed`. For a length-invariant f-string
change (only equal-length literal text changed, interpolations unchanged), a strong
oracle that observes the owner's output solely through a `len(...)` aggregate is not a
discriminator. A format-spec change (which alters an interpolation, so it is not
length-invariant) or an exact string-comparison oracle keeps the credit — companion
unit tests pin both.

## Must Not

- Credit `exposed` when a length-invariant f-string change is observed only via a
  `len(...)` aggregate.
- Regress the #1298 f-string-is-not-a-dict fix, the #1297 dict gate, or the #1299 list
  gate; preserve format-spec and exact-string-comparison f-string discriminators.
- Run any Python runtime; static preview evidence only.
