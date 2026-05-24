# Source-of-truth control plane

This directory defines the repo-native control plane for `ripr` work. It is a
concise front door for the deeper [repo tracking model](../REPO_TRACKING_MODEL.md)
and the longer [spec/proposal system guide](../SPEC_PROPOSAL_SYSTEM.md).

The core rule is:

```text
Do not make every document do every job.
```

Each artifact owns one kind of operating truth:

```text
Proposal = why.
Spec = what.
ADR = durable decision.
Plan = PR-sized sequence.
Active goal = what agents execute now.
Support tiers = what users may believe.
Policy ledgers = what exceptions and obligations exist.
CI = what proved it.
Closeout = what happened.
```

The control plane exists so a maintainer, contributor, Codex session, or CI job
can answer these questions from the repository instead of from chat history:

- why does this work exist;
- what behavior must be true;
- which architecture decision governs it;
- what PR-sized step comes next;
- what proof command validates it;
- what product claim may be made after it lands;
- what policy ledger changed;
- what the last agent did;
- what the next agent should do.

## Documents

- [Source-of-truth control plane proposal](../proposals/RIPR-PROP-0015-source-of-truth-control-plane.md)
  explains why this repo is adopting the control-plane lane.
- [Source-of-truth stack spec](../specs/RIPR-SPEC-0060-source-of-truth-stack.md)
  defines the behavior contract for the linked artifact graph.
- [Source-of-truth implementation plan](../../plans/source-of-truth/implementation-plan.md)
  records the closed PR-sized reconciliation slices and proof commands.
- [Source-of-truth closeout](../handoffs/2026-05-23-source-of-truth-control-plane-closeout.md)
  records what landed, what proof ran, what claims changed, and what remains.
- [Artifact taxonomy](artifact-taxonomy.md) defines each layer's ownership.
- [Linking model](linking-model.md) defines how artifacts form one graph.
- [Agent operating model](agent-operating-model.md) defines how Codex and other
  workers consume the graph.

## Templates

- [Proposal template](../templates/proposal.md)
- [Spec template](../templates/spec.md)
- [ADR template](../templates/adr.md)
- [Implementation plan template](../templates/implementation-plan.md)
- [Plan item template](../templates/plan-item.md)
- [Closeout template](../templates/closeout.md)
- [PR body template](../templates/pr-body.md)

## Current repo path

This repo uses `.ripr/goals/active.toml` as the active execution manifest.
Reusable proof-stack bootstraps may choose another repo-specific active-goal
path, but `ripr` agents should follow the repo-local `.ripr` path unless a
later PR changes the schema and documentation together.

When the active manifest has no ready work items, agents should run
`cargo xtask goals next` and follow the printed blocker boundary. A valid
blocked-only manifest is not stale, and it is not permission to choose work from
chat history as if it belonged to the blocked goal.

`cargo xtask pr-body --work-item <id>` can generate
`target/ripr/reports/source-of-truth-pr-body.md` from one ready or active work
item in the active-goal manifest. It refuses blocked or already done items so
blocked state cannot become PR text by accident. It also requires linked
artifact IDs to resolve through `policy/doc-artifacts.toml`, so a generated PR
body cannot carry an unresolved proposal, spec, or plan reference into review.
`cargo xtask closeout --goal <goal-id>` can generate a handoff scaffold under
`docs/handoffs/` and an archive copy under `.ripr/goals/archive/`.

Both commands produce scaffolds. Support-tier impact, policy impact, proof
results, and final closeout status stay unchecked until the PR author reviews
the actual diff and validation evidence.

## Enforcement state

The original doctrine slice defined the model before enforcing it. The current
repo now has advisory validators and a source-of-truth workflow for the
registered graph, active goals, and support-tier claim map. Those checks are
still narrower than product correctness and are not branch-protection gates.

Existing repo checks cover the model at different layers:

```bash
cargo xtask check-doc-artifacts
cargo xtask check-doc-index
cargo xtask check-spec-format
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-support-tiers
cargo xtask check-goals
cargo xtask goals next
cargo xtask repo-contract-report
```

`cargo xtask repo-contract-report` is advisory and report-only. It writes the
source-of-truth graph packet under `target/ripr/reports/`; it does not add a CI
gate, change support-tier claims, or replace the narrower validators listed
above. The report includes ready and blocked work-item state so the next agent
can see that a blocked item is not selectable without resolving the recorded
blocker first.

## Claim boundary

These files explain the source-of-truth stack and point to the proof commands
that operate it. Enforcement comes from the named `cargo xtask` commands and
the advisory workflow, not from this prose. The current checks prove registered
artifact links, active-goal shape, and support-tier proof-command references;
they do not prove product behavior, infer support-tier or policy impact, promote
CI to blocking, or replace policy-specific ledger checks.
