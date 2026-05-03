use crate::app::{CheckInput, Mode, OutputFormat};
use std::path::Path;
use tower_lsp_server::ls_types::InitializeParams;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LspAnalysisConfig {
    pub(super) base_ref: Option<String>,
    pub(super) mode: Mode,
    pub(super) include_unchanged_tests: bool,
}

impl Default for LspAnalysisConfig {
    fn default() -> Self {
        let defaults = CheckInput::default();
        Self {
            base_ref: defaults.base,
            mode: defaults.mode,
            include_unchanged_tests: defaults.include_unchanged_tests,
        }
    }
}

impl LspAnalysisConfig {
    pub(super) fn from_initialize_params(params: &InitializeParams) -> Self {
        let mut config = Self::default();
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

        config
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
