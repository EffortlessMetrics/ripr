# Fixture: match_arm_wrapper_callable_shadow_not_credited

Spec: RIPR-SPEC-0094

Owner: analysis-fixtures

Issue: #6297

## Given

The diff changes the `Unit::Fortnight` arm of `seconds`.
A test-module constant callable shadows the real transparent wrapper. The exact bridge assertion passes for either changed-arm value because it calls the constant, so it cannot bind owner identity.

## When

```bash
cargo xtask fixtures match_arm_wrapper_callable_shadow_not_credited
```

## Then

The finding remains `weakly_exposed`. The same admitted oracle must bind
owner identity, changed-arm observation, and strong discrimination.
The honesty corpus rejects promotion independently of these golden outputs.

## Must Not

- Credit token spelling as callable or enum-variant identity.
- Borrow the unrelated reaching test's identity for the bridge oracle.
- Claim runtime adequacy.
