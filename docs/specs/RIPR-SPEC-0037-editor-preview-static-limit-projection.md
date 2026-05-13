# RIPR-SPEC-0037: Editor Preview Static-Limit Projection

Status: proposed

## Problem

Preview language findings are syntax-first and advisory. They can still help a
human or coding agent write one focused test, but only if the editor makes the
evidence boundary visible before presenting an action.

Without an explicit projection contract, hover text, status text, and actions
could make TypeScript-family or Python preview evidence look like stable Rust
evidence. That would undermine the saved-workspace cockpit's trust model.

The proposal context is in
[RIPR-PROP-0003: Editor Preview Routing](../proposals/RIPR-PROP-0003-editor-preview-routing.md).
The projection-only architecture decision is recorded in
[ADR-0011: Editor preview routing is projection-only](../adr/0011-editor-preview-routing-is-projection-only.md).
The routing contract is in
[RIPR-SPEC-0036: Editor preview routing](RIPR-SPEC-0036-editor-preview-routing.md).

## Behavior

Core rule:

```text
Preview static limits must appear before suggested action language.
```

For every preview finding projected into hover or status, the editor must make
these fields visible when present:

```text
Language: <language or document-family label>
Status: preview
Evidence: syntax-first
Static limit: <kind or stable text>
Action: advisory only, when evidence supports a bounded next step
Limits: static evidence only; no runtime adequacy claim
```

When multiple static limits exist, show a stable, deterministic list before
action language. The editor may group limits by `static_limit_kind` when the
field exists.

Structured/static text rule:

- If `static_limit_kind` exists, use it for display grouping and stable labels.
- If `static_limit_kind` does not exist, render stable static-limit text as
  evidence only.
- Do not branch code-action semantics on parsed prose.
- Do not hide a preview finding merely because static-limit metadata is text
  rather than structured kind.

Preview projection must not imply:

- runtime mutation execution;
- runtime adequacy;
- policy or gate eligibility;
- Rust-level maturity;
- generated-test availability;
- provider-backed analysis;
- source-edit authority.

## Required Evidence

A preview hover or status entry needs:

- `language`;
- `language_status = "preview"`;
- current workspace-root match;
- current artifact freshness;
- stable finding or diagnostic identity;
- static-limit text or `static_limit_kind` when available;
- related test identity when the action opens or copies a related test;
- suggested assertion text when the action copies an assertion;
- verify and receipt command payloads when those actions are offered.

Missing preview labels or missing root/freshness proof means the preview
finding must not be projected as a normal finding. Missing structured
`static_limit_kind` does not block projection when stable static-limit text is
available; it only prevents kind-based behavior.

## Inputs

| Input | Required? | Purpose |
| --- | --- | --- |
| Preview finding metadata | yes | Supplies language and preview status. |
| Static-limit kind | optional | Enables stable grouping and labels. |
| Static-limit text | required when no kind exists | Explains what RIPR could not establish. |
| Suggested next action | optional | Provides bounded user action language. |
| Related test or suggested assertion | optional | Enables related-test and copy-assertion actions. |
| Verify or receipt command | optional | Enables pasteable command actions. |
| Stale, malformed, missing, or wrong-root state | yes when present | Keeps fail-closed status dominant. |

The editor does not consume provider output, generated tests, runtime mutation
results, or unsaved-buffer analysis for preview static-limit projection.

## Outputs

Hover output for a preview finding must show, in order:

1. language and preview status;
2. syntax-first evidence label;
3. static-limit kind or stable static-limit text;
4. what RIPR observed;
5. what RIPR could not establish;
6. bounded advisory action, if any;
7. copy/open command affordances already supported by the cockpit.

Status output must make preview state and static limits visible before any
message that suggests what to do next.

Diagnostics should preserve preview identity through `diagnostic.data` and use
diagnostic text that does not make preview evidence look stable. The detailed
diagnostic wording may be shorter than hover/status, but the preview boundary
must remain recoverable from the diagnostic and hover path.

Actions use the same cockpit model as Rust. Preview findings may offer only
bounded actions backed by evidence:

- copy packet;
- copy brief;
- copy suggested assertion, when present;
- open related test, when present;
- copy after-snapshot, verify, or receipt commands, when valid;
- refresh analysis.

Actions must not be added merely because a static-limit kind appears. Static
limits may suppress or qualify action language when the artifact says the next
step is unsupported, but text parsing must not create new semantics.

## Non-Goals

- No new preview-only action model.
- No source edits.
- No generated tests.
- No provider calls.
- No runtime mutation execution.
- No policy, gate, or default-blocking changes.
- No runtime adequacy claims.
- No CodeLens, inlay hints, semantic tokens, inline patches, or unsaved-buffer
  overlays.
- No prose parsing for action semantics.

## Acceptance Examples

Preview hover with structured static limit:

```text
Language: python
Status: preview
Evidence: syntax-first
Static limit: missing_import_graph
RIPR saw: related test path and assertion shape
RIPR could not establish: imported call target resolution
Action: advisory only - copy packet
```

Preview hover with text-only static limit:

```text
Language: typescript
Status: preview
Evidence: syntax-first
Static limit: mocked module call may hide runtime dispatch
RIPR saw: test file and broad matcher
RIPR could not establish: exact runtime call target
Action: advisory only - open related test
```

Preview status for disabled language:

- Status reports that the preview language is disabled.
- No preview diagnostic action language is shown.

Preview status for stale report:

- Stale status remains dominant.
- Static-limit and action text from stale preview findings is not projected.

Preview action with no suggested assertion:

- Copy packet and refresh may be available.
- Copy suggested assertion is omitted.
- Hover explains the preview static limit before any action text.

Structured kind absent:

- The editor displays the stable static-limit text.
- The editor does not parse that prose to choose a different action.

## Test Mapping

Follow-up behavior work must add or preserve tests for:

- preview hover ordering with `static_limit_kind`;
- preview hover ordering with text-only static-limit evidence;
- preview status ordering with preview labels before action language;
- diagnostics preserving preview identity and access to hover evidence;
- actions staying bounded to the existing cockpit command model;
- omitted actions when related test, suggested assertion, verify, or receipt
  payloads are absent;
- stale, malformed, missing, and wrong-root artifacts suppressing preview
  action text;
- VS Code e2e coverage for preview labels and static limits in hover/status.

## Implementation Mapping

Implementation belongs after preview routing and preview adapter artifacts are
ready:

- `lsp(language): surface preview labels and static limits`;
- `fixtures: add preview editor workflow fixtures`;
- `test(vscode): smoke preview saved-workspace routing`;
- `docs(editor): document preview-language workflow`;
- `campaign(lane3): close editor preview routing`.

Likely implementation surfaces:

- `crates/ripr/src/lsp/hover.rs`;
- `crates/ripr/src/lsp/diagnostics.rs`;
- `crates/ripr/src/lsp/actions.rs`;
- `crates/ripr/src/lsp/state.rs`;
- `crates/ripr/src/lsp/tests.rs`;
- `editors/vscode/test/suite/extension.test.ts`;
- `fixtures/editor_lsp_workflow/`;
- `cargo xtask lsp-cockpit-report`.

## Metrics

Future metrics may count:

- `editor_preview_static_limit_hovers`;
- `editor_preview_static_limit_status_entries`;
- `editor_preview_static_limit_structured_kind`;
- `editor_preview_static_limit_text_only`;
- `editor_preview_actions_with_static_limit`;
- `editor_preview_actions_omitted_missing_payload`;
- `editor_preview_stale_suppressed_actions`;
- `editor_preview_wrong_root_suppressed_actions`.
