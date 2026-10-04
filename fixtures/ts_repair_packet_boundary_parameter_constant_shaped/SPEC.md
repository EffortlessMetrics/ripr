# Fixture: ts_repair_packet_boundary_parameter_constant_shaped

Spec: RIPR-SPEC-0087

## Given

The #4759 parameter-pair shape where the boundary operand is a CONSTANT_CASE
owner parameter: `discount(amount, LIMIT)` has a boundary condition change
(`>` → `>=`) between two of its own parameters, neither written in the owner,
and oracle-eligible related tests with:

- A direct import-aware call relation (`import { discount } from '../src/pricing'`)
- An exact-value oracle at `discount(50, 100)`, below the limit
- A discoverable `package.json` with `jest` in `devDependencies` and `scripts.test`
- `package-lock.json` confirming npm runner
- A named missing discriminator (`amount == LIMIT`)

The analysis side records the parameter-pair fact for the uppercase name, so
the projection must read `LIMIT` as a parameter, not as a module constant.

## When

```bash
ripr check \
  --root fixtures/ts_repair_packet_boundary_parameter_constant_shaped/input \
  --diff fixtures/ts_repair_packet_boundary_parameter_constant_shaped/diff.patch
```

## Then

The TypeScript preview adapter:

- Classifies the finding as `WeaklyExposed`
- Emits `typescript_boundary_parameters: parameter=amount;index=0;operand=LIMIT;operand_index=1`
- Projects a `GapRecord` whose `assertion_shape` is the derived boundary input
  `expect(discount(100, 100)).toBe(expected)`, never the unresolved-constant
  placeholder for `LIMIT`, with a stop condition naming the observed call as
  context and the derivation (`LIMIT` = 100)
- Passes the shared validator: `repair_packet_ready: true`,
  `actionability_category: complete_repair_packet`, `gap_state: actionable`
- Keeps `authority_boundary: preview_advisory_only`

## Must Not

- Parse a CONSTANT_CASE owner parameter as an unresolved module constant and
  close the packet the parameter-pair fact is evidence for
- Present the observed below-limit input as the assertion shape
- Emit the parameter-pair evidence when either parameter is written in the
  owner (see `ts_repair_packet_boundary_parameter_written`)
- Emit `typescript_repair_packet` without the shared validator returning `Ok(())`
- Change `schema_version`
