# Fixture: python_adversarial_mock_call_not_value (adversarial false-exposed guard)

Spec: RIPR-SPEC-0028

## Given

An adversarial **mock-observes-the-call-not-the-sink** over-credit trap. The
changed owner `send_alert` alters a *value* it passes downstream (`"priority":
"high"` → `"critical"`), and the only related test reaches `send_alert(...)` but
its oracle asserts that the call *happened*, not what value was passed:

```python
# changed sink: the "priority" payload value
return client.post("/alerts", {"level": level, "priority": "critical"})

# oracle observes that .post was called, NOT the changed value
client = MagicMock()
send_alert(client, "error")
client.post.assert_called_once()
```

The test genuinely reaches the owner (`SyntacticCall`), but `assert_called_once()`
is a *mock-expectation* oracle: it cannot discriminate `"high"` from `"critical"`.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr classifies the change `weakly_exposed`. The mock-expectation oracle is
`OracleStrength::Medium` (rank 4) — below `Strong` (rank 5) — so the `exposed`
branch is unreachable and `oracle_alignment` stays `unknown` /
`no_strong_oracle`. This is the contract-honoring conservative direction: a call
assertion verifies reachability, not discrimination.

**This fixture must NEVER read `exposed`.** Promoting mock-expectation oracles to
`Strong`, or crediting reach-plus-mock as discrimination, would flip this to a
false `exposed` — the test would then be reported as discriminating a value it
provably does not observe.

## Must Not

- Credit `exposed` from a `mock_expectation` (call-happened) oracle.
- Run any Python runtime; static preview evidence only.
