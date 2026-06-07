//! Repository configuration loader for `ripr.toml`.
//!
//! The loader is intentionally small and repo-root scoped. It does not read
//! user-global config, environment variables, or hidden alternate config
//! paths. Command adapters decide precedence by applying explicit flags or LSP
//! initialization options after this file is loaded.

use crate::app::{CheckInput, Mode};
use crate::domain::{LanguageId, OracleStrength};
use serde::Deserialize;
use std::path::{Component, Path, PathBuf};

mod model;
mod python;

use model::{BunUbProfileConfig, FindingSeverityConfig, ProfilesConfig, SeamSeverityConfig};
pub(crate) use model::{
    CheckInputExplicit, ConfigSeverity, OraclePolicy, RiprConfig, SeverityConfig,
};
pub(crate) use python::detect_python_project;

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

[languages]
# Per RIPR-SPEC-0026, only `rust` is enabled by default. Add `typescript`,
# `python`, or `perl` to opt into preview adapters when the ripr binary was
# built with the matching Cargo feature (`lang-typescript`, `lang-python`, or
# `lang-perl`). When this file is absent, Python project markers can enable
# Python preview analysis
# automatically for the detected repository root; this explicit list remains
# authoritative when present.
# Valid values: rust, typescript, python, perl.
enabled = ["rust"]

# Optional Bun stable-byte UB advisory profile. Leave this commented unless the
# repository wants TypeScript-family preview evidence for Bun Rust/FFI seams.
# JavaScript test files are covered by the `typescript` adapter.
#
# [profiles.bun_ub]
# test_roots = [
#   "test/js/**/*.test.ts",
#   "test/js/**/*.test.js",
# ]
# bridge_hints = "ripr.bun.bridge.toml"
"#;

pub(crate) fn load_for_root(root: &Path) -> Result<RiprConfig, String> {
    let path = root.join(CONFIG_FILE_NAME);
    if !path.exists() {
        return default_config_for_root(root);
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|err| format!("read {} failed: {err}", path.display()))?;
    let mut config = parse_config(&text).map_err(|err| format!("{}: {err}", path.display()))?;
    config.source_path = Some(path);
    config.source_text = Some(text);
    Ok(config)
}

fn default_config_for_root(root: &Path) -> Result<RiprConfig, String> {
    let mut config = RiprConfig::default();
    if detect_python_project(root) {
        if !LanguageId::Python.is_available() {
            return Err(
                "Python project markers were detected, but this ripr binary was built without Cargo feature `lang-python`; use a Python-enabled ripr binary or add `ripr.toml` with `[languages] enabled = [\"rust\"]` to keep Python preview disabled"
                    .to_string(),
            );
        }
        if !config.languages.enabled.contains(&LanguageId::Python) {
            config.languages.enabled.push(LanguageId::Python);
        }
    }
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
        if let Some(languages) = raw.languages
            && let Some(enabled) = languages.enabled
        {
            config.languages.enabled = parse_languages_enabled(&enabled)?;
        }
        if let Some(profiles) = raw.profiles {
            config.profiles = parse_profiles(profiles)?;
        }
        Ok(config)
    }
}

fn parse_languages_enabled(values: &[String]) -> Result<Vec<LanguageId>, String> {
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        let language = match value.as_str() {
            "rust" => LanguageId::Rust,
            "typescript" => LanguageId::TypeScript,
            "python" => LanguageId::Python,
            "perl" => LanguageId::Perl,
            other => {
                return Err(format!(
                    "languages.enabled lists unknown language `{other}`; valid values are rust, typescript, python, perl"
                ));
            }
        };
        if parsed.contains(&language) {
            return Err(format!(
                "languages.enabled lists `{value}` more than once; remove the duplicate"
            ));
        }
        if !language.is_available() {
            return Err(format!(
                "languages.enabled lists `{value}`, but this ripr binary was built without Cargo feature `{}`",
                language.required_feature()
            ));
        }
        parsed.push(language);
    }
    Ok(parsed)
}

fn parse_profiles(raw: RawProfilesConfig) -> Result<ProfilesConfig, String> {
    Ok(ProfilesConfig {
        bun_ub: raw.bun_ub.map(parse_bun_ub_profile).transpose()?,
    })
}

fn parse_bun_ub_profile(raw: RawBunUbProfileConfig) -> Result<BunUbProfileConfig, String> {
    let test_roots = raw
        .test_roots
        .ok_or_else(|| "profiles.bun_ub.test_roots is required".to_string())?;
    if test_roots.is_empty() {
        return Err("profiles.bun_ub.test_roots must list at least one test root".to_string());
    }
    let mut parsed_roots = Vec::with_capacity(test_roots.len());
    for root in test_roots {
        let trimmed = root.trim();
        parse_relative_path("profiles.bun_ub.test_roots", trimmed)?;
        if parsed_roots.iter().any(|existing| existing == trimmed) {
            return Err(format!(
                "profiles.bun_ub.test_roots lists `{trimmed}` more than once; remove the duplicate"
            ));
        }
        parsed_roots.push(trimmed.to_string());
    }
    let bridge_hints = raw
        .bridge_hints
        .ok_or_else(|| "profiles.bun_ub.bridge_hints is required".to_string())
        .and_then(|path| parse_relative_path("profiles.bun_ub.bridge_hints", &path))?;
    Ok(BunUbProfileConfig {
        test_roots: parsed_roots,
        bridge_hints,
    })
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
    languages: Option<RawLanguagesConfig>,
    profiles: Option<RawProfilesConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLanguagesConfig {
    enabled: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProfilesConfig {
    bun_ub: Option<RawBunUbProfileConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBunUbProfileConfig {
    test_roots: Option<Vec<String>>,
    bridge_hints: Option<String>,
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
mod tests;
