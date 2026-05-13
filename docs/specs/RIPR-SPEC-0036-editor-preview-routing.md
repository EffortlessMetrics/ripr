# RIPR-SPEC-0036: Editor Preview Routing

Status: proposed

## Problem

The saved-workspace Rust editor cockpit already projects RIPR evidence at the
point of coding: diagnostics, hover evidence, code actions, context packets,
status, related-test opening, and copyable packet, brief, after-snapshot,
verify, receipt, and refresh commands.

Campaign 27 adds syntax-first preview adapters for TypeScript-family files and
Python. Those findings should become useful in the same editor cockpit only
when explicitly enabled, and without making preview evidence look like stable
Rust evidence. The editor must also keep its current fail-closed behavior for
wrong-root, stale, missing, and malformed artifacts.

The proposal context is in
[RIPR-PROP-0003: Editor Preview Routing](../proposals/RIPR-PROP-0003-editor-preview-routing.md).
The projection-only architecture decision is recorded in
[ADR-0011: Editor preview routing is projection-only](../adr/0011-editor-preview-routing-is-projection-only.md).
Static-limit display ordering is specified separately in
[RIPR-SPEC-0037: Editor preview static-limit projection](RIPR-SPEC-0037-editor-preview-static-limit-projection.md).

## Behavior

The editor remains a saved-workspace projection surface. It consumes RIPR
artifacts and adapter output produced for the workspace root; it does not run
hidden analysis, parse source independently, generate tests, edit source, call
providers, execute mutation tools, or decide policy.

Rust behavior is the baseline:

- When `[languages]` is absent, the effective editor route is Rust only.
- When `[languages] enabled = ["rust"]`, behavior is the same as the default.
- Rust diagnostics, hover, actions, status, command payloads, and
  `lsp-cockpit-report` coverage stay unchanged by default.
- Existing wrong-root, stale, missing, malformed, and unsupported-report
  handling remains dominant over language routing.

Language routing is config-gated:

- `rust` remains enabled by default.
- `typescript` enables TypeScript-family preview routing for
  `typescript`, `typescriptreact`, `javascript`, and `javascriptreact`
  documents.
- `python` enables Python preview routing for `python` documents.
- `[languages] enabled = []` disables saved-workspace diagnostics and makes
  `ripr: Show Status` explain that languages are off.
- Disabled preview-language documents must not produce diagnostics, hover
  findings, or code actions from stale preview artifacts.
- Unknown or invalid language configuration is owned by configuration and
  doctor surfaces. The editor must not invent a fallback preview route or
  reinterpret invalid config as opt-in.

VS Code may register activation events for preview language IDs so the
extension can start in preview-language files. The effective LSP document
selector and diagnostic projection still depend on repo configuration and the
workspace artifacts being current, rooted, and well-formed.

The selector vocabulary is:

| VS Code language ID | RIPR route | Default |
| --- | --- | --- |
| `rust` | Rust stable adapter | enabled |
| `typescript` | TypeScript preview adapter | disabled |
| `typescriptreact` | TypeScript preview adapter | disabled |
| `javascript` | TypeScript preview adapter | disabled |
| `javascriptreact` | TypeScript preview adapter | disabled |
| `python` | Python preview adapter | disabled |

Preview findings must preserve the language metadata emitted by upstream
artifacts. For TypeScript-family files, the current adapter language metadata
is `language = "typescript"` with `language_status = "preview"` unless a later
output contract adds distinct JavaScript metadata.

## Required Evidence

An editor finding is projectable only when the saved-workspace artifact or
adapter output provides enough evidence to preserve identity and boundaries:

- `language`;
- `language_status`;
- stable seam, finding, or diagnostic identity;
- workspace-root identity or equivalent root check;
- source file path and range;
- static-limit text, or `static_limit_kind` when available;
- related test identity and path when available;
- suggested assertion text when available;
- packet, brief, after-snapshot, verify, receipt, and refresh command payloads
  when valid for the finding;
- stale, wrong-root, malformed, unsupported, or missing-artifact state when the
  finding must fail closed.

Preview routing must not infer these fields from editor buffer text. Missing
identity, missing root proof, stale artifacts, or malformed report state means
no preview diagnostic projection.

## Inputs

| Input | Required? | Purpose |
| --- | --- | --- |
| Workspace root | yes | Same root used by the saved-workspace cockpit. |
| Current document URI and VS Code language ID | yes | Chooses the candidate route. |
| Repo configuration | yes | Determines whether a preview language is enabled. |
| Saved-workspace RIPR artifact | yes | Supplies diagnostics, hover evidence, status, and actions. |
| Artifact root and freshness state | yes | Preserves wrong-root and stale fail-closed behavior. |
| Preview adapter output | required for preview findings | Supplies language metadata, identity, related tests, assertions, commands, and static limits. |

The editor does not consume unsaved-buffer analysis, provider output, generated
test content, mutation-runner output, or policy promotion state for this route.

## Outputs

The editor may project these existing cockpit outputs when the route is
enabled and evidence is current:

- diagnostics with stable `diagnostic.data` identity;
- hover evidence;
- `ripr: Show Status` state;
- code actions;
- bounded context packets;
- related-test opening;
- copyable packet, brief, after-snapshot, verify, receipt, and refresh
  commands.

For preview findings, outputs must visibly include preview status where the
surface has text space or structured payload space. Static-limit rendering
details belong to `RIPR-SPEC-0037`, but this routing spec requires that static
limits be preserved and made available to those projection surfaces.

## Non-Goals

- No analyzer changes.
- No generated tests.
- No source edits.
- No provider calls.
- No runtime mutation execution.
- No policy, gate, or default-blocking changes.
- No CodeLens, inlay hints, semantic tokens, inline patches, or unsaved-buffer
  overlays.
- No new preview-only action model.
- No runtime adequacy claims for preview languages.
- No per-language editor extension or LSP server.

## Acceptance Examples

Rust-only workspace, default config:

- Opening a Rust file produces the same diagnostics, hover, code actions,
  status, packet, brief, verify, receipt, and refresh behavior as before.
- `cargo xtask lsp-cockpit-report` output remains stable for the Rust cockpit.

Rust workspace with `[languages] enabled = ["rust"]`:

- Behavior matches the default config.

Workspace with `[languages] enabled = []`:

- No saved-workspace diagnostics are projected.
- `ripr: Show Status` explains that languages are off.
- The editor does not invent diagnostics from existing artifacts.

Mixed workspace, default config:

- Rust files keep current behavior.
- TypeScript, TSX, JavaScript, JSX, and Python files do not produce preview
  diagnostics, hover findings, or code actions.

Mixed workspace, TypeScript enabled:

- `typescript`, `typescriptreact`, `javascript`, and `javascriptreact`
  documents may receive TypeScript-preview diagnostics only from current,
  root-matched, well-formed artifacts.
- Diagnostics and hover/status text label the finding as preview.
- Mixed-language related-test routing does not point at another language's
  owner or test.

Mixed workspace, Python enabled:

- `python` documents may receive Python-preview diagnostics only from current,
  root-matched, well-formed artifacts.
- Diagnostics and hover/status text label the finding as preview.

Wrong-root report:

- No diagnostics are projected from the report.
- Status explains the wrong-root state through the existing fail-closed path.

Stale analysis:

- Stale status remains dominant.
- Preview routing does not show diagnostics from stale artifacts.

Malformed report:

- No preview diagnostics are projected.
- The editor uses the existing malformed-report status path.

Invalid language config:

- Configuration and doctor surfaces own the error.
- The editor does not treat an invalid value as preview opt-in.

## Test Mapping

Follow-up behavior work must add or preserve tests for:

- default Rust behavior with `[languages]` absent;
- Rust-only behavior with `[languages] enabled = ["rust"]`;
- languages-off behavior with `[languages] enabled = []`;
- disabled preview-language documents producing no diagnostics;
- enabled TypeScript-family preview routing;
- enabled Python preview routing;
- mixed-language routing that does not cross-route owners or tests;
- wrong-root, stale, missing, unsupported, and malformed artifacts failing
  closed;
- command payload contracts for preview findings with valid packet, brief,
  after-snapshot, verify, receipt, and refresh payloads;
- VS Code extension activation and document selector behavior for Rust default
  and enabled preview languages.

## Implementation Mapping

Implementation belongs to Campaign 27 after `analysis/python-preview-adapter`
emits editor-projectable preview artifacts or the active manifest explicitly
supersedes that blocker.

Expected slices:

- `test(lsp): preserve Rust routing contract`;
- `lsp(language): add opt-in editor language routing`;
- `fixtures: add preview editor workflow fixtures`;
- `test(vscode): smoke preview saved-workspace routing`;
- `docs(editor): document preview-language workflow`.

Likely implementation surfaces:

- `editors/vscode/package.json`;
- `editors/vscode/src/client.ts`;
- `crates/ripr/src/lsp/config.rs`;
- `crates/ripr/src/lsp/diagnostics.rs`;
- `crates/ripr/src/lsp/tests.rs`;
- `fixtures/editor_lsp_workflow/`;
- `cargo xtask lsp-cockpit-report`.

## Metrics

Future metrics may count:

- `editor_language_routing_rust_default_sessions`;
- `editor_language_routing_preview_documents_seen`;
- `editor_language_routing_preview_documents_enabled`;
- `editor_language_routing_preview_documents_disabled`;
- `editor_language_routing_wrong_root_reports`;
- `editor_language_routing_stale_reports`;
- `editor_language_routing_malformed_reports`;
- `editor_language_routing_preview_diagnostics_projected`;
- `editor_language_routing_preview_diagnostics_suppressed`.
