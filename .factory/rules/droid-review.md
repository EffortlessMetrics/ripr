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

## Finding requirement

Each finding should help another coding agent fix the PR.

Use:

```text
[P0|P1|P2] title

Failure mode:
Why here:
Fix direction:
Validation:
```

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
