# Handoff: First Useful PR Loop Continuation Closeout

Date: 2026-05-23

Branch: `campaign-first-useful-pr-loop-continuation-closeout`

Current work item: `campaign/first-useful-pr-loop-continuation-closeout`

Active goal: `first-useful-pr-loop-continuation`

## Current State

First Useful PR Loop Continuation is closed. The campaign made the existing
repair-loop machinery easier to use on one Rust PR without reopening analyzer
truth:

```text
changed behavior
-> top repairable gap or no-action state
-> missing discriminator
-> focused proof intent
-> verify command
-> receipt command or path
-> reviewer- and agent-readable static boundary
```

The final cross-surface state is intentionally static and advisory. The CLI
first screen, generated CI summary, `cargo xtask pr-summary`, VS Code first-pr
and queue packets, agent packets, outcome receipts, output contracts, and goal
board now use one repair unit and one non-claim boundary.

`.ripr/goals/active.toml` now records `status = "closed"` and
`no_current_goal = true`. A future agent must select the next campaign from
repo-owned state rather than continuing from chat history.

## Prompt-To-Artifact Audit

| Requirement | Artifact | Validation |
| --- | --- | --- |
| Campaign selected from repo-owned state | #326 selected `first-useful-pr-loop-continuation`; #328 advanced the queue from the active manifest. | `cargo xtask check-goals`, `cargo xtask goals next`, `cargo xtask check-pr` |
| Proof-stack language stayed repo-native | #329 and #330 reconciled external proof-stack terms into the existing context system without adding a runner-local namespace. | `cargo xtask check-doc-index`, `cargo xtask markdown-links`, `cargo xtask check-static-language`, `cargo xtask check-doc-roles`, `cargo xtask check-pr` |
| Stale goal state fails visibly | #324, #327, and #331 hardened goal/archive validation so closed campaigns and stale archive references do not become hidden agent entrypoints. | `cargo test -p xtask campaign_manifest --bin xtask`, `cargo xtask check-goals`, `cargo xtask goals next`, `cargo xtask check-pr` |
| `ripr first-pr` is the front door | #332 made first-pr stdout explain the top repairable gap or recovery state; #334 closed that slice. | `cargo test -p ripr --lib first_pr`, `cargo xtask fixtures`, `cargo xtask goldens check`, `cargo xtask check-output-contracts`, `cargo xtask check-pr` |
| One-screen recommendation contract is pinned | #335 enforced changed behavior, why it matters, weak evidence, missing discriminator, focused proof intent, verify, receipt, and static boundary; #336 advanced the campaign. | `cargo test -p ripr --lib first_pr`, `cargo xtask goldens check`, `cargo xtask check-output-contracts`, `cargo xtask check-static-language`, `cargo xtask check-pr` |
| Outcome receipts are reviewer-native | #338 made reviewer-visible claim boundaries explicit; #340 advanced the campaign. | `cargo test -p ripr --lib outcome`, `cargo xtask check-output-contracts`, `cargo xtask check-static-language`, `cargo xtask check-pr` |
| Demo story proves the path | #341 pinned the boundary-gap fixture story; #343 advanced the campaign. | `cargo xtask fixtures`, `cargo xtask goldens check`, `cargo xtask check-fixture-contracts`, `cargo xtask check-pr` |
| CI, editor, and agent packets use the same story | #344 aligned generated CI, `pr-summary`, VS Code, and agent packets around changed behavior, missing discriminator, focused proof intent, verify, receipt, and static-advisory non-claims. | `cargo xtask pr-summary`, `cargo xtask lsp-cockpit-report`, `npm --prefix editors/vscode run compile`, `npm --prefix editors/vscode run test:e2e`, `cargo xtask check-output-contracts`, `cargo xtask check-pr` |
| Active goal is closed and archived | This closeout marks the final work item done, closes `.ripr/goals/active.toml`, archives the manifest, and records this handoff. | `cargo xtask closeout --goal first-useful-pr-loop-continuation`, closeout validation |

## PR Chain

- #324 `xtask: reject archived goal reactivation`
- #326 `campaign: activate first useful pr loop continuation`
- #327 `xtask: keep goal archive checks id-only`
- #328 `goals: advance first useful loop queue`
- #329 `docs: reconcile proof stack terminology`
- #330 `docs: keep proof stack guidance repo-native`
- #331 `goals: advance past freshness check`
- #332 `cli: surface first-pr repair context`
- #333 `goals: advance first-pr front door slice`
- #334 `cli: close first-pr front-door polish`
- #335 `output: enforce first-pr one-screen contract`
- #336 `goals: advance one-screen recommendation slice`
- #338 `outcome: make reviewer claim boundary explicit`
- #340 `goals: advance reviewer-native outcome slice`
- #341 `fixtures: pin first-pr demo story contract`
- #343 `goals: advance first-pr demo story slice`
- #344 `surfaces: converge first-pr packet language`
- `campaign: close first useful pr loop continuation`

## Closeout Validation

Closeout validation for this PR:

```bash
cargo xtask check-goals
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

The final surface-convergence PR also passed:

```bash
cargo test -p ripr agent_seam_packets --lib
cargo test -p ripr init_generated_github_workflow --lib
cargo test -p ripr --test cli_smoke editor_agent_loop_fixture_outputs_match_expected
cargo test -p xtask editor_first_pr_bridge --bin xtask
cargo test -p xtask pr_actionable_front_panel --bin xtask
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask pr-summary
cargo xtask lsp-cockpit-report
cargo xtask check-goals
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-doc-roles
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

Local validation produced generated report artifacts under `target/ripr/`.
Those artifacts are validation outputs, not committed source truth.

## Claim And Support-Tier Changes

No support-tier promotion landed. The user-facing claim remains bounded to
advisory static evidence.

What users may believe:

- `ripr first-pr` is the primary front door for one Rust PR;
- the first screen names changed behavior, weak proof, missing discriminator,
  focused proof intent, verify command, receipt command or path, and the
  static-advisory boundary;
- generated CI, VS Code, `pr-summary`, and agent packets now mirror the same
  repair story instead of introducing separate vocabularies;
- outcome receipts are better reviewer context for before/after static
  movement.

What users must not infer:

- runtime adequacy;
- coverage adequacy;
- mutation confirmation;
- proof of correctness;
- policy eligibility;
- gate pass/fail;
- merge readiness;
- support-tier promotion;
- source-edit automation;
- generated tests;
- provider/model execution.

## Policy Ledger Changes

No CI, branch-protection, gate, badge endpoint, release, package, no-panic,
network, dependency, support-tier, or file-policy ledger changed.

The only policy-adjacent change is lifecycle state: the active campaign
manifest is closed, marked with `no_current_goal = true`, and archived for
history.

## Remaining Limits

- Rust remains the stable first-pr path.
- TypeScript, JavaScript, and Python remain preview evidence outside this
  campaign's first useful PR promise.
- Start-here, generated CI, editor handoff, `pr-summary`, and agent packets are
  advisory orientation surfaces, not gate authority.
- Receipts record static artifact relationships and observed movement, not
  mutation proof.
- No successor campaign is selected in `.ripr/goals/active.toml`.

## Artifacts

- `.ripr/goals/active.toml`
- `.ripr/goals/archive/2026-05-23-first-useful-pr-loop-continuation.toml`
- `docs/handoffs/2026-05-23-first-useful-pr-loop-continuation-closeout.md`
- `docs/IMPLEMENTATION_CAMPAIGNS.md`
- `docs/OUTPUT_SCHEMA.md`
- `fixtures/first_successful_pr/boundary-gap/`
- `target/ripr/reports/start-here.md`
- `target/ripr/reports/goals-next.md`
- `target/ripr/reports/lsp-cockpit-report.md`

## Next Recommended Goal

No successor campaign is selected. The active manifest intentionally records
`no_current_goal = true`.

Select the next campaign from repo-owned state in this order:

1. open pull requests and required checks;
2. `cargo xtask goals next`;
3. `docs/IMPLEMENTATION_CAMPAIGNS.md`;
4. `docs/IMPLEMENTATION_PLAN.md`;
5. accepted proposals, specs, ADRs, plans, and open issues that cite them.

Near-term product pressure remains evidence quality and first-use trust: choose
the next campaign from live repo state, not from chat history.

## What Not To Do

- Do not keep adding behavior to First Useful PR Loop Continuation after this
  closeout.
- Do not infer a successor campaign from chat history.
- Do not promote first-pr advice into default blocking or gate authority.
- Do not claim runtime, coverage, mutation, correctness, merge, or policy proof
  from the static first-pr loop.
- Do not add source edits, generated tests, provider calls, mutation execution,
  PR comment publishing, release work, marketplace work, or editor UI expansion
  to this closed campaign.
