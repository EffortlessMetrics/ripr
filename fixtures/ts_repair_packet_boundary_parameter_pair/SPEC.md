# Fixture: ts_repair_packet_boundary_parameter_pair

Spec: RIPR-SPEC-0087

## Given

A single-package TypeScript workspace (the #4759 release-smoke shape) where
`discount(amount, threshold)` has a boundary condition change (`>` → `>=`)
between two of its own parameters, neither written in the owner, and
oracle-eligible related tests with:

- A direct import-aware call relation (`import { discount } from '../src/pricing'`)
- An exact-value oracle at `discount(50, 100)`, below the threshold
- A discoverable `package.json` with `jest` in `devDependencies` and `scripts.test`
- `package-lock.json` confirming npm runner
- A named missing discriminator (`amount == threshold`)

## When

```bash
ripr check \
  --root fixtures/ts_repair_packet_boundary_parameter_pair/input \
  --diff fixtures/ts_repair_packet_boundary_parameter_pair/diff.patch
```

## Then

The TypeScript preview adapter:

- Classifies the finding as `WeaklyExposed`
- Emits `typescript_boundary_parameters: parameter=amount;index=0;operand=threshold;operand_index=1`
  (both parameters are read-only in the owner and the changed line runs on
  every call)
- Projects a `GapRecord` whose `assertion_shape` is the derived boundary input
  `expect(discount(100, 100)).toBe(expected)`, never the observed
  `discount(50, 100)`, with a stop condition naming the observed call as
  context and the derivation (`threshold` = 100)
- Passes the shared validator: `repair_packet_ready: true`,
  `actionability_category: complete_repair_packet`, `gap_state: actionable`
- Keeps `authority_boundary: preview_advisory_only`

## Must Not

- Present the observed below-threshold input as the assertion shape
- Emit the parameter-pair evidence when either parameter is written in the
  owner (see `ts_repair_packet_boundary_parameter_written`)
- Emit `typescript_repair_packet` without the shared validator returning `Ok(())`
- Change `schema_version`
