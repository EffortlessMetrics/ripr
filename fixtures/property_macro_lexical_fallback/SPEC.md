# Fixture: property_macro_lexical_fallback

Spec: RIPR-SPEC-0001

## Given

A changed valid threshold owner and intentionally malformed neighboring test
source. The malformed source forces the existing lexical fallback. Its only
apparent test is discarded by an opaque property macro.

## When

The public analyzer reads the boundary diff. The integration test
`property_macro_fallback_has_no_synthetic_test_evidence` pairs this path with
an ordinary fallback test and malformed-delimiter controls.

## Then

No synthetic related test, assertion oracle, or exposed classification is
created from macro tokens. The named macro-reach limitation explains unresolved
execution. These intentionally malformed sources have no runtime-test claim.

## Must Not

- Treat a parser failure as permission to promote opaque macro tokens
- Report test absence or propose a new test from this unsupported path
