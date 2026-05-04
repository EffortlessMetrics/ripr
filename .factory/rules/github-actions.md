# GitHub Actions Rules

## Secret handling

- Do not expose repository or org secrets to untrusted fork PR code.
- Prefer `pull_request` with same-repo guards for secrets-backed PR workflows.
- Avoid `pull_request_target` unless the workflow is specifically designed to avoid checking out or running untrusted code.

## Permissions

- Set `permissions:` explicitly.
- Use the minimum write scope needed.
- Avoid broad write permissions for workflows triggered by comments or external actor input.

## Action refs

- Avoid mutable refs such as `@main` in workflows with secrets or write permissions.
- Prefer versioned tags for quick setup and full commit SHAs for locked-down rollout.

## Workflow budget

- Every workflow file must be listed in `policy/workflow_allowlist.txt`.
- If adding shell `run:` blocks, update the allowed non-empty run-line budget.
- If a workflow uses only reusable actions and no shell blocks, budget can be `0`.

## Droid workflows

- Automatic Droid review should run on same-repo PRs.
- Manual `@droid` triggers should be restricted to trusted actors.
- Do not enable `show_full_output` in normal operation.
