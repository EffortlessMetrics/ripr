# Handoff: Campaign 25 Closeout

Date: 2026-05-10
Branch / PR: `campaign-report-packet-index-closeout` / pending at authoring
Latest merged PR: #706 `dogfood: add report packet index receipts` (commit `9c84c01`)

## Current Work Item

`campaign/report-packet-index-closeout`

Campaign 25 made the uploaded `ripr-reports` packet navigable as one
reviewer-first index over explicit existing artifacts:

```text
PR review front panel, PR guidance, first useful action, assistant proof,
assistant-loop health, PR evidence ledger, baseline delta, RIPR Zero,
gate decision, receipts, calibration, coverage/grip frontier, SARIF,
badges, and validation reports
-> report-packet-index index.{json,md}
-> generated CI summary and report-packet upload
-> reviewer, developer, maintainer, and coding-agent workflow
-> dogfood receipts for representative packet states
```

The campaign did not change analyzer identity, recommendation ranking, gate
policy semantics, LSP/editor behavior, generated workflow defaults,
source-edit behavior, generated-test behavior, provider calls, mutation
execution, public crate shape, release posture, or security posture.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Campaign opened explicitly | #700 opened Campaign 25 as Report Packet Index after PR Review Front Panel, keeping it separate from analyzer, policy, editor, platform, inline-comment, and future summary-polish lanes. |
| Report contract exists before implementation | #701 added RIPR-SPEC-0024, output-schema coverage, capability metadata, traceability, campaign docs, roadmap, plan, and changelog updates for an advisory report-packet index over explicit existing artifacts. |
| Fixture corpus is pinned before producer work | #702 added `fixtures/boundary_gap/expected/report-packet-index/` with complete, sparse advisory, missing-front-panel, blocked-gate, missing-assistant-proof, missing-receipts, and coverage/grip-present packet cases plus an xtask fixture guard. |
| Read-only producer exists | #703 added `ripr reports index`, JSON/Markdown rendering, explicit artifact directory inputs, CLI/help wiring, fixture-backed output coverage, and CLI smoke coverage without hidden analysis reruns. |
| Generated CI projection exists | #704 runs `ripr reports index` only when indexed artifacts exist, uploads `index.{json,md}` with `ripr-reports`, and appends a compact advisory packet-index summary. |
| Generated CI projection remains advisory | #704 keeps the index separate from gate authority and does not change default blocking behavior. |
| Reader-facing workflow docs exist | #705 added `docs/REPORT_PACKET_INDEX_WORKFLOW.md`, explaining reviewer, maintainer, developer, and coding-agent use of the grouped packet map, missing-surface regeneration, and advisory gate boundaries. |
| Dogfood receipts cover packet states | #706 extended `cargo xtask dogfood` with checked repo-local report-packet index receipts for complete, sparse advisory, missing-front-panel, blocked-gate, missing-proof, missing-receipts, and coverage/grip-present cases. |
| Capability and traceability surfaces are updated | `docs/CAPABILITY_MATRIX.md`, `metrics/capabilities.toml`, `.ripr/traceability.toml`, `docs/IMPLEMENTATION_CAMPAIGNS.md`, `docs/IMPLEMENTATION_PLAN.md`, and `docs/ROADMAP.md` point to the closed Campaign 25 evidence package. |
| Future lane boundary is explicit | No next campaign is opened by this closeout. Future PR summary polish, packet history, inline comments, policy, editor, analyzer, platform, release, dependency, or MSRV work must be opened explicitly. |

## PR Chain

- #700 `campaign: open report packet index`
- #701 `spec: define report packet index contract`
- #702 `fixtures: pin report packet index corpus`
- #703 `report: add report packet index producer`
- #704 `ci: surface report packet index summary`
- #705 `docs: explain report packet index workflow`
- #706 `dogfood: add report packet index receipts`
- `campaign/report-packet-index-closeout`

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

No ready work item remains in `.ripr/goals/active.toml` from Campaign 25 after
this closeout.

Choose the next campaign explicitly before opening another product lane.
Likely follow-up lanes should stay separated:

- report-packet history or trend rollups over existing indexes;
- PR summary polish over existing front-panel and packet-index artifacts;
- optional inline-comment publisher with changed-line-only placement and caps;
- assistant-loop quality or trend reporting over multiple proof/front-panel
  reports;
- analyzer evidence, recommendation ranking, policy, editor, platform, release,
  dependency, or MSRV cleanup.

Those should not be folded into Campaign 25 closeout.

## What Not To Do

- Do not make generated workflows blocking by default.
- Do not make report-packet indexes the pass/fail authority.
- Do not claim runtime mutation outcomes, adequacy, correctness, or proof from
  static evidence.
- Do not hide missing-input, missing-front-panel, missing-proof,
  missing-receipt, sparse, blocked-gate, warning, baseline, acknowledged,
  waived, suppressed, calibration, coverage/grip, SARIF, or badge states.
- Do not run cargo-mutants or any mutation engine from packet-index workflows.
- Do not move analyzer identity, recommendation ranking, gate policy semantics,
  or editor behavior into packet-index closeout work.
- Do not generate tests, edit source, post inline comments, or call LLM
  providers from the packet-index surface by default.
