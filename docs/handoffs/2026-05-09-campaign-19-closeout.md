# Handoff: Campaign 19 Closeout

Date: 2026-05-09
Branch / PR: `campaign-pr-evidence-ledger-closeout` / #621
Latest merged PR: #620 `docs: add PR evidence ledger workflow` (commit `bd0e4dd`)

## Current Work Item

`campaign/pr-evidence-ledger-closeout`

Campaign 19 made PR evidence ledgers operational without changing analyzer
identity, gate policy, recommendation ranking, or generated-CI advisory
defaults:

```text
PR guidance
-> optional gate decision
-> optional baseline debt delta
-> optional RIPR Zero status
-> optional repair receipt and coverage summary
-> PR evidence ledger
-> optional coverage/grip frontier report
-> user workflow docs
```

The campaign did not change LSP/editor behavior, mutation execution, automatic
source edits, generated tests, public crate shape, release/security posture, or
branch protection.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Campaign opened explicitly | #615 opened Campaign 19 as PR Evidence Ledger, separate from Campaign 18 RIPR Zero reporting. |
| Ledger contract exists | #616 added RIPR-SPEC-0018, output-schema coverage, capability metadata, and traceability for append-only per-PR movement records without analyzer identity rewrites or default blocking. |
| Ledger producer exists | #617 added `ripr pr-ledger record`, a read-only JSON/Markdown report over existing PR guidance, gate decisions, baseline debt deltas, RIPR Zero status, recommendation calibration, agent receipts, optional coverage, and optional history. |
| Generated CI surfaces the ledger | #618 runs `ripr pr-ledger record` on pull requests when PR guidance exists, uploads `pr-evidence-ledger.{json,md}`, and appends a PR movement summary while leaving `ripr gate evaluate` as the pass/fail authority. |
| Coverage/grip frontier exists | #619 added `ripr coverage-grip frontier`, a read-only report that keeps execution coverage movement and RIPR behavioral grip movement as separate advisory axes. |
| Users have a ledger workflow | #620 added `docs/PR_EVIDENCE_LEDGER_WORKFLOW.md`, explaining waiver aging, baseline burn-down, repair receipts, coverage/grip frontier signals, and movement toward RIPR 0 without internal report topology. |
| Defaults remain advisory | Campaign docs, generated workflow checks, output contracts, CI docs, and capability metadata keep the ledger and frontier as advisory evidence. Gate decisions remain explicit and opt-in. |

## PR Chain

- #615 `campaign: open PR evidence ledger`
- #616 `spec: define PR evidence ledger`
- #617 `report: add PR evidence ledger`
- #618 `ci: surface PR evidence ledger`
- #619 `report: add coverage grip frontier`
- #620 `docs: add PR evidence ledger workflow`
- `campaign/pr-evidence-ledger-closeout`

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

No ready work item remains in `.ripr/goals/active.toml` from Campaign 19 after
this closeout.

Choose the next campaign explicitly before opening another product lane. A
likely future Lane 4 campaign is deeper PR/CI adoption polish: artifact index
ergonomics, optional inline comment publishing, portfolio adoption ledgers, or
coverage/grip frontier projection in generated CI. That should be opened as a
new campaign rather than folded into PR Evidence Ledger closeout.

## What Not To Do

- Do not make generated workflows blocking by default.
- Do not make the PR evidence ledger the pass/fail authority.
- Do not use the ledger to rewrite baselines, adopt new current debt, or hide
  acknowledgements, suppressions, stale entries, invalid entries, or
  missing-input warnings.
- Do not treat coverage movement as test adequacy.
- Do not treat RIPR 0 as perfect tests, 100 percent coverage, or runtime
  mutation confirmation.
- Do not run cargo-mutants or any mutation engine from ledger workflows.
- Do not move analyzer identity, recommendation ranking, gate policy
  semantics, or editor behavior into ledger closeout work.
- Do not generate tests, edit source, post inline comments, or call LLM
  providers from the ledger workflow by default.
