# RIPR-SPEC-0143: LSP Client Feature Profile

Status: accepted

Owner: product / swarm

Created: 2026-07-22

Linked issues:

- [#1987](https://github.com/EffortlessMetrics/ripr-swarm/issues/1987) - one
  typed `ClientFeatureProfile` as the initialize-capability authority.

Support-tier impact:

- No tier change. This spec centralizes how client capabilities are parsed
  and disclosed. It does not change any classification, finding set,
  ExposureClass, probe family, confidence score, `repair_packet_ready`
  authority, output schema version, or pass/fail behavior. Client capability
  may weaken projection; it never changes producer actionability, canonical
  identities, or complete evidence.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

Relationship to RIPR-SPEC-0136 / 0138:

- RIPR-SPEC-0136 (configuration pull) and RIPR-SPEC-0138 (CodeLens refresh)
  each negotiated one boolean through an ad-hoc helper and deferred the
  typed profile to #1987. This spec lands that profile: both negotiations
  are now fields on `ClientFeatureProfile`, parsed once at `initialize`.
  Their behaviors are unchanged; only the parse authority moves.

## Problem

Client capabilities were inspected through scattered helper functions and ad
hoc initialization behavior. Position encoding and pull-diagnostic support
were negotiated, while code-action literal/data/disabled/resolve support,
diagnostic optional fields, hover formats, `window/showDocument`, workspace
edits, progress, cancellation, and the RIPR experimental blocks shared no
typed session authority. That made it possible for one surface to emit a
feature another surface believed unsupported, and made support disclosures
difficult to reproduce.

## Behavior

### One typed profile, parsed once

At `initialize` the server builds one immutable `ClientFeatureProfile` from
`InitializeParams` (`crates/ripr/src/lsp/client_features.rs`) and stores it
on the session. Downstream code consumes the profile (or session state
populated from it), never raw `InitializeParams` capability trees and never
client-name checks. The previous ad-hoc negotiation helpers are replaced by
profile fields.

### Captured standard fields

- `general.positionEncodings` (advertised list) and the selected session
  encoding (UTF-16 preferred, then the first advertised encoding the server
  can produce, UTF-16 default);
- `general.staleRequestSupport.cancel` (stale-request cancellation);
- push diagnostics: the `textDocument.publishDiagnostics` block presence
  plus its optional-field flags (relatedInformation, tags, version,
  codeDescription, data);
- `textDocument.diagnostic` pull support (document and workspace pull) and
  related-document support;
- `workspace.diagnostics.refreshSupport` (diagnostic refresh);
- hover content formats in client preference order;
- code-action literal support, the advertised kind value set, `data`,
  `disabled`, `isPreferred`, resolve support with resolvable properties,
  and change-annotation honoring;
- `workspaceEdit.documentChanges` (versioned edits) and change-annotation
  support;
- `window.showDocument` support;
- work-done progress support;
- workspace-folder support;
- `workspace.codeLens.refreshSupport` (RIPR-SPEC-0138);
- `workspace/didChangeWatchedFiles` dynamic registration (required for automatic `ripr.toml`/`Cargo.toml` reload; clients without it must use `ripr: Refresh Full Analysis` after config edits — #2629);
- the session-configuration transport (RIPR-SPEC-0136).

### Captured RIPR experimental blocks

From `capabilities.experimental`:

- `riprEditor`: extension version, client-command list, and the
  `guardedTestEdit` opt-in (absence is `false`);
- `riprAgent`: protocol version (only supported majors parse), preferred
  profiles, and preferred delivery channels from closed vocabularies.

### Fail-closed rules

- Capability absence never implies support: every flag defaults to
  unsupported unless explicitly advertised; absent and explicitly `false`
  are indistinguishable.
- Unknown or malformed optional experimental fields fail closed to
  unsupported: the affected block becomes `None` (unsupported protocol
  major, unknown profile or delivery literal, wrong JSON type, or an
  over-bound string or list). Unknown extra keys are simply not captured.
  A malformed experimental block never breaks the standard session.

### Identity rules

The profile is immutable for the session. Profile equality is the semantic
capability identity: equivalent capability maps with different JSON key
order select equal profiles, and client name, PID, timing, initialization
options, and unrelated initialization fields are never captured, so the
identity changes only when the selected behavior changes. The profile does
not participate in snapshot, input-identity, diagnostic, action, or receipt
identities.

### Bounded status disclosure

The selected profile is exposed as a bounded projection in workspace status
(`analysis_status.client_features`) and generic-client receipts
(`receipt_status.client_features`): typed negotiated values, capped string
lists with omission counts, and `null` for absent experimental blocks. The
raw capability document is never dumped, and client name, PID, and timing
never appear.

A session that cannot store the parsed profile (a poisoned session lock)
fails closed through the blocking-failure channel: the status payload
discloses a `session_state_inconsistent` failure and analysis stays paused
rather than running on a torn negotiation where sibling session fields were
populated from a profile the session no longer holds.

### Editor advertisement

The VS Code extension advertises the `riprEditor` block (extension version,
registered `ripr.*` client commands, `guardedTestEdit: false`) in
`capabilities.experimental` at initialize. It does not opt into guarded
test edits.

## Required Evidence

- `ClientFeatureProfile` in `crates/ripr/src/lsp/client_features.rs` with
  the standard fields and the `riprEditor` / `riprAgent` blocks above,
  parsed exactly once by `Backend::initialize` and stored on the session.
- Table-driven fixtures: minimal standard client; full modern standard
  client; VS Code enhanced client; headless agent client; malformed or
  unknown experimental fields failing closed without breaking the standard
  session; absent versus explicitly `false` capabilities; equivalent
  capability maps with different JSON key order; capability identity
  changing only when the selected behavior changes; guarded-test-edit
  requiring explicit opt-in.
- In-process tests: workspace status and receipt status disclose the
  bounded projection, never leak the client name, and disclose malformed
  experimental blocks as absent while the standard session stays
  negotiated.
- Extension: `capabilities.experimental` advertisement in
  `editors/vscode/src/client.ts` with a controller test asserting the
  block shape.

## Non-Goals

- Implementing every negotiated feature. The profile is the authority;
  feature-specific slices consume it later (hover format selection,
  code-action resolve, workspace edits, guarded test edits).
- Changing any classification, finding set, actionability, canonical
  identity, or evidence content by client capability.
- Advertising agent requests or promoting client support tiers.
- Source edits of any kind; the session stays read-only.
- Raw capability-document dumps in status or receipts.

## Acceptance Examples

### Minimal client gets protocol defaults

```text
Given a client that advertises no capabilities,
when the session initializes,
then every optional feature is unsupported, the encoding is UTF-16,
and the configuration transport is initialization-only.
```

### Malformed experimental blocks fail closed

```text
Given a client whose riprEditor block has a non-string version,
when the session initializes,
then the riprEditor block is unsupported, standard pull-diagnostics
negotiation still applies, and the initialize result is unaffected.
```

### Identity follows behavior, not the client label

```text
Given two initialize handshakes differing only in client name, PID,
and JSON key order,
when the profiles are compared,
then they are equal; a handshake that drops work-done progress support
selects a different profile.
```

### Bounded disclosure

```text
Given any initialized session,
when workspace status or receipt status is requested,
then client_features shows the typed negotiated values with capped lists
and never the raw capability document or the client name.
```

## Test Mapping

- `crates/ripr/src/lsp/client_features.rs::tests::minimal_standard_client_gets_only_protocol_defaults`
  — minimal standard client selects only protocol defaults.
- `crates/ripr/src/lsp/client_features.rs::tests::full_modern_standard_client_enables_every_standard_feature`
  — full modern standard client enables every advertised standard feature.
- `crates/ripr/src/lsp/client_features.rs::tests::vscode_enhanced_client_parses_editor_block_and_ignores_unknown_keys`
  — VS Code enhanced client surfaces the riprEditor block; unknown keys are
  not captured.
- `crates/ripr/src/lsp/client_features.rs::tests::headless_agent_client_parses_agent_preferences`
  — headless agent client surfaces protocol, profile, and delivery
  preferences.
- `crates/ripr/src/lsp/client_features.rs::tests::malformed_or_unknown_experimental_fields_fail_closed_without_breaking_the_session`
  — malformed and unknown experimental fields fail closed; the standard
  session is unaffected.
- `crates/ripr/src/lsp/client_features.rs::tests::absent_and_explicitly_false_capabilities_are_indistinguishable`
  — absence never implies support.
- `crates/ripr/src/lsp/client_features.rs::tests::equivalent_capability_maps_with_different_key_order_select_equal_profiles`
  — JSON key order is not semantic.
- `crates/ripr/src/lsp/client_features.rs::tests::session_capability_identity_changes_only_when_selected_behavior_changes`
  — client name, PID, and initialization options are not semantic; a
  captured capability change is.
- `crates/ripr/src/lsp/client_features.rs::tests::guarded_test_edit_requires_explicit_opt_in`
  — guarded test edits require an explicit boolean opt-in.
- `crates/ripr/src/lsp/client_features.rs::tests::status_projection_is_bounded_and_never_includes_client_identity`
  — the projection caps client-advertised lists and never leaks client
  identity.
- `crates/ripr/src/lsp/client_features.rs::tests::status_projection_caps_every_projected_list`
  — every projected list is capped with a disclosed omission count; short
  lists project unchanged.
- `crates/ripr/src/lsp/client_features.rs::tests::ripr_agent_entries_fail_closed_beyond_the_string_byte_bound`
  — over-bound agent profile/delivery strings fail closed; at-bound strings
  are accepted.
- `crates/ripr/src/lsp/client_features.rs::tests::position_encoding_selection_matches_the_documented_preference`
  — encoding preference order.
- `crates/ripr/src/lsp/tests.rs::initialize_discloses_bounded_client_feature_profile_in_workspace_status`
  — initialize stores the profile; workspace status discloses the bounded
  projection without the client name.
- `crates/ripr/src/lsp/tests.rs::receipt_status_discloses_bounded_client_feature_profile`
  — receipts disclose the bounded projection including agent preferences.
- `crates/ripr/src/lsp/tests.rs::malformed_experimental_blocks_keep_the_standard_session_and_disclose_unsupported`
  — malformed blocks keep the standard session and disclose unsupported.
- `crates/ripr/src/lsp/tests.rs::initialize_surfaces_poisoned_client_features_store_as_a_session_failure`
  — a poisoned profile store surfaces a `session_state_inconsistent`
  blocking failure instead of a silently torn session.
- `editors/vscode/test/suite/extension.test.ts::initialize advertises the RIPR experimental capability block`
  — the extension advertises the riprEditor block.

## Implementation Mapping

- `crates/ripr/src/lsp/client_features.rs` — `ClientFeatureProfile`,
  `RiprEditorClientCapabilities`, `RiprAgentClientPreferences`, the
  fail-closed experimental parsers, and the bounded `status_projection`.
- `crates/ripr/src/lsp/capabilities.rs` — the ad-hoc negotiation helpers
  removed; `initialize_result_for_client` unchanged.
- `crates/ripr/src/lsp/backend.rs` — `client_features` session field,
  single parse at `initialize`, session fields populated from the profile,
  and `client_features` disclosure in analysis status and receipt status.
- `crates/ripr/src/lsp/config.rs` — `LspAnalysisConfig` reads the selected
  encoding from the profile instead of re-negotiating.
- `crates/ripr/src/lsp/agent_protocol.rs` — protocol-version parsing
  shared with the profile.
- `editors/vscode/src/client.ts` — `riprExperimentalClientCapabilities`
  and the initialize-params merge.

## Metrics

- Unit and integration tests listed above pass under `cargo test -p ripr`.
- `cargo xtask goldens check` remains clean: the change is LSP-only and
  touches no CLI goldens.
- `cargo xtask check-static-language` clean: disclosure strings use
  conservative language only.
