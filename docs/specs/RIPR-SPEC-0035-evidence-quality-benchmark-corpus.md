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

### Expected-behavior validity is a separate fixture axis

An optional case-level `semantic_oracle` describes the correctness of a test's
expected behavior. Its closed statuses are `valid`, `invalid`, and
`unreviewed`. Omission on a legacy row means **unreviewed**, never valid.
Malformed reviewed declarations or unsupported statuses reject. An explicit
`unreviewed` declaration makes no acceptance claim; its other metadata is not
validated by this axis. An invalid-oracle negative control can
be valid corpus data; fixture acceptance must preserve its invalid label.

This bounded contract supports a corrected test, the original wrong-sign test,
and a weak variant that removes exactly the two named boundary assertions.
`semantic_oracle.variant` is `corrected` for a valid declaration or `original`
for an invalid declaration. The declaration references `answer_key`,
`native_pairing`, and `independent_review` files by contained relative `path`,
exact `bytes`, and `sha256`,
using the existing retained-fixture file checks. Paths in these files resolve
from the corpus directory. No command is executed during fixture validation.

Reviewed keys declare a contained `production_source_path` separately from the
library target's source path. Every basis entry has a closed `kind`:
`source_document` preserves an opaque cited document, while
`separate_matcher_semantic_witness` identifies the structured retained matcher
record. Missing, null or unknown kinds reject rather than falling back to text.
The typed matcher entry declares its own package/version, library target,
manifest/source paths and `native_test_variant`. That variant identifies the
captured workspace input envelope; it does not claim that the standalone
matcher program executes the primary HIR test.

The answer key binds:

- exact case, package/version, library target, manifest/library source paths,
  full test ID, and original test source path;
- independent expected-behavior claim and retained semantic source artifacts
  with their source URLs;
- fixed/broken production and corrected/original/weak complete test files;
- the two corrected assertions and their original opposite-polarity forms;
- the actual source line of each intended assertion failure.

The validator checks that the weak file differs from the corrected file only
by removing those two assertions, preserving all neighboring assertions. It
rejects identical fixed/broken source, wrong assertion polarity, unresolved
artifacts, wrong case identities, stale hashes, or a failure line that points
to a neighboring assertion. Retained citations and independent review carry
the semantic judgment; the checker does not infer arbitrary domain semantics
or authenticate who authored that judgment.

The separate independent-review record has accepted `disposition`, nonblank
`reviewer` and `rationale`, and an exact `reviewed_subject` object containing
the case ID, declared status, variant, and complete answer-key/native-pairing
file descriptors. Those file hashes transitively bind the claim, citations,
source/test bytes and every retained native capture to the actual judgment.
Refreshing ordinary file hashes cannot carry an old review forward to changed
subjects or a changed verdict. Review covers semantic basis and historical
native capture together. This is a reviewed declaration, not an automated
derivation of semantic truth or an authenticated attestation service.

Valid/invalid acceptance also requires all six exact observed pairings:

| Production | Test | Required native observation |
| --- | --- | --- |
| Fixed | Corrected | Pass |
| Broken | Corrected | Intended corrected-assertion failure |
| Fixed | Weak | Pass |
| Broken | Weak | Pass |
| Fixed | Original | Intended original wrong-sign assertion failure |
| Broken | Original | Pass |

Each observation carries before/after source, test and generated-lock input
digests, exact runner/compiled-artifact hashes, tool versions, resolved final
and intermediate build paths, working directory, command, test identity and
counts. Its retained stdout/stderr must match their hashes. The isolated
Cargo/libtest route selects one exact library test through an absolute Linux
Cargo executable with `test --locked --offline --manifest-path Cargo.toml -p
<package> --lib <test-id> -- --exact`. Intended, discovered, selected and
executed counts are each one. Passed/failed/ignored counts, native exit,
complete libtest terminal line and named test row must agree. Failing controls
must identify the exact named assertion and its source line. Missing, repeated,
zero-subject, ignored, setup-failed, timed-out, compile-failed, process-failed,
stale or wrong-subject observations cannot establish validity.

Each observation also references one compact `retained_native_capture` file.
Its case and complete observation must match the pairing row exactly. It binds
the Cargo-selected package ID, manifest, library target/source, test profile
and executable path to a nonempty artifact's byte count/digest. The retained
executable, exact one-test discovery, and direct frozen
artifact replay must agree with that artifact and the expected native result.
Discovery/replay output is retained and checked by the same file/output
machinery. Missing, swapped, duplicate or malformed capture records, or bare
hash strings without these bindings, reject. Identical executable bytes across
variants are allowed when the independently reviewed capture supports them.
A retained `post_capture_frozen_executable_rehash` record names its observation
time and exact frozen artifact descriptor. It describes a later measurement
of the retained executable, not a contemporaneous measurement of the mutable
compiler-selected path. The record must match the capture's frozen artifact.

Every native capture must retain both full-workspace input inventories. The
native capture owner verifies their bytes and requires equal before/after
arrays with unique contained logical paths, file/symlink kinds, byte counts and
SHA-256 digests. The selected production, complete test and lock entries must
match the reviewed key and pairing; selected manifest/library paths must exist.
When an original parent inventory is declared, the entire footprint must equal
that baseline after only the declared production/test substitutions and lock
insertion. The baseline's `type` is mapped explicitly to capture `kind`; no
other input is discarded. The Regex case keeps all 249 rows, including parent
CHANGELOG and unchanged manifests. A three-row source/test/lock subset is not
an equivalent original workspace. Generic absence of a parent envelope does
not make the capture's before/after fences optional.

The typed matcher retains fixed and broken observations, each with original
and observed program roles. All program, output and input descriptors are
verified. Its input inventories must equal the declared primary native variant;
its separate compiler package/target/manifest/source, linked-library identity,
artifact argv, zero exit and recorded value/output format must agree. Its
nonconsumption citation must identify a declared source document. The checker
validates this recorded consistency, without deriving semantic truth from Rust
program text or executing a matcher. Linked libraries and matcher executables
remain external identity declarations; their current availability and bytes
are not established by this check.

Artifact custody is explicit. `local` custody requires a contained retained
file whose actual bytes match the captured executable; missing/corrupt local
bytes reject without an external fallback. The canonical file path must stay
within the canonical fixture root. In-root file/parent links, root aliases and
root relocation remain valid; outside file/parent links reject even when their
bytes match. This is containment for a stable fixture snapshot; resistance to
concurrent filesystem replacement is outside this contract. `external` custody carries a
nonblank task-evidence locator and no local file claim. Routine fixture checks
validate compact capture/review identities and disclose external executable
bytes as **NOT_REVERIFIED**. They neither require large executables in Git nor
claim to have inspected unavailable bytes. Historical native execution,
independently reviewed capture, current local byte checks, and fresh execution
remain separate facts. Portable replay retains source/tests and declared lock
resolution; it never substitutes old capture for a new run.

The native-pairing file binds the same case and answer-key digest and retains
the exact Cargo.lock artifact. This is a replay lock generated for the stated
toolchain when upstream has no lock; it must not be presented as an original
upstream lock. Exact input and artifact fences plus contrasting behavior are
required when capturing evidence; a Git build stamp alone is insufficient.
Original workspace replay precedes any source/manifest reduction, whose
equivalence needs separate observed evidence.

Declared native-pairing support is checked through the same contained-path,
byte-count and digest authority. `original_workspace` may be absent. When
present it must be an object with `archives`, `inventories` and `provenance`
object maps. Archives and inventories each require `parent` and `fixed`;
provenance requires `parent-git-commit.json`, `fixed-git-commit.json`,
`parent-git-tree.json` and `fixed-git-tree.json`. Every declared map entry,
including additional entries, is verified. Archive payloads are opaque bytes;
inventory payloads must parse as JSON arrays and provenance payloads as JSON
objects. Empty or malformed maps and missing required slots reject.

The top-level `native_packet`, `native_receipt`, `resolution_receipt`,
`offline_setup_receipt` and `capture_checker_interruption` descriptors are
individually optional. Every present descriptor must resolve to the declared
bytes and parse as a JSON object; null is not absence. Future cases need not
invent setup or interruption history. The concrete Regex membership test pins
all thirteen existing support descriptors. Removing or corrupting their files
must reject through the production fixture report even when the pairing and
review bytes are unchanged. Generic declarations without this support metadata
and absent legacy controls retain their existing behavior. These checks do not
extract archives, replay commands, recursively interpret historical paths,
authenticate provenance, or reverify external executable bytes.

`check-fixture-contracts` consumes this axis through the existing benchmark
validator and emits a `PolicyDisclosure` in `fixture-contracts.md` with
valid/invalid/unreviewed, legacy-absent and rejected declaration counts. A
failed declaration is reported as rejected, not silently counted as its claimed
valid status. Missing/unreadable corpus data yields NOT_ESTABLISHED rather
than a successful zero-count disclosure. The disclosure is rendered on both
passing and failing fixture checks.
The report calls valid/invalid labels reviewed expected-behavior declarations,
reports local byte-check and external NOT_REVERIFIED artifact counts, and states
that the fixture check does not rerun tests or authenticate the producer.
These custody totals count accepted observation references/checks, not unique
executable files; two declarations may refer to the same retained binaries.
The totals cover the six primary native test observations per accepted view.
Matcher executable and linked-library references have separate external
NOT_REVERIFIED disclosure and are not silently included in those totals.

This fixture axis does not change static discrimination, `evidence_record`,
Lane 1 audit/scorecard semantics, #3806 judgments, #4795 runtime calibration,
or frozen selection/opportunity denominators. Connecting these separate
consumers remains work for their existing owners. No real upstream case is
accepted solely by this contract extension; source-only proposals and synthetic
unit-test receipt models remain outside the accepted corpus until real native
pairing and independent semantic review exist.

## Required Evidence

### Historical semantic controls outside the static case denominator

The same corpus may carry an optional top-level `semantic_oracle_controls`
array. It contains exact reviewed alternate oracle views of historical cases,
without requiring an `evidence_class`, `expected_repo_exposure` record or a
normative static classification. The existing `cases` collection retains its
static/calibration meaning and unique-ID rule. Controls add no pilot, calibration,
blind-opportunity or frozen-selection membership.

A control is an object with nonblank `id` and `fixture_reference`, a complete
explicit reviewed `semantic_oracle`, and an `observed_static` retained-file
descriptor. Legacy missing-status fallback does not apply to controls. The
closed reviewed variants remain corrected/valid and original/invalid; weak is
only the linked six-pair removal control. Control identity is `(id, variant)`.
Duplicate views reject, and same-ID views must reference identical answer-key,
native-pairing and historical static-capture descriptors. The semantic checker
and exact independent-review subject binding are reused without a new runner.
Every declared control case must retain both successfully reviewed roles.
Omitting either view, substituting a duplicate role, or disagreeing on the
shared subject rejects the case. Complete historical-case counts exclude these
cases; individually reviewed row judgments remain visible alongside an explicit
incomplete-case count. Absence of the whole optional collection retains the
legacy behavior.

The observed static capture is historical data, explicitly marked as having
no normative static expectation. Its retained producer and execution receipts
must agree on source head/tree, selected executable, byte count/digest, command,
root, diff and per-command identity fences. The selected artifact's own head/tree
must also match, and its compiler target must be the `ripr` binary. Three distinct
corrected/original/weak observations bind exact retained stdout/stderr and before/after input
inventories to their actual commands. Each full inventory must equal its
fixed-production native variant inventory and carry the exact fixed source, test and
lock identities. Report root/shape and any retained classification summary
must agree with the captured JSON. Missing or corrupt reports, wrong variants,
stale inputs/producers and conflicting same-case subjects reject. This checks
retained capture consistency, not current executable bytes or analyzer truth.

Declared static `subject_alignment`, `corrected_summary`,
`original_summary_before_correction` and `old_attempt.receipt` attachments must
match their retained bytes and parse as JSON objects. Their historical internal
paths are not live corpus dependencies. The obsolete summary and budget stop
remain history, not current analysis authority. The static production path must
agree with the reviewed key's production path.

The report keeps benchmark-case counts separate from historical control-view
and historical-case counts. Malformed collections are unavailable; rejected
views are not accepted or silently treated as unreviewed. Semantic review does
not accept the newly attached static capture. Both native and static external
executable custody remain NOT_REVERIFIED by the fixture command.

The first locked historical case is rust-lang/regex PR 860's second commit,
parent `72f09f1aeb0ff3f703b1afdbdd21f5ff63162fb4` to fixed
`88a2a62d861d189faae539990f63cb9cf195bd8c`. Its 32/35/33-assertion original,
corrected and weak tests retain all neighbors and the independent matcher
witness. Exact native pairing and expected-behavior judgments are distinct
from the recorded RIPR 68770f7c observation: all three outputs happened to be
identical `exposed` call-deletion findings. That output is not a desired
permanent analyzer behavior. The eight related tests are a capped reported
list; absence means not reported, not internally unselected. The call-omission
counterfactual also differs from the historical argument change, so these
observations establish no deletion-specific false exposure.

Real-corpus tests pin both views, exact upstream and source/test identities,
and the production validation/report route. They do not pin a desired analyzer
classification. The 82 existing static/calibration rows remain unchanged.

Real-case preservation tests also pin README, three license copies, three
auxiliary patches and the curated historical native review note. These eight
files remain required package contents even though ordinary semantic validation
does not use their bytes as proof authority. The complete bounded local graph
has 120 unique referenced files; metadata inventories distinguish those checked
dependencies from documentary preservation and external artifact declarations.

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

- `xtask/src/fixture_contracts/benchmark_oracles/tests.rs` contains synthetic
  contract tests for absent/unreviewed status, corrected/original polarity,
  independent basis, exact weak-test removal, complete pairing, input fences,
  actual subjects, intended failures, retained-file identity, and the production
  fixture reader plus common policy-report disclosure renderer. These modeled
  records are not real upstream runtime receipts.

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
  `variant`, and `arm`, plus argument/context tokens such as `source`,
  `current_owner`, and `out` from full call expressions.
- `test_grip_evidence::tests::given_call_presence_when_assertion_mentions_short_specific_call_target_then_affinity_remains`
  pins that specific call targets remain eligible for assertion-target
  affinity as medium-confidence relation evidence without satisfying activation
  by themselves. The benchmark records this as
  `activation_owner_call_absent_call_presence_target_affinity` routed to
  `analysis/call-presence-target-affinity-owner-call-tracing`, not as public
  test debt.
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
- `test_grip_evidence::tests::given_call_presence_when_integration_test_calls_production_wrapper_then_activation_is_yes`
  pins the `call_presence` production-wrapper activation sub-shape: an
  integration test can call an unambiguous production one-hop wrapper that
  directly calls the owner without inventing synthetic observed values.
- `test_grip_evidence::tests::given_call_presence_when_integration_test_calls_two_hop_production_wrapper_then_activation_is_yes`
  pins the bounded production call-graph sub-shape: an integration test can
  call an unambiguous production wrapper that routes through one same-file
  helper before the owner, without inventing synthetic observed values.
- `test_grip_evidence::tests::given_call_presence_when_two_hop_production_wrapper_reaches_multiple_owners_then_activation_stays_unknown`
  pins the mixed-owner graph guard: a production wrapper graph that reaches
  multiple supported owners stays a static owner-call limitation rather than
  becoming helper-owner-call activation.
- `test_grip_evidence::tests::given_call_presence_when_production_wrapper_calls_same_owner_multiple_times_then_activation_is_yes`
  pins command-builder style production-wrapper activation: a wrapper may call
  the same specific owner helper more than once and still activate a
  value-insensitive seam without inventing synthetic observed values.
- `test_grip_evidence::tests::given_call_presence_when_production_wrapper_calls_multiple_owners_then_activation_stays_unknown`
  pins the mixed-owner guard: production wrappers that call more than one
  supported owner helper do not become helper-owner-call activation.
- `test_grip_evidence::tests::given_call_presence_when_production_wrapper_name_is_ambiguous_then_activation_stays_unknown`
  pins the production-wrapper ambiguity guard: duplicate production helper
  names with different owner-call targets do not become helper-owner-call
  activation.
- `test_grip_evidence::tests::given_call_presence_when_module_qualified_ambiguous_production_wrapper_has_target_affinity_then_activation_is_yes`
  and `test_grip_evidence::tests::given_call_presence_when_module_qualified_wrapper_asserts_other_target_then_activation_stays_unknown`
  pin the qualified-wrapper exception: an explicit `module::wrapper(...)`
  call can disambiguate duplicate production wrapper names for value-insensitive
  `call_presence` target-affinity activation, but only when the related
  assertion mentions the matching call target and no activation values are
  invented.
- `test_grip_evidence::tests::given_call_presence_when_aliased_module_wrapper_has_target_affinity_then_activation_is_yes`
  and `test_grip_evidence::tests::given_call_presence_when_bare_aliased_module_wrapper_has_target_affinity_then_activation_stays_unknown`
  pin the module-alias form of the qualified-wrapper exception: only a
  crate-local module alias such as `use crate::module as alias` can resolve
  `alias::wrapper(...)`; bare or external aliases stay limited even when the
  assertion mentions the matching call target.
- `test_grip_evidence::tests::given_call_presence_when_direct_imported_wrapper_has_target_affinity_then_activation_is_yes`
  and `test_grip_evidence::tests::given_call_presence_when_external_direct_import_matches_local_owner_name_then_activation_stays_unknown`
  pin directly imported owner calls inside production wrappers: an explicit
  crate-local `use crate::module::{owner as alias}` can establish a
  wrapper-to-owner call only when the import path resolves to an indexed module,
  the imported owner name is unambiguous, and the assertion mentions the
  matching call target. External imports and ambiguous imported owner names stay
  limited.
- `test_grip_evidence::tests::given_call_presence_when_unit_test_calls_same_file_target_affinity_wrapper_then_activation_is_yes`
  and `test_grip_evidence::tests::given_call_presence_when_test_local_helper_shadows_target_affinity_wrapper_then_activation_stays_unknown`
  pin the same-source-file unit-test exception: a call that resolves to a
  production wrapper imported from the parent module can use production wrapper
  target affinity, while an unqualified test-local helper shadow does not
  inherit that production relation.
- `test_grip_evidence::tests::given_call_presence_when_test_local_helper_shadows_production_wrapper_then_activation_stays_unknown`
  pins that a test-local helper with the same name as a production wrapper does
  not inherit the production wrapper's owner-call relation.
- `test_grip_evidence::tests::given_call_presence_when_test_local_helper_wraps_owner_call_in_err_then_activation_is_yes`
  pins that `Err(owner(...))` is a safe one-hop helper constructor for
  value-insensitive `call_presence` activation when the helper directly calls
  the owner. The related assertion may still mention the call target, but
  assertion-target affinity alone remains non-actionable without the helper
  owner-call proof.
- `test_grip_evidence::tests::given_call_presence_when_test_local_helper_borrows_owner_call_result_then_activation_is_yes`
  pins a bounded post-owner borrow chain: `owner(...).as_ref().unwrap().clone()`
  can satisfy value-insensitive `call_presence` activation because the helper
  directly evaluates the owner call, while arbitrary post-owner methods remain
  limited.
- `test_grip_evidence::tests::given_full_evidence_when_one_hop_helper_does_not_call_owner_then_activation_stays_unknown`
  pins the helper-name-only must-not-claim guard as
  `activation_owner_call_absent_same_file_only` routed to
  `analysis/same-file-owner-call-tracing`.
- `test_grip_evidence::tests::given_full_evidence_when_generic_helper_name_mentions_owner_then_activation_stays_unknown`
  pins the generic-owner helper guard for names such as `parse`.
- `test_grip_evidence::tests::given_call_presence_when_same_file_wrapper_skips_owner_then_activation_stays_unknown`
  pins that wrapper names cannot activate `call_presence` when the wrapper body
  skips the owner.
- `test_grip_evidence::tests::given_call_presence_when_test_local_two_hop_helper_calls_owner_then_activation_is_yes`
  pins the bounded test-local helper graph sub-shape: a test-local helper can
  route through one same-file helper before the owner for value-insensitive
  `call_presence` activation, without inventing synthetic observed values.

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
