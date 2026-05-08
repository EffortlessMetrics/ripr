# Handoff: Campaign 15 Closeout

Date: 2026-05-08
Branch / PR: `campaign-calibrated-gate-closeout` / #567
Latest merged PR: #566 `docs: add calibrated gate policy guide` (commit `a78fefc`)

## Current Work Item

`campaign/calibrated-gate-closeout`

Campaign 15 turned the existing PR-time evidence loop into an optional policy
decision layer:

```text
PR guidance + existing reports + labels + optional calibration
-> read-only gate evaluation
-> deterministic decision JSON/Markdown
-> optional generated-CI execution
```

The campaign did not change analyzer behavior, add LSP/editor feature work, run
mutation testing, make generated workflows block by default, hide acknowledged
decisions, post comments, edit source, generate tests, add telemetry or external
services, change SARIF or badge schemas, or split the public crate surface.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Gate policy is specified before implementation | #559 added `docs/specs/RIPR-SPEC-0014-calibrated-gate-policy.md`, the gate decision JSON/Markdown contract in `docs/OUTPUT_SCHEMA.md`, traceability, capability metadata, and campaign docs. |
| Evaluator is read-only and consumes existing artifacts | #560 added `ripr gate evaluate` and `crates/ripr/src/output/gate.rs`; the command consumes existing PR guidance plus optional repo exposure, SARIF policy, labels, agent verify, agent receipt, recommendation calibration, mutation calibration, and baseline inputs, and writes `gate-decision.{json,md}`. |
| Default posture remains advisory | RIPR-SPEC-0014, `docs/CALIBRATED_GATE_POLICY.md`, and generated-workflow tests keep `RIPR_GATE_MODE` unset by default; the generated workflow only evaluates the gate when the repository explicitly configures a mode. |
| Blocking requires explicit policy mode | #560 pinned `visible-only`, `acknowledgeable`, `baseline-check`, and `calibrated-gate` modes; `visible-only` remains non-blocking for RIPR evidence, while blocking reports are written before non-zero exit. |
| Waiver labels stay visible | `ripr-waive` is the default acknowledgement label; `fixtures/boundary_gap/expected/calibrated-gate/acknowledged-waiver/` pins an `acknowledged` top-level decision and visible per-candidate reason instead of a silent skip. |
| Fixtures pin the decision matrix | #561 added `fixtures/boundary_gap/expected/calibrated-gate/` for visible-only advisory, acknowledged waiver, baseline-check existing gap, calibrated high-confidence new gap, summary/suppressed candidates, missing input, and calibration disagreement. |
| Generated CI is opt-in and evidence-preserving | #563 wired generated GitHub workflows to run `ripr gate evaluate` only when `RIPR_GATE_MODE` is configured; #564 kept report, SARIF, badge, and gate artifacts uploaded with `always()` behavior even when explicit gate modes fail. |
| Documentation explains operation and boundaries | #566 added `docs/CALIBRATED_GATE_POLICY.md`, updated README/CI/PR guidance/recommendation calibration references, and aligned examples with SARIF policy inputs. |
| Static/runtime vocabulary stays separate | RIPR-SPEC-0014, `docs/CALIBRATED_GATE_POLICY.md`, fixture `limits_note` fields, and `cargo xtask check-static-language` keep runtime mutation language confined to imported calibration context. |

## PR Chain

- #554 `campaign: open calibrated gate policy`
- #559 `spec: define calibrated gate policy`
- #560 `gate: add calibrated policy evaluator`
- #561 `fixtures: pin calibrated gate cases`
- #563 `ci: wire optional generated gate decision`
- #564 `ci: preserve gate evidence uploads`
- #566 `docs: add calibrated gate policy guide`
- `campaign/calibrated-gate-closeout`

## Verification Run

Closeout proof before opening this PR:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

## Next Work Item

No ready work item remains in `.ripr/goals/active.toml`. No Campaign 16 is
opened by this closeout. Future stronger policy, ranking, or adoption-feedback
work should start from a new explicit spec and campaign manifest rather than
extending Campaign 15.

## What Not To Do

- Do not make generated workflows blocking by default.
- Do not run cargo-mutants or any mutation engine from the gate.
- Do not hide acknowledged decisions from summaries.
- Do not post GitHub comments from `ripr gate evaluate`.
- Do not generate tests or edit source from the gate.
- Do not treat runtime mutation calibration as static proof.
- Do not add LSP/editor feature work as part of Campaign 15 maintenance.
- Do not add telemetry, external services, or public crate splits.
- Do not change SARIF, badge, PR guidance, recommendation calibration,
  mutation calibration, or agent receipt schemas without a new compatibility
  contract.
