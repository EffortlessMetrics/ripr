# Agent operating model

Codex and other coding agents should start from repository artifacts, not from
chat memory. Chat can explain intent, but the repo owns execution state.

## Start order

For normal source-of-truth work in this repo:

1. Read [`.ripr/goals/active.toml`](../../.ripr/goals/active.toml).
2. If the manifest is active, select the next ready work item.
3. Read the linked implementation plan.
4. Read the linked spec.
5. Read the linked proposal for context.
6. Read linked ADRs only when the slice touches durable architecture.
7. Make one PR-sized change.
8. Run the proof commands named by the plan item or prompt.
9. Update support tiers only if product claims change.
10. Update policy ledgers only if policy obligations or exceptions change.
11. Write a PR body with links, scope, proof, claim boundary, and rollback.
12. Add or update closeout notes only when the lane or goal completes.

If the active manifest is closed or stale, do not invent the next campaign. Use
the linked plans and the user's current scoped prompt as the active contract,
and record the gap instead of fabricating manifest state.

## One-slice rule

One PR should carry:

```text
one semantic change
one contract layer
one proof path
one claim boundary
```

Do not combine proposal, spec, validator, CI wiring, support-tier promotion, and
closeout in one PR unless the plan explicitly explains why splitting would make
the evidence less reviewable.

## Verification rule

Agents must verify every named command, workflow, lint, path, feature, and
policy before relying on it. In this repo, `cargo xtask check-goals`,
`cargo xtask goals next`, `cargo xtask check-doc-artifacts`,
`cargo xtask check-support-tiers`, and `cargo xtask repo-contract-report` exist
today. `repo-contract-report` is advisory/report-only proof of the generated
source-of-truth graph packet; it must not be cited as enforcement, support-tier
promotion, or release authority.

## Policy rule

Do not invent repo policies. In particular, do not add fields that reserve
merge completion for a special actor, stronger branch-protection claims,
release authority changes, or new support-tier promises unless current repo
docs and schemas explicitly define them. For scoped implementation, review,
repair, validation, merge, and post-merge verification should finish when
checks and review are clean unless a repo policy or user instruction says
otherwise.

## Claim rule

Every public-facing claim should have one of these states:

- support-tier mapped with a proof command;
- explicitly experimental or advisory;
- explicitly out of scope;
- not claimed.

README copy, release notes, PR bodies, and editor/UI surfaces should not promote
a stronger claim than the support-tier map supports.

## Handoff rule

When stopping work, leave the next agent enough repo-native context to continue:

- changed files;
- proof commands and results;
- unrun validation and why it was skipped;
- remaining work item or blocker;
- claim and policy boundaries.

Prefer durable handoff files under [`docs/handoffs/`](../handoffs/) when a lane
lands or a long-running goal closes. Do not use a handoff to create new behavior
contracts; create or update the proposal/spec/plan chain instead.
