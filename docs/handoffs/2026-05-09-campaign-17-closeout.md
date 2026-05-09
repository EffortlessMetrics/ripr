# Handoff: Campaign 17 Closeout

Date: 2026-05-09
Branch / PR: `campaign-ripr-zero-adoption-closeout` / #603
Latest merged PR: #601 `docs: add baseline ledger workflow` (commit `3779c6a`)

## Current Work Item

`campaign/ripr-zero-adoption-closeout`

Campaign 17 made RIPR Zero adoption operational without changing the default
advisory posture:

```text
existing gate decision evidence
-> reviewed baseline ledger
-> baseline debt delta report
-> shrink-only baseline refresh
-> generated CI debt movement artifacts
-> baseline ledger workflow docs
```

The campaign did not change analyzer behavior, recommendation ranking, gate
policy semantics, generated workflow defaults, LSP/editor behavior, mutation
execution, automatic source edits, generated tests, public crate shape, or
release/security posture.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Baseline debt delta contract exists | #590 added RIPR-SPEC-0016, output-schema coverage, capability metadata, and traceability for comparing current PR/CI evidence to reviewed baseline debt without changing analyzer identity, auto-adopting new debt, or making CI blocking by default. |
| Users can create reviewed baseline ledgers | #591 added `ripr baseline create`, stable `.ripr/gate-baseline.json` output, `--dry-run`, overwrite protection, hidden-decision skips, CLI smoke coverage, and output contract docs. |
| Users can see debt movement | #594 added `ripr baseline diff`, advisory `baseline-debt-delta.{json,md}` output, checked mixed fixtures, Markdown summary rendering, deterministic identity matching, and buckets for still-present, resolved, new policy-eligible, acknowledged, suppressed, stale, invalid, and missing-current-input identities. |
| Users can shrink baselines safely | #597 added `ripr baseline update --remove-resolved`, preserving malformed or ambiguous entries for review, removing only resolved reviewed debt, and refusing to auto-adopt new current debt. |
| Generated CI exposes debt deltas without owning pass/fail | #599 runs `ripr baseline diff` only when `RIPR_GATE_BASELINE` and `gate-decision.json` are present, uploads `baseline-debt-delta.{json,md}`, and summarizes movement while the gate evaluator remains the pass/fail authority. |
| Users have an adoption workflow | #601 added `docs/BASELINE_LEDGER_WORKFLOW.md`, linking initial visible-only adoption, reviewed baseline creation, `baseline-check`, shrink-only refresh, new debt review, waiver versus baseline versus suppression, and RIPR 0 under configured scope. |
| Defaults remain advisory | Campaign docs, generated workflow tests, CI docs, and output contracts keep generated workflows non-blocking by default; baseline delta reports explain movement and do not make policy decisions. |

## PR Chain

- #588 `campaign: open RIPR Zero adoption`
- #590 `spec: define baseline debt delta report`
- #591 `baseline: add baseline create`
- #594 `baseline: add baseline diff`
- #597 `baseline: add shrink-only baseline update`
- #599 `ci: upload baseline debt delta artifacts`
- #601 `docs: add baseline ledger workflow`
- `campaign/ripr-zero-adoption-closeout`

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

No ready work item remains in `.ripr/goals/active.toml` from Campaign 17.
Choose the next campaign explicitly before opening another product lane.

The likely next adoption lane is Baseline Ledger v2 / RIPR 0 Reporting: baseline
age, owner/reason metadata, stale baseline warnings, repo-level RIPR 0 status,
debt trends, and top debt areas. That should be opened as a new explicit
campaign rather than folded into Campaign 17 closeout.

## What Not To Do

- Do not make generated workflows blocking by default.
- Do not treat baselines as suppressions or accepted-forever debt.
- Do not auto-adopt new current debt into baselines.
- Do not auto-refresh or rewrite baselines in generated CI.
- Do not hide acknowledged, suppressed, stale, invalid, or missing-input entries
  from debt movement summaries.
- Do not move analyzer identity, recommendation ranking, or gate policy
  semantics into baseline closeout work.
- Do not run cargo-mutants or any mutation engine from adoption workflows.
- Do not generate tests, edit source, or call LLM providers from baseline
  adoption surfaces.
- Do not start the editor lane, analyzer lane, or release lane by editing
  Campaign 17 in place.
