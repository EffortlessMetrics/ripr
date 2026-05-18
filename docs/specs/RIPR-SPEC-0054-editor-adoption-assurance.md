# RIPR-SPEC-0054: Editor Adoption Assurance

Status: proposed

## Problem

The editor can already project RIPR evidence, repair packets, receipt state,
and first-pr packet state. The adoption gap is not missing evidence; it is
unclear setup and no-output state during the first successful PR path.

This spec defines the editor behavior contract for adoption assurance:
diagnose the local extension/server/workspace/artifact state, name the safe
next action, and fail closed before repair actions when the state is unsafe.

## Behavior

Editor adoption assurance is a read-only projection over existing extension
metadata, server metadata, workspace state, config, saved artifacts, first-pr
packets, receipts, and language capability state.

The editor may render this state in:

- `ripr: Diagnose Setup`;
- `ripr: Show Status`;
- hover/status text when a diagnostic is already available;
- bounded code actions for opening or copying existing artifacts;
- `cargo xtask lsp-cockpit-report`;
- editor fixtures and VS Code e2e proof.

### Setup and compatibility status

When available, setup diagnosis should report:

```text
extension_version
resolved_server_path
server_version
server_start_state
expected_protocol_or_feature_set
supported_artifact_schema_versions
workspace_root
config_path
enabled_languages
available_language_adapters
unavailable_language_adapters
artifact_paths
artifact_freshness
first_pr_packet_state
receipt_state
next_safe_action
non_claims
```

Missing fields must be explicit. Unknown compatibility must not be treated as
compatible.

### Workspace-root states

The editor should distinguish:

```text
single_root
no_workspace
multi_root_selected
multi_root_ambiguous
nested_workspace
wrong_root_artifact
path_normalized
path_mismatch
```

Ambiguous or wrong-root states fail closed. They may show setup guidance or a
refresh/regeneration command, but they must suppress repair packets, first-pr
packet open/copy actions, and receipt claims that depend on the wrong root.

### Artifact and no-output states

No-output is not a single state. The editor should name the most specific known
state:

```text
setup_ok
server_missing
server_version_mismatch
config_missing
artifact_missing
artifact_stale
first_pr_packet_missing
first_pr_packet_ready
first_pr_packet_mismatch
first_pr_packet_malformed
receipt_missing
receipt_found
receipt_stale
receipt_gap_mismatch
receipt_movement_improved
receipt_movement_unchanged
language_disabled
preview_adapter_unavailable
preview_limited
no_actionable_gap
malformed_artifact
unsupported_schema
unsafe_path
unsafe_command
```

Unsafe states may provide setup diagnosis, refresh, regeneration, or manual
inspection guidance. They must not claim repair readiness.

### Bounded actions

Actions may appear only when the current state supports them:

| Action | Required evidence |
| --- | --- |
| Open related test | Workspace-local path, current root, compatible language, existing test target. |
| Copy repair packet | Gap identity, repair route, current artifact, safe commands, non-claims. |
| Open first-pr packet | Workspace-local Markdown packet and matching root. |
| Copy first-pr summary | Valid packet state and advisory boundary. |
| Copy verify command | Safe command for the active workspace and matching gap when applicable. |
| Copy receipt command | Safe command and receipt chain for the active workspace. |
| Refresh / diagnose setup | Server or extension state can report a safe next action. |

Stale, wrong-root, malformed, missing, unsupported, ambiguous, path-unsafe, or
command-unsafe states suppress repair and first-pr actions except refresh,
diagnose setup, or documented regeneration guidance.

### Preview-language boundary

TypeScript, JavaScript, and Python findings remain opt-in preview evidence.
When preview state appears in adoption assurance output, it must show:

```text
language_status = preview
syntax-first advisory boundary
static_limit_kind or stable static-limit text
no runtime adequacy claim
no policy eligibility claim
```

Static limits appear before suggested action language.

## Required Evidence

Follow-up PRs should provide evidence appropriate to the changed surface:

- LSP tests for current editor contract and fail-closed states;
- VS Code e2e smoke for real extension activation, server resolution, setup
  diagnosis, root state, first-pr packet state, receipt state, and action
  gating;
- editor adoption fixtures for setup, compatibility, root, artifact, first-pr,
  receipt, and preview-unavailable states;
- docs explaining install-to-first-pr recovery states;
- dogfood receipts for external-style adoption scenarios.

## Non-Goals

- No analyzer behavior changes.
- No first-pr packet producer changes.
- No generated CI summary composition.
- No PR comment publishing.
- No policy, gate, baseline, suppression, or default-blocking behavior.
- No source edits or generated tests.
- No provider/model calls.
- No mutation execution or runtime adequacy claims.
- No auto-install, binary download, config mutation, or hidden server
  replacement.
- No CodeLens, inlays, semantic tokens, inline patches, or unsaved-buffer
  overlays.

## Non-Claims

Editor adoption assurance must not claim:

- tests are adequate;
- runtime behavior is proven;
- mutation testing ran;
- a PR is merge-approved;
- a policy gate passed unless a gate artifact says so;
- preview evidence is Rust-level confidence;
- the editor produced first-pr packets, receipts, source edits, or tests.

## Acceptance Examples

Setup OK:

- Diagnose Setup reports extension and server versions, active root, config,
  enabled/available languages, current artifact state, and next safe action.

Server mismatch:

- Diagnose Setup reports the mismatch and suppresses repair actions that
  depend on compatible server behavior.

Multi-root ambiguous:

- Show Status names ambiguity, reports no selected repair root, and suppresses
  repair and first-pr actions.

Wrong-root artifact:

- Status explains the mismatch, allows refresh/regeneration guidance, and does
  not project repair packet or receipt proof from that artifact.

First-pr packet ready:

- Status names the packet path and permits open/copy actions only if the packet
  is workspace-local and current.

Preview adapter unavailable:

- Status names the unavailable preview adapter, keeps Rust defaults intact, and
  does not imply that the preview language is clean.

## Implementation Mapping

Planned slices:

1. `docs(lane3): open editor adoption assurance stack`
2. `test(lsp): pin editor adoption baseline`
3. `vscode: add extension/server compatibility diagnosis`
4. `vscode: harden workspace-root and multi-root diagnosis`
5. `fixtures(editor): add adoption-assurance fixture corpus`
6. `test(vscode): smoke editor adoption assurance path`
7. `docs(editor): write install-to-first-pr editor guide`
8. `dogfood(lane3): record external-style editor adoption receipts`
9. `campaign(lane3): close editor adoption assurance`

Each slice should preserve the projection-only boundary and add tests, fixtures,
docs, e2e proof, or receipts only for the state family it changes.

## Test Mapping

Expected proof surfaces as the burn-down lands:

- `crates/ripr/src/lsp/*` tests for setup/status/action gating;
- `editors/vscode/test/suite/extension.test.ts` for the live extension path;
- `fixtures/editor_adoption_assurance` for success and fail-closed states;
- `cargo xtask lsp-cockpit-report` for cockpit coverage;
- documentation link and static-language checks;
- dogfood receipts under `docs/handoffs/` or the campaign closeout path.

## Metrics

Candidate metrics for follow-up PRs:

- `editor_adoption_setup_ok`;
- `editor_adoption_server_missing`;
- `editor_adoption_server_version_mismatch`;
- `editor_adoption_root_ambiguous`;
- `editor_adoption_wrong_root_artifact`;
- `editor_adoption_first_pr_packet_ready`;
- `editor_adoption_receipt_stale`;
- `editor_adoption_actions_suppressed_unsafe_state`;
- `editor_adoption_preview_adapter_unavailable`.

Metrics should be added only when backed by code, tests, traceability, and
reviewable output contracts.
