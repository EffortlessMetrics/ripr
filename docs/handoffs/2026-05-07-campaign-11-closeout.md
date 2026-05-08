# Handoff: Campaign 11 Closeout

Date: 2026-05-07
Branch / PR: `campaign-llm-work-loop-closeout` / #519
Latest merged PR: #518 `docs: add LLM operator guide`

## Current Work Item

`campaign/llm-work-loop-closeout`

Campaign 11 made the completed editor-agent evidence loop stateful enough for
humans and external LLM tools to resume from repository artifacts instead of
chat history:

```text
agent status -> workflow packet -> packet or brief -> focused external test
edit -> after snapshot -> agent verify -> agent receipt -> reviewer summary
```

The campaign did not add analyzer families, LSP feature expansion,
unsaved-buffer overlays, model calls, model selection, automatic edits,
generated tests, runtime mutation execution, default CI blocking, or new public
crates.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Agents can inspect loop state without rerunning analysis | `agent/loop-status-report` added `ripr agent status --root . --json` and Markdown output over existing before snapshot, after snapshot, agent brief, packet, verify, and receipt artifacts. |
| Command strings and artifact paths are centralized | `agent/centralize-loop-command-templates` added shared templates reused by CLI, LSP copy actions, pilot follow-up commands, agent brief, operator cockpit, generated CI, docs, and fixtures. |
| A selected seam has a source-edit-free workflow packet | `agent/workflow-manifest` added `ripr agent start --root . --seam-id <id> --out target/ripr/workflow` with `workflow.json`, `commands.md`, and `agent-brief.json`. |
| Receipts are reviewer-traceable | `agent/receipt-provenance` added version, root, config fingerprint, command-template version, timestamp, artifact hashes, seam ID, before/after classes, movement, and static boundary flags. |
| Receipt states explain the next static step | `agent/next-action-guidance` added bounded next-action guidance for improved, changed, regressed, unchanged, resolved, and new-gap states. |
| Reviewers get one joined packet | `agent/reviewer-summary` added Markdown and JSON review summaries that join status, workflow, receipt, cockpit, repo exposure, optional LSP cockpit, and local CI artifact state. |
| Edge states are pinned | `fixtures/llm-work-loop` added happy, unchanged, regressed, missing-artifact, stale-artifact, configured-off, path-with-spaces, and Windows-separator fixtures. |
| Generated CI carries the work packets | `ci/llm-work-packets` uploads `target/ripr/workflow` status, workflow, packet, brief, verify, review summary, `target/ripr/reports/agent-receipt.json`, and operator cockpit artifacts as advisory evidence. |
| Humans and external LLM tools have one guide | `docs/LLM_OPERATOR_GUIDE.md` documents the source-edit-free loop, artifact paths, CI/editor paths, minimal handoff, and explicit anti-goals. |

## PR Chain

- #497 `agent: add loop status report`
- `agent: centralize loop command templates`
- #507 `agent: add workflow manifest`
- `agent: add receipt provenance`
- #514 `agent: add receipt next-action guidance`
- #515 `agent: add review summary`
- #516 `fixtures: pin LLM work loop matrix`
- #517 `ci: upload LLM work-loop packets`
- #518 `docs: add LLM operator guide`
- #519 `campaign/llm-work-loop-closeout`

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

The final docs slice also ran:

```bash
cargo xtask check-traceability
cargo xtask check-capabilities
```

## Next Work Item

No ready work item remains in `.ripr/goals/active.toml`.

Campaign 12, First-Hour UX, is queued in `docs/IMPLEMENTATION_CAMPAIGNS.md`.
Open it explicitly before adding editor or CI first-screen behavior.

## What Not To Do

- Do not reopen Campaign 11 to add PR comments or CI annotations; those belong
  in the First-Hour UX / PR guidance lane.
- Do not add LLM provider integration, model selection, prompt orchestration, or
  automatic test generation.
- Do not make RIPR edit production or test source.
- Do not run mutation testing or claim runtime confirmation from static receipt
  movement.
- Do not make generated CI blocking by default.
- Do not add speculative LSP features or unsaved-buffer overlays as part of this
  closeout.
