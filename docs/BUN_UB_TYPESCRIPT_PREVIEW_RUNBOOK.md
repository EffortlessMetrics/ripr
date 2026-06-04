# Bun UB TypeScript Preview Runbook

Use this runbook when a Bun Rust or FFI stable-byte seam changed and the real
observer may be a TypeScript or JavaScript integration test.

The question is narrow:

```text
This Rust/FFI seam changed. Do Bun's TypeScript integration tests discriminate
the boundary that would catch the stable-byte bug?
```

The answer is preview/advisory only. It helps the operator avoid manual grep
and wrong-language test placement. It does not prove UB, run Bun, generate
tests, or create repair packets.

## When To Run

Run this loop when all of these are true:

- The changed seam is Rust/native/FFI code reachable from Bun's JavaScript or
  TypeScript API surface.
- The risk is stable bytes: JS-owned backing storage can be shared, resized,
  detached, mutated, raced, replaced, or reentered after native code observes
  or stores bytes.
- Existing Bun integration tests may be the right observer route.

For the calibrated 0.8.1 route, the known seam is:

```text
Rust file: src/jsc/Blob.rs
Rust owner: Blob::from_js_without_defer_gc
Boundary: array_buffer.shared || array_buffer.resizable
Suggested TS file: test/js/web/fetch/blob.test.ts
```

There is also a calibrated FFI panic-boundary limitation sample:

```text
Rust file: src/bun.js/bindings/FFIObject.rs
Rust owner: FFIObject::read
Boundary: usize::try_from(to_int32()).expect("int cast")
External entrypoint sample: read.u8(ptr, -1)
Suggested TS file: not_applicable
```

## Configure The Preview

In the Bun checkout, keep TypeScript preview explicit in `ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]

[profiles.bun_ub]
test_roots = [
  "test/js/**/*.test.ts",
  "test/js/**/*.test.js",
]
bridge_hints = "ripr.bun.bridge.toml"
```

There is no `ripr check --profile` flag. The Bun UB profile is a config stanza
that records the operator's test roots and bridge-hint path. The bounded Bun
Blob / ArrayBuffer bridge evidence remains preview/advisory.

Check the config first:

```bash
ripr doctor --root /path/to/bun
```

Expected signal:

```text
Bun UB profile: configured (preview advisory only)
```

If the profile is not configured, do not credit TypeScript evidence to a Rust
seam as a configured Bun UB review result.

## Run Diff-Scoped Evidence

For ordinary PR review, start with changed-surface output:

```bash
ripr check \
  --root /path/to/bun \
  --base origin/main \
  --mode draft \
  --format human
```

For a machine-readable artifact:

```bash
mkdir -p target/ripr/bun-ub
ripr check \
  --root /path/to/bun \
  --base origin/main \
  --mode draft \
  --format json \
  > target/ripr/bun-ub/check.json
```

Use repo-wide `repo-exposure-json` only for an intentional large-repo refresh.
The normal Bun UB operator loop should not require a full no-ledger repo scan.

## Read The States

`rust_ungripped_ts_discriminated`

The configured Rust seam is not gripped by Rust tests, but TypeScript evidence
has the required discriminator facts, Blob/view input, stable-byte oracle, and
credited bridge leg.

Action:

```text
No missing bridge discriminator. Continue manual unsafe/FFI review.
Do not generate a test or open a public repair packet from this preview result.
```

`rust_ungripped_ts_missing_discriminator`

The configured bridge exists, but one required branch is absent.

Action for the Blob / ArrayBuffer route:

```text
Missing shared_array_buffer:
  add a SharedArrayBuffer Blob stable-byte case in test/js/web/fetch/blob.test.ts

Missing resizable_array_buffer:
  add a resizable ArrayBuffer Blob stable-byte case in test/js/web/fetch/blob.test.ts
```

This is advisory placement only. It does not edit Bun source and it does not
create a repair packet.

`ts_mention_not_observer`

A token such as `maxByteLength` appears, but it is not a real observer. String
or comment mentions, non-Blob inputs, smoke assertions, and token-only reads do
not prove stable copied bytes.

Action:

```text
Reject the token mention. Look for a real Blob/view input plus byte/text/value
assertion before crediting TypeScript evidence.
```

`bridge_unknown`

TypeScript evidence may exist, but the binding or FFI graph leg is missing.

Action:

```text
Inspect or add bridge evidence before crediting the TypeScript test to the Rust
seam. Do not call this no_static_path, and do not suggest a Rust test merely
because the bridge is unknown.
```

`public_reachable_panic_boundary_unrevealed`

A public FFI entrypoint appears to reach a Rust panic boundary, but the
negative-offset panic oracle and safe external observer target are unresolved.

Action:

```text
Keep the named limitation. Do not suggest a Rust or TypeScript test file, do
not invent a verify or receipt command, and do not emit a public repair packet.
The unlock route is analysis/cross-language-panic-boundary-visibility.
```

## Check Calibration Receipts

From the `ripr` repository, the calibrated receipts are available without
running Bun:

```bash
cargo xtask bun-ub-calibration
cargo xtask dogfood
```

Useful output files:

```text
target/ripr/reports/bun-ub-calibration.md
target/ripr/reports/bun-ub-calibration.json
target/ripr/reports/dogfood.md
target/ripr/reports/dogfood.json
```

The calibration report should show the known-good Blob route, stripped
shared/resizable variants, mention-only control, and bridge-unknown limitation.
The dogfood report should include `bun_ub_cross_language_witnesses` with the
#31648-style known-good, stripped-resizable, mention-only, and #950 FFI
negative-offset panic-boundary operator receipts.

## Do Not Claim

Do not claim any of these from the preview result:

- TypeScript or JavaScript parity with Rust.
- Full Bun binding graph coverage.
- Runtime Bun, Jest, Vitest, `tsc`, `tsserver`, or mutation execution.
- Generated tests or Bun source edits.
- Public repair packets, default gates, badges, baselines, RIPR Zero, or
  support-tier promotion.
- UB proof or runtime behavior proof.

The useful claim is only:

```text
For calibrated Bun Blob / ArrayBuffer stable-byte seams, ripr can give an
advisory signal that TypeScript evidence is discriminated, missing a named
discriminator, token-only, or blocked by an unknown bridge. For the calibrated
Bun FFI negative-offset sample, ripr can give an advisory named limitation when
the panic-boundary oracle and safe external observer target are unresolved.
```
