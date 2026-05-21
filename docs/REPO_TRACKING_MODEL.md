# Repo Tracking Model

This is `ripr`'s agent-neutral, repo-owned tracking model. It is the
centralized source of truth for proposals, behavior contracts, architectural
decisions, multi-PR campaigns, the currently active campaign, scoped review
units, and closeouts.

External agents have their own goal or task systems — Codex `/goal`, Kiro
specs/tasks, Claude Code's task tools, Cursor rules, and so on. Those are
agent-side execution state. They consume this repo tracking model; they do
not replace it. Repo tracking is the durable substrate that lets any agent
or operator resume long-running work from artifacts, not from a chat
transcript.

For the Codex `/goal` runner specifically, see [Codex Goals](CODEX_GOALS.md).

## The Layers

Each doc has exactly one role. Avoid mixing roles in one file.

| Layer | Artifact | Source of truth for |
| --- | --- | --- |
| Strategy | [`docs/ROADMAP.md`](ROADMAP.md) | Release direction and product sequencing. |
| Design brief / PRD | [`docs/proposals/`](proposals/) | Why a change should exist, what shape it should take, alternatives, risks, success criteria. |
| Behavior contract | [`docs/specs/`](specs/) | What `ripr` must do, expressed for humans, tests, fixtures, tools, and future agents. |
| Durable decision | [`docs/adr/`](adr/) | Why a load-bearing architectural or product decision was made. |
| Campaign ledger | [`docs/IMPLEMENTATION_CAMPAIGNS.md`](IMPLEMENTATION_CAMPAIGNS.md) | Multi-PR campaign history, open campaigns, and closed-campaign audits. |
| Work queue | [`docs/IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) | Current and upcoming implementation slices. |
| Campaign-specific plan | [`plans/`](../plans/) | Extra sequencing, acceptance, proof commands, and rollback notes for a narrow campaign slice when the ledger would become too dense. |
| Active execution manifest | `.ripr/goals/active.toml` | The current execution campaign; `status = "closed"` requires `successor = "<campaign-id>"` or `no_current_goal = true`. |
| Campaign archive | [`.ripr/goals/archive/`](../.ripr/goals/archive/) | Frozen manifests of closed campaigns. |
| Scoped PR | The PR itself | Mergeable review units, governed by the [scoped PR contract](SCOPED_PR_CONTRACT.md). |
| Closeout | [`docs/handoffs/`](handoffs/) | What happened, what passed, what remains. |
| Generated evidence | `target/ripr/{reports,receipts,fixtures,dogfood}/` | Receipts, summaries, blocked reports, fixtures. |

## Lifecycle

```text
1. Proposal (docs/proposals/RIPR-PROP-NNNN-*.md)
     What problem? Why now? Alternatives? Success criteria?

2. Behavior specs (docs/specs/RIPR-SPEC-NNNN-*.md)
     What must ripr do, exactly?

3. ADRs (docs/adr/ADR-NNNN-*.md), when needed
     What durable architectural decision did we make?

4. Campaign entry (docs/IMPLEMENTATION_CAMPAIGNS.md)
     Objective, end state, work items, dependencies, references.

5. Optional campaign-specific plan (plans/<campaign>/<slice>.md)
     Detailed PR sequence, acceptance, proof commands, and rollback for one
     slice when the campaign ledger should stay concise.
     Reviewer automation treats `plans/` files as documentation evidence and
     campaign-planning input, not production behavior.

6. Active manifest (.ripr/goals/active.toml)
     The agent/operator executes work items one PR at a time while the
     campaign is active. After closeout, the top-level status may be `closed`
     until the next campaign manifest replaces it.

7. Scoped PRs (governed by SCOPED_PR_CONTRACT.md)
     One production delta + the evidence package needed to review it.

8. Closeout (docs/handoffs/YYYY-MM-DD-<campaign>-closeout.md)
     What shipped, what was deferred, what the next campaign should be.

9. Archive (.ripr/goals/archive/YYYY-MM-DD-<campaign>.toml)
     Frozen manifest. Read-only history.
```

A change does not need every layer. Most behavior PRs touch a spec, a
campaign work item, fixtures, code, and a closeout. Only repo-shape or
multi-spec changes need a proposal. Only load-bearing architectural choices
need an ADR.

## Layer separation rules

To prevent overloading individual docs:

- A spec defines behavior. It must not carry a work queue or a campaign
  decision.
- A proposal explains design intent. It must not duplicate a spec's behavior
  contract or define output schemas.
- An ADR records a durable decision. It must not double as a spec, proposal,
  or work plan.
- A campaign ledger entry sequences PRs. It must not redefine specs or
  duplicate proposal reasoning.
- A campaign-specific plan adds operational detail for one campaign slice. It
  must not redefine specs, ADRs, or active manifest state.
- The active manifest names the current execution campaign. It may stay on a
  closed campaign only when the manifest also declares
  `successor = "<campaign-id>"` or `no_current_goal = true`. Closed manifests
  also move to the archive.
- A scoped PR is the smallest reviewable unit. It must not bundle unrelated
  contracts.
- A closeout records what happened. It must not invent new contracts; new
  contracts go through proposal → spec → campaign.

When in doubt about where something belongs, ask which question the reader
will be asking when they reach for the doc. A reader asking "why does this
exist?" wants the proposal. A reader asking "what must `ripr` do?" wants
the spec. A reader asking "what is the agent doing right now?" wants the
active manifest. A reader asking "what shipped last week?" wants the
handoffs.

## Agent neutrality

Any agent or operator runner may consume these artifacts:

- Codex `/goal` reads `.ripr/goals/active.toml` and writes blocked reports
  under `target/ripr/reports/blocked.md`. See [Codex Goals](CODEX_GOALS.md).
- Kiro specs/tasks, Claude Code task tools, Cursor rules, and other agent
  task systems may read the same manifest and the linked campaign
  references.
- A human operator can run the same `cargo xtask` commands the agents do.

External agent state stays on the agent side. Repo tracking stays in the
repo. The two coexist; neither replaces the other.

## Validation

Run:

```bash
cargo xtask check-doc-index
cargo xtask check-campaign
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask goals next
cargo xtask check-pr
```

These checks keep the spec index, ADR index, campaign manifest, focused tracker
manifests, traceability manifest, capability matrix, and PR-shape rails
consistent across the layers above. `check-campaign` also verifies that tracker
manifest paths referenced from campaign docs exist, focused trackers remain
separate from `.ripr/goals/active.toml`, done tracker work items carry proof
commands, declared proposal/plan/spec/receipt/closeout paths exist, and closed
tracker capability rows point at `maintenance`.
