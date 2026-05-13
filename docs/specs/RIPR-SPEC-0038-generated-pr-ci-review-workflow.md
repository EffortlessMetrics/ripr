# RIPR-SPEC-0038: Generated PR CI Review Workflow

Status: proposed

Owner: ripr maintainers

Lane: 4

Linked proposal:
[RIPR-PROP-0004: PR / CI Review Cockpit](../proposals/RIPR-PROP-0004-pr-ci-review-cockpit.md)

Linked plan:
[Lane 4 implementation plan](../../plans/lane4-pr-ci-review-cockpit/implementation-plan.md)

## Problem

Generated GitHub CI is the surface most reviewers see first. RIPR already
produces a PR review front panel, a report packet index, advisory summaries,
gate decisions, receipts, calibration context, and language-preview evidence.
Without an explicit generated-workflow contract, those surfaces can drift into
a file pile, duplicate authority, or hide missing artifacts.

Lane 4 needs generated CI to compose explicit artifacts into a reviewer-first
story while preserving the boundary that summaries and indexes are advisory
projections. Gate decisions, when configured, remain the pass/fail authority.

## Behavior

The generated PR CI workflow must be cheap, installable, predictable, and
advisory by default. Its canonical shape is:

```text
checkout and setup
-> install or run ripr
-> run public advisory report commands
-> render front panel and packet index when inputs exist
-> append a compact reviewer-first job summary
-> upload the complete report packet
-> preserve configured gate-decision authority
```

The workflow may summarize and upload evidence. It must not create analyzer
truth, rerun hidden analysis to patch missing artifacts, edit source, generate
tests, call providers, run mutation testing, publish inline comments, change
branch protection, or change default CI blocking.

The generated workflow must keep Rust-default behavior unchanged unless a later
spec explicitly changes that default. Language-aware grouping is opt-in and
appears only when configured language metadata includes more than Rust.

## Required Evidence

A generated workflow run should make these evidence boundaries visible:

- the public commands used to generate advisory PR-time artifacts;
- the path to the PR review front panel when present;
- the path to the report packet index when present;
- the uploaded report packet name and included directories;
- warnings for missing expected artifacts and known regeneration commands;
- the configured gate-decision artifact when present;
- advisory text explaining that the job summary, front panel, and packet index
  are not gate authority;
- preview-language labels when TypeScript, Python, or later preview adapters
  contribute grouped evidence;
- receipts or movement reports when supplied by existing artifacts.

Missing optional inputs remain warnings or missing-artifact entries. Missing
optional inputs must not fail generated CI unless an explicit gate decision
already reports `blocked` or `config_error`.

## Inputs

The generated workflow may consume only explicit repository and RIPR artifact
inputs:

| Input | Purpose |
| --- | --- |
| `ripr.toml` | Public configuration, including optional language settings. |
| `target/ripr/reports/**` | Report JSON, Markdown, calibration, gate, receipt, and index artifacts. |
| `target/ripr/review/**` | PR guidance and review-comment artifacts. |
| `target/ripr/workflow/**` | Agent and repair workflow evidence. |
| `target/ripr/agent/**` | Agent handoff packets when generated. |
| `target/ripr/receipts/**` | Repair and validation receipts. |
| `target/ci/**` | CI-local outputs and summaries. |

Generated CI must not inspect source to infer missing report fields, call
external providers for missing evidence, or depend on unpublished `xtask`
commands when a public `ripr` command is the intended installed surface.

## Outputs

The generated workflow should produce or upload:

- `target/ripr/reports/pr-review-front-panel.{json,md}` when its explicit
  inputs are available;
- `target/ripr/reports/index.{json,md}` when report packet inputs are
  available;
- lower-level report artifacts used by those projections;
- a GitHub job summary with reviewer-first sections;
- an uploaded `ripr-reports` artifact packet.

The job summary should prefer reviewer questions over schema topology:

```text
Start here
Top issue or fallback state
Policy and gate authority
Repair or regeneration command
Receipt and movement state
Uploaded artifacts
Advisory limits
```

When an expected artifact is missing, the summary or packet index should show
the missing artifact and the regeneration command when known. If no command is
known, the missing entry must say so explicitly.

## Generated Workflow Sections

The generated workflow should keep these sections stable enough for tests and
reviewers:

1. setup and tool availability;
2. advisory RIPR report generation;
3. optional gate evaluation when configured;
4. PR review front panel rendering;
5. report packet index rendering;
6. job-summary append;
7. artifact upload.

Gate evaluation may set pass/fail according to its own configured policy. The
front panel, packet index, and job summary may point to that decision, but they
do not become authority.

## Language-Aware Grouping

Rust-only generated CI output remains the default. For a configuration that
declares only Rust, generated workflow output must remain unchanged except for
explicitly accepted wording improvements.

When configuration declares additional preview languages:

- findings and advisory summaries may be grouped by language;
- preview languages must be labeled `preview` and advisory;
- preview groups must not be promoted to gate eligibility by the workflow;
- missing preview evidence must be shown as missing, disabled, or unsupported
  rather than hidden;
- gate authority remains unchanged.

Language grouping is a presentation and routing feature. It does not change
analysis, recommendation ranking, editor behavior, or policy semantics.

## Repair And Regeneration Commands

Generated CI should prefer copyable commands that use public `ripr` surfaces:

- regenerate the PR review front panel;
- regenerate the report packet index;
- run or refresh PR guidance;
- run gate evaluation when configured;
- run agent verification when a receipt path is available.

Commands must be bounded to explicit inputs and outputs. They must not imply
that generated CI can create tests, edit source, or run provider-backed repair.

## Non-Goals

- No analyzer behavior changes.
- No recommendation ranking changes.
- No report producer implementation in this spec PR.
- No hidden artifact discovery.
- No hidden analysis reruns.
- No generated tests.
- No source edits.
- No provider or API calls.
- No mutation execution.
- No LSP or editor behavior changes.
- No inline comment publishing.
- No branch-protection changes.
- No default CI blocking changes.
- No gate policy semantic changes.
- No runtime correctness or complete-test-suite claims.

## Acceptance Examples

- Rust-default generated CI runs the existing advisory workflow and produces
  the same required job outcomes as before.
- A complete report packet appends a summary that points first to
  `pr-review-front-panel.md` and `index.md`.
- A missing front panel leaves lower-level artifacts visible and shows the
  front-panel regeneration command when the command is known.
- A missing optional calibration report is a warning, not a workflow failure.
- A configured blocked gate points to `gate-decision.md` as authority and does
  not treat the job summary as the gate.
- A TypeScript preview packet groups advisory evidence under a preview label
  without changing gate eligibility.
- A Python preview packet with parse scaffold but no findings reports preview
  availability without inventing findings.
- A generated workflow never auto-refreshes baselines, branch protection, or
  gate defaults.

## Test Mapping

Existing generated-workflow tests cover advisory defaults, artifact upload, and
non-blocking workflow generation:

- `crates/ripr/src/cli/commands.rs::tests::init_generated_github_workflow_is_advisory`;
- `crates/ripr/src/cli/commands.rs::tests::init_generated_github_workflow_never_auto_refreshes_baseline`;
- `crates/ripr/src/cli/commands.rs::tests::init_generated_github_workflow_uploads_reports_and_makes_sarif_optional`;
- `crates/ripr/src/cli/commands.rs::tests::init_generated_github_workflow_matches_smoke_fixture`;
- `crates/ripr/tests/cli_smoke.rs::init_ci_github_dry_run_prints_config_and_workflow_without_writing`;
- `crates/ripr/tests/cli_smoke.rs::init_ci_github_writes_non_blocking_report_workflow`.

Follow-up implementation should add focused tests for:

- front-panel and packet-index summary placement;
- missing-artifact warnings and regeneration commands;
- gate-decision authority text;
- Rust-only output staying unchanged;
- configured preview-language grouping.

## Implementation Mapping

Existing implementation surfaces:

- `crates/ripr/src/cli/commands.rs` generates GitHub workflow YAML;
- `crates/ripr/tests/cli_smoke.rs` exercises public `ripr init ci github`
  behavior;
- `docs/PR_AUTOMATION.md` documents shape/check/report packet automation.

Follow-up Lane 4 implementation should:

- audit current generated workflow behavior against this spec;
- add or adjust generated summary text only where it improves cockpit
  readability without changing gate defaults;
- wire language-aware grouping only after preview-language evidence is ready
  and explicitly configured;
- add dogfood receipts for missing-proof, blocked-gate, improved, unchanged,
  and preview-language packet cases.

## Metrics

Future metrics may count:

- `generated_pr_ci_workflows`;
- `generated_pr_ci_front_panel_present`;
- `generated_pr_ci_packet_index_present`;
- `generated_pr_ci_missing_expected_artifacts`;
- `generated_pr_ci_regeneration_commands`;
- `generated_pr_ci_gate_authority_links`;
- `generated_pr_ci_language_groups`;
- `generated_pr_ci_preview_language_groups`;
- `generated_pr_ci_rust_default_unchanged`;
- `generated_pr_ci_advisory_summaries`.

## Validation

The spec PR should run:

```bash
cargo xtask check-spec-format
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-pr
git diff --check
```
