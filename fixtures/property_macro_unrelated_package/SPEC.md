# Fixture: property_macro_unrelated_package

Spec: RIPR-SPEC-0001

## Given

Two Cargo packages. Package a owns a changed threshold; package b contains a
same-name mention inside an opaque property macro and has no dependency on a.

## When

The threshold changes and a comment-only hunk admits package b to Fast-mode
indexing. `property_mentions_respect_known_package_boundaries` also exercises
the same separation in whole-workspace mode, with same/unknown-package and
ordinary-assertion controls.

## Then

The real gap remains `no_static_path`, without a property-macro limitation or
related tests. An unrelated package's identifier cannot suppress useful gap
guidance. No execution or test-sufficiency claim is made.

## Must Not

- Replace the gap with a limitation based on known unrelated package text
- Infer a call edge or test from the property macro's spelling
