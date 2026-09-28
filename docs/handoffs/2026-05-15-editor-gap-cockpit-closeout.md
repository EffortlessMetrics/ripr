# Handoff: Editor Gap Cockpit Closeout

Date: 2026-05-15
Branch / PR: `campaign-lane3-editor-gap-closeout` / pending at authoring
Latest merged PR: #998 `dogfood(lane3): record editor gap cockpit receipts`
(commit `21438bada83e24db09e00ce4eb8536cf96052aff`)

## Current Work Item

`campaign/lane3-editor-gap-cockpit-closeout`

Editor Gap Cockpit made the saved-workspace editor consume existing RIPR gap
and evidence artifacts as a local repair cockpit:

```text
diagnostic
-> hover evidence
-> gap state / preview limit / repair route
-> related test or repair packet
-> one focused test
-> verify
-> receipt
-> refresh
```

The campaign stayed read-only and projection-only. It did not add analyzer
truth, policy or gate decisions, PR comments, generated CI summaries, source
edits, generated tests, provider calls, mutation execution, default blocking,
CodeLens, inlays, semantic tokens, inline patches, or unsaved-buffer overlays.

## Prompt-To-Artifact Audit

| Requirement | Artifact | Validation |
| --- | --- | --- |
| Source-of-truth stack exists | #967 added `RIPR-PROP-0007`, `RIPR-SPEC-0047`, ADR 0012, the implementation plan, lane tracker state, indexes, and traceability | `cargo xtask check-doc-index`, `cargo xtask markdown-links`, `cargo xtask check-doc-roles` |
| Post-Campaign 27 editor contract is pinned | #970 pins Rust default behavior, preview metadata, disabled-language status, stale/wrong-root handling, related-test safety, and static-limit hover order | `cargo test -p ripr lsp --lib`, `cargo test -p ripr lsp::tests --lib`, `cargo xtask lsp-cockpit-report` |
| Gap artifacts validate read-only | #973 validates schema, root, identity, freshness, language, path, static-limit, and command payloads before stronger projection | LSP tests and `cargo xtask check-output-contracts` |
| Gap records project into diagnostics through identity | #969 projects gap records through existing diagnostic identity and fails closed for invalid artifacts | LSP diagnostics tests and `fixtures/editor_gap_cockpit` |
| Show Status projects gap state | #976 reports actionable, preview, stale, wrong-root, disabled, unavailable, malformed, and no-action states without making gate claims | LSP tests, VS Code status tests, and `lsp-cockpit-report` |
| Hover projects repair route after limits | #981 renders language/status, preview/static limits, gap state, repair route, verify command, receipt command, and limits in the required order | LSP hover tests and fixture hover artifacts |
| Actions remain bounded and fail closed | #983 gates related-test opening, repair packets, verify commands, receipt commands, static-limit notes, and refresh on validated artifacts | LSP action tests and VS Code command tests |
| Fixtures cover core and failure cases | #985 adds `fixtures/editor_gap_cockpit` for Rust actionable, TypeScript preview static limit, Python preview static limit, disabled language, wrong root, stale artifact, and no actionable gap | `cargo xtask fixtures`, `cargo xtask goldens check`, `cargo xtask check-fixture-contracts` |
| Real VS Code path is covered | #993 covers extension activation, server resolution, Rust default, preview-enabled workspace, disabled preview, hover, status, bounded actions, related-test safety, wrong-root, and stale handling | `npm --prefix editors/vscode run compile`, `npm --prefix editors/vscode run test:e2e` |
| User workflow is documented | #996 adds `docs/EDITOR_GAP_COCKPIT_WORKFLOW.md` and links it from editor docs | `cargo xtask check-doc-index`, `cargo xtask markdown-links` |
| Dogfood receipts record the loop | #998 extends `cargo xtask dogfood` with editor gap cockpit receipts and adds `docs/handoffs/2026-05-14-editor-gap-cockpit-receipts.md` | `cargo test -p xtask dogfood`, `cargo xtask dogfood` |

## PR Chain

- #967 `docs(lane3): open editor gap cockpit source-of-truth stack`
- #970 `test(lsp): pin post-campaign editor contract`
- #973 `lsp(gap): add read-only gap artifact validation`
- #969 `lsp(gap): project gap records into editor diagnostics`
- #976 `lsp(gap): project gap state in Show Status`
- #981 `lsp(gap): enrich hover repair route`
- #983 `lsp(gap): add bounded repair actions`
- #985 `fixtures(editor): add gap cockpit workflow fixtures`
- #993 `test(vscode): smoke editor gap cockpit`
- #996 `docs(editor): document gap cockpit workflow`
- #998 `dogfood(lane3): record editor gap cockpit receipts`
- `campaign/lane3-editor-gap-cockpit-closeout`

## Verification Run

Closeout validation for this PR:

```bash
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-doc-roles
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-pr
git diff --check
```

Result: pass.

The final dogfood receipt PR also passed:

```bash
cargo test -p xtask dogfood
cargo xtask dogfood
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-pr
git diff --check
```

## Remaining Limits

- Rust remains the stable default path.
- TypeScript, JavaScript, and Python remain opt-in preview evidence.
- Preview evidence remains syntax-first, advisory, and static-limit bounded.
- Static evidence does not imply runtime adequacy.
- Stale, wrong-root, malformed, disabled, unavailable, and no-action states
  fail closed.
- Policy, gates, PR/CI summaries, PR comments, receipts, and analyzer truth
  remain owned by their lanes.

## Artifacts

- `docs/EDITOR_GAP_COCKPIT_WORKFLOW.md`
- `docs/handoffs/2026-05-14-editor-gap-cockpit-receipts.md`
- `fixtures/editor_gap_cockpit/`
- `target/ripr/reports/lsp-cockpit.md`
- `target/ripr/reports/lsp-cockpit.json`
- `target/ripr/reports/dogfood.md`
- `target/ripr/reports/dogfood.json`

## Next Work Item

No behavior-bearing Lane 3 work item is selected after this closeout. Future
first-run/install polish, CodeLens, inlay hints, semantic tokens, inline patch
application, automatic source edits, generated tests, provider calls, mutation
execution, or unsaved-buffer overlays require a new editor campaign.

## What Not To Do

- Do not reopen Editor Gap Cockpit for speculative editor furniture.
- Do not make the editor infer actionability by parsing prose.
- Do not show repair actions for stale, wrong-root, malformed, disabled, or
  unavailable artifacts except refresh.
- Do not promote preview evidence to Rust-level confidence.
- Do not add source edits, generated tests, provider calls, mutation execution,
  policy decisions, gate behavior, PR comments, or generated CI composition to
  Lane 3.
