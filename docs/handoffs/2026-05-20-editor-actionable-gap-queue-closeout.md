# Handoff: Editor Actionable Gap Queue Closeout

Date: 2026-05-20

Branch: `campaign/editor-actionable-gap-queue-closeout`

Latest merged PR: #38 `dogfood(lane3): record actionable gap queue receipts`
(commit `8dc36ce209f625ce44b4811c0fe0cbef3bfd0c7c`)

## Current Work Item

`campaign/lane3-actionable-gap-queue-closeout`

Editor Actionable Gap Queue made the closed Lane 3 editor cockpit project the
current repo repair queue from existing typed `actionable-gaps` artifacts:

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

The campaign stayed saved-workspace first, read-only, projection-only, and
typed-fields-over-prose. It did not add analyzer truth, actionable-gaps
producer or schema behavior, PR/CI summaries, policy or gate behavior, release
behavior, source edits, generated tests, provider/model calls, mutation
execution, CodeLens, inlay hints, semantic tokens, inline patches, unsaved
overlays, or automatic repair.

## Prompt-To-Artifact Audit

| Requirement | Artifact | Validation |
| --- | --- | --- |
| Source-of-truth stack exists | Source #1310 added `RIPR-PROP-0013`, `RIPR-SPEC-0055`, ADR 0017, the implementation plan, indexes, lane tracker state, traceability, and capability wiring. | `cargo xtask check-doc-index`, `cargo xtask markdown-links`, `cargo xtask check-doc-roles`, `cargo xtask check-traceability` |
| Post-adoption editor contract remains green | Swarm #6 pinned Diagnose Setup, Show Status, Rust diagnostics, preview labels, first repair packet, first-pr packet, receipt projection, wrong-root, stale, direct repair no-active-editor, and multi-root fail-closed behavior. | `cargo test -p ripr lsp --lib`, `cargo test -p ripr lsp::tests --lib`, `cargo xtask lsp-cockpit-report`, VS Code e2e |
| Actionable-gaps packet validation exists | Swarm #11 validates `target/ripr/reports/actionable-gaps.json` as a read-only input seam with missing, malformed, unsupported, wrong-root, missing identity, unsafe path, unsafe command, stale, producer-exclusion, and limitation guards. | LSP tests and VS Code queue validation tests |
| Show Status projects queue state | Swarm #13 projects top repair, related test, verify command, receipt state, report-only counts, static-limit counts, and no-action/fail-closed state without producing packets. | VS Code status model tests and `cargo xtask lsp-cockpit-report` |
| Copy Current Repair Packet is bounded | Swarm #14 adds one repair packet action only for validated actionable gaps with root, identity, repair route, safe paths, safe commands, and freshness. | VS Code command tests and e2e queue smoke |
| Copy Repo Gap Map is read-only | Swarm #15 adds repo-level orientation with top gaps, report-only/static-limit groups, receipt state, first-pr state, refresh guidance, and non-claims. | VS Code command tests and e2e queue smoke |
| Fixtures cover success and fail-closed states | Swarm #16 adds `fixtures/editor_actionable_gap_queue` for setup-ok, top-gap-ready, multiple-gaps-bounded, no-action, static-limit-only, stale, wrong-root, malformed, receipt-improved, and receipt-unchanged states. | `cargo xtask fixtures`, `cargo xtask goldens check`, `cargo xtask check-fixture-contracts`, `cargo xtask lsp-cockpit-report` |
| Real VS Code path is smoke-tested | Swarm #26 proves extension activation, server resolution, queue status, current repair packet copy, repo gap map copy, stale/wrong-root/malformed suppression, receipt state, Rust default behavior, and preview boundaries. | `npm --prefix editors/vscode run compile`, `npm --prefix editors/vscode run test:e2e` |
| User workflow is documented | Swarm #33 adds `docs/EDITOR_ACTIONABLE_GAP_QUEUE.md` for the queue workflow, recovery states, agent handoff, verify/receipt/refresh path, and non-claims. | Docs checks, static-language check, traceability, and capability checks |
| Dogfood receipts prove use | Swarm #38 adds `docs/handoffs/2026-05-20-editor-actionable-gap-queue-receipts.md` for actionable, multiple-gap, no-action, static-limit-only, receipt, wrong-root, stale, malformed, and preview-boundary proof. | LSP cockpit report, VS Code compile/e2e, docs checks, traceability, capability checks, `cargo xtask check-pr` |

## PR Chain

- source #1310 `docs(lane3): open editor actionable gap queue stack`
- swarm #6 `test(lsp): pin post-adoption editor contract`
- swarm #11 `lsp(queue): validate actionable gap packet artifacts`
- swarm #13 `lsp(queue): project repair queue in Show Status`
- swarm #14 `lsp(queue): add Copy Current Repair Packet`
- swarm #15 `lsp(queue): add Copy Repo Gap Map`
- swarm #16 `fixtures(editor): add actionable gap queue corpus`
- swarm #21 `docs(lane3): point queue follow-ups to swarm issues`
- swarm #26 `test(vscode): smoke actionable gap queue`
- swarm #33 `docs(editor): document actionable gap queue`
- swarm #38 `dogfood(lane3): record actionable gap queue receipts`
- `campaign(lane3): close editor actionable gap queue`

## Closeout Validation

Closeout validation for this PR:

```bash
cargo xtask goals next
cargo xtask lsp-cockpit-report
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-doc-roles
cargo xtask check-pr
git diff --check
```

Result: pass before PR creation on 2026-05-20.

The first VS Code e2e attempt timed out once in
`startCurrentRepair executes the nearest existing repair action` after the
extension host reported an unresponsive/responsive transition. No behavior
changes were made; the immediate rerun passed all 65 tests and exited 0.

The final dogfood receipt PR also passed:

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

## Remaining Limits

- Rust remains the stable default editor path.
- TypeScript, JavaScript, and Python remain opt-in preview evidence.
- Preview evidence remains syntax-first, advisory, and static-limit bounded.
- Static evidence does not imply runtime adequacy.
- Receipts record static artifact relationships, not mutation proof.
- The actionable gap queue is advisory orientation and repair handoff, not
  merge approval, gate decision, policy eligibility, runtime proof, mutation
  proof, or coverage adequacy.
- Missing, stale, wrong-root, malformed, unsupported, disabled, unavailable,
  unsafe, receipt-mismatched, first-pr-mismatched, and
  actionable-packet-mismatched states fail closed.
- Policy, gates, PR/CI summaries, PR comments, actionable-gaps production,
  receipts, first-pr packets, and analyzer truth remain owned by their lanes.

## Artifacts

- `docs/EDITOR_ACTIONABLE_GAP_QUEUE.md`
- `docs/handoffs/2026-05-20-editor-actionable-gap-queue-receipts.md`
- `fixtures/editor_actionable_gap_queue/`
- `target/ripr/reports/lsp-cockpit.md`
- `target/ripr/reports/lsp-cockpit.json`

## Next Work Item

No behavior-bearing Lane 3 work item is selected after this closeout. Lane 3
returns to maintenance and should review editor-impacting upstream changes
without opening speculative editor furniture.

Future editor work needs a new source-of-truth stack if it adds or changes
editor behavior, PR/CI projection, policy, gate, release/install behavior,
source edits, generated tests, provider/model calls, mutation execution,
CodeLens, inlays, semantic tokens, inline patches, automatic repair, or
unsaved-buffer overlays.

## What Not To Do

- Do not reopen Editor Actionable Gap Queue to add dashboard surfaces.
- Do not make the editor produce or re-rank `actionable-gaps` artifacts.
- Do not infer queue actionability by parsing Markdown prose.
- Do not expose repair actions for stale, wrong-root, malformed, unsupported,
  disabled, unavailable, unsafe, receipt-mismatched, first-pr-mismatched, or
  actionable-packet-mismatched states except refresh, setup diagnosis, or
  regeneration guidance.
- Do not promote preview evidence to Rust-level confidence.
- Do not add source edits, generated tests, provider calls, mutation execution,
  policy decisions, gate behavior, PR comments, generated CI composition,
  CodeLens, inlays, semantic tokens, inline patches, or unsaved-buffer overlays
  to Lane 3.
