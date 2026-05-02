# Agent Workflows

This repository is intentionally structured so humans and coding agents can work
on long-context goals without relying on chat history.

The core idea is:

```text
objective -> roadmap slice -> implementation-plan item -> spec -> fixture/test
-> code module -> output contract -> metric -> changelog/learning
```

An agent should be able to resume from repository artifacts, choose a safe work
item, produce one scoped PR or blocked report for that item, and leave enough
evidence for the next agent to continue.

## Why This Exists

`ripr` asks whether changed behavior appears to have a meaningful discriminator
in tests. The repository should be built the same way:

- claims are backed by specs and tests
- unknowns are explicit
- behavior is traceable to code
- output contracts have goldens
- capability progress is visible in metrics
- decisions survive in ADRs
- lessons survive in learnings

This is especially important for long-running, agent-assisted work. Chat context
expires, but repository artifacts remain.

## Starting Work

1. Read [Roadmap](ROADMAP.md) for sequence.
2. Read [Implementation plan](IMPLEMENTATION_PLAN.md) for the next scoped PR.
3. Read [Capability matrix](CAPABILITY_MATRIX.md) for current status.
4. Read [PR automation](PR_AUTOMATION.md) for the local shape/check/report loop.
5. Read [Codex Goals](CODEX_GOALS.md) and
   [Implementation campaigns](IMPLEMENTATION_CAMPAIGNS.md) when the task is part
   of a long implementation campaign.
6. Read [Scoped PR contract](SCOPED_PR_CONTRACT.md) for the PR-sized work item
   evidence bar.
7. Read the relevant spec in [Specs](specs/README.md).
8. Check [Spec-test-code traceability](SPEC_TEST_CODE.md) and
   `.ripr/traceability.toml`.
9. Inspect existing tests, fixtures, and goldens before editing code.

## Codex Goals

Codex Goals is the autonomous loop. The repository provides the harness.

A Codex Goals run should:

- recover state from repo artifacts, not chat history
- read `.ripr/goals/active.toml`
- pick the next unblocked implementation-campaign work item
- produce one scoped PR, blocked report, or explicit planning update per work
  item
- run `cargo xtask shape`, `cargo xtask fix-pr`, `cargo xtask check-pr`, and
  `cargo xtask pr-summary`
- continue only to independent or explicitly stackable work items
- stop on policy, architecture, credential, merge, or scope decisions
- leave durable learnings only when future agents should not rediscover them

## Choosing A Subset

When a task is large, choose the smallest vertical slice that can produce a
complete evidence package.

Good subset:

```text
one probe family
one fixture
one output contract
one metric row
one implementation module seam
```

Bad subset:

```text
parser rewrite plus classifier changes plus LSP hovers plus schema redesign
```

Use production delta and evidence delta:

- production delta: the narrow behavior or seam changed
- evidence delta: specs, fixtures, tests, goldens, docs, metrics, ADRs, and
  learnings that make the production delta reviewable

## Finishing Work

Before opening a PR, update the durable artifacts:

- specs for behavior contracts
- tests and fixtures for behavior proof
- goldens for output contracts
- capability matrix and traceability for progress tracking
- README only for headline capability changes
- changelog for user-visible, workflow, or public-doc changes
- ADRs for durable decisions
- learnings for repo knowledge future agents should not rediscover

Then run:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask pr-summary
cargo xtask check-pr
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask check-static-language
cargo xtask check-no-panic-family
```

Run the full release/package gates before marking a branch ready.

## Handoff Notes

Every PR should leave a clear handoff:

- what changed
- what evidence proves it
- what remains out of scope
- what the next roadmap item is
- which metric should move next

If work stops midstream, update [Learnings](LEARNINGS.md) or the relevant spec
with the blocker and current evidence. Do not rely on chat state.
