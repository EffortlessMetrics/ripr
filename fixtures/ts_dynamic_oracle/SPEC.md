# Fixture: ts_dynamic_oracle

Spec: RIPR-SPEC-0087

## Given

A TypeScript production module changes its boundary predicate (`>` → `>=`) in
`computePrice`. The test calls `expect(result).toBe(expected)` where `expected`
is a local variable produced by `getExpectedValue()` — a non-literal dynamic
expression.

This fixture models F1/F2 (dynamic/non-literal oracle): the `.toBe(...)` matcher
receives a dynamic argument, so `has_dynamic_matcher_arg == true` and
`expected_value_or_variant == None`. However, `toBe` has `OracleStrength::Strong`,
which means the finding is classified as `Exposed` and routes through
`strong_oracle_observed` — the G-A precondition fails before G-C is even checked.

The `typescript_dynamic_assertion_unresolved` named limitation is emitted because
the dynamic arg prevents static resolution of the expected discriminator value.

## When

```bash
ripr check \
  --root fixtures/ts_dynamic_oracle/input \
  --diff fixtures/ts_dynamic_oracle/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- Classifies as `Exposed` (oracle_strength Strong → Exposed class)
- Sets `actionability_category: strong_oracle_observed` (G-A: not eligible)
- Sets `gap_state: already_observed`
- Emits `typescript_dynamic_assertion_unresolved` named limitation
- Sets `repair_packet_ready: false`
- Does NOT flip to actionable

## Must Not

- Emit `repair_packet_ready: true`
- Claim the finding is in the `incomplete_repair_packet` category
- Omit the named limitation reason
