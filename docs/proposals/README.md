# Proposals

Proposals are design briefs. They explain why a product or repo change should
exist, what shape it should take, and what alternatives were considered. They
are not behavior contracts and are not work queues.

Proposals decompose into:

- one or more behavior specs in [`docs/specs/`](../specs/)
- ADRs in [`docs/adr/`](../adr/) when a durable architectural decision is
  needed
- campaign and work-item entries in
  [`docs/IMPLEMENTATION_CAMPAIGNS.md`](../IMPLEMENTATION_CAMPAIGNS.md) and
  [`docs/IMPLEMENTATION_PLAN.md`](../IMPLEMENTATION_PLAN.md)
- the active execution manifest at `.ripr/goals/active.toml`
- fixtures, tests, goldens, output contracts, and metrics in the rest of the
  repo
- a closeout handoff in [`docs/handoffs/`](../handoffs/) when the campaign
  finishes

## When to write a proposal

Write a proposal when:

- the change spans more than one spec, more than one campaign, or more than
  one product surface
- the change introduces a new public concept (a new language, new evidence
  surface, new public schema area)
- the change reshapes how repo artifacts relate (storage model, doc layout,
  CI topology)
- the design decision is non-obvious and reviewers need the reasoning to
  evaluate it

Do not write a proposal for an ordinary PR-sized fix. Use the PR description
and the existing scoped PR contract instead.

## Naming and lifecycle

Proposals use sequential identifiers:

```text
docs/proposals/RIPR-PROP-NNNN-<kebab-title>.md
```

Status values:

- `proposed`: design is being shaped; specs and ADRs may still change.
- `accepted`: the campaign that implements this proposal is open or has
  landed.
- `superseded`: another proposal replaces this one; link the replacement.
- `withdrawn`: the design is no longer being pursued; record the reason.

The lifecycle is:

```text
proposal (proposed)
  -> behavior specs in docs/specs/
  -> ADRs in docs/adr/ when needed
  -> campaign + work items in IMPLEMENTATION_CAMPAIGNS.md / IMPLEMENTATION_PLAN.md
  -> active manifest in .ripr/goals/active.toml
  -> fixtures, tests, goldens, output contracts, metrics
  -> closeout handoff in docs/handoffs/
  -> proposal status: accepted
```

When a proposal is accepted, leave it in place as historical context. Do not
keep editing it after the work has shipped; future behavior changes belong
in their own specs and proposals.

## Template

Start new proposals from
[`docs/templates/PROPOSAL_TEMPLATE.md`](../templates/PROPOSAL_TEMPLATE.md).

## Index

| Proposal | Status | Topic |
| --- | --- | --- |
| [RIPR-PROP-0001](RIPR-PROP-0001-multi-language-adapter-preview.md) | proposed | Multi-language adapter preview |
| [RIPR-PROP-0002](RIPR-PROP-0002-lane-1-evidence-quality-leadership.md) | proposed | Lane 1 evidence quality leadership |
