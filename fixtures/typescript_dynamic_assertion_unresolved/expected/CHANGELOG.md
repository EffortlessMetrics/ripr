# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0085 §PR5: new fixture for typescript_dynamic_assertion_unresolved limitation.
The test uses `expect(clamp(-5, 0, 10)).toBe(expectedMin)` where `expectedMin` is a local
variable (non-literal dynamic argument). The adapter emits the
`typescript_dynamic_assertion_unresolved` named limitation and the
`typescript_oracle_observed`/`typescript_oracle_confidence`/`typescript_oracle_evidence_ref`
metadata lines. No `typescript_oracle_expected` is emitted because the argument is dynamic.

Command:
`cargo xtask goldens bless typescript_dynamic_assertion_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
