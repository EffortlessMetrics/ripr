# Fixture: typescript_same_method_owner_identity_positive (positive control — true receiver identity keeps exposed)

Spec: RIPR-SPEC-0108

Corpus case: `ts_same_method_owner_identity_positive_control` in
`fixtures/evidence-promotion-honesty-corpus/corpus.json` (issue #1983,
required positive control: exact same-entity relation MUST still fire for the
true owner).

## Given

The mirror image of `fixtures/typescript_adversarial_same_method_other_class`.
The changed owner is again `TokenValidator.validate` (src/auth.ts) flipping
`token` → `token.trim()`, but this time the test provides genuine receiver
identity: it imports `TokenValidator` from the owner file, constructs it
directly, and asserts the method's exact return value on the receiver:

```ts
import { TokenValidator } from '../src/auth';
const validator = new TokenValidator(["abc"]);
expect(validator.validate(" abc ")).toBe(true);
```

## When

`ripr check` analyzes the diff against the TypeScript preview adapter.

## Then

ripr credits the receiver-owner relation (`new TokenValidator(...)` +
`validator.validate(...)`, owner class imported from the owner file) and
classifies the change `exposed` with an `exact_value`/`strong` oracle.

**This control must NEVER lose `exposed`.** It proves the
same-method-different-class guard did not degenerate into "disable all method
relations": the true owner, reached with real receiver identity and observed
by a strong oracle, keeps its `exposed` classification under the current
preview contract (preview/advisory per support policy — no gate, badge, or
RIPR Zero role is implied).

## Must Not

- Downgrade a same-entity receiver relation with a strong exact-value oracle
  below `exposed`.
- Emit a repair packet or receipt command for this already-observed change.
- Run any TypeScript runtime; static preview evidence only.
