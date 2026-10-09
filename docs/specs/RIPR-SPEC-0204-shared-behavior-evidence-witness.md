# RIPR-SPEC-0204: Shared behavior-evidence witness adapters and parity corpus

Status: proposed

Owner: product-analysis

Created: 2026-09-29

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #4790 — shared witness adapters and a diff/repo parity corpus (this slice)
- #3160 — parent typed witness kernel campaign
- #3507 — portable `TestEvidenceSummary` projection
- #3402 — `PropagationWitnessV1` adapter
- #4792 — later owner/relation/activation authority migration (not this slice)
- #4793 — later propagation/observation alignment (not this slice)
- #4794 — later duplicate-builder deletion (not this slice)

Linked PRs:

- #4847 — this slice

Support-tier impact:

- None. This spec adds an internal analysis projection and a paired parity
  corpus. It does not promote a language, gate, editor surface, or public
  support claim.
- Claim boundaries remain governed by
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- No new crates, binaries, public schemas, cache generations, network
  allowlist rows, process spawners, or support-tier changes.

## Problem

Rust diff findings and repository seam grip share five-stage vocabulary but
not one comparable evidence record. Canonical gap IDs and output
reconciliation reduce visible drift; they do not prove that both paths
projected the same producer facts. Later slices cannot migrate authority
until there is a shared witness and a discriminating parity denominator.

## Behavior

`analysis::witness` defines one internal `BehaviorEvidenceWitnessV1` and
projects existing `Finding` and `ClassifiedSeam` facts into it.

The witness retains portable item identity, language and family, owner and
normalized expression, required discriminator and expected sink, candidate
versus established test relations, five stage records, selected target or
typed absence, earliest unresolved edge, limitations and non-claims, a
public-class projection, and a semantic digest.

Each stage record retains producer state, basis, source identities,
established versus candidate facts, first unresolved edge, confidence, and
limitations. Adapters copy producer stage meaning; they do not recompute
`ExposureClass`, `SeamGripClass`, actionability, cache generation, or route
readiness.

Missing or unrepresentable source facts become typed adapter limitations.
Candidate-only relations stay candidate and cannot appear as established
reach. The selected-target state is inside the digest: the selected-existing
kind with its portable identities, and the typed-absence kind with its
reason, so a changed target under a stale digest is `not_comparable`.
Absolute checkout spelling, timestamps, traversal order, renderer
prose, line movement, and volatile process identities stay outside the
portable semantic digest.

A deterministic paired parity report compares two witnesses and records
`equal`, `explained_scope_difference`, `contradiction`, or `not_comparable`.
JSON and Markdown derive from the same normalized DTO. Inherent
diff-only versus workspace-complete pairing is the normal cross-path
comparison; it does not by itself explain a discriminator, oracle, or
stage contradiction. Partial index, stale or wrong input, preview language,
and named cross-language limits may explain a difference.

## Required Evidence

- One shared witness can represent both current Rust analysis paths.
- Diff and repo adapters are pure projections over existing producer facts.
- The paired corpus covers the load-bearing families in #4790.
- Removing one source identity or digest makes the row `not_comparable`.
- Reordering source facts yields byte-stable JSON and Markdown.
- Same-name owner or test tokens cannot join unrelated portable identities.
- Public classes on the source `Finding` / `ClassifiedSeam` are unchanged.

## Non-Goals

- no classifier or route behavior change
- no new owner, relation, activation, propagation, observation, oracle, or
  discriminator producer
- no removal of existing stage builders
- no public schema, protocol, cache-generation, support-tier, release, or
  publication change
- no #4792–#4794 authority migration or deletion

## Acceptance Examples

1. Matching equality-boundary facts on both paths, before and after an exact
   test, compare `equal`; crossing before with after is not `equal`.
2. Exact error versus broad error, sibling field, unrelated strong
   assertion, missing observer, sibling match arm, and discriminator
   mismatch compare `contradiction`.
3. Partial-index versus workspace-complete relation sets, stale or wrong
   input on one path, and preview/cross-language limits compare
   `explained_scope_difference`.
4. A producer `reach=yes` backed only by `weak_token_substring` keeps that
   relation in candidate facts and names
   `producer_reach_without_established_relation`.
5. A favorable observation stage paired with an absent stage is not `equal`.

## Test Mapping

- `crates/ripr/src/analysis/witness/tests.rs::adapters_copy_producer_classes_and_do_not_invent_targets_on_findings`
- `crates/ripr/src/analysis/witness/tests.rs::equality_predicate_before_and_after_the_exact_boundary_test`
- `crates/ripr/src/analysis/witness/tests.rs::exact_versus_broad_error_oracle`
- `crates/ripr/src/analysis/witness/tests.rs::exact_field_versus_sibling_field_assertion`
- `crates/ripr/src/analysis/witness/tests.rs::direct_return_versus_unrelated_strong_assertion`
- `crates/ripr/src/analysis/witness/tests.rs::side_effect_with_and_without_aligned_observer`
- `crates/ripr/src/analysis/witness/tests.rs::call_presence_direct_caller_and_unresolved_propagation`
- `crates/ripr/src/analysis/witness/tests.rs::match_arm_sibling_variant_control`
- `crates/ripr/src/analysis/witness/tests.rs::owner_relations_and_wrong_owner_token_collision_do_not_join`
- `crates/ripr/src/analysis/witness/tests.rs::no_test_macro_closure_dispatch_opaque_fixture_and_cross_language_limits`
- `crates/ripr/src/analysis/witness/tests.rs::partial_index_versus_workspace_complete_relation_state`
- `crates/ripr/src/analysis/witness/tests.rs::equivalent_roots_and_harmless_line_movement_keep_digest`
- `crates/ripr/src/analysis/witness/tests.rs::stale_or_wrong_input_on_one_path_is_an_explained_scope_difference`
- `crates/ripr/src/analysis/witness/tests.rs::reordering_source_facts_is_byte_stable`
- `crates/ripr/src/analysis/witness/tests.rs::candidate_only_relation_cannot_appear_as_established_reach`
- `crates/ripr/src/analysis/witness/tests.rs::favorable_stage_plus_absent_stage_cannot_collapse_to_equality`
- `crates/ripr/src/analysis/witness/tests.rs::removing_identity_or_digest_makes_the_row_not_comparable`
- `crates/ripr/src/analysis/witness/tests.rs::changing_selected_target_state_breaks_the_stale_digest`
- `crates/ripr/src/analysis/witness/tests.rs::json_and_markdown_derive_from_the_same_normalized_dto`
- `crates/ripr/src/analysis/witness/tests.rs::unspecified_repo_completeness_is_a_typed_limitation_not_optimistic_parity`
- `crates/ripr/src/analysis/witness/tests.rs::production_retain_does_not_mutate_producer_classes_or_relations`

## Implementation Mapping

- `crates/ripr/src/analysis/witness/mod.rs` — witness record and semantic digest
- `crates/ripr/src/analysis/witness/stage.rs` — stage projection
- `crates/ripr/src/analysis/witness/relations.rs` — candidate versus established split
- `crates/ripr/src/analysis/witness/adapters.rs` — finding and classified-seam adapters plus production retain hooks
- `crates/ripr/src/analysis/language/rust/mod.rs` — diff-path retain (`push_retained_finding`) at the classify call sites
- `crates/ripr/src/analysis/seam_inventory.rs` — repo-path retain after classify

Retain hooks run on release builds too, but until #4792 consumes the
projection the `digest_matches` result has no observable consumer: the
per-finding and per-classified-seam witness build plus SHA-256 digest is an
accepted exercisability cost that keeps the adapter code on the production
path, not a checked invariant and not a liveness proof. Gating it behind
`debug_assertions` would return the adapter to an unexercised debug-only path.
- `crates/ripr/src/analysis/witness/parity.rs` — paired report DTO
- `crates/ripr/src/analysis/witness/tests.rs` — corpus and controls

## Metrics

This slice does not claim a public support-tier or classifier-quality score.
The traceability metric is `unit_test_pass_rate` for the paired corpus.
Internal parity-row counts by disposition are test evidence only.
