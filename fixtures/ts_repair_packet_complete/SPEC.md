# Fixture: ts_repair_packet_complete

Spec: RIPR-SPEC-0087

## Given

A single-package TypeScript workspace where `applyDiscount` has a boundary
condition change (`>` → `>=`) and an oracle-eligible related test with:

- A direct import-aware call relation (`import { applyDiscount } from '../src/discount'`)
- A weak relational oracle (`expect(result).toBeGreaterThan(50)`) with a concrete
  literal expected value (`50`) and `has_dynamic_matcher_arg == false`
- A discoverable `package.json` with `jest` in `devDependencies` and `scripts.test`
- `package-lock.json` confirming npm runner
- A named missing discriminator (`amount == threshold`) from the boundary expression

## When

```bash
ripr check \
  --root fixtures/ts_repair_packet_complete/input \
  --diff fixtures/ts_repair_packet_complete/diff.patch
```

## Then

The TypeScript preview adapter:

- Classifies the finding as `WeaklyExposed` (oracle strength is Weak, not Strong)
- Sets `actionability_category: incomplete_repair_packet` (G-A passes)
- Emits `typescript_oracle_expected: 50` (G-C: concrete literal, non-dynamic)
- Resolves the verify command from package.json: `jest tests/discount.test.ts`
- Projects a `GapRecord` that passes `validate_agent_gap_record_packet`
- Flips `repair_packet_ready: true` (RIPR-SPEC-0087 §PR7 — the ONLY flip condition)
- Sets `actionability_category: complete_repair_packet` (§1.3)
- Sets `gap_state: actionable` (§1.3)
- Sets `missing_actionability_fields: []` (§1.3)
- Keeps `authority_boundary: preview_advisory_only` (TypeScript stays preview)

## Must Not

- Emit `repair_packet_ready: true` without the shared validator returning `Ok(())`
- Change `schema_version`
- Add new public symbols
- Execute runtime code, call providers, or generate tests
