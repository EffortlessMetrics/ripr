# Fixture: match_arm_wrapper_parameter_callable_not_credited

Spec: RIPR-SPEC-0094

Owner: analysis-fixtures

Issue: #6297

## Given

The diff changes the `Unit::Fortnight` arm of `seconds`. The wrapper's
`seconds` parameter shadows the free function and dereferences to a fixed
callable. Its unchanged exact bridge assertion passes for either arm value.

## When

```bash
cargo xtask fixtures match_arm_wrapper_parameter_callable_not_credited
```

## Then

The finding must not be promoted to `exposed`: a parameter callable cannot
supply the free-function owner's identity.

## Must Not

- Resolve a parameter binding as the same-named owner function.
- Claim runtime adequacy.
