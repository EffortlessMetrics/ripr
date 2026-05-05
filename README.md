# ripr

[![ripr](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/EffortlessMetrics/ripr/main/badges/ripr.json)](docs/BADGE_POLICY.md)
[![ripr+](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/EffortlessMetrics/ripr/main/badges/ripr-plus.json)](docs/BADGE_POLICY.md)
[![CI](https://github.com/EffortlessMetrics/ripr/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/EffortlessMetrics/ripr/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/EffortlessMetrics/ripr/branch/main/graph/badge.svg)](https://codecov.io/gh/EffortlessMetrics/ripr)
[![crates.io](https://img.shields.io/crates/v/ripr.svg)](https://crates.io/crates/ripr)
[![crates.io downloads](https://img.shields.io/crates/d/ripr.svg)](https://crates.io/crates/ripr)
[![docs.rs](https://docs.rs/ripr/badge.svg)](https://docs.rs/ripr)
[![VS Marketplace Installs (manual)](https://img.shields.io/badge/VS%20Marketplace-1%20install-0078D4)](https://marketplace.visualstudio.com/items?itemName=EffortlessMetrics.ripr)
[![Open VSX Downloads](https://img.shields.io/open-vsx/dt/EffortlessMetrics/ripr?label=Open%20VSX%20downloads)](https://open-vsx.org/extension/EffortlessMetrics/ripr)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](Cargo.toml)
[![MSRV](https://img.shields.io/badge/MSRV-1.93-blue)](https://www.rust-lang.org/)

<!-- The `ripr` and `ripr+` badges count unresolved repo-scoped exposure findings under the configured policy — inbox-zero, not coverage. The v1 baseline reflects currently-probeable production syntax shapes; it is not full seam inventory or mutation adequacy proof. For v1 dogfood, this repo commits the Shields endpoint JSON under `badges/` and Shields fetches it from `raw.githubusercontent.com`; refresh with `cargo xtask update-badge-endpoints`. The ripr product contract is "ripr emits Shields-compatible JSON" — downstream users can self-host the JSON anywhere stable and are not expected to enable GitHub Pages. See [docs/BADGE_POLICY.md](docs/BADGE_POLICY.md) and `deferred/hosted-badge-service` in [docs/DEFERRED.md](docs/DEFERRED.md). -->


<!-- VS Marketplace install count is manually maintained. Last checked: 2026-05-02 from the public listing. Refresh from publisher metrics after publish. Do not use live VS Marketplace Shields routes. -->

`ripr` helps Rust developers and coding agents answer a draft-time testing
question:

```text
For the behavior changed in this diff, do the current tests appear to contain
a discriminator that would notice if that behavior were wrong?
```

`ripr` is a static **Reachability-Infection-Propagation-Revealability (RIPR)**
exposure analyzer for Rust workspaces. It reads changed Rust code, builds
mutation-shaped static probes, and reports whether existing tests appear to
reach, affect, propagate, observe, and discriminate the changed behavior.

It is alpha software. The current release is useful for fast feedback while a
pull request is moving. It is not a proof system, and it does not replace real
mutation testing.

## The Problem

Coverage can tell you that code executed.

Mutation testing can tell you whether a concrete mutated version of the code was
caught by the test suite.

During everyday development, teams often need a cheaper question answered
earlier:

```text
This behavior changed. Do the nearby tests actually check the thing that changed?
```

That is what `ripr` is for.

It is designed to find weak or missing test discriminators, such as:

- a boundary change without equality-boundary assertions
- an error-path change checked only with `is_err()`
- a return-value change covered only by a smoke assertion
- a field construction change without field or object assertions
- a side effect without a mock, event, state, persistence, or metric oracle

## What ripr Does

`ripr` analyzes a diff and classifies static exposure evidence for changed
behavior.

It looks for:

```text
Reachability:
  can a related test reach the changed code?

Infection:
  could the changed expression alter local state or control?

Propagation:
  could that altered state reach a visible boundary?

Revealability:
  does a test oracle appear to observe the affected behavior?
```

Then it reports whether the current tests appear to expose the changed behavior
to a meaningful discriminator.

## What ripr Does Not Do

`ripr` does not run mutants.

It does not report `killed` or `survived`, prove test adequacy, replace
coverage, or replace real mutation testing.

Use `ripr` for fast, static, draft-time oracle-gap feedback. Use a real mutation
runner, such as `cargo-mutants`, when the change is ready for execution-backed
confirmation.

## Where ripr Fits

```text
coverage:
  did this code execute?

ripr:
  does changed behavior appear exposed to a meaningful test oracle?

mutation testing:
  did tests fail when a concrete mutant was run?
```

`ripr` is the middle layer: faster and more targeted than mutation testing, more
oracle-aware than coverage.

## Install

Install from crates.io:

```bash
cargo install ripr
```

Links:

- crates.io: <https://crates.io/crates/ripr>
- docs.rs: <https://docs.rs/ripr>

For local development from this repository:

```bash
cargo install --path crates/ripr
```

`ripr` targets Rust 2024 and requires Rust `1.93` or newer.

## Quick Start

Check local tooling and workspace shape:

```bash
ripr doctor
```

Check the current Git diff against `origin/main`:

```bash
ripr check --base origin/main
```

Analyze an explicit unified diff:

```bash
ripr check --diff example.diff
```

Emit JSON for tools, editors, CI, or agents:

```bash
ripr check --diff example.diff --json
```

Emit GitHub Actions annotations:

```bash
ripr check --diff example.diff --format github
```

Explain a finding:

```bash
ripr explain --diff example.diff probe:src_lib.rs:88:predicate
```

Emit an agent context packet:

```bash
ripr context --diff example.diff --at probe:src_lib.rs:88:predicate --json
```

Start the experimental language server:

```bash
ripr lsp --stdio
```

## Example Finding

```text
WARNING src/pricing.rs:88

Static exposure: weakly_exposed
Probe: predicate

Changed behavior:
  if amount >= discount_threshold {

Evidence:
  Reachability:     related tests found
  Infection:        changed predicate can alter branch behavior
  Propagation:      branch appears to influence returned total
  Revealability:    tests assert returned values, but no equality-boundary case was found

Gap:
  No detected test input for amount == discount_threshold.

Recommended next step:
  Add below, equal, and above-threshold tests with exact assertions.
```

The wording is intentionally conservative. Static analysis can identify evidence
and gaps; it should not claim real mutation outcomes.

## Classifications

| Classification | Meaning |
| --- | --- |
| `exposed` | Static evidence suggests a complete RIPR path to a strong oracle. |
| `weakly_exposed` | A path exists, but infection or discrimination appears weak. |
| `reachable_unrevealed` | Related tests appear reachable, but no meaningful oracle was found. |
| `no_static_path` | No static test path was found for the changed owner. |
| `infection_unknown` | Reachability exists, but input or fixture evidence is opaque. |
| `propagation_unknown` | The changed behavior crosses an opaque propagation boundary. |
| `static_unknown` | Static analysis cannot make a credible judgment. |

`ripr` should not use mutation-runtime outcome language such as `killed` or
`survived` unless explicit real mutation data is being reported in a calibration
context.

## Current Scope

The current alpha line is intentionally narrow:

- one published package: `ripr`
- one CLI binary: `ripr`
- one shared analysis engine
- syntax-backed unified diff analysis
- parser-backed Rust function, test, assertion, owner, and probe facts with
  lexical fallback
- human, JSON, and GitHub outputs
- experimental LSP sidecar

The package is not split into `ripr-core`, `ripr-cli`, or `ripr-lsp`. Public
crate boundaries can be added later if external consumers need them.

## Current Capability Snapshot

`ripr` is currently strongest as a fast, syntax-backed draft signal with
first-class repo seam evidence.

Current capabilities:

| Capability | Current state | Next checkpoint |
| --- | --- | --- |
| Distribution | Crate and extension packaging paths exist. | Verify one-click editor install from a fresh profile. |
| Diff analysis | Evidence-first Voice A findings with syntax-backed changed-line probes, probe-relative oracle strength, local flow sinks, observed/missing activation values, and explicit stop reasons. | Campaign 5B config and policy. |
| Repo seam inventory | First-class `RepoSeam` model with deterministic seam IDs, cached seam fact layers, test-grip evidence across the five RIPR stages, and 11-class `SeamGripClass` classification. | Campaign 5B config and policy. |
| Test discovery | Parser-backed test and assertion facts with exact, broad, relational, snapshot, mock, smoke, custom-helper, side-effect observer, and unknown oracle kinds; per-test efficiency ledger with smoke/broad/disconnected/opaque/circular/likely-vacuous reasons and duplicate-discriminator groups. | Campaign 5B config and policy. |
| Output | Human, JSON, context, and GitHub formats render evidence-first findings with stop reasons; repo exposure report and v2 agent seam packets render classified seam evidence; `ripr` and `ripr+` Shields badges publish unresolved gap counts. | Seam-native badge count mapping after config. |
| LSP | Experimental `tower-lsp-server` sidecar with evidence-aware Finding diagnostics, related-test links, hovers, server-side context packets, seam-native diagnostics + hover, and seam code actions for copying packets/assertions and opening related tests (seam diagnostics remain off by default). | `ripr.toml` seam-diagnostic defaults. |
| Agent context | Compact context packet plus per-seam `write_targeted_test` and `inspect_static_limitation` packets carrying recommended test placement, nearest tests to imitate, candidate values, missing discriminators, patterns to imitate/avoid, and assertion templates. Documented agent dispatch workflow in `docs/AGENT_DISPATCH_WORKFLOW.md`. | Campaign 5B config and policy. |
| Calibration | Advisory `cargo xtask mutation-calibration` joins imported cargo-mutants runtime data to static seam evidence by `seam_id` or unambiguous file/line and writes `target/ripr/reports/mutation-calibration.{json,md}`; ambiguous file/line candidates stay unassigned. | Future calibration fixtures; runtime mutation language stays inside calibration/runtime reports. |

Campaign 5A is complete. The active product work is Campaign 5B —
operationalizing the seam evidence:

```text
ripr.toml config
SARIF and opt-in CI policy
seam-native badge count mapping
```

Deeper capability state lives in [Capability matrix](docs/CAPABILITY_MATRIX.md)
and [Metrics](docs/METRICS.md).

## Editor Extension

The VS Code extension starts `ripr lsp --stdio` and can resolve the server from:

```text
1. explicit ripr.server.path
2. bundled server binary, if present
3. downloaded cached server binary
4. verified first-run GitHub Release download
5. ripr on PATH
6. actionable error
```

See:

- [Editor extension](docs/EDITOR_EXTENSION.md)
- [Server provisioning](docs/SERVER_PROVISIONING.md)
- [Marketplace release](docs/RELEASE_MARKETPLACE.md)

## For Contributors

Most contributors should use the repo automation instead of remembering the gate
order manually.

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask check-pr
cargo xtask pr-summary
```

Useful evidence commands:

```bash
cargo xtask fixtures
cargo xtask goldens check
cargo xtask golden-drift
cargo xtask test-oracle-report
cargo xtask dogfood
cargo xtask reports index
cargo xtask receipts
cargo xtask receipts check
cargo xtask check-allow-attributes
cargo xtask check-local-context
cargo xtask metrics
```

A good `ripr` PR is scoped by production risk, not line count:

```text
small production delta
complete evidence package
clear spec-test-code-output-metric trail
```

Large fixture, golden, spec, docs, metrics, or traceability diffs are welcome
when they make one production behavior reviewable.

See:

- [Scoped PR contract](docs/SCOPED_PR_CONTRACT.md)
- [PR automation](docs/PR_AUTOMATION.md)
- [Engineering rules](docs/ENGINEERING.md)
- [Agent workflows](docs/AGENT_WORKFLOWS.md)
- [Codex Goals](docs/CODEX_GOALS.md)

## Supporting Docs

Start here:

| Need | Doc |
| --- | --- |
| Understand the model | [Static exposure model](docs/STATIC_EXPOSURE_MODEL.md) |
| Understand JSON/context output | [Output schema](docs/OUTPUT_SCHEMA.md) |
| See current product direction | [Roadmap](docs/ROADMAP.md) |
| See active campaigns | [Implementation campaigns](docs/IMPLEMENTATION_CAMPAIGNS.md) |
| See implementation checkpoints | [Implementation plan](docs/IMPLEMENTATION_PLAN.md) |
| Run Codex Goals | [Codex Goals](docs/CODEX_GOALS.md) |
| See behavior contracts | [Specs](docs/specs/README.md) |
| See design decisions | [ADRs](docs/adr/README.md) |
| Add or review fixtures | [Testing](docs/TESTING.md) and [Test taxonomy](docs/TEST_TAXONOMY.md) |
| Understand repo automation | [PR automation](docs/PR_AUTOMATION.md) |
| Understand architecture | [Architecture](docs/ARCHITECTURE.md) |
| Review capability state | [Capability matrix](docs/CAPABILITY_MATRIX.md) and [Metrics](docs/METRICS.md) |
| Contribute a scoped PR | [Contributing](CONTRIBUTING.md) and [Scoped PR contract](docs/SCOPED_PR_CONTRACT.md) |
| Understand CI | [CI strategy](docs/CI.md) |
| Understand dogfooding | [Dogfooding](docs/DOGFOODING.md) |
| Understand docs organization | [Documentation system](docs/DOCUMENTATION.md) |
| Capture repo knowledge | [Learnings](docs/LEARNINGS.md) |
| Release the crate or extension | [Release](docs/RELEASE.md), [Publishing](docs/PUBLISHING.md), and [Open VSX](docs/OPENVSX.md) |
| Work on extension assets | [Brand assets](assets/logo/README.md) |
| Find repo instructions for agents | [Agent instructions](AGENTS.md) |

## Development

Run the normal local gate:

```bash
cargo xtask check-pr
```

Run the full Rust checks directly:

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
```

Release/package checks:

```bash
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

Useful sample commands:

```bash
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff
cargo run -p ripr -- check --diff crates/ripr/examples/sample/example.diff --json
```
