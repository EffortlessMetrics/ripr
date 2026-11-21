# Fixture: error_variant_wrapper_foreign_pin

Spec: RIPR-SPEC-0106

Issue: #3700

## Given

A boxed-error wrapper seam (`parse_summary` converts `try_parse_summary`
through `.map_err(..)`). The only related test calls the inner callee but
pins an exact variant against a *different* call (`unrelated_check`, whose
error enum shares the terminal variant name `MalformedSource` under a
different qualifier `OtherError`). The test is named after the callee it
exercises (`try_parse_summary_with_foreign_pin`): test-to-seam linkage is
name-anchored, so a generic name would report `no_static_path` instead.

## When

```bash
cargo xtask fixtures error_variant_wrapper_foreign_pin
```

or:

```bash
ripr check --root fixtures/error_variant_wrapper_foreign_pin/input --diff fixtures/error_variant_wrapper_foreign_pin/diff.patch --mode fast
```

## Then

The `error_path` probe for `try_parse_summary(raw).map_err(Into::into)`
reports `weakly_exposed` (NOT `exposed`). The foreign pin constrains another
call's result, so no wrapper-to-variant binding is established and the
discriminate stage emits `observation_unverified`.

## Must Not

- Classify the wrapper seam as `exposed` when the exact-variant pin targets
  another call (the BUG-2 false promotion).
- Align equal terminal variant names across different error enums.
