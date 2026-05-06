use crate::app::{CheckInput, Mode, OutputFormat};
use crate::config::{
    CheckInputExplicit, DEFAULT_LSP_SEAM_DIAGNOSTICS, RiprConfig, apply_to_check_input,
};
use std::path::Path;
use tower_lsp_server::ls_types::InitializeParams;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LspAnalysisConfig {
    pub(super) base_ref: Option<String>,
    pub(super) mode: Mode,
    pub(super) include_unchanged_tests: bool,
    pub(super) repo_config: RiprConfig,
    /// Enable repo seam evidence diagnostics. The default is bounded to
    /// saved-workspace, draft-mode analysis so the installed editor surface is
    /// useful without requiring `ripr init`.
    pub(super) enable_seam_diagnostics: bool,
}

impl Default for LspAnalysisConfig {
    fn default() -> Self {
        let defaults = CheckInput::default();
        Self {
            base_ref: defaults.base,
            mode: defaults.mode,
            include_unchanged_tests: defaults.include_unchanged_tests,
            repo_config: RiprConfig::default(),
            enable_seam_diagnostics: DEFAULT_LSP_SEAM_DIAGNOSTICS,
        }
    }
}

impl LspAnalysisConfig {
    pub(super) fn from_initialize_params(
        params: &InitializeParams,
        repo_config: RiprConfig,
    ) -> Self {
        let mut config = Self::from_repo_config(repo_config);
        let Some(options) = params.initialization_options.as_ref() else {
            return config;
        };

        if let Some(base_ref) = options
            .get("baseRef")
            .and_then(|value| value.as_str())
            .map(str::trim)
        {
            config.base_ref = if base_ref.is_empty() {
                None
            } else {
                Some(base_ref.to_string())
            };
        }

        if let Some(mode) = options
            .get("checkMode")
            .and_then(|value| value.as_str())
            .and_then(parse_mode)
        {
            config.mode = mode;
        }

        if let Some(include_unchanged_tests) = options
            .get("includeUnchangedTests")
            .and_then(|value| value.as_bool())
        {
            config.include_unchanged_tests = include_unchanged_tests;
        }

        if let Some(enable_seam_diagnostics) = options
            .get("seamDiagnostics")
            .and_then(|value| value.as_bool())
        {
            config.enable_seam_diagnostics = enable_seam_diagnostics;
        }

        config
    }

    fn from_repo_config(repo_config: RiprConfig) -> Self {
        let mut input = CheckInput::default();
        apply_to_check_input(&mut input, &repo_config, CheckInputExplicit::default());
        Self {
            base_ref: input.base,
            mode: input.mode,
            include_unchanged_tests: input.include_unchanged_tests,
            enable_seam_diagnostics: repo_config
                .lsp()
                .seam_diagnostics()
                .unwrap_or(DEFAULT_LSP_SEAM_DIAGNOSTICS),
            repo_config,
        }
    }

    pub(super) fn check_input(&self, root: &Path) -> CheckInput {
        CheckInput {
            root: root.to_path_buf(),
            base: self.base_ref.clone(),
            mode: self.mode.clone(),
            format: OutputFormat::Json,
            include_unchanged_tests: self.include_unchanged_tests,
            ..CheckInput::default()
        }
    }

    pub(super) fn repo_config(&self) -> &RiprConfig {
        &self.repo_config
    }
}

fn parse_mode(value: &str) -> Option<Mode> {
    match value {
        "instant" => Some(Mode::Instant),
        "draft" => Some(Mode::Draft),
        "fast" => Some(Mode::Fast),
        "deep" => Some(Mode::Deep),
        "ready" => Some(Mode::Ready),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tower_lsp_server::ls_types::ClientCapabilities;

    fn params_with(options: serde_json::Value) -> InitializeParams {
        InitializeParams {
            initialization_options: Some(options),
            capabilities: ClientCapabilities::default(),
            ..InitializeParams::default()
        }
    }

    #[test]
    fn seam_diagnostics_defaults_to_true_when_option_is_missing() {
        let params = params_with(json!({}));
        let config = LspAnalysisConfig::from_initialize_params(&params, RiprConfig::default());
        assert!(config.enable_seam_diagnostics);
    }

    #[test]
    fn seam_diagnostics_true_in_init_options_enables_flag() {
        let params = params_with(json!({"seamDiagnostics": true}));
        let config = LspAnalysisConfig::from_initialize_params(&params, RiprConfig::default());
        assert!(config.enable_seam_diagnostics);
    }

    #[test]
    fn seam_diagnostics_false_in_init_options_disables_default() {
        let params = params_with(json!({"seamDiagnostics": false}));
        let config = LspAnalysisConfig::from_initialize_params(&params, RiprConfig::default());
        assert!(!config.enable_seam_diagnostics);
    }

    #[test]
    fn non_boolean_seam_diagnostics_value_is_ignored() {
        let params = params_with(json!({"seamDiagnostics": "yes"}));
        let config = LspAnalysisConfig::from_initialize_params(&params, RiprConfig::default());
        // Falls back to the default rather than misinterpreting a
        // string as truthy.
        assert!(config.enable_seam_diagnostics);
    }

    #[test]
    fn parse_mode_accepts_only_known_literals() {
        let known_modes = [
            ("instant", Mode::Instant),
            ("draft", Mode::Draft),
            ("fast", Mode::Fast),
            ("deep", Mode::Deep),
            ("ready", Mode::Ready),
        ];

        for (literal, expected_mode) in known_modes {
            assert_eq!(parse_mode(literal), Some(expected_mode));
        }

        for unknown in [
            "",
            " Instant",
            "Instant",
            "INSTANT",
            "ready ",
            "deep-mode",
            "0",
            "yes",
        ] {
            assert_eq!(
                parse_mode(unknown),
                None,
                "unexpected parse for {unknown:?}"
            );
        }
    }

    #[test]
    fn lsp_options_property_boolean_fields_match_json_booleans() {
        for include_unchanged_tests in [false, true] {
            for seam_diagnostics in [false, true] {
                let params = params_with(json!({
                    "includeUnchangedTests": include_unchanged_tests,
                    "seamDiagnostics": seam_diagnostics,
                }));
                let config =
                    LspAnalysisConfig::from_initialize_params(&params, RiprConfig::default());
                assert_eq!(config.include_unchanged_tests, include_unchanged_tests);
                assert_eq!(config.enable_seam_diagnostics, seam_diagnostics);
            }
        }
    }

    #[test]
    fn repo_config_sets_defaults_when_initialization_options_are_missing() -> Result<(), String> {
        let repo_config = crate::config::tests_only_parse(
            r#"
[analysis]
mode = "deep"
include_unchanged_tests = false

[lsp]
seam_diagnostics = true
"#,
        )?;
        let params = params_with(json!({}));
        let config = LspAnalysisConfig::from_initialize_params(&params, repo_config);

        assert_eq!(config.mode, Mode::Deep);
        assert!(!config.include_unchanged_tests);
        assert!(config.enable_seam_diagnostics);
        Ok(())
    }

    #[test]
    fn initialization_options_override_repo_config_defaults() -> Result<(), String> {
        let repo_config = crate::config::tests_only_parse(
            r#"
[analysis]
mode = "deep"
include_unchanged_tests = false

[lsp]
seam_diagnostics = true
"#,
        )?;
        let params = params_with(json!({
            "checkMode": "instant",
            "includeUnchangedTests": true,
            "seamDiagnostics": false
        }));
        let config = LspAnalysisConfig::from_initialize_params(&params, repo_config);

        assert_eq!(config.mode, Mode::Instant);
        assert!(config.include_unchanged_tests);
        assert!(!config.enable_seam_diagnostics);
        Ok(())
    }
}
