# Fixture: ts_repair_packet_boundary_constant_first

Spec: RIPR-SPEC-0087

## Given

A single-package TypeScript workspace (the onboarding `tsapp` shape, issue
#4215) where `discountedTotal` has a boundary condition change
(`DISCOUNT_THRESHOLD < amount` → `DISCOUNT_THRESHOLD <= amount`) with the named
module constant written first, and oracle-eligible
related tests with:

- A direct import-aware call relation (`import { discountedTotal } from '../src/pricing'`)
- Exact-value oracles at `5000` and `20000`, neither of which is the threshold
- A discoverable `package.json` with `jest` in `devDependencies` and `scripts.test`
- `package-lock.json` confirming npm runner
- A named missing discriminator (`DISCOUNT_THRESHOLD == amount`, operand order
  kept by the analysis side) whose boundary operand is a single immutable
  integer module `const`

## When

```bash
ripr check \
  --root fixtures/ts_repair_packet_boundary_constant_first/input \
  --diff fixtures/ts_repair_packet_boundary_constant_first/diff.patch
```

## Then

The TypeScript preview adapter:

- Classifies the finding as `WeaklyExposed`
- Emits `typescript_boundary_input: parameter=amount;index=0;operand=DISCOUNT_THRESHOLD;value=10000`:
  the module binds `DISCOUNT_THRESHOLD` once with `export const ... = 10000`
- Reads the leading constant with the operator mirrored and projects a
  `GapRecord` whose `assertion_shape` is the derived boundary input
  `expect(discountedTotal(10000)).toBe(expected)`; it must NOT reuse the
  observed `discountedTotal(20000)` (G-G, #4215)
- Passes the shared validator: `repair_packet_ready: true`,
  `actionability_category: complete_repair_packet`, `gap_state: actionable`
- Keeps `authority_boundary: preview_advisory_only`
- Emits `typescript_repair_packet`

## Must Not

- Treat the constant written first as the receiver (#4215 review)
- Present a non-boundary observed input as the assertion shape for the missing
  discriminator
- Emit `typescript_repair_packet` without the shared validator returning `Ok(())`
- Change `schema_version`
