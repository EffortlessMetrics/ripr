# Fixture: ts_repair_packet_boundary_literal_guarded

Spec: RIPR-SPEC-0087

## Given

The `ts_repair_packet_boundary_literal_derived` workspace, except that
`shipping` returns early on `amount === 5000` before the changed comparison
(`amount >= 5000` → `amount > 5000`). The related test asserts
`expect(shipping(6000)).toBe(1)`, which does not reach the boundary.

## When

```bash
ripr check \
  --root fixtures/ts_repair_packet_boundary_literal_guarded/input \
  --diff fixtures/ts_repair_packet_boundary_literal_guarded/diff.patch
```

## Then

The TypeScript preview adapter:

- Classifies the finding as `WeaklyExposed` with missing discriminator
  `amount == 5000`
- Emits no `typescript_boundary_input` line: the derived input
  `shipping(5000)` returns at the earlier guard and never executes the changed
  comparison, so operand equality does not establish a boundary input
- Keeps the repair packet non-delegatable (`repair_packet_ready: false`)

## Must Not

- Emit `expect(shipping(5000)).toBe(expected)` as a delegatable assertion shape
- Emit `typescript_repair_packet` as ready
