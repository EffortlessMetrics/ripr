# Quickstart

Use this guide to get useful RIPR feedback in the first hour without learning
the full report topology. Pick one path, inspect a Rust finding or a clear
no-action state, and keep the evidence.

The [saved-diff CLI path](#choose-the-change-to-analyze) works with the published
`0.10.0` release as well as current source builds. Newer navigation and repair
commands are identified below; check your installed help before using them.

RIPR finds changed Rust code where the nearby tests may not actually catch
the changed behavior. The static, draft-time question it answers is:

```text
For the behavior changed in this diff, do the current tests include an
assertion or check that would catch the changed behavior?
```

It does not edit source, generate tests, run mutation testing, or prove test
adequacy. On builds with the current repair command, the first-hour loop is:

```text
inspect one change
-> select one named gap
-> start the repair transaction
-> write one focused test outside RIPR
-> finish the transaction
-> keep the advisory receipt
```

RIPR calls these locations "seams" in JSON, specs, and reports. First-hour docs
use plain language first; [Terminology](TERMINOLOGY.md) bridges to the internal
model when you need it.

The [public command hierarchy](COMMAND_HIERARCHY.md) keeps the first-hour roles
distinct: `check` is ordinary first value, `pilot` is guided repo adoption,
`agent repair` is the repair transaction, and `first-pr` composes PR evidence.

Each repair's before phase prints the exact `--attempt` command needed after
the test edit. Retain it: the repair-attempt ID identifies the prepared
transaction and differs from the seam ID used to select the gap.
[Repair attempt identity](REPAIR_ATTEMPT.md) documents continuation and recovery.

The Rust gap-repair loop is `usable alpha`. Its package, editor, bounded packet,
and before/after transaction paths are proved when RIPR emits a complete route;
the governed real-repository corpus does not yet establish how often ordinary
changes yield such a route or complete successfully. Treat a no-action or
limited result as evidence, not as a failed product promise or a clean bill of
health.

## Choose Your Path

Most adopters should choose one of these first-hour paths:

| Path | Use when | Start with | First success |
| --- | --- | --- | --- |
| CLI first | You want feedback on tracked local edits. | Create the patch below, then `ripr check --diff ripr-local.diff`. | A finding to investigate or an honest empty/limited result. |
| PR first | You want reviewers to see advisory evidence in GitHub. | `ripr init --ci github` | Non-blocking summary, repair card, artifact packet. |
| Editor or agent first | You are repairing while coding, or handing work to an LLM. | VS Code `ripr: Show Status`, or the [version-aware agent path](#agent-or-reviewer-first-hour). | Inspect the finding and related test; use a bounded repair route when supported. |

For TypeScript, JavaScript, or broader Python static-fact evaluation, first read
[Language adapter preview workflow](LANGUAGE_ADAPTER_PREVIEW.md). Preview
language evidence is syntax-first, visibly preview/advisory, and not a default
gate input. The scoped Python repair-routing loop is `usable alpha` for selected
pytest/unittest repair cards, packets, verify commands, and receipts; it remains
advisory and does not run imports, execute tests, or generate tests by default.

`ripr.toml` is optional. `ripr init` materializes repo-local policy when a team
wants to review, version, and tune it. It is not activation, and it is not
required for first value.

## VS Code First Hour

Use this path when you want saved-workspace feedback while writing or reviewing
Rust.

1. Install `EffortlessMetrics.ripr` from VS Code Marketplace or Open VSX.
2. Open a Rust/Cargo workspace.
3. Check the `ripr` status bar item or run `ripr: Show Status`.
4. Open the Problems panel, hover a RIPR diagnostic, and inspect the gap.
5. Use `ripr: Start Current Repair` or the focused-test actions to copy the
   repair packet, open the related test, and copy the verify command.

The current editor command is the contextual shell for the same bounded
before/edit/after idea. For a CLI whose `ripr agent repair --help` lists
before/after phases and `--attempt`, the equivalent is below. On `0.10.0`,
use the [manual test-and-recheck loop](#complete-one-bounded-repair).

```bash
ripr agent repair --root . --seam-id <id> --phase before
# edit one focused test outside RIPR
ripr agent repair --root . --attempt <repair-attempt-id> --phase after
```

Normal editor install should not require `cargo install ripr`. The extension
resolves the server from `ripr.server.path`, bundled or cached assets, verified
GitHub Release download, or PATH.

If no diagnostics appear, start with the status path:

```text
ripr: Show Status
ripr: Show Output
ripr: Restart Server
```

The editor analyzes the saved workspace. Unsaved-buffer overlays are not enabled
by default. Save the file or refresh analysis before trusting a stale diagnostic.

Deep links: [Editor evidence workflow](EDITOR_EVIDENCE_WORKFLOW.md),
[Editor extension](EDITOR_EXTENSION.md), [Server provisioning](SERVER_PROVISIONING.md).

## CI First Hour

Use this path when you want PR-visible advisory evidence without asking every
reviewer to download raw artifacts.

Generate the GitHub workflow:

```bash
ripr init --ci github
```

Or copy the workflow from [CI strategy](CI.md) when adopting from the GitHub UI.

The generated workflow is advisory by default. On a PR, read the job summary
first. It should show the first-run status, top repairable gap or no-action
state, repair route, verify command, artifact links, and gate-authority
boundary. The uploaded packet keeps the detailed pilot, workflow, agent,
report, and review artifacts.

Do not make generated CI blocking until the repository has reviewed its first
advisory baseline and explicitly opted into a policy gate.

Deep links: [CI strategy](CI.md), [PR review guidance](PR_REVIEW_GUIDANCE.md),
[Blocking readiness](BLOCKING_READINESS.md).

## CLI First Hour

Use this path when you want the reproducible local proof loop.

### Install and identify the CLI

For installation with Cargo, use Rust 1.95 or newer and Cargo. Git must be on
`PATH` for the Git-based checks below. Run the analysis commands from your
project's repository root, with at least one commit.

```bash
cargo install ripr --locked
ripr --version
```

Keep `--locked` to use the published dependency lockfile. Without it, Cargo
can select incompatible transitive dependency updates before RIPR first runs.

For prebuilt installation, choose the `ripr-server` archive for your operating
system and CPU from [GitHub Releases](https://github.com/EffortlessMetrics/ripr/releases),
verify it against the published checksum, and extract it. Add the directory
containing `ripr` (or `ripr.exe` on Windows) to `PATH`, then run `ripr --version`.
The archive contains the RIPR CLI, with the commands supported by that release.
A Rust compiler is not needed to install it; the toolchain needed to execute
your project's tests is a separate requirement.

When developing RIPR from this repository's root, use:

```bash
cargo install --path crates/ripr --locked
```

These docs follow the current source tree. An installed release may have a
different set of commands; check `ripr --version`, `ripr check --help`, and the
[changelog](../CHANGELOG.md) before following a source-only workflow. In
particular, `0.10.0` has no `--worktree`, `--write-artifact`, `human-full`, or
`agent repair`; use the saved-diff and manual test-and-recheck path below.

### Choose the change to analyze

For edits you have not committed yet:

```bash
git diff --no-ext-diff --submodule=short --output=ripr-local.diff HEAD -- && ripr check --diff ripr-local.diff
```

Git writes a patch from `HEAD` to the tracked working tree, including staged
and unstaged edits; `&&` runs RIPR only if that succeeds. This works without an
`origin` remote and with `0.10.0`'s existing `--diff` option. Regenerate the patch
after changing source or tests. New untracked files are excluded; add the
intended files to Git before expecting them in this check.

If the installed `ripr check --help` lists `--worktree`, the shorter equivalent
is:

```bash
ripr check --base HEAD --worktree
```

The scope reference below describes current source builds. For an older build
without a listed option, use the saved-diff recipe to select the input
explicitly instead of assuming the newer option exists.

| What you want to inspect | Command | Scope |
| --- | --- | --- |
| Tracked edits since the current commit | `ripr check --base HEAD --worktree` | Staged and unstaged tracked changes relative to `HEAD`. |
| A committed branch change | `ripr check --base origin/main` | The merge base of `origin/main` and `HEAD` through `HEAD`, equivalent to the scope of `git diff origin/main...HEAD`. Local edits are excluded. |
| The current tracked tree against a chosen base | `ripr check --base origin/main --worktree` | The base commit directly against the working tree, including committed, staged, and unstaged differences. This is not a merge-base comparison. |
| A saved unified diff | `ripr check --diff change.patch` | The patch file path resolves from the current working directory. Source paths inside the patch resolve against the selected root (`--root`). Keep the patch and project sources aligned. |

Replace `origin/main` with the target branch or commit that actually exists in
your clone. If it is missing, fetch the intended branch or pass another valid
`--base`. An explicitly selected base never falls back silently.

Without `--base` or `--diff`, RIPR resolves the local `origin/HEAD` reference,
then tries `origin/main`, `origin/master`, `main`, and `master`. This selects a
base; it does not opt into local edits. `--worktree` is still needed for those.
`--worktree` and `--diff` cannot be combined.

For reproducible review evidence, choose a base explicitly and record the
command with the output. Use `--root <path>` when selecting another project,
and quote paths that contain spaces.

The table covers diff-scoped output such as the default human view and
`--format json`. Repository-wide formats such as `repo-exposure-json` analyze
the repository regardless of `--base` or `--diff`; they are not a shortcut to
more detail about the same diff. Use `--format human-full` when supported, or
inspect the scoped JSON output.

### Read the result before starting a repair

On current source builds, the bounded human output begins with one `Start here:`
state. It either names
the top repair-ready gap, reports an honest no-action state, or names the
limitation that prevents current guidance. Use `--format human-full` when
supported for complete human evidence, or `--json` for machine data. For
example, both current source and `0.10.0` accept:

```bash
ripr check --diff ripr-local.diff --json
```

| Result | What to do next |
| --- | --- |
| A gap with a complete repair route | Read the changed behavior, related test, and missing assertion; then use the supplied repair guidance. |
| A finding with weak or unknown evidence | Inspect the full evidence and static limits before choosing a test. A finding does not guarantee an executable repair route. |
| No findings, no scope, or omitted local edits | Check the selected base and regenerate the local patch. Use `--base HEAD --worktree` only when supported; staging alone does not include local edits in a committed-only check. |
| A partial or limited result | Follow the reported limitation or retry action. Do not interpret incomplete analysis as a clean result. |

`ripr check` is advisory: a successful command can return findings, warnings,
or an empty result with exit code `0`. Exit code `2` indicates a command error;
a separate `ripr gate evaluate` command can also reject a policy decision.
See [Exit codes](EXIT_CODES.md) and [Finding triage](how-to/triage-a-finding.md).

If your installed `ripr check --help` lists `--write-artifact`, you can keep
worktree findings available for `explain` and `context` by writing a reusable
analysis artifact:

```bash
ripr check --base HEAD --worktree --write-artifact ripr-check.json
```

Copy the printed navigation commands so the original input and finding ID
stay together. This artifact is distinct from `--format json` output; a
redirected JSON report is not a substitute for `--write-artifact`. Regenerate
the artifact when its inputs change instead of reusing stale evidence.
On older builds without this option, inspect the saved-diff findings with
`ripr check --diff ripr-local.diff --json`.

### Complete one bounded repair

The basic manual loop works without the newer repair subcommand: inspect the
finding, add a focused test, run the project's test command, regenerate the
patch, and repeat the saved-diff check. Record the test result separately from
RIPR's static evidence.

Current source builds additionally offer the ordinary repair transaction below.
Use it only if `ripr agent repair --help` lists before/after phases and
`--attempt`; `0.10.0` does not have this subcommand. For a repair-ready gap,
start with:

```bash
ripr agent repair --root . --seam-id <id> --phase before
```

Make one focused test edit outside RIPR, then finish:

```bash
ripr agent repair --root . --attempt <repair-attempt-id> --phase after
```

The before phase writes the pre-edit snapshot and repair packet. The after phase
writes the post-edit snapshot, persists static verification JSON, and emits a
receipt. RIPR does not author or apply the test edit.

Run the focused test command supplied by the repair guidance separately. The
receipt records static evidence movement; it does not show that a test or a
mutant executed.

For guided repository adoption and materialized pilot reports, use:

```bash
ripr pilot --root .
```

`pilot` is broader than the ordinary one-change check. If it reports a partial
result, use the retry command it prints rather than guessing at cache or timeout
settings.

For explicit low-level before/after comparison, the underlying commands remain
available:

```bash
ripr check --root . --mode ready --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json
ripr outcome \
  --before target/ripr/pilot/repo-exposure.json \
  --after target/ripr/pilot/after.repo-exposure.json
```

Those commands are useful for control and debugging; they are not required
plumbing for the ordinary two-phase repair path.

To compose PR-facing evidence from existing artifacts, use:

```bash
ripr first-pr --root . --base origin/main --head HEAD
```

It writes `target/ripr/reports/start-here.{json,md}` and does not add analyzer
truth or repair a gap. Inside this repo, `cargo xtask first-pr` is a
compatibility wrapper over the same public command.

Read the front door with the same vocabulary across CLI, editor, and PR
surfaces:

- `start here`: open `target/ripr/reports/start-here.md` first when it exists.
- `safe next action`: repair one named gap, regenerate missing evidence, or
  stop on no-action.
- `missing artifact`, `stale evidence`, `wrong root`, and `malformed artifact`:
  fail closed before repair work.
- `preview-limited evidence`: syntax-first and advisory, with static limits
  before repair language.
- `verify command`, `receipt command`, and `receipt path`: the static movement
  proof rail, not runtime adequacy or gate approval.

When a surface boundary is unclear, use the ownership table in
[First successful PR workflow](FIRST_PR_WORKFLOW.md#surface-ownership). It
names which surface owns start-here, generated CI, editor handoff, agent
packets, badges, PR evidence, and gate authority.

## Agent Or Reviewer First Hour

Use this path when a human or external coding agent needs a deterministic work
packet for one focused test.

First inspect the installed `ripr agent --help`. The before/after repair
transaction below requires a build whose `ripr agent repair --help` lists
those phases and `--attempt`. On `0.10.0`, start with the
[manual test-and-recheck loop](#complete-one-bounded-repair); its existing
lower-level `agent start` workflow is also available below.

When the repair command is supported and a current finding supplies a seam ID,
use the primary repair command:

```bash
ripr agent repair --root . --seam-id <seam_id> --phase before
# edit one focused test outside RIPR
ripr agent repair --root . --attempt <repair-attempt-id> --phase after
```

Ask RIPR what local artifacts already exist when resuming or diagnosing:

```bash
ripr agent status --root .
```

For explicit low-level control, `ripr agent start`, `brief`, `packet`, `verify`,
`receipt`, and `review-summary` remain available. For example:

```bash
ripr agent start --root . --seam-id <seam_id> --out target/ripr/workflow
```

The generated `target/ripr/workflow/commands.md` lists the explicit snapshot,
packet, verification, and receipt steps. This workflow is available in
`0.10.0`; it does not provide the newer repair transaction's durable
`--attempt` identity.

The repair, status, and workflow commands read or write artifacts. They do not
edit source files, generate tests, call an LLM API, run mutation testing,
refresh LSP state, or enable CI blocking.

See [LLM operator guide](LLM_OPERATOR_GUIDE.md).

## Troubleshooting

| Symptom | First check |
| --- | --- |
| `ripr` is not found after installation. | Check that the Cargo binary directory or extracted archive directory is on `PATH`, open a new terminal if needed, then run `ripr --version`. |
| `origin/main` cannot be resolved. | Fetch the intended target branch, pass a valid local `--base`, or use the [saved-diff recipe](#choose-the-change-to-analyze) for tracked local edits without a remote. |
| A local change produces no findings. | Check `git status --short` and the [selected scope](#choose-the-change-to-analyze). Committed-only checks exclude staged and unstaged changes; worktree checks exclude untracked files. An empty result does not establish test strength. |
| A JSON command exits with `2`. | Read stderr and the [exit-code guide](EXIT_CODES.md). The redirected file may be empty or incomplete; do not treat it as a successful analysis report. |
| VS Code shows no RIPR state, or shows no focused test gap. | Run `ripr: Show Status`, then `ripr: Show Output`. Confirm a Rust/Cargo workspace is open and saved. |
| VS Code cannot start the server. | Check [Server provisioning](SERVER_PROVISIONING.md) for configured path, bundled or cached assets, GitHub Release download, and PATH fallback. |
| Diagnostics look stale. | Save the workspace file or run `Refresh Analysis - Saved Workspace Check`. |
| CI has no top recommendation. | Open the advisory job summary, then inspect the uploaded report packet. |
| Agent status says artifacts are missing. | Run the `next_command` printed by `ripr agent status`. |
| Local CLI behavior is surprising. | Run `ripr doctor` and inspect config precedence in [Configuration](CONFIGURATION.md). |

## Known Limits

RIPR reports static exposure evidence. It should not be read as runtime proof.

It does not:

- run mutants;
- report `killed` or `survived` outside supplied runtime calibration reports;
- prove test adequacy;
- generate tests;
- edit source files;
- replace coverage or execution-backed mutation testing;
- analyze unsaved editor buffers by default;
- make generated CI blocking by default.

Static classifications stay conservative: `exposed`, `weakly_exposed`,
`reachable_unrevealed`, `no_static_path`, `infection_unknown`,
`propagation_unknown`, and `static_unknown`.

When runtime mutation data already exists, import it as advisory calibration
data through [runtime calibration](TARGETED_TEST_WORKFLOW.md#runtime-calibration).
Runtime vocabulary belongs in that calibration report, not in ordinary static
RIPR findings.

## Next Docs

- [Public command hierarchy](COMMAND_HIERARCHY.md) for the stable task roles.
- [Terminology](TERMINOLOGY.md) for the bridge between plain wording and the
  internal model (seam, discriminator, grip, canonical gap, etc.).
- [First successful PR workflow](FIRST_PR_WORKFLOW.md) for the one-PR path from
  a repairable Rust gap to a focused proof and receipt.
- [Targeted test workflow](TARGETED_TEST_WORKFLOW.md) for the deeper
  before/after evidence and optional calibration loop.
- [Editor extension](EDITOR_EXTENSION.md) for VS Code install, commands, and
  saved-workspace refresh behavior.
- [CI strategy](CI.md) for the generated advisory workflow and artifact packet.
- [LLM operator guide](LLM_OPERATOR_GUIDE.md) for the source-edit-free agent
  loop.
- [Configuration](CONFIGURATION.md) for `ripr.toml`, modes, severities, and
  editor settings.
- [Language adapter preview workflow](LANGUAGE_ADAPTER_PREVIEW.md) for opt-in
  TypeScript, JavaScript, and Python evidence.
- [Output schema](OUTPUT_SCHEMA.md) for JSON contracts.
- [Exit codes](EXIT_CODES.md) for command completion, errors, and policy decisions.
