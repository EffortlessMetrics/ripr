# Fixture: ts_repair_packet_boundary_constant_resolved

Spec: RIPR-SPEC-0087

## Given

A single-package TypeScript workspace (the onboarding `tsapp` shape, #4215
follow-up) where `discountedTotal` has a boundary condition change (`>` → `>=`)
against `DISCOUNT_THRESHOLD`, which the owner's module binds exactly once, at
the top level, with `export const DISCOUNT_THRESHOLD = 10000;`, and
oracle-eligible related tests with:

- A direct import-aware call relation (`import { discountedTotal } from '../src/pricing'`)
- Exact-value oracles at `5000` and `20000`, neither of which is the threshold
- A discoverable `package.json` with `jest` in `devDependencies` and `scripts.test`
- `package-lock.json` confirming npm runner
- A named missing discriminator (`amount == DISCOUNT_THRESHOLD`)

## When

```bash
ripr check \
  --root fixtures/ts_repair_packet_boundary_constant_resolved/input \
  --diff fixtures/ts_repair_packet_boundary_constant_resolved/diff.patch
```

## Then

The TypeScript preview adapter:

- Classifies the finding as `WeaklyExposed`
- Emits `typescript_boundary_input: parameter=amount;index=0;operand=DISCOUNT_THRESHOLD;value=10000`
  (the owner reads parameter `amount` unchanged; the constant is a single
  immutable integer module `const` read only in plain expression positions)
- Projects a `GapRecord` whose `assertion_shape` is the derived boundary input
  `expect(discountedTotal(10000)).toBe(expected)`, never the observed
  `discountedTotal(20000)`, with a stop condition naming the observed call as
  context and the derivation (`DISCOUNT_THRESHOLD` = 10000)
- Passes the shared validator: `repair_packet_ready: true`,
  `actionability_category: complete_repair_packet`, `gap_state: actionable`
- Keeps `authority_boundary: preview_advisory_only`
- Emits `typescript_repair_packet`, and the default human Start-here line names
  the packet's action, test file, and verify command

## Must Not

- Present the observed off-boundary input as the assertion shape
- Resolve a constant that the module could rebind or shadow (see
  `ts_repair_packet_boundary_constant_unresolved`)
- Emit `typescript_repair_packet` without the shared validator returning `Ok(())`
- Change `schema_version`
