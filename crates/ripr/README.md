# ripr

**Write tests that notice changed behavior.**

ripr reads your diff and related tests to find potential gaps in what they
check. It explains the evidence and suggests where a focused test could help.
Use the finding during review or hand it to a coding agent.

The analysis is static and advisory. ripr does not run mutants or prove test
adequacy; real mutation testing remains the execution-backed confirmation step.

## Example

```diff
- if amount > discount_threshold {
+ if amount >= discount_threshold {
```

Tests below and above the threshold can pass in both versions. The equality
case, with an assertion on the result, distinguishes the changed behavior.
ripr looks for missing cases and weak assertions like this.

See the [product README](https://github.com/EffortlessMetrics/ripr/blob/main/README.md)
for an output example and the [static exposure model](https://github.com/EffortlessMetrics/ripr/blob/main/docs/STATIC_EXPOSURE_MODEL.md)
for the analysis behind a finding.

## Install

```bash
cargo install ripr
```

Cargo installation requires Rust 1.95 or newer. Git is required for the diff
workflow. The package installs the `ripr` binary; you do not need a checkout of
ripr to analyze your own repository.

This source package is **0.11.0 development, pending publication**. The latest
GitHub release is [0.10.0](https://github.com/EffortlessMetrics/ripr/releases/tag/v0.10.0).
`cargo install ripr` installs the published package, not this checkout.
Use the [versioned release instructions](https://github.com/EffortlessMetrics/ripr/blob/v0.10.0/README.md)
for that release, or the [source installation guide](https://github.com/EffortlessMetrics/ripr-swarm/blob/main/docs/QUICKSTART.md#installation)
for development features.

## Quick Start

Run this from a Rust repository on a branch with committed changes. The 0.10
release uses `origin/main` as its default base; use `--base REF` to choose another
existing branch or commit:

```bash
ripr check
```

Read the changed behavior, related tests, and suggested next test. Configuration
is optional. Use `ripr doctor` when setup is not working, rather than as a
required step before each check.

The [CLI quickstart](https://github.com/EffortlessMetrics/ripr-swarm/blob/main/docs/QUICKSTART.md#cli-first-hour)
covers diff selection, development output, and finding inspection. Use the
`explain` or `context` command printed by a development build to retain the
finding's root, diff, mode, and ID instead of copying an ID from an example.

## Workflows

`check` inspects one change. `pilot` explores the repository more broadly.
In a development build, `agent repair` prepares and records a supported
before/edit/after attempt. The human or external agent writes the test.
The receipt records static evidence movement, separately from any test execution.

Probe IDs from `check` are not the seam IDs accepted by `agent repair`. Follow
[Repair a gap](https://github.com/EffortlessMetrics/ripr-swarm/blob/main/docs/QUICKSTART.md#agent-or-reviewer-first-hour)
from selection through completion; do not skip the preparation step.

[VS Code](https://github.com/EffortlessMetrics/ripr/blob/main/docs/EDITOR_EXTENSION.md)
provides saved-workspace feedback. [Advisory CI](https://github.com/EffortlessMetrics/ripr/blob/main/docs/CI.md#copyable-ripr-advisory-workflow)
brings findings into PR review. The [command guide](https://github.com/EffortlessMetrics/ripr-swarm/blob/main/docs/COMMAND_HIERARCHY.md)
also covers PR evidence composition, read-only MCP status, and advanced commands.

## Output Formats

Human output is for local inspection. JSON is versioned for integrations;
consumers should check `schema_version` before reading the rest of the result.
Use the [output reference](https://github.com/EffortlessMetrics/ripr/blob/main/docs/OUTPUT_SCHEMA.md)
for JSON, GitHub annotations, SARIF, and report formats.

## Support and limitations

Rust analysis and bounded repair are `usable alpha`. A valid repair route is
required; ordinary real-repository route yield and success remain unestablished.
Python analysis is `preview`, with selected pytest/unittest repair routes at
`usable alpha`. TypeScript and JavaScript are opt-in previews. Perl requires a
`lang-perl` build and the unpublished `perl-ripr-facts` exporter; it is not usable
from a released build/exporter combination.

No findings does not mean the tests are complete. Opaque fixtures, generated
code, and unsupported flows can limit static analysis. See
[Support tiers](https://github.com/EffortlessMetrics/ripr/blob/main/docs/status/SUPPORT_TIERS.md),
[static limits](https://github.com/EffortlessMetrics/ripr/blob/main/docs/STATIC_LIMITS.md),
and [terminology](https://github.com/EffortlessMetrics/ripr/blob/main/docs/TERMINOLOGY.md).

## Development

From the development repository root:

```bash
cargo install --path crates/ripr
cargo xtask precommit
```

`precommit` is focused local validation, not the complete hosted matrix.
Use `cargo xtask ci-full` for the complete local pass, or rerun a specific failed
gate. See [Contributing](https://github.com/EffortlessMetrics/ripr-swarm/blob/main/CONTRIBUTING.md)
and [Agent instructions](https://github.com/EffortlessMetrics/ripr-swarm/blob/main/AGENTS.md).

## License

MIT OR Apache-2.0, at your option.
