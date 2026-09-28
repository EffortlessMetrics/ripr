# Validation Commands

Use the smallest validation set that proves the changed behavior, then inspect
the required hosted checks before merge. Do not equate a command inventory with
a requirement to run every command locally.

## Environment and shell

Identify the repository root, worktree, HEAD, actual shell, host/target platform
and toolchain before composing commands. Read `rust-toolchain.toml` rather than
copying a version from a retained session. Detect installed tools instead of
assuming Unix utilities or PowerShell cmdlets exist.

The shell determines grammar; the OS does not. Use PowerShell syntax when the
active shell is PowerShell, including PowerShell on Linux. Use the actual Bash
or POSIX grammar in Bash/WSL/Linux/macOS. A native Windows qualification still
needs a native Windows execution environment; WSL is not equivalent evidence.

Cargo writes normal progress to stderr. Neither stderr output nor a wrapper's
red icon establishes native command failure. Conversely, a success-looking test
line does not override a nonzero exit status. Read both the native status and
the terminal report; retain conflicting or missing evidence as an instrument
failure until reconciled.

PowerShell example, without a pipeline that obscures the native status:

```powershell
& cargo test -p xtask agent_skills
$commandExit = $LASTEXITCODE
if ($null -eq $commandExit) { throw 'Native command exit status missing' }
if ($commandExit -ne 0) { throw "Cargo exited $commandExit" }
```

In Bash, capture `$?` immediately after the command. If a pipeline is genuinely
necessary, capture its native command status before another command overwrites
it; `echo done` is not a test result. Do not discard a failed step because a
later command returned zero.

## Owned background work

Bind a background command to its task/session handle, candidate identity and
log destination. An unchanged report, missing final binary, process-name filter
or quiet interval does not show that the driver died or that children are
orphans. Check the original task's terminal result before retrying. Do not
label a process harmless without ownership, parent/driver and cleanup evidence.

Serialize Cargo operations that share a candidate worktree, target lock or
memory bottleneck. Do not start another broad run against a tree while its
verification is still consuming mutable files. Use separate workers/worktrees
for independent claims when available. Do not kill unrelated Cargo processes
or invent worker access.

A waiting command blocks its transition, not the parent goal. Advance another
ready claim or do independent read-only work. If every useful transition is
waiting, report it as in flight instead of repeatedly polling unchanged state.

## Local and hosted proof

Local proof is for fast discrimination and candidate shaping:

```bash
cargo xtask check-fast
cargo xtask precommit
```

Add the focused test, narrow compile and changed-surface checks required by the
claim. For `check-fast`, independently verify the diff selector and base before
using it as evidence. An empty or failed selector is not proof that nothing
needs testing.

PR CI supplies the required merge matrix. Publishing a coherent candidate with
`REVIEW_INCOMPLETE` allows hosted proof and review to run; it is not permission
to merge without them. When local execution is unavailable, record that limit,
publish the candidate for the available hosted route, and inspect its exact-head
results. Do not claim that a check ran locally or use unavailable local proof as
a permanent reason to keep the branch unpublished.

Read the actual job steps and reports: selected/executed subjects, skips,
features, target, runner, command status and candidate identity. A green
aggregate or zero unresolved review threads does not replace semantic review.
An unavailable review provider is missing evidence for that provider, not an
obligation to wait forever if an adequate permitted review path is available.

Release-readiness, package/install rehearsal, immutable-candidate qualification,
source integration and publication are separate fixed-object proof planes. A
pass in one plane must not become another plane's pass in a summary. Historical
local receipts remain evidence for their original object, not current release
authority.

## Rust

Broad reference commands, not a per-edit checklist:

```bash
cargo check --workspace --all-targets
cargo test --workspace
```

Use the checked test-runner contract for required CI and release qualification.
Do not assert Cargo test/nextest equivalence without comparing subject inventory,
features, doctests, ignored tests, configuration, retries and execution behavior.

## Workflow policy

```bash
cargo xtask check-workflows
```

Run this for changes to `.github/workflows/**`, `policy/workflow_allowlist.txt`,
release workflows or CI/security workflows.

## Droid workflow policy

```bash
cargo xtask check-droid-review-config
```

Run this with `check-workflows` for changes to:

- `.github/workflows/droid-review.yml`, `droid.yml`, or `droid-security-scan.yml`;
- `docs/agent-context/review-invariants.md` or `droid-smoke-tests.md`;
- `.factory/skills/review-guidelines/SKILL.md` or `.factory/rules/droid-review.md`.

## Security-sensitive review

For secrets, workflows, dependency policy, release scripts or command execution,
inspect permissions, event triggers, fork behavior, artifact/log exposure and
whether secrets can be printed or written. Diagnose branch protection and
rulesets read-only; ordinary PR repair never authorizes weakening them.
