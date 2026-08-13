//! Pure configuration data types, defaults, and accessors.

use crate::analysis::seams::SeamGripClass;
use crate::app::Mode;
use crate::domain::{ExposureClass, LanguageId, OracleStrength};
use std::path::{Path, PathBuf};

use super::{
    DEFAULT_CONTEXT_RELATED_TESTS, DEFAULT_LSP_SEAM_DIAGNOSTICS, DEFAULT_SUPPRESSIONS_PATH,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RiprConfig {
    pub analysis: AnalysisConfig,
    pub oracles: OraclePolicy,
    pub severity: SeverityConfig,
    pub lsp: LspConfig,
    pub reports: ReportsConfig,
    pub suppressions: SuppressionsConfig,
    pub languages: LanguagesConfig,
    pub profiles: ProfilesConfig,
    pub typescript: TypescriptConfig,
    pub perl: PerlConfig,
    pub source_path: Option<PathBuf>,
    pub source_text: Option<String>,
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

    pub(crate) fn typescript(&self) -> &TypescriptConfig {
        &self.typescript
    }

    /// Perl producer configuration (Campaign 31 Phase D, #1407).
    pub(crate) fn perl(&self) -> &PerlConfig {
        &self.perl
    }

    pub(crate) fn source_text(&self) -> Option<&str> {
        self.source_text.as_deref()
    }

    pub(crate) fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnalysisConfig {
    pub mode: Option<Mode>,
    pub include_unchanged_tests: Option<bool>,
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
pub struct OraclePolicy {
    pub snapshot_strength: OracleStrength,
    pub mock_expectation_strength: OracleStrength,
    pub broad_error_strength: OracleStrength,
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
pub struct LspConfig {
    pub seam_diagnostics: Option<bool>,
    pub diagnostic_profile: Option<LspDiagnosticProfile>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LspDiagnosticProfile {
    #[default]
    Actionable,
    Full,
}

impl LspDiagnosticProfile {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Actionable => "actionable",
            Self::Full => "full",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "actionable" => Ok(Self::Actionable),
            "full" => Ok(Self::Full),
            other => Err(format!(
                "invalid lsp.diagnostic_profile {other:?}; expected \"actionable\" or \"full\""
            )),
        }
    }
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            seam_diagnostics: Some(DEFAULT_LSP_SEAM_DIAGNOSTICS),
            diagnostic_profile: None,
        }
    }
}

impl LspConfig {
    pub(crate) fn seam_diagnostics(&self) -> Option<bool> {
        self.seam_diagnostics
    }

    pub(crate) fn diagnostic_profile(&self) -> Option<LspDiagnosticProfile> {
        self.diagnostic_profile
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportsConfig {
    pub max_related_tests: usize,
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
pub struct SuppressionsConfig {
    pub path: PathBuf,
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

/// `[typescript]` opt-in configuration for the TypeScript preview adapter.
///
/// All options default to `false` / off. This section may be absent from
/// `ripr.toml`; absence is equivalent to `TypescriptConfig::default()`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypescriptConfig {
    /// When `true`, the adapter reads `compilerOptions.paths` from `tsconfig.json`
    /// (or `jsconfig.json`) and uses the alias map to resolve non-relative import
    /// specifiers to workspace files during owner↔test discovery.
    ///
    /// Default: `false` (opt-in, fail-closed per RIPR-SPEC-0099).
    pub resolve_tsconfig_paths: bool,
}

impl TypescriptConfig {
    pub(crate) fn resolve_tsconfig_paths(&self) -> bool {
        self.resolve_tsconfig_paths
    }
}

/// `[languages]` repository configuration per RIPR-SPEC-0026.
///
/// `enabled` is the ordered list of source languages the analysis pipeline
/// will dispatch to. The default is `["rust"]`. Adding `typescript` or
/// `python` opts in to the preview adapters once they ship.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguagesConfig {
    pub enabled: Vec<LanguageId>,
    pub rust: RustLanguageConfig,
}

impl Default for LanguagesConfig {
    fn default() -> Self {
        Self {
            enabled: vec![LanguageId::Rust],
            rust: RustLanguageConfig::default(),
        }
    }
}

impl LanguagesConfig {
    pub(crate) fn enabled(&self) -> &[LanguageId] {
        &self.enabled
    }

    pub(crate) fn generated_file_patterns(&self) -> &[String] {
        &self.rust.generated_file_patterns
    }

    #[cfg(test)]
    pub(crate) fn enabled_owned(&self) -> Vec<LanguageId> {
        self.enabled.clone()
    }
}

/// `[languages.rust]` repository configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RustLanguageConfig {
    pub generated_file_patterns: Vec<String>,
}

fn canonical_generated_file_patterns(patterns: &[String]) -> String {
    let mut ordered = patterns.iter().collect::<Vec<_>>();
    ordered.sort_unstable();

    let mut encoded = ordered.len().to_string();
    for pattern in ordered {
        encoded.push('|');
        encoded.push_str(&pattern.len().to_string());
        encoded.push(':');
        encoded.push_str(pattern);
    }
    encoded
}

/// `[perl]` repository configuration (Campaign 31 Phase D, #1407).
///
/// When `producer` is set to `"perllsp"`, ripr invokes a Perl facts
/// exporter to generate a fact packet, then consumes it through the
/// production `PerlAdapter`. The canonical producer binary is
/// `perl-ripr-facts`; `perllsp` and `perl-lsp` are compatibility wrappers
/// for that same exporter. This is the managed producer mode — the user
/// does not need to run the exporter manually.
///
/// When `producer` is `None` (default), the user must supply `--perl-facts
/// PATH` explicitly. No silent invocation occurs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PerlConfig {
    /// When `"perllsp"`, enables managed producer mode.
    pub producer: Option<String>,
    /// Override path to the Perl facts exporter executable. The canonical
    /// binary is `perl-ripr-facts`; `perllsp` and `perl-lsp` are
    /// compatibility wrappers. When `None`, uses `perllsp` from PATH.
    pub executable: Option<PathBuf>,
    /// Timeout in milliseconds for the producer invocation. Default: 30000.
    pub timeout_ms: u64,
    /// Cache directory for produced fact packets. Default:
    /// `target/ripr/perl-facts/`.
    pub cache_dir: Option<PathBuf>,
}

impl PerlConfig {
    pub(crate) fn producer(&self) -> Option<&str> {
        self.producer.as_deref()
    }

    pub(crate) fn executable(&self) -> Option<&Path> {
        self.executable.as_deref()
    }

    pub(crate) fn timeout_ms(&self) -> u64 {
        if self.timeout_ms == 0 {
            30_000
        } else {
            self.timeout_ms
        }
    }

    pub(crate) fn cache_dir(&self) -> Option<&Path> {
        self.cache_dir.as_deref()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfilesConfig {
    pub bun_ub: Option<BunUbProfileConfig>,
}

impl ProfilesConfig {
    pub(crate) fn bun_ub(&self) -> Option<&BunUbProfileConfig> {
        self.bun_ub.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BunUbProfileConfig {
    pub test_roots: Vec<String>,
    pub bridge_hints: PathBuf,
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
pub enum ConfigSeverity {
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
pub struct SeverityConfig {
    pub findings: FindingSeverityConfig,
    pub seams: SeamSeverityConfig,
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
pub struct FindingSeverityConfig {
    pub exposed: ConfigSeverity,
    pub weakly_exposed: ConfigSeverity,
    pub reachable_unrevealed: ConfigSeverity,
    pub no_static_path: ConfigSeverity,
    pub infection_unknown: ConfigSeverity,
    pub propagation_unknown: ConfigSeverity,
    pub static_unknown: ConfigSeverity,
}

impl Default for FindingSeverityConfig {
    fn default() -> Self {
        Self {
            exposed: ConfigSeverity::Warning,
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
pub struct SeamSeverityConfig {
    pub strongly_gripped: ConfigSeverity,
    pub weakly_gripped: ConfigSeverity,
    pub ungripped: ConfigSeverity,
    pub reachable_unrevealed: ConfigSeverity,
    pub activation_unknown: ConfigSeverity,
    pub propagation_unknown: ConfigSeverity,
    pub observation_unknown: ConfigSeverity,
    pub discrimination_unknown: ConfigSeverity,
    pub opaque: ConfigSeverity,
    pub intentional: ConfigSeverity,
    pub suppressed: ConfigSeverity,
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
pub struct CheckInputExplicit {
    pub(crate) mode: bool,
    pub(crate) include_unchanged_tests: bool,
}

/// Version of the check-artifact config-identity contract (RIPR-SPEC-0140).
///
/// Any PR that adds a finding-affecting `ripr.toml` field must classify it in
/// [`RiprConfig::check_artifact_identity_fields`] and bump this version in the
/// same PR. The classification is closed: the field enumerator destructures
/// every config struct without `..`, so an unclassified field fails to
/// compile, and a unit test pins the resulting role of every field.
pub const CHECK_ARTIFACT_CONFIG_IDENTITY_VERSION: u32 = 1;

/// How one `ripr.toml` field participates in the check-artifact identity gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigIdentityRole {
    /// Finding-affecting: the canonical value is hashed into the config
    /// identity, so a changed value fails artifact reuse closed.
    FindingAffecting,
    /// Finding-affecting but already recorded elsewhere in the artifact
    /// identity (resolved mode, enabled languages, or a `CheckInput`
    /// analysis option); hashing it again would be redundant.
    CapturedElsewhere,
    /// Excluded from the identity: render-only, LSP-only, loader container
    /// metadata, or not consumed by the diff-check analysis pipeline.
    Excluded,
}

/// One classified config field in the check-artifact identity contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigIdentityField {
    /// Dotted field path as written in `ripr.toml` (or the loader container).
    pub(crate) name: &'static str,
    pub(crate) role: ConfigIdentityRole,
    /// Canonical normalized value with defaults materialized. Present only
    /// for [`ConfigIdentityRole::FindingAffecting`] fields.
    pub(crate) value: Option<String>,
    /// Where the field is captured instead, or why it is excluded.
    pub(crate) note: &'static str,
}

impl RiprConfig {
    /// Classify every `ripr.toml` config field for the check-artifact
    /// identity gate (RIPR-SPEC-0140).
    ///
    /// This is the closed allowlist: each struct is destructured WITHOUT a
    /// `..` rest pattern, so adding a field to any config type fails
    /// compilation here until the author explicitly classifies the new field.
    /// Render-only knobs (severity display, reports, output format) are
    /// excluded and honored fresh at render time by the consuming command.
    pub(crate) fn check_artifact_identity_fields(&self) -> Vec<ConfigIdentityField> {
        let RiprConfig {
            analysis,
            oracles,
            severity,
            lsp,
            reports,
            suppressions,
            languages,
            profiles,
            typescript,
            perl,
            source_path: _,
            source_text: _,
        } = self;
        let AnalysisConfig {
            mode: _,
            include_unchanged_tests: _,
        } = analysis;
        let OraclePolicy {
            snapshot_strength,
            mock_expectation_strength,
            broad_error_strength,
        } = oracles;
        let SeverityConfig { findings, seams } = severity;
        let FindingSeverityConfig {
            exposed: _,
            weakly_exposed: _,
            reachable_unrevealed: _,
            no_static_path: _,
            infection_unknown: _,
            propagation_unknown: _,
            static_unknown: _,
        } = findings;
        let SeamSeverityConfig {
            strongly_gripped: _,
            weakly_gripped: _,
            ungripped: _,
            reachable_unrevealed: _,
            activation_unknown: _,
            propagation_unknown: _,
            observation_unknown: _,
            discrimination_unknown: _,
            opaque: _,
            intentional: _,
            suppressed: _,
        } = seams;
        let LspConfig {
            seam_diagnostics: _,
            diagnostic_profile: _,
        } = lsp;
        let ReportsConfig {
            max_related_tests: _,
        } = reports;
        let SuppressionsConfig { path: _ } = suppressions;
        let LanguagesConfig {
            enabled,
            rust: RustLanguageConfig {
                generated_file_patterns,
            },
        } = languages;
        let ProfilesConfig { bun_ub } = profiles;
        let TypescriptConfig {
            resolve_tsconfig_paths,
        } = typescript;
        let PerlConfig {
            producer,
            executable,
            timeout_ms,
            cache_dir,
        } = perl;
        let generated_file_patterns_identity = if enabled.contains(&LanguageId::Rust) {
            canonical_generated_file_patterns(generated_file_patterns)
        } else {
            String::new()
        };
        let mut fields = vec![
            ConfigIdentityField {
                name: "analysis.mode",
                role: ConfigIdentityRole::CapturedElsewhere,
                value: None,
                note: "resolved into the artifact identity `mode` via CheckInput",
            },
            ConfigIdentityField {
                name: "analysis.include_unchanged_tests",
                role: ConfigIdentityRole::CapturedElsewhere,
                value: None,
                note: "resolved into the artifact identity analysis input options via CheckInput",
            },
            ConfigIdentityField {
                name: "oracles.snapshot_strength",
                role: ConfigIdentityRole::FindingAffecting,
                value: Some(snapshot_strength.as_str().to_string()),
                note: "oracle strength policy changes classification evidence",
            },
            ConfigIdentityField {
                name: "oracles.mock_expectation_strength",
                role: ConfigIdentityRole::FindingAffecting,
                value: Some(mock_expectation_strength.as_str().to_string()),
                note: "oracle strength policy changes classification evidence",
            },
            ConfigIdentityField {
                name: "oracles.broad_error_strength",
                role: ConfigIdentityRole::FindingAffecting,
                value: Some(broad_error_strength.as_str().to_string()),
                note: "oracle strength policy changes classification evidence",
            },
        ];
        for name in [
            "severity.findings.exposed",
            "severity.findings.weakly_exposed",
            "severity.findings.reachable_unrevealed",
            "severity.findings.no_static_path",
            "severity.findings.infection_unknown",
            "severity.findings.propagation_unknown",
            "severity.findings.static_unknown",
            "severity.seams.strongly_gripped",
            "severity.seams.weakly_gripped",
            "severity.seams.ungripped",
            "severity.seams.reachable_unrevealed",
            "severity.seams.activation_unknown",
            "severity.seams.propagation_unknown",
            "severity.seams.observation_unknown",
            "severity.seams.discrimination_unknown",
            "severity.seams.opaque",
            "severity.seams.intentional",
            "severity.seams.suppressed",
        ] {
            fields.push(ConfigIdentityField {
                name,
                role: ConfigIdentityRole::Excluded,
                value: None,
                note: "render-only severity display; honored fresh at render time",
            });
        }
        fields.extend([
            ConfigIdentityField {
                name: "lsp.seam_diagnostics",
                role: ConfigIdentityRole::Excluded,
                value: None,
                note: "LSP-only; the CLI diff-check pipeline does not read it",
            },
            ConfigIdentityField {
                name: "lsp.diagnostic_profile",
                role: ConfigIdentityRole::Excluded,
                value: None,
                note: "LSP-only; the CLI has no diagnostic profile surface",
            },
            ConfigIdentityField {
                name: "reports.max_related_tests",
                role: ConfigIdentityRole::Excluded,
                value: None,
                note: "render-only context packet bound; honored fresh at render time",
            },
            ConfigIdentityField {
                name: "suppressions.path",
                role: ConfigIdentityRole::Excluded,
                value: None,
                note: "marks check summary suppression only; the artifact carries the finding set, which suppression does not mutate",
            },
            ConfigIdentityField {
                name: "languages.enabled",
                role: ConfigIdentityRole::CapturedElsewhere,
                value: None,
                note: "recorded as the artifact identity `enabled_languages` (resolved, including the explicit --perl-facts opt-in)",
            },
            ConfigIdentityField {
                name: "languages.rust.generated_file_patterns",
                role: ConfigIdentityRole::FindingAffecting,
                value: Some(generated_file_patterns_identity),
                note: "custom Rust generated-file patterns change which source files are analyzed",
            },
        ]);
        match bun_ub {
            Some(profile) => {
                let BunUbProfileConfig {
                    test_roots: _,
                    bridge_hints: _,
                } = profile;
                fields.push(ConfigIdentityField {
                    name: "profiles.bun_ub.test_roots",
                    role: ConfigIdentityRole::Excluded,
                    value: None,
                    note: "not consumed by the diff-check analysis pipeline (doctor reporting only)",
                });
                fields.push(ConfigIdentityField {
                    name: "profiles.bun_ub.bridge_hints",
                    role: ConfigIdentityRole::Excluded,
                    value: None,
                    note: "not consumed by the diff-check analysis pipeline (doctor reporting only)",
                });
            }
            None => {
                fields.push(ConfigIdentityField {
                    name: "profiles.bun_ub",
                    role: ConfigIdentityRole::Excluded,
                    value: None,
                    note: "not consumed by the diff-check analysis pipeline (doctor reporting only)",
                });
            }
        }
        fields.extend([
            ConfigIdentityField {
                name: "typescript.resolve_tsconfig_paths",
                role: ConfigIdentityRole::FindingAffecting,
                value: Some(resolve_tsconfig_paths.to_string()),
                note: "flows into AnalysisOptions for the TypeScript adapter",
            },
            ConfigIdentityField {
                name: "perl.producer",
                role: ConfigIdentityRole::FindingAffecting,
                value: Some(producer.clone().unwrap_or_else(|| "none".to_string())),
                note: "managed producer mode changes whether a Perl fact packet is produced",
            },
            ConfigIdentityField {
                name: "perl.executable",
                role: ConfigIdentityRole::FindingAffecting,
                value: Some(
                    executable
                        .as_ref()
                        .map(|path| path.to_string_lossy().replace('\\', "/"))
                        .unwrap_or_else(|| "none".to_string()),
                ),
                note: "which producer binary generates the Perl fact packet",
            },
            ConfigIdentityField {
                name: "perl.timeout_ms",
                role: ConfigIdentityRole::FindingAffecting,
                value: Some(if *timeout_ms == 0 {
                    30_000_u64.to_string()
                } else {
                    timeout_ms.to_string()
                }),
                note: "producer timeout changes whether a Perl fact packet is produced",
            },
            ConfigIdentityField {
                name: "perl.cache_dir",
                role: ConfigIdentityRole::FindingAffecting,
                value: Some(
                    cache_dir
                        .as_ref()
                        .map(|path| path.to_string_lossy().replace('\\', "/"))
                        .unwrap_or_else(|| "none".to_string()),
                ),
                note: "where the managed Perl fact packet is written and read",
            },
            ConfigIdentityField {
                name: "source_path",
                role: ConfigIdentityRole::Excluded,
                value: None,
                note: "loader container metadata, not a ripr.toml field",
            },
            ConfigIdentityField {
                name: "source_text",
                role: ConfigIdentityRole::Excluded,
                value: None,
                note: "loader container metadata, not a ripr.toml field",
            },
        ]);
        fields
    }
}
