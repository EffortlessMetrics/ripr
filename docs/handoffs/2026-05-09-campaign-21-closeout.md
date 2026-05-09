# Handoff: Campaign 21 Closeout

Date: 2026-05-09
Branch / PR: `campaign-test-oracle-assistant-report-closeout` / pending at authoring
Latest merged PR: #631 `docs: explain assistant proof report` (commit `ee2b853`)

## Current Work Item

`campaign/test-oracle-assistant-report-closeout`

Campaign 21 turned the proved test-oracle assistant loop into a public
read-only report surface and optional generated-CI projection:

```text
PR guidance
-> editor or agent handoff
-> before/after static evidence
-> receipt
-> PR evidence ledger
-> optional gate and coverage/grip inputs
-> test-oracle-assistant-proof.{json,md}
-> advisory CI summary and artifact projection
```

The campaign did not change analyzer identity, recommendation ranking, gate
policy semantics, LSP/editor behavior, generated workflow defaults,
source-edit behavior, generated-test behavior, provider calls, mutation
execution, public crate shape, release posture, or security posture.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Campaign opened explicitly | #628 opened Campaign 21 as Test-Oracle Assistant Report Producer, separate from Campaign 20 proof work and future PR/CI polish. |
| Public proof producer exists | #629 added `ripr assistant-loop proof`, `crates/ripr/src/output/test_oracle_assistant_proof.rs`, CLI parsing/help, JSON/Markdown renderers, canonical fixture tests, output-schema docs, traceability, capability tracking, and campaign metadata. |
| Producer is read-only and explicit-input only | #629 keeps all inputs as explicit paths and the output contract says no hidden analysis reruns, source edits, generated tests, provider calls, mutation execution, or CI blocking. |
| Proof report preserves the selected seam and repair packet | #629 tests and fixtures pin seam identity, missing discriminator, placement, suggested focused test, related test, handoff command, verify command, receipt path, PR ledger path, optional gate path, optional coverage/grip path, warnings, and limits. |
| Complete, summary-only, missing, optional, unchanged, and advisory-limit cases are covered | `cargo test -p ripr test_oracle_assistant` covers canonical output plus incomplete selected seam, invalid supplied inputs, summary-only guidance, guidance-only regression, and unknown/no-handoff movement cases. |
| Generated CI projection exists | #630 runs `ripr assistant-loop proof` only when PR guidance, agent brief, before/after evidence, agent receipt, and PR evidence ledger artifacts exist, uploads `test-oracle-assistant-proof.{json,md}`, and appends proof summary content only when the report exists. |
| Generated CI projection remains advisory | #630 keeps the step `continue-on-error: true`, skips projection when required inputs are absent, does not print placeholders, and leaves `ripr gate evaluate` as the only optional pass/fail authority. |
| Reader-facing proof-report docs exist | #631 added `docs/TEST_ORACLE_ASSISTANT_PROOF_REPORT.md`, explaining how reviewers, maintainers, and coding agents read status, first-screen fields, warnings, static movement, optional CI projection, handoff fields, and limits without artifact archaeology. |
| Documentation map and cross-links are updated | #631 linked the proof-report guide from `docs/DOCUMENTATION.md`, `docs/CI.md`, `docs/OUTPUT_SCHEMA.md`, and `docs/TEST_ORACLE_ASSISTANT_WORKFLOW.md`. |
| Static evidence vocabulary boundaries are preserved | The spec, schema docs, proof-report guide, generated-CI summary, and closeout all avoid runtime mutation terms except to say they require imported runtime calibration. |
| Current next work is explicit | `.ripr/goals/active.toml`, `docs/IMPLEMENTATION_CAMPAIGNS.md`, `docs/IMPLEMENTATION_PLAN.md`, `docs/ROADMAP.md`, `docs/CAPABILITY_MATRIX.md`, and `metrics/capabilities.toml` point to this closeout before the campaign is marked done. |

## PR Chain

- #628 `campaign: open test-oracle assistant report producer`
- #629 `report: add test-oracle assistant proof`
- #630 `ci: surface assistant proof artifacts`
- #631 `docs: explain assistant proof report`
- `campaign/test-oracle-assistant-report-closeout`

## Verification Run

Closeout proof before opening this PR:

```bash
cargo test -p ripr test_oracle_assistant
cargo test -p ripr init_generated_github_workflow
cargo xtask check-spec-format
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-output-contracts
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-pr
git diff --check
```

## Next Work Item

No ready work item remains in `.ripr/goals/active.toml` from Campaign 21 after
this closeout.

Choose the next campaign explicitly before opening another product lane.
Likely future work should be separated by lane:

- PR/CI review UX polish such as artifact indexing, optional inline comment
  publishing, proof-report summary card refinement, or portfolio adoption
  ledgers;
- analyzer evidence improvements that could make selected static movement more
  precise;
- editor UX work that makes the saved-workspace proof loop easier to run;
- imported runtime calibration joins that stay explicit and supplied-data-only.

Those should not be folded into Campaign 21 closeout.

## What Not To Do

- Do not make generated workflows blocking by default.
- Do not make assistant proof reports the pass/fail authority.
- Do not claim runtime mutation outcomes from static evidence.
- Do not hide summary-only, acknowledged, suppressed, stale, invalid, or
  missing-input states.
- Do not treat coverage movement as test adequacy.
- Do not run cargo-mutants or any mutation engine from proof workflows.
- Do not move analyzer identity, recommendation ranking, gate policy
  semantics, or editor behavior into closeout work.
- Do not generate tests, edit source, post inline comments, or call LLM
  providers from the proof report by default.
