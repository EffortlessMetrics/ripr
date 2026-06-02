# Fixture: typescript-bun-ub-calibration

This manifest-only fixture pins the first Bun UB calibration target for
TypeScript preview evidence.

The corpus models the #31648 Blob `ArrayBuffer` seam where the Rust boundary is:

```text
array_buffer.shared || array_buffer.resizable
```

## Given

A Bun Rust/FFI seam changes behavior around `Blob` construction from JavaScript
`ArrayBuffer` inputs.

The useful proof lives in Bun TypeScript integration tests under:

```text
test/js/web/fetch/blob.test.ts
```

## When

Future TypeScript cross-language preview logic evaluates that seam, it should
credit TypeScript tests only when they provide all of the relevant observer
facts:

- `SharedArrayBuffer` construction;
- resizable `ArrayBuffer` construction through `maxByteLength`;
- `Blob` input through a buffer or view;
- a byte, text, or value assertion that observes stable copied bytes.

## Then

The corpus distinguishes:

- shared and resizable both discriminated;
- shared present but resizable missing;
- resizable present but shared missing;
- both sides missing;
- `maxByteLength` mentioned without a relevant Blob observer;
- TypeScript discriminators present while the bridge remains unknown.

## Must Not

- Promote TypeScript beyond preview.
- Emit complete repair packets.
- Suggest Rust test placement for the Bun Blob observer route.
- Claim runtime Bun execution, mutation execution, gate authority, badge
  authority, baseline authority, RIPR Zero authority, source edits, generated
  tests, provider calls, or support-tier promotion.

## Validate

```bash
cargo xtask check-fixture-contracts
cargo test -p xtask typescript_bun_ub_calibration_cases_are_checked -- --test-threads=1
```
