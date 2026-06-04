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

That row has TypeScript Blob callsite and oracle samples, but it remains
`bridge_unknown` until a configured or generated binding edge connects the
external observer path to `copy_to_unshared`.

## When

RIPR evaluates the fixture corpus, it should distinguish:

- a complete advisory TypeScript witness;
- missing external discriminator evidence;
- TypeScript token mentions that are not Blob observers;
- unknown binding or FFI routes;
- unresolved cross-language test target placement.
Each row names its graph `profile` so later non-Blob profiles can be added
without weakening the Bun Blob contract.

## Then

Complete configured witnesses stay advisory and non-actionable. Incomplete graph
rows become `static_limitation` records with a named limitation category,
repair route, missing graph legs, unlock condition, and structured raw refs.
Configured missing-discriminator rows may name
`test/js/web/fetch/blob.test.ts` as advisory placement; bridge-unknown,
mention-only, missing-oracle, and target-unresolved rows must not.

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
