# Fixture: ts_cross_language_bridge_limit

Spec: RIPR-SPEC-0087

## Given

A TypeScript owner `processData` changes a predicate (`>` → `>=`). The test
file directly imports `processData` and uses `toBe('hello')` — a strong
exact-value oracle. However, the test file ALSO uses `vi.mock('../src/nativeModule')`
creating an opaque cross-module boundary: the adapter cannot resolve what the
mock substitutes, preventing a bounded repair packet.

This fixture models F11 (unresolved cross-language oracle visibility): the
`mocked_module` static limit fires because a `vi.mock`/`jest.mock` call is
present in the test file. The adapter correctly routes to `static_limitation`
/ `mocked_module` (a named limitation) rather than emitting an actionable
repair packet. G-A excludes it because the category is `mocked_module`, not
`incomplete_repair_packet`.

Note: The Bun-specific G-F guard in `typescript_packet_projection.rs` checks
for `route_cross_language_oracle_visibility_limitation` evidence (emitted only
in the bun-ub corpus context). The mocked_module static limit represents the
general cross-language bridge concern at the static analysis level.

## When

```bash
ripr check \
  --root fixtures/ts_cross_language_bridge_limit/input \
  --diff fixtures/ts_cross_language_bridge_limit/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- Detects `vi.mock('../src/nativeModule')` → `mocked_module` named static limit
- Sets `gap_state: static_limitation`
- Sets `actionability_category: mocked_module` (named limit, G-A excludes)
- Sets `repair_packet_ready: false`
- Does NOT flip to actionable

## Must Not

- Emit `repair_packet_ready: true`
- Bypass the static-limit early-return due to a strong oracle being present
- Omit the named mock boundary reason
