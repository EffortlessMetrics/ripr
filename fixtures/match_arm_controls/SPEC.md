# Fixture: match_arm_controls

Spec: RIPR-SPEC-0164

## Given

Four caller-side equality boundaries over match helpers that each
break one rule of the strict grammar: a computed arm value
(`"word" => pick(kind)`), a guard arm (`"word" if kind.len() > 2`),
a bare-identifier pattern (`word => "alpha"`), and a char scrutinee
(`match c { 'a' => ... }`). Each caller binds the helper return into
`final_label` and compares it at an equality boundary; the diff
changes each comparison constant (`"alpha"` -> `"beta"`). The tests
call each classify with an exact literal.

## When

```bash
cargo xtask fixtures match_arm_controls
```

## Then

Every probe stays `weakly_exposed`: the match evaluator refuses each
variant by rule (a computed value is not an exact return; a guard or
unresolved pattern could match anything; a bare identifier may be a
binding — the token-coincidence family; a non-string scrutinee is a
named limitation).

## Must Not

- Produce any `exposed` finding or any hop-provenance string on any
  of the four probes.
- Skip the helper authority entirely: each variant must be refused by
  the match grammar, not by argument binding (the arguments stay exact
  literals or the owner's bound parameter).
