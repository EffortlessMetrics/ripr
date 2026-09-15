# Fixture: error_variant_wrapper_wrong_receiver_pin

Spec: RIPR-SPEC-0106

Issue: #3700

## Given

A boxed-error wrapper seam owned by a method (`ParserA::parse_summary`
converts `try_parse_summary` through `.map_err(..)`). One test calls the
inner callee and pins the exact variant (`ParseSummaryError::MalformedSource`),
establishing the wrapper-to-variant binding. A second test invokes a
same-named method on another receiver (`ParserB::parse_summary`) and pins the
same qualified variant — but it never invokes the wrapper owner. Relation is
name-anchored (`DirectOwnerCall` fires on the bare terminal name), so only the
receiver-aware invocation verdict keeps the seam weak.

## When

```bash
cargo xtask fixtures error_variant_wrapper_wrong_receiver_pin
```

or:

```bash
ripr check --root fixtures/error_variant_wrapper_wrong_receiver_pin/input --diff fixtures/error_variant_wrapper_wrong_receiver_pin/diff.patch --mode fast
```

## Then

The `error_path` probe for `try_parse_summary(raw).map_err(Into::into)`
reports `weakly_exposed` (NOT `exposed`). The other-receiver pin supplies a
qualified-variant oracle but no wrapper-owner invocation, so the discriminate
stage emits `observation_unverified`.

## Must Not

- Confirm wrapper observation through a receiver-qualified same-name call
  (the wrong-receiver false promotion).
- Credit a same-named method on another receiver as invoking the changed
  owner.
