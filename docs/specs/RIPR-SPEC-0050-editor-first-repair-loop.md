# RIPR-SPEC-0050: Editor First Repair Loop

Status: accepted

## Problem

The editor can project gap records and bounded actions, but a first-time user
still needs a clear path from one diagnostic to one receipt. The first repair
loop should make the safe sequence visible without requiring the user to know
RIPR's internal artifact model.

The editor should answer:

```text
What is the first focused repair, what command verifies it, and what receipt
records movement?
```

## Behavior

The first repair loop is a read-only editor projection over existing
diagnostics, evidence records, gap records, repair routes, verify commands,
receipt commands, and receipt artifacts.

The intended Rust-first flow is:

```text
open workspace
-> read RIPR status
-> inspect diagnostic
-> hover for evidence and gap state
-> open related test or copy first repair packet
-> write one focused test
-> run verify command
-> run receipt command
-> refresh evidence
-> read receipt status
```

### First repair packet

The editor may expose a `Copy first repair packet` action when the current
validated artifact provides enough typed evidence. The packet should include:

- gap identity;
- language and language status;
- static limit, when present;
- related test or repair target, when safe;
- suggested action;
- verify command;
- receipt command;
- limits and non-claims.

The packet must be scoped to one gap and one repair route. It must not ask the
recipient to edit production code unless the source artifact explicitly scopes
that work. It must not imply runtime adequacy, gate eligibility, or mutation
runtime confirmation.

### Receipt status

`ripr: Show Status` may surface receipt state when existing receipt artifacts
are available:

| Receipt state | Required behavior |
| --- | --- |
| `receipt_found` | Show that a receipt exists for the current gap and workspace. |
| `receipt_missing` | Explain that no matching receipt was found. |
| `receipt_stale` | Explain that the receipt is older than the current evidence and fail closed. |
| `receipt_gap_mismatch` | Explain that the receipt does not match the current gap identity and fail closed. |
| `receipt_movement_improved` | Show that the existing receipt records improved movement for the current gap. |
| `receipt_movement_unchanged` | Show that the existing receipt records unchanged movement and suggest the next safe inspection step when known. |

Receipt status must consume existing receipt artifacts only. This spec does
not add a new receipt producer.

### Action bounds

Code actions should remain conditional:

| Action | Required evidence |
| --- | --- |
| Open related test | Workspace-local, current-language path exists. |
| Copy first repair packet | Gap identity, repair route, verify command, and receipt command exist. |
| Copy verify command | Safe command references the current workspace. |
| Copy receipt command | Verify/receipt chain exists and references the current workspace. |
| Refresh evidence | Server is available enough to request refresh. |

Stale or invalid artifacts suppress repair actions except refresh or setup
guidance. Wrong-root receipts, stale receipts, malformed receipts, and
gap-mismatched receipts must not produce movement claims.

## Required Evidence

Future implementation must provide:

- tests that receipt status consumes only existing artifacts;
- tests that stale, wrong-root, malformed, and gap-mismatched receipts fail
  closed;
- tests that first repair packet actions are absent when required typed fields
  are missing;
- fixtures for receipt improved and receipt unchanged states;
- VS Code e2e coverage for copying the first repair packet and seeing receipt
  status;
- docs that explain what a receipt records and what it does not claim;
- dogfood receipts for one first Rust repair path.

## Inputs

The first repair loop may consume:

- LSP diagnostics and `diagnostic.data`;
- evidence records;
- gap records;
- first-useful-action reports;
- repair routes;
- related-test facts;
- static-limit metadata;
- verify commands;
- receipt commands;
- existing receipt artifacts;
- workspace-root and freshness state.

## Outputs

Lane 3 may output:

- hover sections for first repair guidance;
- `ripr: Show Status` receipt state;
- copyable first repair packet text;
- copyable verify and receipt commands;
- related-test open commands;
- fixture artifacts;
- `lsp-cockpit-report` receipt and action coverage;
- user docs and dogfood handoff receipts.

Lane 3 must not output new analyzer facts, new receipt artifacts, policy
decisions, gate decisions, PR comments, generated tests, source edits,
provider calls, or mutation runs.

## Non-Goals

- No analyzer changes.
- No new receipt producer.
- No evidence schema invention in the editor.
- No automatic source edits.
- No generated tests.
- No provider or model calls.
- No runtime mutation execution.
- No policy, gate, default-blocking, badge, baseline, waiver, or suppression
  changes.
- No PR comment publishing or generated CI summary composition.
- No CodeLens, inlay hints, semantic tokens, inline patches, or
  unsaved-buffer overlays.
- No preview-language promotion to stable Rust confidence.

## Acceptance Examples

Actionable Rust gap:

- Given a validated Rust diagnostic with gap identity, repair route, related
  test, verify command, and receipt command, the editor can show the first
  repair packet action and copy a bounded packet.

Missing receipt:

- Given no matching receipt artifact, status says receipt missing and does not
  claim movement.

Improved receipt:

- Given a matching current receipt with improved movement, status says receipt
  found and movement improved.

Unchanged receipt:

- Given a matching current receipt with unchanged movement, status says
  movement unchanged and does not claim repair success.

Gap mismatch:

- Given a receipt whose gap identity does not match the current diagnostic,
  status reports the mismatch and suppresses movement claims.

Preview static limit:

- Given a preview-language gap packet with a static limit, the packet and hover
  show preview status and static limit before suggested action language.

## Test Mapping

Follow-up PRs should add or update:

- `crates/ripr/src/lsp/actions.rs` and related tests for first repair packet
  action bounds;
- `crates/ripr/src/lsp/tests.rs` for receipt state and fail-closed behavior;
- `editors/vscode/test/suite/extension.test.ts` for live copy-command and
  receipt-status smoke;
- `fixtures/editor_first_run_usability/*` for receipt status fixtures;
- `cargo xtask lsp-cockpit-report` coverage for first repair packets and
  receipt status.

This docs PR does not add behavior tests.

## Implementation Mapping

Planned slices:

1. `docs(lane3): open editor first-run usability stack`
2. `lsp: link receipts into Show Status`
3. `lsp: add first-repair action packet`
4. `fixtures(editor): add first-run usability fixtures`
5. `test(vscode): smoke first-run and no-output states`
6. `docs(editor): write first-run-to-first-receipt guide`
7. `dogfood(lane3): record first-run repair receipts`
8. `campaign(lane3): close editor first-run usability`

Setup and no-output behavior is defined in `RIPR-SPEC-0049`.

## Metrics

Future implementation may add metrics only when backed by code and traceable
tests. Candidate metrics:

- `editor_first_repair_packets_copied`;
- `editor_first_repair_packets_suppressed_missing_identity`;
- `editor_receipt_status_found`;
- `editor_receipt_status_missing`;
- `editor_receipt_status_stale`;
- `editor_receipt_status_gap_mismatch`;
- `editor_receipt_status_movement_improved`;
- `editor_receipt_status_movement_unchanged`.
