# Fixture: python_cross_file_construct_call (cross-file construct-call linking)

Spec: RIPR-SPEC-0028

## Given

A callable class changes a predicate boundary inside its `__call__` method, and a
strong discriminating test in a **different file** (whose stem does not match the
owner's, so name proximity cannot link it) invokes the owner through an **inline
construct-call** `OwnerClass()(...)` rather than naming `__call__`:

```python
# src/formatter.py            (owner: Formatter.__call__)
if any(c < " " for c in key):    # changed from `<= " "`
    raise ValueError(f'Invalid key: "{key}"')

# tests/test_render.py        (stem "render" != owner stem "formatter")
from src.formatter import Formatter
with pytest.raises(ValueError, match='Invalid key'):
    Formatter()({"bad key": "value"})
```

This is the structlog `LogfmtRenderer()(...)` shape that the Tier B panel measured
as a true false-actionable (#1160): a strong exact-error oracle that genuinely
discriminates the change, which ripr missed because its relation heuristics never
linked the test (same-stem reached the wrong file; the test never names
`__call__`).

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr classifies the change `exposed` with `oracle_alignment: changed_sink_token`,
linking the test via a `construct_call` relation. The `pytest.raises(ValueError,
match='Invalid key')` oracle is strong (ExactErrorVariant) and observes the
changed `key`-validation sink (the changed token `key` appears as a whole word in
the match string), so static evidence supports a complete RIPR path.

Without construct-call linking the finding is `no_static_path` (the cross-file
test is never reached) — this fixture is a regression guard proving the
`ConstructCall` relation flips it to `exposed`.

## Must Not

- Link a bound local `x = OwnerClass(); x(...)` — only an **inline** construct-call
  `OwnerClass()(...)` qualifies (the local-binding case stays `weakly_exposed`,
  per python_local_callable_binding).
- Link a same-named class from an unrelated module: the test must import the
  owner's class.
- Credit `exposed` when the changed owner is not the `__call__` method.
- Run any Python runtime; static preview evidence only.
