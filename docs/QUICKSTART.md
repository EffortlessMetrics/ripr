# Quickstart

Inspect a change, read the testing gap, and decide which focused test to add.
ripr uses static analysis; it does not run mutants or write the test for you.

## Choose Your Path

[CLI](#cli-first-hour) · [VS Code](#vs-code-first-hour) ·
[GitHub CI](#ci-first-hour) · [Agent or reviewer](#agent-or-reviewer-first-hour)

## Installation

Install the published CLI with Cargo:

```bash
cargo install ripr
```

This requires Rust 1.95 or newer. Git must be available for the diff workflow.
The editor installation below normally does not require Cargo.

If Cargo refuses because ripr requires rustc 1.95, run `rustup update stable`.
Inside a repository whose `rust-toolchain.toml` pins an older Rust, Cargo uses
that pin; install from another directory or with `cargo +stable install ripr`.
Cargo may name an older ripr that supports your compiler; that release predates
this guide.

Rust 1.95 is needed to build ripr, not by the repository you analyze: ripr's
static analysis does not run your compiler, so a repository pinned to an older
toolchain can still be analyzed. Your own tests still run with that repository's toolchain.
`ripr doctor --profile source-build` checks what building ripr needs.

The latest GitHub release is [0.10.0](https://github.com/EffortlessMetrics/ripr/releases/tag/v0.10.0).
This guide describes **0.11 development**, including `--worktree`, bounded
`Start here:` output, and durable repair attempts. Those instructions are not a
claim that the published package has these features. For a released install,
use the [versioned README](https://github.com/EffortlessMetrics/ripr/blob/v0.10.0/README.md)
and that binary's help. The 0.10 CLI defaults to `origin/main`; use `--base REF`
with another existing branch or commit when needed.

To use this guide's development features, install the development build
(Rust 1.95 or newer):

```bash
cargo install --locked --git https://github.com/EffortlessMetrics/ripr-swarm ripr
```

From a `ripr-swarm` checkout, `cargo install --path crates/ripr` does the same.

For another installation method or a pinned server, see
[Server provisioning](SERVER_PROVISIONING.md).

## CLI First Hour

From a Rust repository on a branch with committed changes:

```bash
ripr check
```

No `ripr.toml` is required. The development CLI prints a summary and a bounded
`Start here:` section with a finding, a no-action result, or the limitation
preventing useful guidance. Read the changed behavior, related test, and
suggested next test. No findings does not mean the tests are complete.

When a finding is selected, copy its printed `ripr explain` command to inspect
the evidence, or its `ripr context` command for an agent handoff. Those commands
retain the root, diff selection, mode, and finding ID. Do not substitute an ID
from a documentation example.

To expand the output:

```bash
ripr check --format human-full
ripr check --format json
```

A finding is not automatically a runnable repair. The
[repair workflow](#agent-or-reviewer-first-hour) prepares a supported route
before any test edit.

### Choose the change

By default, `check` takes the changed lines from committed history; it reads
test files as they are on disk. In this development build, include staged and
unstaged edits to tracked files in the diff with:

```bash
ripr check --worktree
```

The development CLI tries `origin/HEAD`, then `origin/main`, `origin/master`,
`main`, and `master` for its base. To choose another comparison, replace `REF`
with an existing branch or commit:

```bash
ripr check --base REF
```

On an unchanged base branch, an empty diff is expected. To analyze a saved patch,
use `ripr check --diff PATH`, replacing `PATH` with your patch file. Details and
options are available in `ripr help check`.

### Explore the repository

For broader repository analysis and a suggested Rust repair, run:

```bash
ripr pilot --root .
```

This is broader than checking one change. It writes reports under
`target/ripr/pilot/` and either names a supported next action or explains why
no repair is ready. Follow its retry guidance when analysis is partial.
Non-Rust workspaces are routed to the appropriate diff-analysis or
unavailable-adapter guidance, not the Rust repair sequence.

## VS Code First Hour

Install [EffortlessMetrics.ripr](https://marketplace.visualstudio.com/items?itemName=EffortlessMetrics.ripr)
from VS Code Marketplace or [Open VSX](https://open-vsx.org/extension/EffortlessMetrics/ripr),
then open a Rust/Cargo workspace. The released extension normally resolves a
matching native server without `cargo install ripr`.

Open Problems and hover a ripr diagnostic to inspect the changed behavior,
related test, and missing assertion. Use the available focused-test actions to
open the test or copy a brief for an agent. You write the test in your editor;
ripr does not apply it.

The development extension can start a supported attempt with
`ripr: Start Current Repair`. A source-built extension needs a compatible
server; do not assume an unreleased matching server is downloadable. Follow
[Editor extension](EDITOR_EXTENSION.md) and [Server provisioning](SERVER_PROVISIONING.md)
for local builds and explicit server paths.

If nothing appears, run `ripr: Show Status`, then `ripr: Show Output` to inspect
setup or analysis errors. Save files before refreshing: the editor analyzes the
saved workspace, not unsaved buffers, by default.

See [the editor repair walkthrough](EDITOR_FIRST_RUN_TO_FIRST_RECEIPT.md) for
verification and receipt steps. For Remote-SSH, containers, or WSL, install the
extension on the workspace's host. Browser-only VS Code is not supported.

## CI First Hour

Generate the advisory GitHub workflow:

```bash
ripr init --ci github
```

Review the generated files before committing them. On a PR, read the job summary
first, then open the linked artifacts for the evidence and suggested test.
The generated workflow is non-blocking by default; a policy gate is a separate,
explicit adoption decision.

The [copyable CI recipe](CI.md#copyable-ripr-advisory-workflow) is the downstream
usage section of the repository's CI guide. See [PR review guidance](PR_REVIEW_GUIDANCE.md)
for reading results and [Blocking readiness](BLOCKING_READINESS.md) before
choosing a gate.

## Agent Or Reviewer First Hour

This is the **development Rust repair path**. Start from a current supported
route, not an arbitrary finding ID:

```bash
ripr pilot --root .
```

Before starting a repair, make sure `target/` is in a committed `.gitignore`
(see below). When pilot recommends a repair, run the exact
`ripr agent repair ... --phase before` command it prints. That command carries pilot's seam ID, which names a code
location across the repository. The `probe:...` IDs printed by `ripr check`
name findings in one diff and are not accepted here.

Before starting a Rust repair, ensure Git ignores Cargo's entire `target/`
directory, for example with `/target/` in `.gitignore`. Effective local or global
Git exclude rules also work. The before phase checks this even before a build
exists and stops with recovery guidance if it is missing; ignoring only
`target/debug/` does not cover Cargo's root-level build files. Set the rule before
the attempt so running the focused test cannot turn build output into an edit.

The before phase prepares the packet and prints an `--attempt` continuation
command. Read the allowed test files, proposed assertion, verification command,
and stop conditions. Make the focused test edit yourself or delegate that packet
to an external coding agent, and change only test files. Run the test yourself:
ripr does not run it. The attempt refuses any new file outside the allowed test
files, whether Git ignores it or not, except build output under a `target/`
that was already gitignored when the before phase ran; a `.gitignore` change
made mid-attempt is itself refused. Redirect ripr output under `target/ripr/` or
outside the repository.

Then run the exact `--attempt ... --phase after` command printed before the edit.
It records the after snapshot and a receipt of static evidence movement.
Keep the test execution result separately: a static improvement is not proof
that a test ran or that a mutant was caught.

No complete route, partial analysis, stale evidence, or an unsupported target
means stop and inspect the guidance, not guess a repair command.

To resume an interrupted attempt:

```bash
ripr agent status --root .
```

Use the reported continuation or recovery step. The full phase and identity
reference is [Repair attempt identity](REPAIR_ATTEMPT.md).
A trust-bound Python repair adds a third `verify` phase, run with its own
authorization flags; follow [the Python sequence](REPAIR_ATTEMPT.md#governed-python-sequence),
not the Rust sequence above.

For lower-level control, see [Agent workflows](AGENT_WORKFLOWS.md) and
[the LLM operator guide](LLM_OPERATOR_GUIDE.md). No LLM provider is called by
these commands.

### Prepare a PR summary

After generating the needed evidence, use `ripr first-pr` to compose the
PR-facing packet. It does not run analysis or repair the code. See
[First successful PR workflow](FIRST_PR_WORKFLOW.md) for the inputs and
[the command guide](COMMAND_HIERARCHY.md) for its options.

## Troubleshooting

| Symptom | Next step |
| --- | --- |
| Cargo installation fails. | Check the first Cargo error and `rustc --version` (Rust 1.95 or newer). Fix the reported build or download problem before retrying. |
| ripr is installed, but repository setup fails. | Run `ripr doctor` and inspect its individual capability results. |
| `check` sees no change after an edit. | In a development build, use `--worktree` for staged and unstaged edits. Otherwise inspect a committed change using your installed version's supported options. |
| The wrong base is selected. | Use `--base REF` with an existing reference in this repository. |
| Configuration is rejected. | Run `ripr config validate` and fix the named setting. Configuration is optional, but an invalid file is not ignored. |
| Editor diagnostics are missing or stale. | Check `ripr: Show Status`, save the file, and use the extension's refresh action. |
| The editor cannot start its server. | Check [Server provisioning](SERVER_PROVISIONING.md), especially the version and remote host. |
| An attempt cannot continue. | Run `ripr agent status --root .` and follow [repair recovery](REPAIR_ATTEMPT.md). |
| Analysis is partial or limited. | Read the limitation and suggested retry. Missing evidence is not a successful empty result. |

## Known Limits

The Rust analysis and bounded repair workflows are `usable alpha`; a valid
route is required, and ordinary real-repository route yield and success remain
unestablished. Python analysis is `preview`, with selected pytest/unittest
repair routes at `usable alpha`. Only complete weak-evidence routes get repair
cards; zero cards is normal for unsupported or non-repairable findings.

TypeScript and JavaScript are opt-in previews. Diff analysis recognizes:

```text
.ts  .tsx  .mts  .cts  .js  .jsx  .mjs  .cjs
```

In the development build, these extensions also share the language
classification used by repair eligibility, related-test packet labels, and
targeted reruns. Recognizing a file does not make its findings repairable:
a complete supported route and sufficient evidence are still required. Read
the reported limitation or next action; an absent repair packet is not evidence
that the change is safe.

Perl needs a `lang-perl` build and the unpublished `perl-ripr-facts` exporter,
so it is not usable from a released build/exporter combination.

The CLI does not generate tests, apply code edits, run mutants, or turn an empty
result into proof of correctness. Generated CI is advisory by default.
See [Support tiers](status/SUPPORT_TIERS.md), [Static limits](STATIC_LIMITS.md),
and [Language adapters](LANGUAGE_ADAPTER_PREVIEW.md) for supported cases.

## Next Docs

[Command guide](COMMAND_HIERARCHY.md) · [Configuration](CONFIGURATION.md) ·
[Targeted tests](TARGETED_TEST_WORKFLOW.md) · [Output formats](OUTPUT_SCHEMA.md) ·
[Terminology](TERMINOLOGY.md)
