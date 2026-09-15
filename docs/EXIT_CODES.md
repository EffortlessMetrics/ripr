# Exit Codes

ripr uses a simple two-value exit code contract:

| Code | Meaning |
|------|---------|
| `0`  | Success: analysis completed and no blocking findings were found. |
| `2`  | Failure: analysis failed, or blocking findings were found, or a user error occurred. |

## Why code 2?

ripr uses exit code `2` for all failure conditions (not `1`) to distinguish
it from shell-level errors (which typically use `1`). This makes it easier
to distinguish "ripr ran and found issues" from "the shell could not run
ripr" in CI pipelines.

## When you see exit code 2

- **Blocking findings**: `ripr check` found `exposed` or `weakly_exposed`
  findings that the gate considers blocking.
- **Gate failure**: `ripr gate evaluate` blocked the PR.
- **Analysis error**: the diff could not be parsed, the base ref could not
  be resolved, or the workspace root could not be determined.
- **User error**: unknown command, missing required argument, or invalid
  config.
- **Internal error**: a panic occurred (with a `ripr: internal error` message).

## `ripr doctor` exit codes

`ripr doctor` uses the same contract: `0` when all checks pass, `2` when
any check fails (including missing language runtimes that are enabled in
the effective configuration).

## CI integration

In generated GitHub Actions workflows, ripr preserves the exit code:

```yaml
ripr check --root . --mode draft --format json > check.json || check_status=$?
```

The `|| check_status=$?` pattern captures the exit code without failing
the step, so downstream review-comments and gate steps can consume the
partial output even when the check finds issues.
