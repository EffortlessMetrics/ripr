# RIPR-SPEC-0047: Editor Gap Projection

Status: proposed

## Problem

The editor already projects saved-workspace RIPR evidence as diagnostics,
hover, status, and bounded code actions. RIPR now also has gap records, repair
routes, first-useful-action reports, receipts, and preview-language static
limits. Lane 3 needs a behavior contract for projecting those artifacts into a
single local repair cockpit without inventing analyzer truth, policy truth, or
source edits.

The editor should answer:

```text
What is this gap, what can I safely do next, and what proves movement?
```

## Behavior

Editor gap projection resolves every rendered gap through a stable identity
path:

```text
diagnostic.data
-> canonical_gap_id / seam_id / finding_id
-> evidence_record or gap_record
-> repair route / related test / verify command
-> receipt command or receipt path
```

Lane 3 consumes existing artifacts. It does not create gap records, repair
routes, analyzer facts, policy decisions, gate decisions, PR comments,
generated CI summaries, source edits, generated tests, provider calls, or
mutation runs.

### Artifact validation

Before rendering gap-specific status, hover, or actions, the LSP layer must
validate the referenced artifact:

| Rule | Required behavior |
| --- | --- |
| Supported schema | Unsupported schema versions fail closed with status guidance. |
| Workspace root | Artifacts whose root does not match the active workspace are ignored for projection. |
| Identity | A `canonical_gap_id`, `seam_id`, or `finding_id` must be present and stable enough to match `diagnostic.data`. |
| Freshness | Stale artifacts suppress gap actions except refresh and show stale status. |
| Language | Stable Rust remains enabled by default; preview languages require opt-in config and preview metadata. |
| Paths | Related tests and command paths must stay inside the active workspace. |
| Commands | Verify and receipt commands must be present, non-empty, and rooted in the current workspace before copy actions appear. |
| Static limits | Registered `static_limit_kind` values are preferred; text-only static limits render as evidence only. |

Fail closed means no gap-specific diagnostic/action/hover claim is projected
from the invalid artifact. The editor may still show a root, stale, disabled,
or refresh status when that status is supported by existing editor state.

### Status projection

`ripr: Show Status` should summarize the current safe next step using existing
artifact state. Examples:

```text
ripr: 1 actionable gap
ripr: preview evidence available
ripr: no actionable seam
ripr: stale evidence; refresh before acting
ripr: Python preview unavailable in this binary
ripr: language disabled by config
ripr: wrong-root report ignored
```

Status must name enough context for first-run and no-output diagnosis:

- workspace root;
- server source and command when known;
- enabled languages;
- language unavailable or disabled states;
- freshness;
- whether the current evidence is stable Rust or opt-in preview evidence;
- the next safe command or config change when one is known.

### Hover projection

When a diagnostic maps to a validated gap or evidence record, hover should
render in this order:

1. language and preview status;
2. static limits, when present;
3. gap state;
4. why the gap matters;
5. related test or repair target;
6. verify command;
7. receipt command or receipt path;
8. limits and non-claims.

Preview/static-limit boundaries must appear before suggested action language.
Hover must not imply runtime adequacy, proof, gate eligibility, or Rust-level
confidence for preview-language evidence.

### Action projection

The editor may expose bounded actions only when the current artifact provides
enough typed evidence:

| Action | Show only when |
| --- | --- |
| Open related test | A workspace-local, current-language related-test path exists. |
| Copy repair packet | Gap identity and repair route exist. |
| Copy verify command | A valid verify command exists and references the current workspace. |
| Copy receipt command | A valid receipt command or receipt chain exists. |
| Copy static-limit note | A static limit exists. |
| Refresh evidence | The server is available enough to request refresh. |

Actions must not be shown with empty payloads, path escapes, stale artifacts
except refresh, wrong-root artifacts, disabled languages, or malformed command
payloads. Preview languages use the same action model with sharper labels; the
editor must not add a separate preview-only command system.

## Required Evidence

Implemented editor gap projection must provide:

- tests that pin Rust default diagnostics, hover, actions, and status before
  new gap projection lands;
- tests for preview-language diagnostic metadata and static-limit hover order;
- tests for disabled-language no-diagnostic status;
- tests for stale, wrong-root, malformed, unsupported-schema, missing-identity,
  out-of-workspace path, and malformed-command fail-closed behavior;
- fixtures for actionable Rust, TypeScript preview static limit, Python preview
  static limit, disabled language, wrong root, stale artifact, and no
  actionable gap;
- live VS Code e2e coverage for activation, server resolution, hover, status,
  code action bounds, related-test safety, stale state, and wrong-root state;
- docs that explain the local repair cockpit workflow and all hard non-goals.

## Inputs

Editor gap projection may consume:

- LSP diagnostics and `diagnostic.data`;
- evidence records;
- gap records and gap decision ledgers;
- first-useful-action reports;
- repair-card or PR guidance artifacts when they already exist as files;
- receipt artifacts and receipt commands;
- language metadata and preview status;
- static-limit metadata;
- workspace-root and freshness state;
- related-test facts;
- verify commands.

Missing or ambiguous inputs must suppress the stronger projection instead of
inventing a fallback claim.

## Outputs

Lane 3 outputs are editor projections, not new analyzer truth:

- LSP diagnostics with stable identity data;
- hover Markdown;
- `ripr: Show Status` content;
- code action payloads;
- context packets and repair packets;
- related-test open commands;
- verify and receipt command copy payloads;
- editor workflow fixtures;
- `lsp-cockpit-report` evidence.

The output should preserve existing Rust behavior by default and add
gap-specific fields only when validated matching artifacts exist.

## Non-Goals

- No analyzer changes.
- No evidence schema invention in the editor.
- No PR or CI summary composition.
- No PR comment publishing.
- No policy, gate, default-blocking, badge, baseline, waiver, or suppression
  changes.
- No generated tests.
- No automatic source edits or inline patches.
- No provider or model calls.
- No runtime mutation execution.
- No CodeLens, inlay hints, semantic tokens, or unsaved-buffer overlays.
- No preview-language promotion to stable Rust confidence.

## Acceptance Examples

Rust default:

- Given a Rust workspace with no gap artifact, existing saved-workspace
  diagnostics, hover, actions, and status remain unchanged.
- Given a matching validated Rust gap record, the editor may add repair route,
  verify command, receipt command, and related-test actions.

Preview static limit:

- Given TypeScript or Python preview evidence with a static limit, hover shows
  language, preview status, and static limit before action language.
- Code actions remain advisory and bounded.

Disabled language:

- Given a preview-language file when the language is disabled by config, no
  preview diagnostics are projected.
- `ripr: Show Status` explains the disabled language state and safe next
  config action when known.

Wrong root:

- Given a report whose workspace root does not match the active workspace, the
  editor ignores the report for diagnostics, hover, and actions.
- Status may report wrong-root evidence was ignored.

Stale artifact:

- Given stale evidence, the editor suppresses gap actions except refresh.
- Status and hover prefer stale guidance over repair instructions.

No actionable gap:

- Given validated evidence that reports no actionable gap or already observed
  behavior, the editor shows no repair action and explains the no-action state.

## Test Mapping

Follow-up PRs should add or update:

- `crates/ripr/src/lsp/tests.rs` for contract and fail-closed behavior;
- `editors/vscode/test/suite/extension.test.ts` for live extension smoke;
- `fixtures/editor_gap_cockpit/*` for pinned workflow artifacts;
- `cargo xtask lsp-cockpit-report` coverage for gap projection;
- `cargo xtask check-output-contracts` only when an owning output contract
  changes outside Lane 3.

This docs PR does not add behavior tests.

## Implementation Mapping

Planned Lane 3 slices:

1. `docs(lane3): open editor gap cockpit source-of-truth stack`
2. `test(lsp): pin post-campaign editor contract`
3. `lsp(gap): add read-only gap artifact validation`
4. `lsp(gap): project gap state in Show Status`
5. `lsp(gap): enrich hover with repair route`
6. `lsp(gap): add bounded repair packet actions`
7. `fixtures(editor): add gap cockpit workflow fixtures`
8. `test(vscode): smoke editor gap cockpit`
9. `docs(editor): document gap cockpit workflow`
10. `dogfood(lane3): record editor gap cockpit receipts`
11. `campaign(lane3): close editor gap cockpit`

Implementation must preserve the boundary that other lanes provide evidence,
policy, PR/CI, and receipt truth while Lane 3 renders editor projections.

## Metrics

Future implementation should add metrics only when they are backed by code and
fixtures. Candidate metric families:

- `editor_gap_artifacts_seen`;
- `editor_gap_artifacts_validated`;
- `editor_gap_artifacts_rejected_wrong_root`;
- `editor_gap_artifacts_rejected_stale`;
- `editor_gap_artifacts_rejected_unsupported_schema`;
- `editor_gap_artifacts_rejected_missing_identity`;
- `editor_gap_actions_projected`;
- `editor_gap_actions_suppressed_stale`;
- `editor_gap_actions_suppressed_path_escape`;
- `editor_gap_status_actionable`;
- `editor_gap_status_no_action`;
- `editor_gap_status_preview_limited`.
