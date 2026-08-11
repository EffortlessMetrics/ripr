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

The `.ripr/goals/` scheduler was deleted in #1701. Live work selection now
comes from GitHub issues, PRs, checks, reviews, and the local worktree; one
PR's scope is its `ImplementationSliceV1` under `.allow/spec-system/slices/`.
Normative behavior is RIPR-SPEC requirements.

When no PR is ready, reconcile the live state from GitHub:

```bash
gh issue list --state open --limit 20
gh pr list --state open
git status --short
```

A valid blocked-only state is not stale, and it is not permission to choose
work from chat history. Resolve the named blocker in the issue graph.

The retired `cargo xtask pr-body --work-item` and `cargo xtask closeout`
commands generated scaffolds from the deleted goal manifest. Draft PR bodies
from the issue's acceptance criteria and the diff instead.

Both approaches produce scaffolds. Support-tier impact, policy impact, proof
results, and final closeout status stay unchecked until the PR author reviews
the actual diff and validation evidence.

## Enforcement state

The original doctrine slice defined the model before enforcing it. The current
repo now has advisory validators for document artifacts, active goals, and the
support-tier claim map. The source-of-truth workflow runs those three narrow
validators; the graph report remains a separate report-only command. Those
checks are still narrower than product correctness and are not
branch-protection gates.

Existing repo checks cover the model at different layers:

```bash
cargo xtask check-doc-artifacts
cargo xtask check-doc-index
cargo xtask check-spec-format
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-support-tiers
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
