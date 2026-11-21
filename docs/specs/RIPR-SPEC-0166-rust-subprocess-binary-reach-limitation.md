# RIPR-SPEC-0166: Rust subprocess binary reach limitation

Status: proposed

## Problem

An integration test can invoke a Cargo-built binary without ripr being able to
map that executable back to the changed Rust owner. Treating that subprocess as
proof of reach, receipt validity, or coverage would overstate static evidence.

## Behavior

For a `no_static_path` finding whose owner is in `src/main.rs` or
`src/bin/**`, ripr may emit:

```text
static_limit_kind = rust_subprocess_binary_reach_unresolved
```

when an integration test contains either a `Command::new` invocation using
`CARGO_BIN_EXE_*` with `.output()`/`.status()`, or an `assert_cmd`
`cargo_bin(...)` invocation with an assertion/output/status observation.

The recognizer is source-only and deterministic. It does not spawn a process,
parse arbitrary shell commands, infer a binary target map, or credit the test as
reaching any owner.

## Required Evidence

The emitted limitation must identify the changed owner and the deterministic
integration-test location. Its evidence must state that the executable-to-owner
mapping is unresolved and that no subprocess reach, receipt, coverage, or repair
claim is made.

## Non-Goals

This limitation does not count a subprocess test as related coverage, validate a
receipt, infer Cargo binary targets, resolve arbitrary shell execution, or
replace existing transitive/macro witness behavior.

## Acceptance Examples

Accepted shapes include `Command::new(env!("CARGO_BIN_EXE_worker")).output()`
and `Command::cargo_bin("worker").unwrap().assert().success()`. A shell command
such as `Command::new("sh")`, even when a Cargo executable token is mentioned
nearby, is not accepted.

## Test Mapping

- `crates/ripr/src/analysis/language/rust.rs::tests::cargo_binary_invocation_shape_is_conservative_and_deterministic`
- `crates/ripr/src/analysis/language/rust.rs::tests::subprocess_limit_only_applies_to_binary_source_paths_and_integration_tests`
- `crates/ripr/src/domain/language.rs` static-limit wire and description tests
- `crates/ripr/src/lsp/gap_artifacts.rs` static-limit validation test

## Metrics

This slice records only the count of unresolved Cargo subprocess boundaries via
the typed static-limit kind. It intentionally emits no subprocess reach,
receipt-validity, coverage, or repair-success metric.

## Implementation Mapping

- Rust adapter classification in `crates/ripr/src/analysis/language/rust.rs`
- Wire value and description in `crates/ripr/src/domain/language.rs`
- LSP known-kind validation in `crates/ripr/src/lsp/gap_artifacts.rs`
- Contract registration in `.ripr/traceability.toml`, `docs/STATIC_LIMITS.md`,
  and `policy/doc-artifacts.toml`

## Follow-up Boundary

Counting a subprocess test as evidence requires a separate Cargo binary-target
map and a bounded owner relation. That work is intentionally not part of this
limitation slice.
