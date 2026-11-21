# Fixture: python_adversarial_changed_sink_non_delta_operand (false-exposed guard — changed-sink token must be the delta)

Spec: RIPR-SPEC-0028

## Given

An adversarial **non-delta operand** over-credit trap. The changed owner
`Account.balance` (a `@property`) wraps its return in `max(0, ...)`; the only
related test reads the **unchanged backing field** `_balance` and never invokes
the changed property:

```python
# changed owner: Account.balance  (delta: wrap in max(0, ...))
return max(0, self._balance)

# the ONLY related test — observes the UNCHANGED operand `_balance`, never the property
from src.account import Account

def test_account_init():
    account = Account(100)
    assert account._balance == 100
```

The changed line contains the token `_balance`, but `_balance` is **unchanged**
between the old and new line — the behavior delta is the added `max(...)` wrap.
The oracle observes `_balance` (an init field), so it cannot notice if the
`max(...)` behavior were wrong.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr does **not** classify the change `exposed`. The `changed_sink_token`
alignment credits only when a strong oracle observes a token that is part of the
**delta** — the tokens that differ between the removed and added line — not an
unchanged operand that merely appears on the changed line. Here the only delta
token is `max`, which the oracle does not observe, so the change is treated as
observing a different sink.

**This fixture must NEVER read `exposed`.** The companion unit test
`changed_sink_token_credits_when_oracle_observes_the_delta_value` pins the
inverse: when the oracle observes the changed VALUE (the actual delta, e.g.
`status == "paid"` after `"settled"` -> `"paid"`), the change stays `exposed`.

## Must Not

- Credit `exposed` from a `changed_sink_token` match on an unchanged operand of
  the changed line (a token present in both the old and new line).
- Run any Python runtime; static preview evidence only.
