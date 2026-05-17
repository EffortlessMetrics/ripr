# RIPR-SPEC-0052: Editor First-PR Packet Projection

Status: proposed

## Problem

The editor can guide one local repair and show receipt state, while
`cargo xtask first-pr` can write a first successful PR start-here packet. The
missing behavior is a read-only editor bridge between those surfaces.

The editor should answer:

```text
Is there a first-pr packet for this workspace and gap, is it safe to inspect,
and what should I do next?
```

It should do that without producing the first-pr packet, publishing PR
comments, composing CI summaries, deciding gates, editing source, generating
tests, calling providers, running mutation testing, or claiming PR readiness.

## Behavior

Lane 3 may validate and project existing first-pr packet artifacts:

```text
target/ripr/first-pr/start-here.json
target/ripr/first-pr/start-here.md
target/ripr/reports/start-here.json
target/ripr/reports/start-here.md
```

The JSON packet is the typed source for editor state. The Markdown packet is a
human artifact the editor may open or point to after path validation. The editor
must not parse Markdown prose for action semantics.

### Packet States

`ripr: Diagnose Setup` and `ripr: Show Status` should be able to project these
states:

| State | Required behavior |
| --- | --- |
| `first_pr_missing` | Explain that no first-pr packet was found and show a regeneration command when known. |
| `first_pr_found` | Show the packet path and advisory status. |
| `first_pr_top_repairable_gap` | Show that a top repairable gap is available when typed packet fields support it. |
| `first_pr_no_action` | Explain the no-action state without claiming runtime adequacy or general correctness. |
| `first_pr_stale` | Explain that packet evidence is stale and suppress repair/open-copy actions except refresh or regeneration guidance. |
| `first_pr_wrong_root` | Fail closed and explain expected versus observed workspace root when known. |
| `first_pr_malformed` | Fail closed and name the malformed artifact and parser error summary. |
| `first_pr_gap_mismatch` | Fail closed when the packet gap identity does not match the current diagnostic. |
| `first_pr_unsafe_path` | Suppress open actions when packet or related paths are outside the workspace. |
| `first_pr_unsafe_command` | Suppress copy-command actions when command payloads are malformed or outside the current workspace contract. |

### Validation Rules

Before projecting stronger state or actions, the editor must validate:

- supported schema version;
- workspace root matches the current workspace;
- freshness or explicit stale state is known;
- state is one of the supported packet states;
- gap identity is present when a top repairable gap is selected;
- current diagnostic identity matches the packet gap identity when the action is
  diagnostic-scoped;
- related paths and packet paths are workspace-local;
- command payloads are safe, bounded, and reference the current workspace;
- language is stable Rust or an enabled preview language;
- preview language status and static limits remain visible when present.

If validation fails, the editor fails closed. It may explain the state and show
refresh, setup diagnosis, or regeneration guidance, but it must suppress
repair, open, and copy actions that depend on the failed validation.

### Status Projection

Status is explanatory, not authoritative. It may say:

```text
First PR packet: missing
First PR packet: found
First PR packet: stale
First PR packet: wrong root
First PR packet: malformed
First PR packet: no actionable gap
First PR packet: top repairable gap available
```

Status must not say that a PR is merge-ready, gate-passing, mutation-proven, or
runtime-adequate unless a separate owned artifact explicitly carries that
authority. A first-pr packet is advisory unless a gate-decision artifact is
linked as an external authority.

### Bounded Actions

The editor may expose these actions only when validation supports them:

| Action | Required evidence |
| --- | --- |
| Open first-pr packet | Workspace-local Markdown packet path exists and root matches. |
| Copy first-pr summary | Typed packet state, advisory boundary, and selected item or no-action reason exist. |
| Copy first-pr repair packet | Top repairable gap, repair route, verify command, receipt command, and gap identity exist. |
| Copy verify command | Safe command payload references the current workspace. |
| Copy receipt command | Safe verify/receipt chain exists and references the current workspace. |
| Refresh | Server is available enough to request refresh. |
| Regenerate first-pr packet guidance | A safe regeneration command is known or documented. |

Missing, stale, wrong-root, malformed, gap-mismatched, path-unsafe, and
command-unsafe packets suppress all actions except refresh, setup diagnosis, or
regeneration guidance.

### Preview Boundary

For TypeScript, JavaScript, and Python preview evidence, projection must remain:

- opt-in;
- syntax-first;
- advisory;
- `language_status = "preview"` visible;
- static-limit labeled when present;
- no runtime adequacy claim;
- no mutation proof claim;
- no policy eligibility or gate authority claim.

Static limits must appear before suggested action language.

## Required Evidence

Future implementation must provide:

- LSP tests for first-pr packet validation and fail-closed states;
- status tests for packet missing, found, no-action, stale, wrong-root, and
  malformed states;
- action tests proving actions appear only with safe typed packet fields;
- fixtures for packet success and fail-closed states;
- VS Code e2e smoke for Diagnose Setup, Show Status, open/copy actions, and
  wrong-root/stale/malformed suppression;
- docs explaining what the first-pr packet proves and does not prove;
- dogfood receipts using real first-pr artifacts.

## Inputs

The editor may consume:

- first-pr packet JSON and Markdown artifacts;
- diagnostics and `diagnostic.data`;
- gap records and evidence records;
- first-useful-action reports;
- repair cards and repair routes;
- related-test facts;
- verify and receipt commands;
- existing receipt artifacts;
- workspace root, config, language, server, freshness, and setup state;
- preview language metadata and static-limit metadata.

## Outputs

Lane 3 may output:

- setup diagnosis text that includes first-pr packet state;
- `ripr: Show Status` first-pr packet state;
- hover text that points from a current diagnostic to the matching packet when
  validated;
- bounded code actions for opening or copying first-pr packet content;
- fixture artifacts;
- VS Code smoke coverage;
- docs and dogfood handoff receipts.

Lane 3 must not output analyzer facts, first-pr packet artifacts, generated CI
summaries, PR comments, source edits, generated tests, provider results,
mutation results, gate decisions, or policy changes.

## Non-Goals

- No analyzer changes.
- No first-pr packet producer changes.
- No generated CI summary composition.
- No PR comment publishing.
- No gate, policy, default-blocking, badge, baseline, waiver, or suppression
  changes.
- No source edits, inline patches, or automatic repair application.
- No generated tests.
- No provider or model calls.
- No mutation execution.
- No runtime adequacy, mutation proof, or merge-readiness claim.
- No CodeLens, inlay hints, semantic tokens, inline patches, or unsaved-buffer
  overlays.
- No preview-language promotion to stable Rust confidence.

## Acceptance Examples

Packet missing:

- Given no first-pr packet artifact, Diagnose Setup and Show Status report the
  packet as missing and show regeneration guidance when known.
- No first-pr open/copy repair actions appear.

Repairable Rust packet:

- Given a validated workspace-local packet with a top repairable Rust gap,
  repair route, verify command, receipt command, and matching diagnostic gap
  identity, status reports the top gap and code actions may open the packet or
  copy a bounded first-pr repair packet.

No-action packet:

- Given a validated no-action packet, status reports no actionable gap and does
  not imply runtime adequacy, coverage adequacy, mutation adequacy, or merge
  readiness.

Stale packet:

- Given a stale first-pr packet, status reports stale state and suppresses
  packet repair actions except refresh or regeneration guidance.

Wrong-root packet:

- Given a packet from a different workspace root, the editor fails closed,
  reports wrong-root state, and suppresses open/copy actions.

Malformed packet:

- Given malformed packet JSON, the editor fails closed, reports malformed
  state, and treats Markdown as a human artifact only when path-safe.

Preview static limit:

- Given a preview-language first-pr packet with a static limit, status, hover,
  and copied text show preview status and the static limit before suggested
  action language.

## Test Mapping

Follow-up PRs should add or update:

- `crates/ripr/src/lsp/tests.rs` for packet validation, status states, and
  action suppression;
- `crates/ripr/src/lsp/actions.rs` for bounded first-pr packet actions;
- `editors/vscode/test/suite/extension.test.ts` for live Diagnose Setup,
  Show Status, open/copy action, and fail-closed smoke;
- `fixtures/editor_first_pr_bridge/*` for packet success and failure states;
- `cargo xtask lsp-cockpit-report` coverage for first-pr packet state.

This docs PR does not add behavior tests.

## Implementation Mapping

Planned slices:

1. `docs(lane3): open editor first-pr bridge source-of-truth stack`
2. `test(lsp): pin post-first-run editor contract`
3. `lsp(first-pr): validate first-pr packet artifacts`
4. `lsp(first-pr): project first-pr state in Show Status`
5. `lsp(first-pr): add bounded first-pr packet actions`
6. `fixtures(editor): add first-pr bridge fixtures`
7. `test(vscode): smoke first-pr bridge`
8. `docs(editor): document first successful PR bridge`
9. `dogfood(lane3): record first-pr bridge receipts`
10. `campaign(lane3): close editor first-pr bridge`

## Metrics

Future implementation may add metrics only when backed by code and traceable
tests. Candidate metrics:

- `editor_first_pr_packet_missing`;
- `editor_first_pr_packet_found`;
- `editor_first_pr_packet_stale`;
- `editor_first_pr_packet_wrong_root`;
- `editor_first_pr_packet_malformed`;
- `editor_first_pr_packet_no_action`;
- `editor_first_pr_top_gap_available`;
- `editor_first_pr_actions_suppressed_unsafe_state`;
- `editor_first_pr_open_packet_actions`;
- `editor_first_pr_copy_packet_actions`.
