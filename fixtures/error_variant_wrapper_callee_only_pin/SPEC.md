# Fixture: error_variant_wrapper_callee_only_pin

Spec: RIPR-SPEC-0106

Issue: #3700

## Given

A boxed-error wrapper seam (`parse_summary` converts `try_parse_summary`
through `.map_err(..)`). The only related test calls the inner callee and
pins the exact variant against the callee's own result; no test invokes the
wrapper owner.

## When

```bash
cargo xtask fixtures error_variant_wrapper_callee_only_pin
```

or:

```bash
ripr check --root fixtures/error_variant_wrapper_callee_only_pin/input --diff fixtures/error_variant_wrapper_callee_only_pin/diff.patch --mode fast
```

## Then

The `error_path` probe for `try_parse_summary(raw).map_err(Into::into)`
reports `weakly_exposed` (NOT `exposed`). The callee-only pin supplies
variant identity but observes nothing about the wrapper, so the discriminate
stage emits `observation_unverified`.

## Must Not

- Classify the wrapper seam as `exposed` when no wrapper-invoking test
  confirms the established variant binding (the BUG-1 false promotion).
- Treat a callee-only exact-variant pin as wrapper observation.
