# Handoff: Start-Here Surface Convergence Closeout

Date: 2026-05-22

Branch: `campaign-start-here-surface-convergence-closeout`

Current work item: `campaign/start-here-surface-convergence-closeout`

Archived manifest:
`.ripr/goals/archive/2026-05-22-start-here-surface-convergence.toml`

## Current State

Start-Here Surface Convergence is closed. The campaign made PR/CI, CLI,
receipt, fail-closed, policy, dogfood, and documentation surfaces converge on
the same reviewer and agent question:

```text
what is the one repairable gap or no-action state,
why does it matter,
where should focused proof go,
what verifies movement,
what receipt records it,
and what remains limited or advisory?
```

The campaign stayed advisory by default. It did not change analyzer truth,
recommendation ranking, generated CI blocking, branch protection, PR comment
publishing, preview-language promotion, source editing, generated tests,
provider/model calls, mutation execution, or editor UI scope.

`.ripr/goals/active.toml` now records `status = "closed"` and
`no_current_goal = true`. A future agent must select the next campaign from
repo-owned artifacts rather than continuing from chat history.

## Prompt-To-Artifact Audit

| Requirement | Artifact | Validation |
| --- | --- | --- |
| Source-of-truth stack exists | #210 added `RIPR-PROP-0011`, `RIPR-SPEC-0053`, ADR 0015, the implementation plan, indexes, traceability, and the swarm issue burn-down. | `cargo xtask check-doc-index`, `cargo xtask markdown-links`, `cargo xtask check-doc-roles`, `cargo xtask check-traceability`, `cargo xtask check-pr` |
| PR/CI first screens lead with the canonical start-here unit | #212 and #214 made PR evidence, review front-panel, and generated summary surfaces lead with typed start-here repair fields before raw counts. | `cargo test -p ripr --lib output`, `cargo test -p xtask first_pr`, `cargo test -p xtask pr_evidence_summary --bin xtask`, `cargo xtask check-output-contracts`, `cargo xtask check-pr` |
| CLI surfaces use converged safe-action language | #215 and #217 aligned `doctor`, `first-pr`, `pr-ready`, generated workflow summary copy, safe next actions, fail-closed state names, verify command, receipt command, and receipt path language. | `cargo test -p ripr --lib first_pr`, `cargo test -p ripr --lib help`, `cargo test -p ripr --lib cli`, `cargo test -p ripr --test cli_smoke`, `cargo xtask check-output-contracts`, `cargo xtask check-pr` |
| Receipt lifecycle state is consistent | #218 standardized found, missing, stale, gap mismatch, improved, unchanged, and not-applicable receipt states across agent receipt, first-pr, PR evidence, front-panel, actionable-gap outcome, and editor projection fixtures. | `cargo test -p ripr --lib receipt`, `cargo test -p xtask dogfood`, `cargo xtask check-output-contracts`, `cargo xtask check-pr` |
| No-output and fail-closed states are explicit outside the editor | #220 standardized clean, no-action, missing, stale, wrong-root, disabled, unavailable, malformed, partial, and unsafe output states outside the editor. | `cargo test -p ripr --lib output`, `cargo test -p ripr --test cli_smoke`, `cargo xtask check-output-contracts`, `cargo xtask check-traceability`, `cargo xtask check-pr` |
| Preview promotion criteria are policy-owned | #221 added `docs/policy/PREVIEW_PROMOTION_CRITERIA.md` and support-tier links; #222 locked the criteria into specs and clarified TypeScript/JavaScript/Python advisory boundaries. | `cargo xtask check-doc-index`, `cargo xtask markdown-links`, `cargo xtask check-static-language`, `cargo xtask check-traceability`, `cargo xtask check-capabilities`, `cargo xtask check-pr` |
| External-style receipts exist | #223 added `docs/handoffs/2026-05-22-start-here-surface-convergence-receipts.md`, covering normal Rust, no-action, stale, missing, malformed, disabled preview, preview-limited, and receipt-movement states. | `cargo xtask first-pr`, `cargo xtask dogfood`, docs checks, traceability, `cargo xtask check-pr` |
| Active goal is closed and archived | This closeout marks the final work item done, closes `.ripr/goals/active.toml`, archives the manifest, and records this handoff. | `cargo xtask closeout --goal start-here-surface-convergence`, `cargo xtask goals next`, closeout validation |

## PR Chain

- #210 `campaign: activate start-here surface convergence`
- #212 `report: lead PR evidence with start-here unit`
- #214 `report: lead PR summary with start-here repair unit`
- #215 `cli: converge start-here command language`
- #217 `cli: finish start-here state labels`
- #218 `receipt: standardize lifecycle state`
- #220 `output: standardize start-here states`
- #221 `policy: document preview promotion criteria`
- #222 `policy: lock preview promotion criteria`
- #223 `dogfood: record start-here convergence receipts`
- `campaign: close start-here surface convergence`

## Closeout Validation

Closeout validation for this PR:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-traceability
cargo xtask check-output-contracts
cargo xtask goals next
cargo xtask check-pr
git diff --check
```

The final dogfood receipt PR also passed:

```bash
cargo xtask first-pr
cargo xtask dogfood
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask goals next
git diff --check
cargo xtask check-pr
```

`cargo xtask dogfood` exits successfully while rendering advisory report status
`warn` for an existing generated-CI cockpit regeneration command-count warning.
The start-here, first-useful-action, language preview, editor cockpit, and
editor first-pr receipt rows listed in #223 reported no row errors.

## Claim And Support-Tier Changes

No support-tier promotion landed. #221 updated
`docs/status/SUPPORT_TIERS.md` to point preview evidence policy promotion at
the new criteria, but TypeScript, JavaScript, and Python evidence remains
preview, opt-in, static, advisory, and not gate, RIPR Zero, or baseline-check
eligible without a later explicit policy-owned promotion packet.

## Policy Ledger Changes

No policy exception ledger changed. The policy-facing change is the new
preview promotion criteria reference and its spec/support-tier links. Active
goal lifecycle changed: the campaign manifest is closed and archived.

## Remaining Limits

- Start-here surfaces are advisory orientation surfaces, not gate authority.
- Static evidence does not imply runtime adequacy, coverage adequacy, mutation
  confirmation, general correctness, merge approval, or support-tier promotion.
- Preview language evidence remains opt-in and static-limit bounded.
- Missing, stale, wrong-root, malformed, unsupported, unavailable, unsafe,
  disabled, receipt-mismatched, and first-pr mismatched states fail closed.
- Generated CI remains advisory unless a separate explicit gate policy changes
  that.
- PR comments remain unchanged.
- RIPR still does not edit source, generate tests, call providers, or run
  mutation testing.

## Artifacts

- `.ripr/goals/active.toml`
- `.ripr/goals/archive/2026-05-22-start-here-surface-convergence.toml`
- `docs/handoffs/2026-05-22-start-here-surface-convergence-closeout.md`
- `docs/handoffs/2026-05-22-start-here-surface-convergence-receipts.md`
- `docs/policy/PREVIEW_PROMOTION_CRITERIA.md`
- `docs/specs/RIPR-SPEC-0053-start-here-surface-convergence.md`
- `docs/proposals/RIPR-PROP-0011-start-here-surface-convergence.md`
- `docs/adr/0015-start-here-surfaces-use-canonical-gap-records.md`
- `plans/start-here-surface-convergence/implementation-plan.md`
- `target/ripr/reports/start-here.md`
- `target/ripr/reports/dogfood.md`

## Next Recommended Goal

No successor campaign is selected. The active manifest intentionally records
`no_current_goal = true`. To select new work, inspect repo-owned sources in
this order:

1. open pull requests and required checks;
2. `docs/IMPLEMENTATION_CAMPAIGNS.md`;
3. `docs/IMPLEMENTATION_PLAN.md`;
4. accepted proposals, specs, ADRs, and campaign plans;
5. open issues that cite those repo artifacts.

Record the selected successor in `.ripr/goals/active.toml` before starting new
behavior work.

## What Not To Do

- Do not keep working from Start-Here Surface Convergence after this closeout.
- Do not infer a successor campaign from chat history.
- Do not promote start-here advice into default blocking or gate authority.
- Do not promote preview evidence without a policy-owned promotion packet.
- Do not claim runtime, coverage, mutation, correctness, merge, or support-tier
  proof from the static start-here loop.
- Do not add source edits, generated tests, provider calls, mutation execution,
  PR comments, generated CI blocking, or editor UI expansion to this closed
  campaign.
