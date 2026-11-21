# Droid Review Rules

Droid review output is an inter-agent repair queue and inspection record.

## Review target

Review changed behavior against the repository artifacts:

- `AGENTS.md`
- `docs/ENGINEERING.md`
- `docs/ARCHITECTURE.md`
- `docs/PR_AUTOMATION.md`
- `docs/SCOPED_PR_CONTRACT.md`
- `docs/CI.md`
- `policy/workflow_allowlist.txt`

## Clean review requirement

Do not emit a naked `LGTM`.

If no actionable findings are emitted, write an inspection record with:

- inspected surfaces;
- checks performed;
- why no comments were emitted;
- residual risk;
- validation signal.

Prefer this wording:

```text
No actionable findings emitted.

Inspected surfaces:
Checks performed:
Why no comments:
Residual risk:
Validation signal:
  Observed:
  Reported:
  Not verified:
```

Avoid vague residual-risk labels such as `minimal`, `low risk`, or `safe`
unless they are tied to concrete observed evidence.

## Finding requirement

Each finding should help another coding agent fix the PR.

Use:

```text
[P0|P1|P2] title

Failure mode:
Why here:
Fix direction:
Validation:
Confidence:
```

## Repair-ready comments

Each finding should preserve enough context for a follow-up coding agent to fix the issue without repeating the research.

* Include failure mode, repo invariant, fix direction, validation, and confidence.
* Do not optimize for short comments at the expense of repair value.
* Preserve useful repo research in the comment or summary.
* If Droid inspected specs, policies, or in-repo docs, name the source.
* Name likely files, tests, policies, and generated outputs involved.
* Include validation commands that would prove the repair.

Treat every actionable comment as a standalone repair packet. A follow-up agent
should be able to identify what can fail, why the repo cares, where to edit,
and how to verify the fix without reconstructing Droid's research.

## Suggested fix policy

Use GitHub suggestion blocks only for high-confidence local edits that should
apply cleanly.

Do not use suggestion blocks for cross-file, policy, fixture, golden,
traceability, schema, or design-dependent changes. Use an ordered repair plan
instead:

1. Name the likely files, tests, policies, and generated outputs involved.
2. Describe the smallest safe repair steps.
3. Include validation commands.

## Evidence provenance

Distinguish observed evidence from reported evidence.

* `Observed:` for CI checks, files, logs, or artifacts Droid directly inspected.
* `Reported:` for claims in PR body, commit message, or comments.
* `Not verified:` for validation Droid did not run or observe.

Do not treat PR-body claims as independently verified facts.

## Notification hygiene

Droid review comments are an inter-agent repair queue. They should not notify humans unless explicitly requested.

* Do not @mention users, teams, bots, or organizations.
* Do not refer to the PR author by username.
* Avoid second-person instructions.
* Do not write `cc @username`, `asking @username`, or `Droid finished @username's task`.
* Avoid direct author-directed wording such as `you should`.
* Use neutral references such as `this PR`, `this diff`, `the changed code`, `the follow-up agent`, and `the next repair pass`.
* If the Factory wrapper adds a mention outside Droid's review body, do not repeat it.

## Repo priorities

Prioritize:

* product contract drift;
* static-output language drift;
* missing evidence package;
* output/schema/golden drift;
* workflow/secret/policy failures;
* Rust panic-family shortcuts;
* architecture seam violations;
* release or extension packaging regressions.

Do not prioritize style-only or formatting-only issues.
