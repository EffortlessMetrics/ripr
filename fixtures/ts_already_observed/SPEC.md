# Fixture: ts_already_observed

Spec: RIPR-SPEC-0087

## Given

A TypeScript owner `validateScore` changes a boundary predicate (`>` → `>=`).
The related test calls `expect(validateScore(75)).toBe(true)` — a concrete
exact-value oracle with `oracle_strength: Strong` and a real literal expected
value (`true`). No `package.json` is present.

This fixture models F12 (already-observed strong oracle): the `toBe(true)` matcher
produces `OracleStrength::Strong` → `ExposureClass::Exposed` → the
`actionability_category: strong_oracle_observed` branch in `actionability.rs`
fires before G-A is evaluated. The finding stays preview with a named reason.

## When

```bash
ripr check \
  --root fixtures/ts_already_observed/input \
  --diff fixtures/ts_already_observed/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- Classifies as `Exposed` (oracle_strength Strong → Exposed class)
- Sets `actionability_category: strong_oracle_observed` (G-A: not eligible)
- Sets `gap_state: already_observed`
- Sets `repair_packet_ready: false`
- Does NOT flip to actionable (G-A fails — category is not `incomplete_repair_packet`)

## Must Not

- Emit `repair_packet_ready: true`
- Omit the named reason for staying non-actionable
