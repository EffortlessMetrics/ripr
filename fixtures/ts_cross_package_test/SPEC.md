# Fixture: ts_cross_package_test

Spec: RIPR-SPEC-0087

## Given

A TypeScript owner `checkStock` changes a boundary predicate (`>` → `>=`).
No test file exists in the workspace — there are no related tests for
`checkStock`. The fixture models a case where the test coverage lives in a
separate package or has not been written yet.

This fixture models F4 (cross-package / no related test → `missing_context`):
`ExposureClass::NoStaticPath` (0 related tests found). G-A does not apply
because the `actionability.rs::typescript_actionability_for` function routes
`NoStaticPath` to `missing_context` before reaching the
`incomplete_repair_packet` branch.

## When

```bash
ripr check \
  --root fixtures/ts_cross_package_test/input \
  --diff fixtures/ts_cross_package_test/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- Finds 0 related tests for owner `checkStock`
- Classifies as `no_static_path`
- Sets `actionability_category: missing_context`
- Sets `gap_state: advisory`
- Sets `repair_packet_ready: false`
- Does NOT flip to actionable

## Must Not

- Emit `repair_packet_ready: true`
- Claim a related test exists when none does
- Omit the named reason for staying non-actionable
