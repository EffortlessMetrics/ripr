# RIPR-SPEC-0035: Evidence Quality Benchmark Corpus

Status: proposed

## Problem

Lane 1 evidence-quality improvements should not be driven by aggregate audit
counts alone. Analyzer and calibration changes need a reusable benchmark corpus
that pins the exact evidence class being improved, the claim RIPR should make,
the claim RIPR must not make, and the audit delta expected after the repair.

Without a benchmark corpus, future Lane 1 work can overfit one dogfood file,
collapse distinct gaps into one identity, turn static limitations into user
test gaps, or promote confidence without fixture or runtime proof.

## Behavior

The evidence quality benchmark corpus is a repo-local fixture set for Lane 1
evidence quality. It lives under:

```text
fixtures/evidence-quality-benchmark/
```

The corpus must include a machine-readable manifest, fixture inputs or fixture
references, expected evidence-record subsets, and must-not-claim guards. It is
advisory fixture data for maintainers and agents. It is not a public output
schema, not a user-facing report, and not a generated-test system.

`cargo xtask check-fixture-contracts` validates the corpus shape once the
fixture implementation lands. A valid corpus includes:

- positive cases that demonstrate supported evidence behavior;
- negative cases that prevent overclaiming;
- metamorphic line-movement cases;
- equivalent-code cases;
- must-not-claim guards;
- calibration cases with imported runtime outcomes when available;
- known static limitations;
- before and after audit expectations for audit-driven fixes.

Each case must declare its evidence class and whether it is static-only,
fixture-backed, calibrated, ambiguous, or unsupported in current scope.

## Required Evidence

The benchmark corpus must include fixture classes for:

- duplicate canonical gap;
- match-arm discriminator split;
- wrong related-test top choice;
- broad versus exact error oracle;
- self-computed expected value;
- opaque helper static limitation;
- cross-file constant limitation;
- presentation text constant;
- config and policy constant, including behavior selectors;
- side-effect observer;
- snapshot discriminator;
- mock expectation;
- call-presence assertion affinity;
- runtime-only signal;
- ambiguous runtime join.

Every case must include:

- stable case ID and evidence class;
- source fixture path or fixture reference;
- expected `repo-exposure-json` `seams[].evidence_record` subset;
- expected audit or scorecard signal;
- expected claim;
- `must_not_claim` guards;
- capability or calibration scope, when applicable;
- repair route, when actionable;
- static limitation category, when the current behavior should remain unknown;
- before and after expectation, when a targeted analyzer or calibration repair
  has already landed.

## Inputs

- checked fixture source files or references;
- expected `repo-exposure-json` evidence-record subsets;
- expected Lane 1 audit or scorecard signal fragments;
- optional runtime calibration fixture outcomes;
- capability rows or planned capability rows for class-scoped maturity
  vocabulary.

The corpus may reference existing fixtures when they already pin the relevant
behavior, but the benchmark manifest must still state the Lane 1 evidence class
and must-not-claim guards.

## Outputs

The fixture implementation should add:

```text
fixtures/evidence-quality-benchmark/README.md
fixtures/evidence-quality-benchmark/corpus.json
```

The corpus manifest should include:

- `kind = "lane1_evidence_quality_benchmark_corpus"`;
- `schema_version`;
- `spec = "RIPR-SPEC-0035"`;
- `cases`;
- `evidence_classes`;
- `required_case_kinds`;
- `capability_scope`;
- `calibration_scope`;
- `audit_expectations`.

The validator should report missing classes, missing expected claims, missing
must-not-claim guards, invalid fixture references, and class-specific invariant
violations.

## Non-Goals

- No analyzer behavior changes in the spec or initial corpus-definition slice.
- No report implementation.
- No gate or policy decision.
- No PR or CI projection.
- No LSP or editor output.
- No generated tests.
- No automatic source edits.
- No provider or model calls.
- No mutation execution.
- No capability promotion without separate proof-backed capability updates.

## Acceptance Examples

Given the same match arm moved by a line shift, the benchmark expects the same
canonical gap identity while allowing the raw seam identity or source line to
change.

Given different match arms in the same owner, the benchmark expects different
canonical gap identities and must not allow generic match-arm overgrouping.

Given a runtime-only signal, the benchmark expects the signal to appear in
calibration evidence and must not allow it to create a new static gap.

Given an opaque helper call, the benchmark keeps the case as a static
limitation unless a supported helper pattern is added with positive and
negative fixtures.

Given a self-computed expected value, the benchmark prevents RIPR from treating
that assertion as strong exact-value evidence unless the expected value is
independent of the behavior under test.

Given a changed presentation text constant, the benchmark expects one
evidence-quality item for the declaration and literal, records visibility and
actionability, and prevents RIPR from treating text alone as user test debt or
mutation-testing work.

Given a snapshot oracle with a known discriminating field, the benchmark
distinguishes field-specific observation from broad snapshot output.

Given a call-presence seam whose call expression shares only a generic argument
or local token with an unrelated assertion, the benchmark expects no
assertion-target affinity. Specific call target tokens remain eligible for
affinity.

## Test Mapping

- `xtask::tests::evidence_quality_benchmark_corpus_is_valid` validates the
  checked-in corpus.
- `xtask::tests::evidence_quality_benchmark_requires_all_case_kinds` pins the
  required fixture-class list.
- `xtask::tests::evidence_quality_benchmark_reports_missing_must_not_claims`
  pins negative-guard enforcement.
- `xtask::tests::evidence_quality_benchmark_requires_static_limitation_category_at_case_level`
  pins static-limitation category placement.
- `xtask::tests::evidence_quality_benchmark_keeps_runtime_only_signal_nonstatic`
  pins the runtime-only signal rule.
- `xtask::tests::evidence_quality_benchmark_pins_line_movement_identity`
  validates metamorphic identity cases.
- `rust_index::tests::classifies_only_clear_custom_helpers_as_exact_value_oracles`
  pins the positive and negative custom helper oracle guards.
- `rust_index::tests::classifies_duplicative_equality_as_weak_oracle` and
  `test_grip_evidence::tests::duplicative_equality_assertion_stays_weak_oracle`
  pin the duplicative equality must-not-claim guard.
- `test_grip_evidence::tests::opaque_custom_assertion_helper_stays_unknown_oracle`
  pins the opaque helper static-limitation guard.
- `test_grip_evidence::tests::given_full_evidence_when_owner_call_with_opaque_args_reaches_return_seam_then_activation_is_yes`
  pins value-insensitive owner-call activation without synthetic observed
  values.
- `test_grip_evidence::tests::given_call_presence_when_direct_owner_call_has_mock_expectation_then_activation_is_yes`
  pins fixture-backed call-presence activation for a direct owner call plus an
  explicit mock expectation without synthetic observed values.
- `test_grip_evidence::tests::given_call_presence_when_assertion_mentions_only_generic_argument_token_then_no_affinity`
  pins the negative guard for generic call argument, field, common method,
  enum-field, and match-arm tokens such as `path`, `description`, `is_empty`,
  `variant`, and `arm`.
- `test_grip_evidence::tests::given_call_presence_when_assertion_mentions_short_specific_call_target_then_affinity_remains`
  pins that specific call targets remain eligible for assertion-target
  affinity as medium-confidence relation evidence without satisfying activation
  by themselves. The benchmark records this as
  `activation_owner_call_absent_assertion_target_affinity` routed to
  `analysis/assertion-target-affinity-owner-call-tracing`, not as public test
  debt.
- `test_grip_evidence::tests::given_value_insensitive_seam_when_only_affinity_related_then_activation_names_owner_call_limitation`
  pins the same assertion-target affinity limitation route for value-insensitive
  seams: relation evidence alone stays non-actionable and routes to
  `analysis/assertion-target-affinity-owner-call-tracing`. Comment or string
  mentions of `owner_name(` do not count as owner-call activation.
- `test_grip_evidence::tests::given_full_evidence_when_one_hop_helper_calls_owner_then_value_insensitive_activation_is_yes`
  pins same-file one-hop helper owner-call activation for value-insensitive
  seams without synthetic observed values.
- `test_grip_evidence::tests::given_call_presence_when_same_file_wrapper_directly_calls_owner_then_activation_is_yes`
  pins the `call_presence` same-file direct-wrapper activation sub-shape without
  synthetic observed values.
- `test_grip_evidence::tests::given_call_presence_when_test_local_helper_wraps_owner_call_in_err_then_activation_is_yes`
  pins that `Err(owner(...))` is a safe one-hop helper constructor for
  value-insensitive `call_presence` activation when the helper directly calls
  the owner. The related assertion may still mention the call target, but
  assertion-target affinity alone remains non-actionable without the helper
  owner-call proof.
- `test_grip_evidence::tests::given_full_evidence_when_one_hop_helper_does_not_call_owner_then_activation_stays_unknown`
  pins the helper-name-only must-not-claim guard as
  `activation_owner_call_absent_same_file_only` routed to
  `analysis/same-file-owner-call-tracing`.
- `test_grip_evidence::tests::given_full_evidence_when_generic_helper_name_mentions_owner_then_activation_stays_unknown`
  pins the generic-owner helper guard for names such as `parse`.
- `test_grip_evidence::tests::given_call_presence_when_same_file_wrapper_skips_owner_then_activation_stays_unknown`
  pins that wrapper names cannot activate `call_presence` when the wrapper body
  skips the owner.
- `test_grip_evidence::tests::given_call_presence_when_test_calls_two_hop_wrapper_then_activation_stays_unknown`
  pins that two-hop wrappers remain outside the one-hop activation contract.

## Implementation Mapping

- `fixtures/evidence-quality-benchmark/corpus.json` contains benchmark cases.
- `fixtures/evidence-quality-benchmark/README.md` explains corpus scope,
  evidence classes, and must-not-claim rules.
- `xtask/src/main.rs` validates the corpus through
  `check-fixture-contracts`.
- The future Lane 1 Evidence Quality Leadership tracker records the benchmark
  corpus as the fixture foundation for later analyzer and calibration work when
  that tracker lands.

## Metrics

The benchmark corpus feeds these Lane 1 metrics:

- `lane1_evidence_benchmark_cases`;
- `lane1_evidence_benchmark_positive_cases`;
- `lane1_evidence_benchmark_negative_guards`;
- `lane1_evidence_benchmark_line_movement_cases`;
- `lane1_evidence_benchmark_equivalent_code_cases`;
- `lane1_evidence_benchmark_static_limitation_cases`;
- `lane1_evidence_benchmark_calibration_cases`;
- `lane1_evidence_benchmark_must_not_claim_guards`.

## Validation

The implementation must be pinned by:

- focused xtask unit tests;
- `cargo xtask check-fixture-contracts`;
- `cargo xtask check-static-language`;
- `cargo xtask check-spec-format`;
- `cargo xtask check-traceability`;
- `cargo xtask check-capabilities`;
- `cargo xtask check-pr`.
