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

## First-Run Loop

In the Bun checkout:

1. Add or inspect the repo-root `ripr.toml` Bun UB preview stanza.
2. Run `ripr doctor --root .` and confirm the Bun UB profile is configured.
3. Run `ripr check --root . --base origin/main --mode draft --format human`
   for the current Rust/FFI change.
4. If an agent or reviewer needs structured evidence, also write JSON with
   `ripr check --root . --base origin/main --mode draft --format json`.

In the `ripr` checkout, use the calibrated receipts when you need a known-good
operator reference without running Bun:

```bash
cargo xtask bun-ub-preview-summary
cargo xtask configured-bridge-inventory
cargo xtask bun-ub-calibration
cargo xtask dogfood
```

The first screen to read is
`target/ripr/reports/bun-ub-preview-summary.md`. If the summary points at a
missing bridge or manifest-only surface, read
`target/ripr/reports/configured-bridge-inventory.md` before suggesting any
test placement.

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
ripr doctor --root .
```

Expected signal:

```text
Bun UB profile: configured (preview advisory only)
```

If the profile is not configured, do not credit TypeScript evidence to a Rust
seam as a configured Bun UB review result. Add or fix `ripr.toml` first, then
rerun `ripr doctor`; do not replace missing configuration with a guessed Rust
test target.

## Run Diff-Scoped Evidence

For ordinary PR review, start with changed-surface output:

```bash
ripr check \
  --root . \
  --base origin/main \
  --mode draft \
  --format human
```

For a machine-readable artifact:

```bash
mkdir -p target/ripr/bun-ub
ripr check \
  --root . \
  --base origin/main \
  --mode draft \
  --format json \
  > target/ripr/bun-ub/check.json
```

Use repo-wide `repo-exposure-json` only for an intentional large-repo refresh.
The normal Bun UB operator loop should not require a full no-ledger repo scan.

The JSON preview card may include a Bun cross-language advisory packet with
the Rust seam, TypeScript placement, missing discriminators, bridge confidence,
missing graph legs, `must_not_change`, stop condition, raw refs, and
`repair_packet_ready=false`. The proof-mode fields are advisory labels such as
`observable_red_green`, `mutation_plus_miri`, `helper_gated`,
`bridge_unknown`, or `static_limitation`; they do not mean RIPR ran Bun, Miri,
or mutation tests.

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

`named_static_limitation`

A configured or manifest-only row is visible but intentionally not safely
actionable yet. Examples include node:fs scalar write manifest-only routing,
Bun.write helper-gated routing, or an unresolved public panic-boundary route.

Action:

```text
Read the named missing graph legs and unlock condition. Do not infer placement
from the inventory row, do not emit a public repair packet, and do not claim
coverage until the bridge, helper, and observer legs are credited.
```

## Check Calibration Receipts

From the `ripr` repository, the calibrated receipts are available without
running Bun:

```bash
cargo xtask bun-ub-preview-summary
cargo xtask configured-bridge-inventory
cargo xtask bun-ub-calibration
cargo xtask dogfood
```

Useful output files:

```text
target/ripr/reports/bun-ub-preview-summary.md
target/ripr/reports/bun-ub-preview-summary.json
target/ripr/reports/configured-bridge-inventory.md
target/ripr/reports/configured-bridge-inventory.json
target/ripr/reports/bun-ub-calibration.md
target/ripr/reports/bun-ub-calibration.json
target/ripr/reports/dogfood.md
target/ripr/reports/dogfood.json
```

The preview summary is the one-screen operator receipt. It reports calibrated
routes, state counts, named static limitations, dogfood receipt counts, public
packet exclusions, and the preview/advisory authority boundary.

The configured bridge inventory is report-only. It lists configured Blob,
`copy_to_unshared`, and MarkdownObject bridge profiles, the Blob
`bridge_unknown` row, node:fs scalar write and Bun.write manifest-only future
surfaces, and named limitations. It must not be used as inferred reachability
or automatic placement evidence.

The calibration report should show the known-good Blob route, stripped
shared/resizable variants, mention-only control, and bridge-unknown limitation.
The dogfood report should include `bun_ub_cross_language_witnesses` with the
#31648-style known-good, stripped-resizable, mention-only, bridge-unknown,
`copy_to_unshared`, MarkdownObject, node:fs scalar write, Bun.write, and #950
FFI negative-offset panic-boundary operator receipts.

Schema details for these surfaces live in:

- [Bun UB Preview Summary Report](OUTPUT_SCHEMA.md#bun-ub-preview-summary-report)
- [Configured Bridge Inventory Report](OUTPUT_SCHEMA.md#configured-bridge-inventory-report)
- [Cross-language evidence router UX](specs/RIPR-SPEC-0063-cross-language-evidence-router-ux.md)

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
