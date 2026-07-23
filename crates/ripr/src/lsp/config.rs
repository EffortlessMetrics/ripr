use crate::app::{CheckInput, Mode, OutputFormat};
use crate::config::{
    CheckInputExplicit, DEFAULT_LSP_SEAM_DIAGNOSTICS, LspDiagnosticProfile, RiprConfig,
    apply_to_check_input,
};
use serde_json::Value;
use std::path::Path;
use tower_lsp_server::ls_types::{InitializeParams, PositionEncodingKind};

use super::client_features::ClientFeatureProfile;

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
    /// Validated settings pulled from the client via `workspace/configuration`
    /// (#2031, RIPR-SPEC-0136). Retained as a distinct layer — like
    /// `session_options` — so a repository configuration reload re-applies the
    /// pull precedence contract: pulled values win over initialization options
    /// for the keys the pull returned; initialization options remain the
    /// compatibility fallback for keys the pull did not return.
    pub(super) pulled_options: Option<Value>,
    /// Enable repo seam evidence diagnostics. The default is bounded to
    /// saved-workspace, draft-mode analysis so the installed editor surface is
    /// useful with no `ripr.toml` and without running `ripr init`.
    pub(super) enable_seam_diagnostics: bool,
    /// Defaults to `actionable`; valid initialization options override the
    /// repository setting, while unknown options retain that resolved value.
    pub(super) diagnostic_profile: LspDiagnosticProfile,
    /// The position encoding negotiated at `initialize` from the client's
    /// `general.positionEncodings` (#1626 PR B / #1749). Defaults to UTF-16 and
    /// is preserved across repository-config and session-option reloads.
    pub(super) position_encoding: PositionEncodingKind,
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
            pulled_options: None,
            enable_seam_diagnostics: DEFAULT_LSP_SEAM_DIAGNOSTICS,
            diagnostic_profile: LspDiagnosticProfile::default(),
            position_encoding: PositionEncodingKind::UTF16,
        }
    }
}

impl LspAnalysisConfig {
    /// Build the session config from initialization options plus the typed
    /// client-feature profile (#1987, RIPR-SPEC-0143). The profile is parsed
    /// once at `initialize`; the position encoding is read from it rather
    /// than re-negotiated here.
    pub(super) fn from_initialize_params(
        params: &InitializeParams,
        repo_config: RiprConfig,
        profile: &ClientFeatureProfile,
    ) -> Self {
        let mut config =
            Self::from_repo_config_and_options(repo_config, params.initialization_options.as_ref());
        config.position_encoding = profile.selected_position_encoding.clone();
        config
    }

    pub(super) fn from_repo_config_and_options(
        repo_config: RiprConfig,
        options: Option<&Value>,
    ) -> Self {
        Self::from_repo_config_and_layers(repo_config, options, None)
    }

    /// Resolve the effective config from the three retained layers
    /// (RIPR-SPEC-0136): repository config defaults, then initialization
    /// options for the keys the pull did not return, then validated pulled
    /// settings for the keys the pull returned.
    fn from_repo_config_and_layers(
        repo_config: RiprConfig,
        options: Option<&Value>,
        pulled: Option<&Value>,
    ) -> Self {
        let mut config = Self::from_repo_config(repo_config);
        let session = options
            .and_then(session_options_object)
            .map(supported_session_options)
            .filter(|options| !options.is_empty());
        let pulled = pulled
            .and_then(Value::as_object)
            .map(supported_session_options)
            .filter(|options| !options.is_empty());
        if session.is_none() && pulled.is_none() {
            return config;
        }
        let mut effective_session = session.clone().unwrap_or_default();
        if let Some(pulled) = &pulled {
            for key in pulled.keys() {
                effective_session.remove(key);
            }
        }
        config.session_options = session.map(Value::Object);
        config.pulled_options = pulled.clone().map(Value::Object);
        apply_session_options(&mut config, &effective_session);
        if let Some(pulled) = &pulled {
            apply_session_options(&mut config, pulled);
        }
        config
    }

    /// Rebuild with a new validated pulled layer (#2031). The pulled layer
    /// replaces the retained one wholesale: a key absent from the latest pull
    /// falls back to initialization options or repository defaults.
    pub(super) fn with_pulled_options(&self, pulled: Option<&Value>) -> Self {
        let mut next = Self::from_repo_config_and_layers(
            self.repo_config.clone(),
            self.session_options.as_ref(),
            pulled,
        );
        next.position_encoding = self.position_encoding.clone();
        next
    }

    /// Whether two configs agree on the effective analysis settings. Used by
    /// the pull-apply no-op guard so a re-pull whose validated values do not
    /// change the effective settings does not reschedule analysis (#2031).
    pub(super) fn effective_settings_eq(&self, other: &Self) -> bool {
        self.base_ref == other.base_ref
            && self.mode == other.mode
            && self.include_unchanged_tests == other.include_unchanged_tests
            && self.enable_seam_diagnostics == other.enable_seam_diagnostics
            && self.diagnostic_profile == other.diagnostic_profile
    }

    /// Per-field source disclosure for the five governed session keys
    /// (`pulled` | `initialization` | `repo` | `default`), surfaced in the
    /// analysis status payload so defaults never masquerade as accepted
    /// requested settings (#2031).
    ///
    /// Known limitation: `seam_diagnostics` is attributed `repo` whenever a
    /// `ripr.toml` was loaded, because the repository default and the
    /// built-in default coincide and cannot be distinguished after parsing.
    pub(super) fn session_value_sources(&self) -> serde_json::Map<String, Value> {
        let session = self.session_options.as_ref().and_then(Value::as_object);
        let pulled = self.pulled_options.as_ref().and_then(Value::as_object);
        let repo_config = self.repo_config();
        let entries: [(&str, &str, bool); 5] = [
            ("base_ref", "baseRef", false),
            (
                "check_mode",
                "checkMode",
                repo_config.analysis().mode().is_some(),
            ),
            (
                "include_unchanged_tests",
                "includeUnchangedTests",
                repo_config.analysis().include_unchanged_tests().is_some(),
            ),
            (
                "seam_diagnostics",
                "seamDiagnostics",
                repo_config.source_path().is_some()
                    && repo_config.lsp().seam_diagnostics().is_some(),
            ),
            (
                "diagnostic_profile",
                "diagnosticProfile",
                repo_config.lsp().diagnostic_profile().is_some(),
            ),
        ];
        let mut sources = serde_json::Map::new();
        for (payload_key, option_key, repo_explicit) in entries {
            let source = if pulled.is_some_and(|options| options.contains_key(option_key)) {
                "pulled"
            } else if session.is_some_and(|options| options.contains_key(option_key)) {
                "initialization"
            } else if repo_explicit {
                "repo"
            } else {
                "default"
            };
            sources.insert(payload_key.to_string(), Value::String(source.to_string()));
        }
        sources
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
        let mut next = Self::from_repo_config_and_layers(
            self.repo_config.clone(),
            Some(&Value::Object(merged)),
            self.pulled_options.as_ref(),
        );
        next.position_encoding = self.position_encoding.clone();
        Some(next)
    }

    pub(super) fn reload_repo_config(&self, repo_config: RiprConfig) -> Self {
        let mut next = Self::from_repo_config_and_layers(
            repo_config,
            self.session_options.as_ref(),
            self.pulled_options.as_ref(),
        );
        next.position_encoding = self.position_encoding.clone();
        next
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
            pulled_options: None,
            position_encoding: PositionEncodingKind::UTF16,
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

/// Validate one `workspace/configuration` response for the bounded `ripr`
/// section before anything is applied (#2031, RIPR-SPEC-0136).
///
/// The response must be an array of exactly one item matching the single
/// requested `ConfigurationItem`. A `null` item means the client holds no
/// `ripr` settings and yields no pulled layer. An object item is checked
/// key by key: supported keys with the wrong JSON type or an unknown enum
/// literal fail the whole pull (fail-closed — a silently ignored requested
/// setting would masquerade as accepted), while unsupported keys are outside
/// the governed section and ignored.
pub(super) fn validated_pulled_options(values: &[Value]) -> Result<Option<Value>, String> {
    if values.len() != 1 {
        return Err(format!(
            "workspace/configuration returned {} items for one requested `ripr` section",
            values.len()
        ));
    }
    let Some(item) = values.first() else {
        return Err("workspace/configuration returned no items".to_string());
    };
    if item.is_null() {
        return Ok(None);
    }
    let Some(object) = item.as_object() else {
        return Err(format!(
            "workspace/configuration item for the `ripr` section must be an object or null, got {item}"
        ));
    };
    for (key, value) in object {
        if !is_supported_session_option(key) {
            continue;
        }
        validate_pulled_value(key, value)?;
    }
    Ok(Some(Value::Object(supported_session_options(object))))
}

/// Pulled settings arrive outside the initialization-options ingress bound
/// (#2034), so each value is bounded here: a transport-sized string must
/// fail validation instead of being stored and re-rendered (#2211 review).
const MAX_PULLED_VALUE_BYTES: usize = 4096;

fn validate_pulled_value(key: &str, value: &Value) -> Result<(), String> {
    if value
        .as_str()
        .is_some_and(|text| text.len() > MAX_PULLED_VALUE_BYTES)
    {
        return Err(format!(
            "workspace/configuration value for `{key}` exceeds the {MAX_PULLED_VALUE_BYTES}-byte pulled-value bound"
        ));
    }
    let valid = match key {
        "baseRef" => value.as_str().is_some(),
        "checkMode" => value
            .as_str()
            .is_some_and(|literal| parse_mode(literal).is_some()),
        "includeUnchangedTests" | "seamDiagnostics" => value.as_bool().is_some(),
        "diagnosticProfile" => value
            .as_str()
            .is_some_and(|literal| LspDiagnosticProfile::parse(literal).is_ok()),
        _ => true,
    };
    if valid {
        return Ok(());
    }
    Err(format!(
        "workspace/configuration value for `{key}` has an invalid type or literal: {value}"
    ))
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

    /// Test helper mirroring the production flow: the profile is parsed once
    /// from the params and passed into the config build.
    fn config_from_params(params: &InitializeParams, repo_config: RiprConfig) -> LspAnalysisConfig {
        LspAnalysisConfig::from_initialize_params(
            params,
            repo_config,
            &ClientFeatureProfile::from_initialize_params(params),
        )
    }

    #[test]
    fn seam_diagnostics_defaults_to_true_when_option_is_missing() {
        let params = params_with(json!({}));
        let config = config_from_params(&params, RiprConfig::default());
        assert!(config.enable_seam_diagnostics);
        assert_eq!(config.diagnostic_profile, LspDiagnosticProfile::Actionable);
    }

    #[test]
    fn diagnostic_profile_init_option_selects_full_visibility() {
        let params = params_with(json!({"diagnosticProfile": "full"}));
        let config = config_from_params(&params, RiprConfig::default());
        assert_eq!(config.diagnostic_profile, LspDiagnosticProfile::Full);
    }

    #[test]
    fn position_encoding_survives_repo_reload_and_session_option_changes() -> Result<(), String> {
        let config = LspAnalysisConfig {
            position_encoding: PositionEncodingKind::UTF8,
            ..LspAnalysisConfig::default()
        };

        let reloaded = config.reload_repo_config(RiprConfig::default());
        if reloaded.position_encoding != PositionEncodingKind::UTF8 {
            return Err("reload_repo_config dropped the negotiated position encoding".to_string());
        }

        let with_options = config
            .with_changed_session_options(&json!({ "diagnosticProfile": "full" }))
            .ok_or_else(|| "session option change should rebuild the config".to_string())?;
        if with_options.position_encoding != PositionEncodingKind::UTF8 {
            return Err(
                "with_changed_session_options dropped the negotiated position encoding".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn unknown_diagnostic_profile_init_option_keeps_the_default() {
        let params = params_with(json!({"diagnosticProfile": "unknown"}));
        let config = config_from_params(&params, RiprConfig::default());
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
        let config = config_from_params(&params, RiprConfig::default());
        assert!(config.enable_seam_diagnostics);
    }

    #[test]
    fn seam_diagnostics_false_in_init_options_disables_default() {
        let params = params_with(json!({"seamDiagnostics": false}));
        let config = config_from_params(&params, RiprConfig::default());
        assert!(!config.enable_seam_diagnostics);
    }

    #[test]
    fn non_boolean_seam_diagnostics_value_is_ignored() {
        let params = params_with(json!({"seamDiagnostics": "yes"}));
        let config = config_from_params(&params, RiprConfig::default());
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
                let config = config_from_params(&params, RiprConfig::default());
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
        let config = config_from_params(&params, repo_config);

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
        let config = config_from_params(&params, repo_config);

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

    #[test]
    fn validated_pulled_options_rejects_wrong_item_count() -> Result<(), String> {
        for values in [vec![], vec![json!({}), json!({})]] {
            if validated_pulled_options(&values).is_ok() {
                return Err(format!("wrong item count must fail closed: {values:?}"));
            }
        }
        Ok(())
    }

    #[test]
    fn validated_pulled_options_rejects_oversized_string_value() -> Result<(), String> {
        let oversized = "x".repeat(MAX_PULLED_VALUE_BYTES + 1);
        let values = vec![json!({ "baseRef": oversized })];
        if validated_pulled_options(&values).is_ok() {
            return Err("an oversized pulled string must fail closed".to_string());
        }
        Ok(())
    }

    #[test]
    fn validated_pulled_options_accepts_null_item_as_no_settings() -> Result<(), String> {
        if validated_pulled_options(&[Value::Null])?.is_some() {
            return Err("a null item must yield no pulled layer".to_string());
        }
        Ok(())
    }

    #[test]
    fn validated_pulled_options_rejects_non_object_item_and_bad_values() -> Result<(), String> {
        for values in [
            vec![json!("ripr")],
            vec![json!({"checkMode": "turbo"})],
            vec![json!({"checkMode": 42})],
            vec![json!({"seamDiagnostics": "yes"})],
            vec![json!({"includeUnchangedTests": 1})],
            vec![json!({"baseRef": null})],
            vec![json!({"diagnosticProfile": "quiet"})],
        ] {
            if validated_pulled_options(&values).is_ok() {
                return Err(format!("malformed pull must fail closed: {values:?}"));
            }
        }
        Ok(())
    }

    #[test]
    fn validated_pulled_options_keeps_supported_keys_and_ignores_extras() -> Result<(), String> {
        let pulled = validated_pulled_options(&[json!({
            "checkMode": "fast",
            "seamDiagnostics": false,
            "editor": {"formatOnSave": true}
        })])?
        .ok_or_else(|| "expected a pulled layer".to_string())?;
        assert_eq!(
            pulled,
            json!({"checkMode": "fast", "seamDiagnostics": false})
        );
        Ok(())
    }

    #[test]
    fn pulled_settings_override_initialization_options_for_returned_keys() -> Result<(), String> {
        let repo_config = crate::config::tests_only_parse(
            r#"
[analysis]
mode = "deep"
include_unchanged_tests = false
"#,
        )?;
        let config = LspAnalysisConfig::from_repo_config_and_options(
            repo_config,
            Some(&json!({"checkMode": "instant", "baseRef": "origin/init"})),
        )
        .with_pulled_options(Some(&json!({"checkMode": "ready"})));

        // The pulled key wins over the initialization option...
        assert_eq!(config.mode, Mode::Ready);
        // ...while keys the pull did not return fall back to initialization
        // options, then repository defaults.
        assert_eq!(config.base_ref.as_deref(), Some("origin/init"));
        assert!(!config.include_unchanged_tests);
        assert!(config.enable_seam_diagnostics);
        Ok(())
    }

    #[test]
    fn empty_pull_layer_restores_initialization_and_repo_precedence() -> Result<(), String> {
        let config = LspAnalysisConfig::from_repo_config_and_options(
            RiprConfig::default(),
            Some(&json!({"checkMode": "fast"})),
        )
        .with_pulled_options(Some(&json!({"checkMode": "deep"})));
        assert_eq!(config.mode, Mode::Deep);

        let cleared = config.with_pulled_options(None);
        assert_eq!(cleared.mode, Mode::Fast);
        if cleared.pulled_options.is_some() {
            return Err("an empty pull must clear the retained pulled layer".to_string());
        }
        Ok(())
    }

    #[test]
    fn repository_reload_preserves_pulled_overrides() -> Result<(), String> {
        let config = LspAnalysisConfig::from_repo_config_and_options(
            RiprConfig::default(),
            Some(&json!({"checkMode": "instant", "baseRef": "origin/init"})),
        )
        .with_pulled_options(Some(&json!({"checkMode": "ready"})));

        let reloaded_repo = crate::config::tests_only_parse(
            r#"
[analysis]
mode = "draft"
include_unchanged_tests = false
"#,
        )?;
        let reloaded = config.reload_repo_config(reloaded_repo);

        assert_eq!(reloaded.mode, Mode::Ready);
        assert_eq!(reloaded.base_ref.as_deref(), Some("origin/init"));
        assert!(!reloaded.include_unchanged_tests);
        Ok(())
    }

    #[test]
    fn session_option_change_preserves_pulled_layer() -> Result<(), String> {
        let config = LspAnalysisConfig::from_repo_config_and_options(
            RiprConfig::default(),
            Some(&json!({"checkMode": "instant"})),
        )
        .with_pulled_options(Some(&json!({"checkMode": "deep"})));
        let Some(changed) = config.with_changed_session_options(&json!({"checkMode": "fast"}))
        else {
            return Err("session option change should rebuild the config".to_string());
        };
        // The pulled layer still wins for checkMode even though the pushed
        // session option changed.
        assert_eq!(changed.mode, Mode::Deep);
        Ok(())
    }

    #[test]
    fn effective_settings_eq_ignores_layer_representation() -> Result<(), String> {
        let from_repo = LspAnalysisConfig::from_repo_config_and_options(
            crate::config::tests_only_parse("[analysis]\nmode = \"fast\"\n")?,
            None,
        );
        let from_pull =
            LspAnalysisConfig::from_repo_config_and_options(RiprConfig::default(), None)
                .with_pulled_options(Some(&json!({"checkMode": "fast"})));
        if from_repo == from_pull {
            return Err("distinct retained layers must remain distinguishable".to_string());
        }
        if !from_repo.effective_settings_eq(&from_pull) {
            return Err("same effective settings must compare equal".to_string());
        }
        let different = from_pull.with_pulled_options(Some(&json!({"checkMode": "deep"})));
        if from_pull.effective_settings_eq(&different) {
            return Err("different effective settings must compare unequal".to_string());
        }
        Ok(())
    }

    #[test]
    fn session_value_sources_disclose_per_field_origin() -> Result<(), String> {
        let repo_config = crate::config::tests_only_parse(
            r#"
[analysis]
mode = "deep"

[lsp]
diagnostic_profile = "full"
"#,
        )?;
        let config = LspAnalysisConfig::from_repo_config_and_options(
            repo_config,
            Some(&json!({"baseRef": "origin/init"})),
        )
        .with_pulled_options(Some(&json!({"seamDiagnostics": false})));
        let sources = config.session_value_sources();
        assert_eq!(
            sources.get("base_ref").and_then(Value::as_str),
            Some("initialization")
        );
        assert_eq!(
            sources.get("check_mode").and_then(Value::as_str),
            Some("repo")
        );
        assert_eq!(
            sources.get("seam_diagnostics").and_then(Value::as_str),
            Some("pulled")
        );
        assert_eq!(
            sources.get("diagnostic_profile").and_then(Value::as_str),
            Some("repo")
        );
        assert_eq!(
            sources
                .get("include_unchanged_tests")
                .and_then(Value::as_str),
            Some("default")
        );
        Ok(())
    }
}
