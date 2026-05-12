# RIPR-SPEC-0032: Lane 1 Evidence Quality Failure Fixtures

Status: proposed

## Problem

The Lane 1 evidence-quality audit identifies dogfood gaps, but analyzer work
should not start from aggregate counts alone. Future changes need fixture-pinned
examples that say what `ripr` should claim, what remains unknown, and what must
not be inferred.

## Product Contract

Lane 1 evidence-quality failure fixtures live at:

```text
fixtures/boundary_gap/expected/evidence-quality-failures/corpus.json
```

The corpus is advisory fixture data for maintainers and agents. It is not a
public output schema and does not add a new user-facing report.

Each case records:

- the audit signal that made the case worth pinning;
- the expected `repo-exposure-json` `seams[].evidence_record` subset;
- the claim RIPR should make in the current syntax-first scope;
- the claims RIPR must not make.

The corpus must include both failure-mode cases and negative guards.

## Behavior

`cargo xtask check-fixture-contracts` validates the corpus shape. A valid corpus
has:

- `kind = "lane1_evidence_quality_failure_corpus"`;
- `schema_version = "0.1"`;
- `spec = "RIPR-SPEC-0032"`;
- a source audit report summary;
- required cases for duplicate canonical groups, missing discriminators, static
  limitations, side-effect or mock observer semantics, and calibration gaps;
- non-empty `expected_claims` and `must_not_claim` entries for each case.

The expected `evidence_record` subset pins stable fields that matter for audit
driven work: identity, owner, seam kind, grip class, headline eligibility,
evidence path states, counts, top related test, recommendation action,
actionability class, calibration state, and static limitations. After an
audit-driven analyzer improvement lands, the same case may keep the original
audit signal while updating the expected record to pin the corrected behavior
and guard against regression.

## Required Evidence

The corpus must pin audit-derived examples for:

- duplicate-looking canonical groups and their corrected post-fix behavior when
  an audit-driven fix has landed;
- missing equality-boundary discriminators;
- activation static limitations with no observed values;
- side-effect or mock observer semantics;
- no-runtime-data calibration gaps.

Every case must include:

- the audit metric path and observed count;
- an expected `repo-exposure-json` `evidence_record` subset;
- at least one expected claim;
- at least one claim RIPR must not make.

## Acceptance Examples

Given a duplicate-looking canonical group from the audit, the fixture records
the original audit signal and the current expected canonical behavior. Before a
fix, that may be a canonical gap ID and group size greater than one. After a
fix, it may be a concrete discriminator and group size `1`, with a
`must_not_claim` guard against returning to generic identity.

Given a missing equality-boundary discriminator, the fixture records the missing
discriminator count, concrete guidance, and nearest related test, and must not
allow nearby exact-value tests to be treated as covering the missing boundary.

Given an activation static limitation, the fixture records unknown activation
and a static limitation, and must not allow future work to invent observed
values from a call expression.

Given a mock expectation or side-effect observer signal, the fixture keeps the
recognized oracle semantics distinct from unrelated activation limitations.

Given no imported runtime data, the fixture records `not_imported` and
`no_runtime_data`, and must not allow static evidence to be described as
calibrated.

## Test Mapping

- `xtask::tests::lane1_evidence_quality_failure_corpus_is_valid` validates the
  checked-in corpus.
- `xtask::tests::lane1_evidence_quality_failure_guard_reports_contract_drift`
  pins required fields, case kinds, audit signals, positive and negative case
  coverage, and case-specific invariants.
- `cargo xtask check-fixture-contracts` runs the validator with the normal
  fixture contract gate.

## Implementation Mapping

- `fixtures/boundary_gap/expected/evidence-quality-failures/corpus.json`
  contains the audit-derived fixture cases.
- `fixtures/boundary_gap/expected/evidence-quality-failures/README.md`
  explains the corpus boundary.
- `xtask/src/main.rs` validates the corpus through
  `check-fixture-contracts`.

## Non-Goals

- No analyzer behavior changes in the initial fixture-pinning slice. Later
  audit-driven analyzer slices may update the expected records to pin corrected
  behavior.
- No gate or policy decision.
- No PR or CI projection.
- No evidence-health field folding.
- No LSP, editor, provider, release, dependency, or platform work.
- No mutation execution.
- No generated tests.

## Metrics

The fixture corpus is driven by the Lane 1 audit metrics:

- `lane1_evidence_audit_duplicate_looking_groups`;
- `lane1_evidence_audit_missing_discriminators`;
- `lane1_evidence_audit_static_limitations`;
- `lane1_evidence_audit_uncalibrated_records`.

## Validation

The implementation is pinned by:

- focused xtask unit tests;
- `cargo xtask check-fixture-contracts`;
- `cargo xtask check-static-language`;
- `cargo xtask check-traceability`;
- `cargo xtask check-capabilities`;
- `cargo xtask check-pr`.
