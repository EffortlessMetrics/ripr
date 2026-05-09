# Cost and Verification Policy

## The Core Principle

We are not reducing CI because we want less verification.

We are reducing wasted CI so we can afford more verification, more often, at agentic development
volume.

When every routine documentation fix pays for a full Rust proof including VS Code extension
compilation, coverage, and advisory scans, the cost of even light contributions gets high
enough that contributors (human or AI) learn to batch changes or skip CI. That is the
opposite of the goal.

The target posture: cheap required gates at the front door, advisory evidence by default, and
label-gated expensive proof for PRs that actually need it.

## Why Verification Demand is Rising

Published Blacksmith runner spend from OpenClaw (directional, not audited): roughly $511k
since February, mapped against commit volume, works out to approximately $20 per commit on
Blacksmith runners alone. OpenClaw appears to squash-merge PRs, so commit count is a
reasonable proxy for merged PR count—though the number is directional.

We do not read that as "OpenClaw tests too much." We read it as evidence that verification
demand is rising faster than verification efficiency. Agentic development amplifies this:
agents make more commits, smaller PRs, and more iteration. If each iteration costs $20, the
budget runs out fast.

## How `ripr` Changes the Cost Curve

`ripr` asks the mutation-testing-shaped question at static-analysis prices:

> For the behavior changed in this diff, do the current tests appear to contain a discriminator
> that would notice if that behavior were wrong?

Mutation testing answers the same question but costs 10×–100× more: it runs the test suite
once per mutant, and a real project has thousands of mutants. `ripr` skips execution entirely
and reasons structurally over the diff and the test graph.

This is not a replacement for mutation testing. It is a cheap pre-filter:

- Run `ripr` on every PR (seconds, not minutes).
- Reserve mutation testing for the areas `ripr` flags as weakly exposed.
- Reserve full integration proof for PRs that actually change behavior.

## What `ripr` Does Not Do

`ripr` is a **static** RIPR exposure analyzer. It does **not**:

- Run mutants.
- Claim `killed` or `survived` outcomes.
- Prove test adequacy.
- Replace review or mutation testing.

Findings use conservative static language only: `exposed`, `weakly_exposed`,
`reachable_unrevealed`, `no_static_path`, `infection_unknown`, `propagation_unknown`,
`static_unknown`. Any output using runtime mutation vocabulary is a bug.

## The Three-Posture Model

CI lanes fall into three postures:

| Posture | Purpose | Default behavior |
|---------|---------|-----------------|
| **Required** | Cheap merge-safety and policy invariants. `fmt`, `cargo check`, clippy, focused tests, policy gates. | Blocking on PRs that touch the relevant surface. |
| **Advisory** | Evidence that helps review but should not block until calibrated. Coverage, `ripr` self-dogfood, SARIF, droid review. | Upload artifacts; do not fail the PR. |
| **On-demand / release** | Expensive or release-bearing proof. `cargo package`, VSIX, server archive, release readiness. | `main`, manual dispatch, or release labels only. |

## The $1/PR Hard Ceiling

Ordinary PRs should land well below $0.50 (12–35 LEM). The hard ceiling is $1/PR (roughly
125 LEM on `ubuntu-latest`). PRs that exceed the ceiling require an explicit label
(`full-ci` or `ci-budget-override`) or they fail the soft budget guard.

This ceiling is not the design center—it is the outer bound. The design center is sub-$0.50
for routine contributions.

## Calibration Requirement

The soft budget guard, the `ripr` soft gate, and learned LEM estimates all share one
prerequisite: actuals data. None of them enforce until `target/ci/ci-actuals.json` has
accumulated at least 14 days of run history across the relevant lanes.

Enforcement before actuals exist is premature optimization. We warn first.

## See Also

- [`docs/ci/lem-budgeting.md`](lem-budgeting.md) — LEM definition and band table
- [`docs/ci/budget-guard.md`](budget-guard.md) — soft guard behavior matrix
- [`docs/CI.md`](../CI.md) — full CI strategy reference
- [`policy/ci-budget.toml`](../../policy/ci-budget.toml) — machine-readable budget ledger
