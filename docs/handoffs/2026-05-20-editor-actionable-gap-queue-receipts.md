# Editor Actionable Gap Queue Dogfood Receipts

Date: 2026-05-20

Lane: 3, Editor / LSP UX

Work item: `dogfood/lane3-actionable-gap-queue-receipts`

Branch: `dogfood/editor-actionable-gap-queue-receipts`

Issue: [#19](https://github.com/EffortlessMetrics/ripr-swarm/issues/19)

## Scope

This receipt records dogfood proof for the Editor Actionable Gap Queue. The
queue path projects existing `actionable-gaps` artifacts into the editor so a
human or coding-agent operator can answer:

```text
What is safe to work on now?
```

The checked loop is:

```text
Diagnose Setup
-> Show Status
-> Current Repair Queue
-> Copy Current Repair Packet or Copy Repo Gap Map
-> open related test
-> verify
-> receipt
-> refresh
-> next gap or no-action
```

This receipt does not add editor behavior, analyzer truth, actionable-gaps
producer changes, PR/CI rendering, policy decisions, gates, source edits,
generated tests, provider calls, or mutation execution.

## Checked Queue Cases

| Case | Fixture / Smoke Surface | Editor State Observed | Safe Next Action |
| --- | --- | --- | --- |
| Top actionable Rust gap | `fixtures/editor_actionable_gap_queue/top_gap_ready` | `top_actionable_gap`; one Rust stable queue item with `ripr.copyCurrentRepairPacket`, `ripr.copyRepoGapMap`, related-test opening, and refresh. | Copy the bounded current repair packet for the top gap, verify, receipt, and refresh. |
| Multiple actionable gaps | `fixtures/editor_actionable_gap_queue/multiple_gaps_bounded` | `top_actionable_gap`; 3 actionable, 2 report-only, and 1 static-limit-only item; only the top repair packet is actionable. | Copy one current repair packet; use repo map only for orientation. |
| No actionable gap | `fixtures/editor_actionable_gap_queue/no_actionable_gap` | `no_action`; repair packet and related-test actions are suppressed. | Copy repo map or refresh; do not ask an agent to repair from prose. |
| Static-limit-only queue | `fixtures/editor_actionable_gap_queue/report_only_static_limit` | `static_limit_only`; report-only and static-limit evidence are visible without a repair packet. | Read the static limit and refresh; do not turn it into a repair task. |
| Receipt improved | `fixtures/editor_actionable_gap_queue/receipt_improved` and VS Code queue smoke | Receipt state is `found`, movement is `improved`, and the packet remains advisory/static. | Record movement and refresh before selecting the next gap. |
| Receipt unchanged | `fixtures/editor_actionable_gap_queue/receipt_unchanged` | Receipt state is `found`, movement is `unchanged`, and the unchanged state remains visible. | Inspect the attempted repair and refresh; do not claim runtime adequacy. |
| Wrong-root packet | `fixtures/editor_actionable_gap_queue/wrong_root_packet` | `wrong_root`; queue projection fails closed and only refresh remains. | Regenerate artifacts from the active workspace root. |
| Stale packet | `fixtures/editor_actionable_gap_queue/stale_actionable_packet` | `stale`; queue repair and repo-map actions are suppressed. | Refresh saved-workspace evidence before copying packets. |
| Malformed packet | `fixtures/editor_actionable_gap_queue/malformed_packet` | `malformed`; all queue repair orientation actions are suppressed. | Regenerate a valid `actionable-gaps.json` artifact. |
| Preview advisory packet boundary | VS Code smoke `real server surfaces preview gap diagnostic, hover, status, and bounded actions`; [Editor Actionable Gap Queue](../EDITOR_ACTIONABLE_GAP_QUEUE.md#preview-evidence) | Preview evidence remains labeled `preview`, static-limit bounded, and advisory. | Treat preview packets as bounded advisory evidence; do not promote them to Rust confidence, policy eligibility, or gate authority. |

## Artifact Paths

The dogfood evidence is fixture-backed and editor-smoke-backed:

```text
fixtures/editor_actionable_gap_queue/*/expected/vscode-status.json
fixtures/editor_actionable_gap_queue/*/expected/lsp-code-actions.json
fixtures/editor_actionable_gap_queue/*/expected/current-repair-packet.md
fixtures/editor_actionable_gap_queue/*/expected/repo-gap-map.md
fixtures/editor_actionable_gap_queue/*/expected/receipt-status.json
fixtures/editor_actionable_gap_queue/SPEC.md
target/ripr/reports/lsp-cockpit.json
target/ripr/reports/lsp-cockpit.md
```

The VS Code smoke path writes temporary workspace artifacts while testing the
packaged command path:

```text
target/ripr/reports/actionable-gaps.json
target/ripr/agent/agent-receipt.json
target/ripr/first-pr/start-here.json
```

## Receipt And Queue States

| State Family | Covered States |
| --- | --- |
| Queue | queue available, top actionable gap, multiple actionable gaps, no action, static-limit-only, malformed, stale, wrong-root. |
| Packet actions | `Copy Current Repair Packet` appears only for validated actionable gaps and is suppressed for no-action, static-limit-only, stale, wrong-root, and malformed states. |
| Repo map actions | `Copy Repo Gap Map` appears for safe orientation states and is suppressed for stale, wrong-root, and malformed states. |
| Receipt | missing, improved, unchanged, and not-projected states. |
| Preview | preview advisory evidence remains opt-in, syntax-first, static-limit labeled, and non-gating. |
| Non-claims | runtime adequacy, mutation proof, policy gate authority, and merge readiness are false in checked queue fixtures. |

## Validation

```bash
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

Result: pass at authoring.

## Limits

- Editor behavior remains saved-workspace, read-only, and projection-only.
- The editor consumes `actionable-gaps` artifacts; it does not produce or
  re-rank them.
- Queue actionability is driven by typed fields, not Markdown prose.
- Unsafe packet states suppress repair actions and make no proof claim.
- Rust remains the stable/default confidence path.
- Preview evidence remains opt-in, syntax-first, advisory, and static-limit
  bounded.
- No runtime adequacy claim.
- No mutation proof.
- No merge approval.
- No gate or policy eligibility authority.
- No PR comment publishing or generated CI summary composition.
- No source edits or generated tests.
- No provider or model calls.

## Next Work Item

`campaign(lane3): close editor actionable gap queue`

Close only after the campaign closeout maps requirements to merged artifacts,
records validation, and confirms no analyzer, schema-producer, policy, PR/CI,
release, source-edit, generated-test, provider, mutation, gate, CodeLens,
inlay, semantic-token, inline-patch, or unsaved-buffer scope landed.
