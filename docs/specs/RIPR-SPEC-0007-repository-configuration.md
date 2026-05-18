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

`ripr` should discover and parse a repo-root `ripr.toml` file.

The configuration layer should:

- look for `ripr.toml` at the workspace root;
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
- keep output schemas stable unless a later scoped PR explicitly adds config
  metadata.

Precedence is:

```text
explicit CLI or LSP option > ripr.toml > built-in default
```

## Required Evidence

Repository configuration evidence should cover:

- absent config preserving previous defaults;
- valid config changing each supported policy surface;
- explicit CLI options overriding config values;
- explicit LSP initialization options overriding config values;
- malformed values returning actionable errors;
- unknown keys failing closed;
- unsafe relative-path shapes being rejected where paths are configurable;
- `ripr doctor` reporting loaded config path, missing-config defaults, and
  malformed config errors without printing config source text;
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
- `crates/ripr/src/app.rs::tests::configured_finding_severity_applies_to_human_json_and_github`
- `crates/ripr/src/lsp/diagnostics.rs::seam_diagnostic_tests::configured_seam_severity_can_disable_a_class`
- `crates/ripr/tests/cli_smoke.rs::doctor_reports_missing_config_defaults`
- `crates/ripr/tests/cli_smoke.rs::doctor_reports_loaded_config_path`
- `crates/ripr/tests/cli_smoke.rs::doctor_reports_malformed_config_error`

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
