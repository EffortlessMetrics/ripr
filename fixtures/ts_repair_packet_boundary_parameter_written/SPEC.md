# Fixture: ts_repair_packet_boundary_parameter_written

Spec: RIPR-SPEC-0087

## Given

The `ts_repair_packet_boundary_parameter_pair` workspace, except that the
owner writes its `threshold` parameter (`threshold = Math.max(threshold, 1);`)
before the changed comparison `amount >= threshold`. The observed test call
`discount(50, 100)` therefore does not show which value reaches the
comparison.

## When

```bash
ripr check \
  --root fixtures/ts_repair_packet_boundary_parameter_written/input \
  --diff fixtures/ts_repair_packet_boundary_parameter_written/diff.patch
```

## Then

The TypeScript preview adapter:

- Classifies the finding as `WeaklyExposed`
- Emits no `typescript_boundary_parameters` evidence
- Fails the packet closed through the shared validator:
  `repair_packet_ready: false` and no `typescript_repair_packet`
- The default human Start-here line says the packet is not ready because
  boundary operand `threshold` is not bound to a concrete value by the
  observed call input `discount(50, 100)`

## Must Not

- Present `discount(50, 100)` as the assertion shape for `amount == threshold`
- Report the repair packet as complete
- Change `schema_version`
