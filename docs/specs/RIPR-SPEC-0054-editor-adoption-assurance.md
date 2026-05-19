# RIPR-SPEC-0054: Editor Adoption Assurance

Status: accepted

## Problem

The editor cockpit is useful after setup succeeds. Adoption fails when setup or
workspace state is ambiguous and the editor cannot explain whether the active
server, root, artifacts, receipt, and first-pr packet are safe to use.

The editor should answer:

```text
What is active, what is incompatible or unsafe, and what is safe to do next?
```

It should answer without running hidden analysis, installing binaries,
mutating config, producing PR/CI artifacts, editing source, generating tests,
calling providers, running mutation tests, or deciding policy.

## Behavior

Editor adoption assurance is a read-only projection over extension, server,
workspace, config, language, artifact, receipt, and first-pr packet state. It
extends `ripr: Diagnose Setup` and `ripr: Show Status`; it does not invent
diagnostics from setup state alone.

### Compatibility State

When data is available, the editor should expose:

| Field | Meaning |
| --- | --- |
| `extension_version` | Version of the active VS Code extension. |
| `server_path` | Resolved `ripr` server binary path or unresolved state. |
| `server_version` | Version reported by the active server. |
| `expected_server_version` | Version expected by the extension or pinned config. |
| `protocol_features` | Feature/protocol capabilities used by the cockpit. |
| `supported_artifact_schemas` | Artifact schema versions the editor can validate. |
| `unsupported_schema_state` | Unsupported artifact schemas that are ignored. |
| `next_safe_action` | Refresh, diagnose setup, regenerate, inspect docs, or stop. |

Version or feature mismatch must fail closed for repair actions that depend on
unsupported fields. The editor may still explain the mismatch and show setup or
regeneration guidance.

### Workspace and Root State

The editor should name the active workspace root when one is available and
distinguish:

- no workspace;
- single-root workspace;
- multi-root workspace with a selected safe root;
- multi-root workspace with ambiguous root state;
- nested workspace;
- workspace path with spaces;
- Windows-normalized paths;
- wrong-root artifact;
- first-pr packet root mismatch;
- receipt root or gap mismatch.

Ambiguous, wrong-root, path-unsafe, or mismatch states suppress repair packet,
open related test, open first-pr packet, verify-command, and receipt-command
actions unless the action is explicitly setup, refresh, or regeneration
guidance.

### Fail-Closed States

The editor fails closed on:

- wrong workspace root;
- stale artifact;
- malformed artifact;
- unsupported schema;
- missing identity;
- disabled language;
- unavailable adapter;
- path escape;
- unsafe command payload;
- receipt mismatch;
- first-pr packet mismatch;
- extension/server compatibility mismatch for required fields.

Fail closed means: explain the state, suppress stronger repair actions, offer
refresh/setup/regeneration guidance when safe, and make no proof, gate,
runtime, mutation, or merge-readiness claim.

### Action Authority Matrix

Editor actions are derived from validated state, not from prose. Any new
command path, context-menu path, code-action path, hover path, or first-pr path
must obey the same authority matrix.

| State | Repair packet | Related test/proof | Verify command | Receipt command | Refresh/setup guidance |
| --- | --- | --- | --- | --- | --- |
| Fresh Rust canonical repairable gap | Allowed | Allowed when workspace-local | Allowed | Allowed | Optional |
| Stale artifact | Suppressed | Suppressed | Suppressed | Suppressed | Required |
| Wrong root or ambiguous root | Suppressed | Suppressed | Suppressed | Suppressed | Required |
| Malformed artifact | Suppressed | Suppressed | Suppressed | Suppressed | Required |
| Unsupported schema or version | Suppressed | Suppressed | Suppressed | Suppressed | Required |
| Missing identity | Suppressed | Suppressed | Suppressed | Suppressed | Required |
| Disabled language | Suppressed | Suppressed | Suppressed | Suppressed | Required |
| Preview adapter unavailable | Suppressed | Suppressed | Suppressed | Suppressed | Required |
| Preview-only evidence | No stable repair authority | Advisory inspection only | Advisory only when explicitly safe | Suppressed as authority | Required |
| No actionable gap | Suppressed | Inspection only when workspace-local | Suppressed | Suppressed | Optional |
| Receipt stale or mismatched | Suppressed | Suppressed | Regeneration or verify guidance only | Suppressed | Required |
| First-pr packet mismatch | Suppressed | Suppressed | Suppressed | Suppressed | Required |
| Unsafe path or command payload | Suppressed | Suppressed | Suppressed | Suppressed | Required |
| Compatibility mismatch | Suppressed for unsupported fields | Suppressed for unsupported fields | Suppressed for unsupported fields | Suppressed for unsupported fields | Required |

`Allowed` means the editor may project a bounded, read-only action after every
artifact, root, language, identity, and command check passes. `Suppressed`
means the editor must not copy or open a stronger repair action and should
instead explain the unsafe state. Advisory preview actions must be visibly
preview/advisory and must not claim Rust-level confidence, gate eligibility, or
runtime adequacy.

### Artifact Authority Contract

An editor artifact is action-authoritative only when every required field is
present, supported, and consistent with the current workspace:

- `schema_version` is one of the editor-supported versions for that artifact;
- `tool` and `kind` identify a known RIPR artifact shape;
- `ripr_version`, protocol features, or equivalent compatibility state are
  supported for every field the editor depends on;
- `workspace_root`, `root`, or equivalent root identity matches the selected
  workspace root after platform-aware normalization;
- `generated_at`, receipt age, status, or an equivalent freshness marker does
  not mark the artifact stale for the action being projected;
- `language` and `language_status` authorize the requested action;
- stable repair actions require stable Rust evidence;
- `canonical_gap_id`, `gap_id`, or packet identity matches the current
  diagnostic or selected first-pr packet before repair, verify, or receipt
  actions are shown;
- receipt identity matches the same root and gap identity before receipt
  movement is projected;
- command payloads are typed, bounded, and accepted by the editor command
  safety contract;
- malformed, missing, unknown, unsafe, or unsupported fields fail closed.

The editor may display unavailable or advisory state for incomplete artifacts,
but it must not promote incomplete artifacts into repair authority.

### Root Equivalence And Command Targeting

Root equality is evaluated after platform-aware normalization. The editor must
not compare raw path strings when:

- Windows path separators differ;
- drive casing differs;
- URI encoding differs;
- paths contain spaces;
- a path is nested under another workspace;
- the active file is outside the selected root;
- a command has no active file and no target URI.

Root-scoped commands must resolve the active document or explicit target URI
before projecting a repair packet, static-limit note, related test/proof,
verify command, or receipt command. In a multi-root workspace, the editor must
use the selected safe root or fail closed. It must never guess across roots.

### Start Current Repair Contract

`ripr: Start Current Repair` is the cockpit entry point for the editor repair
loop. It must:

1. resolve an active saved document and selected workspace root;
2. reject no-active-file, wrong-root, and ambiguous-root states before action
   lookup;
3. consume only saved-workspace diagnostics and artifacts;
4. validate artifact authority before presenting repair actions;
5. match diagnostic identity to the canonical gap or first-pr packet identity
   required by the action;
6. present bounded actions in deterministic order:
   repair packet, related test/proof, verify command, receipt command,
   static-limit note, first-pr/open-or-regenerate guidance;
7. fall back to setup, refresh, or regeneration guidance when any authority
   check fails.

The command must not install binaries, run hidden analysis, generate artifacts,
edit source, generate tests, call providers, run mutation execution, or parse
Markdown prose to decide action authority.

### Receipt Authority

A receipt may show that a static repair loop was attempted, unchanged, or
improved for a matching gap, root, and artifact identity. The editor may show
receipt state only within that identity scope.

A receipt is not:

- merge approval;
- runtime mutation proof;
- coverage adequacy;
- policy eligibility;
- gate authority;
- preview-language promotion.

Stale, wrong-root, malformed, unsupported, missing-identity, or mismatched
receipts suppress receipt-based repair authority and should point the user to
verify/receipt regeneration.

### Preview Boundary

TypeScript, JavaScript, and Python evidence remains:

- opt-in;
- syntax-first;
- advisory;
- `language_status = "preview"` visible;
- static-limit labeled when present;
- not Rust-level confidence;
- not runtime adequacy;
- not mutation proof;
- not policy eligible;
- not gate authority.

Static limits appear before suggested action language.

## Required Evidence

Future implementation must provide:

- LSP tests that pin the closed baseline before behavior changes;
- tests for compatibility mismatch, unsupported schema, root mismatch, and
  command/path safety;
- VS Code e2e smoke for compatibility, workspace root, status, receipt, and
  first-pr packet state;
- fixtures for success and fail-closed adoption states;
- docs explaining install-to-first-pr usage and recovery;
- dogfood receipts from external-style repo states.

## Inputs

The editor may consume:

- VS Code extension metadata;
- server resolution state and version response;
- workspace roots;
- repository config;
- enabled and available languages;
- saved-workspace evidence and gap artifacts;
- first-useful-action reports;
- repair cards;
- receipts;
- first-pr packets;
- static-limit metadata;
- verify and receipt commands.

## Outputs

Lane 3 may output:

- Diagnose Setup text;
- Show Status text;
- hover explanation;
- bounded code actions;
- fixture artifacts;
- VS Code smoke assertions;
- docs and dogfood handoff receipts.

Lane 3 must not output analyzer facts, first-pr packets, generated CI
summaries, PR comments, source edits, generated tests, provider results,
mutation results, gate decisions, policy changes, or release artifacts.

## Non-Goals

- No analyzer changes.
- No hidden analysis reruns from the editor.
- No binary installation, binary download, or config mutation.
- No policy, gate, default-blocking, badge, waiver, baseline, or suppression
  changes.
- No PR comment publishing or generated CI summary composition.
- No release behavior.
- No source edits, inline patches, or automatic repair application.
- No generated tests.
- No provider or model calls.
- No runtime mutation execution.
- No CodeLens, inlay hints, semantic tokens, inline patches, or
  unsaved-buffer overlays.
- No preview-language promotion.

## Acceptance Examples

Compatible setup:

- Given a server version compatible with the extension, a selected workspace
  root, and fresh artifacts, Diagnose Setup names the version, root, artifact
  state, and next safe action.

Version mismatch:

- Given an incompatible server version for required artifact fields, status
  reports the mismatch and suppresses repair actions that depend on those
  fields.

Multi-root ambiguous:

- Given multiple workspace roots with no safe selected root, status reports
  ambiguous root state and suppresses root-scoped repair actions.

Wrong-root artifact:

- Given a receipt or first-pr packet from another root, the editor reports the
  mismatch and suppresses open/copy/repair actions.

Preview unavailable:

- Given a preview language enabled in config but unavailable in the current
  server capability set, status explains adapter unavailable and makes no
  repair claim.

## Test Mapping

Traceability for this spec includes:

- `crates/ripr/src/lsp/tests.rs` for status serialization and fail-closed
  behavior;
- `editors/vscode/test/suite/extension.test.ts` for setup/status,
  first-pr packet, receipt, and packaged-extension smoke coverage;
- `fixtures/editor_adoption_assurance/*` for setup and mismatch states;
- `cargo xtask lsp-cockpit-report` coverage after status enters the cockpit
  report.

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

## Metrics

Future implementation may add metrics only when backed by code and traceable
tests. Candidate metrics:

- `editor_adoption_compatibility_ok`;
- `editor_adoption_server_version_mismatch`;
- `editor_adoption_no_workspace`;
- `editor_adoption_multi_root_ambiguous`;
- `editor_adoption_wrong_root_artifact`;
- `editor_adoption_first_pr_packet_mismatch`;
- `editor_adoption_receipt_mismatch`;
- `editor_adoption_actions_suppressed_unsafe_state`.
