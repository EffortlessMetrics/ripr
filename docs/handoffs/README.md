# Handoff Ledger

Committed handoff documents capture the load-bearing state when work
crosses a real boundary — not every transcript recap. This convention
exists because handoff state needs a discoverable home other than the
session transcript, but the cost of committing every recap is doc
churn that buries the useful entries.

## When to commit a handoff

Use a committed handoff document when **at least one** of:

- **Multi-session work**: the work pauses for hours or days and the
  next session needs the load-bearing state without scrolling
  transcripts.
- **Campaign closeout or opening**: the boundary between two
  campaigns is a useful place to record the final architecture, the
  PR chain, and the deferred items.
- **High-risk endpoint or release work**: anything touching network
  policy, public README badges, store-facing copy, secrets, or
  release pipelines.
- **Major architecture change**: a crate split, a new public module
  surface, a new external service.
- **Owner-stepping-away handoff**: when the owner is leaving for a
  long enough window that an autonomous loop or a fresh-session
  resume needs the current state in a discoverable location.

## When *not* to commit a handoff

Routine work does not need committed handoffs:

- single-session PR work
- mechanical repairs and gate-pass edits
- routine docs PRs
- friction-log entries (those go to `docs/FRICTION_LOG.md` directly,
  not here)
- per-iteration loop summaries (those belong in
  `target/ripr/reports/plan-forward.md` while the loop is running,
  and graduate to a committed handoff only at session boundary)

If the handoff is going to be obsolete in a day, leave it in
`target/ripr/reports/` (gitignored) rather than committing it.

## File layout

```text
docs/handoffs/
├── README.md                                   # this file
└── YYYY-MM-DD-<topic>.md                       # one file per handoff
```

The date prefix sorts handoffs chronologically when a directory listing
is the index. The `<topic>` slug should match the campaign or PR-cluster
the handoff covers (e.g. `2026-05-04-campaign-4a-closeout.md`).

## Current Handoffs

- [0.8.0 source release](2026-06-02-0.8.0-source-release.md)
- [0.8.0 release freeze](2026-06-02-0.8.0-release-freeze.md)
- [0.8.0 swarm freeze](0.8.0-swarm-freeze.md)
- [Python Repair Routing usable-alpha closeout](2026-05-31-python-repair-routing-usable-alpha-closeout.md)
- [TypeScript Preview Completion closeout](2026-05-30-typescript-preview-completion-closeout.md)
- [Adoption Integration Cleanup reconciliation](2026-05-23-adoption-integration-cleanup-reconciliation.md)
- [First Useful PR Loop Continuation closeout](2026-05-23-first-useful-pr-loop-continuation-closeout.md)
- [Actionable Surface Translation closeout](2026-05-23-actionable-surface-translation-closeout.md)
- [Lane 1 Value Resolution Audit Fixes closeout](2026-05-23-lane1-value-resolution-audit-fixes-closeout.md)
- [Source-of-Truth Control Plane closeout](2026-05-23-source-of-truth-control-plane-closeout.md)
- [Value Resolution Audit Delta](2026-05-22-value-resolution-audit-delta.md)
- [Lane 1 Finding Alignment Burn-Down closeout](2026-05-22-lane1-finding-alignment-burndown-closeout.md)
- [Start-Here Surface Convergence closeout](2026-05-22-start-here-surface-convergence-closeout.md)
- [Start-Here Surface Convergence dogfood receipts](2026-05-22-start-here-surface-convergence-receipts.md)
- [First Useful PR Loop closeout](2026-05-22-first-useful-pr-loop-closeout.md)
- [0.7.0 Source Release Proof](2026-05-20-0.7.0-source-release-proof.md)
- [0.7 Release Readiness closeout](2026-05-20-0.7-release-readiness-closeout.md)
- [0.7 Swarm Repair Loop dogfood](2026-05-20-0.7-swarm-repair-loop-dogfood.md)
- [Editor Actionable Gap Queue closeout](2026-05-20-editor-actionable-gap-queue-closeout.md)
- [Editor Actionable Gap Queue dogfood receipts](2026-05-20-editor-actionable-gap-queue-receipts.md)
- [Public Badge Projection Realignment closeout](2026-05-19-public-badge-projection-realignment-closeout.md)
- [Editor Adoption Assurance closeout](2026-05-19-editor-adoption-assurance-closeout.md)
- [Editor Adoption Assurance dogfood receipts](2026-05-19-editor-adoption-assurance-receipts.md)
- [0.6.0 release execution closeout](2026-05-18-0.6.0-release-execution-closeout.md)
- [0.6.0 publish decision](2026-05-18-0.6.0-publish-decision.md)
- [0.6.x final proof](2026-05-18-0.6.x-final-proof.md)
- [0.6.x release freeze](2026-05-18-0.6.x-release-freeze.md)
- [0.6.x PR board disposition](2026-05-18-0.6.x-pr-board-disposition.md)
- [0.6.x release finalization](2026-05-17-0.6.x-release-finalization.md)
- [Editor First-PR Bridge closeout](2026-05-17-editor-first-pr-bridge-closeout.md)
- [Lane 1 Shippable Finding Alignment closeout](2026-05-17-lane1-shippable-finding-alignment-closeout.md)
- [0.6.0 dry-run proof](2026-05-17-0.6.0-dry-run-proof.md)
- [0.6.0 release readiness](2026-05-17-0.6.0-release-readiness.md)
- [Editor First-PR Bridge dogfood receipts](2026-05-17-editor-first-pr-bridge-receipts.md)
- [First-Run UX and Adoption Hardening closeout](2026-05-16-first-run-ux-adoption-hardening-closeout.md)
- [Repo-Ops UX Cockpit closeout](2026-05-16-repo-ops-ux-cockpit-closeout.md)
- [Finding Alignment Consumer Contract v2](2026-05-16-finding-alignment-consumer-contract-v2.md)
- [Editor First-Run Usability closeout](2026-05-16-editor-first-run-usability-closeout.md)
- [Lane 3 First-Run Repair dogfood receipts](2026-05-16-lane3-first-run-repair-receipts.md)
- [Rust Usable Gap Projection closeout](2026-05-15-rust-usable-gap-projection-closeout.md)
- [Editor Gap Cockpit closeout](2026-05-15-editor-gap-cockpit-closeout.md)
- [Generated evidence discipline closeout](2026-05-14-generated-evidence-discipline-closeout.md)
- [Lane 1 User-Visible Output Evidence closeout](2026-05-14-user-visible-output-evidence-closeout.md)
- [Editor Gap Cockpit dogfood receipts](2026-05-14-editor-gap-cockpit-receipts.md)
- [Presentation text consumer contract](2026-05-14-presentation-text-consumer-handoff.md)
- [Policy Operations closeout](2026-05-13-policy-operations-closeout.md)
- [Lane 1 Evidence Quality Leadership closeout](2026-05-13-lane-1-evidence-quality-leadership-closeout.md)
- [Campaign 27 Language Adapter Preview closeout](2026-05-13-campaign-27-closeout.md)
- [Lane 4 PR / CI Review Cockpit closeout](2026-05-13-lane4-pr-ci-review-cockpit-closeout.md)
- [Language Adapter Preview dogfood receipts](2026-05-13-language-adapter-preview-receipts.md)
- [Generated CI cockpit dogfood receipts](2026-05-13-generated-ci-cockpit-receipts.md)
- [Lane 1 Evidence Accuracy Evaluation closeout](2026-05-12-lane-1-evidence-accuracy-closeout.md)
- [Policy Readiness closeout](2026-05-12-policy-readiness-closeout.md)
- [Campaign 26 closeout](2026-05-10-campaign-26-closeout.md)
- [Campaign 25 closeout](2026-05-10-campaign-25-closeout.md)
- [PR inline comment publisher dogfood receipts](2026-05-10-pr-inline-comment-publisher-receipts.md)
- [Report packet index dogfood receipts](2026-05-10-report-packet-index-receipts.md)
- [Campaign 24 closeout](2026-05-10-campaign-24-closeout.md)
- [Campaign 23 closeout](2026-05-09-campaign-23-closeout.md)
- [Campaign 22 closeout](2026-05-09-campaign-22-closeout.md)
- [First useful action dogfood receipts](2026-05-09-first-useful-action-receipts.md)
- [Campaign 21 closeout](2026-05-09-campaign-21-closeout.md)
- [Campaign 20 closeout](2026-05-09-campaign-20-closeout.md)
- [Test-oracle assistant dogfood receipt](2026-05-09-test-oracle-assistant-receipt.md)
- [Campaign 19 closeout](2026-05-09-campaign-19-closeout.md)
- [Campaign 18 closeout](2026-05-09-campaign-18-closeout.md)
- [Campaign 17 closeout](2026-05-09-campaign-17-closeout.md)
- [Editor Evidence UX closeout](2026-05-09-editor-evidence-ux-closeout.md)
- [Editor Evidence UX audit](2026-05-08-editor-evidence-ux-audit.md)
- [Campaign 16 closeout](2026-05-08-campaign-16-closeout.md)
- [Campaign 15 closeout](2026-05-08-campaign-15-closeout.md)
- [Campaign 14 closeout](2026-05-08-campaign-14-closeout.md)
- [Campaign 13 closeout](2026-05-08-campaign-13-closeout.md)
- [PR Review Guidance audit](2026-05-08-pr-review-guidance-audit.md)
- [Campaign 12 closeout](2026-05-08-campaign-12-closeout.md)
- [Campaign 11 closeout](2026-05-07-campaign-11-closeout.md)
- [Campaign 10 closeout](2026-05-07-campaign-10-closeout.md)
- [Editor agent readiness proof](2026-05-07-editor-agent-readiness-proof.md)
- [Editor agent integration contract audit](2026-05-07-editor-agent-integration-contract-audit.md)
- [LLM work loop status report](2026-05-07-llm-work-loop-status.md)
- [Campaign 7 closeout](2026-05-07-campaign-7-closeout.md)
- [Campaign 8 closeout](2026-05-07-campaign-8-closeout.md)
- [Hot sidecar latency audit](2026-05-07-hot-sidecar-latency-audit.md)
- [Repo exposure latency report](2026-05-07-repo-exposure-latency-report.md)
- [Repo exposure warm-path reuse](2026-05-07-repo-exposure-warm-path-reuse.md)

## Template

Use this minimal template for ordinary handoffs. For campaign closeouts,
start from [`docs/templates/CLOSEOUT_TEMPLATE.md`](../templates/CLOSEOUT_TEMPLATE.md) and
keep this ledger's "when to commit" rules intact.

```markdown
# Handoff: <topic>

Date: YYYY-MM-DD
Branch / PR: <branch> / #NNN
Latest merged PR: #NNN <title> (commit <sha>)

## Current work item

<which manifest entry / which PR / which scope>

## Next work item

<the single concrete next action — not a todo list>

## Open decisions

- <decision> — owner: <who> — option set: <…>

## Current blockers

- <blocker> — what would unblock it

## Verification run

<output of the load-bearing gate commands; e.g. `cargo xtask check-pr`,
`cargo xtask check-campaign`, live endpoint curl results>

## Artifacts

- <ephemeral artifacts in `target/ripr/reports/` worth knowing about>
- <committed artifacts the next session should read first>

## Friction log entries

- <links to relevant `docs/FRICTION_LOG.md` entries; whether resolved
  or open>

## Deferred decisions

- <links to relevant `docs/DEFERRED.md` entries; what would trigger
  revisit>

## Recommended next action

<one concrete first step the next session should take>

## What not to do

<things the next session might be tempted to do that the current
session has explicit reason to defer or avoid>
```

The template is minimal on purpose. Most fields are short. If a field
doesn't apply, omit it rather than padding it.

## Relationship to other surfaces

| Surface | When to use it instead |
| --- | --- |
| `docs/FRICTION_LOG.md` | raw, same-day observations of friction; goes here only after multiple sessions or campaign-boundary crossing |
| `docs/LEARNINGS.md` | settled principles; the executor should not add to LEARNINGS from a handoff — graduate from FRICTION_LOG instead |
| `docs/DEFERRED.md` | "v1 simple, revisit later" decisions; reference these from a handoff but don't duplicate them |
| PR body | per-PR review context; the handoff references PRs by number |
| `target/ripr/reports/plan-forward.md` | live, ephemeral, per-iteration plan during an autonomous loop; graduate to a committed handoff only at session boundary |
| `.ripr/goals/active.toml` | machine-readable manifest; a committed handoff complements this rather than duplicating it |

## See also

- [`docs/reference/AGENT_HANDOFF_PROTOCOL.md`](../reference/AGENT_HANDOFF_PROTOCOL.md)
  — the operating contract these handoffs live inside.
- [`docs/SCOPED_PR_CONTRACT.md`](../SCOPED_PR_CONTRACT.md) — the
  scoped-PR contract.
- [`docs/IMPLEMENTATION_CAMPAIGNS.md`](../IMPLEMENTATION_CAMPAIGNS.md)
  — the campaign-level plan.
