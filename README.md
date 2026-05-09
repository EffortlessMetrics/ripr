# ripr

[![ripr](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/EffortlessMetrics/ripr/main/badges/ripr.json)](docs/BADGE_POLICY.md)
[![ripr+](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/EffortlessMetrics/ripr/main/badges/ripr-plus.json)](docs/BADGE_POLICY.md)
[![CI](https://github.com/EffortlessMetrics/ripr/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/EffortlessMetrics/ripr/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/EffortlessMetrics/ripr/branch/main/graph/badge.svg)](https://codecov.io/gh/EffortlessMetrics/ripr)
[![crates.io](https://img.shields.io/crates/v/ripr.svg)](https://crates.io/crates/ripr)
[![crates.io downloads](https://img.shields.io/crates/d/ripr.svg)](https://crates.io/crates/ripr)
[![docs.rs](https://docs.rs/ripr/badge.svg)](https://docs.rs/ripr)
[![VS Marketplace Installs (manual)](https://img.shields.io/badge/VS%20Marketplace-2%20installs-0078D4)](https://marketplace.visualstudio.com/items?itemName=EffortlessMetrics.ripr)
[![Open VSX Downloads](https://img.shields.io/open-vsx/dt/EffortlessMetrics/ripr?label=Open%20VSX%20downloads)](https://open-vsx.org/extension/EffortlessMetrics/ripr)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](Cargo.toml)
[![MSRV](https://img.shields.io/badge/MSRV-1.93-blue)](https://www.rust-lang.org/)

<!-- The public `ripr` and `ripr+` badges count unresolved repo-scoped seam-native exposure gaps under the configured policy — inbox-zero, not coverage. Diff-scoped badge artifacts remain legacy finding-exposure artifacts for PR summaries. This repo commits the Shields endpoint JSON under `badges/` and Shields fetches it from `raw.githubusercontent.com`; refresh with `cargo xtask update-badge-endpoints`. The ripr product contract is "ripr emits Shields-compatible JSON" — downstream users can self-host the JSON anywhere stable and are not expected to enable GitHub Pages. See [docs/BADGE_POLICY.md](docs/BADGE_POLICY.md) and `deferred/hosted-badge-service` in [docs/DEFERRED.md](docs/DEFERRED.md). -->


<!-- VS Marketplace install count is manually maintained. Last checked: 2026-05-07 after the 0.4.0 publish from the public listing. Refresh from publisher metrics when updating this manual count. Do not use live VS Marketplace Shields routes. -->

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

## Coverage

The Codecov badge above indicates execution-surface evidence only: whether code paths executed during testing. Coverage does not prove test adequacy, correctness, safety, or completeness. See [Coverage](docs/ci/coverage.md) for what the coverage signal measures and does not claim.

## Quick Start

Most users start from the surface they already use. The CLI is the shared
engine behind each path, but it is not the required first interface for editor
or CI users.

| User type | First action | Main doc |
| --- | --- | --- |
| VS Code user | Install `EffortlessMetrics.ripr`, open a Rust workspace, then use the status bar, Problems, hover, and seam actions. | [Quickstart](docs/QUICKSTART.md#vs-code-first-hour) |
| CI user | Generate or copy the advisory GitHub workflow. | [Quickstart](docs/QUICKSTART.md#ci-first-hour) |
| CLI user | Run `ripr pilot --root .` and add one focused test for the top seam. | [Quickstart](docs/QUICKSTART.md#cli-first-hour) |
| Agent or reviewer | Start from `ripr agent status --root .` or a selected-seam workflow packet. | [LLM operator guide](docs/LLM_OPERATOR_GUIDE.md) |

For the full first-hour path, including troubleshooting and known limits, read
[Quickstart](docs/QUICKSTART.md).

### VS Code

Install `EffortlessMetrics.ripr` from VS Code Marketplace or Open VSX, then
open a Rust/Cargo workspace. Normal editor installs do not require
`cargo install ripr`; the extension resolves the server from configuration,
bundled or cached assets, GitHub Releases, or PATH.

Use the `ripr` status bar item or `ripr: Show Status` first. Then inspect RIPR
Problems diagnostics, hover evidence, `Write targeted test` actions, agent
handoff commands, and `Open Best Related Test`.

### CI

```bash
ripr init --ci github
```

The generated workflow is advisory by default. It runs `ripr pilot`, writes a
RIPR advisory summary, uploads pilot/workflow/agent/report/review artifacts,
writes badge JSON, can optionally upload SARIF, and does not run mutation
tests.

### CLI

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

Run a zero-config pilot packet:

```bash
ripr pilot --root .
```

Read `target/ripr/pilot/pilot-summary.md`, add one focused test for the top
missing discriminator, then compare before and after snapshots:

```bash
ripr check --root . --mode ready --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json
ripr outcome --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json
```

For agent handoff or review automation, write the matching verification packet
and focused receipt:

```bash
mkdir -p target/ripr/agent
ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json > target/ripr/agent/agent-verify.json
ripr agent receipt --root . --verify-json target/ripr/agent/agent-verify.json --seam-id <seam_id> --json --out target/ripr/agent/agent-receipt.json
```

For local agent handoff state, start with:

```bash
ripr agent status --root .
ripr agent start --root . --seam-id <seam_id> --out target/ripr/workflow
ripr agent review-summary --root .
```

If the first run behaves unexpectedly:

```bash
ripr doctor
```

`ripr.toml` is optional. `ripr init` materializes repo-local policy when a team
wants to review, version, and tune it; it does not unlock basic usefulness.

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
| Distribution | `0.4.0` is published on crates.io, GitHub Releases, VS Marketplace, and Open VSX with server archives, VSIX packaging, generated CI workflow artifacts, release-readiness proof, and installed editor-agent loop smoke checks. | Post-release maintenance; keep registry, server, and marketplace surfaces aligned. |
| Diff analysis | Evidence-first Voice A findings with syntax-backed changed-line probes, probe-relative oracle strength, local flow sinks, observed/missing activation values, and explicit stop reasons. | Maintenance; no active analyzer-refactor lane. |
| Repo seam inventory | First-class `RepoSeam` model with deterministic seam IDs, cached seam fact layers, test-grip evidence across the five RIPR stages, and 11-class `SeamGripClass` classification. | Maintenance; no active analyzer-refactor lane. |
| Test discovery | Parser-backed test and assertion facts with exact, broad, relational, snapshot, mock, smoke, custom-helper, side-effect observer, and unknown oracle kinds; per-test efficiency ledger with smoke/broad/disconnected/opaque/circular/likely-vacuous reasons and duplicate-discriminator groups. | Maintenance; no active analyzer-refactor lane. |
| Output | Human, JSON, context, and GitHub formats render evidence-first findings with stop reasons; `ripr pilot` writes a zero-config first-run packet; `ripr outcome` compares before/after repo exposure snapshots; repo exposure report and v2 agent seam packets render classified seam evidence; public `ripr` and `ripr+` Shields badges publish seam-native unresolved gap counts while diff badge artifacts remain finding-exposure based. | Output contract maintenance. |
| LSP | Experimental `tower-lsp-server` sidecar with evidence-aware Finding diagnostics, related-test links, hovers, server-side context packets, seam-native diagnostics + hover, and seam code actions for copying packets/assertions and opening related tests. Saved-workspace diagnostics remain advisory; unsaved-buffer overlays are not default behavior. | Editor contract maintenance. |
| Agent context | Compact context packet plus per-seam `write_targeted_test` and `inspect_static_limitation` packets carrying recommended test placement, nearest tests to imitate, candidate values, missing discriminators, patterns to imitate/avoid, and assertion templates. `ripr agent start --root . --seam-id <id> --out target/ripr/workflow` writes a source-edit-free workflow packet, `ripr agent status --root . --json` reports local LLM loop artifact state and the next command without rerunning analysis, `ripr agent receipt` emits provenance plus bounded next-action guidance, and `ripr agent review-summary --root .` joins existing loop artifacts into compact review Markdown or schema `0.1` JSON. | Agent loop maintenance. |
| First useful action | `ripr first-action` writes advisory `first-useful-action.{json,md}` from explicit PR guidance, assistant proof, PR evidence ledger, baseline delta, receipt, optional gate, optional coverage/grip frontier, and editor context inputs without hidden analysis, source edits, generated tests, provider calls, mutation execution, or default CI blocking; generated CI projects the report as advisory summary/artifact content, and VS Code status/Show Status can project an existing workspace-matched report without new diagnostics. | `docs/first-useful-action-workflow` |
| Repository config | Repo-root `ripr.toml` can set analysis mode, oracle policy, severity mapping, suppressions path, report related-test caps, and LSP seam-diagnostic defaults. Explicit CLI flags and LSP initialization options still win. | Policy feedback after adoption. |
| SARIF and CI policy | `ripr check --format sarif` emits diff-scoped Finding SARIF and `--format repo-sarif` emits repo seam SARIF with configured severity, suppression metadata, stable rule IDs, and stable fingerprints. `ripr init --ci github` generates a non-blocking GitHub Actions report workflow with pilot/report artifacts, repo badge JSON, and optional SARIF rendering/upload; `cargo xtask sarif-policy` compares current SARIF to a baseline only when explicitly requested. | Advisory policy feedback after adoption. |
| Calibration | Advisory `ripr calibrate cargo-mutants` and repo-local `cargo xtask mutation-calibration` join imported cargo-mutants runtime data to static seam evidence by `seam_id` or unambiguous file/line; ambiguous file/line candidates stay unassigned. `fixtures/CALIBRATION_CORPUS.md` maps current fixtures to controlled calibration scenarios, `fixtures/EXAMPLE_CORPUS.md` links the checked boundary-gap calibration sample into the operator loop, and `fixtures/boundary_gap/calibration/runtime-fixtures-v1/` pins the main static/runtime agreement buckets. | Maintenance; runtime mutation language stays inside calibration/runtime reports. |

Campaigns 5A, 5B, 6, 7, 8, 9, 10, and 11 are complete. Campaign 7 closed the
defaults-first adoption lane: `ripr pilot`, `ripr outcome`, advisory
calibration import, the operator cockpit, the generated GitHub Action
entrypoint, the documented VS Code install path, the public example corpus, and
release/install proof are in place. Campaign 8 keeps runtime calibration
supplied and optional. Campaign 9 made the editor/operator warm paths measured
and bounded. Campaign 10 aligned the saved-workspace editor loop with the agent
CLI loop from diagnostic to evidence, packet or brief, focused test, outcome,
agent verify, agent receipt, cockpit, CI artifacts, and release-readiness proof.
Campaign 11 closed after the LLM work loop gained read-only artifact status,
centralized command templates, workflow manifests, provenance-backed receipts,
bounded next-action guidance, reviewer summaries, fixtures, generated CI packet
uploads, and the LLM operator guide. Campaign 12 then closed the First-Hour UX
lane: the editor first-run status path, intent-titled code actions, generated
CI advisory summary, generated workflow smoke fixture, and user-type
first-hour docs are pinned. Campaign 13 then closed PR Review Guidance:
`ripr review-comments` writes the advisory review report, generated CI runs it
before emitting advisory summaries and check annotations, placement and
suppression cases are fixture-pinned, and
[PR review guidance](docs/PR_REVIEW_GUIDANCE.md) documents the command, CI
behavior, summary-only fallback, and inline-comment opt-in boundary. Campaign
14 closed Recommendation Calibration: the checked advisory `cargo xtask
recommendation-calibration` report now measures whether PR-time
recommendations are useful, safely placed, properly suppressed, pointed at the
expected test target, and correlated with before/after static movement.
[Recommendation calibration workflow](docs/RECOMMENDATION_CALIBRATION.md)
documents how to read that report. Campaign 15 is closed as Calibrated Gate
Policy: RIPR-SPEC-0014 pins the optional gate contract, `ripr gate evaluate`
writes the read-only decision report, generated CI can run it only when
`RIPR_GATE_MODE` is explicitly configured, and
[Calibrated gate policy](docs/CALIBRATED_GATE_POLICY.md) documents the
operator workflow. [Baseline ledger workflow](docs/BASELINE_LEDGER_WORKFLOW.md)
shows how to create, diff, and shrink reviewed behavioral-grip debt baselines
on the path toward RIPR 0. Campaigns 20 and 21 then made the test-oracle
assistant proof loop and read-only proof report producer first-class advisory
artifacts. Campaign 22 is closed as First Useful Action: `ripr first-action`
now writes the read-only advisory report, generated CI projects it as advisory
summary/artifact content, the VS Code status path can show an existing
workspace-matched report without adding diagnostics or editor decorations, and
checked dogfood receipts prove the documented routing cases. The default
generated workflow remains advisory.

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

Normal editor install should not require `cargo install ripr`. The Cargo install
path remains available for offline, pinned, or controlled environments. The
`v0.4.0` release line includes the server manifest, per-target server
archives, checksums, and VSIX needed for this default path.

See:

- [Editor extension](docs/EDITOR_EXTENSION.md)
- [Server provisioning](docs/SERVER_PROVISIONING.md)
- [Marketplace release](docs/RELEASE_MARKETPLACE.md)
- [PR review guidance](docs/PR_REVIEW_GUIDANCE.md)

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
cargo xtask evidence-health
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
| Choose the first-hour path by user type | [Quickstart](docs/QUICKSTART.md) |
| Understand the model | [Static exposure model](docs/STATIC_EXPOSURE_MODEL.md) |
| Understand JSON/context output | [Output schema](docs/OUTPUT_SCHEMA.md) |
| Turn seam evidence into a test | [Targeted test workflow](docs/TARGETED_TEST_WORKFLOW.md) |
| Adopt a baseline debt ledger | [Baseline ledger workflow](docs/BASELINE_LEDGER_WORKFLOW.md) |
| Understand easy-start defaults | [Defaults-first adoption spec](docs/specs/RIPR-SPEC-0009-defaults-first-adoption.md) |
| See current product direction | [Roadmap](docs/ROADMAP.md) |
| See active campaigns | [Implementation campaigns](docs/IMPLEMENTATION_CAMPAIGNS.md) |
| See implementation checkpoints | [Implementation plan](docs/IMPLEMENTATION_PLAN.md) |
| Run Codex Goals | [Codex Goals](docs/CODEX_GOALS.md) |
| See behavior contracts | [Specs](docs/specs/README.md) |
| See design decisions | [ADRs](docs/adr/README.md) |
| Add or review fixtures | [Testing](docs/TESTING.md) and [Test taxonomy](docs/TEST_TAXONOMY.md) |
| Understand repo automation | [PR automation](docs/PR_AUTOMATION.md) |
| Roll out Droid review | [Roll out Factory Droid review](docs/how-to/roll-out-droid.md) and [Droid rollout checklist](docs/agent-context/droid-rollout.md) |
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
