# Bun UB Cross-Language Dogfood Receipts

Spec: RIPR-SPEC-0062

## Given

The Bun Blob / ArrayBuffer TypeScript preview stack has calibrated route-quality
evidence for the #31648-shaped Rust seam:

```text
src/jsc/Blob.rs
Blob::from_js_without_defer_gc
array_buffer.shared || array_buffer.resizable
```

## When

An operator asks whether the TypeScript integration tests discriminate that
stable-byte seam, the dogfood corpus records the known-good witness, a stripped
resizable branch, and a maxByteLength token-only false positive.

## Then

The receipts must show:

- `rust_ungripped_ts_discriminated` for the known-good case.
- `rust_ungripped_ts_missing_discriminator` with
  `resizable_array_buffer` and `test/js/web/fetch/blob.test.ts` placement for
  the stripped-resizable case.
- `ts_mention_not_observer` for the token-only case.

## Must Not

- Do not run Bun, Jest, Vitest, tsc, tsserver, mutation, or provider calls.
- Do not edit Bun source or generate tests.
- Do not mark any receipt as repair-packet-ready.
- Do not contribute to a default gate, badge, baseline, RIPR Zero, or support
  tier.
