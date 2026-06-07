//! Pure configuration data types, defaults, and accessors.

use crate::analysis::seams::SeamGripClass;
use crate::app::Mode;
use crate::domain::{ExposureClass, LanguageId, OracleStrength};
use std::path::{Path, PathBuf};

use super::{
    DEFAULT_CONTEXT_RELATED_TESTS, DEFAULT_LSP_SEAM_DIAGNOSTICS, DEFAULT_SUPPRESSIONS_PATH,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RiprConfig {
    pub(super) analysis: AnalysisConfig,
    pub(super) oracles: OraclePolicy,
    pub(super) severity: SeverityConfig,
    pub(super) lsp: LspConfig,
    pub(super) reports: ReportsConfig,
    pub(super) suppressions: SuppressionsConfig,
    pub(super) languages: LanguagesConfig,
    pub(super) profiles: ProfilesConfig,
    pub(super) source_path: Option<PathBuf>,
    pub(super) source_text: Option<String>,
}

impl RiprConfig {
    pub(crate) fn analysis(&self) -> &AnalysisConfig {
        &self.analysis
    }

    pub(crate) fn oracles(&self) -> &OraclePolicy {
        &self.oracles
    }

    pub(crate) fn severity(&self) -> &SeverityConfig {
        &self.severity
    }

    pub(crate) fn lsp(&self) -> &LspConfig {
        &self.lsp
    }

    pub(crate) fn reports(&self) -> &ReportsConfig {
        &self.reports
    }

    pub(crate) fn suppressions(&self) -> &SuppressionsConfig {
        &self.suppressions
    }

    pub(crate) fn languages(&self) -> &LanguagesConfig {
        &self.languages
    }

    pub(crate) fn profiles(&self) -> &ProfilesConfig {
        &self.profiles
    }

    pub(crate) fn source_text(&self) -> Option<&str> {
        self.source_text.as_deref()
    }

    pub(crate) fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AnalysisConfig {
    pub(super) mode: Option<Mode>,
    pub(super) include_unchanged_tests: Option<bool>,
}

impl AnalysisConfig {
    pub(crate) fn mode(&self) -> Option<&Mode> {
        self.mode.as_ref()
    }

    pub(crate) fn include_unchanged_tests(&self) -> Option<bool> {
        self.include_unchanged_tests
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OraclePolicy {
    pub(super) snapshot_strength: OracleStrength,
    pub(super) mock_expectation_strength: OracleStrength,
    pub(super) broad_error_strength: OracleStrength,
}

impl Default for OraclePolicy {
    fn default() -> Self {
        Self {
            snapshot_strength: OracleStrength::Medium,
            mock_expectation_strength: OracleStrength::Medium,
            broad_error_strength: OracleStrength::Weak,
        }
    }
}

impl OraclePolicy {
    pub(crate) fn strength_for_kind(
        &self,
        kind: &crate::domain::OracleKind,
        current: OracleStrength,
    ) -> OracleStrength {
        match kind {
            crate::domain::OracleKind::Snapshot => self.snapshot_strength.clone(),
            crate::domain::OracleKind::MockExpectation => self.mock_expectation_strength.clone(),
            crate::domain::OracleKind::BroadError => self.broad_error_strength.clone(),
            _ => current,
        }
    }

    #[cfg(test)]
    pub(crate) fn snapshot_strength(&self) -> &OracleStrength {
        &self.snapshot_strength
    }

    #[cfg(test)]
    pub(crate) fn mock_expectation_strength(&self) -> &OracleStrength {
        &self.mock_expectation_strength
    }

    #[cfg(test)]
    pub(crate) fn broad_error_strength(&self) -> &OracleStrength {
        &self.broad_error_strength
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LspConfig {
    pub(super) seam_diagnostics: Option<bool>,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            seam_diagnostics: Some(DEFAULT_LSP_SEAM_DIAGNOSTICS),
        }
    }
}

impl LspConfig {
    pub(crate) fn seam_diagnostics(&self) -> Option<bool> {
        self.seam_diagnostics
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReportsConfig {
    pub(super) max_related_tests: usize,
}

impl Default for ReportsConfig {
    fn default() -> Self {
        Self {
            max_related_tests: DEFAULT_CONTEXT_RELATED_TESTS,
        }
    }
}

impl ReportsConfig {
    pub(crate) fn max_related_tests(&self) -> usize {
        self.max_related_tests
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SuppressionsConfig {
    pub(super) path: PathBuf,
}

impl Default for SuppressionsConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from(DEFAULT_SUPPRESSIONS_PATH),
        }
    }
}

impl SuppressionsConfig {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn display_path(&self) -> String {
        self.path.to_string_lossy().replace('\\', "/")
    }
}

/// `[languages]` repository configuration per RIPR-SPEC-0026.
///
/// `enabled` is the ordered list of source languages the analysis pipeline
/// will dispatch to. The default is `["rust"]`. Adding `typescript` or
/// `python` opts in to the preview adapters once they ship.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LanguagesConfig {
    pub(super) enabled: Vec<LanguageId>,
}

impl Default for LanguagesConfig {
    fn default() -> Self {
        Self {
            enabled: vec![LanguageId::Rust],
        }
    }
}

impl LanguagesConfig {
    pub(crate) fn enabled(&self) -> &[LanguageId] {
        &self.enabled
    }

    #[cfg(test)]
    pub(crate) fn enabled_owned(&self) -> Vec<LanguageId> {
        self.enabled.clone()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProfilesConfig {
    pub(super) bun_ub: Option<BunUbProfileConfig>,
}

impl ProfilesConfig {
    pub(crate) fn bun_ub(&self) -> Option<&BunUbProfileConfig> {
        self.bun_ub.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BunUbProfileConfig {
    pub(super) test_roots: Vec<String>,
    pub(super) bridge_hints: PathBuf,
}

impl BunUbProfileConfig {
    pub(crate) fn test_roots(&self) -> &[String] {
        &self.test_roots
    }

    pub(crate) fn display_bridge_hints(&self) -> String {
        self.bridge_hints.to_string_lossy().replace('\\', "/")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConfigSeverity {
    Off,
    Info,
    Warning,
    Note,
}

impl ConfigSeverity {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ConfigSeverity::Off => "off",
            ConfigSeverity::Info => "info",
            ConfigSeverity::Warning => "warning",
            ConfigSeverity::Note => "note",
        }
    }

    pub(crate) fn github_annotation_level(self) -> Option<&'static str> {
        match self {
            ConfigSeverity::Off => None,
            ConfigSeverity::Info | ConfigSeverity::Note => Some("notice"),
            ConfigSeverity::Warning => Some("warning"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SeverityConfig {
    pub(super) findings: FindingSeverityConfig,
    pub(super) seams: SeamSeverityConfig,
}

impl SeverityConfig {
    pub(crate) fn for_exposure(&self, class: &ExposureClass) -> ConfigSeverity {
        self.findings.for_class(class)
    }

    pub(crate) fn for_seam(&self, class: SeamGripClass) -> ConfigSeverity {
        self.seams.for_class(class)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FindingSeverityConfig {
    pub(super) exposed: ConfigSeverity,
    pub(super) weakly_exposed: ConfigSeverity,
    pub(super) reachable_unrevealed: ConfigSeverity,
    pub(super) no_static_path: ConfigSeverity,
    pub(super) infection_unknown: ConfigSeverity,
    pub(super) propagation_unknown: ConfigSeverity,
    pub(super) static_unknown: ConfigSeverity,
}

impl Default for FindingSeverityConfig {
    fn default() -> Self {
        Self {
            exposed: ConfigSeverity::Info,
            weakly_exposed: ConfigSeverity::Warning,
            reachable_unrevealed: ConfigSeverity::Warning,
            no_static_path: ConfigSeverity::Warning,
            infection_unknown: ConfigSeverity::Warning,
            propagation_unknown: ConfigSeverity::Note,
            static_unknown: ConfigSeverity::Note,
        }
    }
}

impl FindingSeverityConfig {
    fn for_class(&self, class: &ExposureClass) -> ConfigSeverity {
        match class {
            ExposureClass::Exposed => self.exposed,
            ExposureClass::WeaklyExposed => self.weakly_exposed,
            ExposureClass::ReachableUnrevealed => self.reachable_unrevealed,
            ExposureClass::NoStaticPath => self.no_static_path,
            ExposureClass::InfectionUnknown => self.infection_unknown,
            ExposureClass::PropagationUnknown => self.propagation_unknown,
            ExposureClass::StaticUnknown => self.static_unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SeamSeverityConfig {
    pub(super) strongly_gripped: ConfigSeverity,
    pub(super) weakly_gripped: ConfigSeverity,
    pub(super) ungripped: ConfigSeverity,
    pub(super) reachable_unrevealed: ConfigSeverity,
    pub(super) activation_unknown: ConfigSeverity,
    pub(super) propagation_unknown: ConfigSeverity,
    pub(super) observation_unknown: ConfigSeverity,
    pub(super) discrimination_unknown: ConfigSeverity,
    pub(super) opaque: ConfigSeverity,
    pub(super) intentional: ConfigSeverity,
    pub(super) suppressed: ConfigSeverity,
}

impl Default for SeamSeverityConfig {
    fn default() -> Self {
        Self {
            strongly_gripped: ConfigSeverity::Off,
            weakly_gripped: ConfigSeverity::Warning,
            ungripped: ConfigSeverity::Warning,
            reachable_unrevealed: ConfigSeverity::Warning,
            activation_unknown: ConfigSeverity::Info,
            propagation_unknown: ConfigSeverity::Info,
            observation_unknown: ConfigSeverity::Info,
            discrimination_unknown: ConfigSeverity::Info,
            opaque: ConfigSeverity::Info,
            intentional: ConfigSeverity::Off,
            suppressed: ConfigSeverity::Off,
        }
    }
}

impl SeamSeverityConfig {
    fn for_class(&self, class: SeamGripClass) -> ConfigSeverity {
        match class {
            SeamGripClass::StronglyGripped => self.strongly_gripped,
            SeamGripClass::WeaklyGripped => self.weakly_gripped,
            SeamGripClass::Ungripped => self.ungripped,
            SeamGripClass::ReachableUnrevealed => self.reachable_unrevealed,
            SeamGripClass::ActivationUnknown => self.activation_unknown,
            SeamGripClass::PropagationUnknown => self.propagation_unknown,
            SeamGripClass::ObservationUnknown => self.observation_unknown,
            SeamGripClass::DiscriminationUnknown => self.discrimination_unknown,
            SeamGripClass::Opaque => self.opaque,
            SeamGripClass::Intentional => self.intentional,
            SeamGripClass::Suppressed => self.suppressed,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CheckInputExplicit {
    pub(crate) mode: bool,
    pub(crate) include_unchanged_tests: bool,
}
