# Fixture: match_arm_wrapper_unreachable_arm_not_credited

Spec: RIPR-SPEC-0094

Owner: analysis-fixtures

Issue: #6297

## Given

The diff changes the `Unit::Fortnight` arm of `seconds`.
A preceding wildcard always returns a fixed value. The named Fortnight arm is unreachable, so its exact bridge assertion passes for either changed-arm value.

## When

```bash
cargo xtask fixtures match_arm_wrapper_unreachable_arm_not_credited
```

## Then

The finding must not be promoted to `exposed`. Its own admitted assertion
must bind the actual owner and the changed arm; the independent honesty corpus
refuses promotion regardless of golden formatting.

## Must Not

- Credit token spelling or possible reach as selected-arm identity.
- Borrow another oracle's owner identity or changed-arm reference.
- Claim runtime adequacy.
