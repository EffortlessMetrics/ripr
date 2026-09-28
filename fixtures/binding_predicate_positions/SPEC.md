# Fixture: binding_predicate_positions

Spec: RIPR-SPEC-0157

## Given

Three changed local bindings, each feeding a different supported direct
position in its own function: a boolean `if open` test, a `match band`
scrutinee, and a `margin` comparison operand. No initializer contains an
operation ripr cannot name.

## When

```bash
cargo xtask fixtures binding_predicate_positions
```

## Then

Each changed binding retargets to its own use: three predicate-family
probes at the boolean-test line, the `match` scrutinee line, and the
comparison line, each with the changed initializer as after-evidence and
`binding_predicate_relation` evidence. The boolean and scrutinee
bindings resolve their literal initializers to text, so no
earliest-operation limitation line appears for them.

## Must Not

- Relate a binding through anything but its direct identifier operand.
- Invent an unresolved-operation limitation for a bare literal
  initializer.
