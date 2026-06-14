# RIPR-SPEC-0106: Error-Seam unwrap_err / expect_err Variant Binding

Status: accepted

Owner: product / swarm

Created: 2026-06-14

Linked issues:

- #1168

Linked PRs:

- None yet

Support-tier impact:

- Honesty fix for Rust primary-language output: `error_path` seams backed by a
  `let err = f().unwrap_err(); assert_eq!(err, MyError::Variant)` pattern now
  correctly report `exposed` with a `strong` discriminator instead of
  `weakly_exposed`. Grip may only RISE when the assertion structurally pins the
  changed seam's exact variant. Sibling-variant, generic-error, and
  unprovable-variant assertions remain `weakly_gripped` (fail-closed).
  Claim boundaries and tier labels remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, or LSP servers.
- New `pub(crate)` functions `unwrap_err_bound_variables`,
  `is_unwrap_err_bound_error_assertion`, `contains_named_enum_variant` in
  `crates/ripr/src/analysis/extract/oracles/patterns.rs` and
  `crates/ripr/src/analysis/extract/oracles/scan.rs`.
- New `pub(in crate::analysis)` re-exports in
  `crates/ripr/src/analysis/classify/mod.rs` for `enum_variant_values` and
  `exact_error_variant`.
- `assertion_matches_probe_detail` in `classify/reveal.rs` gains an
  `error_path_variant: Option<&str>` parameter; all callers updated.
- `discriminate_evidence` in `test_grip_evidence.rs` delegates to new
  `oracle_discriminates_seam` (variant-aware) instead of plain
  `oracle_kind_matches_seam`.
- No `schema_version` bump. The `exposed` / `strong` shape is an existing
  valid output contract.
- Register this spec in `policy/doc-artifacts.toml` and `docs/specs/README.md`.

## Problem

For an `ErrorVariant` seam, the common test pattern is:

```rust
let err = f(-1).unwrap_err();
assert_eq!(err, MyError::Variant);
```

Before this PR, the `assert_eq!(err, MyError::Variant)` assertion was lexed as
`OracleKind::ExactValue` (not `ExactErrorVariant`) because the line does not
contain `Err(`. This caused:

- `error_path` seams to report `weakly_exposed` even when the test structurally
  pins the exact error variant — a false-weak rating.
- The seam to show a misleading "missing discriminator" warning when the
  discriminator is actually present.

### The defect root

`is_exact_error_variant_assertion` in `patterns.rs` required `Err(` in the
assertion line. A `unwrap_err()` binding eliminates the `Err(` wrapper:

```rust
// asserted directly (ExactErrorVariant via Err( — old path works):
assert_eq!(result, Err(MyError::Variant));

// asserted via binding (ExactValue erroneously — this PR fixes):
let err = result.unwrap_err();
assert_eq!(err, MyError::Variant);
```

## Behavior

### Part A — Recognition (both paths)

Before classifying assertions in a test body, perform a pre-pass to collect
`unwrap_err_bound_variables`: variables bound as
`let <var> [: <type>] = <expr>.unwrap_err()` or
`let <var> [: <type>] = <expr>.expect_err("…")`.

When an assertion references one of these bound variables AND contains a named
enum-variant token (`SomePath::Variant` with uppercase last component), upgrade
the oracle from `ExactValue` to `ExactErrorVariant` / `Strong`. This upgrade
applies in BOTH the lexical path (`extract_assertions` in `scan.rs`) and the
rust-analyzer path (`extract_parser_oracles` in `ra.rs`).

FAIL-CLOSED rule: when the binding is present but the assertion does NOT
contain a named enum-variant token (e.g. `assert!(err.to_string().contains(…))`),
do NOT upgrade. The classification stays at its original kind.

### Part B — Variant binding (over-credit guard, both paths)

For `ErrorPath` probes, an `ExactErrorVariant` assertion only credits the seam
when it pins the probe's SPECIFIC variant token. A sibling-variant assertion
(`CalcError::Negative`) must NOT credit a `CalcError::TooLarge` seam, even
though both share the `CalcError` qualifier token.

Implementation:

- **Diff-mode** (`classify/reveal.rs`): `error_path_variant_token` extracts the
  post-`::` uppercase component of the probe's required error expression. In
  `assertion_matches_probe_detail`, when the probe family is `ErrorPath` and the
  assertion kind is `ExactErrorVariant` and `error_path_variant` is `Some`, the
  match is restricted to assertions whose text contains the specific variant
  token. Without a parseable variant, fall through to the standard match.

- **Repo-exposure** (`test_grip_evidence.rs`): `oracle_discriminates_seam`
  replaces the plain `oracle_kind_matches_seam` call for grip grading.
  For `ErrorVariant` seams, it additionally calls
  `error_variant_oracle_matches_seam_variant`, which parses both the
  `RequiredDiscriminator::ErrorVariant { variant }` on the seam and the
  oracle assertion text and rejects a mismatch.

## Non-Goals

- Does NOT recognize `expect_err` in a middle position (only at the end of the
  binding expression).
- Does NOT handle indirect rebinding (`let e = err; assert_eq!(e, …)`).
- Does NOT add any Rust-analyzer cross-function flow; this remains lexical.
- Does NOT change `oracle_kind_matches_seam_kind` (unchanged single source of
  truth for seam-kind ↔ oracle-kind matching used by grip grading).
- Does NOT bump `schema_version`.
- Does NOT bump crate version, publish, or touch release workflows.
- Static-language clean: all new code and output uses allowed vocabulary only.

## Required Evidence

### Fixture 1 — POSITIVE (unwrap_err_variant_positive)

- Changed seam: `return Err(CalcError::Negative)`
- Test: `let err = compute(-1).unwrap_err(); assert_eq!(err, CalcError::Negative);`
- Expected: `error_path` → `exposed`, `discriminator yes`, `exact_error_variant`

### Fixture 2 — SIBLING-VARIANT (unwrap_err_sibling_variant)

- Two error returns: `CalcError::Negative` and `CalcError::TooLarge`
- Changed seam: `TooLarge`
- Test: only pins `CalcError::Negative`
- Expected: `error_path` for `TooLarge` → NOT `exposed` (at most `weakly_exposed`)

### Fixture 3 — GENERIC (unwrap_err_generic_is_err)

- Changed seam: `return Err(CalcError::Negative)`
- Test: `let err = compute(-1).unwrap_err(); assert!(err.to_string().contains("error"));`
- Expected: `error_path` → `weakly_exposed` (generic assertion; no variant token)

### Fixture 4 — SUCCESS-PATH NO REGRESSION (existing: weak_error_oracle)

- Test uses `assert!(authenticate("").is_err())` with no `unwrap_err()` binding
- Expected: `error_path` remains `weakly_exposed` (no regression from this PR)

## Unit Tests

Tests in `crates/ripr/src/analysis/extract/oracles/` and
`crates/ripr/src/analysis/classify/reveal.rs`:

1. `unwrap_err_binding_recognized_and_variable_collected` — `unwrap_err_bound_variables`
   collects the bound variable name.
2. `is_unwrap_err_bound_error_assertion_upgrades_named_variant` — upgrade fires
   when assertion references bound var AND contains enum variant token.
3. `generic_assertion_on_bound_var_not_upgraded` — assertion without variant
   token stays `ExactValue`.
4. `sibling_variant_assertion_does_not_match_tool_large_probe` —
   `assertion_matches_probe_detail` with `error_path_variant = Some("TooLarge")`
   and assertion text containing only `Negative` → `(false, false)`.

## Test Mapping

| Test | Fixture |
|---|---|
| `unwrap_err_binding_recognized_and_variable_collected` | Part A recognition |
| `is_unwrap_err_bound_error_assertion_upgrades_named_variant` | Fixture 1 positive |
| `generic_assertion_on_bound_var_not_upgraded` | Fixture 3 generic |
| `sibling_variant_assertion_does_not_match_tool_large_probe` | Fixture 2 sibling |

## Acceptance Examples

### Before (incorrect — weakly_exposed even with exact variant test)

```
Probe
  family: error_path
  delta:  value

Static exposure
  weakly_exposed (warning, confidence 0.92)

Evidence
  - discriminator weak: Medium oracle found: property or partial structural assertion
  - related test tests/errors.rs:4 uses medium exact error variant oracle: assert_eq!(err, CalcError::Negative);
```

### After (correct — exposed, strong discriminator)

```
Probe
  family: error_path
  delta:  value

Static exposure
  exposed (info, confidence 1.00)

Evidence
  - discriminator yes: Strong oracle found: exact error variant assertion
  - related test tests/errors.rs:4 uses strong exact error variant oracle: assert_eq!(err, CalcError::Negative);
```

## Implementation Mapping

| Behavior | Code location |
|---|---|
| `unwrap_err_bound_variables` body pre-pass | `crates/ripr/src/analysis/extract/oracles/scan.rs` |
| `is_unwrap_err_bound_error_assertion` upgrade gate | `crates/ripr/src/analysis/extract/oracles/patterns.rs` |
| `contains_named_enum_variant` token check | `crates/ripr/src/analysis/extract/oracles/patterns.rs` |
| Lexical path upgrade (`extract_assertions`) | `crates/ripr/src/analysis/extract/oracles/scan.rs` |
| RA path upgrade (`extract_parser_oracles`) | `crates/ripr/src/analysis/syntax/ra.rs` |
| Sibling-variant guard — diff-mode | `crates/ripr/src/analysis/classify/reveal.rs` |
| Sibling-variant guard — repo-exposure | `crates/ripr/src/analysis/test_grip_evidence.rs` |
| `enum_variant_values`, `exact_error_variant` re-exported | `crates/ripr/src/analysis/classify/mod.rs` |
| Spec registration | `policy/doc-artifacts.toml`, `docs/specs/README.md` |
| Traceability | `.ripr/traceability.toml` |

## Metrics

- `unwrap_err_variant_grip_raise`: `error_path` seam with exact variant
  `unwrap_err` binding → `exposed` (fixture 1).
- `sibling_variant_no_credit`: `error_path` seam for `TooLarge` not credited
  by a `Negative`-only test (fixture 2, fail-closed guard).
