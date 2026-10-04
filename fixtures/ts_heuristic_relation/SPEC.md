# Fixture: ts_heuristic_relation

Spec: RIPR-SPEC-0087

## Given

A TypeScript owner `formatCurrency` changes a predicate (`<` → `<=`). The
test file `formatter.test.ts` imports `formatCurrency` and references it as a
value (`const format = formatCurrency; format(10, 'USD')`) but never calls it
by name, so no owner-call or import-call relation is established. Because the
test does reference the owner, the same-stem file earns a heuristic same-file
proximity relation (`has_oracle_eligible_relation == false`). A test that only
names the owner in a title or calls an unrelated object's `formatCurrency`
method would not be related at all (RIPR-SPEC-0027).

This fixture models F3 (heuristic-only relation): G-D fails because the
related-test link is same-stem proximity over a value reference, not an
owner call or import call.
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
