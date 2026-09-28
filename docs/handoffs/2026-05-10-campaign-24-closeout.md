# Handoff: Campaign 24 Closeout

Date: 2026-05-10
Branch / PR: `campaign-pr-review-front-panel-closeout` / pending at authoring
Latest merged PR: #698 `dogfood: add PR review front-panel receipts` (commit `e91afb7`)

## Current Work Item

`campaign/pr-review-front-panel-closeout`

Campaign 24 composed the existing PR-time evidence stack into one advisory
front panel for GitHub PR review:

```text
PR guidance, first useful action, assistant proof, assistant-loop health,
PR evidence ledger, baseline delta, gate decision, receipts, calibration,
and optional coverage/grip frontier artifacts
-> pr-review-front-panel.{json,md}
-> generated CI summary and report-packet upload
-> reviewer, developer, maintainer, and coding-agent workflow
-> dogfood receipts for representative reviewer states
```

The campaign did not change analyzer identity, recommendation ranking, gate
policy semantics, LSP/editor behavior, generated workflow defaults, source-edit
behavior, generated-test behavior, provider calls, mutation execution, public
crate shape, release posture, or security posture.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Campaign opened explicitly | #672 opened Campaign 24 as PR Review Front Panel after Assistant Loop Health, keeping it separate from analyzer, policy, editor, platform, and future summary-polish lanes. |
| Report contract exists before implementation | #673 added RIPR-SPEC-0023, output-schema coverage, capability metadata, traceability, campaign docs, roadmap, plan, and changelog updates for an advisory front-panel report over explicit existing artifacts. |
| Fixture corpus is pinned before producer work | #676 added `fixtures/boundary_gap/expected/pr-review-front-panel/` with advisory-only, actionable, summary-only, acknowledged, suppressed, baseline-resolved, blocked, missing-proof, and coverage-flat-grip-improved cases plus an xtask fixture guard. |
| Read-only producer exists | #681 added `ripr pr-review front-panel`, JSON/Markdown rendering, explicit artifact input parsing, fixture-backed output coverage, and CLI dispatch without hidden analysis reruns. |
| Generated CI projection exists | #686 runs `ripr pr-review front-panel` only when explicit input artifacts exist, uploads `pr-review-front-panel.{json,md}` with `ripr-reports`, and appends a compact advisory PR review summary. |
| Generated CI projection remains advisory | #686 keeps the front-panel report separate from gate authority and does not change default blocking behavior. |
| Reader-facing workflow docs exist | #689 added `docs/PR_REVIEW_FRONT_PANEL_WORKFLOW.md`, explaining first-screen reading, repair routes, artifact groups, developer and coding-agent handoff, maintainer interpretation, and advisory gate boundaries. |
| Dogfood receipts cover reviewer states | #698 extended `cargo xtask dogfood` with checked repo-local front-panel receipts for actionable, acknowledged, suppressed, baseline-resolved, blocked, missing-proof, no-actionable, and coverage-flat-grip-improved cases. |
| Capability and traceability surfaces are updated | `docs/CAPABILITY_MATRIX.md`, `metrics/capabilities.toml`, `.ripr/traceability.toml`, `docs/IMPLEMENTATION_CAMPAIGNS.md`, `docs/IMPLEMENTATION_PLAN.md`, and `docs/ROADMAP.md` point to the closed Campaign 24 evidence package. |
| Future lane boundary is explicit | No next campaign is opened by this closeout. Future PR summary polish, inline comments, artifact indexing, policy, editor, analyzer, or platform work must be opened explicitly. |

## PR Chain

- #672 `campaign: open PR review front panel`
- #673 `spec: define PR review front panel report`
- #676 `fixtures: pin PR review front panel corpus`
- #681 `report: add PR review front panel`
- #686 `ci: surface PR review front panel`
- #689 `docs: explain PR review front panel workflow`
- #698 `dogfood: add PR review front-panel receipts`
- `campaign/pr-review-front-panel-closeout`

## Verification Run

Closeout validation before opening this PR:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Next Work Item

No ready work item remains in `.ripr/goals/active.toml` from Campaign 24 after
this closeout.

Choose the next campaign explicitly before opening another product lane.
Likely follow-up lanes should stay separated:

- PR summary front-panel polish over existing report artifacts;
- artifact indexing and report-packet navigation;
- optional inline-comment publisher with changed-line-only placement and caps;
- assistant-loop quality or trend reporting over multiple proof/front-panel
  reports;
- analyzer evidence, recommendation ranking, policy, editor, platform, release,
  dependency, or MSRV cleanup.

Those should not be folded into Campaign 24 closeout.

## What Not To Do

- Do not make generated workflows blocking by default.
- Do not make PR review front-panel reports the pass/fail authority.
- Do not claim runtime mutation outcomes, adequacy, correctness, or proof from
  static evidence.
- Do not hide missing-input, summary-only, no-actionable, already-improved,
  acknowledged, waived, suppressed, baseline, gated, warning, or
  coverage-flat-grip-improved states.
- Do not run cargo-mutants or any mutation engine from front-panel workflows.
- Do not move analyzer identity, recommendation ranking, gate policy semantics,
  or editor behavior into front-panel closeout work.
- Do not generate tests, edit source, post inline comments, or call LLM
  providers from the front-panel surface by default.
