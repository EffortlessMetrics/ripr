# RIPR-SPEC-0107: ErrorPath Requires a Variant-Observing Oracle

Status: accepted

Owner: product / swarm

Created: 2026-06-14

Linked issues:

- (none — self-identified honesty bug)

Linked PRs:

- None yet

Support-tier impact:

- Honesty fix for Rust primary-language output: `error_path` seams previously
  reported `exposed` when only a sibling exact-value oracle or a broad `is_err()`
  oracle existed. These now correctly report `weakly_exposed` via
  `observation_unverified`. A genuine variant-pinning oracle (`ExactErrorVariant`
  whose assertion text contains the probe's specific variant token) still clears
  the guard and keeps the seam `exposed`. Grip may only FALL (fail-closed).
  Claim boundaries and tier labels remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, functions, or LSP servers.
- One-line addition: `ProbeFamily::ErrorPath` added to `needs_token_confirmation`
  in `crates/ripr/src/analysis/classify/reveal.rs`.
- No `schema_version` bump. The `weakly_exposed` / `observation_unverified`
  shape is an existing valid output contract (RIPR-SPEC-0094).
- Register this spec in `policy/doc-artifacts.toml` and `docs/specs/README.md`.

## Problem

### The fake-clean

An `error_path` probe for `return Err(ParseError::TooLong(len))` was reported
as `exposed` (confidence 1.0) when only a sibling `ExactValue` oracle existed:

```rust
assert_eq!(validate_or_default("hello"), Ok("valid"));
```

This oracle covers the happy-path return value — it cannot observe whether the
error variant is `TooLong` or `TooShort`. Yet `ripr check` emitted:

```
classification: exposed
discriminate: yes (Strong oracle found: exact value or pattern assertion)
```

while simultaneously listing:

```
missing: ["Missing discriminator value: ParseError::TooLong"]
```

The finding self-contradicted: `exposed` + `missing` at the same time. The
`agent brief` surface graded the same seam `weakly_gripped`, creating a
cross-surface disagreement.

### The root cause

`ProbeFamily::ErrorPath` was NOT in `needs_token_confirmation` (~L73 of
`crates/ripr/src/analysis/classify/reveal.rs`). RIPR-SPEC-0094's
`observation_unverified` guard (which already protects
`MatchArm`/`ReturnValue`/`FieldConstruction`/`SideEffect`/`CallDeletion`)
therefore never fired for `error_path` probes.

So an `error_path` seam reached the `OracleStrength::Strong` arm of
`build_discriminate_evidence` and graded `exposed`/`discriminate=yes` while
crediting a sibling `ExactValue` oracle that observed a different expression
entirely.

RIPR-SPEC-0106's variant gate (`assertion_matches_probe_detail` ~L327-334)
only restricts `ExactErrorVariant` oracles for `ErrorPath` probes. A sibling
`ExactValue` oracle bypassed it entirely.

## Behavior

### The fix (one line)

Add `ProbeFamily::ErrorPath` to `needs_token_confirmation` in
`crates/ripr/src/analysis/classify/reveal.rs`:

```rust
fn needs_token_confirmation(family: &ProbeFamily) -> bool {
    matches!(
        family,
        ProbeFamily::MatchArm
            | ProbeFamily::ReturnValue
            | ProbeFamily::FieldConstruction
            | ProbeFamily::SideEffect
            | ProbeFamily::CallDeletion
            | ProbeFamily::ErrorPath   // RIPR-SPEC-0107
    )
}
```

### What this does

When `needs_token_confirmation` returns true for a probe, the loop in
`analyze_related_assertions` initializes `observation_unverified = false` and
sets it to `true` on the first matching assertion unless `has_token_match=true`
(or, for effect families, an effect observer fires). If `observation_unverified`
is still true after all assertions, `build_discriminate_evidence` returns
`StageState::Weak` with the "Discriminator unconfirmed: no assertion text
references this probe's changed expression (observation_unverified)" message.

For `ErrorPath`, confirmation works via the existing RIPR-SPEC-0106 (Part B)
path: `assertion_matches_probe_detail` fast-paths `ErrorPath+ExactErrorVariant`
and returns `has_token_match=true` when the assertion text contains the probe's
specific variant token. So a genuine `assert_eq!(err, ParseError::TooLong(12))`
or `assert!(matches!(err, ParseError::TooLong(_)))` clears `observation_unverified`
and the seam stays `exposed`. A sibling `CalcError::Negative` oracle or a
broad `is_err()` oracle does not.

### ErrorPath is a value family, not an effect family

`is_effect_family` returns `false` for `ErrorPath`. This is correct and
unchanged. A mock/expectation/snapshot must NOT clear `observation_unverified`
for an error-variant seam — only a genuine variant-pinning oracle may confirm it.

## Controls

### Control A — REPRO (fake-clean removed)

Error-path probe for `return Err(ParseError::TooLong)` with only a sibling
`ExactValue` oracle (`assert_eq!(validate_or_default("hello"), Ok("valid"))`)
or only a broad `is_err()` oracle (`assert!(authenticate("").is_err())`):

- **Before**: `exposed`, `discriminate=yes`, self-contradicting with `missing`
- **After**: `weakly_exposed`, `discriminate=weak (observation_unverified)`, no
  self-contradiction

Covered by:
- Fixture `error_path_sibling_oracle_fake_clean` (new)
- Unit test `error_path_broad_oracle_only_downgrades_discriminate_to_weak`
- Unit test `error_path_sibling_exact_value_oracle_downgrades_discriminate_to_weak`

### Control B — MUST-NOT-OVER-CORRECT (genuine variant oracle stays exposed)

Error-path probe for `return Err(ParseError::TooLong(len))` with a real
variant-pinning oracle:

```rust
let err = validate("x".repeat(256)).unwrap_err();
assert_eq!(err, ParseError::TooLong(12));
// or:
assert!(matches!(err, ParseError::TooLong(_)));
```

These assertions contain the variant token `TooLong`, so the RIPR-SPEC-0106
Part B path in `assertion_matches_probe_detail` returns `(true, true)` →
`has_token_match=true` → `observation_unverified=false` → seam stays `exposed`.
The RIPR-SPEC-0106 / RIPR-SPEC-0094 variant-credit path is preserved.

Covered by:
- Unit test `error_path_exact_variant_oracle_keeps_discriminate_yes`
- Unit test `error_path_matches_variant_oracle_keeps_discriminate_yes`
- Existing fixture `strong_error_oracle` (unchanged: stays `exposed`)
- Existing fixture `weak_error_oracle_assert_matches` (unchanged: stays `exposed`)
- Existing fixture `unwrap_err_variant_positive` (unchanged: stays `exposed`)

### Control C — IS-EFFECT-FAMILY guard

`is_effect_family(&ProbeFamily::ErrorPath)` must return false. A mock or
snapshot must NOT clear `observation_unverified` for an error-variant seam.

Covered by:
- Unit test `error_path_is_not_effect_family`

## Required Evidence

### Fixture — REPRO (error_path_sibling_oracle_fake_clean)

- Changed seam: `return Err(ParseError::TooLong)`
- Test: only a sibling `ExactValue` oracle on the happy-path return
  (`assert_eq!(validate_or_default("hello"), Ok("valid"))`)
- Expected: `error_path` probe → `weakly_exposed`, `discriminate=weak`,
  summary contains `observation_unverified`
- Verified with: `cargo xtask fixtures error_path_sibling_oracle_fake_clean`

### Non-regression — POSITIVE (existing: strong_error_oracle)

- Changed seam: `return Err(AuthError::RevokedToken)`
- Test: `assert!(matches!(authenticate(""), Err(AuthError::RevokedToken)))`
  (ExactErrorVariant, variant token `RevokedToken` in assertion text)
- Expected: `error_path` probe → `exposed` (unchanged from pre-spec)
- Verified with: `cargo xtask fixtures strong_error_oracle`

### Non-regression — POSITIVE (existing: weak_error_oracle_assert_matches)

- Changed seam: `return Err(AuthError::RevokedToken)`
- Test: `assert_matches!(authenticate(""), Err(AuthError::RevokedToken))`
  (ExactErrorVariant, variant token matches)
- Expected: `error_path` probe → `exposed` (unchanged from pre-spec)

### Non-regression — POSITIVE (existing: unwrap_err_variant_positive)

- Changed seam: `return Err(CalcError::Negative)`
- Test: `let err = compute(-1).unwrap_err(); assert_eq!(err, CalcError::Negative);`
  (ExactErrorVariant via RIPR-SPEC-0106 upgrade, variant token `Negative` matches)
- Expected: `error_path` probe → `exposed` (unchanged from pre-spec)

### Re-blessed — DRIFT (existing: weak_error_oracle)

- Changed seam: `return Err(AuthError::RevokedToken)`
- Test: `assert!(authenticate("").is_err())` (BroadError, no variant token)
- Expected: `error_path` probe → `weakly_exposed` (classification unchanged)
- Drift: discriminate summary changes to `observation_unverified` (correct)

### Re-blessed — DRIFT (existing: unwrap_err_generic_is_err)

- Changed seam: `return Err(CalcError::Negative)`
- Test: `assert!(err.to_string().contains("error"))` (RelationalCheck, no variant token)
- Expected: `error_path` probe → `weakly_exposed` (classification unchanged)
- Drift: discriminate summary changes to `observation_unverified` (correct)

## Acceptance Examples

### Before (incorrect — exposed with self-contradiction)

```
Static exposure
  exposed (info, confidence 1.00)

Evidence
  - discriminator yes: Strong oracle found: exact value or pattern assertion
  - related test tests/validation.rs:8 valid_input_returns_valid uses strong exact value oracle

Missing
  - Missing discriminator value: ParseError::TooLong
```

The finding says `exposed` (discriminated) but simultaneously lists
`Missing discriminator value: ParseError::TooLong`. Self-contradiction.

### After (correct — weakly_exposed, no self-contradiction)

```
Static exposure
  weakly_exposed (warning, confidence 0.92)

Evidence
  - discriminator weak: Discriminator unconfirmed: no assertion text references
    this probe's changed expression (observation_unverified)

Missing
  - No exact error variant discriminator was detected
  - Missing discriminator value: ParseError::TooLong
```

### Variant-pinning positive (must-not-over-correct)

```
// Test: assert_eq!(err, ParseError::TooLong(12));

Static exposure
  exposed (info, confidence 1.00)

Evidence
  - discriminator yes: Strong oracle found: exact error variant assertion
```

This stays `exposed`. The variant token `TooLong` appears in the assertion text,
so `has_token_match=true` clears `observation_unverified`.

## Test Mapping

| Test | Spec control |
|---|---|
| `error_path_broad_oracle_only_downgrades_discriminate_to_weak` | Control A — broad is_err() repro |
| `error_path_sibling_exact_value_oracle_downgrades_discriminate_to_weak` | Control A — sibling ExactValue repro |
| `error_path_exact_variant_oracle_keeps_discriminate_yes` | Control B — variant-pinning must not over-correct |
| `error_path_matches_variant_oracle_keeps_discriminate_yes` | Control B — matches! variant stays exposed |
| `error_path_is_not_effect_family` | Control C — mock cannot clear error seam |
| `given_broad_is_err_assertion_when_error_variant_changes_then_oracle_is_weak` | Integration — classifier confirms observation_unverified (updated) |
| Fixture `error_path_sibling_oracle_fake_clean` | End-to-end repro — binary output confirmed |

## Non-Goals

- Does NOT add a new function, JSON field, schema version, or output contract.
- Does NOT change the RIPR-SPEC-0106 variant binding or the RIPR-SPEC-0094
  observation_unverified machinery beyond extending the family list by one entry.
- Does NOT change `is_effect_family` (mock/snapshot still cannot clear an
  error-path seam).
- Does NOT bump crate version, publish, or touch release workflows.
- Does NOT affect Python/TypeScript adapters (different code paths).
- Static-language clean: output uses `exposed`, `weakly_exposed`,
  `observation_unverified` only — all allowed vocabulary.

## Golden Drift (re-blessed fixtures)

Adding `ErrorPath` to `needs_token_confirmation` caused the discriminate message
to change in two existing fixtures. Both kept `weakly_exposed` classification.

### `weak_error_oracle`

- Test oracle: `assert!(authenticate("").is_err())` — broad `is_err()`,
  `BroadError/Weak`. No variant token (`AuthError`, `RevokedToken`) appears in
  the assertion text.
- Before: discriminate summary "Only broad error oracle found; is_err() does not
  discriminate exact error variants" (oracle-strength path)
- After: discriminate summary "Discriminator unconfirmed: no assertion text
  references this probe's changed expression (observation_unverified)"
- Classification: `weakly_exposed` → `weakly_exposed` (unchanged)
- Re-bless rationale: the oracle-strength message was technically correct but
  missed the root reason — the assertion also provides no token confirming this
  specific seam. `observation_unverified` is the more honest diagnosis.

### `unwrap_err_generic_is_err`

- Test oracle: `assert!(err.to_string().contains("error"))` — generic string
  check, `RelationalCheck/Weak`. No variant token (`CalcError`, `Negative`)
  appears in the assertion text.
- Before: discriminate summary "Only relational oracle found; it may not
  discriminate the changed value exactly" (oracle-strength path)
- After: discriminate summary "Discriminator unconfirmed: no assertion text
  references this probe's changed expression (observation_unverified)"
- Classification: `weakly_exposed` → `weakly_exposed` (unchanged)
- Re-bless rationale: same — the assertion fires via single-assertion escape
  hatch (assertion count == 1) without any variant token confirming this seam.
  `observation_unverified` is the more accurate diagnosis (RIPR-SPEC-0107).

No fixture changed from `exposed` → `weakly_exposed`. No existing `exposed`
seam was wrongfully downgraded.

## Acceptance Criteria

1. `ripr check --diff <sibling-oracle-repro> --json` shows `error_path` probe
   as `weakly_exposed` with `discriminate.summary` containing
   `observation_unverified`. No `exposed` finding with self-contradicting
   `missing` list.
2. `ripr check --diff <variant-pinning-repro> --json` shows `error_path` probe
   as `exposed` with `discriminate.state: yes`. No regression.
3. `cargo xtask goldens check` passes (no unexpected drift).
4. `cargo xtask fixtures` passes for all fixtures.
5. `cargo test -p ripr` passes (including new control tests).
6. `cargo clippy -p ripr --all-targets -- -D warnings` clean.
7. `cargo fmt --check` clean under rustfmt 1.9.0-stable (pinned 1.95.0).

## Implementation Mapping

| Behavior | Code location |
|---|---|
| Add ErrorPath to needs_token_confirmation | `crates/ripr/src/analysis/classify/reveal.rs` |
| Control A — repro unit tests | `crates/ripr/src/analysis/classify/reveal.rs::tests` |
| Control B — must-not-over-correct unit tests | `crates/ripr/src/analysis/classify/reveal.rs::tests` |
| Control C — is_effect_family unit test | `crates/ripr/src/analysis/classify/reveal.rs::tests` |
| Control A — repro fixture | `fixtures/error_path_sibling_oracle_fake_clean/` |
| Update classifier.rs integration test | `crates/ripr/src/analysis/classifier.rs::tests` |
| Spec registration | `policy/doc-artifacts.toml`, `docs/specs/README.md` |
| Traceability | `.ripr/traceability.toml` |

## Metrics

- `error_path_variant_confirmation_required`: classification `weakly_exposed`
  fires via `observation_unverified` when no variant-confirming oracle exists.
- `error_path_variant_positive_unchanged`: existing `strong_error_oracle` /
  `weak_error_oracle_assert_matches` / `unwrap_err_variant_positive` fixtures
  still report `exposed` (no regression).
