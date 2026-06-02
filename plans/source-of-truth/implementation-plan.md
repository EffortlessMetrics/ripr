# Source-of-truth control plane implementation plan

Status: done

Owner: repo-infra / source-of-truth

Created: 2026-05-23

Plan ID: RIPR-PLAN-0060

Linked proposal:

- [RIPR-PROP-0015: Source-of-truth Control Plane](../../docs/proposals/RIPR-PROP-0015-source-of-truth-control-plane.md)

Linked specs:

- [RIPR-SPEC-0060: Source-of-truth Stack](../../docs/specs/RIPR-SPEC-0060-source-of-truth-stack.md)

Linked ADRs:

- None.

Linked plan: self

Support-tier impact:

- [Source-of-truth artifact graph](../../docs/status/SUPPORT_TIERS.md) remains
  `stable building block`.

Policy impact:

- Registers this plan in [`policy/doc-artifacts.toml`](../../policy/doc-artifacts.toml).

Required evidence:

- `cargo xtask check-doc-artifacts`
- `cargo xtask check-goals`
- `cargo xtask check-support-tiers`
- `cargo xtask repo-contract-report`
- `cargo xtask check-doc-index`
- `cargo xtask check-static-language`
- `cargo xtask check-pr`
- `git diff --check`

Non-goals:

- No analyzer behavior changes.
- No output schema changes.
- No branch-protection promotion.
- No runner routing changes.
- No release, publish, signing, or marketplace work.
- No support-tier promotion beyond the existing source-of-truth row.
- No policy exception or gate posture change.

Claim boundary:

- This plan records the source-of-truth lane and its proof commands. It does not
  prove product correctness, infer support-tier or policy impact, or make the
  advisory source-of-truth workflow blocking.

Rollback:

- Revert this plan and its ledger/spec links. No runtime behavior changes.

## Current state

The repo has the core source-of-truth stack in place:

- doctrine docs under [`docs/source-of-truth/`](../../docs/source-of-truth/);
- artifact templates under [`docs/templates/`](../../docs/templates/);
- source-of-truth proposal and spec seed artifacts;
- [`policy/doc-artifacts.toml`](../../policy/doc-artifacts.toml);
- `cargo xtask check-doc-artifacts`;
- `cargo xtask check-support-tiers`;
- `.ripr/goals/active.toml` validation through `cargo xtask check-goals`;
- source-of-truth PR and issue templates under [`.github/`](../../.github/);
- advisory Source of Truth workflow under
  [`.github/workflows/source-of-truth.yml`](../../.github/workflows/source-of-truth.yml);
- `cargo xtask repo-contract-report`;
- `cargo xtask pr-body --work-item <id>`;
- `cargo xtask closeout --goal <goal-id>`.

The plan is intentionally descriptive for the closed lane state. It does not
select a new active campaign; `.ripr/goals/active.toml` remains the active-goal
manifest and currently records `no_current_goal = true`.

## Work items

### Work item: docs/source-of-truth-current-state

Status: done

Linked proposal:

- RIPR-PROP-0015

Linked spec:

- RIPR-SPEC-0060

Linked ADR:

- n/a

Blocks:

- `docs/source-of-truth-closeout`

Blocked by:

- n/a

Branch:

- `docs-source-of-truth-enforcement-state`

Issue:

- n/a

PR:

- #298

#### Goal

Make the source-of-truth docs, plan link, and artifact ledger reflect the
current advisory validator state instead of describing the validator as future
or doctrine-only.

#### Production delta

- Add this implementation plan.
- Register this plan in `policy/doc-artifacts.toml`.
- Link `RIPR-SPEC-0060` to this plan.
- Refresh source-of-truth front-door docs so they name the current advisory
  validators and their claim boundary.

#### Non-goals

- No analyzer, editor, CI blocking, runner, release, support-tier promotion, or
  policy-exception behavior changes.

#### Acceptance

- The source-of-truth plan path exists and is registered.
- `RIPR-SPEC-0060` links to the plan path.
- The front-door source-of-truth docs describe current advisory checks without
  claiming product correctness or branch-protection authority.

#### Proof commands

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

#### Support-tier impact

- No tier promotion. The existing source-of-truth row remains limited to
  registered artifact links, active-goal shape, and support-tier proof-command
  references.

#### Policy impact

- `policy/doc-artifacts.toml` gains the plan artifact registration.

#### Claim boundary

- This slice proves doc/ledger consistency for the source-of-truth lane. It does
  not prove product behavior or make the advisory source-of-truth workflow
  blocking.

#### Rollback

- Revert this plan, the `policy/doc-artifacts.toml` row, the spec plan link, and
  the front-door source-of-truth doc edits.

### Work item: docs/source-of-truth-closeout

Status: done

Linked proposal:

- RIPR-PROP-0015

Linked spec:

- RIPR-SPEC-0060

Linked ADR:

- n/a

Blocks:

- n/a

Blocked by:

- `docs/source-of-truth-current-state`

Branch:

- `docs-source-of-truth-closeout`

Issue:

- n/a

PR:

- #300

#### Goal

Accept or close the source-of-truth proposal/spec only after the plan, ledger,
validators, advisory CI, support-tier row, PR/issue templates, graph report, PR
body generator, and closeout generator have current proof recorded in one
closeout.

#### Production delta

- Update proposal/spec status to match current evidence.
- Add a closeout handoff that records proof, claim boundary, and remaining
  future tokmd/productization work.
- Register the closeout in `policy/doc-artifacts.toml`.

#### Non-goals

- No new validator behavior.
- No branch-protection promotion.
- No support-tier promotion beyond the current mapped claim.

#### Acceptance

- Proposal/spec status matches current evidence.
- Closeout records commands run, claim changes, policy changes, remaining work,
  and next recommended goal or `no_current_goal = true`.
- `policy/doc-artifacts.toml` registers the done closeout and marks this plan
  done.

#### Proof commands

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

#### Support-tier impact

- Only update `docs/status/SUPPORT_TIERS.md` if the closeout changes what users
  may believe.

#### Policy impact

- Register any new closeout artifact if the proposal/spec status changes.

#### Claim boundary

- The closeout can claim the source-of-truth control-plane lane is documented
  and mechanically checked only to the extent the cited proof commands support.

#### Rollback

- Revert the closeout and any status/ledger changes. Leave already-landed
  validators and templates in place unless the closeout PR changed them.

## Closeout criteria

- The source-of-truth proposal and spec are accepted only to the proof level
  recorded for the lane.
- The plan, support-tier row, policy ledger, advisory workflow, PR/issue
  templates, graph report, PR body generator, and closeout generator are
  verified current or explicitly left as remaining work.
- `.ripr/goals/active.toml` intentionally records `no_current_goal = true`.
