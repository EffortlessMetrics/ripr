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

The Bun FFI TypeScript preview stack also has a calibrated #950-shaped public
entrypoint sample:

```text
src/bun.js/bindings/FFIObject.rs
FFIObject::read
usize::try_from(to_int32()).expect("int cast")
read.u8(ptr, -1)
```

## When

An operator asks whether the TypeScript integration tests discriminate that
stable-byte seam, the dogfood corpus records the known-good witness, a stripped
resizable branch, and a maxByteLength token-only false positive. When the
operator asks about the FFI negative-offset panic boundary, the corpus records
the unresolved oracle as a closed limitation receipt.

## Then

The receipts must show:

- `rust_ungripped_ts_discriminated` for the known-good case.
- `rust_ungripped_ts_missing_discriminator` with
  `resizable_array_buffer` and `test/js/web/fetch/blob.test.ts` placement for
  the stripped-resizable case.
- `ts_mention_not_observer` for the token-only case.
- `public_reachable_panic_boundary_unrevealed` for the FFI negative-offset
  case, with `negative_offset`, `external_oracle:negative_offset_panic_boundary`,
  and `safe_external_observer_target` unresolved and no suggested test file.

## Must Not

- Do not run Bun, Jest, Vitest, tsc, tsserver, mutation, or provider calls.
- Do not edit Bun source or generate tests.
- Do not mark any receipt as repair-packet-ready.
- Do not contribute to a default gate, badge, baseline, RIPR Zero, or support
  tier.
