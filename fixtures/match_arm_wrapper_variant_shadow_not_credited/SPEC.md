# Fixture: match_arm_wrapper_variant_shadow_not_credited

Spec: RIPR-SPEC-0094

Owner: analysis-fixtures

Issue: #6297

## Given

The diff changes the `Unit::Fortnight` arm of `seconds`.
The test-module Unit::Fortnight associated constant denotes the owner enum's Week variant. The exact bridge assertion passes for either Fortnight-arm value, so path spelling cannot establish selected-arm identity.

## When

```bash
cargo xtask fixtures match_arm_wrapper_variant_shadow_not_credited
```

## Then

The finding remains `weakly_exposed`. The same admitted oracle must bind
owner identity, changed-arm observation, and strong discrimination.
The honesty corpus rejects promotion independently of these golden outputs.

## Must Not

- Credit token spelling as callable or enum-variant identity.
- Borrow the unrelated reaching test's identity for the bridge oracle.
- Claim runtime adequacy.
