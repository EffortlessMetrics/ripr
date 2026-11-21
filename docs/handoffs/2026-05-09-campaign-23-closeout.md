# Handoff: Campaign 23 Closeout

Date: 2026-05-09
Branch / PR: `campaign-assistant-loop-health-closeout` / pending at authoring
Latest merged PR: #668 `docs: add assistant loop health workflow` (commit `f6cabe0`)

## Current Work Item

`campaign/assistant-loop-health-closeout`

Campaign 23 turned assistant-loop proof packets into a read-only health
surface:

```text
test-oracle-assistant-proof.{json,md}
-> assistant-loop-health.{json,md}
-> proof completeness, static movement, warnings, and repair queue
-> advisory CI summary and maintainer / coding-agent workflow
```

The campaign did not change analyzer identity, recommendation ranking, gate
policy semantics, LSP/editor behavior, generated workflow defaults, source-edit
behavior, generated-test behavior, provider calls, mutation execution, public
crate shape, release posture, or security posture.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Campaign opened explicitly | #653 opened Campaign 23 as Assistant Loop Health after First Useful Action closed, separate from evidence-spine, platform, policy, and editor lanes. |
| Report contract exists before implementation | #655 added RIPR-SPEC-0022, output-schema coverage, traceability, capability metadata, campaign docs, roadmap, plan, and changelog updates for an advisory assistant-loop-health report over explicit proof inputs. |
| Schema examples were tightened before implementation | #656 aligned the examples and docs around explicit proof-state, movement, warning, and repair-queue vocabulary. |
| Fixture corpus is pinned | #657 added `fixtures/boundary_gap/expected/assistant-loop-health/` with complete-improved, partial-missing-optional, missing-required-input, unchanged, regressed, warning-heavy, and multi-proof report cases plus representative proof inputs. |
| Corpus contract is guarded | #659 added fixture-corpus checks so the health report cases stay complete and visible to follow-up agents. |
| Read-only producer exists | #660 added `ripr assistant-loop health`, JSON/Markdown rendering, explicit `--proof` input parsing, fixture-backed output tests, and CLI smoke coverage. |
| Generated CI projection exists | #665 runs `ripr assistant-loop health` only when `test-oracle-assistant-proof.json` exists, uploads `assistant-loop-health.{json,md}` with `ripr-reports`, and appends a compact advisory summary. |
| Generated CI projection remains advisory | #665 keeps the health report separate from gate authority and does not change default blocking behavior. |
| Reader-facing workflow docs exist | #668 added `docs/ASSISTANT_LOOP_HEALTH_WORKFLOW.md`, explaining proof report versus health report, complete/partial/missing states, static movement interpretation, generated-CI summary use, repair routing, coding-agent handoff, and advisory limits. |
| Capability and traceability surfaces are updated | `docs/CAPABILITY_MATRIX.md`, `metrics/capabilities.toml`, `.ripr/traceability.toml`, `docs/IMPLEMENTATION_CAMPAIGNS.md`, `docs/IMPLEMENTATION_PLAN.md`, and `docs/ROADMAP.md` point to the closed Campaign 23 evidence package. |
| Future lane boundary is explicit | No next campaign is opened by this closeout; future assistant-loop quality, evidence-spine, policy, editor, or platform work must be opened explicitly. |

## PR Chain

- #653 `campaign: open assistant loop health`
- #655 `spec: define assistant loop health report`
- #656 `docs: tighten assistant loop health schema examples`
- #657 `fixtures: pin assistant loop health corpus`
- #659 `fixtures: guard assistant loop health corpus contract`
- #660 `report: add assistant loop health`
- #665 `ci: surface assistant loop health artifacts`
- #668 `docs: add assistant loop health workflow`
- `campaign/assistant-loop-health-closeout`

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

No ready work item remains in `.ripr/goals/active.toml` from Campaign 23 after
this closeout.

Choose the next campaign explicitly before opening another product lane.
Likely follow-up lanes should stay separated:

- assistant-loop quality or trend reporting over multiple health reports;
- evidence-spine stabilization and movement-contract hardening;
- PR / CI review summary polish over existing reports;
- editor UX work that projects existing health and first-action reports;
- platform, release, dependency, or MSRV cleanup.

Those should not be folded into Campaign 23 closeout.

## What Not To Do

- Do not make generated workflows blocking by default.
- Do not make assistant-loop-health reports the pass/fail authority.
- Do not claim runtime mutation outcomes, adequacy, correctness, or proof from
  static evidence.
- Do not hide incomplete, missing-input, unchanged, regressed, warning-heavy,
  stale, acknowledged, waived, or suppressed states.
- Do not run cargo-mutants or any mutation engine from health workflows.
- Do not move analyzer identity, recommendation ranking, gate policy
  semantics, or editor behavior into health closeout work.
- Do not generate tests, edit source, post inline comments, or call LLM
  providers from the health report by default.
