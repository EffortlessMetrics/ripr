# Fixture: ts_heuristic_relation

Spec: RIPR-SPEC-0087

## Given

A TypeScript owner `formatCurrency` changes a predicate (`<` → `<=`). The
test file `formatter.test.ts` references the owner name in a describe block
but does NOT directly import `formatCurrency` — only a heuristic same-file
proximity relation is established (`has_oracle_eligible_relation == false`).

This fixture models F3 (heuristic-only relation): G-D fails because the
related-test link is name-proximity only, not import-aware or owner-call.
The finding stays preview with `ambiguous_related_test`.

## When

```bash
ripr check \
  --root fixtures/ts_heuristic_relation/input \
  --diff fixtures/ts_heuristic_relation/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- Identifies a heuristic same-file-proximity test relation
- Sets `actionability_category: ambiguous_related_test` (G-D: not eligible)
- Sets `gap_state: advisory`
- Sets `repair_packet_ready: false`
- Does NOT flip to actionable

## Must Not

- Emit `repair_packet_ready: true`
- Treat a heuristic-only relation as oracle-eligible
- Omit the named reason for staying non-actionable
