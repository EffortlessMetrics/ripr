# Fixture: python_local_callable_binding (documented limitation)

Spec: RIPR-SPEC-0028

## Given

A Python callable class changes the predicate boundary inside its `__call__`
method, and a unittest test binds a local variable directly to the class
constructor and then calls it — the tenacity `stop_after_attempt` shape that the
Tier B starter judging measured as the one false-actionable (#1160):

```python
stop = stop_after_attempt(3)
self.assertTrue(stop(3))
```

The fixture workspace enables the Python preview adapter explicitly
(`input/ripr.toml`). The changed owner is `stop_after_attempt.__call__`. The
`assertTrue(stop(3))` assertion *does* discriminate the `>`→`>=` boundary flip
(`3 >= 3` is `True`; the `>` bug makes `3 > 3` `False` and the assertion fails),
because `stop` is a single unambiguous binding to the owner's class.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr classifies the changed predicate `weakly_exposed` (a known, conservative
limitation). It links the test only by **`same_stem` name proximity** (an
*uncertain* relation) and extracts **no oracle** (`oracle_strength: unknown`),
because the test reaches the owner through the local binding `stop(3)` rather
than naming `stop_after_attempt` / `__call__` directly — so neither the relation
heuristic nor oracle extraction traces the local-callable binding.

This under-credits a behavior that is in fact discriminated, but it is the
**conservative direction** (under-credit / over-suggest, never over-credit): it
is the false-actionable measured on tenacity, not a dangerous false-`exposed`.
This fixture pins that current behavior honestly. When the limitation is resolved
(tracker `analysis/python-local-callable-instance-alignment`) — by tracing a
single unambiguous `local = OwnerClass(...)` binding in **relation + oracle
extraction**, not only in sink-alignment — this fixture should flip to `exposed`.

## Must Not

- Over-credit: a strong oracle that observes a genuinely different sink, or a
  reassigned / `Retrying`-wrapper-routed binding, must stay `weakly_exposed`.
- Run any Python runtime; static preview evidence only.
