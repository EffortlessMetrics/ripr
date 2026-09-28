# Fixture: oracle_confirmation_mixed

Spec: RIPR-SPEC-0094

Owner: analysis-fixtures

Issue: #4404

## Given

The changed call mutates `rows`. One test calls its owner without observing
the resulting rows. Another test compares two unrelated local vectors and
checks a constant string containing the call's name. These assertions are
intentional analyzed fixture input, governed by the existing `fixtures/**`
source-input policy; they are not assertions in RIPR's test harness.

The old call mutates a fresh discarded vector; the new call mutates the
caller's vector. Both revisions are valid Rust. The fixture's two tests
observe neither version of that effect.

The exact equality supplies strong oracle shape. The string check supplies
weak token confirmation. Neither assertion observes the changed effect.
Removing only the production call leaves both fixture tests passing.

## When

```bash
cargo xtask fixtures oracle_confirmation_mixed
```

The public diff analysis examines the changed call on `src/lib.rs:2`.

## Then

The call-deletion finding must stay `weakly_exposed`. Discrimination must
explain `oracle_confirmation_mixed`: an unrelated strongest oracle cannot
borrow confirmation from a weaker assertion. Candidate related tests remain
visible, without promoting their combined evidence to exact discrimination.

The honesty corpus independently prohibits `exposed` even if a golden is
changed. Focused classifier tests cover assertion and test order, value and
effect families, and the equally strong confirmed positive control.

## Must Not

- Combine one assertion's strength with another assertion's confirmation.
- Treat running the owner without observing its effect as discrimination.
- Claim general sink binding, population accuracy, or runtime adequacy.
- Claim this pooling fix resolves literal-token identity or every retained
  `p1706-wiring-rows-gap` judgment; those remain separate evidence boundaries.
