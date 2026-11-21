# Fixture: ts_static_limit

Spec: RIPR-SPEC-0087

## Given

A TypeScript owner `dispatchAction` changes from `handlers.default()` to
`handlers[key]()` — a computed member invocation. This triggers the
`dynamic_dispatch` named static limit: syntax alone cannot resolve which
handler is called.

This fixture models F10 (static limit short-circuits before oracle/relation
checks): `actionability.rs` line 51 short-circuits to `static_limitation`
BEFORE the `Exposed`/`NoStaticPath`/`ambiguous_related_test` branches and
before `incomplete_repair_packet`. G-A excludes it because the category is
`dynamic_dispatch`, not `incomplete_repair_packet`.

## When

```bash
ripr check \
  --root fixtures/ts_static_limit/input \
  --diff fixtures/ts_static_limit/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- Detects computed member invocation → `dynamic_dispatch` static limit
- Sets `gap_state: static_limitation`
- Sets `actionability_category: dynamic_dispatch` (the named limit)
- Sets `repair_packet_ready: false`
- Does NOT flip to actionable (G-A fails — not `incomplete_repair_packet`)

## Must Not

- Emit `repair_packet_ready: true`
- Bypass the static-limit early-return in `actionability.rs`
- Omit the named limit kind
