# ripr

`ripr` is a static RIPR mutation-exposure analyzer for Rust workspaces.

It answers a draft-time testing question:

```text
For the behavior changed in this diff, do the current tests appear to contain
a discriminator that would notice if that behavior were wrong?
```

`ripr` is alpha software. The current release is a syntax-first scanner that is
useful for early feedback, not a proof system.

This is the product repository for `ripr`. The `0.2.x` line keeps the analyzer
scope narrow while adding extension-managed server provisioning.

## Mission

`ripr` helps Rust developers and coding agents write tests that actually notice
changed behavior.

It performs static RIPR exposure analysis over changed Rust code, creates
mutation-shaped probes, and reports whether existing tests appear to contain the
discriminators needed to expose those changes.

It is a fast draft-mode companion to real mutation testing: static guidance
while the pull request is moving, real mutation confirmation when the change is
ready.

## Vision

The vision for `ripr` is to become the live test-adequacy layer between coverage
and mutation testing for Rust/Cargo workspaces.

Coverage tells developers what executed. Mutation testing tells developers what
survived. `ripr` tells developers, reviewers, and agents what changed behavior
appears to lack a meaningful oracle before the expensive mutation run begins.

The end state is an LSP-native sidecar that watches changed code, identifies
missing discriminators, explains the evidence path, emits agent-ready test
intent, and calibrates itself against real mutation outcomes.

## Category

`ripr` defines its category as:

```text
Static Mutation Exposure Analysis
```

More specifically:

```text
Static oracle-gap analysis for diff-derived mutation probes.
```

The tool is not trying to be a coverage dashboard, a full mutation engine, a
proof system, a second rust-analyzer, or a generic LLM test generator. Its job is
to shorten the path from "this behavior changed" to "this is the exact test
oracle that should notice."

## What ripr Does

`ripr` reads changed Rust code, creates mutation-shaped probes, and estimates
whether related tests appear to reach, infect, propagate, observe, and
discriminate the changed behavior.

It looks for missing or weak test oracles such as:

- boundary changes without boundary-value assertions
- error-path changes checked only with `is_err()`
- return-value changes checked only with smoke assertions
- field construction changes without field, object, or snapshot assertions
- side effects without mock, event, state, persistence, or metric oracles

## What ripr Does Not Do

`ripr` does not run mutants.

It does not report `killed` or `survived`, prove test adequacy, replace coverage,
or replace real mutation testing. Use a real mutation runner, such as
`cargo-mutants`, when the change is ready for confirmation.

## Where It Fits

```text
coverage:
  did this code execute?

ripr:
  does changed behavior appear exposed to a meaningful oracle?

mutation testing:
  did tests fail when a concrete mutant was run?
```

The goal is fast, honest oracle-gap feedback while code is still changing.

## Ecosystem Positioning

| Existing layer | What it answers | Gap |
| --- | --- | --- |
| `cargo-llvm-cov` | Did code execute? | Not oracle-aware |
| selective retest tools | Which tests are impacted? | Not assertion-aware |
| `cargo-mutants` | Did real mutants survive? | Too expensive for live draft feedback |
| `rust-analyzer` | What does Rust code mean in the editor? | Does not rank mutation exposure |
| `ripr` | Does changed behavior appear exposed to a meaningful oracle? | New middle layer |

## Install

Install from crates.io:

```bash
cargo install ripr
```

Links:

- crates.io: https://crates.io/crates/ripr
- docs.rs: https://docs.rs/ripr

For local development from this repository:

```bash
cargo install --path crates/ripr
```

`ripr` targets Rust 2024 and requires Rust `1.92` or newer.

## Quick Start

```bash
# Check local tooling and workspace shape
ripr doctor

# Analyze the current Git diff against origin/main
ripr check --base origin/main

# Analyze an explicit unified diff
ripr check --diff example.diff

# Emit stable JSON for tools and agents
ripr check --diff example.diff --json

# Emit GitHub Actions annotations
ripr check --diff example.diff --format github

# Explain one finding
ripr explain --diff example.diff probe:src_lib.rs:88:predicate

# Emit an agent-ready context packet
ripr context --diff example.diff --at probe:src_lib.rs:88:predicate --json

# Start the experimental LSP sidecar
ripr lsp
```

## Example Finding

```text
WARNING src/pricing.rs:88

Static exposure: weakly_exposed (predicate, control)

Changed behavior:
  after:  if amount >= discount_threshold {

RIPR:
  Reach:        yes
  Infect:      weak
  Propagate:   yes
  Observe:     yes
  Discriminate: weak

Gap:
  - No detected boundary input for the changed predicate
  - No strong discriminator was detected

Recommended next step:
  Add below, equal, and above threshold tests with exact assertions.
```

## Output Formats

Human output is optimized for local use.

JSON output is versioned and intended for editor integrations, CI, and coding
agents:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "mode": "draft",
  "base": "origin/main",
  "findings": []
}
```

GitHub output emits workflow annotations.

## Classifications

| Classification | Meaning |
| --- | --- |
| `exposed` | Static evidence suggests a complete RIPR path to a strong oracle. |
| `weakly_exposed` | A path exists, but infection or discrimination appears weak. |
| `reachable_unrevealed` | Related tests appear reachable, but no meaningful oracle was found. |
| `no_static_path` | No static test path was found for the changed owner. |
| `infection_unknown` | Reachability exists, but input or fixture evidence is opaque. |
| `propagation_unknown` | The changed behavior crosses an opaque propagation boundary. |
| `static_unknown` | Syntax-first analysis cannot make a credible judgment. |

## Current Scope

The current alpha line is intentionally narrow:

- one published package: `ripr`
- one CLI binary: `ripr`
- one shared analysis engine
- syntax-first unified diff analysis
- basic Rust function, test, and assertion indexing
- human, JSON, and GitHub outputs
- experimental LSP sidecar

The package is not split into `ripr-core`, `ripr-cli`, or `ripr-lsp`. Public
crate boundaries can be added later if external consumers need them.

## Current Capability Snapshot

`ripr` is currently strongest as a fast, syntax-first draft signal. The next
planned work is to make findings more grounded through fixtures, file facts,
parser-backed syntax, stable probe ownership, richer oracle facts, local flow,
and activation/value modeling.

| Capability | Current state | Next checkpoint |
| --- | --- | --- |
| Distribution | Crate and extension packaging paths exist. | Verify one-click editor install from a fresh profile. |
| Diff analysis | Syntax-first changed-line probes. | Parser-backed changed-node facts. |
| Test discovery | Basic Rust test/assertion indexing. | AST-backed test and oracle extraction. |
| Output | Human, JSON, GitHub annotation formats. | Golden fixture lab and stable DTOs. |
| LSP | Experimental sidecar. | Evidence-aware diagnostics, hover, and context actions. |
| Agent context | Compact context packet. | Test-writing brief with missing values and assertion shape. |
| Calibration | Not yet connected to real mutation outcomes. | `cargo-mutants` import after static facts improve. |

## Development

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

Useful sample commands:

```bash
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff --json
```
