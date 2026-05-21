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

## Enforcement state

This docs slice defines doctrine only. It does not add validators or CI gates.

Existing repo checks already cover parts of the model:

```bash
cargo xtask check-doc-index
cargo xtask check-spec-format
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-goals
cargo xtask goals next
```

Later source-of-truth PRs may add narrower controls such as
`cargo xtask check-doc-artifacts`, `cargo xtask check-support-tiers`, and
`cargo xtask repo-contract-report`. Until those commands exist in `xtask`, docs
must describe them as planned controls rather than available proof.

## Claim boundary

These files make the source-of-truth stack easier to find and apply. They do not
prove artifact links, validate support-tier claims, enforce policy ledgers, or
change CI behavior.
