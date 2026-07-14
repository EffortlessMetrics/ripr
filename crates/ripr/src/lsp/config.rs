use crate::app::{CheckInput, Mode, OutputFormat};
use crate::config::{
    CheckInputExplicit, DEFAULT_LSP_SEAM_DIAGNOSTICS, LspDiagnosticProfile, RiprConfig,
    apply_to_check_input,
};
use serde_json::Value;
use std::path::Path;
use tower_lsp_server::ls_types::InitializeParams;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LspAnalysisConfig {
    pub(super) base_ref: Option<String>,
    pub(super) mode: Mode,
    pub(super) include_unchanged_tests: bool,
    pub(super) repo_config: RiprConfig,
    /// Session overrides are retained so a repository configuration reload
    /// preserves the initialization precedence contract. This is a bounded
    /// projection of the four supported LSP options, not an authority for
    /// arbitrary client settings.
    pub(super) session_options: Option<Value>,
    /// Enable repo seam evidence diagnostics. The default is bounded to
    /// saved-workspace, draft-mode analysis so the installed editor surface is
    /// useful with no `ripr.toml` and without running `ripr init`.
    pub(super) enable_seam_diagnostics: bool,
    /// Defaults to `actionable`; valid initialization options override the
    /// repository setting, while unknown options retain that resolved value.
    pub(super) diagnostic_profile: LspDiagnosticProfile,
}

impl Default for LspAnalysisConfig {
    fn default() -> Self {
        let defaults = CheckInput::default();
        Self {
            base_ref: defaults.base,
            mode: defaults.mode,
            include_unchanged_tests: defaults.include_unchanged_tests,
            repo_config: RiprConfig::default(),
            session_options: None,
            enable_seam_diagnostics: DEFAULT_LSP_SEAM_DIAGNOSTICS,
            diagnostic_profile: LspDiagnosticProfile::default(),
        }
    }
}

impl LspAnalysisConfig {
    pub(super) fn from_initialize_params(
        params: &InitializeParams,
        repo_config: RiprConfig,
    ) -> Self {
        Self::from_repo_config_and_options(repo_config, params.initialization_options.as_ref())
    }

    pub(super) fn from_repo_config_and_options(
        repo_config: RiprConfig,
        options: Option<&Value>,
    ) -> Self {
        let mut config = Self::from_repo_config(repo_config);
        let Some(options) = options.and_then(session_options_object) else {
            return config;
        };
        let options = supported_session_options(options);
        if options.is_empty() {
            return config;
        }
        config.session_options = Some(Value::Object(options.clone()));
        apply_session_options(&mut config, &options);
        if let Some(profile) = options
            .get("diagnosticProfile")
            .and_then(|value| value.as_str())
            .and_then(|value| LspDiagnosticProfile::parse(value).ok())
        {
            config.diagnostic_profile = profile;
        }
        config
    }

    pub(super) fn with_changed_session_options(&self, settings: &Value) -> Option<Self> {
        let changed = session_options_object(settings)?;
        let mut merged = self
            .session_options
            .as_ref()
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (key, value) in changed {
            if !is_supported_session_option(key) {
                continue;
            }
            if value.is_null() {
                merged.remove(key);
            } else {
                merged.insert(key.clone(), value.clone());
            }
        }
        Some(Self::from_repo_config_and_options(
            self.repo_config.clone(),
            Some(&Value::Object(merged)),
        ))
    }

    pub(super) fn reload_repo_config(&self, repo_config: RiprConfig) -> Self {
        Self::from_repo_config_and_options(repo_config, self.session_options.as_ref())
    }

    pub(super) fn has_session_option_changes(settings: &Value) -> bool {
        session_options_object(settings)
            .is_some_and(|options| options.keys().any(|key| is_supported_session_option(key)))
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
            diagnostic_profile: repo_config.lsp().diagnostic_profile().unwrap_or_default(),
            repo_config,
            session_options: None,
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

fn session_options_object(value: &Value) -> Option<&serde_json::Map<String, Value>> {
    let object = value.as_object()?;
    object
        .get("ripr")
        .and_then(Value::as_object)
        .or(Some(object))
}

fn apply_session_options(config: &mut LspAnalysisConfig, options: &serde_json::Map<String, Value>) {
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

    if let Some(profile) = options
        .get("diagnosticProfile")
        .and_then(|value| value.as_str())
        .and_then(|value| LspDiagnosticProfile::parse(value).ok())
    {
        config.diagnostic_profile = profile;
    }
}

fn supported_session_options(
    options: &serde_json::Map<String, Value>,
) -> serde_json::Map<String, Value> {
    options
        .iter()
        .filter(|(key, _)| is_supported_session_option(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn is_supported_session_option(key: &str) -> bool {
    matches!(
        key,
        "baseRef" | "checkMode" | "includeUnchangedTests" | "seamDiagnostics" | "diagnosticProfile"
    )
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
        assert_eq!(config.diagnostic_profile, LspDiagnosticProfile::Actionable);
    }

    #[test]
    fn diagnostic_profile_init_option_selects_full_visibility() {
        let params = params_with(json!({"diagnosticProfile": "full"}));
        let config = LspAnalysisConfig::from_initialize_params(&params, RiprConfig::default());
        assert_eq!(config.diagnostic_profile, LspDiagnosticProfile::Full);
    }

    #[test]
    fn unknown_diagnostic_profile_init_option_keeps_the_default() {
        let params = params_with(json!({"diagnosticProfile": "unknown"}));
        let config = LspAnalysisConfig::from_initialize_params(&params, RiprConfig::default());
        assert_eq!(config.diagnostic_profile, LspDiagnosticProfile::Actionable);
    }

    #[test]
    fn invalid_repo_diagnostic_profile_is_rejected() {
        let error = match crate::config::tests_only_parse(
            r#"
[lsp]
diagnostic_profile = "quiet"
"#,
        ) {
            Ok(_) => "invalid diagnostic profile was accepted".to_owned(),
            Err(error) => error,
        };
        assert!(error.contains("diagnostic_profile"));
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
diagnostic_profile = "full"
"#,
        )?;
        let params = params_with(json!({}));
        let config = LspAnalysisConfig::from_initialize_params(&params, repo_config);

        assert_eq!(config.mode, Mode::Deep);
        assert!(!config.include_unchanged_tests);
        assert!(config.enable_seam_diagnostics);
        assert_eq!(config.diagnostic_profile, LspDiagnosticProfile::Full);
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

    #[test]
    fn repository_reload_preserves_initialization_overrides() -> Result<(), String> {
        let initial_repo = crate::config::tests_only_parse(
            r#"
[analysis]
mode = "deep"
include_unchanged_tests = false
"#,
        )?;
        let config = LspAnalysisConfig::from_repo_config_and_options(
            initial_repo,
            Some(&json!({
                "baseRef": "origin/release",
                "checkMode": "fast",
                "includeUnchangedTests": true,
            })),
        );

        let reloaded_repo = crate::config::tests_only_parse(
            r#"
[analysis]
mode = "ready"
include_unchanged_tests = false
"#,
        )?;
        let reloaded = config.reload_repo_config(reloaded_repo);

        assert_eq!(reloaded.base_ref.as_deref(), Some("origin/release"));
        assert_eq!(reloaded.mode, Mode::Fast);
        assert!(reloaded.include_unchanged_tests);
        Ok(())
    }

    #[test]
    fn configuration_settings_merge_with_existing_session_options() -> Result<(), String> {
        let config = LspAnalysisConfig::from_repo_config_and_options(
            RiprConfig::default(),
            Some(&json!({"baseRef": "origin/main", "checkMode": "deep"})),
        );
        let Some(changed) =
            config.with_changed_session_options(&json!({"ripr": {"checkMode": "fast"}}))
        else {
            return Err("object settings should produce a config".to_string());
        };

        assert_eq!(changed.base_ref.as_deref(), Some("origin/main"));
        assert_eq!(changed.mode, Mode::Fast);
        Ok(())
    }

    #[test]
    fn unrelated_configuration_settings_do_not_trigger_session_reload() {
        assert!(!LspAnalysisConfig::has_session_option_changes(
            &json!({"editor": {"formatOnSave": true}})
        ));
    }
}
