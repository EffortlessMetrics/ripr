# Run Codex Goals

Use this guide when starting a Codex `/goal` run against `ripr`.

Codex `/goal` is a multi-PR implementation campaign runner. It is not a command
for producing one PR and stopping automatically.

## Before Starting

Read:

- [Codex Goals](../CODEX_GOALS.md)
- [Implementation campaigns](../IMPLEMENTATION_CAMPAIGNS.md)
- [Implementation plan](../IMPLEMENTATION_PLAN.md)
- [Scoped PR contract](../SCOPED_PR_CONTRACT.md)
- [PR automation](../PR_AUTOMATION.md)
- `.ripr/goals/active.toml`

Run:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask pr-summary
cargo xtask receipts
cargo xtask receipts check
cargo xtask check-campaign
cargo xtask goals next
```

## Prompt

```text
/goal: Advance the active ripr implementation campaign.

You are running Codex Goals for EffortlessMetrics/ripr.

A goal is a multi-PR implementation campaign, not a single PR.

Work through the active campaign until its documented end state is met, you are
blocked, or the run budget is exhausted.

For each work item:
- create or update one scoped PR
- keep the production delta narrow
- add the required evidence package
- run the repo shaping and verification commands
- generate reports and receipts
- open or update the PR
- continue only to the next independent or explicitly stackable work item

Do not combine unrelated work items into one PR.

Do not silently bless goldens, add policy exceptions, add dependencies, change
schemas, add non-Rust automation, or broaden scope.

If blocked, write:

target/ripr/reports/blocked.md

with:
- active campaign
- work item
- failing command
- blocker
- why continuing would broaden scope or require human judgment
- recommended next action
```

## Work Item Rules

For each work item, follow the [Scoped PR contract](../SCOPED_PR_CONTRACT.md).

Continue to another work item only when:

- the next work item is independent, or
- the campaign manifest marks it `stackable = true`

Stop at human review boundaries when:

- the next work item depends on an unmerged PR
- the manifest says `requires_human_merge = true`
- continuing would require a policy exception, schema decision, dependency
  decision, credential, release, or marketplace action

## Reports

Reports go under:

```text
target/ripr/reports/
```

At minimum, a run should leave:

- `pr-summary.md` for each opened PR
- check reports for failed policy or gate commands
- `blocked.md` when the campaign cannot safely continue

The report is the handoff. Do not rely on chat history for campaign state.
