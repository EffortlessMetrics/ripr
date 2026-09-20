# Fixture: rust_binding_status_wrong_owner_observer

Spec: RIPR-SPEC-0108

## Given

A test reaches the changed `receipt_binding_matches` comparison and carries
a strong oracle, but the oracle observes a nearby value (the `Ready` variant
spelling, the artifact ledger's own `status`) — never the comparison
decision itself.

## When

```bash
cargo xtask fixtures rust_binding_status_wrong_owner_observer
```

## Then

RIPR must not promote the binding-comparison seam to strong exposure.
Reach plus a strong oracle on a nearby value is the coverage mistake, not
discrimination: no assertion observes the changed sink's accept/refuse
decision.

## Must Not

- Treat an oracle naming `Ready`/`status`/`blocked` as an oracle on the
  `status == status && violations == violations` decision.
- Emit a repair-ready or clean result for the comparison seam.
- Claim runtime mutation adequacy.
