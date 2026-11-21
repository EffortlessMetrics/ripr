# Handoff: Source-of-Truth Control Plane Closeout

Date: 2026-05-23

Branch / PR: `docs-source-of-truth-closeout` / #300

Current work item: `docs/source-of-truth-closeout`

Closeout ID: `RIPR-CLOSEOUT-0060`

Linked proposal: [RIPR-PROP-0015](../proposals/RIPR-PROP-0015-source-of-truth-control-plane.md)

Linked spec: [RIPR-SPEC-0060](../specs/RIPR-SPEC-0060-source-of-truth-stack.md)

Linked plan: [RIPR-PLAN-0060](../../plans/source-of-truth/implementation-plan.md)

## Current State

The source-of-truth control-plane lane is accepted and closed for the
repo-local proof stack. The repo now has distinct artifacts for why, what,
durable decisions when needed, PR-sized plans, active-goal state,
support-tier claims, policy ledgers, PR/issue intake, advisory CI proof, graph
reporting, PR body generation, and closeout scaffolding.

The stack is intentionally bounded. It proves registered artifact integrity,
active-goal shape, support-tier proof-command references, and advisory
source-of-truth workflow execution. It does not prove analyzer correctness,
runtime adequacy, coverage adequacy, mutation outcomes, release readiness,
branch-protection promotion, or automatic support-tier or policy-impact
inference.

`.ripr/goals/active.toml` still records `status = "closed"` and
`no_current_goal = true`. This lane does not select a successor campaign.

## What Landed

| Surface | Evidence |
| --- | --- |
| Doctrine docs | #112 added `docs/source-of-truth/` docs for artifact roles, linking, and agent operation. |
| Templates | #116 added proof-stack templates for proposal, spec, ADR, implementation plan, plan item, closeout, and PR body. |
| Proposal | #117 added RIPR-PROP-0015, the source-of-truth control-plane proposal. |
| Spec | #118 added RIPR-SPEC-0060, the behavior contract for the source-of-truth stack. |
| Document artifact ledger | #120 added `policy/doc-artifacts.toml`; #121 added `cargo xtask check-doc-artifacts`. |
| Support-tier claim map | #126 mapped the source-of-truth artifact graph claim; #128 added `cargo xtask check-support-tiers`. |
| PR and issue intake | #130 added source-of-truth PR template fields; #131 added source-of-truth issue templates. |
| Advisory CI | #132 added the advisory Source of Truth workflow. |
| Graph report | #133 added `cargo xtask repo-contract-report`. |
| PR body generator | #135 added `cargo xtask pr-body --work-item <id>`. |
| Closeout generator | #139 added `cargo xtask closeout --goal <goal-id>`. |
| Active goal artifact validation | #177 hardened active-goal artifact reference validation. |
| Current-state reconciliation | #298 added RIPR-PLAN-0060, registered it in the artifact ledger, and aligned proposal/spec/front-door docs with the current validator-backed state. |
| Closeout state | #300 accepted the proposal/spec, marked RIPR-PLAN-0060 done, registered this closeout, and recorded the lane boundary. |

## Proof Executed

Closeout validation for #300:

```bash
cargo xtask check-doc-artifacts
cargo xtask check-goals
cargo xtask check-support-tiers
cargo xtask repo-contract-report
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

The source-of-truth lane also landed focused proof across its implementation
slices:

```bash
cargo test -p xtask doc_artifacts
cargo test -p xtask support_tiers
cargo test -p xtask goals
cargo test -p xtask repo_contract_report
cargo test -p xtask pr_body
cargo test -p xtask closeout
cargo xtask check-doc-artifacts
cargo xtask check-goals
cargo xtask check-support-tiers
cargo xtask repo-contract-report
```

## Claim And Support-Tier Changes

No support-tier promotion landed in this closeout. The existing
`Source-of-truth artifact graph` row remains a `stable building block` claim
backed by `cargo xtask check-doc-artifacts` and
`cargo xtask check-support-tiers`.

The supported claim is narrow:

- registered proposal/spec/plan/closeout IDs, paths, statuses, kind/path fit,
  links, and supersession references are mechanically checked;
- active-goal manifests are checked for valid status, proof commands, artifact
  references, and unsupported policy fields;
- stable or stabilizing support-tier rows must carry proof commands;
- graph reports, PR body generation, and closeout generation are useful
  scaffolds, not automatic proof of completion or policy impact.

## Policy Ledger Changes

`policy/doc-artifacts.toml` now records the accepted proposal and spec, the
done implementation plan, and this done closeout. No CI lane, lint, file,
package, no-panic, network, release, or branch-protection policy changed.

The advisory Source of Truth workflow remains advisory. This closeout does not
promote any source-of-truth check into a required branch-protection check.

## Remaining Work

- Tokmd productization remains future work. This repo-local closeout does not
  add `tokmd init proof-stack`, `tokmd check --profile proof-stack`,
  `tokmd graph`, `tokmd next`, or `tokmd explain`.
- Support-tier claim scanning remains intentionally bounded to the current
  validator behavior. Future README or release-claim scanning needs its own
  scoped PR and support-tier boundary.
- Policy-ledger convergence beyond `policy/doc-artifacts.toml` remains owned by
  the relevant policy ledgers and validators.
- Advisory-to-blocking CI promotion remains deferred until a later explicit
  policy decision after burn-in.

## Artifacts

- `docs/source-of-truth/README.md`
- `docs/source-of-truth/artifact-taxonomy.md`
- `docs/source-of-truth/linking-model.md`
- `docs/source-of-truth/agent-operating-model.md`
- `docs/proposals/RIPR-PROP-0015-source-of-truth-control-plane.md`
- `docs/specs/RIPR-SPEC-0060-source-of-truth-stack.md`
- `plans/source-of-truth/implementation-plan.md`
- `policy/doc-artifacts.toml`
- `docs/status/SUPPORT_TIERS.md`
- `.github/pull_request_template.md`
- `.github/ISSUE_TEMPLATE/`
- `.github/workflows/source-of-truth.yml`
- `docs/handoffs/2026-05-23-source-of-truth-control-plane-closeout.md`
- `target/ripr/reports/source-of-truth-graph.md`
- `target/ripr/reports/source-of-truth-graph.json`

## Next Recommended Goal

No successor campaign is selected. The active manifest intentionally records
`no_current_goal = true`. Select the next campaign from repo-owned state in
this order:

1. open pull requests and required checks;
2. `cargo xtask goals next`;
3. `docs/IMPLEMENTATION_CAMPAIGNS.md`;
4. `docs/IMPLEMENTATION_PLAN.md`;
5. accepted proposals, specs, ADRs, plans, and open issues that cite them.

## What Not To Do

- Do not infer a successor campaign from this closeout or chat history.
- Do not turn advisory source-of-truth checks into blocking checks without a
  separate policy decision.
- Do not duplicate support-tier truth inside specs or proposal text.
- Do not duplicate CI lane truth inside specs.
- Do not claim product correctness, runtime adequacy, coverage adequacy,
  mutation confirmation, release readiness, or merge approval from the
  source-of-truth validators.
- Do not add tokmd product behavior in this repo-local closeout PR.
