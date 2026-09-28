# Fixture: python_adversarial_method_owner_imported_unbound_receiver (false-exposed guard — receiver identity over class identity)

Spec: RIPR-SPEC-0028

## Given

An adversarial **imported-but-unbound-receiver** over-credit trap — the residual
false-`exposed` the #1253 owner-*class* identity gate left open. The changed owner
`TokenValidator.validate` (src/auth.py) flips `token` → `token.strip()`. The only
related test **imports and even constructs** `TokenValidator`, but its strong
exact-value oracle asserts on a **different receiver** — a `PaymentProcessor`:

```python
# changed owner: TokenValidator.validate
return token.strip() in self._valid

# the ONLY related test — imports AND constructs TokenValidator, but the
# asserted .validate( runs on an UNRELATED receiver (proc: PaymentProcessor)
from src.auth import TokenValidator
from src.billing import PaymentProcessor

def test_billing_validate():
    reference = TokenValidator(["card1234"])   # class identity present...
    proc = PaymentProcessor()
    assert proc.validate("card1234 ") == True  # ...receiver identity absent
```

The #1253 gate credited `exposed` here because it only required the owner class to
be imported and observed *somewhere* in the test body — and `TokenValidator` is
both imported and constructed. But the changed method never runs: the asserted
`.validate(` is on `proc`, a `PaymentProcessor`. Class identity is not receiver
identity.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr classifies the change `weakly_exposed` with `oracle_alignment: orthogonal`.
For a method / classmethod owner, the bare method-name token credits `direct`
alignment only when a strong observing test calls the owner method on a receiver
**statically bound to the owner class** — inline `TokenValidator(...).validate(...)`,
a local binding `v = TokenValidator(...); v.validate(...)`, or a classmethod /
direct call `TokenValidator.validate(...)`. Here the only bound name (`reference`)
is never asserted on, and the asserted receiver (`proc`) is a different class, so
the strong oracle is treated as observing a different sink.

**This fixture must NEVER read `exposed`.** Constructing or importing the owner
class is not evidence that the changed method ran. The companion unit tests pin
the inverse positives: `method_name_inline_construct_call_credits_exposed`,
`method_name_classmethod_direct_call_credits_exposed`, and
`method_name_with_class_import_identity_credits_exposed` (local binding) keep
`exposed` when the receiver is genuinely bound to the owner class.

## Must Not

- Credit `exposed` from a bare method-name / bare `.method(` token match when the
  asserted receiver is not statically bound to the owner class — even if the owner
  class is imported and constructed elsewhere in the test.
- Run any Python runtime; static preview evidence only.
