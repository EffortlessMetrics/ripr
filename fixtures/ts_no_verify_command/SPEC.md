# Fixture: ts_no_verify_command

Spec: RIPR-SPEC-0087

## Given

A TypeScript owner `multiply` changes a predicate (`>` → `>=`). The test
directly imports `multiply` and calls `expect(result).toBeGreaterThan(10)` —
a weak relational oracle with a concrete literal. No `package.json` is present,
so no framework/runner is discoverable and no `typescript_verify_command`
evidence is emitted.

This fixture models F5 (missing verify command): G-A through G-E all pass
(`incomplete_repair_packet` category, direct import, concrete oracle, named
discriminator), but the projection returns `None` because no
`typescript_verify_command` evidence exists (no package.json). The validator
is never reached; the repair packet stays non-actionable.

## When

```bash
ripr check \
  --root fixtures/ts_no_verify_command/input \
  --diff fixtures/ts_no_verify_command/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- Finds 1 related test with a weak oracle and a named discriminator
- Classifies as `WeaklyExposed`, category `incomplete_repair_packet`
- Sets `typescript_package_confidence: none` (no package.json)
- Sets `repair_packet_ready: false` (projection returns None — no verify command)
- Does NOT flip to actionable

## Must Not

- Emit `repair_packet_ready: true`
- Skip the package discovery step
- Omit the named reason for staying non-actionable
