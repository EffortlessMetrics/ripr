# RIPR-SPEC-0021: Evidence Record

Status: proposed

## Problem

RIPR has several advisory consumers of seam evidence: repo exposure, RIPR Zero
status, agent packets, before/after movement, PR ledgers, assistant proof
reports, editor status, and gates. Those consumers should not reconstruct seam
identity, missing discriminators, related tests, recommendation shape, static
limitations, or calibration context independently.

Lane 1 needs one seam-native evidence projection that downstream reports can
consume without changing analyzer truth.

## Product Contract

The evidence record is an additive projection over existing static analyzer
facts. It must not:

- change seam classification;
- create a gate decision;
- mutate a baseline;
- post comments;
- edit source;
- generate tests;
- call a provider;
- run mutation testing.

The record preserves conservative static language. Runtime mutation data, when
supplied by later calibration work, is confidence context only.

## Behavior

The canonical behavior is:

```text
ClassifiedSeam
-> seam-native evidence_record
-> additive repo-exposure JSON field
-> downstream consumers can read one shared shape
```

The projection must copy existing analyzer facts without changing them. Unknown
or opaque stages must be explicit static limitations. Missing runtime
calibration must remain `no_runtime_data`.

## Required Evidence

The first implementation uses only existing static inputs:

| Evidence | Source |
| --- | --- |
| Seam identity, owner, location, kind | `RepoSeam` |
| Grip class and headline eligibility | `ClassifiedSeam` |
| Reach, activate, propagate, observe, discriminate stages | `TestGripEvidence` |
| Observed values | `ValueFact` |
| Missing discriminators and flow sinks | `MissingDiscriminatorFact` |
| Related tests and oracle strength | `RelatedTestGrip` |
| Recommended test, candidate value, assertion shape | Existing agent seam packet helpers |

The projection must not read hidden artifacts or rerun analysis.

## Output Location

Repo exposure JSON includes the record under each seam:

```json
{
  "seams": [
    {
      "seam_id": "f3c9e4d21a0b7c88",
      "evidence_record": {
        "schema_version": "0.1"
      }
    }
  ]
}
```

Repo exposure keeps existing top-level seam fields for compatibility. The
record is additive in repo exposure schema `0.3`.

## Required Fields

Each `seams[].evidence_record` must include:

- `schema_version`: evidence record schema version, currently `"0.1"`.
- `seam_id`: the seam identity copied from the containing seam.
- `canonical_gap_id`: generated canonical behavioral gap identity for
  headline-eligible gap classes, or `null` when the seam is not canonical
  behavioral debt.
- `canonical_gap_group_size`: number of raw seams in the current repo-exposure
  snapshot that share the same canonical gap identity, or `null`.
- `canonical_gap_reason`: the deterministic grouping reason, or `null`.
- `owner`: owner symbol copied from the seam.
- `location.file` and `location.line`: source locator fields.
- `seam_kind`: seam kind copied from the seam.
- `grip_class`: seam grip class copied from current classification.
- `headline_eligible`: current headline eligibility.
- `evidence_path.reach`, `activate`, `propagate`, `observe`, and
  `discriminate`: typed stage records with `state`, `confidence`, and
  `summary`.
- `observed_values`: structured observed activation values.
- `missing_discriminators`: structured missing discriminator facts and optional
  flow sink context.
- `related_tests_total` and `related_tests`: ranked related-test evidence.
- `related_tests[].oracle_semantics`: structured `observes`, `missing`, and
  nullable `upgrade_suggestion` strings explaining the related test's oracle
  shape under the seam kind.
- `recommendation`: bounded test-intent guidance derived from existing
  evidence.
- `actionability`: advisory actionability class and available guidance signals.
- `calibration`: static/runtime confidence placeholder.
- `static_limitations`: unknown or opaque static evidence stages. Each entry
  keeps the original reason and adds a normalized analyzer limitation
  `category` plus `repair_route` so Lane 1 can group repair work without
  converting analyzer limits into user test gaps.

## Actionability Vocabulary

`actionability.class` must be one of:

- `actionable_focused_test`
- `actionable_assertion_upgrade`
- `actionable_related_test_extension`
- `needs_human_design`
- `static_limitation`
- `not_policy_relevant`

These classes are advisory and do not change policy, baselines, suppressions, or
gate authority.

## Calibration Placeholder

Before static/runtime calibration labels are implemented, the record must carry:

```json
{
  "calibration": {
    "availability": "not_imported",
    "confidence": "unknown",
    "agreement": "no_runtime_data"
  }
}
```

`no_runtime_data` means no imported runtime calibration was supplied. It does
not confirm or reject static evidence.

## Backward Compatibility

Consumers that already read repo exposure may continue to use existing fields:

- `seam_id`
- `kind`
- `file`
- `line`
- `owner`
- `expression`
- `grip_class`
- `headline_eligible`
- `evidence`
- `related_tests_total`
- `related_tests`
- `observed_values`
- `missing_discriminators`

The first implementation kept the record additive. Follow-up consumer slices
may read the record as an additive source of truth while preserving legacy
fields as fallback.

## Canonical Gap Identity

Canonical gap identity is distinct from `seam_id`.

`seam_id` stays source-location-sensitive so before/after movement can track a
specific seam. `canonical_gap_id` ignores locators and groups headline-eligible
gap records by:

```text
owner symbol
+ seam kind
+ flow sink kind
+ missing discriminator
+ assertion shape
```

The ID is deterministic across runs and line movement. Different missing
discriminators or duplicate function names in different modules produce
different IDs. Strong, opaque, intentional, and suppressed seams keep
`canonical_gap_id: null` because they are not actionable canonical behavioral
debt under this record.

## Related-Test Ranking

The record copies ranked related-test evidence from repo exposure. Ranking is
deterministic and uses:

1. relation confidence;
2. relation reason priority;
3. oracle strength;
4. activation-value overlap;
5. file, name, and line.

The rank affects the capped `related_tests` array and the nearest related test
to imitate. It must not change `related_tests_total`, mint a new seam identity,
or turn a weak relationship into a strong one. Activation overlap is a static
tie-breaker from values RIPR already observes, for example a direct owner call
whose predicate-boundary arguments are equal at the changed boundary.

## Oracle Semantics

Related-test oracle semantics are an evidence explanation, not a new
classification. They make the existing `oracle_kind` and `oracle_strength`
actionable by naming:

- what the oracle observes;
- what discriminator remains missing;
- what assertion upgrade would improve the seam's behavioral grip, when an
  upgrade is known.

Broad error assertions such as `assert!(result.is_err())` observe that some
error occurred, but they do not discriminate the exact error variant or payload.
Smoke-only assertions such as unwrap/expect/ok-shape checks observe that the
call completed or returned a broad shape, but they do not observe the output
value, error variant, field, effect, or call discriminator. Exact-value
assertions do not get an upgrade suggestion merely because they are exact in
the supported static scope.

## Consumer Routing

The first consumer slice routes two existing advisory surfaces through the
shared record without changing analyzer behavior:

- Agent seam packets carry `packets[].evidence_record` next to the existing
  top-level work-order fields. The packet's legacy fields stay present so
  coding agents and editor consumers do not need an immediate migration.
- RIPR Zero status repair routes prefer `evidence_record` when a baseline debt
  delta item supplies it. The route may copy the record's location, grip class,
  missing discriminator, related test, assertion shape, verification command,
  and static limitations. Legacy baseline-delta fields remain fallback.
- Targeted-test outcome and agent verify compare before/after
  `evidence_record` fields when present. Stage movement, observed-value
  movement, missing-discriminator movement, oracle strength movement,
  related-test movement, and no-movement reasons are emitted additively while
  legacy repo-exposure fields remain fallback.
- Test-oracle assistant proof prefers `evidence_record` when the selected agent
  packet or matching repo-exposure seam supplies it. The proof may copy the
  record's seam identity, canonical gap ID, owner/location, grip class, missing
  discriminator, static limitations, related test, assertion shape,
  verification command, and before/after movement classes while preserving
  legacy fields as fallback.
- Baseline ledgers and PR evidence ledgers carry `canonical_gap_id` when source
  artifacts supply it directly, under `identity.canonical_gap_id`, or under
  `evidence_record.canonical_gap_id`. Baseline diff and shrink-only update use
  canonical gap identity before seam/source/id/dedupe/fallback matching, while
  PR ledger records copy it into waiver, suppression, receipt, and top repair
  route records.

This routing must not invent commands, generate tests, change gate authority,
or mutate baselines.

## Acceptance Examples

- `repo-exposure.json` includes `seams[].evidence_record`.
- Existing repo exposure fields remain present.
- Headline-eligible gap records carry generated `canonical_gap_id`,
  `canonical_gap_group_size`, and `canonical_gap_reason`.
- Line movement does not change canonical identity when owner, seam kind, flow
  sink, missing discriminator, and assertion shape are unchanged.
- Different missing discriminators and duplicate function names in different
  modules do not collide.
- Agent seam packets include `evidence_record` while preserving existing
  top-level work-order fields.
- RIPR Zero status repair routes prefer supplied `evidence_record` guidance and
  static limitations while preserving legacy fallback fields.
- Targeted-test outcome and agent verify prefer `evidence_record` movement
  fields while preserving legacy fallback fields and existing movement buckets.
- Test-oracle assistant proof prefers `evidence_record` selected-seam,
  recommendation, static-limit, and movement fields while preserving legacy
  fallback fields and existing advisory proof boundaries.
- Baseline create, diff, and shrink-only update preserve canonical behavioral
  gap identity when supplied while preserving older ledgers that lack it.
- PR evidence ledger copies canonical gap identity into identity-bearing
  waiver, suppression, receipt, and top repair route records when supplied.
- Evidence record schema `0.1` is documented in `docs/OUTPUT_SCHEMA.md`.
- Related tests include oracle semantics explaining what broad, smoke-only,
  unknown, and exact oracle shapes observe and miss without changing their
  oracle kind or strength.
- `fixtures/boundary_gap/expected/evidence-record-contract/corpus.json` pins
  representative v0.1 records for the supported contract matrix.
- Unit tests pin identity, grip class, evidence path, recommendation,
  actionability, calibration placeholder, and static limitations.
- No analyzer behavior changes.
- No gate, policy, LSP, editor, first-useful-action, movement, assistant proof,
  or baseline mutation behavior changes.

Additional examples:

- A weakly gripped predicate boundary carries the missing equality
  discriminator, candidate value, recommended assertion shape, and verify
  command.
- An activation-unknown seam carries `static_limitations[]` and does not claim
  concrete focused-test guidance.
- An opaque seam carries a classification-level static limitation.
- A supplied canonical gap identity remains a record field that baseline and PR
  ledger consumers may carry without turning baseline state into analyzer truth.
- Current v0.1 calibration records use `no_runtime_data`. Imported
  static/runtime confidence labels live in mutation-calibration reports and do
  not enter the shared record until a future schema revision explicitly adds
  runtime-backed calibration context.

## Test Mapping

| Behavior | Test |
| --- | --- |
| Record carries identity, evidence path, recommendation, actionability, and calibration placeholder | `evidence_record_carries_identity_path_guidance_and_calibration_placeholder` |
| Unknown stages become static limitations | `evidence_record_names_static_limitations_from_unknown_stages` |
| Opaque classification is static limitation work | `evidence_record_marks_opaque_seams_as_static_limitation_work` |
| Repo exposure schema and metrics remain present | `json_carries_schema_version_scope_and_metrics` |
| Repo exposure carries existing seam fields plus the new record | `json_carries_full_classified_record` |
| Agent seam packets carry the shared record while preserving legacy fields | `packet_carries_shared_evidence_record_projection` |
| RIPR Zero status repair routes prefer supplied record guidance | `ripr_zero_status_prefers_evidence_record_repair_context` |
| Targeted-test outcome prefers record-level before/after movement | `targeted_test_outcome_prefers_evidence_record_movement` |
| Targeted-test outcome names unchanged record movement reason | `targeted_test_outcome_records_no_movement_reason` |
| Test-oracle assistant proof prefers agent packet evidence records | `test_oracle_assistant_proof_prefers_agent_packet_evidence_record` |
| Test-oracle assistant proof prefers repo-exposure evidence records for movement | `test_oracle_assistant_proof_prefers_repo_exposure_evidence_record_movement` |
| Baseline create copies supplied canonical gap identity | `baseline_create_uses_canonical_gap_identity_when_supplied` |
| Baseline diff matches moved lines by canonical gap identity | `baseline_delta_matches_by_canonical_gap_id_across_line_movement` |
| Baseline update preserves refactored entries matched by canonical gap identity | `baseline_update_preserves_refactored_entry_matched_by_canonical_gap_id` |
| PR evidence ledger carries canonical gap identity through joined records | `pr_evidence_ledger_joins_primary_artifacts` |
| Generated canonical identity is stable across line movement | `canonical_gap_id_is_stable_across_line_movement` |
| Generated canonical identity changes with the missing discriminator | `canonical_gap_id_changes_when_missing_discriminator_changes` |
| Generated canonical identity groups equivalent raw seams | `canonical_gap_identities_report_group_size_for_equivalent_gaps` |
| Related-test ranking prefers strong oracle imitation targets inside the same relation | `given_related_tests_with_same_relation_when_ranked_then_strong_oracle_precedes_smoke_oracle` |
| Related-test ranking uses activation overlap before file ordering inside the same relation and oracle strength | `given_related_tests_with_same_relation_and_oracle_when_ranked_then_activation_overlap_precedes_file_order` |
| Fixture contract corpus pins representative record shapes | `evidence_record_contract_fixture_corpus_is_valid` |
| Fixture contract checker reports record-shape drift | `evidence_record_contract_fixture_guard_reports_missing_fields` |
| Fixture contract checker requires oracle semantics shape | `evidence_record_contract_fixture_guard_requires_oracle_semantics` |
| Broad error oracle semantics explain the exact error discriminator gap | `oracle_semantics_explains_broad_error_gap_and_upgrade` |
| Smoke-only oracle semantics explain the missing boundary discriminator | `oracle_semantics_explains_smoke_only_boundary_gap` |
| Exact-value oracle semantics do not invent an upgrade suggestion | `oracle_semantics_keeps_exact_value_without_extra_upgrade` |
| Oracle semantics cover the supported oracle families | `oracle_semantics_covers_supported_oracle_families` |
| Opaque custom assertion helpers stay unknown instead of overclaiming exact-value grip | `opaque_custom_assertion_helper_stays_unknown_oracle` |
| Duplicative equality assertions stay weak instead of overclaiming exact-value grip | `duplicative_equality_assertion_stays_weak_oracle` |
| Evidence records carry oracle semantics on related tests | `evidence_record_carries_identity_path_guidance_and_calibration_placeholder` |

## Implementation Mapping

| Surface | File |
| --- | --- |
| Evidence record projection | `crates/ripr/src/output/evidence_record.rs` |
| Repo exposure JSON attachment | `crates/ripr/src/output/repo_exposure.rs` |
| Agent seam packet projection | `crates/ripr/src/output/agent_seam_packets.rs` |
| Targeted-test outcome movement | `crates/ripr/src/output/outcome.rs` |
| RIPR Zero status repair route consumer | `crates/ripr/src/output/ripr_zero_status.rs` |
| Test-oracle assistant proof consumer | `crates/ripr/src/output/test_oracle_assistant_proof.rs` |
| Baseline ledger canonical identity consumer | `crates/ripr/src/output/baseline.rs`, `crates/ripr/src/output/baseline_delta.rs`, `crates/ripr/src/output/baseline_update.rs` |
| PR evidence ledger canonical identity consumer | `crates/ripr/src/output/pr_evidence_ledger.rs` |
| Output module registration | `crates/ripr/src/output/mod.rs` |
| Evidence record fixture contract | `fixtures/boundary_gap/expected/evidence-record-contract/corpus.json`, `xtask/src/main.rs` |
| Schema reference | `docs/OUTPUT_SCHEMA.md` |
| Capability tracking | `docs/CAPABILITY_MATRIX.md`, `metrics/capabilities.toml` |
| Traceability | `.ripr/traceability.toml` |

## Metrics

The capability metric labels are:

- `evidence_record_projected_seams`
- `evidence_record_actionable_guidance`
- `evidence_record_static_limitations`

These are tracking labels for capability maturity. This PR does not add a
runtime metric emitter.

## Non-Goals

- No analyzer behavior changes.
- No gate or policy changes.
- No LSP or editor changes.
- No first-useful-action docs, dogfood, or closeout work.
- No RIPR Zero gate or baseline mutation changes.
- No further evidence movement routing changes.
- No baseline mutation or PR policy changes.
- No mutation execution.
