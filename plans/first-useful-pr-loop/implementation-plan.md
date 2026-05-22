# RIPR-PLAN-0028: First Useful PR Loop Implementation Plan

Status: closed

Owner: repo-infra / first-useful-pr-loop

Created: 2026-05-21

Linked proposal: `RIPR-PROP-0009`

Linked specs:

- `RIPR-SPEC-0051`
- `RIPR-SPEC-0020`
- `RIPR-SPEC-0009`
- `RIPR-SPEC-0053`

Linked ADR: n/a

Active goal: `.ripr/goals/active.toml`

Support-tier impact:

- None for this plan slice. Product claim changes stay in
  `docs/status/SUPPORT_TIERS.md`.

Policy impact:

- Registers this plan in `policy/doc-artifacts.toml`.

Required evidence:

- `cargo xtask check-doc-artifacts`
- `cargo xtask check-goals`
- `cargo xtask goals next`
- `cargo xtask pr-body --work-item first-pr/front-door-polish`
- `cargo xtask repo-contract-report`
- `cargo xtask check-doc-index`
- `git diff --check`

Non-goals:

- No analyzer behavior changes.
- No output schema changes.
- No generated CI, editor, or agent packet behavior changes.
- No support-tier promotion.
- No policy or CI enforcement promotion.
- No new active campaign.

Claim boundary:

- This plan records the closed Campaign 28 sequence for humans and agents to
  audit from repo artifacts. It does not select the next campaign or promote
  any support-tier claim by itself.

Rollback:

- Revert this plan, the active-manifest link fields, the plan index entry, and
  the `policy/doc-artifacts.toml` rows added for this plan slice.

## Current state

Campaign 28 is closed and archived. The source-of-truth control-plane doctrine,
templates, artifact ledger, validators, support-tier row, advisory workflow,
graph report, PR-body generator, and closeout generator already exist.

The closed product gap was first-useful-PR adoption, not another standalone
doctrine layer. The active manifest now records `no_current_goal = true` until a
successor campaign is selected. This plan remains the repo-native record for
the proposal, spec, plan, proof commands, claim boundary, and rollback path that
Campaign 28 used without relying on chat history.

## Work items

### Work item: context/proof-stack-reconciliation

Status: done

Linked proposal: `RIPR-PROP-0015`

Linked spec: `RIPR-SPEC-0060`

Linked ADR: n/a

Blocks: `goals/active-freshness-validation`

Blocked by: n/a

Branch: `context-proof-stack-reconciliation`

Issue: n/a

PR: n/a

#### Goal

Reconcile proof-stack language into RIPR's existing context system and
source-of-truth stack.

#### Production delta

Document the repo-native control-plane mapping without creating another active
goals namespace.

#### Acceptance

`docs/source-of-truth/README.md` and
`docs/agent-context/CONTEXT_SYSTEM.md` map proposal, spec, ADR, implementation
plan, active manifest, support tiers, policy ledgers, and closeouts to the
repo-native files users and agents already consume.

#### Proof commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-pr
```

#### Support-tier impact

- No support-tier promotion in this work item.

#### Policy impact

- No policy-ledger mutation in this work item.

#### Claim boundary

- This reconciled language only. It did not add the first-pr front-door product
  behavior.

#### Rollback

- Revert the docs-only reconciliation and rerun the listed proof commands.

### Work item: goals/active-freshness-validation

Status: done

Linked proposal: `RIPR-PROP-0015`

Linked spec: `RIPR-SPEC-0060`

Linked ADR: n/a

Blocks: `first-pr/front-door-polish`

Blocked by: `context/proof-stack-reconciliation`

Branch: `goals-active-freshness-validation`

Issue: n/a

PR: n/a

#### Goal

Reject stale active execution state before agents choose the next work item.

#### Production delta

Extend goal validation so a closed active campaign must declare a successor or
explicit no-current-goal marker, and keep work-item commands and blockers
mechanically checked.

#### Acceptance

Goal checks catch missing work-item commands, missing paths where supported,
done items without proof commands, blocked items without blockers, unsupported
policy fields, and stale branch references where those signals are available.

#### Proof commands

```bash
cargo test -p xtask goals
cargo xtask goals next
cargo xtask check-campaign
cargo xtask check-pr
```

#### Support-tier impact

- No support-tier promotion in this work item.

#### Policy impact

- No policy-ledger mutation in this work item.

#### Claim boundary

- This hardened execution-state validation. It did not implement the first-pr
  UX changes.

#### Rollback

- Revert the goal-validation changes and rerun the listed proof commands.

### Work item: first-pr/front-door-polish

Status: done

Linked proposal: `RIPR-PROP-0009`

Linked spec: `RIPR-SPEC-0051`

Linked ADR: n/a

Blocks: `first-pr/one-screen-recommendation`

Blocked by: `goals/active-freshness-validation`

Branch: `first-pr-front-door-polish`

Issue: n/a

PR: n/a

#### Goal

Make `ripr first-pr --root . --base origin/main --head HEAD` the obvious
front door for one changed Rust PR.

#### Production delta

Preflight git, diff, base/head, Cargo workspace, config/defaults, writable
artifacts, mode, and next command. Expected missing inputs should return clear
recovery packets instead of raw internal errors.

#### Non-goals

- No source edits generated by RIPR.
- No generated tests.
- No provider or model calls.
- No runtime mutation execution.
- No default blocking gate.
- No public badge semantic change.

#### Acceptance

Expected missing inputs produce clear recovery packets and next commands, while
valid first-pr inputs still preserve the static-advisory boundary.

#### Proof commands

```bash
cargo test -p ripr --lib first_pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask check-output-contracts
cargo xtask check-pr
```

#### Support-tier impact

- Update `docs/status/SUPPORT_TIERS.md` only if the PR changes what users may
  rely on.

#### Policy impact

- Added a narrow `policy/process_allowlist.txt` entry for the public first-pr
  front-door git preflight. The preflight uses bounded `git` subprocess probes
  without shell expansion and stays advisory.

#### Claim boundary

- This work item improves first-pr recovery and front-door behavior. It does
  not claim runtime adequacy, coverage adequacy, mutation confirmation,
  correctness, or merge approval.

#### Rollback

- Revert the first-pr front-door changes and rerun the listed proof commands.

### Work item: first-pr/one-screen-recommendation

Status: done

Linked proposal: `RIPR-PROP-0009`

Linked specs:

- `RIPR-SPEC-0051`
- `RIPR-SPEC-0020`

Linked ADR: n/a

Blocks: `outcome/reviewer-native-receipts`

Blocked by: `first-pr/front-door-polish`

Branch: `first-pr-one-screen-recommendation`

Issue: n/a

PR: #176

#### Goal

Stabilize a golden-backed first screen for top gap/no-action, changed behavior,
current evidence strength, missing discriminator, focused proof intent, verify
command, receipt command or explicit unavailable reason, and static-advisory
boundary.

#### Production delta

Update first-pr presentation and goldens after the front-door preflight path is
stable.

#### Acceptance

The first screen names a top actionable gap or no-action state and gives one
focused next step with proof and receipt guidance. Receipt guidance may be an
exact command when the gap ledger carries one, or an explicit unavailable reason
when the receipt command is not available yet.

#### Proof commands

```bash
cargo test -p ripr --lib first_pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
```

#### Support-tier impact

- Update support tiers only if the user-facing claim changes.

#### Policy impact

- None expected.

#### Claim boundary

- The first screen remains advisory static evidence, not runtime or coverage
  authority.

#### Rollback

- Revert the first-screen changes and affected goldens, then rerun the proof
  commands.

### Work item: outcome/reviewer-native-receipts

Status: done

Linked proposal: `RIPR-PROP-0009`

Linked specs:

- `RIPR-SPEC-0009`
- `RIPR-SPEC-0051`

Linked ADR: n/a

Blocks: `fixtures/first-pr-boundary-gap-demo`

Blocked by: `first-pr/one-screen-recommendation`

Branch: `outcome-reviewer-native-receipts`

Issue: n/a

PR: #181

#### Goal

Make `ripr outcome` receipts explain what changed, what RIPR flagged before,
what focused proof was added outside RIPR, what moved after verification, what
remains weak or unknown, and what reviewers should and should not believe.

#### Production delta

Updated outcome receipt rendering, the xtask targeted-test outcome mirror,
schema docs, specs, traceability, tests, and the boundary-gap fixture after the
one-screen recommendation stabilized.

#### Acceptance

Receipts stay usable from CLI, PR summary, editor handoff, and agent packet
surfaces without claiming runtime, coverage, mutation, or correctness authority.

#### Proof commands

```bash
cargo test -p ripr --lib outcome
cargo xtask fixtures
cargo xtask goldens check
cargo xtask check-output-contracts
cargo xtask check-pr
```

#### Support-tier impact

- Update support tiers only if the receipt claim changes.

#### Policy impact

- None expected.

#### Claim boundary

- Receipts describe static before/after movement and remaining unknowns; they
  do not validate runtime adequacy.

#### Rollback

- Revert receipt rendering and related goldens, then rerun the proof commands.

### Work item: fixtures/first-pr-boundary-gap-demo

Status: done

Linked proposal: `RIPR-PROP-0009`

Linked spec: `RIPR-SPEC-0051`

Linked ADR: n/a

Blocks: `surfaces/one-screen-loop-convergence`

Blocked by: n/a

Branch: `fixtures-first-pr-boundary-gap-demo`

Issue: n/a

PR: n/a

#### Goal

Add a tiny canonical demo or fixture story for before -> `ripr first-pr` -> top
gap -> focused external proof -> `ripr outcome` -> receipt.

#### Production delta

Added a case-local boundary-gap demo story to the first successful PR corpus,
linked it from the public demo, and pinned it in the fixture contract.

#### Acceptance

The story can be used as documentation, smoke coverage, release demo, and agent
training path.

#### Proof commands

```bash
cargo xtask fixtures
cargo xtask goldens check
cargo xtask check-output-contracts
cargo xtask check-fixture-contracts
cargo xtask check-pr
```

#### Support-tier impact

- Update support tiers only if the demo changes a claim.

#### Policy impact

- None expected.

#### Claim boundary

- The demo validates the first-pr story in a bounded fixture, not general
  correctness, runtime adequacy, coverage adequacy, mutation proof, or merge
  approval.

#### Rollback

- Revert the fixture/demo assets and rerun the proof commands.

### Work item: surfaces/one-screen-loop-convergence

Status: done

Linked proposal: `RIPR-PROP-0011`

Linked specs:

- `RIPR-SPEC-0053`
- `RIPR-SPEC-0051`

Linked ADR: `ADR-0015`

Blocks: `campaign/first-useful-pr-loop-closeout`

Blocked by: n/a

Branch: `surfaces-one-screen-loop-convergence`

Issue: n/a

PR: #187

#### Goal

Align generated CI, VS Code/editor handoff, and agent packet surfaces to mirror
the same first-useful-pr wording and repair unit.

#### Production delta

Align generated surfaces on changed behavior, missing discriminator, focused
proof intent, verify command, receipt command, artifacts, and
static-advisory boundary.

#### Acceptance

CLI, CI, editor, and agent packet surfaces tell the same first-useful-pr story
without adding default blocking, source edits, generated tests, providers,
mutation execution, or another mental model.

#### Proof commands

```bash
cargo test -p ripr lsp --lib
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
```

#### Support-tier impact

- Update support tiers only if the convergence changes product claims.

#### Policy impact

- None expected unless generated CI posture changes, which should be split if
  possible.

#### Claim boundary

- Surface convergence does not create new analyzer truth, runtime authority, or
  default gate behavior.

#### Rollback

- Revert the surface convergence changes and rerun the proof commands.

### Work item: campaign/first-useful-pr-loop-closeout

Status: done

Linked proposal: `RIPR-PROP-0009`

Linked specs:

- `RIPR-SPEC-0051`
- `RIPR-SPEC-0053`

Linked ADR: n/a

Blocks: n/a

Blocked by: n/a

Branch: `campaign-first-useful-pr-loop-closeout`

Issue: n/a

PR: n/a

#### Goal

Closed Campaign 28 after one command, one top gap or no-action state, one
focused repair intent, one verification command, one receipt, one demo, one
generated-CI summary, one editor handoff, and one agent packet told the same
static-advisory repair story.

#### Production delta

Write the closeout and archive the completed active goal after the campaign
evidence has landed.

#### Acceptance

The closeout records landed work, proof commands, claim changes, policy
changes, remaining work, and the next recommended goal.

#### Proof commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-traceability
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-pr
```

#### Support-tier impact

- Update support tiers only if the closeout changes what users may rely on.

#### Policy impact

- Archive active goal state only after the campaign is complete.

#### Claim boundary

- The closeout records what landed; it does not create new behavior contracts.

#### Rollback

- Revert the closeout/archive changes if the campaign is not complete.

## Closeout criteria

Campaign 28 can close when these are true:

- `ripr first-pr --root . --base origin/main --head HEAD` is the front door for
  one changed Rust PR.
- The first screen names one top repairable gap or one clear no-action state.
- The recommendation includes changed behavior, proof weakness, missing
  discriminator, repair intent, verify command, receipt command, and
  static-advisory boundary.
- `ripr outcome` receipts explain before/after movement and non-claims.
- A fixture or demo validates the before -> first-pr -> repair -> outcome ->
  receipt loop.
- CI, editor, and agent packet surfaces mirror the same repair story.
- Support/status claims remain mapped to proof and preserve advisory/static
  boundaries.
