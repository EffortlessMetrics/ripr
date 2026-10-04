# Fixture: ts_repair_packet_boundary_literal_derived

Spec: RIPR-SPEC-0087

## Given

A single-package TypeScript workspace (the RC rehearsal `ts-pricing` shape)
where `shipping` has a boundary condition change on a literal
(`amount > 5000` → `amount >= 5000`) and an oracle-eligible related test with:

- A direct import-aware call relation (`import { shipping } from '../src/shipping'`)
- An exact-value oracle `expect(shipping(1000)).toBe(500)`, whose input does
  not reach the boundary
- A discoverable `package.json` with `jest` in `devDependencies` and `scripts.test`
- `package-lock.json` confirming npm runner
- A named missing discriminator (`amount == 5000`)

## When

```bash
ripr check \
  --root fixtures/ts_repair_packet_boundary_literal_derived/input \
  --diff fixtures/ts_repair_packet_boundary_literal_derived/diff.patch
```

## Then

The TypeScript preview adapter:

- Classifies the finding as `WeaklyExposed`
- Emits `typescript_boundary_input: parameter=amount;index=0;operand=5000;value=5000`
- Projects a `GapRecord` whose `assertion_shape` is the derived boundary input
  `expect(shipping(5000)).toBe(expected)`; the observed `shipping(1000)` stays
  as context in a stop condition
- Passes the shared validator: `repair_packet_ready: true`,
  `actionability_category: complete_repair_packet`, `gap_state: actionable`
- Keeps `authority_boundary: preview_advisory_only`
- Emits `typescript_repair_packet`

## Must Not

- Fail the packet closed when a boundary input is statically derivable (#4105
  still fails closed when it is not, see `ts_repair_packet_boundary_unreachable`)
- Present the observed off-boundary input as the assertion shape
- Emit `typescript_repair_packet` without the shared validator returning `Ok(())`
- Change `schema_version`
