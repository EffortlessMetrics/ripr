# Editor First-PR Bridge Dogfood Receipts

Date: 2026-05-17

Lane: 3, Editor / LSP UX

Work item: `dogfood/lane3-first-pr-bridge-receipts`

Branch / PR: `dogfood-editor-first-pr-bridge-receipts` / #1116

## Scope

This receipt records the editor first-pr bridge proof loop using existing,
fixture-backed first-pr packet projections. It proves that the editor can carry
the user from local repair state to first-pr packet inspection without changing
analyzer behavior, first-pr packet production, PR or CI rendering, policy,
gates, source files, generated tests, provider behavior, mutation execution, or
PR comment publishing.

The checked handoff loop is:

```text
Diagnose Setup
-> Show Status
-> inspect diagnostic
-> copy first repair packet
-> verify
-> receipt
-> refresh
-> inspect first-pr packet
```

## Checked Editor First-PR Cases

| Case | Packet state | Purpose |
| --- | --- | --- |
| `setup_ok` | `found` | Packet can be opened or summarized, but no diagnostic-scoped repair action is projected. |
| `packet_missing` | `missing` | Missing packet fails closed and leaves regeneration guidance. |
| `packet_found_repairable` | `top_repairable_gap` | Repairable packet exposes bounded packet, verify, receipt, and regeneration actions. |
| `packet_no_action` | `no_action` | No-action packet remains inspectable without repair, verify, or receipt copy actions. |
| `packet_stale` | `stale` | Stale packet fails closed and suppresses packet-derived actions. |
| `packet_wrong_root` | `wrong_root` | Wrong-root packet fails closed and suppresses packet-derived actions. |
| `packet_malformed` | `malformed` | Malformed packet fails closed and suppresses packet-derived actions. |
| `receipt_improved_packet_ready` | `top_repairable_gap` | Improved receipt is visible with the packet without claiming PR readiness. |
| `receipt_unchanged_packet_ready` | `top_repairable_gap` | Unchanged receipt is visible with the packet and remains advisory. |

## Validation

```bash
cargo test -p xtask dogfood_editor_first_pr_bridge
cargo xtask check-fixture-contracts
cargo xtask dogfood
```

Result: pass.

The dogfood report writes advisory receipts to:

```text
target/ripr/reports/dogfood.json
target/ripr/reports/dogfood.md
```

The report now records:

```text
target/ripr/reports/dogfood.json -> editor_first_pr_bridge
target/ripr/reports/dogfood.md -> Editor First-PR Bridge Receipts
```

The `editor_first_pr_bridge` section recorded 9 cases with zero errors:

| State | Count |
| --- | ---: |
| `found` | 1 |
| `missing` | 1 |
| `top_repairable_gap` | 3 |
| `no_action` | 1 |
| `stale` | 1 |
| `wrong_root` | 1 |
| `malformed` | 1 |

Receipt movement coverage:

| Movement | Count |
| --- | ---: |
| `improved` | 1 |
| `unchanged` | 1 |

## Limits

- Editor behavior remains saved-workspace and read-only.
- The editor consumes existing first-pr packet artifacts; it does not produce
  them.
- Unsafe packet states suppress open, summary, repair, verify, and receipt copy
  actions except regeneration guidance.
- Rust default behavior is unchanged.
- Preview evidence remains opt-in, advisory, and static-limit bounded.
- No runtime adequacy claim.
- No mutation proof.
- No merge approval or PR readiness claim.
- No policy or gate authority.
- No source edits or generated tests.
- No provider or model calls.
- No PR comment publishing or generated CI summary composition.

## Next Work Item

`campaign(lane3): close editor first-pr bridge`

Close only after the campaign closeout maps the prompt requirements to landed
artifacts, records the full validation run, and confirms no analyzer, policy,
PR-comment, CI-summary, source-edit, generated-test, provider, mutation, gate,
CodeLens, inlay, semantic-token, inline-patch, or unsaved-overlay scope landed.
