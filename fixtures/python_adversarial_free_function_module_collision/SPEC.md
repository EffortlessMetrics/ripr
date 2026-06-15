# Fixture: python_adversarial_free_function_module_collision (false-exposed guard — free-function module identity)

Spec: RIPR-SPEC-0028

## Given

An adversarial **same-named free function, different module** over-credit trap
(Cluster B). The changed owner `src/handler.py::validate` flips `payload` →
`payload.strip()`. The only related test imports a **same-named free function
from a different module** (`src/checker.py::validate`) under a strong oracle:

```python
# changed owner: src.handler.validate
return payload.strip() == "ok"

# the ONLY related test — imports validate from a DIFFERENT module (src.checker)
from src.checker import validate

def test_checker_validate():
    assert validate("ok") is True
```

The test links to the owner only because both functions are named `validate`,
and the strong oracle text contains the bare token `validate`. That is token
coincidence, not identity: the test exercises `src.checker.validate`, never
`src.handler.validate`. Before this guard the parser discarded the import's
source module, so the analyzer could not tell the two `validate`s apart.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr does **not** classify the change `exposed`. For a free-function owner, the
bare function-name token credits `direct` (or an import alias credits `alias`)
only when a strong observing test imports the function **from the owner's
module** (`from src.handler import validate`). Here the import source module is
`src.checker`, not `src.handler`, so the bare `validate` token is treated as
observing a different function.

**This fixture must NEVER read `exposed`.** The companion unit tests pin the
inverse positives: `from src.handler import normalize` (same module) and
`from src.handler import normalize as norm` (aliased, same module) keep
`exposed`.

## Must Not

- Credit `exposed`/`alias` for a free-function owner from a same-named function
  imported from a different module.
- Run any Python runtime; static preview evidence only.
