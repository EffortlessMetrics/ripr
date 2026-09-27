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
  kept by the analysis side) whose boundary
  operand is a constant the repair-packet projection does not resolve to a value

## When

```bash
ripr check \
  --root fixtures/ts_repair_packet_boundary_constant_first/input \
  --diff fixtures/ts_repair_packet_boundary_constant_first/diff.patch
```

## Then

The TypeScript preview adapter:

- Classifies the finding as `WeaklyExposed`
- Projects a `GapRecord` whose `assertion_shape` is the boundary placeholder
  `expect(discountedTotal(/* boundary input for DISCOUNT_THRESHOLD == amount */)).toBe(expected)`;
  it must NOT reuse an observed input such as `discountedTotal(20000)`, which is
  not shown to equal the unresolved constant (G-G, #4215)
- Fails the packet closed through the shared validator
  (`agent_packet` ineligible): `repair_packet_ready` stays `false`
- Keeps `actionability_category: incomplete_repair_packet` and
  `gap_state: advisory` (no complete-packet flip)
- Keeps `authority_boundary: preview_advisory_only`
- Omits `typescript_repair_packet` from the check JSON

## Must Not

- Emit `repair_packet_ready: true` when the discriminator's boundary operand is
  an unresolved named constant, on either side, and no observed argument names
  it (#4215 review)
- Present a non-boundary observed input as the assertion shape for the missing
  discriminator
- Emit `typescript_repair_packet` without the shared validator returning `Ok(())`
- Change `schema_version`
