//! Repository configuration loader for `ripr.toml`.
//!
//! The loader is intentionally small and repo-root scoped. It does not read
//! user-global config, environment variables, or hidden alternate config
//! paths. Command adapters decide precedence by applying explicit flags or LSP
//! initialization options after this file is loaded.

use crate::analysis::seams::SeamGripClass;
use crate::app::{CheckInput, Mode};
use crate::domain::{ExposureClass, OracleStrength};
use serde::Deserialize;
use std::path::{Component, Path, PathBuf};

pub(crate) const CONFIG_FILE_NAME: &str = "ripr.toml";
pub(crate) const DEFAULT_CONTEXT_RELATED_TESTS: usize = 5;
pub(crate) const DEFAULT_LSP_SEAM_DIAGNOSTICS: bool = true;
const DEFAULT_SUPPRESSIONS_PATH: &str = ".ripr/suppressions.toml";
const INIT_CONFIG_TEXT: &str = r#"[analysis]
# Default analysis mode when CLI flags or LSP initialization options do not
# set one explicitly. Valid: instant, draft, fast, deep, ready.
mode = "draft"
include_unchanged_tests = true

[oracles]
# Probe-relative defaults for oracle shapes that are repo-policy-sensitive.
# Valid strengths: strong, medium, weak, smoke, none, unknown.
snapshot_strength = "medium"
mock_expectation_strength = "medium"
broad_error_strength = "weak"

[severity.findings]
# Valid severities: info, warning, note.
exposed = "info"
weakly_exposed = "warning"
reachable_unrevealed = "warning"
no_static_path = "warning"
infection_unknown = "warning"
propagation_unknown = "note"
static_unknown = "note"

[severity.seams]
# Valid severities: off, info, warning, note.
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
# Built-in defaults enable bounded saved-workspace seam diagnostics. LSP
# initializationOptions.seamDiagnostics still wins explicitly, and repo policy
# may disable this with seam_diagnostics = false.
seam_diagnostics = true

[reports]
# Default for context packets and editor collect-context commands when no
# explicit --max-related-tests argument is supplied.
max_related_tests = 5

[suppressions]
# Repo-relative, slash-separated path. Badge renderers load this path.
path = ".ripr/suppressions.toml"
"#;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RiprConfig {
    analysis: AnalysisConfig,
    oracles: OraclePolicy,
    severity: SeverityConfig,
    lsp: LspConfig,
    reports: ReportsConfig,
    suppressions: SuppressionsConfig,
    source_path: Option<PathBuf>,
    source_text: Option<String>,
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

    pub(crate) fn source_text(&self) -> Option<&str> {
        self.source_text.as_deref()
    }

    pub(crate) fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AnalysisConfig {
    mode: Option<Mode>,
    include_unchanged_tests: Option<bool>,
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
    snapshot_strength: OracleStrength,
    mock_expectation_strength: OracleStrength,
    broad_error_strength: OracleStrength,
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
    seam_diagnostics: Option<bool>,
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
    max_related_tests: usize,
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
    path: PathBuf,
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
    findings: FindingSeverityConfig,
    seams: SeamSeverityConfig,
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
struct FindingSeverityConfig {
    exposed: ConfigSeverity,
    weakly_exposed: ConfigSeverity,
    reachable_unrevealed: ConfigSeverity,
    no_static_path: ConfigSeverity,
    infection_unknown: ConfigSeverity,
    propagation_unknown: ConfigSeverity,
    static_unknown: ConfigSeverity,
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
struct SeamSeverityConfig {
    strongly_gripped: ConfigSeverity,
    weakly_gripped: ConfigSeverity,
    ungripped: ConfigSeverity,
    reachable_unrevealed: ConfigSeverity,
    activation_unknown: ConfigSeverity,
    propagation_unknown: ConfigSeverity,
    observation_unknown: ConfigSeverity,
    discrimination_unknown: ConfigSeverity,
    opaque: ConfigSeverity,
    intentional: ConfigSeverity,
    suppressed: ConfigSeverity,
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

pub(crate) fn load_for_root(root: &Path) -> Result<RiprConfig, String> {
    let path = root.join(CONFIG_FILE_NAME);
    if !path.exists() {
        return Ok(RiprConfig::default());
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|err| format!("read {} failed: {err}", path.display()))?;
    let mut config = parse_config(&text).map_err(|err| format!("{}: {err}", path.display()))?;
    config.source_path = Some(path);
    config.source_text = Some(text);
    Ok(config)
}

pub(crate) fn generated_init_config() -> &'static str {
    INIT_CONFIG_TEXT
}

pub(crate) fn config_fingerprint(source_text: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in source_text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("fnv1a64:{hash:016x}")
}

pub(crate) fn apply_to_check_input(
    input: &mut CheckInput,
    config: &RiprConfig,
    explicit: CheckInputExplicit,
) {
    if !explicit.mode
        && let Some(mode) = config.analysis.mode()
    {
        input.mode = mode.clone();
    }
    if !explicit.include_unchanged_tests
        && let Some(include) = config.analysis.include_unchanged_tests()
    {
        input.include_unchanged_tests = include;
    }
}

fn parse_config(text: &str) -> Result<RiprConfig, String> {
    let raw: RawConfig = toml::from_str(text).map_err(|err| format!("invalid ripr.toml: {err}"))?;
    RiprConfig::from_raw(raw)
}

#[cfg(test)]
pub(crate) fn tests_only_parse(text: &str) -> Result<RiprConfig, String> {
    parse_config(text)
}

impl RiprConfig {
    fn from_raw(raw: RawConfig) -> Result<Self, String> {
        let mut config = RiprConfig::default();
        if let Some(analysis) = raw.analysis {
            if let Some(mode) = analysis.mode {
                config.analysis.mode = Some(parse_mode_value(&mode)?);
            }
            config.analysis.include_unchanged_tests = analysis.include_unchanged_tests;
        }
        if let Some(oracles) = raw.oracles {
            if let Some(strength) = oracles.snapshot_strength {
                config.oracles.snapshot_strength = parse_oracle_strength(&strength)?;
            }
            if let Some(strength) = oracles.mock_expectation_strength {
                config.oracles.mock_expectation_strength = parse_oracle_strength(&strength)?;
            }
            if let Some(strength) = oracles.broad_error_strength {
                config.oracles.broad_error_strength = parse_oracle_strength(&strength)?;
            }
        }
        if let Some(severity) = raw.severity {
            config.severity = merge_severity(config.severity, severity)?;
        }
        if let Some(lsp) = raw.lsp
            && let Some(seam_diagnostics) = lsp.seam_diagnostics
        {
            config.lsp.seam_diagnostics = Some(seam_diagnostics);
        }
        if let Some(reports) = raw.reports
            && let Some(max) = reports.max_related_tests
        {
            config.reports.max_related_tests = max;
        }
        if let Some(suppressions) = raw.suppressions
            && let Some(path) = suppressions.path
        {
            config.suppressions.path = parse_relative_path("suppressions.path", &path)?;
        }
        Ok(config)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    analysis: Option<RawAnalysisConfig>,
    oracles: Option<RawOraclePolicy>,
    severity: Option<RawSeverityConfig>,
    lsp: Option<RawLspConfig>,
    reports: Option<RawReportsConfig>,
    suppressions: Option<RawSuppressionsConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAnalysisConfig {
    mode: Option<String>,
    include_unchanged_tests: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOraclePolicy {
    snapshot_strength: Option<String>,
    mock_expectation_strength: Option<String>,
    broad_error_strength: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLspConfig {
    seam_diagnostics: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReportsConfig {
    max_related_tests: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSuppressionsConfig {
    path: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSeverityConfig {
    findings: Option<RawFindingSeverityConfig>,
    seams: Option<RawSeamSeverityConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFindingSeverityConfig {
    exposed: Option<String>,
    weakly_exposed: Option<String>,
    reachable_unrevealed: Option<String>,
    no_static_path: Option<String>,
    infection_unknown: Option<String>,
    propagation_unknown: Option<String>,
    static_unknown: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSeamSeverityConfig {
    strongly_gripped: Option<String>,
    weakly_gripped: Option<String>,
    ungripped: Option<String>,
    reachable_unrevealed: Option<String>,
    activation_unknown: Option<String>,
    propagation_unknown: Option<String>,
    observation_unknown: Option<String>,
    discrimination_unknown: Option<String>,
    opaque: Option<String>,
    intentional: Option<String>,
    suppressed: Option<String>,
}

fn merge_severity(
    mut current: SeverityConfig,
    raw: RawSeverityConfig,
) -> Result<SeverityConfig, String> {
    if let Some(findings) = raw.findings {
        merge_finding_severity(&mut current.findings, findings)?;
    }
    if let Some(seams) = raw.seams {
        merge_seam_severity(&mut current.seams, seams)?;
    }
    Ok(current)
}

fn merge_finding_severity(
    current: &mut FindingSeverityConfig,
    raw: RawFindingSeverityConfig,
) -> Result<(), String> {
    assign_severity(
        &mut current.exposed,
        raw.exposed,
        "severity.findings.exposed",
        false,
    )?;
    assign_severity(
        &mut current.weakly_exposed,
        raw.weakly_exposed,
        "severity.findings.weakly_exposed",
        false,
    )?;
    assign_severity(
        &mut current.reachable_unrevealed,
        raw.reachable_unrevealed,
        "severity.findings.reachable_unrevealed",
        false,
    )?;
    assign_severity(
        &mut current.no_static_path,
        raw.no_static_path,
        "severity.findings.no_static_path",
        false,
    )?;
    assign_severity(
        &mut current.infection_unknown,
        raw.infection_unknown,
        "severity.findings.infection_unknown",
        false,
    )?;
    assign_severity(
        &mut current.propagation_unknown,
        raw.propagation_unknown,
        "severity.findings.propagation_unknown",
        false,
    )?;
    assign_severity(
        &mut current.static_unknown,
        raw.static_unknown,
        "severity.findings.static_unknown",
        false,
    )?;
    Ok(())
}

fn merge_seam_severity(
    current: &mut SeamSeverityConfig,
    raw: RawSeamSeverityConfig,
) -> Result<(), String> {
    assign_severity(
        &mut current.strongly_gripped,
        raw.strongly_gripped,
        "severity.seams.strongly_gripped",
        true,
    )?;
    assign_severity(
        &mut current.weakly_gripped,
        raw.weakly_gripped,
        "severity.seams.weakly_gripped",
        true,
    )?;
    assign_severity(
        &mut current.ungripped,
        raw.ungripped,
        "severity.seams.ungripped",
        true,
    )?;
    assign_severity(
        &mut current.reachable_unrevealed,
        raw.reachable_unrevealed,
        "severity.seams.reachable_unrevealed",
        true,
    )?;
    assign_severity(
        &mut current.activation_unknown,
        raw.activation_unknown,
        "severity.seams.activation_unknown",
        true,
    )?;
    assign_severity(
        &mut current.propagation_unknown,
        raw.propagation_unknown,
        "severity.seams.propagation_unknown",
        true,
    )?;
    assign_severity(
        &mut current.observation_unknown,
        raw.observation_unknown,
        "severity.seams.observation_unknown",
        true,
    )?;
    assign_severity(
        &mut current.discrimination_unknown,
        raw.discrimination_unknown,
        "severity.seams.discrimination_unknown",
        true,
    )?;
    assign_severity(
        &mut current.opaque,
        raw.opaque,
        "severity.seams.opaque",
        true,
    )?;
    assign_severity(
        &mut current.intentional,
        raw.intentional,
        "severity.seams.intentional",
        true,
    )?;
    assign_severity(
        &mut current.suppressed,
        raw.suppressed,
        "severity.seams.suppressed",
        true,
    )?;
    Ok(())
}

fn assign_severity(
    target: &mut ConfigSeverity,
    raw: Option<String>,
    field: &str,
    allow_off: bool,
) -> Result<(), String> {
    if let Some(value) = raw {
        *target = parse_severity(field, &value, allow_off)?;
    }
    Ok(())
}

fn parse_mode_value(value: &str) -> Result<Mode, String> {
    match value {
        "instant" => Ok(Mode::Instant),
        "draft" => Ok(Mode::Draft),
        "fast" => Ok(Mode::Fast),
        "deep" => Ok(Mode::Deep),
        "ready" => Ok(Mode::Ready),
        _ => Err(format!(
            "analysis.mode `{value}` is not supported; expected instant, draft, fast, deep, or ready"
        )),
    }
}

fn parse_oracle_strength(value: &str) -> Result<OracleStrength, String> {
    match value {
        "strong" => Ok(OracleStrength::Strong),
        "medium" => Ok(OracleStrength::Medium),
        "weak" => Ok(OracleStrength::Weak),
        "smoke" => Ok(OracleStrength::Smoke),
        "none" => Ok(OracleStrength::None),
        "unknown" => Ok(OracleStrength::Unknown),
        _ => Err(format!(
            "oracle strength `{value}` is not supported; expected strong, medium, weak, smoke, none, or unknown"
        )),
    }
}

fn parse_severity(field: &str, value: &str, allow_off: bool) -> Result<ConfigSeverity, String> {
    match value {
        "info" => Ok(ConfigSeverity::Info),
        "warning" => Ok(ConfigSeverity::Warning),
        "note" => Ok(ConfigSeverity::Note),
        "off" if allow_off => Ok(ConfigSeverity::Off),
        "off" => Err(format!(
            "{field} cannot be `off`; use suppressions for accepted debt"
        )),
        _ => Err(format!(
            "{field} `{value}` is not supported; expected info, warning, or note{}",
            if allow_off { ", or off" } else { "" }
        )),
    }
}

fn parse_relative_path(field: &str, value: &str) -> Result<PathBuf, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if trimmed.contains('\\') {
        return Err(format!(
            "{field} `{value}` uses backslashes; use `/` separators"
        ));
    }
    if trimmed.contains(':') {
        return Err(format!(
            "{field} `{value}` uses a drive or scheme prefix; use a repository-relative path"
        ));
    }
    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        return Err(format!("{field} `{value}` must be repository-relative"));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!("{field} `{value}` must stay within the repository"));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::OracleKind;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("ripr-config-{name}-{stamp}"));
        fs::create_dir_all(&root).map_err(|err| format!("create temp root failed: {err}"))?;
        Ok(root)
    }

    #[test]
    fn missing_config_uses_behavior_preserving_defaults() -> Result<(), String> {
        let root = temp_root("missing")?;
        let config = load_for_root(&root)?;

        assert!(config.source_path().is_none());
        assert!(config.analysis().mode().is_none());
        assert_eq!(config.lsp().seam_diagnostics(), Some(true));
        assert_eq!(
            config.reports().max_related_tests(),
            DEFAULT_CONTEXT_RELATED_TESTS
        );
        Ok(())
    }

    #[test]
    fn config_file_sets_core_operational_defaults() -> Result<(), String> {
        let config = parse_config(
            r#"
[analysis]
mode = "deep"
include_unchanged_tests = false

[oracles]
snapshot_strength = "strong"
mock_expectation_strength = "strong"
broad_error_strength = "medium"

[lsp]
seam_diagnostics = true

[reports]
max_related_tests = 9

[suppressions]
path = ".ripr/custom-suppressions.toml"

[severity.findings]
exposed = "note"
weakly_exposed = "info"
reachable_unrevealed = "warning"
no_static_path = "note"
infection_unknown = "info"
propagation_unknown = "warning"
static_unknown = "warning"

[severity.seams]
strongly_gripped = "off"
weakly_gripped = "warning"
ungripped = "info"
reachable_unrevealed = "note"
activation_unknown = "info"
propagation_unknown = "warning"
observation_unknown = "note"
discrimination_unknown = "info"
opaque = "note"
intentional = "off"
suppressed = "off"
        "#,
        )?;

        assert_eq!(config.analysis().mode(), Some(&Mode::Deep));
        assert_eq!(config.analysis().include_unchanged_tests(), Some(false));
        assert_eq!(
            config.oracles().snapshot_strength(),
            &OracleStrength::Strong
        );
        assert_eq!(
            config.oracles().mock_expectation_strength(),
            &OracleStrength::Strong
        );
        assert_eq!(
            config.oracles().broad_error_strength(),
            &OracleStrength::Medium
        );
        assert_eq!(config.lsp().seam_diagnostics(), Some(true));
        assert_eq!(config.reports().max_related_tests(), 9);
        assert_eq!(
            config.suppressions().display_path(),
            ".ripr/custom-suppressions.toml"
        );
        assert_eq!(
            config.severity().for_exposure(&ExposureClass::Exposed),
            ConfigSeverity::Note
        );
        assert_eq!(
            config
                .severity()
                .for_exposure(&ExposureClass::WeaklyExposed),
            ConfigSeverity::Info
        );
        assert_eq!(
            config
                .severity()
                .for_exposure(&ExposureClass::ReachableUnrevealed),
            ConfigSeverity::Warning
        );
        assert_eq!(
            config.severity().for_exposure(&ExposureClass::NoStaticPath),
            ConfigSeverity::Note
        );
        assert_eq!(
            config
                .severity()
                .for_exposure(&ExposureClass::InfectionUnknown),
            ConfigSeverity::Info
        );
        assert_eq!(
            config
                .severity()
                .for_exposure(&ExposureClass::PropagationUnknown),
            ConfigSeverity::Warning
        );
        assert_eq!(
            config
                .severity()
                .for_exposure(&ExposureClass::StaticUnknown),
            ConfigSeverity::Warning
        );
        assert_eq!(
            config.severity().for_seam(SeamGripClass::StronglyGripped),
            ConfigSeverity::Off
        );
        assert_eq!(
            config.severity().for_seam(SeamGripClass::WeaklyGripped),
            ConfigSeverity::Warning
        );
        assert_eq!(
            config.severity().for_seam(SeamGripClass::Ungripped),
            ConfigSeverity::Info
        );
        assert_eq!(
            config
                .severity()
                .for_seam(SeamGripClass::ReachableUnrevealed),
            ConfigSeverity::Note
        );
        assert_eq!(
            config.severity().for_seam(SeamGripClass::ActivationUnknown),
            ConfigSeverity::Info
        );
        assert_eq!(
            config
                .severity()
                .for_seam(SeamGripClass::PropagationUnknown),
            ConfigSeverity::Warning
        );
        assert_eq!(
            config
                .severity()
                .for_seam(SeamGripClass::ObservationUnknown),
            ConfigSeverity::Note
        );
        assert_eq!(
            config
                .severity()
                .for_seam(SeamGripClass::DiscriminationUnknown),
            ConfigSeverity::Info
        );
        assert_eq!(
            config.severity().for_seam(SeamGripClass::Opaque),
            ConfigSeverity::Note
        );
        assert_eq!(
            config.severity().for_seam(SeamGripClass::Intentional),
            ConfigSeverity::Off
        );
        assert_eq!(
            config.severity().for_seam(SeamGripClass::Suppressed),
            ConfigSeverity::Off
        );
        Ok(())
    }

    #[test]
    fn generated_init_config_is_conservative_and_parseable() -> Result<(), String> {
        let config = parse_config(generated_init_config())?;

        assert_eq!(config.analysis().mode(), Some(&Mode::Draft));
        assert_eq!(config.analysis().include_unchanged_tests(), Some(true));
        assert_eq!(
            config.oracles().snapshot_strength(),
            &OracleStrength::Medium
        );
        assert_eq!(
            config.oracles().mock_expectation_strength(),
            &OracleStrength::Medium
        );
        assert_eq!(
            config.oracles().broad_error_strength(),
            &OracleStrength::Weak
        );
        assert_eq!(config.lsp().seam_diagnostics(), Some(true));
        assert_eq!(
            config.reports().max_related_tests(),
            DEFAULT_CONTEXT_RELATED_TESTS
        );
        assert_eq!(
            config.suppressions().display_path(),
            DEFAULT_SUPPRESSIONS_PATH
        );
        assert_eq!(
            config.severity().for_seam(SeamGripClass::StronglyGripped),
            ConfigSeverity::Off
        );
        assert_eq!(
            config.severity().for_seam(SeamGripClass::WeaklyGripped),
            ConfigSeverity::Warning
        );
        assert_eq!(
            config.severity().for_seam(SeamGripClass::Ungripped),
            ConfigSeverity::Warning
        );
        assert_eq!(
            config
                .severity()
                .for_seam(SeamGripClass::ReachableUnrevealed),
            ConfigSeverity::Warning
        );
        assert_eq!(
            config.severity().for_seam(SeamGripClass::Intentional),
            ConfigSeverity::Off
        );
        assert_eq!(
            config.severity().for_seam(SeamGripClass::Suppressed),
            ConfigSeverity::Off
        );
        Ok(())
    }

    #[test]
    fn generated_init_config_matches_builtin_defaults() -> Result<(), String> {
        let builtin = RiprConfig::default();
        let generated = parse_config(generated_init_config())?;

        let mut builtin_input = CheckInput::default();
        apply_to_check_input(&mut builtin_input, &builtin, CheckInputExplicit::default());
        let mut generated_input = CheckInput::default();
        apply_to_check_input(
            &mut generated_input,
            &generated,
            CheckInputExplicit::default(),
        );

        assert_eq!(builtin_input.mode, generated_input.mode);
        assert_eq!(
            builtin_input.include_unchanged_tests,
            generated_input.include_unchanged_tests
        );
        assert_eq!(builtin.oracles(), generated.oracles());
        assert_eq!(builtin.lsp(), generated.lsp());
        assert_eq!(builtin.reports(), generated.reports());
        assert_eq!(builtin.suppressions(), generated.suppressions());

        for class in [
            ExposureClass::Exposed,
            ExposureClass::WeaklyExposed,
            ExposureClass::ReachableUnrevealed,
            ExposureClass::NoStaticPath,
            ExposureClass::InfectionUnknown,
            ExposureClass::PropagationUnknown,
            ExposureClass::StaticUnknown,
        ] {
            assert_eq!(
                builtin.severity().for_exposure(&class),
                generated.severity().for_exposure(&class)
            );
        }

        for class in [
            SeamGripClass::StronglyGripped,
            SeamGripClass::WeaklyGripped,
            SeamGripClass::Ungripped,
            SeamGripClass::ReachableUnrevealed,
            SeamGripClass::ActivationUnknown,
            SeamGripClass::PropagationUnknown,
            SeamGripClass::ObservationUnknown,
            SeamGripClass::DiscriminationUnknown,
            SeamGripClass::Opaque,
            SeamGripClass::Intentional,
            SeamGripClass::Suppressed,
        ] {
            assert_eq!(
                builtin.severity().for_seam(class),
                generated.severity().for_seam(class)
            );
        }

        Ok(())
    }

    #[test]
    fn generated_init_config_matches_checked_in_example() -> Result<(), String> {
        let example_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ripr.toml.example");
        let example = fs::read_to_string(&example_path)
            .map_err(|err| format!("read {} failed: {err}", example_path.display()))?;
        assert_eq!(generated_init_config(), example.as_str());
        Ok(())
    }

    #[test]
    fn config_file_discovery_records_source_metadata() -> Result<(), String> {
        let root = temp_root("present")?;
        let config_path = root.join(CONFIG_FILE_NAME);
        fs::write(&config_path, "[analysis]\nmode = \"fast\"\n")
            .map_err(|err| format!("write config failed: {err}"))?;

        let config = load_for_root(&root)?;

        assert_eq!(config.source_path(), Some(config_path.as_path()));
        assert_eq!(config.source_text(), Some("[analysis]\nmode = \"fast\"\n"));
        assert_eq!(config.analysis().mode(), Some(&Mode::Fast));
        Ok(())
    }

    #[test]
    fn oracle_strength_literals_round_trip_through_config() -> Result<(), String> {
        let weak_smoke_none = parse_config(
            r#"
[oracles]
snapshot_strength = "weak"
mock_expectation_strength = "smoke"
broad_error_strength = "none"
"#,
        )?;
        assert_eq!(
            weak_smoke_none.oracles().snapshot_strength(),
            &OracleStrength::Weak
        );
        assert_eq!(
            weak_smoke_none.oracles().mock_expectation_strength(),
            &OracleStrength::Smoke
        );
        assert_eq!(
            weak_smoke_none.oracles().broad_error_strength(),
            &OracleStrength::None
        );

        let unknown = parse_config("[oracles]\nbroad_error_strength = \"unknown\"\n")?;
        assert_eq!(
            unknown.oracles().broad_error_strength(),
            &OracleStrength::Unknown
        );
        Ok(())
    }

    #[test]
    fn explicit_cli_mode_wins_over_config_mode() -> Result<(), String> {
        let config = parse_config("[analysis]\nmode = \"deep\"\n")?;
        let mut input = CheckInput {
            mode: Mode::Instant,
            include_unchanged_tests: true,
            ..CheckInput::default()
        };
        apply_to_check_input(
            &mut input,
            &config,
            CheckInputExplicit {
                mode: true,
                include_unchanged_tests: false,
            },
        );
        assert_eq!(input.mode, Mode::Instant);
        Ok(())
    }

    #[test]
    fn config_mode_applies_when_cli_mode_is_not_explicit() -> Result<(), String> {
        let config = parse_config("[analysis]\nmode = \"ready\"\n")?;
        let mut input = CheckInput::default();
        apply_to_check_input(&mut input, &config, CheckInputExplicit::default());
        assert_eq!(input.mode, Mode::Ready);
        Ok(())
    }

    #[test]
    fn malformed_or_unknown_config_is_actionable() {
        let invalid_mode = parse_config("[analysis]\nmode = \"slow\"\n");
        assert!(matches!(invalid_mode, Err(message) if message.contains("analysis.mode")));

        let unknown_field = parse_config("[analysis]\nunknown = true\n");
        assert!(matches!(unknown_field, Err(message) if message.contains("unknown field")));

        let invalid_oracle = parse_config("[oracles]\nsnapshot_strength = \"mystery\"\n");
        assert!(matches!(invalid_oracle, Err(message) if message.contains("oracle strength")));

        let finding_off = parse_config("[severity.findings]\nweakly_exposed = \"off\"\n");
        assert!(matches!(finding_off, Err(message) if message.contains("use suppressions")));

        let bad_severity = parse_config("[severity.findings]\nweakly_exposed = \"loud\"\n");
        assert!(
            matches!(bad_severity, Err(message) if message.contains("severity.findings.weakly_exposed"))
        );
    }

    #[test]
    fn config_rejects_unsafe_suppression_paths() {
        for text in [
            "[suppressions]\npath = \"\"\n".to_string(),
            "[suppressions]\npath = \"../outside.toml\"\n".to_string(),
            format!("[suppressions]\npath = \"{}tmp/suppressions.toml\"\n", '/'),
            "[suppressions]\npath = \"file:tmp/suppressions.toml\"\n".to_string(),
            "[suppressions]\npath = 'a\\b.toml'\n".to_string(),
        ] {
            assert!(
                parse_config(&text).is_err(),
                "expected invalid path for {text:?}"
            );
        }
    }

    #[test]
    fn oracle_policy_rewrites_configurable_oracle_strengths() {
        let policy = OraclePolicy {
            snapshot_strength: OracleStrength::Strong,
            mock_expectation_strength: OracleStrength::Weak,
            broad_error_strength: OracleStrength::Medium,
        };
        assert_eq!(
            policy.strength_for_kind(&OracleKind::Snapshot, OracleStrength::Medium),
            OracleStrength::Strong
        );
        assert_eq!(
            policy.strength_for_kind(&OracleKind::MockExpectation, OracleStrength::Medium),
            OracleStrength::Weak
        );
        assert_eq!(
            policy.strength_for_kind(&OracleKind::BroadError, OracleStrength::Weak),
            OracleStrength::Medium
        );
        assert_eq!(
            policy.strength_for_kind(&OracleKind::ExactValue, OracleStrength::Strong),
            OracleStrength::Strong
        );
    }
}
