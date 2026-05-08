# Handoff: Campaign 16 Closeout

Date: 2026-05-08
Branch / PR: `campaign-gate-adoption-ux-closeout` / pending
Latest merged PR: #582 `docs: add RIPR blocking readiness guide` (commit `8009385`)

## Current Work Item

`campaign/gate-adoption-ux-closeout`

Campaign 16 made calibrated gate adoption operational without changing the
default advisory posture:

```text
explicit gate mode
-> copyable CI setup
-> visible waiver and baseline workflows
-> first-screen gate summary
-> checked dogfood receipts
-> blocking-readiness guide
```

The campaign did not change analyzer behavior, recommendation ranking,
generated-test behavior, LSP/editor features, mutation execution, default CI
blocking, release/security posture, public crate shape, or gate policy
semantics.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Generated-CI adoption examples are copyable | #573 added docs for default advisory, `visible-only`, `acknowledgeable`, `baseline-check`, and `calibrated-gate` repository-variable setups. |
| Waivers are visible acknowledgements | #575 documented `ripr-waive`, `target/ci/labels.json`, reviewer workflow, and the boundary between PR-time acknowledgement, durable suppressions, and baselines. |
| Baselines are historical debt, not suppressions | #576 documented `.ripr/gate-baseline.json` creation, review, refresh, shrink-only maintenance, and `baseline-check` adoption for existing debt. |
| Gate summaries are first-screen reviewer surfaces | #578 added generated-CI at-a-glance gate summaries; #581 hardened Markdown escaping for untrusted or artifact-derived summary values. |
| Dogfood receipts prove modes locally | #580 extended `cargo xtask dogfood` with checked receipts for visible-only, acknowledged waiver, baseline-existing, baseline-new, missing-baseline, and explicit calibrated-gate decisions. |
| Blocking readiness is staged | #582 added `docs/BLOCKING_READINESS.md`, explaining when to stay advisory, require acknowledgement, use baseline-check, or enable calibrated blocking. |
| Advisory defaults remain intact | Campaign docs, generated workflow tests, dogfood receipts, and capability metadata keep `default_generated_ci_blocking = false`; gates run only when `RIPR_GATE_MODE` is explicitly configured. |

## PR Chain

- #571 `campaign: open gate adoption ux`
- #573 `docs: add gate adoption examples`
- #575 `docs: add gate waiver workflows`
- #576 `docs: add gate baseline workflow`
- #578 `ci: polish gate decision summary`
- #581 `ci: harden gate summary markdown`
- #580 `dogfood: add gate adoption receipts`
- #582 `docs: add RIPR blocking readiness guide`
- `campaign/gate-adoption-ux-closeout`

## Verification Run

Closeout proof before opening this PR:

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

## Next Work Item

Open Campaign 17, RIPR Zero Adoption, as a separate campaign. The first slice
should define the baseline debt delta report before wiring new CI surfaces.

The campaign should stay in Lane 4: PR summaries, CI projection, artifact
layout, waiver/baseline visibility, and repair paths. It should not redefine
analyzer semantics, gate policy semantics, LSP behavior, mutation execution, or
default blocking.

## What Not To Do

- Do not make generated workflows blocking by default.
- Do not hide acknowledged or waived decisions from summaries.
- Do not treat baselines as suppressions or accepted-forever debt.
- Do not add new gate modes in the adoption closeout.
- Do not run cargo-mutants or any mutation engine from adoption workflows.
- Do not generate tests or edit source from PR/CI adoption surfaces.
- Do not move recommendation ranking, analyzer semantics, or LSP/editor UX into
  Lane 4 closeout work.
- Do not open RIPR Zero Adoption by editing Campaign 16 in place; start it as a
  new explicit campaign.
