# RIPR-SPEC-0048: Editor Setup Status

Status: proposed

## Problem

The editor cockpit can project saved-workspace RIPR evidence, but first-run and
no-output states can still look like silence. A user who opens VS Code needs to
know whether RIPR started, which binary is active, which workspace is being
analyzed, which languages are enabled and available, which artifacts were
found, and what safe step comes next.

Setup status must answer:

```text
Why do I see what I see, and what should I do first?
```

## Behavior

Editor setup status is a read-only projection over known extension, server,
workspace, configuration, language, artifact, freshness, and receipt state.
It may appear in `ripr: Show Status` and in a dedicated `ripr: Diagnose Setup`
command, but it must not invent diagnostics or repair actions from setup state
alone.

The setup status model should capture typed fields when available:

| Field | Meaning |
| --- | --- |
| `server_path` | Resolved `ripr` server binary path or unresolved state. |
| `server_version` | Server version reported by the active binary, when known. |
| `server_started` | Whether the LSP server started and can respond. |
| `workspace_root` | Active workspace root used for saved-workspace artifact matching. |
| `config_path` | Config file path or missing/configless state. |
| `enabled_languages` | Languages enabled by runtime config. |
| `available_languages` | Languages compiled into the current binary or server capability set. |
| `artifact_paths` | Evidence, gap, status, receipt, or cockpit artifacts found or missing. |
| `artifact_state` | Fresh, stale, wrong-root, missing, malformed, or unsupported. |
| `next_safe_action` | One setup or refresh command that is safe for the user to run. |
| `limits` | Non-claims such as no gate decision, no runtime adequacy claim, and no hidden analysis. |

The status surface should distinguish these states when evidence exists:

- no workspace;
- server unavailable;
- server available;
- missing config;
- Rust-only default;
- preview language disabled;
- preview adapter unavailable in the current build;
- artifact missing;
- artifact stale;
- wrong-root artifact ignored;
- malformed artifact ignored;
- no actionable gap;
- actionable gap available.

### No-output semantics

No diagnostics must not collapse into one generic state. The editor should
prefer the most specific safe explanation it can support:

| State | Required status behavior |
| --- | --- |
| No workspace | Explain that no workspace root is available and no saved-workspace analysis can be matched. |
| Server unavailable | Explain that the server did not start or respond and show the known binary path or missing path. |
| Artifact missing | Explain that no supported saved-workspace artifact was found and name the expected artifact path when known. |
| Artifact stale | Explain that evidence is stale and suppress repair actions except refresh. |
| Wrong root | Explain that an artifact was ignored because its root does not match the workspace. |
| Language disabled | Explain that the language is disabled by config and no diagnostics are projected. |
| Adapter unavailable | Explain that the language is not available in the current binary or server capability set. |
| No actionable gap | Explain that evidence exists but no local repair action is currently supported. |
| Actionable gap | Explain that a safe local action is available and point to hover or code actions. |

### Fail-closed rules

Setup status must fail closed on:

- wrong workspace root;
- stale artifact;
- missing identity;
- unsupported schema;
- malformed artifact;
- disabled language;
- unavailable preview adapter;
- out-of-workspace path;
- malformed command payload.

Fail closed means no stronger diagnostic, repair action, receipt claim, or
preview confidence claim is projected from the invalid state. The editor may
still show setup guidance, refresh guidance, or missing-state explanation.

## Required Evidence

Future implementation must provide:

- LSP or extension tests for setup status fields;
- VS Code e2e coverage for no workspace, server unavailable, server
  available, missing config, Rust-only default, disabled preview language,
  unavailable adapter, stale evidence, no actionable gap, and actionable gap;
- fixtures for setup and no-output states;
- `lsp-cockpit-report` coverage when setup status becomes part of the cockpit
  report;
- docs that explain each no-output state in user-facing terms.

## Inputs

Editor setup status may consume:

- server resolution state;
- server version response;
- extension activation state;
- workspace root;
- repository configuration;
- language runtime config;
- available language capability metadata;
- saved-workspace artifact paths and metadata;
- evidence and gap freshness;
- receipt artifact metadata;
- existing refresh, verify, and receipt commands.

## Outputs

Editor setup status may render:

- `ripr: Show Status` lines;
- a `ripr: Diagnose Setup` report;
- setup/no-output fixture artifacts;
- status sections in `lsp-cockpit-report`;
- docs and dogfood receipts.

It must not output new analyzer facts, policy decisions, gate decisions, PR
comments, generated tests, source edits, provider calls, mutation runs, or
generated CI summaries.

## Non-Goals

- No analyzer changes.
- No hidden analysis reruns from the editor.
- No binary installation, download, or repair from this spec.
- No config mutation from the editor.
- No new diagnostic invented solely from setup state.
- No generated tests or source edits.
- No provider or model calls.
- No runtime mutation execution.
- No policy, gate, default-blocking, badge, waiver, or baseline changes.
- No PR comment publishing.
- No CodeLens, inlay hints, semantic tokens, inline patches, or
  unsaved-buffer overlays.

## Acceptance Examples

Setup ok:

- Given the server starts, a workspace root is active, Rust is enabled by
  default, and fresh artifacts exist, status shows the binary, workspace,
  enabled languages, artifact state, and next safe action.

Server missing:

- Given no usable server binary is resolved, status explains that the server is
  unavailable and suppresses diagnostics and repair actions.

Preview disabled:

- Given a Python file and preview languages disabled by config, no Python
  diagnostics are projected and status names the disabled language state.

Adapter unavailable:

- Given a language enabled by config but unavailable in the current binary,
  status explains the unavailable adapter state without suggesting a repair
  action for code.

Stale artifact:

- Given stale saved-workspace evidence, status says refresh is required before
  repair and suppresses stale repair actions.

No actionable gap:

- Given valid evidence with no actionable gap, status explains the no-action
  state instead of implying setup failure.

## Test Mapping

Follow-up PRs should add or update:

- `editors/vscode/test/suite/extension.test.ts` for live setup and no-output
  smoke coverage;
- `crates/ripr/src/lsp/tests.rs` for status serialization and fail-closed
  behavior;
- `fixtures/editor_first_run_usability/*` for setup status fixtures;
- `cargo xtask lsp-cockpit-report` coverage after status enters the cockpit
  report.

This docs PR does not add behavior tests.

## Implementation Mapping

Planned slices:

1. `docs(lane3): open editor first-run usability stack`
2. `vscode: add setup diagnosis status model`
3. `vscode: add ripr Diagnose Setup command`
4. `test(vscode): smoke first-run and no-output states`
5. `fixtures(editor): add first-run usability fixtures`
6. `docs(editor): write first-run-to-first-receipt guide`
7. `dogfood(lane3): record first-run repair receipts`
8. `campaign(lane3): close editor first-run usability`

Receipt-specific behavior is defined in `RIPR-SPEC-0049`.

## Metrics

Future implementation may add metrics only when backed by code and traceable
tests. Candidate metrics:

- `editor_setup_status_reports`;
- `editor_setup_server_unavailable`;
- `editor_setup_artifact_missing`;
- `editor_setup_artifact_stale`;
- `editor_setup_language_disabled`;
- `editor_setup_adapter_unavailable`;
- `editor_setup_next_safe_action_refresh`;
- `editor_setup_no_actionable_gap`.
