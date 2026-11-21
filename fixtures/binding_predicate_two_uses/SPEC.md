# Fixture: binding_predicate_two_uses

Spec: RIPR-SPEC-0157

## Given

One changed binding (`ceiling`) whose value feeds two direct predicate
uses in the same function: a right comparison operand
(`count > ceiling`) and a left `!=` operand (`ceiling != 50`).

## When

```bash
cargo xtask fixtures binding_predicate_two_uses
```

## Then

Both uses receive their own predicate-family probe at their own line
with their own content-addressed identity, in deterministic scan order;
each carries the same changed initializer as causal after-evidence.

## Must Not

- Merge the two uses into one finding, or let the second use borrow the
  first use's identity.
- Drop either use in favor of the other.
