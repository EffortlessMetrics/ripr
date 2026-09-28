# Handoff: PR Review Guidance Audit

Date: 2026-05-08
Branch / PR: `campaign-pr-review-guidance-audit` / pending
Latest merged PR: #529 `campaign: close first-hour UX`

## Current Work Item

`campaign/pr-review-guidance-audit`

The completion audit restated the long-term control-plane objective as concrete
surfaces:

```text
changed seam -> evidence -> bounded packet or brief -> focused test intent
-> before/after static snapshot -> verify -> provenance-backed receipt
-> reviewer summary -> editor, CLI, CI, and PR-review convergence
```

The current repo satisfies the editor, CLI, agent, cockpit, generated CI
artifact, and first-hour documentation parts. The missing surface is the
PR-review producer: RIPR-SPEC-0012 defines `ripr review-comments`, and generated
CI can consume `target/ripr/review/comments.json`, but the command does not
exist yet.

## Prompt-To-Artifact Checklist

| Requirement | Evidence | Status |
| --- | --- | --- |
| Identify weak seam for a change | Repo seam inventory, repo exposure report, LSP seam diagnostics, agent seam packets, and `ripr pilot` are documented in `docs/CAPABILITY_MATRIX.md`, `README.md`, and Campaign 10/11 closeouts. | Covered |
| Explain why it matters | LSP hover, agent packet fields, pilot summary, and operator cockpit are pinned in docs/specs and fixtures. | Covered |
| Produce bounded work packet | `ripr agent brief`, `ripr agent packet`, and `ripr agent start` workflow manifest exist; Campaign 11 closed with source-edit-free workflow packets. | Covered |
| Point to focused test to write | Agent seam packets and brief fields include recommended test placement, related tests to imitate, assertion shape, and bounded missing discriminator guidance. | Covered |
| Verify static before/after movement | `ripr agent verify` compares before/after repo-exposure snapshots and feeds receipt/review-summary surfaces. | Covered |
| Emit provenance-backed receipt | `ripr agent receipt` emits schema `0.3` provenance, artifact hashes, static boundary flags, and bounded next-action guidance. | Covered |
| Emit reviewer summary | `ripr agent review-summary` joins status, workflow, receipt, cockpit, repo exposure, optional LSP cockpit, and CI artifact state. | Covered |
| Keep LLMs on rails | `docs/LLM_OPERATOR_GUIDE.md`, status/review-summary Markdown, generated CI text, and static-language checks repeat no automatic edits, no generated tests, no runtime mutation execution, and no runtime proof claims. | Covered |
| Human cockpit and audit trail | Operator cockpit joins repo exposure, LSP cockpit, verify, receipt, SARIF, badges, targeted-test outcome, and calibration when present. | Covered |
| Agent state machine | `ripr agent status` reports artifact presence, recoverable `seam_id`, missing-input commands, and stale-looking warnings without rerunning analysis. | Covered |
| Editor/CLI/CI converge on same loop | Campaigns 10 through 12 aligned saved-workspace diagnostics, copy commands, cockpit, generated workflow artifacts, advisory summary, and first-hour docs. | Covered |
| PR review converges on same loop | `docs/specs/RIPR-SPEC-0012-pr-test-guidance.md` and `docs/OUTPUT_SCHEMA.md` define `ripr review-comments`; generated CI has a future consumer hook for `target/ripr/review/comments.json`. `rg` found no CLI/app/output implementation for a `review-comments` producer. | Missing |

## Next Work Item

`review/pr-guidance-renderer`

Add a read-only report producer:

```bash
ripr review-comments \
  --root . \
  --base <sha> \
  --head <sha> \
  --out target/ripr/review/comments.json
```

It should also write Markdown, join existing static evidence, and preserve the
RIPR-SPEC-0012 boundaries: changed-line placement only when safe, summary-only
fallback otherwise, capped recommendations, no GitHub posting, no automatic
edits, no generated tests, no runtime mutation execution, and no CI blocking.

## Verification Run

Audit and campaign-open validation:

```bash
cargo xtask goals next
rg -n "review-comments|target/ripr/review/comments.json" crates/ripr/src docs .ripr metrics
rg -n "review-comments|ReviewComments|review_comments" crates/ripr/src/cli crates/ripr/src/app crates/ripr/src/output
```

Before opening this PR, run:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

## What Not To Do

- Do not implement a free-form LLM reviewer.
- Do not post inline PR comments by default.
- Do not place comments on unrelated unchanged lines.
- Do not make generated CI blocking by default.
- Do not edit source, generate tests, or run mutation testing.
- Do not reopen Campaign 12 for PR guidance; Campaign 13 owns this lane.
