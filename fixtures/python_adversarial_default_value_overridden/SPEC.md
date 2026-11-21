# Fixture: python_adversarial_default_value_overridden (false-exposed guard — a changed default needs a call that omits the parameter)

Spec: RIPR-SPEC-0028

## Given

A function whose default argument value changes (`verbose=False` -> `verbose=True`).
The only strong related oracle calls the owner while **explicitly overriding** the
changed parameter, so the changed default is never reached:

```python
def render(name, verbose=True):
    return f"[debug] {name}" if verbose else name
```

```python
from src.render import render

def test_render_explicit_verbose_false():
    assert render("Sam", verbose=False) == "Sam"   # passes the default explicitly
```

The oracle reaches the owner with a strong exact-value assertion, so a naive
`direct` alignment would credit `exposed`. But `render("Sam", verbose=False)` binds
`verbose` explicitly; it never uses the default, so the assertion passes identically
whether the default is `False` or `True`. The changed default is not discriminated.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr does **not** classify the change `exposed`. A changed default VALUE is
discriminated only by a call that **omits** the parameter (and so reaches the
default). The `missing` text names the parameter to exercise by omission. A
companion unit test pins the inverse: the same change with a call that omits
`verbose` (`render("Sam")`) and a strong oracle stays `exposed`.

## Must Not

- Credit `exposed` for a default-value change when every strong related call binds
  the changed parameter explicitly (keyword or positional).
- Suppress a genuine exposure: a call that omits the parameter still exercises the
  changed default and stays `exposed`.
- Block on anything other than a pure default-value change (an added/removed
  default, a renamed parameter, or a method/classmethod owner all fail open).
- Run any Python runtime; static preview evidence only.
