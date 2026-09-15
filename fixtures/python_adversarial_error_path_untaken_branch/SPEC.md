# Fixture: python_adversarial_error_path_untaken_branch (false-exposed guard — a raise change needs an exception oracle)

Spec: RIPR-SPEC-0028

## Given

A raised-exception type change (`ValueError` -> `KeyError`) on a branch guarded by
`if not text:`. The only strong related oracle is a **normal-path value assertion**
that never triggers the raise:

```python
def parse(text):
    if not text:
        raise KeyError("empty")
    return int(text)
```

```python
from src.parseint import parse

def test_parse_ok():
    assert parse("42") == 42   # "42" is truthy -> the raise branch is never reached
```

The oracle calls the owner, so a naive `direct` alignment would credit `exposed`. But
`parse("42")` never enters the `if not text:` branch, so the changed raise is never
exercised; the test passes identically under `ValueError` and `KeyError`.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr does **not** classify the change `exposed`. A raise / error-path change is
discriminated only by an oracle that observes the **raised exception**
(`pytest.raises` / `assertRaises`); a strong normal-path value oracle does not. A
companion unit test pins the inverse: the same change with a `pytest.raises(KeyError)`
oracle stays `exposed`.

## Must Not

- Credit `exposed` for an error-path change observed only by a normal-path value
  oracle.
- Regress error-path fixtures whose oracle is an exception assertion
  (`python_cross_file_construct_call` and others stay exposed).
- Run any Python runtime; static preview evidence only.
