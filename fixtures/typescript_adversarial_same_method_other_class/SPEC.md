# Fixture: typescript_adversarial_same_method_other_class (false-exposed guard — identity over tokens)

Spec: RIPR-SPEC-0108

Corpus case: `ts_same_method_other_class` in
`fixtures/evidence-promotion-honesty-corpus/corpus.json` (issue #1983,
TypeScript family "same method name on a different class/receiver").

## Given

An adversarial **same-method-name, different-class** over-credit trap, the
TypeScript analogue of `fixtures/python_adversarial_same_method_other_class`.
The changed owner `TokenValidator.validate` (src/auth.ts) flips
`token` → `token.trim()`, and the only related test exercises a **different
class** `PaymentProcessor.validate` (src/billing.ts) under a strong exact-value
oracle:

```ts
// changed owner: TokenValidator.validate
return this.valid.has(token.trim());

// the ONLY related test — a DIFFERENT class, never imports TokenValidator
import { PaymentProcessor } from '../src/billing';
const proc = new PaymentProcessor();
expect(proc.validate("card1234")).toBe(true);
```

The tempting wrong relation is the bare method-name token `validate`: a
token matcher would link the test to the owner even though `proc` is a
`PaymentProcessor`, never a `TokenValidator`.

## When

`ripr check` analyzes the diff against the TypeScript preview adapter.

## Then

ripr classifies the change `no_static_path`. A receiver-owner relation requires
receiver identity: the test must construct the owner's class
(`new TokenValidator(...)`, imported from the owner file) and call the method
on that receiver. This test constructs `PaymentProcessor` and never imports
`TokenValidator`, so no relation is credited and the strong oracle is treated
as observing a different sink.

**This fixture must NEVER read `exposed`.** The companion positive control
`fixtures/typescript_same_method_owner_identity_positive` pins the inverse — a
test that *does* construct and exercise the owner class keeps `exposed`.

## Must Not

- Credit any relation from a bare method-name token match when no test
  provides owner-class receiver identity.
- Borrow the `PaymentProcessor.validate` exact-value oracle as evidence about
  `TokenValidator.validate`.
- Run any TypeScript runtime; static preview evidence only.
