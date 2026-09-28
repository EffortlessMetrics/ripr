# Fixture: ts_predicate_boundary_optional_chaining

Spec: RIPR-SPEC-0027

## Given

Three TypeScript owners whose changed predicates were never
boundary-witnessable because the comparison parser rejected `?` shapes or
the probe fell to the ambiguous fallback (issue #4104 E2):

- `userIsAdmin`: `order?.role !== "admin"` becomes `order?.role === "admin"`
  — optional chaining reads as plain member access for operand extraction,
  and the related test pins the operand's field through an object-literal
  argument (`userIsAdmin({ role: "admin" })`).
- `pickLabel`: `if (name ?? "")` becomes `if (name ?? "temp")` — the
  nullish-coalescing boundary is the literal operand that changes the
  outcome; the nullish boundary input (`pickLabel(null)`) witnesses at the
  left operand's read position, and the fallback literal is witnessable like
  any comparison operand.
- `thresholdsAbove`: `yield total > 50` becomes `yield total >= 50` — a
  generator's `yield <comparison>` tail classifies as a predicate probe, and
  the `yield` keyword strips before comparison parsing
  (`thresholdsAbove(50)` witnesses).

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
ripr check \
  --root fixtures/ts_predicate_boundary_optional_chaining/input \
  --diff fixtures/ts_predicate_boundary_optional_chaining/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- classifies all three changed lines as specific predicate probes,
- witnesses the changed boundary of each through the strong exact-value
  assertion of its related test,
- keeps all three findings `exposed` with no missing boundary discriminator.

## Must Not

- Normalize `?.` or `??` inside string literal contents (a quoted
  `"a ?? b"` never becomes a witnessable operand).
- Credit an off-boundary literal (`{ role: "viewer" }` against
  `role === "admin"`), a nullish argument parked in an argument position the
  owner never reads, or a `yield*` delegation / call-shaped tail.
- Let a parenthesized compound form such as `(value ?? 50) >= 100` parse
  into a boundary it cannot honestly map (it stays fail-closed).
