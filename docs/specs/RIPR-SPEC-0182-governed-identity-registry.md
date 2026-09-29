# RIPR-SPEC-0182: Governed identity registry and compatibility map

Status: proposed

Owner: product-swarm

Created: 2026-09-29

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #4804 — ID01 taxonomy and compatibility foundation
- #1932 — parent identity taxonomy
- #1907 — programme
- #4805–#4809 — typed wrappers and consumer migrations (not this spec)

Linked PRs:

Support-tier impact:

- None. This spec records existing identifier authorities and their
  compatibility posture. It does not change support-tier labels, actionability,
  currentness, transport behavior, or permission to execute project tests.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- Adds `cargo xtask check-identity-registry` (`ci_enforced=true`, precommit).
- Registers generated `docs/identity/REGISTRY.md` and
  `docs/identity/registry.v1.json`.
- No new crates, binaries, dependencies, network allowlist rows, process
  spawners, LSP/MCP write tools, or support-tier changes.

## Problem

RIPR already has several identifier authorities (`LspAnalysisInputIdentity`,
`RepairAttemptId`, diagnostic result ids, command ids, canonical gap identity,
agent `snapshot_id`). Without one checked registry, later typed migrations can
invent a second owner, treat a compatibility alias as authority, or collapse
`RepairAttemptId` into an analysis-attempt or snapshot identity.

## Behavior

1. One machine-readable registry and one generated human relationship table
   agree exactly. Both are projections of
   `crates/ripr/src/domain/identity`. Domain does not import `serde_json`.
2. Every #1932 taxonomy identity has an owner, portability class, invalidation
   law, and serialization posture. Serialized siblings that already exist
   (`SeamLocationId`, `FindingId`, `DiagnosticCodeId`, `FeedbackReceiptId`)
   keep distinct rows so every governed field has a disposition.
3. Existing concrete authorities are recorded rather than wrapped again.
   Competing wrappers remain visible.
4. Compatibility aliases have an explicit removal generation. They grant no
   authority beyond the canonical field.
5. Compile-time consumers look up `identity_field_disposition(surface, field)`
   without parsing prose. Lookup matches the registered surface path exactly,
   or an observed path that ends with a separator plus that registered path.
   A truncated suffix does not inherit a disposition. Unknown governed
   `*_id` / `*_identity` fields have no disposition and fail
   `cargo xtask check-identity-registry`.
6. Agent protocol `snapshot_id` remains the refresh-generation
   `AnalysisAttemptId`. It is not `#1602` `CompletedAnalysisSnapshotId`.
7. `RepairAttemptId` cannot be registered as a parent, child, or field alias of
   `AnalysisAttemptId` or `CompletedAnalysisSnapshotId`.
8. Portable identities reject scheduler generation, timestamp, PID, client
   name, display formatting, and absolute checkout spelling as semantic
   inputs. Equivalent checkout roots preserve portable ids while containment
   (`root_identity`) stays a separate `InputIdentity` component.
9. Reordered registry entries render byte-stable JSON and Markdown.
10. This slice does not migrate consumers, mint replacement types owned by
    #4805–#4809, or change product actionability, currentness, transport, or
    support claims.

## Required Evidence

- Production catalog covers the thirteen #1932 taxonomy kinds and is internally
  consistent.
- Discriminating negatives: missing taxonomy, duplicate owners, two canonical
  owners for one surface field, `RepairAttemptId` colliding with analysis or
  snapshot identity, completed snapshot owning agent `snapshot_id`, portable
  forbidden inputs, alias without removal generation, unknown governed field,
  one-sided parent/child edge, adjacent field also canonical, ungoverned
  serialization surface.
- Same field name on different surfaces keeps distinct meanings (`attempt_id`
  on repair vs feedback; `receipt_id` on feedback vs repair). Truncated
  surface suffixes do not inherit a disposition. The same surface field cannot
  have two owners across canonical, component, alias, or adjacent claims.
- `continuation_id` / `continuation_identity` are one authority with an alias.
- Agent `snapshot_id` disposition is `AnalysisAttemptId`; check
  `snapshot_identity` is `CompletedAnalysisSnapshotId`.
- Reordered identities and adjacent fields render byte-stable JSON/Markdown.
- `cargo xtask check-identity-registry` is network-free and fail-closed on
  unknown governed fields.

## Non-Goals

- No broad DTO migration.
- No new LSP/MCP request or custom protocol.
- No action/edit/command execution.
- No replacement of #1765, #1617, #1899, #2927, or #1665 authority.
- No typed wrappers owned by #4805–#4809.
- No product actionability, currentness, transport, or support-claim changes.
- No release, publication, credential, or milestone mutation.

## Acceptance Examples

1. Given the production catalog, when `identity_registry_violations()` runs,
   then it returns an empty list and the generated Markdown/JSON name every
   required taxonomy identity.
2. Given two records that both claim canonical `snapshot_id` on the agent
   request schema, when the registry check runs, then it reports contradictory
   canonical owners.
3. Given `RepairAttemptId` sharing `snapshot_id` with `AnalysisAttemptId`, when
   the registry check runs, then it fails rather than reconciling by prose.
4. Given a new `brand_new_widget_id` on a governed surface, when
   `identity_field_disposition` is queried, then it returns `None` and the
   xtask scanner reports an unknown field.
5. Given the production catalog reversed, when JSON and Markdown are rendered,
   then the bytes match the forward order.

## Test Mapping

- `crates/ripr/src/domain/identity/tests.rs::production_registry_covers_required_taxonomy_and_is_internally_consistent`
- `crates/ripr/src/domain/identity/tests.rs::missing_taxonomy_identity_fails_the_registry_check`
- `crates/ripr/src/domain/identity/tests.rs::contradictory_duplicate_owners_fail_the_registry_check`
- `crates/ripr/src/domain/identity/tests.rs::same_surface_field_cannot_have_two_canonical_owners`
- `crates/ripr/src/domain/identity/tests.rs::same_field_name_on_different_surfaces_can_keep_distinct_meanings`
- `crates/ripr/src/domain/identity/tests.rs::repair_attempt_id_cannot_be_registered_as_a_snapshot_or_analysis_attempt_alias`
- `crates/ripr/src/domain/identity/tests.rs::completed_snapshot_cannot_own_agent_snapshot_id`
- `crates/ripr/src/domain/identity/tests.rs::production_catalog_keeps_agent_snapshot_id_as_analysis_attempt_not_completed_snapshot`
- `crates/ripr/src/domain/identity/tests.rs::continuation_wire_names_are_one_authority_with_an_alias`
- `crates/ripr/src/domain/identity/tests.rs::portable_identities_reject_scheduler_timestamp_pid_client_and_display_inputs`
- `crates/ripr/src/domain/identity/tests.rs::equivalent_checkout_roots_preserve_portable_identities_and_keep_containment_separate`
- `crates/ripr/src/domain/identity/tests.rs::alias_without_removal_generation_fails`
- `crates/ripr/src/domain/identity/tests.rs::unknown_governed_field_has_no_disposition`
- `crates/ripr/src/domain/identity/tests.rs::reordered_registry_entries_render_byte_stable_json_and_markdown`
- `crates/ripr/src/domain/identity/tests.rs::one_sided_parent_child_edge_fails_the_registry_check`
- `crates/ripr/src/domain/identity/tests.rs::adjacent_field_cannot_also_be_canonical_on_the_same_surface`
- `crates/ripr/src/domain/identity/tests.rs::serialization_on_an_ungoverned_surface_fails_closed`
- `crates/ripr/src/domain/identity/tests.rs::truncated_surface_suffix_does_not_inherit_a_disposition`
- `crates/ripr/src/domain/identity/tests.rs::checkout_prefixed_surface_still_matches_the_registered_path`
- `crates/ripr/src/domain/identity/tests.rs::same_surface_field_cannot_be_canonical_and_component`
- `crates/ripr/src/domain/identity/tests.rs::missing_serialized_sibling_fails_the_registry_check`
- `xtask/src/identity_registry.rs::tests::unknown_field_has_no_disposition`
- `xtask/src/identity_registry.rs::tests::same_attempt_id_name_keeps_repair_and_feedback_dispositions_apart`
- `xtask/src/identity_registry.rs::tests::scanner_reports_unknown_and_zero_field_surfaces`

## Implementation Mapping

- `crates/ripr/src/domain/identity/` — catalog, invariants, byte-stable renderers
- `xtask/src/identity_registry.rs` — `check-identity-registry`
- `docs/identity/REGISTRY.md` — generated human table
- `docs/identity/registry.v1.json` — generated machine-readable registry

## Metrics

- `identity_registry_kinds` — count of catalogued identities, including required
  taxonomy and serialized siblings. Advisory vocabulary coverage only. Not
  consumer-migration completeness, actionability, or support-tier evidence.
