# Fixture: wildcard_oracle_wildcard_original

Spec: RIPR-SPEC-0108

Owner: confidence-calibration

Issue: #5397

## Given

The independent contract is `score(1) == 2`. This original implementation
returns 2. A local `let value = score(1)` binds the
wildcard assertion to the changed return value. The matched runtime control in
`oracles/wildcard_tests.rs` establishes that the whole wildcard accepts both
implementations while exact2 and guarded equality distinguish the wrong value.

## When

The public diff analyzer runs in fast mode against `input` and `diff.patch`,
with a case-owned cold cache. `cargo xtask fixtures wildcard_oracle_wildcard_original` replays the
registered JSON, human and full human projections during normal validation.

## Then

Exactly one return-value finding retains one related `observes_score` test.
It reads `weakly_exposed` with
`relational_check/weak` evidence.
This is static discriminator evidence, not a claim that the implementation
is correct or that its test was executed by the analyzer.

## Must Not

- Promote a whole unguarded wildcard to exposed or call it a strong exact oracle.
- Obtain a passing result by removing the finding, subject or related test.
- Count synthetic charter membership as representative-project accuracy.
