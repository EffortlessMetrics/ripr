# Fixture: binding_predicate_scope_controls

Spec: RIPR-SPEC-0157

## Given

Five changed `let` lines that must each fail closed out of the
binding-predicate relation: a sibling function's same-named binding
(`sibling_b` compares its own `end`, never scanned), an inner-scope
re-binding between declaration and use, a reassignment between
declaration and use, a binding mentioned only in a comment and a string,
and a destructuring declaration.

## When

```bash
cargo xtask fixtures binding_predicate_scope_controls
```

## Then

No retargeted predicate probe exists: every changed line keeps the
generic static-unknown finding at its own line, and no
`binding_predicate_relation` evidence appears anywhere in the output.

## Must Not

- Relate a binding across functions, through a shadow, through a
  reassignment, from comment/string text, or from a destructuring
  declaration.
- Promote any of these controls past static_unknown.
