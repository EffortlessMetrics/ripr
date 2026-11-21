# Fixture: python_adversarial_attribute_sink_other_receiver (false-exposed guard — attribute receiver/value identity)

Spec: RIPR-SPEC-0028

## Given

An adversarial **attribute changed-sink, different-receiver** over-credit trap
(Cluster A). The changed owner `Session.refresh` (src/session.py) flips an
attribute write `self.status = "idle"` → `self.status = "active"`. The only
related test reaches the owner but its strong exact-value oracle asserts on a
**different object's** same-named attribute:

```python
# changed owner: Session.refresh
self.status = "active"

# the ONLY related test — reaches Session.refresh, but the strong oracle
# observes a DIFFERENT receiver's `.status` with a DIFFERENT value
from src.session import Session
from src.conn import Conn

def test_session_refresh():
    Session().refresh()
    conn = Conn()
    assert conn.status == "closed"
```

The test links to the owner because `Session().refresh()` matches the bare
attribute call, and the changed line yields the bare token `status`, which the
oracle text also contains via `conn.status`. Both are token coincidence: `conn`
is a `Conn`, not the changed `Session`, and the asserted value `"closed"` is not
the changed value `"active"`.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr does **not** classify the change `exposed`; the oracle alignment is not
`changed_sink_token`. For an attribute-assignment changed sink `recv.attr =
value`, the bare attribute token credits `changed_sink_token` only when a strong
oracle observes the receiver-qualified `recv.attr` (same receiver), or observes
the assigned **value** together with the attribute name. Here the oracle observes
a different receiver (`conn`) and a different value (`"closed"`), so the bare
`status` token is treated as observing a different sink.

**This fixture must NEVER read `exposed`.** Before the receiver/value gate it read
`exposed`/`strong_oracle_observes_changed_sink_token` — a silent over-credit on a
ubiquitous field name. The companion `python_field_assignment_shape` fixture pins
the inverse: when the oracle observes the assigned value `"paid"` on the changed
field, the change stays `exposed` (value observation is change-specific evidence).

**Known residual (deliberate tradeoff):** an oracle that observes the *same*
attribute name AND the *same* value on a *different* receiver (e.g. changed
`session.status = "active"`, oracle `conn.status == "active"`) still credits via
the value-and-attr path, since this gate verifies value/attribute evidence, not
receiver identity for the attribute sink. Fully closing that corner needs
attribute-receiver identity (analogous to the method-owner receiver-binding work)
and is out of scope here; it is a target for a later slice and red-team round 2.

## Must Not

- Credit `exposed` from a bare attribute-name token match when the strong oracle
  observes a different receiver and a different value than the changed sink.
- Run any Python runtime; static preview evidence only.
