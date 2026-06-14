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

The relation diagnosis is misleading (it implies no direct test exists), but the
`weakly_exposed` classification is the **conservative direction** (under-credit /
over-suggest, never over-credit). This fixture pins that current behavior
honestly.

**Correction (2026-06-13): the resolved state is NOT `exposed`.** The discriminating
assertion here is `assertTrue(stop(3))` — a *broad boolean* oracle, which
`oracle_for_call` classifies as `OracleStrength::Smoke`. The sibling golden
`python_broad_boolean_assertion` deliberately pins the same shape
(`assert is_priority(100)`, a *direct* call on the changed predicate owner) as
`weakly_exposed`/`smoke`: a single truthy check does not pin the boundary, so it
is a weak oracle *by contract*. Flipping this fixture to `exposed` would
contradict that golden and drift `ripr` back toward coverage. When the
limitation is resolved (tracker
`analysis/python-local-callable-instance-alignment`) — by tracing a single
unambiguous `local = OwnerClass(...)` binding so the relation links **direct** —
this fixture should resolve to `weakly_exposed` with `oracle_strength: smoke` and
a **direct** relation (matching `python_broad_boolean_assertion`), and a repair
card that says "strengthen this broad-boolean assertion into an exact-value
assertion" rather than "no direct test found". It must **not** flip to `exposed`.
The cases that legitimately flip to `exposed` are the *strong* missed oracles
(exact-value / exact-error reached through indirect calls — see tracker
`analysis/python-cross-file-strong-oracle-relation`), not this smoke case.

## Must Not

- Over-credit: a strong oracle that observes a genuinely different sink, or a
  reassigned / `Retrying`-wrapper-routed binding, must stay `weakly_exposed`.
- Run any Python runtime; static preview evidence only.
