# RIPR-SPEC-0055: Editor Actionable Gap Queue

Status: accepted

## Problem

The editor can explain setup, root, compatibility, one diagnostic, receipt
state, and first-pr packet state. It does not yet expose the current workspace
repair queue emitted by Lane 1's `actionable-gaps` artifacts.

Users need the editor to answer:

```text
What is safe to work on now?
What is actionable?
What is report-only?
What is blocked by static limits or unsafe artifact state?
What verifies movement?
```

The editor must answer by validating and projecting existing typed artifacts.
It must not rederive analyzer truth, parse prose for action semantics, mutate
artifacts, decide policy, publish PR/CI output, edit source, generate tests,
call providers, run mutation, or claim runtime adequacy.

## Behavior

Editor actionable gap queue projection is read-only consumption of upstream
repair-queue artifacts.

Given a valid actionable-gap artifact for the current workspace, the editor can
project:

- queue state in `ripr: Show Status`;
- the top actionable repair;
- the current repair packet;
- a read-only repo gap map;
- receipt state when available;
- first-pr packet state when available;
- refresh and regeneration guidance.

Given invalid, unsafe, stale, or mismatched artifacts, the editor suppresses
repair actions and explains why.

Text explains. Typed fields decide. No action may be driven by parsing prose
from Markdown, hover text, status text, or comments.

### Consumed artifacts

Lane 3 consumes these artifacts when present:

- `target/ripr/reports/actionable-gaps.json`;
- `target/ripr/reports/actionable-gaps.md`;
- `target/ripr/reports/start-here.json`;
- `target/ripr/first-pr/start-here.json`;
- `target/ripr/reports/first-useful-action.json`;
- `target/ripr/agent/agent-receipt.json`;
- saved-workspace diagnostics;
- gap-ledger artifacts.

Lane 1 owns the `actionable-gaps` producer and schema. Lane 3 owns validation
and projection rules for consuming that schema. If the editor needs a new field
for action safety, the field belongs upstream first. Until that typed field
exists, the editor fails closed.

### Required typed fields

The editor may use these typed fields when present and valid:

| Field | Purpose |
| --- | --- |
| `canonical_gap_id` | Stable queue identity and receipt/first-pr matching. |
| `seam_id` | Compatibility with saved-workspace seam diagnostics. |
| `finding_id` | Compatibility with older finding-backed diagnostics. |
| `language` | Language for routing, preview labeling, and disabled-language checks. |
| `language_status` | Stable, preview, disabled, unavailable, or unsupported boundary. |
| `gap_state` | Actionable, report-only, suppressed, waived, baseline, or no-action state. |
| `repair_kind` | User-readable repair family for queue grouping. |
| `repair_route` | Structured route required before interrupting with a repair packet. |
| `target_test` | Likely target test path or missing-target state. |
| `target_assertion_shape` | Assertion or output-proof shape the repair should add. |
| `related_test` | Related observer/test path when one exists. |
| `static_limit_kind` | Structured limitation kind shown before action language. |
| `verify_command` | Safe command required before copying a repair packet. |
| `receipt_command` | Safe command for recording movement when available. |
| `receipt_movement` | Missing, improved, unchanged, stale, mismatched, or unknown receipt state. |
| `confidence_basis` | Why the queue item is actionable or bounded. |
| `artifact_freshness` | Fresh, stale, unknown, or unsupported freshness state. |
| `workspace_root` | Root used to validate paths, commands, receipts, and first-pr packets. |

### Validation and fail-closed matrix

The editor must validate root, schema, identity, language, freshness, paths,
commands, receipt state, first-pr state, and actionable-packet state before
showing repair actions.

| State | Status behavior | Allowed actions | Repair packet | Repo gap map | Proof claim |
| --- | --- | --- | --- | --- | --- |
| Missing artifact | Explain that `actionable-gaps.json` is missing and show regeneration guidance. | Refresh, Diagnose Setup, regeneration guidance. | Suppressed. | Suppressed or unavailable summary only. | None. |
| Malformed artifact | Explain malformed input and the artifact path. | Refresh, Diagnose Setup, regeneration guidance. | Suppressed. | Suppressed. | None. |
| Unsupported schema | Explain unsupported schema and supported versions when known. | Refresh, Diagnose Setup, regeneration guidance. | Suppressed. | Suppressed. | None. |
| Wrong workspace root | Explain expected and observed roots when known. | Refresh, Diagnose Setup, regeneration guidance. | Suppressed. | Suppressed or shown only as ignored evidence. | None. |
| Stale artifact | Explain stale state and next refresh command. | Refresh, Diagnose Setup, regeneration guidance. | Suppressed. | Read-only stale orientation only when safe. | None. |
| Missing `canonical_gap_id` | Explain missing identity for actionable item. | Refresh, Diagnose Setup, regeneration guidance. | Suppressed. | Report-only orientation only. | None. |
| Missing repair route | Explain that no structured repair route is available. | Refresh, Diagnose Setup, regeneration guidance. | Suppressed. | Report-only orientation only. | None. |
| Missing verify command | Explain that movement cannot be verified from typed fields. | Refresh, Diagnose Setup, regeneration guidance. | Suppressed. | Report-only orientation only. | None. |
| Unsafe command payload | Explain unsafe command payload without rendering it as executable. | Refresh, Diagnose Setup, regeneration guidance. | Suppressed. | Suppressed or redacted orientation only. | None. |
| Path escape | Explain unsafe path or path outside the workspace. | Refresh, Diagnose Setup, regeneration guidance. | Suppressed. | Suppressed or redacted orientation only. | None. |
| Disabled language | Explain disabled language and configured languages. | Refresh, Diagnose Setup, configuration guidance. | Suppressed. | Report-only orientation only. | None. |
| Unavailable adapter | Explain unavailable adapter and preview boundary. | Refresh, Diagnose Setup, configuration guidance. | Suppressed. | Report-only orientation only. | None. |
| Receipt mismatch | Explain receipt gap/root mismatch. | Refresh, Diagnose Setup, receipt regeneration guidance. | Suppressed when packet identity depends on the mismatched receipt. | Read-only orientation with mismatch state. | None. |
| First-pr mismatch | Explain first-pr packet gap/root mismatch. | Refresh, Diagnose Setup, first-pr regeneration guidance. | Suppressed when first-pr identity is required for the action. | Read-only orientation with mismatch state. | None. |
| Actionable packet mismatch | Explain mismatch between diagnostics, queue item, receipt, or first-pr packet. | Refresh, Diagnose Setup, regeneration guidance. | Suppressed. | Read-only orientation with mismatch state. | None. |

Fail closed means the editor explains the state, suppresses stronger repair
actions, offers refresh/setup/regeneration guidance when safe, and makes no
proof, runtime, mutation, gate, policy, or merge-readiness claim.

### Show Status queue projection

`ripr: Show Status` may include a bounded queue section:

```text
Actionable gaps: 3
Top repair: missing boundary assertion
Related test: tests/pricing.rs
Verify: cargo ...
Receipt: missing / improved / unchanged / stale
Report-only gaps: 12
Static-limit-only gaps: 5
Next safe action: copy current repair packet
```

Rules:

- show only a small top slice of the queue;
- prefer no-action clarity over noise;
- never imply gate pass/fail;
- never imply runtime adequacy, mutation proof, policy eligibility, or merge
  readiness;
- do not re-rank independently from upstream artifact order;
- if local filtering is later added, upstream queue order remains
  authoritative, the editor may only filter by current workspace or current
  file context, and the status text must disclose the filtered view.

### `ripr: Copy Current Repair Packet`

This action is available only when:

- a validated actionable gap packet exists;
- the workspace root matches;
- `canonical_gap_id` exists;
- a structured repair route exists;
- a safe verify command exists;
- the packet is fresh;
- related and target paths are workspace-local when present;
- command payloads are safe.

Payload sections:

- Task;
- Context;
- Repair;
- Verification;
- Receipt;
- Stop conditions;
- Do not do.

Suppress the action when the selected item is report-only, static-limit-only,
preview-unavailable, stale, wrong-root, malformed, unsupported, missing a
verify command, missing a repair route, path-unsafe, command-unsafe, or
identity-mismatched.

### `ripr: Copy Repo Gap Map`

This command is read-only orientation. It may include:

- top actionable gaps;
- report-only and static-limit groups;
- receipt states;
- preview boundaries;
- refresh and regeneration commands;
- non-claims.

It must never imply:

- gate pass/fail;
- merge readiness;
- runtime proof;
- mutation proof;
- policy eligibility.

## Required Evidence

Future implementation must provide:

- LSP tests that pin the post-adoption editor contract before queue behavior
  changes;
- artifact validation tests for success and fail-closed states;
- Show Status tests for actionable, no-action, report-only, static-limit,
  stale, wrong-root, malformed, receipt, and first-pr states;
- code action tests for Copy Current Repair Packet and Copy Repo Gap Map;
- fixtures for the queue corpus named below;
- VS Code e2e smoke for the packaged extension path;
- docs explaining the workflow and non-claims;
- dogfood receipts proving actionable, no-action, static-limit-only,
  wrong-root, stale, receipt-improved, receipt-unchanged, and preview-advisory
  states.

## Inputs

- `target/ripr/reports/actionable-gaps.json`;
- `target/ripr/reports/actionable-gaps.md`;
- `target/ripr/reports/start-here.json`;
- `target/ripr/first-pr/start-here.json`;
- `target/ripr/reports/first-useful-action.json`;
- `target/ripr/agent/agent-receipt.json`;
- saved-workspace diagnostics;
- gap-ledger artifacts;
- workspace roots and selected workspace state;
- enabled language and adapter availability state;
- receipt and first-pr packet state already projected by Lane 3.

## Outputs

Lane 3 may output:

- Show Status queue summary;
- bounded copy action payloads;
- current repair packet text;
- repo gap map text;
- hover/status explanation that uses typed fields;
- fixture artifacts;
- VS Code smoke assertions;
- docs and dogfood handoff receipts.

Lane 3 must not output analyzer facts, `actionable-gaps` artifacts, first-pr
packets, generated CI summaries, PR comments, source edits, generated tests,
provider results, mutation results, gate decisions, policy changes, release
artifacts, or runtime proof claims.

## Non-Goals

- No analyzer changes.
- No `actionable-gaps` producer or schema changes.
- No hidden analysis reruns from the editor.
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

Top gap ready:

- Given a fresh, current-root `actionable-gaps.json` with one actionable Rust
  gap, a canonical gap id, repair route, related test, safe verify command,
  and safe receipt command, Show Status names the top repair and Copy Current
  Repair Packet is available.

Multiple gaps:

- Given multiple actionable gaps, Show Status shows only a bounded top slice
  in upstream queue order and exposes one current repair packet at a time.

No actionable gap:

- Given no actionable gaps, Show Status explains no-action state and suppresses
  repair packets.

Report-only static limit:

- Given report-only static-limit entries, Show Status may orient the user, Copy
  Repo Gap Map may describe the limitation, and Copy Current Repair Packet is
  suppressed.

Wrong root:

- Given an actionable-gaps artifact from another workspace root, Show Status
  explains the mismatch and suppresses repair packet and related-test actions.

Receipt improved:

- Given a matching receipt with improved static movement, Show Status reports
  improved movement without claiming runtime adequacy or gate success.

Receipt unchanged:

- Given a matching receipt with unchanged movement, Show Status reports
  unchanged movement and the safe next action without claiming failure or
  runtime proof.

## Fixture Corpus

The manifest-only fixture corpus lives at:

```text
fixtures/editor_actionable_gap_queue/setup_ok/
fixtures/editor_actionable_gap_queue/top_gap_ready/
fixtures/editor_actionable_gap_queue/multiple_gaps_bounded/
fixtures/editor_actionable_gap_queue/no_actionable_gap/
fixtures/editor_actionable_gap_queue/report_only_static_limit/
fixtures/editor_actionable_gap_queue/stale_actionable_packet/
fixtures/editor_actionable_gap_queue/wrong_root_packet/
fixtures/editor_actionable_gap_queue/malformed_packet/
fixtures/editor_actionable_gap_queue/receipt_improved/
fixtures/editor_actionable_gap_queue/receipt_unchanged/
```

Each case carries:

```text
vscode-status.json
lsp-code-actions.json
current-repair-packet.md
repo-gap-map.md
receipt-status.json
```

Fixture rules:

- `top_gap_ready` exposes Copy Current Repair Packet.
- `report_only_static_limit` suppresses repair packets while allowing safe
  orientation.
- `stale_actionable_packet` suppresses repair actions and shows refresh
  guidance.
- `wrong_root_packet` suppresses repair actions and explains the root
  mismatch.
- `receipt_improved` reports improved static movement without runtime
  adequacy.
- `receipt_unchanged` reports unchanged movement and a safe next action.

## Test Mapping

Current traceability includes the source-of-truth docs, queue validation tests,
VS Code command tests, and `fixtures/editor_actionable_gap_queue/*` success and
fail-closed states. Remaining behavior PRs should add:

- `crates/ripr/src/lsp/tests.rs` for artifact validation, status projection,
  fail-closed action gating, packet rendering, and repo gap map rendering;
- `editors/vscode/test/suite/extension.test.ts` for packaged-extension command
  smoke;
- `cargo xtask lsp-cockpit-report` coverage once queue state enters the
  cockpit report.

## Implementation Mapping

Planned slices:

1. `docs(lane3): open editor actionable gap queue stack`
2. `test(lsp): pin post-adoption editor contract`
3. `lsp(queue): validate actionable gap packet artifacts`
4. `lsp(queue): project repair queue in Show Status`
5. `lsp(queue): add Copy Current Repair Packet`
6. `lsp(queue): add Copy Repo Gap Map`
7. `fixtures(editor): add actionable gap queue corpus`
8. `test(vscode): smoke actionable gap queue`
9. `docs(editor): document actionable gap queue`
10. `dogfood(lane3): record actionable gap queue receipts`
11. `campaign(lane3): close editor actionable gap queue`

## Metrics

Future implementation may add metrics only when backed by code and traceable
tests. Candidate metrics:

- `editor_actionable_gap_queue_available`;
- `editor_actionable_gap_queue_top_gap_ready`;
- `editor_actionable_gap_queue_no_action`;
- `editor_actionable_gap_queue_report_only`;
- `editor_actionable_gap_queue_static_limit_only`;
- `editor_actionable_gap_queue_stale`;
- `editor_actionable_gap_queue_wrong_root`;
- `editor_actionable_gap_queue_malformed`;
- `editor_actionable_gap_queue_repair_packet_available`;
- `editor_actionable_gap_queue_repo_map_available`;
- `editor_actionable_gap_queue_actions_suppressed_unsafe_state`;
- `editor_actionable_gap_queue_receipt_improved`;
- `editor_actionable_gap_queue_receipt_unchanged`.
