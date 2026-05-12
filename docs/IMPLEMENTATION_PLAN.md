# Implementation Plan

This is the working PR checklist for building `ripr` incrementally. It is more
operational than the [roadmap](ROADMAP.md): each entry should become a scoped PR
with clear artifacts, tests, documentation updates, and gates.

The checklist is grouped into
[implementation campaigns](IMPLEMENTATION_CAMPAIGNS.md). A campaign may work
through multiple work items, but each work item should follow the
[scoped PR contract](SCOPED_PR_CONTRACT.md).

This file is one layer of the repo's centralized, agent-neutral tracking
model. The full layering — proposals (why), specs (what), ADRs (durable
decisions), this plan (work queue), campaign ledger, active manifest, and
closeout handoffs — is documented in the
[repo tracking model](REPO_TRACKING_MODEL.md). Reach for a proposal in
[`docs/proposals/`](proposals/) before a spec when the change spans more
than one behavior contract or touches repo shape.

## Campaign Map

| Campaign | Objective | Work items |
| --- | --- | --- |
| Agentic DevEx Foundation | Make the repo safe for Codex Goals and human review. | `policy/architecture-guard`, `output/output-contract-check`, `docs/codex-goals-campaigns`, `fixtures/runner-comparison-v1`, `fixtures/first-two-goldens`, `testing/test-oracle-report`, `dogfood/static-self-check` |
| Syntax-Backed Analyzer Foundation | Move the analyzer from lexical facts to syntax-backed facts. | `analysis/file-facts-model`, `analysis/syntax-adapter-mvp`, `design/rust-syntax-substrate`, `analysis/ast-test-oracle-extraction`, `analysis/ast-probe-ownership`, `analysis/ast-probe-generation` |
| Evidence Quality | Improve oracle strength, local flow, activation values, output evidence, and stop reasons. | `output/unknown-stop-reason-invariant`, `analysis/oracle-strength-v2`, `analysis/local-delta-flow-v1`, `analysis/activation-value-modeling-v1`, `output/evidence-first-output`, `fixtures/negative-metamorphic-baseline` |
| Test Efficiency and Vacuity Signals (4A) | Make low-discriminator, smoke-only, broad-oracle, opaque, circular, and duplicate test signals visible as advisory evidence; ship `ripr` and `ripr+` badge artifacts. | `test-efficiency/test-fact-ledger`, `test-efficiency/vacuous-signal-v1`, `test-efficiency/duplicate-discriminator-v1`, `test-efficiency/report-and-metrics`, `badge/ripr-count-v1`, `badge/ripr-plus-count-v1`, `badge/repo-scope-artifacts`, `badge/publish-main-endpoint` |
| Repo Seam Inventory and Test Grip (4B) | Inventory behavior seams, classify test-grip per seam, and turn actionable gaps into editor diagnostics and agent-ready packets. | `spec/repo-seam-inventory`, `analysis/repo-seam-model-v1`, `analysis/repo-seam-inventory-v1`, `analysis/test-grip-evidence-v1`, `analysis/repo-ripr-classification-v1`, `output/repo-exposure-report-v1`, `lsp/repo-seam-diagnostics-v1`, `lsp/seam-evidence-hover-v1`, `context/agent-seam-packets-v1`, `docs/agent-dispatch-workflow-v1` |
| Seam Evidence Usability and Precision (5A) | Make repo seam evidence fast, precise, and directly actionable for developers and coding agents. | Complete: #255, #310, #313, #314, #315, #316, #327, and `campaign/seam-evidence-usability-closeout`. |
| Operationalization (5B) | Govern analyzer behavior with repository config, integrate SARIF/CI policy modes, and remap badges onto seam-native counts. | Complete: `config/ripr-config-v1`, `ci/sarif-ci-policy`, `badge/seam-native-count-mapping`, and `campaign/operationalization-closeout`. |
| Module SRP Refactoring (6) | Refactor internal modules under `crates/ripr/src/` so each module has one product responsibility, without splitting the package. | Complete: #347, the Campaign 6 refactor chain through #405, and `campaign/modularization-closeout`. |
| Defaults-First Operator Adoption (7) | Make a clean install useful through conservative defaults, one operator cockpit, CI artifacts, editor install docs, examples, and install/release proof. | Complete: #409 through #417 plus `campaign/defaults-first-closeout`. |
| Runtime Calibration Fixture Expansion (8) | Expand supplied-runtime calibration fixtures without making RIPR run mutation tests. | Complete: #420 plus `campaign/runtime-calibration-closeout`. |
| Hot Sidecar Latency Proof (9) | Measure current cache and saved-workspace editor refresh behavior before changing warm-path reuse. | Complete: latency reporting, warm-path reuse, bounded `ripr pilot`, first-screen clarity, evidence progress tracing, hot-path indexes, and `campaign/hot-sidecar-latency-closeout`. |
| Editor Agent Integration (10) | Make the saved-workspace editor loop and the agent CLI loop line up from diagnostic to evidence, packet/brief, focused test, verify, receipt, cockpit, CI, and install proof. | Complete: `campaign/editor-agent-integration-closeout`. |
| LLM Work Loop (11) | Make the completed editor-agent loop stateful, deterministic, and useful to LLM agents under review pressure. | Complete: status, command templates, workflow manifests, receipt provenance, next-action guidance, reviewer summary, fixture matrix, CI work packets, operator guide, and `campaign/llm-work-loop-closeout`. |
| First-Hour UX (12) | Make new LSP-first and CI-first users successful without learning RIPR's internal report topology. | Complete: editor status, intent-titled actions, advisory CI summary, workflow smoke fixture, user-type docs, and `campaign/first-hour-ux-closeout`. |
| PR Review Guidance (13) | Project existing RIPR evidence into bounded pull-request review guidance without making CI blocking or turning RIPR into a free-form reviewer. | Complete: renderer, generated CI, fixtures, docs, and `campaign/pr-review-guidance-closeout`. |
| Recommendation Calibration (14) | Measure whether top CI, LSP, and PR recommendations are clear, correctly placed, low-noise, and correlated with better static evidence after one focused test. | Complete: spec, corpus, receipts, report, guide, and `campaign/recommendation-calibration-closeout`. |
| Calibrated Gate Policy (15) | Define optional calibrated gates over existing PR-time evidence without changing advisory defaults or blurring static/runtime evidence. | Complete: spec, evaluator, fixtures, generated CI opt-in wiring, evidence-preserving CI behavior, calibrated gate guide, and `campaign/calibrated-gate-closeout`. |
| Gate Adoption UX (16) | Make optional calibrated gate adoption safe and obvious for real teams without changing advisory defaults. | Complete: generated-CI examples, waiver workflows, baseline guidance, CI gate summary polish, dogfood receipts, blocking-readiness guidance, and `campaign/gate-adoption-ux-closeout`. |
| RIPR Zero Adoption (17) | Turn baselines into burn-down ledgers with create, diff, and shrink-only refresh commands while keeping generated CI advisory by default. | Complete: spec, baseline create, baseline diff, shrink-only update, generated CI baseline-delta artifacts, baseline ledger workflow guide, and `campaign/ripr-zero-adoption-closeout`. |
| RIPR Zero Reporting (18) | Turn reviewed baselines and debt deltas into repo-level RIPR 0 status, stale-debt, trend, and repair-area reporting while preserving advisory defaults. | Complete: spec, baseline metadata, status report, generated CI summary, user workflow docs, and `campaign/ripr-zero-reporting-closeout`. |
| PR Evidence Ledger (19) | Turn per-PR RIPR evidence into an adoption ledger for movement history, waiver aging, baseline burn-down, repair receipts, and coverage/grip frontier signals while preserving advisory defaults. | Complete: spec, producer, generated-CI projection, coverage/grip frontier report, user workflow docs, and `campaign/pr-evidence-ledger-closeout`. |
| Test-Oracle Assistant Proof (20) | Prove the full PR-time loop from changed Rust behavior to static evidence, PR/editor guidance, focused-test handoff, verification, receipt, and advisory CI/ledger projection. | Complete: spec, canonical fixture, dogfood receipt, user workflow docs, and `campaign/test-oracle-assistant-proof-closeout`. |
| Test-Oracle Assistant Report Producer (21) | Turn the proved assistant loop into a public read-only JSON/Markdown report producer and optional advisory CI projection. | Complete: report producer, generated-CI projection, proof-report docs, and `campaign/test-oracle-assistant-report-closeout`. |
| First Useful Action (22) | Compress existing editor, PR, ledger, proof, receipt, optional gate, coverage/grip, and staleness evidence into one advisory next test action. | Complete: RIPR-SPEC-0020 defines the report contract, the routing corpus is pinned, `ripr first-action` writes the read-only advisory report, generated CI surfaces it as advisory summary/artifact content, VS Code status/Show Status project an existing report, workflow docs explain developer, reviewer, and agent use, dogfood receipts are checked, and `campaign/first-useful-action-closeout` records the final audit. |
| Assistant Loop Health (23) | Summarize proof completeness, missing inputs, static evidence movement, recurring warnings, and next repair queues across one or more assistant proof reports. | Complete: RIPR-SPEC-0022 defines the report contract, the assistant-loop-health fixture corpus is pinned, `ripr assistant-loop health` writes advisory JSON/Markdown from explicit proof inputs, generated GitHub CI uploads and summarizes health artifacts when proof artifacts exist, `docs/ASSISTANT_LOOP_HEALTH_WORKFLOW.md` explains maintainer and agent use, and `campaign/assistant-loop-health-closeout` records the final audit. |
| PR Review Front Panel (24) | Compose existing PR guidance, first useful action, assistant proof, assistant-loop health, PR evidence ledger, baseline delta, gate decision, receipts, calibration, and optional coverage/grip frontier artifacts into one advisory generated-CI first screen. | Complete: report contract, fixture corpus, `ripr pr-review front-panel`, generated-CI projection, workflow docs, dogfood receipts, and closeout audit are in place. |
| Report Packet Index (25) | Make the uploaded `ripr-reports` packet navigable as a reviewer-first index over explicit existing artifacts. | Complete: report contract, fixture corpus, `ripr reports index`, generated-CI projection, workflow docs, dogfood receipts, and closeout audit are in place. |
| PR Inline Comment Publisher (26) | Make optional durable PR comments safe and explicit by planning, capping, deduplicating, and publishing only changed-line `review-comments` entries when configured. | Complete: spec, fixture corpus, read-only publish plan, generated-CI opt-in wiring, workflow docs, dogfood receipts, and closeout audit are in place. |
| Language Adapter Preview (27) | Introduce a language-neutral analysis adapter boundary, keep Rust as the reference adapter, and add syntax-first TypeScript and Python preview adapters that feed the existing RIPR domain, output, LSP, agent, and Lane 4 surfaces without changing Rust behavior or default CI blocking. | Active: RIPR-PROP-0001 records the design intent, RIPR-SPEC-0026 pins the adapter contract, RIPR-SPEC-0027 pins the TypeScript preview static-fact contract, and RIPR-SPEC-0028 pins the Python preview static-fact contract; remaining work items cover the adapter boundary, Rust adapter, output metadata, TypeScript and Python preview adapters, editor routing, language-aware advisory CI grouping, workflow docs, dogfood receipts, and `campaign/language-adapter-preview-closeout`. |
| Policy Readiness and Preview Evidence Governance (Lane 2 tracker) | Make policy decisions auditable across stable Rust evidence and preview-language evidence without changing advisory defaults. | Tracker: [#755](https://github.com/EffortlessMetrics/ripr/issues/755) and [Policy readiness](policy/POLICY_READINESS.md) define the focused Lane 2 board; planned work covers readiness reporting, preview evidence policy, waiver aging, suppression health, baseline refresh guardrails, exception ledger convergence, blocking guidance, advisory CI projection, and closeout. |
| Editor Evidence UX (future) | Make the saved-workspace LSP path feel like an editor-native test-intent cockpit from diagnostic to hover, related test, context packet, one test, verify, and receipt. | Complete as an explicit parallel Lane 3 closeout: contract audit, hover hardening, evidence-aware actions, context packet, protocol smoke, VS Code smoke, status/staleness, workflow docs, and closeout audit. |

The active machine-readable campaign is `.ripr/goals/active.toml`. The focused
Lane 2 policy readiness tracker lives in
`.ripr/goals/lane2-policy-readiness.toml` and
[Policy readiness](policy/POLICY_READINESS.md); it is a GitHub issue/PR board,
not the active execution manifest. Campaigns 1 through 8 are complete.
Campaign 6 closed after the internal module SRP chain
landed through #405 while preserving the saved-workspace LSP cockpit contract,
output schemas, public API, SARIF, and badge behavior. Campaign 7 closed after
the defaults-first CLI, editor, CI, fixture, release, and report surfaces were
verified; the closeout audit lives at
`docs/handoffs/2026-05-07-campaign-7-closeout.md`. Campaign 8 added the checked
`fixtures/boundary_gap/calibration/runtime-fixtures-v1/` sample for the main
static/runtime agreement buckets and closed with runtime calibration still
confined to supplied-data reports. Campaign 9 measured the cache/editor proof
surfaces, added bounded latency reporting, reused warm-path facts below rendered
outputs, bounded `ripr pilot`, improved first-screen pilot clarity, added
evidence progress tracing, and closed after hot-path evidence indexes made the
default latency report pass on cache hits. Campaign 10 closed after aligning
the saved-workspace editor and agent CLI loop through diagnostics, evidence,
packet/brief commands, focused-test receipts, cockpit status, generated CI
artifacts, and release-readiness proof. Campaign 11 closed after adding a
read-only `ripr agent status` lens over existing agent-loop artifacts,
centralized command templates for CLI, LSP, cockpit, generated CI, docs, and
fixtures, source-edit-free workflow manifests, provenance-backed receipts,
bounded next-action guidance, review summaries, a fixture matrix, generated CI
work-loop packet uploads, and the LLM operator guide. Campaign 12 closed the
First-Hour UX lane after the LLM work-loop control plane: the PR guidance
annotation contract is pinned, the extension has a first-run status path,
diagnostic actions are titled around user intent, and the generated GitHub
workflow now writes a reviewer-oriented advisory summary before artifact
download. The generated workflow smoke fixture pins artifact paths,
top-seam extraction, agent artifacts, optional SARIF gates, badges, summary
sections, and PR guidance annotation hooks. The first-hour docs route
users by VS Code, CI, CLI, and agent/reviewer path. Campaign 13 closed PR
Review Guidance: `ripr review-comments` now produces the advisory JSON and
Markdown report, generated CI runs it before the existing summary and
annotation consumer steps, placement and suppression fixtures are pinned, and
[PR review guidance](PR_REVIEW_GUIDANCE.md) documents the bounded advisory
workflow. Campaign 14 closed Recommendation Calibration: RIPR-SPEC-0013 pins
the recommendation calibration report contract, the PR-shaped calibration
corpus plus local outcome receipts are checked, `cargo xtask
recommendation-calibration` emits the advisory report, and [Recommendation
calibration](RECOMMENDATION_CALIBRATION.md) documents how to read metrics,
receipts, placement quality, suppression correctness, static movement buckets,
and advisory limits. Campaign 15 closed the Calibrated Gate Policy lane:
RIPR-SPEC-0014 pins optional gates as explicit policy over measured evidence,
with advisory defaults, visible acknowledgement paths, and runtime mutation
calibration only as imported confidence evidence. `gate/policy-evaluator` is
implemented as a read-only report producer; `fixtures/calibrated-gate-cases`
pins the decision matrix; generated GitHub workflows now run gate evaluation
only when `RIPR_GATE_MODE` is explicitly configured; and
[Calibrated gate policy](CALIBRATED_GATE_POLICY.md) documents modes, waivers,
CI behavior, calibration evidence, and the static/runtime boundary. The
closeout handoff records the PR chain, prompt-to-artifact audit, and explicit
boundary that adoption should be opened explicitly after closeout. Campaign 16
closed Gate Adoption UX after making explicit gate adoption safe and reviewable
without changing advisory defaults. It added copyable generated-CI examples for
default advisory, `visible-only`, `acknowledgeable`, `baseline-check`, and
`calibrated-gate` modes; documented `ripr-waive` as visible acknowledgement
rather than suppression; documented baseline creation and shrink refreshes as
a visible historical-debt workflow; polished generated CI summaries for gate
mode, status, labels, waiver, baseline, calibration, blocking reason, and
artifact paths; recorded checked repo-local gate adoption receipts through
`cargo xtask dogfood`; and added [RIPR blocking
readiness](BLOCKING_READINESS.md) for deciding when to stay advisory, require
acknowledgement, use baseline-check, or enable calibrated blocking. The
[Campaign 16 closeout](handoffs/2026-05-08-campaign-16-closeout.md) records
the PR chain and proof commands. Campaign 17 closed RIPR Zero
Adoption: RIPR-SPEC-0016 defines the baseline debt delta report contract,
`ripr baseline create` can write reviewed baseline ledgers, `ripr baseline
diff` can report baseline debt movement, and `ripr baseline update
--remove-resolved` can shrink reviewed baselines without adopting new current
debt. Generated CI can now upload and summarize baseline debt delta artifacts
when a baseline and gate decision are present; the baseline ledger workflow
guide now documents initial adoption, baseline-check rollout, shrink-only
refresh, new debt review, and the path toward RIPR 0. The
[Campaign 17 closeout](handoffs/2026-05-09-campaign-17-closeout.md) records
the PR chain, prompt-to-artifact audit, proof commands, and next-work boundary.
Campaign 18 closed as RIPR Zero Reporting. RIPR-SPEC-0017 defines the
repo-level status surface for baseline owner/reason/age metadata, stale
warnings, trends, top debt areas, and repair routing. Baseline metadata
preservation, the read-only RIPR Zero status report, generated-CI summary
projection, and the user workflow docs are in place. The
[Campaign 18 closeout](handoffs/2026-05-09-campaign-18-closeout.md) records
the PR chain, proof commands, and boundary that progress toward RIPR 0 remains
separate from analyzer identity, gate policy, and advisory defaults.
Editor Evidence UX closed as a separate Lane 3 campaign; its contract audit is
recorded in [Editor Evidence UX](EDITOR_EVIDENCE_UX.md), the user path is
documented in the [editor evidence workflow](EDITOR_EVIDENCE_WORKFLOW.md), and
the [closeout handoff](handoffs/2026-05-09-editor-evidence-ux-closeout.md)
records the prompt-to-artifact audit. Future editor work should be opened as a
new explicit campaign.
Campaign 19 closed as PR Evidence Ledger. It made append-only per-PR movement
records, waiver aging, baseline burn-down, repair receipts, and optional
coverage/grip frontier signals visible without changing advisory defaults.
RIPR-SPEC-0018 pins the contract; `ripr pr-ledger record` writes the read-only
ledger; generated CI uploads `pr-evidence-ledger.{json,md}` and appends the
advisory PR movement card; `ripr coverage-grip frontier` keeps coverage and
behavioral grip movement separate; and
`docs/PR_EVIDENCE_LEDGER_WORKFLOW.md` explains the adoption workflow. Campaign
20 closed as Test-Oracle Assistant Proof. RIPR-SPEC-0019 defines the
end-to-end proof contract from changed Rust behavior through static evidence,
PR/editor guidance, focused-test handoff, verification, receipt, and advisory
CI/ledger projection without changing analyzer, policy, editor, or CI defaults.
The canonical boundary-gap replay corpus pins one seam across recommendation,
handoff, receipt, and ledger projection. The repo-local dogfood receipt traces
seam `67fc764ba37d77bd` through PR guidance, editor/agent handoff,
before/after evidence, receipt, PR ledger projection, and coverage/grip
frontier availability. `docs/TEST_ORACLE_ASSISTANT_WORKFLOW.md` explains the
user-facing PR/editor-to-receipt workflow and static evidence limits. The
[Campaign 20 closeout](handoffs/2026-05-09-campaign-20-closeout.md) records
the prompt-to-artifact audit, proof commands, and boundary that future proof
report producers, PR/CI polish, analyzer improvements, and editor UX work
should be opened as explicit follow-up campaigns. Campaign 21 closed as
Test-Oracle Assistant Report Producer. `ripr assistant-loop proof` now produces
advisory `test-oracle-assistant-proof.{json,md}` artifacts from explicit
existing inputs without changing analyzer, ranking, gate, editor, provider,
mutation, or default CI behavior. Generated GitHub CI now projects that report
only when the required artifact chain already exists.
`docs/TEST_ORACLE_ASSISTANT_PROOF_REPORT.md` now explains how to read the
report, warnings, static movement, optional CI projection, and limits. Campaign
21 closed with
`docs/handoffs/2026-05-09-campaign-21-closeout.md`. Campaign 22 closed as
First Useful Action. It compresses existing evidence into one advisory next
test action; RIPR-SPEC-0020 pins the report contract, the routing corpus pins
expected statuses and fallback outputs, `ripr first-action` writes the
read-only report from explicit artifacts, generated CI projects it as advisory
summary/artifact content, and VS Code status projects existing reports without
rerunning analysis. The first-action workflow docs explain how developers,
reviewers, and coding agents read the action, verify movement, emit receipts,
and interpret fallback states. `cargo xtask dogfood` checks repo-local
first-action receipts for actionable, baseline-only, stale,
missing-required-artifact, unchanged-after-attempt, and no-actionable-seam
routes. The
[Campaign 22 closeout](handoffs/2026-05-09-campaign-22-closeout.md) records
the prompt-to-artifact audit, validation commands, and boundary that future
health, analyzer, policy, or editor lanes need explicit follow-up campaigns.

Campaign 23 closed as Assistant Loop Health. It uses the
[Assistant Loop Health proposal](ASSISTANT_LOOP_HEALTH_PROPOSAL.md) as its
design brief, [RIPR-SPEC-0022](specs/RIPR-SPEC-0022-assistant-loop-health-report.md)
defines the report contract, and the
`fixtures/boundary_gap/expected/assistant-loop-health/` corpus pins the health
states. The producer, generated-CI projection, and
[assistant loop health workflow](ASSISTANT_LOOP_HEALTH_WORKFLOW.md) are in
place. The [Campaign 23 closeout](handoffs/2026-05-09-campaign-23-closeout.md)
records the audit and future-lane boundary. The campaign measures whether
assistant proof packets are complete, stuck, missing receipts, or moving static
evidence over time, without changing analyzer behavior, ranking, gate
semantics, LSP/editor behavior, mutation execution, provider calls, source
files, generated tests, or default CI blocking.

Campaign 24 is now closed as PR Review Front Panel. It uses the
[PR Review Front Panel proposal](PR_REVIEW_FRONT_PANEL_PROPOSAL.md) as its
design brief, and
[RIPR-SPEC-0023](specs/RIPR-SPEC-0023-pr-review-front-panel-report.md) now
defines the report contract. The boundary-gap fixture corpus now pins the
advisory-only, actionable, summary-only, acknowledged, suppressed,
baseline-resolved, blocked, missing-proof, and coverage-flat-grip-improved
routes. `ripr pr-review front-panel` now writes the advisory JSON/Markdown
report from explicit existing artifact paths. Generated GitHub CI now runs the
front-panel producer only when explicit input artifacts exist, uploads
`pr-review-front-panel.{json,md}` with the normal report packet, and appends the
front-panel Markdown and at-a-glance fields to the advisory job summary while
leaving `ripr gate evaluate` as the only explicit pass/fail authority. The
[PR review front panel workflow](PR_REVIEW_FRONT_PANEL_WORKFLOW.md) now
documents how reviewers, maintainers, developers, and coding agents use the
panel, repair routes, receipts, and advisory gate boundary. The
dogfood report now checks repo-local front-panel receipts for actionable,
acknowledged, suppressed, baseline-resolved, blocked, missing-proof,
no-actionable, and coverage-flat-grip-improved reviewer states. The
campaign composes existing PR guidance, first useful action, assistant proof,
assistant-loop health, PR evidence ledger, baseline delta, gate decision,
receipts, calibration, and optional coverage/grip frontier artifacts into one
advisory GitHub PR first screen without changing analyzer behavior,
recommendation ranking, gate semantics, editor behavior, mutation execution,
provider calls, source files, generated tests, inline-comment defaults, or
default CI blocking. The
[Campaign 24 closeout](handoffs/2026-05-10-campaign-24-closeout.md) records the
PR chain, prompt-to-artifact audit, validation plan, and future-lane boundary.

Campaign 25 closed as Report Packet Index. It used the
[Report Packet Index proposal](REPORT_PACKET_INDEX_PROPOSAL.md) as its design
brief. The campaign made `target/ripr/reports/index.{json,md}` the
reviewer front door for the uploaded `ripr-reports` packet, grouping explicit
existing artifacts by start-here, PR review story, repair or agent handoff,
evidence, policy or gates, calibration, validation receipts, and SARIF or badge
outputs. It stayed advisory and read-only: no analyzer behavior,
recommendation ranking, gate semantics, editor behavior, mutation execution,
provider calls, source edits, generated tests, inline-comment defaults, hidden
analysis reruns, or default CI blocking. The fixture corpus now pins complete,
sparse, missing-front-panel, blocked-gate, missing-proof, missing-receipt, and
coverage/grip-present packet states. `ripr reports index` now writes the
read-only `target/ripr/reports/index.{json,md}` producer output from explicit
artifact directories. Generated GitHub CI now runs that producer when indexed
artifacts exist, uploads `index.{json,md}` with the report packet, and appends
the advisory packet-index summary without changing gate authority. The
[report packet index workflow](REPORT_PACKET_INDEX_WORKFLOW.md) now explains
how reviewers, maintainers, developers, and coding agents use the index.
`cargo xtask dogfood` now checks the repo-local report-packet index receipts
for complete, sparse, missing-front-panel, blocked-gate, missing-proof,
missing-receipts, and coverage/grip-present cases. The
[Campaign 25 closeout](handoffs/2026-05-10-campaign-25-closeout.md) records
the PR chain, prompt-to-artifact audit, validation plan, advisory boundary, and
future-lane boundary.

Campaign 26 is closed as PR Inline Comment Publisher. It used the
[PR Inline Comment Publisher proposal](PR_INLINE_COMMENT_PUBLISHER_PROPOSAL.md)
and [RIPR-SPEC-0025](specs/RIPR-SPEC-0025-pr-inline-comment-publisher.md) as
its design contract. The campaign made optional durable PR comments safe
by adding a read-only publish plan over existing `ripr review-comments`
artifacts before anything posts to GitHub. It must stay explicit opt-in and
advisory: no analyzer behavior, recommendation ranking, gate semantics, editor
behavior, mutation execution, provider calls, source edits, generated tests,
branch-protection changes, hidden analysis reruns, `pull_request_target`
defaults, or default CI blocking. The fixture corpus under
`fixtures/boundary_gap/expected/pr-inline-comment-publisher/` now pins the
publishable, summary-only, capped, dedupe/upsert, stale-existing, fork or
no-token, and missing-input cases. `ripr pr-comments plan` now writes the
read-only JSON/Markdown publish plan from those explicit inputs without
posting to GitHub or changing gate authority. Generated GitHub CI now keeps
inline comments disabled by default, emits publish-plan artifacts only in
opt-in modes, and posts or updates comments only when `RIPR_COMMENT_MODE=inline`
and the safe plan permits it. `docs/PR_INLINE_COMMENT_PUBLISHER_WORKFLOW.md`
now documents the opt-in workflow, plan review, fork and permission behavior,
dedupe/upsert, rollback, and advisory gate boundary. `cargo xtask dogfood` now
checks repo-local publish-plan receipts without posting real PR comments.
Campaign 26 is closed by
`docs/handoffs/2026-05-10-campaign-26-closeout.md`.

## PR 0: `planning-and-tracking-docs`

Purpose: put the plan, engineering rules, metrics, ADRs, specs, changelog, and
traceability conventions in the repository before analyzer rewrites begin.

Deliverables:

- [x] Update `docs/ROADMAP.md` with the release sequence and PR queue.
- [x] Add an implementation checklist that future PRs can update.
- [x] Add ADR scaffolding and initial ADRs for product-shaping decisions.
- [x] Add spec scaffolding for behavior contracts.
- [x] Add metrics definitions for capability and regression tracking.
- [x] Add learnings and repo-knowledge log.
- [x] Add spec-test-code traceability rules.
- [x] Update the README doc index and metric summary.
- [x] Add a root changelog.
- [x] Add PR review checklist guidance.
- [x] Add contributor workflow guidance.
- [x] Add CI strategy guidance.
- [x] Add dogfooding guidance.
- [x] Add ADR and spec templates.
- [x] Add changelog policy guidance.
- [x] Add scoped evidence-heavy PR doctrine.
- [x] Add first executable policy checks for static language and panic-family
      debt.

Acceptance:

- [x] A contributor can identify the next PR from docs alone.
- [x] A contributor can identify which spec, tests, and code modules belong
      together for a feature.
- [x] The docs state that production and test code should avoid `panic`,
      `unwrap`, and `expect`, and that existing uses are tracked debt.
- [x] The docs preserve the product contract and conservative static language.
- [x] PRs are scoped by production risk rather than line count.

## PR 1: `verify-one-click-extension-install`

Purpose: verify the normal VS Code extension path without requiring users to
install `ripr` separately.

Deliverables:

- [ ] Manual install verification matrix for VS Marketplace and Open VSX.
- [ ] Fresh-profile check with no `ripr` on `PATH`.
- [ ] Server auto-download and checksum verification evidence.
- [ ] Output-channel log checklist for mode, base, config, server path, and
      download source.
- [ ] Clear-error scenarios for disabled auto-download, missing manifest,
      unsupported platform, and checksum mismatch.

Tests and gates:

- [ ] `cd editors/vscode && npm ci`
- [ ] `cd editors/vscode && npm run compile`
- [ ] `cd editors/vscode && npm run package`

## PR 1A: `xtask-policy-checks`

Purpose: expand the initial policy checks into a broader local and CI quality
rail.

Deliverables:

- [ ] Move static language and panic-family checks into CI.
- [ ] Add markdown local link check.
- [ ] Add doc index check for README, docs, specs, and ADRs.
- [ ] Add traceability manifest validation.
- [ ] Add capability matrix validation.
- [ ] Add PR-scope check for production delta and evidence delta.

Acceptance:

- [ ] `cargo xtask ci-fast` runs the core policy checks.
- [ ] Existing debt is allowlisted with counts, and new debt fails the check.
- [ ] Docs explain how to remove allowlist entries as debt is paid down.

## PR 1B: `rust-first-file-policy`

Purpose: keep repo implementation and automation Rust-first by denying
unapproved non-Rust programming files, checked-in executable scripts, and
workflow shell sprawl.

Deliverables:

- [ ] Add Rust-first file policy docs.
- [ ] Add non-Rust allowlist with owner, kind, and reason.
- [ ] Add workflow shell-budget allowlist.
- [ ] Add `cargo xtask check-file-policy`.
- [ ] Add `cargo xtask check-executable-files`.
- [ ] Add `cargo xtask check-workflows`.
- [ ] Wire checks into `cargo xtask ci-fast`.
- [ ] Wire checks into CI.

Acceptance:

- [ ] Rust is documented as the default implementation and automation language.
- [ ] Existing VS Code, workflow, docs, fixture, asset, and config surfaces are
      explicitly allowlisted.
- [ ] New shell, Python, JavaScript, TypeScript, or other programming files
      outside approved surfaces fail the file policy check.
- [ ] Checked-in executable bits fail unless allowlisted.
- [ ] Long workflow run blocks fail unless allowlisted.

Future policy PRs:

- [x] generated-file policy
- [x] dependency-surface policy
- [x] process-spawn policy
- [x] network policy
- [ ] workspace-shape policy
- [ ] architecture import guard
- [ ] public API guard

## PR 1C: `spec-fixture-contracts`

Purpose: make specs and fixtures agent-readable and mechanically checkable
before fixture and golden output work expands.

Deliverables:

- [ ] Add spec format reference.
- [ ] Add test taxonomy reference.
- [ ] Add fixture contract README.
- [ ] Update existing specs to the checked format.
- [ ] Add `cargo xtask check-spec-format`.
- [ ] Add `cargo xtask check-fixture-contracts`.
- [ ] Wire checks into `cargo xtask ci-fast`.
- [ ] Wire checks into CI.

Acceptance:

- [ ] Every `docs/specs/RIPR-SPEC-*.md` has required sections and a valid
      status.
- [ ] Spec filename IDs match title IDs.
- [ ] Future fixture directories must include `SPEC.md`, `diff.patch`, and
      `expected/check.json`.
- [ ] Fixture `SPEC.md` files must include Given/When/Then/Must Not sections.

## PR 1D: `automation-guardrails`

Purpose: finish the first Rust-first policy family by making generated files,
dependency surfaces, process spawning, and network behavior explicit.

Deliverables:

- [ ] Add generated-file allowlist and `cargo xtask check-generated`.
- [ ] Add dependency-surface allowlist and `cargo xtask check-dependencies`.
- [ ] Add process-spawn allowlist and `cargo xtask check-process-policy`.
- [ ] Add network allowlist and `cargo xtask check-network-policy`.
- [ ] Wire checks into `cargo xtask ci-fast`.
- [ ] Wire checks into CI.
- [ ] Update the file policy, CI docs, contributor docs, and PR template.

Acceptance:

- [ ] Tracked generated lockfiles and future fixture goldens require explicit
      allowlist entries.
- [ ] New dependency manager files fail unless they belong to approved Cargo,
      VS Code, or fixture surfaces.
- [ ] New process spawning fails unless allowlisted with a reason.
- [ ] New network behavior fails unless allowlisted with a reason.

## PR 1E: `shape-fix-pr`

Purpose: add the first mutating PR-shaping commands without changing existing
policy semantics.

Deliverables:

- [ ] Add `cargo xtask shape`.
- [ ] Add `cargo xtask fix-pr`.
- [ ] Run `cargo fmt` through `shape`.
- [ ] Sort `.ripr/*.txt` and `policy/*.txt` allowlists through `shape`.
- [ ] Ensure `target/ripr/reports` exists.
- [ ] Write `target/ripr/reports/shape.md`.
- [ ] Write `target/ripr/reports/fix-pr.md`.
- [ ] Document safe mutations and repair guidance.

Acceptance:

- [ ] `cargo xtask shape` passes.
- [ ] `cargo xtask fix-pr` passes.
- [ ] `cargo xtask ci-fast` passes after shaping.
- [ ] Shaping does not add policy exceptions or bless output drift.

## PR 1F: `pr-summary`

Purpose: generate a reviewer packet before human review without mutating source
files.

Deliverables:

- [ ] Add `cargo xtask pr-summary`.
- [ ] Read changed paths from git diff and git status.
- [ ] Write `target/ripr/reports/pr-summary.md`.
- [ ] Classify production delta and evidence/support delta.
- [ ] Classify detected surfaces, public contracts, and policy exceptions.
- [ ] Suggest reviewer focus files.
- [ ] Update `cargo xtask fix-pr` to refresh the PR summary after shaping.

Acceptance:

- [ ] `cargo xtask pr-summary` passes.
- [ ] `target/ripr/reports/pr-summary.md` exists after the command.
- [ ] `cargo xtask fix-pr` refreshes shape, PR summary, and fix-pr reports.

## PR 1G: `automation-path-docs`

Purpose: document the fix/check/guide operating model and the Codex Goals
campaign handoff so automation and analyzer implementation work share the same
review contract.

Deliverables:

- [x] Add a PR automation operating model.
- [x] Document deterministic shaping, non-mutating checks, and repair briefs.
- [x] Document the scoped PR contract.
- [x] Record the automation cutoff that made Campaign 1 safe to leave setup
      mode.
- [x] Link the new docs from the roadmap, documentation map, agent workflow,
      contributor docs, and README.

Acceptance:

- [x] A contributor can identify which cleanup should be automated and which
      changes require explicit judgment.
- [x] A coding agent can identify the next automation PRs without confusing
      them with product campaign work.
- [x] A coding agent can use a standard task template for the analyzer queue.

## PR 1H: `check-pr-precommit`

Purpose: add obvious local gates for cheap pre-commit checks and review
readiness checks.

Deliverables:

- [x] Add `cargo xtask precommit`.
- [x] Add `cargo xtask check-pr`.
- [x] Keep `precommit` cheap and non-mutating.
- [x] Make `check-pr` run the review-ready command set that exists today.
- [x] Update CI, contributor, and agent docs.

Acceptance:

- [x] `cargo xtask precommit` passes on main.
- [x] `cargo xtask check-pr` passes on main.
- [x] `check-pr` does not run release packaging unless the repo later adds a
      path-aware release lane.

## PR 1I: `guided-check-reports`

Purpose: make existing policy checks emit repair briefs instead of only command
failure text.

Deliverables:

- [x] Add a shared report model or helper for Markdown check reports.
- [x] Upgrade static-language, panic-family, file-policy, executable-file,
      workflow, spec-format, fixture-contract, generated, dependency, process,
      and network checks to write reports under `target/ripr/reports`.
- [x] Classify failures as auto-fixable, author decision, reviewer decision, or
      policy exception.
- [x] Include exact rerun commands and exception templates where useful.

Acceptance:

- [x] Each upgraded check writes a useful report on failure.
- [x] Successful checks either write a pass report or are summarized by
      `pr-summary`.
- [x] Report generation does not hide the non-zero exit status of failed checks.

## PR 1J: `ci-report-artifacts`

Purpose: make CI upload review artifacts even when a check fails.

Deliverables:

- [x] Run `cargo xtask pr-summary` where possible in CI.
- [x] Defer metrics report generation until `cargo xtask metrics` exists.
- [x] Upload `target/ripr/reports` with an always step.
- [x] Document report artifact names and expected contents.

Acceptance:

- [x] CI artifacts include the PR summary and any check reports that were
      generated before failure.
- [x] CI remains non-mutating.

## PR 1K: `fixture-golden-scaffolding`

Purpose: add the command surface for fixture execution and golden comparison
before analyzer internals change.

Deliverables:

- [x] Add `cargo xtask fixtures`.
- [x] Add `cargo xtask fixtures <name>`.
- [x] Add `cargo xtask goldens check`.
- [x] Add `cargo xtask goldens bless <name> --reason "..."`.
- [x] Document the fixture and golden directory conventions.

Acceptance:

- [x] Fixture commands pass with a clear "no fixtures found" message if no
      executable fixtures exist yet.
- [x] Existing fixture contract checks still pass.
- [x] Golden blessing requires an explicit reason.

## PR 1L: `traceability-spec-id-checks`

Purpose: make spec IDs and behavior manifest entries checkable.

Deliverables:

- [x] Harden `.ripr/traceability.toml`.
- [x] Add `cargo xtask check-spec-ids`.
- [x] Add `cargo xtask check-behavior-manifest`.
- [ ] Add warning-only drift checks for analysis, output, docs, fixture, and
      metric changes.

Acceptance:

- [x] Accepted specs point to real docs and at least one test or fixture unless
      explicitly planned.
- [x] Fixture specs reference valid spec IDs.
- [ ] Missing expected evidence appears in the PR summary.

## PR 1M: `capability-metrics-report`

Purpose: make capability progress and automation debt visible.

Deliverables:

- [x] Add or harden a machine-readable capability source.
- [x] Add `cargo xtask metrics`.
- [x] Add `cargo xtask check-capabilities`.
- [x] Write `target/ripr/reports/metrics.md` or `metrics.json`.
- [ ] Keep the README capability snapshot aligned with the capability source.

Acceptance:

- [x] Capability statuses have valid values and required fields.
- [x] Stable or calibrated statuses require the evidence defined by policy.
- [x] Metrics reports are generated without changing product behavior.

## PR 1N: `architecture-guard`

Purpose: protect internal seams while keeping one published package.

Deliverables:

- [x] Add `cargo xtask check-workspace-shape`.
- [x] Add `cargo xtask check-architecture`.
- [x] Add `cargo xtask check-public-api` or document why it is deferred.
- [x] Add policy metadata for allowed workspace packages and module-boundary
      rules.

Acceptance:

- [x] New workspace packages require an explicit approved policy entry.
- [x] Domain and analysis layers cannot accidentally depend on adapters.
- [x] CLI, LSP, and output layers do not own exposure classification.

## PR 1O: `readme-state-and-link-checks`

Purpose: make README state and Markdown links part of the checked trust packet.

Deliverables:

- [x] Add `cargo xtask check-readme-state`.
- [x] Add `cargo xtask markdown-links`.
- [x] Check README front-door sections and headline capability snapshot shape.
- [x] Check README/capability matrix checkpoint drift against
      `metrics/capabilities.toml`.
- [x] Check repo-local Markdown links in tracked `.md` files.
- [x] Wire the checks into `precommit` and `ci-fast`.
- [x] Update CI and PR automation docs.

Acceptance:

- [x] Deleted or renamed docs fail before review when still linked.
- [x] README remains linked to active campaign, metrics, capability, and
      automation docs.
- [x] `cargo xtask check-readme-state` and `cargo xtask markdown-links` pass on
      main.

## PR 1P: `campaign-manifest-check`

Purpose: make the active Codex Goals campaign queue mechanically checkable and
reportable.

Deliverables:

- [x] Add `cargo xtask check-campaign`.
- [x] Add `cargo xtask check-goals` as an alias.
- [x] Add `cargo xtask goals status`.
- [x] Add `cargo xtask goals next`.
- [x] Validate `.ripr/goals/active.toml` against
      `docs/IMPLEMENTATION_CAMPAIGNS.md`.
- [x] Validate work item IDs, statuses, branch fields, acceptance claims,
      stackability, merge boundaries, blocked dependencies, and command names.
- [x] Wire the manifest check into `precommit` and `ci-fast`.

Acceptance:

- [x] `cargo xtask check-campaign` passes on main.
- [x] `cargo xtask goals status` writes `target/ripr/reports/goals.md`.
- [x] `cargo xtask goals next` writes `target/ripr/reports/goals-next.md`.

## PR 1Q: `fixtures-runner-comparison-v1`

Purpose: make fixture and golden commands execute the current product and
compare actual output against checked-in expected output.

Deliverables:

- [x] `cargo xtask fixtures` runs all fixtures when fixture directories exist.
- [x] `cargo xtask fixtures <name>` runs one fixture.
- [x] Actual JSON and human outputs are written under
      `target/ripr/fixtures/<name>/`.
- [x] `cargo xtask goldens check` compares actual `check.json` and optional
      `human.txt` outputs against `fixtures/<name>/expected/`.
- [x] `cargo xtask goldens bless <name> --reason "..."` requires a reason,
      updates `expected/check.json` and `expected/human.txt`, and appends the
      fixture changelog.

Acceptance:

- [x] Fixture commands still pass with a clear report when no fixture
      directories exist.
- [x] Golden checks fail on drift without mutating expected outputs.
- [x] Golden blessing remains explicit and does not run from `shape` or
      `fix-pr`.

## PR 2: `fixture-laboratory`

Purpose: build the regression control bench before changing analyzer internals.

Deliverables:

- [x] `fixtures/boundary_gap`
- [x] `fixtures/weak_error_oracle`
- [ ] `fixtures/field_not_asserted`
- [ ] `fixtures/side_effect_unobserved`
- [ ] `fixtures/smoke_assertion_only`
- [ ] `fixtures/no_static_path`
- [ ] `fixtures/opaque_fixture`
- [ ] `fixtures/workspace_cross_crate`
- [ ] `fixtures/duplicate_symbols`
- [ ] `fixtures/stacked_test_attrs`
- [ ] `fixtures/nested_src_tests_layout`
- [ ] `fixtures/macro_unknown`
- [ ] `fixtures/snapshot_oracle`
- [ ] `fixtures/mock_effect`

Each fixture should include:

- [x] source and tests
- [x] `diff.patch`
- [x] expected JSON output
- [x] expected human output
- [ ] expected context packet
- [ ] expected LSP diagnostic shape when relevant

Invariants:

- [ ] Static output never says `killed` or `survived`.
- [ ] Unknowns include stop reasons.
- [ ] Weak or smoke oracle evidence does not silently become strong.
- [ ] Finding order is deterministic.
- [ ] Context packets are parseable.

## PR 2A: `testing-test-oracle-report`

Purpose: measure `ripr`'s own test oracle strength as analyzer work expands.

Deliverables:

- [x] `cargo xtask test-oracle-report` writes
      `target/ripr/reports/test-oracles.md`.
- [x] `cargo xtask test-oracle-report` writes
      `target/ripr/reports/test-oracles.json`.
- [x] `cargo xtask check-test-oracles` aliases the same advisory report.
- [x] The report classifies detected Rust tests as strong, medium, weak, or
      smoke.
- [x] Existing weak or smoke debt is advisory and non-blocking.

Acceptance:

- [x] `cargo xtask test-oracle-report`
- [x] `cargo xtask check-test-oracles`
- [x] `cargo xtask metrics`
- [x] `cargo xtask check-pr`

## PR 2B: `dogfood-static-self-check`

Purpose: add a focused non-blocking `ripr`-on-`ripr` report.

Deliverables:

- [x] `cargo xtask dogfood` runs stable fixture diffs through
      `ripr check --mode fast`.
- [x] Actual dogfood JSON and human outputs are written under
      `target/ripr/dogfood/<fixture>/`.
- [x] `target/ripr/reports/dogfood.md` summarizes findings, exposure classes,
      runtime, and errors.
- [x] `target/ripr/reports/dogfood.json` provides the same advisory summary for
      future machine readers.
- [x] Dogfood is advisory and non-blocking.

Acceptance:

- [x] `cargo xtask dogfood`
- [x] `cargo xtask check-pr`

## PR 3: `file-facts-model`

Purpose: introduce an internal fact model while preserving current scanner
behavior.

Deliverables:

- [x] `FileFacts`
- [x] `FunctionFact`
- [x] `TestFact`
- [x] `OracleFact`
- [x] `CallFact`
- [x] `ReturnFact`
- [ ] `StructConstructionFact`
- [ ] `EnumConstructionFact`
- [x] `LiteralFact`
- [ ] `BuilderChainFact`
- [ ] `EffectFact`

Acceptance:

- [x] Existing sample findings are unchanged.
- [x] Analysis consumes facts rather than ad hoc scanner structures.
- [x] Scanner behavior remains available as the fallback.

## PR 4: `syntax-adapter-mvp`

Purpose: create the parser boundary before relying on parser-specific details.

Deliverables:

- [x] `RustSyntaxAdapter` trait or equivalent boundary.
- [x] Lexical adapter `summarize_file` implementation.
- [x] Changed range to syntax-node mapping.
- [x] No public API commitment to a parser crate.
- [x] Parser substrate decision recorded in
      [ADR 0006](adr/0006-rust-syntax-substrate.md).
- [x] Parser-backed `summarize_file` implementation.

Acceptance:

- [x] Existing outputs remain stable or intentionally updated with fixture
      evidence.
- [ ] Parser errors produce `static_unknown` or structured diagnostics, not
      panics.

## PR 5: `ast-test-oracle-extraction`

Purpose: extract tests and oracles from syntax nodes instead of line substrings.

Deliverables:

- [x] `#[test]` function extraction.
- [x] Stacked attribute preservation.
- [x] Multi-line assertion macro extraction.
- [x] `assert!`, `assert_eq!`, `assert_ne!`, `assert_matches!`, and `matches!`
      handling.
- [x] `unwrap` and `expect` smoke-oracle handling.

Acceptance:

- [x] Fixture output remains deterministic.
- [x] Line scanning is fallback only.

## PR 6: `ast-probe-ownership`

Purpose: attach probes to stable owner symbols.

Deliverables:

- [x] Diff hunk to changed text range.
- [x] Changed range to syntax-backed owner node.
- [x] Syntax node to enclosing function, method, or module.
- [x] Stable `SymbolId`.

Acceptance:

- [x] Duplicate function names across modules or crates do not cross-link tests.
- [x] Probe IDs remain stable enough for `explain` and `context`.

## PR 7: `ast-probe-generation`

Purpose: generate probes from syntax kind and ownership facts.

Deliverables:

- [x] Predicate boundary probes.
- [x] Return value probes.
- [x] Error path probes.
- [x] Field construction probes.
- [x] Side-effect or call-change probes.
- [x] `static_unknown` fallback with reason.

Acceptance:

- [x] Multi-line predicate changes produce one useful probe.
- [x] Tail-expression return changes produce return probes.
- [x] `Err(Error::X)` changes produce error-path probes.

## PR 8: `oracle-strength-v2`

Purpose: make oracle kind and strength explicit and probe-relative.

Deliverables:

- [x] Exact value oracle.
- [x] Exact error variant oracle.
- [x] Broad error oracle.
- [ ] Whole-object equality oracle.
- [x] Snapshot oracle.
- [x] Mock expectation oracle.
- [x] Relational check oracle.
- [ ] Shape-only oracle.
- [x] Smoke-only oracle.
- [x] Unknown oracle kind.

Acceptance:

- [x] `is_err()` differs from exact error variant assertions.
- [x] `unwrap()` differs from exact return assertions.
- [x] JSON and human output keep the stable schema while rendering
  probe-relative oracle strength.

## PR 9: `local-delta-flow-v1`

Purpose: explain what changed behavior appears to flow to.

Deliverables:

- [x] Changed expression to `let` binding flow.
- [x] Binding to return flow.
- [x] Binding to struct field flow.
- [x] Changed expression to `Ok` or `Err` flow.
- [x] Predicate branch to return or field construction flow.
- [x] Changed call to effect boundary candidate.

Acceptance:

- [x] Findings can name at least one sink when locally visible.
- [x] `propagation_unknown` includes a concrete stop reason.

## PR 10: `activation-value-modeling-v1`

Purpose: detect whether tests appear to activate the changed behavior.

Deliverables:

- [x] Numeric and string literal value facts.
- [x] Function argument value facts.
- [x] Builder-chain value facts.
- [x] Table-row value facts.
- [x] Enum variant value facts.
- [x] Boundary equality discriminator facts.

Acceptance:

- [x] Boundary findings include detected values.
- [x] Boundary findings include missing equality value.
- [x] Opaque fixtures produce `infection_unknown`, not false confidence.

## PR 11: `evidence-first-output`

Purpose: make CLI output the reference explanation.

Deliverables:

- [x] Changed behavior section.
- [x] RIPR stage evidence section.
- [x] Related tests section.
- [x] Oracle evidence section.
- [x] Missing discriminator section.
- [x] Next step section.
- [x] Stop reason section for unknowns.

Acceptance:

- [x] Golden human and JSON output cover current Campaign 3 fixtures.
- [x] Static language remains conservative.
- [ ] Negative and metamorphic fixtures cover noise-only and syntax-variant
      cases.

## PR 12: `lsp-evidence-hover-actions`

Purpose: make editor diagnostics specific and actionable.

Deliverables:

- [ ] Diagnostic data with finding and probe IDs.
- [ ] Stable diagnostic codes.
- [ ] Hover evidence for exact finding.
- [ ] Copy context packet code action.
- [ ] Open related tests code action.
- [ ] Run deep check command.
- [ ] Output-channel lifecycle logs.

Acceptance:

- [ ] `didChange` refreshes diagnostics after debounce.
- [ ] Code action copies the context for the selected finding.

## PR 13: `agent-context-v2`

Purpose: turn `ripr context` into a test-writing brief.

Deliverables:

- [ ] Recommended test location.
- [ ] Related existing tests.
- [ ] Fixture or builder hints.
- [ ] Missing input values.
- [ ] Missing oracle shape.
- [ ] Suggested assertion shapes.
- [ ] Confidence and stop reasons.

Acceptance:

- [ ] Context packet is golden-tested.
- [ ] CLI and LSP use the same packet shape.

## PR 14: `ripr-config-v1`

Purpose: let repositories teach `ripr` topology and oracle conventions.

Deliverables:

- [ ] Workspace-root config discovery.
- [ ] Missing config accepted.
- [ ] Useful invalid-config errors.
- [ ] Test topology override.
- [ ] Custom oracle macro config.
- [ ] Snapshot, mock, and external-boundary config.

Acceptance:

- [ ] Config changes oracle classification only through explicit rules.

## PR 15: `suppression-v1`

Purpose: support honest noise control without hiding the model.

Deliverables:

- [ ] Inline suppression comment form.
- [ ] Config suppression form.
- [ ] Required reason.
- [ ] Optional expiry.
- [ ] `--show-suppressed`.

Acceptance:

- [ ] Suppressed findings remain visible when requested.
- [ ] Suppression rate can be measured.

## PR 16: `sarif-ci-policy`

Purpose: support PR workflows without making default CI noisy.

Deliverables:

- [x] SARIF output.
- [x] Markdown summary.
- [x] JSON artifact guidance.
- [x] Advisory mode.
- [x] Opt-in failure modes.
- [x] Baseline-aware mode.

Acceptance:

- [x] SARIF validates.
- [x] SARIF results point to static evidence locations.
- [x] Blocking policy is opt-in.

## PR 17: `cargo-mutants-calibration-scaffold`

Purpose: compare static predictions with real mutation results.

Deliverables:

- [x] Import cargo-mutants output through `cargo xtask mutation-calibration`
  and public `ripr calibrate cargo-mutants`.
- [x] Match static seam evidence to runtime records by `seam_id` first and
  unambiguous normalized file/line second; report ambiguous file/line
  candidates separately.
- [x] Emit advisory static class vs runtime outcome reports at
  `target/ripr/reports/mutation-calibration.{json,md}`.
- [x] Keep mutation-runtime language out of static findings; runtime vocabulary
  is confined to calibration/runtime reports.

Acceptance:

- [x] Runtime mutation vocabulary appears only in explicit calibration data and
  static-language checks remain clean.

## PR 18: `persistent-cache-v1`

Purpose: cache stable facts after the fact model is worth caching.

Deliverables:

- [ ] File-hash invalidation.
- [ ] Warm `FileFacts` reuse.
- [ ] LSP reuse of test and oracle facts.
- [ ] Graceful stale-cache recovery.

Acceptance:

- [ ] Warm run avoids reparsing unchanged files.

## Required Gates

Rust PRs must run:

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

Extension PRs must run:

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
```
