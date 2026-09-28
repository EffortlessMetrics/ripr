# Fixture: python_noop_bare_var_annotation (no-op guard — module-scope annotation-only var change has no behavior delta)

Spec: RIPR-SPEC-0028

## Given

A Python diff whose only change is a **module-scope variable type annotation** on
`CACHE_TTL` (`int` -> `str`), with the bound value unchanged. Python does not enforce
annotations at runtime at module scope, so the module's behavior is unchanged:

```python
# changed line: src/config.py line 1  (annotation-only: CACHE_TTL: int -> CACHE_TTL: str)
CACHE_TTL: str = 30


def get_ttl():
    return CACHE_TTL
```

```python
# the only related test — a strong exact-value oracle on a reachable owner's output
from src.config import get_ttl

def test_ttl():
    assert get_ttl() == 30
```

The strong `== 30` oracle reaches the module (via `get_ttl`, which returns `CACHE_TTL`)
and observes the output, so a naive classifier would credit `exposed`. But the changed
line touches **only an annotation** at module scope: the target name and the bound value
are byte-identical before and after, so there is no behavior delta for a test to
discriminate.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr emits **no behavior probe** for the module-scope annotation-only change — the
finding set is empty and nothing is classified `exposed`. A value change on the same
line (`= 30` -> `= 60`) is behavioral and still classifies normally (a companion unit
test pins this).

## Must Not

- Classify a module-scope annotation-only variable change `exposed`, or emit a behavior
  probe for it.
- Suppress a value change, a target rename, or an added/removed value on the same line
  (all behavioral, must still classify).
- Suppress an annotation-only change inside a class body — `@dataclass` / Pydantic /
  `attrs` make class-body annotations runtime-meaningful (validation/coercion), and
  base-class tracking does not exist yet, so the guard is module-scope only and fails
  closed for class bodies.
- Run any Python runtime; static preview evidence only.
