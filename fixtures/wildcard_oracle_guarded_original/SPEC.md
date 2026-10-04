# Fixture: wildcard_oracle_guarded_original

Spec: RIPR-SPEC-0108

Owner: confidence-calibration

Issue: #5397

## Given

The independent contract is `score(1) == 2`. This original implementation
returns 2. A local `let value = score(1)` binds the
guarded assertion to the changed return value. The matched runtime control in
`oracles/wildcard_tests.rs` establishes that the whole wildcard accepts both
implementations while exact2 and guarded equality distinguish the wrong value.

## When

The public diff analyzer runs in fast mode against `input` and `diff.patch`,
with a case-owned cold cache. `cargo xtask fixtures wildcard_oracle_guarded_original` replays the
registered JSON, human and full human projections during normal validation.

## Then

Exactly one return-value finding retains one related `observes_score` test.
It reads `exposed` with
`exact_value/strong` evidence.
This is static discriminator evidence, not a claim that the implementation
is correct or that its test was executed by the analyzer.

## Must Not

- Erase the real exact/guarded discriminator to obtain a non-promoted control.
- Obtain a passing result by removing the finding, subject or related test.
- Count synthetic charter membership as representative-project accuracy.
