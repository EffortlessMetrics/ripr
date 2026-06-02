# Handoff: Actionable Surface Translation Closeout

Date: 2026-05-23

Branch: `campaign-actionable-surface-translation-closeout`

Current work item: `campaign/actionable-surface-translation-closeout`

Closeout ID: `RIPR-CLOSEOUT-0059`

Linked proposal: [RIPR-PROP-0016](../proposals/RIPR-PROP-0016-actionable-surface-translation.md)

Linked spec: [RIPR-SPEC-0059](../specs/RIPR-SPEC-0059-actionable-surface-translation.md)

Linked plan: [RIPR-PLAN-0059](../../plans/actionable-surface-translation/implementation-plan.md)

## Current State

Actionable Surface Translation is closed. The campaign made badge-adjacent,
PR evidence, editor status, swarm dry-run, and outcome/trend surfaces lead
with the same repair-first translation unit:

```text
actionable canonical gap
-> repair route
-> verify command
-> receipt state or command
-> static/advisory claim boundary
```

The campaign stayed projection-only. It did not change analyzer truth,
`actionable-gaps` producer schema, support tiers, public badge endpoint
semantics, PR comment publishing, default CI blocking, gate behavior, source
editing, generated tests, provider/model calls, mutation execution, release
behavior, or marketplace behavior.

`.ripr/goals/active.toml` now records `status = "closed"` and
`no_current_goal = true`. A future agent must select the next campaign from
repo-owned state rather than continuing from chat history.

## Prompt-To-Artifact Audit

| Requirement | Artifact | Validation |
| --- | --- | --- |
| Source-of-truth stack exists | #309 added `RIPR-PROP-0016`, linked `RIPR-SPEC-0059` to `RIPR-PLAN-0059`, registered the proposal/spec/plan, updated indexes, selected the active manifest, and kept the activation behavior-free. | `cargo xtask check-doc-artifacts`, `cargo xtask check-goals`, `cargo xtask goals next`, `cargo xtask repo-contract-report`, `cargo xtask check-doc-index`, `cargo xtask markdown-links`, `cargo xtask check-static-language`, `cargo xtask check-doc-roles`, `cargo xtask check-traceability`, `cargo xtask check-capabilities`, `cargo xtask check-support-tiers`, `cargo xtask check-pr` |
| Badge-adjacent copy explains the public count | #314 made badge-basis output define the headline as unresolved actionable static repair gaps, name `canonical_actionable_gap` as public basis, and keep seam-native inventory supporting/internal. | `cargo xtask badge-basis`, `cargo xtask check-badge-diff-policy`, `cargo xtask check-output-contracts`, `cargo xtask check-static-language`, `cargo xtask check-pr` |
| PR evidence leads with actionable delta | #315 made PR evidence/front-panel output lead with advisory actionable repair delta, top next repair packet, receipt state, and blocked/static-limited counts before raw inventory. | `cargo xtask pr-summary`, `cargo xtask check-output-contracts`, `cargo xtask check-pr` |
| Editor status starts with repair clarity | #316 made Show Status start with workspace/current-file actionable state, top repair, related proof, verify command, receipt state, and fail-closed next action; #318 kept the VS Code smoke fixture clean. | `cargo test -p ripr lsp --lib`, `npm --prefix editors/vscode run compile`, `npm --prefix editors/vscode run test:e2e`, `cargo xtask lsp-cockpit-report`, `cargo xtask check-output-contracts`, `cargo xtask check-pr` |
| Swarm dry-run emits a copy-ready bounded packet | #317 added the copy-ready operator packet for `ripr-swarm attempt --dry-run`; #319 hardened the packet so prose-only repair targets fail closed instead of becoming allowed edit files. | `cargo test -p xtask ripr_swarm_attempt --bin xtask`, `cargo xtask check-output-contracts`, `cargo xtask check-pr` |
| Outcome and trend reports lead with movement | #320 added `movement_front` JSON and Markdown front sections for actionable-gap outcomes and evidence-quality trend while keeping receipt-linked movement bounded to `cargo xtask actionable-gap-outcomes`. | `cargo test -p xtask actionable_gap_outcomes --bin xtask`, `cargo test -p xtask evidence_quality_trend --bin xtask`, `cargo xtask lane1-evidence-audit`, `cargo xtask actionable-gap-outcomes`, `cargo xtask evidence-quality-trend`, `cargo xtask check-output-contracts`, `cargo xtask check-pr` |
| Active goal is closed and archived | This closeout marks the final work item done, closes `.ripr/goals/active.toml`, archives the manifest, accepts the proposal/spec, marks the plan done, and records this handoff. | `cargo xtask closeout --goal actionable-surface-translation`, closeout validation |

## PR Chain

- #309 `campaign: activate actionable surface translation`
- #314 `badge: explain actionable basis`
- #315 `pr: surface actionable repair delta`
- #316 `vscode: lead status with repair state`
- #318 `test: keep repair status smoke clean`
- #317 `xtask: emit copy-ready swarm dry-run`
- #319 `xtask: fail closed on prose-only repair targets`
- #320 `outcome: lead with movement front section`
- `campaign: close actionable surface translation`

## Closeout Validation

Closeout validation for this PR:

```bash
cargo xtask check-doc-artifacts
cargo xtask check-goals
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-support-tiers
cargo xtask check-pr
git diff --check
```

The final behavior slice also passed:

```bash
cargo fmt --check
cargo test -p xtask actionable_gap_outcomes --bin xtask
cargo test -p xtask evidence_quality_trend --bin xtask
cargo xtask check-output-contracts
cargo xtask check-goals
cargo xtask check-static-language
cargo xtask lane1-evidence-audit
cargo xtask actionable-gap-outcomes
cargo xtask evidence-quality-trend
cargo xtask check-pr
git diff --check
```

Local validation produced generated report artifacts under `target/ripr/`.
Those artifacts are validation outputs, not committed source truth.

## Claim And Support-Tier Changes

No support-tier promotion landed. `RIPR-PROP-0016`, `RIPR-SPEC-0059`, and
`RIPR-PLAN-0059` move to accepted/done because the scoped surface translation
proof exists, but user-facing claim authority remains unchanged.

What users may believe:

- covered first-screen surfaces now translate existing actionable canonical gap
  evidence into repair-first orientation;
- badge-adjacent copy explains the count as unresolved actionable static repair
  gaps;
- PR evidence, editor status, dry-run packets, and outcome/trend reports expose
  repair route, verify, receipt, and fail-closed boundaries earlier.

What users must not infer:

- runtime adequacy;
- coverage adequacy;
- mutation confirmation;
- general correctness proof;
- merge readiness;
- gate pass/fail;
- policy eligibility;
- support-tier promotion;
- source-edit automation;
- generated tests;
- provider/model execution.

## Policy Ledger Changes

`policy/doc-artifacts.toml` now records:

- `RIPR-PROP-0016` as accepted;
- `RIPR-SPEC-0059` as accepted;
- `RIPR-PLAN-0059` as done;
- `RIPR-CLOSEOUT-0059` as done.

No CI, branch-protection, gate, badge endpoint, release, package, no-panic,
network, dependency, or file-policy ledger changed.

## Remaining Limits

- Covered surfaces translate existing typed evidence; they do not create new
  analyzer truth or rerank actionable packets independently.
- Static evidence remains advisory and does not replace mutation testing.
- Receipt-linked movement requires matching receipt or targeted-test outcome
  artifacts; missing, stale, orphaned, and mismatched receipts stay visible.
- `evidence-quality-trend` reports scorecard movement and points operators to
  `cargo xtask actionable-gap-outcomes` for receipt-linked movement.
- Public badge endpoint refresh remains out of scope unless a dedicated badge
  PR runs the endpoint policy.
- Generated CI remains advisory unless a separate gate policy changes it.

## Artifacts

- `.ripr/goals/active.toml`
- `.ripr/goals/archive/2026-05-23-actionable-surface-translation.toml`
- `docs/handoffs/2026-05-23-actionable-surface-translation-closeout.md`
- `docs/proposals/RIPR-PROP-0016-actionable-surface-translation.md`
- `docs/specs/RIPR-SPEC-0059-actionable-surface-translation.md`
- `plans/actionable-surface-translation/implementation-plan.md`
- `policy/doc-artifacts.toml`
- `docs/OUTPUT_SCHEMA.md`
- `target/ripr/reports/actionable-gap-outcomes.md`
- `target/ripr/reports/evidence-quality-trend.md`

## Next Recommended Goal

No successor campaign is selected. The active manifest intentionally records
`no_current_goal = true`.

Select the next campaign from repo-owned state in this order:

1. open pull requests and required checks;
2. `cargo xtask goals next`;
3. `docs/IMPLEMENTATION_CAMPAIGNS.md`;
4. `docs/IMPLEMENTATION_PLAN.md`;
5. accepted proposals, specs, ADRs, plans, and open issues that cite them.

Near-term product pressure remains the first-use repair loop: keep making the
path from one changed behavior to one missing discriminator, one focused proof,
one verify command, and one receipt easier to run across CLI, PR/CI, editor,
and agent handoff. Open a fresh campaign before starting that behavior work.

## What Not To Do

- Do not keep adding behavior to Actionable Surface Translation after this
  closeout.
- Do not infer a successor campaign from chat history.
- Do not use this campaign to promote support tiers, gates, default CI
  blocking, or badge endpoint semantics.
- Do not claim runtime, coverage, mutation, correctness, merge, or policy proof
  from static surface translation.
- Do not add source edits, generated tests, provider calls, mutation execution,
  PR comment publishing, release work, marketplace work, or editor UI expansion
  to this closed campaign.
