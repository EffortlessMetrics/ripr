# Handoff: Campaign 20 Closeout

Date: 2026-05-09
Branch / PR: `campaign-test-oracle-assistant-proof-closeout` / pending at authoring
Latest merged PR: #626 `docs: document test-oracle assistant workflow` (commit `88fce44`)

## Current Work Item

`campaign/test-oracle-assistant-proof-closeout`

Campaign 20 proved the PR-time test-oracle assistant loop without changing
analyzer semantics, recommendation ranking, gate policy, LSP behavior,
generated workflow defaults, source-edit behavior, generated-test behavior,
provider calls, mutation execution, public crate shape, release posture, or
security posture:

```text
changed Rust behavior
-> static RIPR evidence
-> PR/editor guidance
-> bounded focused-test handoff
-> after-evidence verification
-> receipt
-> advisory CI/ledger projection
```

The campaign keeps the loop static-evidence-only unless imported runtime
mutation calibration is supplied by another explicit artifact.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Campaign opened explicitly | #622 opened Campaign 20 as Test-Oracle Assistant Proof, separate from PR Evidence Ledger and future editor work. |
| End-to-end proof contract exists | #623 added RIPR-SPEC-0019, output-schema coverage, capability metadata, and traceability for the loop from changed Rust behavior through static evidence, PR/editor guidance, focused-test handoff, verification, receipt, and advisory PR/CI projection. |
| Canonical replay fixture exists | #624 added `fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/` and `crates/ripr/tests/cli_smoke.rs::test_oracle_assistant_canonical_review_loop_fixture_pins_expected_surfaces`, pinning one seam across recommendation, related-test context, suggested focused test, before/after static movement, receipt, and PR ledger projection. |
| Dogfood receipt exists | #625 added `docs/handoffs/2026-05-09-test-oracle-assistant-receipt.md`, tracing seam `67fc764ba37d77bd` through PR guidance, editor/agent packet surfaces, verification commands, after-evidence, receipt, PR evidence ledger, and coverage/grip frontier availability. |
| User workflow docs exist | #626 added `docs/TEST_ORACLE_ASSISTANT_WORKFLOW.md`, explaining the user path from PR recommendation or editor diagnostic to bounded handoff, one focused test, after evidence, receipt, and advisory PR/CI projection without internal report topology. |
| Static evidence limits are preserved | RIPR-SPEC-0019, the canonical fixture README, the dogfood receipt, and the workflow doc all state no source edits, no generated tests, no provider calls, no mutation execution, no runtime adequacy claims, and no default CI blocking. |
| Defaults remain advisory | Campaign docs, generated workflow boundaries, ledger language, receipt language, and the workflow doc keep gate decisions separate from proof-loop evidence. The proof loop does not own pass/fail authority. |
| Current analyzer movement is reported honestly | The dogfood receipt records `weakly_gripped -> weakly_gripped` with `state: unchanged`, showing the loop preserves current static movement instead of claiming improvement that the classifier did not emit. |

## PR Chain

- #622 `campaign: open test-oracle assistant proof`
- #623 `spec: define test-oracle assistant loop`
- #624 `fixtures: pin test-oracle assistant loop`
- #625 `dogfood: record test-oracle assistant receipt`
- #626 `docs: document test-oracle assistant workflow`
- `campaign/test-oracle-assistant-proof-closeout`

## Verification Run

Closeout proof before opening this PR:

```bash
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

No ready work item remains in `.ripr/goals/active.toml` from Campaign 20 after
this closeout.

Choose the next campaign explicitly before opening another product lane. Likely
future work should be separated by lane:

- a public proof-report producer for the assistant loop;
- PR/CI adoption polish such as artifact index ergonomics or optional inline
  comment publishing;
- analyzer evidence improvements that could make the canonical boundary case
  move from `weakly_gripped` to `strongly_gripped`;
- editor UX work that builds on the saved-workspace workflow.

Those should not be folded into Campaign 20 closeout.

## What Not To Do

- Do not make generated workflows blocking by default.
- Do not make assistant-loop proof records the pass/fail authority.
- Do not claim runtime mutation outcomes from static evidence.
- Do not hide summary-only, acknowledged, suppressed, stale, invalid, or
  missing-input states.
- Do not treat coverage movement as test adequacy.
- Do not run cargo-mutants or any mutation engine from proof workflows.
- Do not move analyzer identity, recommendation ranking, gate policy semantics,
  or editor behavior into closeout work.
- Do not generate tests, edit source, post inline comments, or call LLM
  providers from the proof loop by default.
