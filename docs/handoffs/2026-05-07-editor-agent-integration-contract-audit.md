# Handoff: Editor Agent Integration Contract Audit

Date: 2026-05-07
Branch / PR: `editor-agent-integration-contract-audit` / pending
Latest merged PR: #463 `campaign: open 0.4 release surface hardening`

## Current work item

`editor-agent/integration-contract-audit`

The audit keeps Campaign 10 as `editor-agent-integration` and treats the
release-surface work from #463 as a release-readiness gate inside that lane.
The PR is docs and manifest only.

## Next work item

`lsp/agent-loop-copy-commands`

Add command-oriented editor/LSP actions so a seam diagnostic can copy the agent
packet, brief, after-snapshot, verify, and receipt commands. Do not add
automatic edits, generated tests, CodeLens, inlay hints, semantic tokens, or
unsaved-buffer overlays.

## Open decisions

- Whether release readiness deserves a later separate campaign after the
  editor-agent loop closes. For now, readiness is a gate in
  `release/editor-agent-readiness-proof`, not the active lane.

## Current blockers

- `operator/verify-receipt-status` waits for the editor command contract so
  cockpit next-command text matches editor copy commands.
- `fixtures/editor-agent-loop` waits for both LSP commands and cockpit joins.
- CI, full-loop docs, and release-readiness proof wait for the fixture.

## Verification run

The audit PR should run:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-static-language
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Artifacts

- `docs/EDITOR_AGENT_INTEGRATION.md`
- `.ripr/goals/active.toml`
- `docs/IMPLEMENTATION_CAMPAIGNS.md`
- `docs/ROADMAP.md`
- `docs/IMPLEMENTATION_PLAN.md`

## Recommended next action

Start `lsp/agent-loop-copy-commands` from current `main` after this docs audit
lands and `cargo xtask goals next` reports that work item as ready.

## What not to do

- Do not reopen Campaign 9.
- Do not replace Campaign 10 with `release-surface-0-4` unless the product goal
  explicitly pivots.
- Do not mix policy or lint cleanup into this integration lane.
- Do not add speculative editor features.
