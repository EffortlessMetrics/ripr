# Fixture: cross-language-oracle-graph-corpus

Spec: RIPR-SPEC-0062

This manifest-only fixture pins bounded cross-language oracle graph states for
configured Bun TypeScript profiles before analyzer actionability changes.

## Given

A Rust seam in Bun is exercised through TypeScript-facing Blob behavior. The
first profile is the configured Bun Blob route:

```text
src/jsc/Blob.rs
Blob::from_js_without_defer_gc
array_buffer.shared || array_buffer.resizable
```

The external evidence lives in `test/js/web/fetch/blob.test.ts`, and the graph
must name the Rust seam, boundary discriminator, binding or FFI edge, external
callsite, external oracle, source locations, raw evidence refs, and non-claims.

The #910 follow-up profile also records the `copy_to_unshared` seam:

```text
src/jsc/array_buffer.rs:341
copy_to_unshared
SharedArrayBuffer and resizable ArrayBuffer copy semantics
```

That row has TypeScript Blob callsite and oracle samples. The bridge-route
slice names a configured binding edge from the external observer path to
`copy_to_unshared`, so the row is credited only as a complete advisory witness.
It still does not become a public repair packet, suggested test target, verify
route, receipt route, or allowed edit surface.

The #951 follow-up profile records the `MarkdownObject::to_string` seam:

```text
src/runtime/api/MarkdownObject.rs:60
MarkdownObject::to_string
self.0.resizable && !self.0.shared
```

That row has a configured bridge from `Bun.markdown` in
`test/js/bun/md/md-edge-cases.test.ts` to the Rust MarkdownObject path. It is
credited only as a complete advisory witness when the TypeScript sample carries
a resizable ArrayBuffer discriminator, a `Bun.markdown` callsite, and a strong
markdown output oracle. It still does not become a public repair packet,
suggested Rust or TypeScript test target, verify route, receipt route, or
allowed edit surface.

The #950 follow-up profile records a Bun FFI panic boundary:

```text
src/bun.js/bindings/FFIObject.rs:277
FFIObject::read
usize::try_from(to_int32()).expect("int cast")
```

That row has a TypeScript-facing `read.u8(ptr, -1)` public entrypoint sample
and an FFI binding edge sample, but the concrete TypeScript test location,
negative-offset panic oracle, and safe external observer target remain
unresolved. It is therefore a named static limitation with no Rust test
suggestion, no external-language test placement, no verify route, no receipt
route, and no allowed edit surface.

The node:fs scalar-write intake row records the first non-Blob stable-byte
profile as manifest-only:

```text
unresolved:node-fs-scalar-write-rust-seam
node:fs scalar write sink
JS-owned bytes must be copied before native write scalar sinks
```

The typed witness path is `test/js/node/fs/fs.test.ts`, and the advisory proof
mode is `observable_red_green`. The row names missing bridge and scalar-write
oracle graph legs, so it stays a named static limitation. The placement path is
only recorded because the manifest includes an explicit typed placement raw ref;
the row still does not create analyzer behavior, public actionability, a verify
route, a receipt route, a generated test, or an allowed edit surface.

The Bun.write helper-gated intake row records the next stable-byte profile as
manifest-only:

```text
unresolved:bun-write-stable-byte-rust-seam
Bun.write stable-byte sink
JS-owned bytes must not cross Bun.write native sinks without a helper
```

The manifest witness path is `test/js/bun/write.test.ts`, but the row records
`suggested_test_file = not_applicable` because the helper, bridge, and
stable-byte write oracle are not credited. The advisory proof mode is
`helper_gated`, and the unlock condition names `bun_write_fixture_helper`.
There is no binding-edge raw ref, no placement raw ref, no verify route, no
receipt route, no generated test, and no allowed edit surface.

## When

RIPR evaluates the fixture corpus, it should distinguish:

- a complete advisory TypeScript witness;
- missing external discriminator evidence;
- TypeScript token mentions that are not Blob observers;
- unknown binding or FFI routes;
- unresolved cross-language test target placement.
- public-reachable panic boundaries whose external oracle path is unresolved.
- manifest-only profile intake rows that name future bridge or oracle legs
  without crediting them.
- helper-gated manifest rows that name the missing helper or primitive without
  suggesting a test file.
Each row names its graph `profile` so later non-Blob profiles can be added
without weakening the Bun Blob or MarkdownObject contracts.

## Then

Complete configured witnesses stay advisory and non-actionable. Incomplete graph
rows become `static_limitation` records with a named limitation category,
repair route, missing graph legs, unlock condition, and structured raw refs.
Configured missing-discriminator rows may name
`test/js/web/fetch/blob.test.ts` as advisory placement; bridge-unknown,
mention-only, missing-oracle, target-unresolved, and panic-boundary limitation
rows must not.

## Must Not

- Emit public repair packets.
- Suggest Rust test files or external-language test files without configured
  missing-discriminator placement evidence.
- Emit verify or receipt commands.
- Populate an allowed edit surface.
- Claim provider evidence, source edits, generated tests, runtime Bun execution,
  mutation execution, default gate authority, public badge contribution,
  baseline authority, RIPR Zero authority, TypeScript Rust parity, full
  cross-language proof, or support-tier promotion.

## Validate

```bash
cargo xtask check-fixture-contracts
cargo test -p xtask cross_language_oracle_graph_corpus_cases_are_checked -- --test-threads=1
```
