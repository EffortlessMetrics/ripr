# Handoff: Self-Hosted Routed Runner Proof Closeout

Date: 2026-06-03

Branch: `campaign-self-hosted-routed-runner-proof-closeout`

Current work item: `campaign/self-hosted-routed-runner-proof-closeout`

Closed goal: `self-hosted-routed-runner-proof`

## Current State

The self-hosted routed Rust proof blocker is closed. The repo now has direct
evidence for both self-hosted routes that were missing from the cutover tracker:

```text
route query
-> selected self-hosted implementation job
-> implementation job success
-> normalized Ripr Rust Small Result success
```

The protected branch surface remains the normalized `Ripr Rust Small Result`.
Conditional implementation jobs for CX53, CX43, CPX42, and GitHub-hosted remain
route-specific proof jobs, not required status checks.

`.ripr/goals/active.toml` now records `status = "closed"` and
`no_current_goal = true`. Future work should be selected from current
repo-owned state, not from this closed proof lane.

## Proof Audit

| Requirement | Evidence | Result |
| --- | --- | --- |
| CX53 primary proof | PR #920 run `26859058862` selected CX53 and passed the normalized result. Result env: `TARGET=cx53`, `REASON=cx53_idle`, `ROUTE_RESULT=success`, `CX53_RESULT=success`, `CX43_RESULT=skipped`, `CPX42_RESULT=skipped`, `GITHUB_RESULT=skipped`, `RUNNER_QUERY=ok`, `RUNNER_TOTAL=23`, `CX53_ONLINE=1`, `CX53_IDLE_READY=1`. | Satisfied |
| CX43 fallback proof | #921 post-merge push run `26860129004` on `main` at `7fe2a6f307a59b0fc2e2112d57a2d196e976ecda` selected CX43 and passed the normalized result. Result env: `TARGET=cx43`, `REASON=cx43_idle`, `ROUTE_RESULT=success`, `CX43_RESULT=success`, `CX53_RESULT=skipped`, `CPX42_RESULT=skipped`, `GITHUB_RESULT=skipped`, `RUNNER_QUERY=ok`, `RUNNER_TOTAL=23`, `CX43_ONLINE=4`, `CX43_IDLE_READY=2`. | Satisfied |
| Branch protection remains normalized-only | Branch protection was rechecked after #921: strict required status checks contain only `Ripr Rust Small Result`. | Satisfied |
| Implementation jobs remain conditional | In the CX53 proof run, hosted/CX43/CPX42 were skipped. In the CX43 proof run, hosted/CX53/CPX42 were skipped. The normalized result consumed only the selected target's success. | Satisfied |
| #24 records current disposition without claiming full machine cutover | #24 comment `4608634496` records the CX53 and CX43 proofs and explicitly leaves broader machine/orchestrator migration and assignment discipline outside this closeout. | Satisfied |
| #34 records proof closeout | #34 comment `4608635545` records the CX53/CX43 proof and closes the self-hosted routed-runner issue. | Satisfied |

## Proof Links

- #24 proof refresh:
  <https://github.com/EffortlessMetrics/ripr-swarm/issues/24#issuecomment-4608634496>
- #34 closeout:
  <https://github.com/EffortlessMetrics/ripr-swarm/issues/34#issuecomment-4608635545>
- CX53 PR proof:
  <https://github.com/EffortlessMetrics/ripr-swarm/actions/runs/26859058862>
- CX43 current-main proof:
  <https://github.com/EffortlessMetrics/ripr-swarm/actions/runs/26860129004>

## PR Chain

- #916 `ci: harden routed rust scratch setup`
- #920 `analysis(ts): bound Bun bridge verdict wording`
- #921 `ci: tune CX43 routed rust scratch guard`
- `campaign: close self-hosted routed runner proof`

## Closeout Validation

Closeout validation for this PR:

```bash
cargo xtask check-goals
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-workflows
cargo xtask check-pr
git diff --check
```

The proof-bearing CI runs also passed their routed Rust checks:

```bash
# PR #920 / CX53
TARGET=cx53
REASON=cx53_idle
CX53_RESULT=success
GITHUB_RESULT=skipped

# #921 post-merge / CX43
TARGET=cx43
REASON=cx43_idle
CX43_RESULT=success
GITHUB_RESULT=skipped
```

## Claim And Support-Tier Changes

No product support-tier promotion landed. This closeout only changes the
repo-ops proof state for the routed Rust CI lane.

What maintainers may believe:

- `ripr-swarm/main` is still protected by the normalized
  `Ripr Rust Small Result` check.
- The routed workflow can select and complete CX53 when a CX53 runner is idle
  and image-ready.
- The routed workflow can select and complete CX43 when CX43 is the selected
  self-hosted route.
- Hosted fallback remains part of the workflow but was not the proof path for
  the closeout runs.

What maintainers must not infer:

- machine/orchestrator migration is complete;
- source `ripr` release authority changed;
- release, publish, signing, marketplace, or badge behavior changed;
- analyzer truth, output schema, product claims, or evidence-to-repair
  actionability changed;
- implementation jobs should be added to branch protection;
- fork PRs may run on self-hosted runners;
- CI pass/fail proves runtime adequacy, mutation adequacy, coverage adequacy,
  policy eligibility, or merge readiness outside the routed check contract.

## Policy Ledger Changes

No support-tier, release, package, network, badge, no-panic, dependency, or
source-promotion policy ledger changed.

The CI policy boundary remains:

- required check: `Ripr Rust Small Result`;
- conditional implementation jobs: routed proof details only;
- source `ripr`: release, security, promotion, and publish/distribution
  authority;
- `ripr-swarm`: normal development trunk.

## Remaining Work

The self-hosted routed-runner proof work has no remaining blocked items.

Work outside this closed goal:

- #24 remains open for broader machine/orchestrator migration and assignment
  discipline.
- Open product/dependency PRs should be handled from current live PR state.
- Any new successor goal must be selected from repo-owned artifacts, not from
  this closeout.

## Archive Updates

- Handoff:
  `docs/handoffs/2026-06-03-self-hosted-routed-runner-proof-closeout.md`
- Archived active goal manifest:
  `.ripr/goals/archive/2026-06-03-self-hosted-routed-runner-proof.toml`
