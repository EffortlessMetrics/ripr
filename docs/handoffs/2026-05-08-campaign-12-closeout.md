# Handoff: Campaign 12 Closeout

Date: 2026-05-08
Branch / PR: `campaign-first-hour-ux-closeout` / pending
Latest merged PR: #528 `docs: organize first-hour UX by user type`

## Current Work Item

`campaign/first-hour-ux-closeout`

Campaign 12 made the first hour useful from the surfaces users already open:

```text
VS Code status/diagnostic/action loop
generated CI advisory summary
CLI pilot proof path
agent or reviewer artifact loop
```

The campaign kept the CLI as the shared engine instead of the required first
interface. It did not add analyzer behavior, new LSP feature classes,
unsaved-buffer overlays, automatic edits, generated tests, runtime mutation
execution, CI blocking by default, public crate splits, or SARIF/badge schema
changes.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| PR guidance had a fixed advisory contract before workflow projection | `spec/pr-test-guidance-annotations` added RIPR-SPEC-0012 with changed-line placement, caps, bounded LLM guidance, check annotations by default, optional inline review comments, JSON shape, and non-blocking CI posture. |
| VS Code users can see first-run state without reading logs first | `vscode/first-run-status` added a status bar and `ripr: Show Status` path for server resolution, workspace detection, analysis running/complete/stale/failed, and no-actionable-seam states. |
| Editor actions read by user intent | `vscode/action-discoverability` grouped seam diagnostic actions around inspect, write targeted test, agent handoff, verify after test, review result, and refresh analysis while preserving command IDs and payloads. |
| Generated CI has a useful first screen | `ci/pr-summary-surface` made `ripr init --ci github` write a `RIPR advisory summary` with pilot recommendation, agent review packet, artifact paths, SARIF and badge status, known limits, and future PR guidance annotation counts. |
| Generated workflow UX is fixture-pinned | `ci/generated-workflow-smoke-fixture` pins artifact paths, top-seam extraction, agent artifact generation, non-blocking posture, optional SARIF gates, badge output, advisory summary sections, and future PR guidance annotation hooks. |
| Installed-user docs are organized by user type | `docs/ux-by-user-type` rewrote the Quickstart around VS Code, CI, CLI, agent/reviewer, troubleshooting, and known limits while keeping README as a short front door. |

## PR Chain

- #521 `campaign: open first-hour UX`
- #523 `spec: refine PR guidance field contract`
- #524 `vscode: add first-run status surface`
- #525 `vscode: group seam action titles by intent`
- #526 `ci: add advisory PR summary surface`
- #527 `ci: pin generated workflow smoke fixture`
- #528 `docs: organize first-hour UX by user type`
- `campaign/first-hour-ux-closeout`

## Verification Run

Closeout proof before opening this PR:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

The docs slice also ran:

```bash
cargo xtask check-capabilities
```

## Next Work Item

No ready work item remains in `.ripr/goals/active.toml`.

Choose the next product campaign explicitly before adding new behavior. Likely
future lanes should start from the roadmap and current user feedback rather
than reopening Campaign 12 by default.

## What Not To Do

- Do not reopen Campaign 12 for speculative editor features.
- Do not add unsaved-buffer overlays, CodeLens, inlay hints, semantic tokens,
  automatic edits, generated tests, or runtime mutation execution under this
  closeout.
- Do not make generated CI blocking by default.
- Do not turn PR guidance into a free-form LLM reviewer; keep it bounded to
  RIPR-selected seams and one focused test.
- Do not split the public package surface or broaden SARIF/badge schemas as
  part of first-hour UX maintenance.
