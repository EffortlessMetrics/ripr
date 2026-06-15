# RIPR-SPEC-0113: Honest `no_static_path` Messaging

Status: proposed

Owner: product / swarm

Created: 2026-06-15

Linked issues:

- dtolnay/semver dogfood (found during 0.10.0 first-run-trust campaign)

Linked PRs:

- None yet

Support-tier impact:

- No tier change. This spec changes only the advisory text of the
  `recommended_next_step` field for `no_static_path` findings and the
  all-no-path disclosure Note in human output. It does not change any
  classification, finding-set, ExposureClass, probe family, confidence score,
  or pass/fail authority.
- The message is advisory guidance; changing its wording is not a schema change.
  No schema_version bump is required.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

## Problem

When ripr cannot statically connect a test to a changed owner (because the test
reaches through macros, transitive `pub -> pub(crate)` call chains, or
integration-test patterns), it classifies the finding as `no_static_path` and
previously told the user:

> Add a co-located test that reaches and observes the changed owner so a
> discriminator exists; ripr found no static test path for this change.

This wording implies that **no test exists**. But a test often does exist; ripr
just could not trace it through the static model. A developer who has a macro-
backed or integration test reads "add a test" and infers ripr is broken or
confused, eroding trust on first contact.

The all-no-path disclosure Note (human output) had the same problem:

> Note: ripr found no static test path for any of the {N} changed expression(s)
> in this diff. This is not a coverage assessment — it means no co-located test
> was found that statically discriminates the changed behavior.

"It means no co-located test was found" is too strong: it conflates "ripr's
static model could not trace a path" with "no test exists."

## Behavior

### Per-finding next step (the `recommended_next_step` field)

The text for `ExposureClass::NoStaticPath` becomes:

> ripr found no static test path to this change — this is not a coverage
> assessment. A test may already exercise it through macros, helper-call chains,
> or integration tests that ripr's static model does not yet trace. If none
> does, add a co-located test that reaches and observes the changed behavior so
> a discriminator exists.

The text is defined as the single `pub(crate) const NO_STATIC_PATH_NEXT_STEP`
in `crates/ripr/src/domain/classification.rs`, re-exported through
`crates/ripr/src/domain/mod.rs`. Both the production call site in
`analysis::classify::decision::recommended_next_step` and the unit-test
assertion in `analysis::classifier` reference this single const; the literal is
not duplicated.

### All-no-path disclosure Note (human output)

When all findings in a diff are `no_static_path / infection_unknown /
propagation_unknown / static_unknown`, the footer Note becomes:

> Note: ripr found no static test path for any of the {N} changed expression(s)
> in this diff. This is not a coverage assessment. A test may already exercise
> these changes through macros, helper-call chains, or integration tests that
> ripr's static model does not yet trace; if none does, add co-located tests
> that observe the changed behavior.

### What is NOT changing

- `ExposureClass` values, labels, or severities.
- Classification logic, reach evidence, probe generation, or confidence scoring.
- The condition under which `recommended_next_step` is populated (still:
  `ExposureClass::NoStaticPath` only).
- The condition under which the all-no-path Note fires.
- Any JSON schema field names or types; no schema_version bump.
- Pass/fail exit codes.

## Non-Goals

- Improving reachability — that is the follow-on P3 campaign.
- Changing any classification or exposure class.
- Adding a new disclosure mechanism, field, or output format.

## Acceptance Examples

1. **Macro-backed test**: a test reaches a changed `pub(crate)` fn only via a
   macro. ripr classifies `no_static_path`. The `recommended_next_step` reads
   the honest new text ("A test may already exercise it..."). The all-no-path
   Note reads the honest new Note.
2. **No test at all**: a changed fn with no test at all. ripr classifies
   `no_static_path`. The new text still applies and still advises adding a test,
   but does not assert "no test exists" as a certainty.
3. **Golden fixtures**: all ~33 blessed fixtures that include `no_static_path`
   findings show only message-text changes — no classification, probe, or
   finding-set changes. `cargo xtask goldens check` passes after re-blessing.

## Required Evidence

- `NO_STATIC_PATH_NEXT_STEP` const defined in `domain/classification.rs`,
  re-exported from `domain/mod.rs`.
- Both `analysis::classify::decision` (production) and `analysis::classifier`
  (test) reference the const — not the literal.
- All ~33 golden fixtures re-blessed with `--reason "P2: honest no-static-path
  messaging (RIPR-SPEC-0113)"`.
- `cargo xtask goldens check` clean.
- `cargo xtask fixtures` clean.

## Test Mapping

- `crates/ripr/src/analysis/classifier.rs::tests::recommended_next_step_returns_guidance_by_class`
  — asserts `ExposureClass::NoStaticPath` yields `NO_STATIC_PATH_NEXT_STEP` (shared const)
- `crates/ripr/src/output/human.rs::tests::render_emits_all_no_path_disclosure_when_all_findings_are_no_path`
  — asserts "A test may already exercise these changes through macros" appears in human output
- `crates/ripr/src/output/human.rs::tests::render_emits_all_no_path_disclosure_for_infection_unknown_findings`
  — asserts the Note fires for infection_unknown/static_unknown classes

## Implementation Mapping

| Component | Location |
|---|---|
| `NO_STATIC_PATH_NEXT_STEP` const | `crates/ripr/src/domain/classification.rs` |
| Re-export | `crates/ripr/src/domain/mod.rs` |
| Production call site | `crates/ripr/src/analysis/classify/decision.rs` |
| Test assertion | `crates/ripr/src/analysis/classifier.rs` |
| All-no-path Note | `crates/ripr/src/output/human.rs` |
| Updated test assertion | `crates/ripr/src/output/human.rs` |
| Golden fixtures | `fixtures/*/expected/human.txt` and `fixtures/*/expected/check.json` |

## CI Proof

- `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` — exit 0.
- `cargo test --workspace` — all pass including updated assertions.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- `cargo xtask check-static-language` pass.
- `cargo xtask check-architecture` pass.
- `cargo xtask check-no-panic-family` pass.
- `cargo xtask check-doc-artifacts` pass.
- `cargo xtask check-doc-index` pass.
- `cargo xtask check-spec-format` pass.
- `cargo xtask check-traceability` pass.
- `cargo xtask goldens check` clean.
- `cargo xtask fixtures` clean.

## Metrics

- Gate: 0 golden drift after re-bless; all message-only (no classification
  drift).
- Promote to accepted when behavioral repro shows honest "A test may already
  exercise it..." wording in human and JSON output for a `no_static_path`
  finding.
