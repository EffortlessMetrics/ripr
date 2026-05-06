# RIPR-SPEC-0009: Defaults-First Adoption

Status: proposed

## Problem

`ripr` now has useful static seam evidence, editor actions, SARIF, badges,
targeted-test receipts, and calibration reports. The product risk has shifted
from missing machinery to too much operator assembly.

A new user should not need to understand every report format, `cargo xtask`
helper, SARIF policy mode, badge count basis, LSP cockpit report, or
calibration artifact before getting value. The default experience should expose
one conservative loop:

```text
install ripr
run one pilot scan
write one focused test
compare before and after evidence
materialize repo policy when the team wants to review or tune defaults
optionally import runtime calibration data
```

Defaults-first adoption defines how each install surface behaves before teams
start tuning policy.

## Behavior

Every public surface should be useful with built-in defaults, while preserving
explicit configuration for teams that need policy control.

The default contract is:

- `ripr.toml` is optional for first value. Missing config uses built-in
  conservative defaults and is reported as such by `ripr doctor`.
- `ripr init` materializes repo policy. It does not unlock the useful
  experience.
- A missing `ripr.toml` and a freshly generated `ripr.toml` should produce
  equivalent default policy behavior, except for filesystem artifacts
  intentionally created by `init` or other commands.
- Explicit CLI flags and LSP initialization options override repo config.
- Repo config overrides built-in defaults.
- Malformed or unknown config fails closed with an actionable path and message.
- Static outputs must use static evidence vocabulary and must not claim runtime
  mutation confirmation.
- Runtime mutation vocabulary belongs only in explicit calibration/runtime
  reports supplied with runtime data.
- CI and SARIF behavior is advisory unless the user explicitly enables a
  baseline failure policy.
- Unsaved-buffer editor overlays are not enabled by default.
- Solved, intentional, and suppressed seams are hidden from default operator
  attention.
- Actionable weak, missing, reachable, and unknown seam classes remain visible
  with conservative severities.

## Surface Defaults

| Surface | Default should do | Default should not do |
| --- | --- | --- |
| CLI | Produce readable static evidence and next-step guidance from ordinary commands. | Require format knowledge or repo config before first value. |
| `ripr doctor` | Show config state, server/tooling availability, and whether defaults or repo policy are active. | Print config source text or silently ignore malformed policy. |
| `ripr init` | Materialize the conservative built-in defaults into `ripr.toml` when requested, and optionally generate advisory GitHub SARIF CI with `--ci github`. | Unlock basic usefulness, overwrite repo policy or generated workflow files without `--force`, or create blocking CI by default. |
| `ripr pilot` | Generate a standard pilot packet and print the top actionable next step. | Run mutation testing, edit files, or require users to know internal report names. |
| `ripr outcome` | Compare two repo-exposure snapshots and explain whether evidence moved. | Require the `ripr` source repo or `cargo xtask`. |
| Operator cockpit | Join existing repo-local reports into one next-action view. | Rerun analysis implicitly, edit code, or replace the public `ripr pilot` path. |
| Calibration import | Join supplied runtime data to static seam evidence and explain agreement buckets. | Run mutation testing or change static classifications. |
| LSP / VS Code | Make bounded saved-workspace diagnostics, hovers, targeted briefs, context packets, best related test, and refresh status discoverable. | Stay inert until `ripr init`, surprise users with expensive live unsaved-buffer analysis, or run deep analysis by default. |
| SARIF / GitHub Actions | Upload advisory code-scanning results from static evidence. | Fail CI without explicit baseline policy. |
| Badges | Report configured-visible unresolved seam counts. | Present counts as coverage, test completeness, or runtime mutation confirmation. |

## Default Config Profile

The generated conservative profile for `ripr init` should materialize the same
policy defaults that built-in missing-config behavior uses:

```toml
[analysis]
mode = "draft"
include_unchanged_tests = true

[oracles]
snapshot_strength = "medium"
mock_expectation_strength = "medium"
broad_error_strength = "weak"

[severity.seams]
strongly_gripped = "off"
weakly_gripped = "warning"
ungripped = "warning"
reachable_unrevealed = "warning"
activation_unknown = "info"
propagation_unknown = "info"
observation_unknown = "info"
discrimination_unknown = "info"
opaque = "info"
intentional = "off"
suppressed = "off"

[lsp]
seam_diagnostics = true

[reports]
max_related_tests = 5

[suppressions]
path = ".ripr/suppressions.toml"
```

Built-in defaults should provide the same conservative operator experience as
the generated init profile. Surfaces may bound cost with saved-workspace,
draft-mode behavior, diagnostic caps, lazy refresh, and clear status messaging,
but they should not be inert by default. Repository config can still disable
surfaces explicitly when a team wants a quieter policy.

## Default Scope And Mode Vocabulary

Defaults-first surfaces use operator vocabulary, but every user-facing word maps
to a concrete analysis mode:

| Operator stance | Concrete mode | Current scope |
| --- | --- | --- |
| Fastest feedback | `instant` | Changed Rust files only. |
| Normal/default | `draft` | Rust files in packages touched by the diff, including unchanged tests. |
| PR fast scan | `fast` | Same package-local scope as `draft` for now. |
| Deep static scan | `deep` | All Rust files in the workspace. |
| Ready preflight | `ready` | All Rust files in the workspace before separate mutation confirmation. |

Repo-scoped reports and badges must represent the published Rust package
surface. They exclude repository automation and non-production trees such as
`xtask/`, top-level fixtures, editor-extension sources, `target/`,
`node_modules/`, tests, examples, benches, and `src/tests.rs`. Passing a
fixture workspace as `--root` remains valid and analyzes that fixture normally.

## Pilot Packet

The first public pilot path should converge on these user-facing files:

```text
target/ripr/pilot/repo-exposure.json
target/ripr/pilot/repo-exposure.md
target/ripr/pilot/agent-seam-packets.json
target/ripr/pilot/pilot-summary.json
target/ripr/pilot/pilot-summary.md
```

The pilot terminal summary should answer:

```text
What is the top actionable seam?
Why did RIPR flag it?
What focused test should I write next?
Which file contains the structured packet?
How do I compare before and after after adding the test?
```

The pilot command must remain advisory. It should not edit source files,
generate tests, run mutation testing, or enable CI blocking policy.

## Outcome Receipt

The public before/after receipt command should expose the existing targeted-test
outcome behavior through the installed `ripr` binary. The user should not need
this repository checked out.

Minimum command shape:

```bash
ripr outcome --before before.repo-exposure.json --after after.repo-exposure.json
```

The receipt should report:

- before and after grip-class counts;
- moved seams;
- unchanged seams with evidence deltas;
- regressed seams;
- new seams;
- removed seams;
- input paths and advisory status.

The command should default to a human-readable Markdown or text surface, with a
JSON option for tools.

## Operator Cockpit

The repo-local operator cockpit should join existing report artifacts under
`target/ripr/reports/` into one "what do I do next?" surface:

```bash
cargo xtask operator-cockpit
```

The command writes:

```text
target/ripr/reports/operator-cockpit.json
target/ripr/reports/operator-cockpit.md
```

The cockpit should answer:

- top weak seams;
- why each listed seam matters;
- the suggested next targeted test;
- whether LSP, SARIF, badge, targeted-test receipt, and calibration surfaces
  are present and aligned with the operator loop;
- the next command to run.

It should not rerun analysis implicitly. Missing inputs should stay visible with
their generator command so operators can decide which report to refresh.
The cockpit should read repo exposure, LSP cockpit, SARIF policy, badge status,
targeted-test outcome, and optional mutation calibration artifacts. Required
inputs should be marked with a `required` boolean, while optional calibration
should remain visible without making the cockpit fail. When an input artifact is
present but its JSON has no top-level `status` field, the cockpit should report
that artifact status as `present` instead of treating the missing status as an
error. Markdown output should include a surface-alignment table with `Surface`,
`State`, `Status`, `Agreement`, and `Signal` columns.

## Calibration Import

Calibration import should expose the existing advisory static/runtime join
through the installed `ripr` binary:

```bash
ripr calibrate cargo-mutants \
  --mutants-json target/mutants/outcomes.json \
  --repo-exposure-json target/ripr/pilot/after.repo-exposure.json
```

The command should:

- accept already-produced runtime mutation data;
- join by `seam_id` first and unambiguous normalized file/line second;
- preserve ambiguous file/line matches without assigning them to the first
  static seam;
- summarize agreement buckets;
- keep static and runtime fields separate;
- keep runtime vocabulary out of static reports.

It must not run mutation testing, change static classifications, or become CI
blocking by default.

## Required Evidence

Defaults-first adoption evidence should cover:

- documentation for every default surface listed above;
- built-in missing-config behavior matching the generated init profile's
  default policy behavior;
- fast/normal/deep operator mode vocabulary pinned to concrete analysis scopes;
- repo-scoped report and badge inputs excluding repository automation and
  non-production trees while fixture roots still work;
- default badge and report behavior staying advisory and configured-visible;
- `ripr init` generated config preserving conservative defaults;
- default repo discovery excluding generated, policy-only, fixture-only, and
  package-manager directories before mode-specific scope is applied;
- `ripr init --dry-run` printing without writing;
- `ripr init --force` being required to overwrite existing `ripr.toml`;
- `ripr init --ci github` generating a non-blocking advisory GitHub workflow;
- `ripr pilot` generating the pilot packet and top actionable next step;
- `ripr outcome` producing the same movement buckets as the xtask receipt;
- `cargo xtask operator-cockpit` joining repo exposure, LSP cockpit,
  SARIF policy, badge status, targeted-test outcome, and optional mutation
  calibration into one next-action report;
- calibration import producing the same agreement buckets as the xtask report;
- missing config remaining healthy and visible in `ripr doctor`;
- malformed config failing closed;
- generated GitHub Actions workflow being advisory by default;
- LSP saved-workspace behavior and no unsaved-buffer overlays by default;
- static output language checks preserving conservative vocabulary.

## Non-Goals

This spec does not require:

- analyzer behavior changes;
- new seam or finding classes;
- LSP protocol changes;
- SARIF renderer changes;
- badge semantic changes;
- CI blocking by default;
- hosted services;
- runtime mutation execution;
- unsaved-buffer overlays;
- automatic test generation;
- crate splitting.

## Acceptance Examples

### Missing config still works

```text
Given a Rust workspace with no ripr.toml,
when a user runs ripr check,
then ripr uses built-in conservative defaults and reports static evidence
without requiring configuration.
```

### Init writes conservative policy

```text
Given a Rust workspace with no ripr.toml,
when a user runs ripr init,
then ripr writes a conservative repo config that hides solved, intentional, and
suppressed seam classes while showing actionable weak and unknown classes.
```

### Init does not overwrite accidentally

```text
Given a workspace with an existing ripr.toml,
when a user runs ripr init without --force,
then ripr exits with an actionable message and leaves the file unchanged.
```

### Pilot points at one next action

```text
Given a workspace with repo seam evidence,
when a user runs ripr pilot,
then ripr writes a pilot packet and prints the top actionable seam, why it was
flagged, and how to compare before and after after a focused test is added.
```

### Outcome is public CLI

```text
Given before and after repo-exposure snapshots,
when a user runs ripr outcome --before before.json --after after.json,
then ripr reports moved, unchanged, regressed, new, and removed seams without
requiring cargo xtask.
```

### Operator cockpit points at the next command

```text
Given repo-local report artifacts under target/ripr/reports,
when a user runs cargo xtask operator-cockpit,
then ripr writes operator-cockpit.md and operator-cockpit.json with top weak
seams, surface alignment, missing-input commands, and the next targeted-test
loop command.
```

### CI remains advisory

```text
Given a generated GitHub Actions workflow,
when the workflow uploads SARIF,
then it does not fail the pull request unless the repository explicitly opts
into a baseline failure policy.
```

## Test Mapping

Current tests and reports that support the contract:

- `crates/ripr/src/config.rs::tests::generated_init_config_is_conservative_and_parseable`
- `crates/ripr/src/config.rs::tests::generated_init_config_matches_checked_in_example`
- `crates/ripr/src/analysis/workspace/discover.rs::tests::discover_skips_default_excluded_directories`
- `crates/ripr/src/cli/commands.rs::tests::init_parses_root_dry_run_and_force`
- `crates/ripr/src/cli/commands.rs::tests::init_requires_root_value`
- `crates/ripr/src/cli/commands.rs::tests::init_rejects_unknown_arguments`
- `crates/ripr/src/cli/commands.rs::tests::init_generated_github_workflow_is_advisory`
- `crates/ripr/src/cli/commands.rs::tests::init_ci_github_writes_workflow_and_preserves_existing_config`
- `crates/ripr/src/cli/commands.rs::tests::init_ci_github_refuses_existing_workflow_without_force`
- `crates/ripr/tests/cli_smoke.rs::init_writes_conservative_config_and_doctor_loads_it`
- `crates/ripr/tests/cli_smoke.rs::init_dry_run_prints_config_without_writing`
- `crates/ripr/tests/cli_smoke.rs::init_ci_github_dry_run_prints_config_and_workflow_without_writing`
- `crates/ripr/tests/cli_smoke.rs::init_ci_github_writes_non_blocking_sarif_workflow`
- `crates/ripr/tests/cli_smoke.rs::init_ci_github_refuses_existing_workflow_without_force`
- `crates/ripr/tests/cli_smoke.rs::init_refuses_existing_config_without_force`
- `crates/ripr/tests/cli_smoke.rs::init_force_overwrites_existing_config`
- `crates/ripr/src/config.rs::tests::generated_init_config_matches_builtin_defaults`
- `crates/ripr/src/config.rs::tests::missing_config_uses_behavior_preserving_defaults`
- `crates/ripr/src/config.rs::tests::malformed_or_unknown_config_is_actionable`
- `crates/ripr/tests/cli_smoke.rs::doctor_reports_missing_config_defaults`
- `crates/ripr/tests/cli_smoke.rs::doctor_reports_loaded_config_path`
- `crates/ripr/src/analysis/workspace/select.rs::tests::operator_mode_tiers_are_pinned_for_defaults_first_adoption`
- `crates/ripr/src/analysis/workspace/classify.rs::tests::production_path_excludes_repository_automation_fixture_and_non_production_trees`
- `crates/ripr/src/analysis/workspace/select.rs::tests::repo_discovery_skips_fixture_tree_but_fixture_roots_still_work`
- `crates/ripr/src/output/badge.rs::tests::seam_badge_summary_counts_visible_headline_eligible_seams`
- `crates/ripr/src/output/badge.rs::tests::seam_badge_summary_respects_configured_off_severity`
- `crates/ripr/tests/cli_smoke.rs::check_repo_badge_json_emits_repo_scope_metadata`
- `crates/ripr/tests/cli_smoke.rs::check_repo_badge_does_not_consult_diff_arg_when_supplied`
- `crates/ripr/tests/cli_smoke.rs::check_repo_badge_plus_json_emits_repo_scope_metadata`
- `crates/ripr/src/output/pilot.rs::tests::pilot_ranking_prefers_actionable_class_order_before_tie_breakers`
- `crates/ripr/src/output/pilot.rs::tests::pilot_ranking_uses_evidence_tie_breakers_then_stable_location`
- `crates/ripr/src/output/pilot.rs::tests::pilot_ranking_excludes_solved_governed_classes`
- `crates/ripr/src/output/pilot.rs::tests::pilot_summary_json_contains_config_state_artifacts_and_next_commands`
- `crates/ripr/tests/cli_smoke.rs::pilot_writes_default_packet_outputs_for_boundary_gap_fixture`
- `crates/ripr/tests/cli_smoke.rs::pilot_uses_repo_config_mode_without_explicit_flag`
- `crates/ripr/tests/cli_smoke.rs::pilot_honors_explicit_mode_over_repo_config`
- `crates/ripr/src/output/outcome.rs::tests::targeted_test_outcome_report_buckets_seam_movement`
- `crates/ripr/src/output/outcome.rs::tests::targeted_test_outcome_json_and_markdown_are_structured`
- `crates/ripr/src/output/outcome.rs::tests::targeted_test_outcome_from_repo_exposure_json_parses_static_evidence`
- `crates/ripr/tests/cli_smoke.rs::outcome_prints_markdown_receipt_by_default`
- `crates/ripr/tests/cli_smoke.rs::outcome_writes_json_receipt_when_requested`
- `crates/ripr/src/output/mutation_calibration.rs::tests::mutation_calibration_summarizes_static_runtime_agreement`
- `crates/ripr/src/output/mutation_calibration.rs::tests::mutation_calibration_joins_by_seam_id_then_file_line_and_keeps_ambiguous`
- `crates/ripr/src/output/mutation_calibration.rs::tests::mutation_calibration_reports_are_advisory_and_structured`
- `crates/ripr/tests/cli_smoke.rs::calibrate_cargo_mutants_prints_markdown_by_default`
- `crates/ripr/tests/cli_smoke.rs::calibrate_cargo_mutants_writes_json_when_requested`
- `xtask/src/main.rs::tests::targeted_test_outcome_report_buckets_seam_movement`
- `xtask/src/main.rs::tests::targeted_test_outcome_json_and_markdown_are_structured`
- `xtask/src/main.rs::tests::mutation_calibration_summarizes_static_runtime_agreement`
- `xtask/src/main.rs::tests::mutation_calibration_reports_are_advisory_and_structured`
- `xtask/src/main.rs::tests::sarif_policy_missing_baseline_is_advisory_by_default`
- `xtask/src/main.rs::tests::lsp_cockpit_report_json_and_markdown_are_structured`
- `xtask/src/reports/operator.rs::tests::operator_cockpit_reports_missing_inputs_with_commands`
- `xtask/src/reports/operator.rs::tests::operator_cockpit_json_and_markdown_are_structured`
- `editors/vscode/test/suite/extension.test.ts::defaults-first check mode is draft`

## Implementation Mapping

Current implementation pieces:

- `crates/ripr/src/config.rs` owns repo config defaults, validation, and
  precedence, plus the conservative generated `ripr init` config text.
- `crates/ripr/src/analysis/workspace/discover.rs` owns default repo discovery
  exclusions before mode-specific file selection.
- `crates/ripr/src/cli/commands.rs` exposes `init`, `pilot`, `outcome`, `check`,
  `explain`, `context`, `doctor`, and `lsp`; `init --ci github` writes the
  advisory GitHub Actions SARIF workflow.
- `crates/ripr/src/app.rs` orchestrates config-aware analysis entry points.
- `crates/ripr/src/output/agent_seam_packets.rs` renders targeted-test work
  orders.
- `crates/ripr/src/output/pilot.rs` ranks actionable seams and renders the
  pilot summary files.
- `crates/ripr/src/output/outcome.rs` compares before/after repo-exposure
  snapshots and renders the public targeted-test outcome receipt.
- `crates/ripr/src/output/mutation_calibration.rs` imports supplied
  cargo-mutants JSON, joins it to repo-exposure snapshots, and renders the
  public advisory calibration report.
- `editors/vscode/package.json` exposes `ripr.check.mode` with the same
  `draft` default as the CLI and LSP missing-config path.
- `xtask/src/main.rs` currently owns repo-local mutation calibration, LSP
  cockpit, SARIF policy, badge artifact, and report-index automation.
- `xtask/src/reports/operator.rs` joins those report artifacts into
  `operator-cockpit.{json,md}` without changing analyzer behavior.
- `docs/TARGETED_TEST_WORKFLOW.md`, `docs/CI.md`, `docs/CONFIGURATION.md`,
  and `docs/EDITOR_EXTENSION.md` document the current adoption path.

## Metrics

- `time_to_first_actionable_seam`
- `pilot_packet_generated`
- `targeted_test_outcome_available`
- `calibration_import_available`
- `advisory_ci_workflow_generated`
- `operator_cockpit_available`
