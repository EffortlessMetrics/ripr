# RIPR-SPEC-0007: Repository Configuration

Status: proposed

## Problem

`ripr` needs a repository-owned configuration surface before SARIF, CI policy,
badge remapping, and editor defaults can become predictable for real
workspaces.

Without a checked-in config file, each entry point has to rely on command-line
flags, editor initialization options, or built-in defaults. That makes CLI,
LSP, reports, and future CI policy harder to keep aligned.

## Behavior

`ripr` should discover and parse the nearest repository-owned `ripr.toml` file
from the selected root.

The configuration layer should:

- walk from the selected root toward its parents for the nearest `ripr.toml`;
- stop after checking a Cargo manifest with a `[workspace]` table or at a
  `.git` boundary; a package-only `Cargo.toml` does not stop discovery;
- use behavior-preserving defaults when the file is absent;
- reject malformed config with actionable errors;
- reject unknown keys so typos do not silently change policy;
- keep explicit CLI options ahead of repository config;
- keep explicit LSP initialization options ahead of repository config;
- allow repository config to set the default analysis mode;
- allow repository config to set whether unchanged tests are included;
- allow repository config to set oracle-strength policy for supported oracle
  shapes;
- allow repository config to set finding and seam diagnostic severity policy;
- allow repository config to point at a relative suppressions file;
- allow repository config to set report related-test caps where the command
  supports that cap;
- make loaded, missing, and malformed config state observable through
  `ripr doctor`;
- provide `ripr config validate [--root PATH]` for isolated ancestor-aware TOML
  validation without workspace tool probes;
- keep output schemas stable unless a later scoped PR explicitly adds config
  metadata.

Precedence is:

```text
explicit CLI or LSP option > ripr.toml > built-in default
```

Scoped amendment (RIPR-SPEC-0136): when the LSP client supports the
server-originated `workspace/configuration` pull model, the five governed LSP
session keys resolve per key as

```text
valid pulled setting > LSP initialization option > ripr.toml > built-in default
```

so initialization options become the compatibility fallback for keys the pull
did not return. In the push-fallback and initialization-only modes the
precedence above is unchanged.

## Required Evidence

Repository configuration evidence should cover:

- absent config preserving previous defaults;
- valid config changing each supported policy surface;
- nested selected roots discovering the nearest config and respecting
  repository boundaries;
- explicit CLI options overriding config values;
- explicit LSP initialization options overriding config values;
- malformed values returning actionable errors;
- unknown keys failing closed;
- unsafe relative-path shapes being rejected where paths are configurable;
- `ripr doctor` reporting loaded config path, missing-config defaults, and
  malformed config errors without printing config source text;
- `ripr doctor` separating installed-binary analysis readiness (the default
  `analysis` profile) from RIPR source-build prerequisites
  (`--profile source-build`), with the Cargo/rustc probes run in the selected
  root;
- `ripr config validate` accepting valid config, following the existing
  missing-config defaults path, and returning path-qualified errors for
  malformed or policy-invalid config;
- output schemas remaining unchanged when severity or report caps come from
  config;
- docs and example config staying aligned with supported keys.

## Non-Goals

This spec does not require:

- SARIF output;
- CI blocking policy;
- badge count remapping;
- user-global config;
- hidden `.ripr/ripr.toml` discovery;
- automatic config migration;
- broad analyzer refactors;
- unsaved-buffer overlays or deep editor analysis by default.

## Acceptance Examples

### Missing config preserves defaults

```text
Given a workspace without ripr.toml,
when ripr check runs,
then the command uses built-in defaults that match the generated conservative
policy profile.
```

### Repository config sets defaults

```text
Given ripr.toml sets analysis.mode = "deep",
when ripr check runs without an explicit mode flag,
then the analysis input uses deep mode.
```

### Explicit CLI options win

```text
Given ripr.toml sets analysis.mode = "deep",
when ripr check runs with --mode fast,
then the analysis input uses fast mode.
```

### Explicit LSP initialization options win

```text
Given ripr.toml enables LSP seam diagnostics,
when the editor sends initializationOptions.seamDiagnostics = false,
then the LSP server keeps seam diagnostics disabled for that session.
```

### Valid pulled LSP settings win over initialization options (pull mode)

```text
Given the LSP client supports workspace/configuration,
and initializationOptions.checkMode = "fast",
when the pulled ripr section returns checkMode = "ready",
then the LSP session uses ready mode,
and initialization options supply only the keys the pull did not return.
```

### Malformed config is actionable

```text
Given ripr.toml contains an unknown key or invalid value,
when ripr loads the config,
then the user-facing error names the config path and parse problem.
```

### Doctor makes config state inspectable

```text
Given ripr doctor runs for a workspace,
when repo config is loaded, missing, or malformed,
then doctor reports the config path or default state and never prints the
config source text.
```

### Doctor separates analysis readiness from source-build prerequisites

```text
Given a Rust workspace whose selected toolchain is missing cargo or rustc, or
whose rustc is older than RIPR's build MSRV,
when ripr doctor runs with the default analysis profile,
then the cargo and rustc checks are reported as advisory, the report status is
pass, and doctor exits 0;
and a missing cargo discloses that evidence read from `cargo metadata` is
withheld.

Given the same workspace,
when ripr doctor runs with --profile source-build,
then the cargo and rustc checks fail, the report status is fail, and doctor
exits 2;
and enabled language runtimes are reported but do not decide that profile's
status.

Given a selected root with its own rustup toolchain selection,
when doctor probes cargo or rustc,
then the probe runs in the selected root rather than the caller's directory.
```

## Test Mapping

Current tests:

- `crates/ripr/src/config.rs::tests::missing_config_uses_behavior_preserving_defaults`
- `crates/ripr/src/config.rs::tests::config_file_sets_core_operational_defaults`
- `crates/ripr/src/config.rs::tests::explicit_cli_mode_wins_over_config_mode`
- `crates/ripr/src/config.rs::tests::config_mode_applies_when_cli_mode_is_not_explicit`
- `crates/ripr/src/config.rs::tests::malformed_or_unknown_config_is_actionable`
- `crates/ripr/src/config.rs::tests::config_rejects_unsafe_suppression_paths`
- `crates/ripr/src/config.rs::tests::oracle_policy_rewrites_configurable_oracle_strengths`
- `crates/ripr/src/lsp/config.rs::tests::repo_config_sets_defaults_when_initialization_options_are_missing`
- `crates/ripr/src/lsp/config.rs::tests::initialization_options_override_repo_config_defaults`
- `crates/ripr/src/lsp/config.rs::tests::pulled_settings_override_initialization_options_for_returned_keys`
- `crates/ripr/src/lsp/config.rs::tests::repository_reload_preserves_pulled_overrides`
- `crates/ripr/src/app.rs::tests::configured_finding_severity_applies_to_human_json_and_github`
- `crates/ripr/src/lsp/diagnostics.rs::seam_diagnostic_tests::configured_seam_severity_can_disable_a_class`
- `crates/ripr/tests/cli_smoke.rs::doctor_reports_missing_config_defaults`
- `crates/ripr/tests/cli_smoke.rs::doctor_reports_loaded_config_path`
- `crates/ripr/tests/cli_smoke.rs::doctor_reports_malformed_config_error`
- `crates/ripr/src/output/doctor.rs::tests::old_workspace_compiler_is_advisory_for_analysis_and_fails_source_build`
- `crates/ripr/src/output/doctor.rs::tests::rust_root_with_missing_cargo_discloses_verification_limitation`
- `crates/ripr/src/output/doctor.rs::tests::doctor_cargo_probe_uses_selected_root`
- `crates/ripr/src/cli/commands/doctor.rs::tests::source_build_profile_keeps_enabled_language_runtime_failure_advisory`
- `crates/ripr/tests/cli_smoke.rs::doctor_discloses_missing_verification_tools_but_still_checks_manifest`
- `crates/ripr/tests/cli_help_hierarchy.rs::doctor_exit_code_guide_distinguishes_analysis_and_source_build`

Planned tests:

- fixture-backed config examples once SARIF and CI policy consume the config
  surface;
- extension smoke coverage for editor settings flowing through LSP session
  config.

## Implementation Mapping

Current implementation:

- `crates/ripr/src/config.rs` owns config parsing, defaults, validation, and
  precedence helpers.
- `crates/ripr/src/cli/commands.rs` loads repo config for `check`, `explain`,
  and `context`, and reports config status through `doctor`.
- `crates/ripr/src/app.rs` provides config-aware orchestration for CLI and LSP
  adapters.
- `crates/ripr/src/lsp/config.rs` merges repo config with LSP initialization
  options.
- `crates/ripr/src/output/human.rs`, `crates/ripr/src/output/human/`,
  `crates/ripr/src/output/json/report.rs`, and
  `crates/ripr/src/output/github.rs` apply configured finding severity.
- `crates/ripr/src/output/suppressions.rs` loads the configured suppressions
  path for badge reports.
- `ripr.toml.example` documents the supported v1 shape.

## Metrics

- `repository_config`
