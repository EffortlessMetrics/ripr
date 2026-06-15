# Fixture: python_local_callable_binding (resolved: direct relation, smoke oracle)

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

ripr classifies the changed predicate `weakly_exposed` and links the test via a
**`local_binding`** relation — an *oracle-eligible, direct* relation that traces
the single unambiguous binding `stop = stop_after_attempt(3)` to its call
`stop(3)`. Because the relation is direct, the existing assertion is surfaced:
`oracle_strength: smoke` / `oracle_kind: smoke_only` / `oracle:
self.assertTrue(stop(3))`, and the repair card reads **"strengthen the existing
unittest assertion"** (`repair_action: strengthen_existing_test`) rather than the
misleading "no direct test found".

The `weakly_exposed` classification is the correct **conservative direction**
(under-credit / over-suggest, never over-credit): a single truthy check does not
pin the `>`→`>=` boundary, so the oracle stays `smoke` and the class stays
`weakly_exposed` *by contract*.

**The resolved state is NOT `exposed`.** The discriminating assertion here is
`assertTrue(stop(3))` — a *broad boolean* oracle, which `oracle_for_call`
classifies as `OracleStrength::Smoke`. The sibling golden
`python_broad_boolean_assertion` deliberately pins the same shape
(`assert is_priority(100)`, a *direct* call on the changed predicate owner) as
`weakly_exposed`/`smoke`. Flipping this fixture to `exposed` would contradict that
golden and drift `ripr` back toward coverage. The
`analysis/python-local-callable-instance-alignment` work resolved the **relation
diagnosis** (link direct, surface the smoke oracle, correct the card) WITHOUT
changing the class — `exposed` would require a `Strong` oracle. The cases that
legitimately flip to `exposed` are the *strong* missed oracles (exact-value /
exact-error reached through indirect calls — see tracker
`analysis/python-cross-file-strong-oracle-relation`, landed in #1228), not this
smoke case.

## Must Not

- Over-credit: a strong oracle that observes a genuinely different sink, or a
  reassigned / `Retrying`-wrapper-routed binding, must stay `weakly_exposed`.
- Run any Python runtime; static preview evidence only.
