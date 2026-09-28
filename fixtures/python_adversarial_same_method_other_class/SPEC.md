# Fixture: python_adversarial_same_method_other_class (false-exposed guard — identity over tokens)

Spec: RIPR-SPEC-0028

## Given

An adversarial **same-method-name, different-class** over-credit trap — a live
false-`exposed` found 2026-06-14 and closed in the same change. The changed owner
`TokenValidator.validate` (src/auth.py) flips `token` → `token.strip()`, and the
only related test exercises a **different class** `PaymentProcessor.validate`
(src/billing.py) under a strong exact-value oracle:

```python
# changed owner: TokenValidator.validate
return token.strip() in self._valid

# the ONLY related test — a DIFFERENT class, never imports TokenValidator
from src.billing import PaymentProcessor
proc = PaymentProcessor()
assert proc.validate("card1234 ") == True
```

The test links to the owner only because `body_calls_owner` matches the bare
attribute call `.validate(` on *any* receiver, and the strong oracle text
contains the owner's bare method-name token `validate`. Both signals are token
coincidence, not identity — `proc` is a `PaymentProcessor`, not a
`TokenValidator`.

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr classifies the change `weakly_exposed` with `oracle_alignment: orthogonal`.
For a method / classmethod owner, the bare method-name token does **not** credit
`direct` alignment without owner-class identity: the owner's class token is
observed, or a strong observing test imports the owner's class. This test imports
`PaymentProcessor`, never `TokenValidator`, so there is no identity and the strong
oracle is treated as observing a different sink.

**This fixture must NEVER read `exposed`.** Before the identity gate it read
`exposed`/`strong_oracle_observes_owner_name` — a silent over-credit: a test that
never touches `TokenValidator` was reported as discriminating its changed
behavior. The companion `method_name_with_class_import_identity_credits_exposed`
unit test pins the inverse — a test that *does* import and exercise the owner
class keeps `exposed`.

## Must Not

- Credit `exposed` from a bare method-name / bare `.method(` token match when no
  test provides owner-class identity.
- Run any Python runtime; static preview evidence only.
