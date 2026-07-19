//! Typed doctor/preflight report for machine-readable output (`ripr doctor --json`).
//!
//! See #1771 / #1614 / #1862. The report captures the core checks (root,
//! Cargo.toml, configuration, and tool availability) as typed `DoctorCheck`
//! values and leaves deeper sub-checks (languages, cache, Perl, and test
//! surfaces) for a follow-up projection. The structure here proves the dual
//! human/JSON projection without a massive one-shot refactor.
//!
//! `cli::commands::doctor` is an argv adapter only: it parses `--root` /
//! `--json`, calls into this module to evaluate the core checks and probe
//! tool availability, and prints either the JSON report or the human-prose
//! projection.

use crate::config::{CONFIG_FILE_NAME, RiprConfig, load_for_root};
use serde::Serialize;
use std::path::Path;

/// The single source of truth for which tools doctor probes for availability.
/// Both the evaluation (which actually spawns each tool to check it) and the
/// human-readable projection (which reads the resulting checks back out of
/// the report) iterate this list, so there is exactly one place that names
/// the probed tools.
pub(crate) const DOCTOR_TOOLS: [&str; 3] = ["git", "cargo", "rustc"];

/// The top-level doctor status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DoctorStatus {
    /// All top-level checks passed.
    Pass,
    /// One or more top-level checks failed.
    Fail,
}

/// A single typed doctor check (root, Cargo.toml, tool availability).
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct DoctorCheck {
    /// The check name (e.g. "root_directory", "cargo_toml", "tool_git").
    pub(crate) name: String,
    /// The check status.
    pub(crate) status: DoctorStatus,
    /// Human-readable evidence (e.g. "Cargo.toml found at /workspace").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) evidence: Option<String>,
}

/// A text-based section from a deeper check (languages, cache, etc.).
/// Typed checks replace these incrementally.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct DoctorSection {
    /// The section name (e.g. "detected_languages", "cache_status").
    pub(crate) name: String,
    /// The captured text output.
    pub(crate) lines: Vec<String>,
}

/// The full doctor report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct DoctorReport {
    pub(crate) schema_version: &'static str,
    pub(crate) tool: &'static str,
    pub(crate) root: String,
    pub(crate) status: DoctorStatus,
    pub(crate) checks: Vec<DoctorCheck>,
    pub(crate) sections: Vec<DoctorSection>,
}

impl DoctorReport {
    pub(crate) const SCHEMA_VERSION: &'static str = "0.1";

    pub(crate) fn new(root: &str) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            tool: "ripr",
            root: root.to_string(),
            status: DoctorStatus::Pass,
            checks: Vec::new(),
            sections: Vec::new(),
        }
    }

    /// Add a typed check and update the overall status.
    pub(crate) fn add_check(&mut self, name: &str, status: DoctorStatus, evidence: Option<String>) {
        if status == DoctorStatus::Fail {
            self.status = DoctorStatus::Fail;
        }
        self.checks.push(DoctorCheck {
            name: name.to_string(),
            status,
            evidence,
        });
    }

    /// Add a text-based section.
    #[cfg(test)]
    pub(crate) fn add_section(&mut self, name: &str, lines: Vec<String>) {
        self.sections.push(DoctorSection {
            name: name.to_string(),
            lines,
        });
    }

    /// Render the report as human-readable text (mirrors the existing prose output).
    #[cfg(test)]
    pub(crate) fn render_text(&self) -> String {
        let mut out = String::new();
        out.push_str("ripr doctor\n");
        out.push_str(&format!("- root: {}\n", self.root));
        for check in &self.checks {
            let icon = if check.status == DoctorStatus::Pass {
                "✓"
            } else {
                "!"
            };
            if let Some(evidence) = &check.evidence {
                out.push_str(&format!("{icon} {evidence}\n"));
            } else {
                out.push_str(&format!("{icon} {}\n", check.name));
            }
        }
        for section in &self.sections {
            for line in &section.lines {
                out.push_str(line);
                out.push('\n');
            }
        }
        match self.status {
            DoctorStatus::Pass => out.push_str("✓ doctor checks passed\n"),
            DoctorStatus::Fail => {
                out.push_str("! doctor checks failed; run `ripr doctor --help` for usage\n")
            }
        }
        out
    }

    /// Render the report as JSON.
    pub(crate) fn render_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map_err(|error| format!("failed to serialize doctor report: {error}"))
    }
}

/// Strip a `toml` parse error down to its first line.
///
/// `toml::de::Error`'s `Display` embeds the offending source excerpt and a
/// caret pointing at the failing column (e.g. `"...\n  |\n1 | [invalid\n  |
/// ^...\n"`). Echoing that excerpt into `ripr doctor --json` would leak
/// `ripr.toml` source text — which may contain repository-specific paths or
/// other content the caller did not intend to publish — into machine-readable
/// output that gets captured in CI logs, PR comments, and agent context
/// (RIPR-SPEC-0007, P2). The first line alone (path, "invalid ripr.toml",
/// parse location) is enough to act on and contains no source text, so
/// doctor's JSON evidence keeps only that line.
fn redact_config_parse_error(error: &str) -> String {
    error.lines().next().unwrap_or(error).trim().to_string()
}

/// Result of evaluating the doctor core checks, plus the raw config load
/// result for the human-readable projection (which prints the full local
/// error to the user's own terminal — not a machine-readable output surface,
/// so it is not subject to the RIPR-SPEC-0007 redaction above).
pub(crate) struct DoctorCoreEvaluation {
    pub(crate) report: DoctorReport,
    pub(crate) config: Result<RiprConfig, String>,
}

/// Evaluate the doctor core checks (root, Cargo.toml, config, tool
/// availability) and return just the typed report.
pub(crate) fn evaluate_doctor_core(root: &Path) -> DoctorReport {
    evaluate_doctor_core_with_config(root).report
}

/// Evaluate the doctor core checks and also return the raw config load
/// result, so the human-readable projection can print full local detail
/// without going through the redacted JSON evidence.
pub(crate) fn evaluate_doctor_core_with_config(root: &Path) -> DoctorCoreEvaluation {
    let mut report = DoctorReport::new(&root.display().to_string());
    if root.is_dir() {
        report.add_check(
            "root_directory",
            DoctorStatus::Pass,
            Some(format!("root directory exists at {}", root.display())),
        );
    } else {
        report.add_check(
            "root_directory",
            DoctorStatus::Fail,
            Some(format!(
                "root directory does not exist at {}",
                root.display()
            )),
        );
    }
    if root.join("Cargo.toml").exists() {
        report.add_check(
            "cargo_toml",
            DoctorStatus::Pass,
            Some(format!(
                "Cargo.toml found at {}",
                root.join("Cargo.toml").display()
            )),
        );
    } else {
        report.add_check(
            "cargo_toml",
            DoctorStatus::Fail,
            Some(format!("no Cargo.toml found at {}", root.display())),
        );
    }
    let config = load_for_root(root);
    match &config {
        Ok(config) => report.add_check(
            "config",
            DoctorStatus::Pass,
            Some(match config.source_path() {
                Some(path) => format!("loaded {} at {}", CONFIG_FILE_NAME, path.display()),
                None => format!("{CONFIG_FILE_NAME} not found; using built-in defaults"),
            }),
        ),
        Err(error) => report.add_check(
            "config",
            DoctorStatus::Fail,
            Some(redact_config_parse_error(error)),
        ),
    }
    for tool in DOCTOR_TOOLS {
        let (status, evidence) = doctor_tool_check(tool);
        report.add_check(&format!("tool_{tool}"), status, Some(evidence));
    }
    DoctorCoreEvaluation { report, config }
}

/// Probe a single tool's availability via `<tool> --version`.
pub(crate) fn doctor_tool_check(tool: &str) -> (DoctorStatus, String) {
    match std::process::Command::new(tool).arg("--version").output() {
        Ok(output) if output.status.success() => (
            DoctorStatus::Pass,
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ),
        _ => (DoctorStatus::Fail, format!("{tool} not available")),
    }
}

/// Translate the report's overall status into the doctor command's exit
/// result.
pub(crate) fn doctor_report_result(report: &DoctorReport) -> Result<(), String> {
    match report.status {
        DoctorStatus::Pass => Ok(()),
        DoctorStatus::Fail => Err("doctor found issues".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_test_dir(label: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "ripr-output-doctor-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn empty_report_is_pass() {
        let report = DoctorReport::new("/workspace");
        assert_eq!(report.status, DoctorStatus::Pass);
        assert!(report.checks.is_empty());
    }

    #[test]
    fn failed_check_flips_overall_status() {
        let mut report = DoctorReport::new("/workspace");
        report.add_check(
            "root_directory",
            DoctorStatus::Pass,
            Some("root exists".to_string()),
        );
        assert_eq!(report.status, DoctorStatus::Pass);
        report.add_check(
            "cargo_toml",
            DoctorStatus::Fail,
            Some("no Cargo.toml".to_string()),
        );
        assert_eq!(report.status, DoctorStatus::Fail);
    }

    #[test]
    fn render_text_shows_pass_and_fail_checks() {
        let mut report = DoctorReport::new("/workspace");
        report.add_check(
            "root_directory",
            DoctorStatus::Pass,
            Some("root exists".to_string()),
        );
        report.add_check(
            "cargo_toml",
            DoctorStatus::Fail,
            Some("no Cargo.toml".to_string()),
        );
        report.add_section("guidance", vec!["run ripr doctor --help".to_string()]);
        let text = report.render_text();
        assert!(text.contains("✓ root exists"));
        assert!(text.contains("! no Cargo.toml"));
        assert!(text.contains("run ripr doctor --help"));
        assert!(text.contains("! doctor checks failed"));
    }

    #[test]
    fn render_text_names_checks_without_evidence_and_passes() {
        let mut report = DoctorReport::new("/workspace");
        report.add_check("config", DoctorStatus::Pass, None);
        let text = report.render_text();
        assert!(text.contains("✓ config"));
        assert!(text.contains("✓ doctor checks passed"));
    }

    #[test]
    fn render_json_produces_valid_json() -> Result<(), String> {
        let mut report = DoctorReport::new("/workspace");
        report.add_check(
            "root_directory",
            DoctorStatus::Pass,
            Some("root exists".to_string()),
        );
        report.add_section("cache", vec!["cache: target/ripr/cache".to_string()]);
        let json = report.render_json()?;
        let parsed: serde_json::Value =
            serde_json::from_str(&json).map_err(|e| format!("invalid JSON: {e}"))?;
        assert_eq!(parsed["schema_version"], "0.1");
        assert_eq!(parsed["tool"], "ripr");
        assert_eq!(parsed["status"], "pass");
        assert_eq!(parsed["checks"][0]["name"], "root_directory");
        assert_eq!(parsed["checks"][0]["status"], "pass");
        assert_eq!(parsed["sections"][0]["name"], "cache");
        Ok(())
    }

    #[test]
    fn sections_are_optional() -> Result<(), String> {
        let report = DoctorReport::new("/workspace");
        let json = report.render_json()?;
        let parsed: serde_json::Value =
            serde_json::from_str(&json).map_err(|e| format!("invalid JSON: {e}"))?;
        assert!(parsed["sections"].is_array());
        assert_eq!(parsed["sections"].as_array().map(Vec::len), Some(0));
        Ok(())
    }

    /// RIPR-SPEC-0007 (P2): a malformed `ripr.toml` must never echo its
    /// source excerpt into the doctor report's evidence. The evidence should
    /// still be actionable (it must name the parse failure and its
    /// location), just not reproduce the offending source line.
    #[test]
    fn config_check_redacts_source_excerpt_from_malformed_toml() -> Result<(), String> {
        let dir = unique_test_dir("redact-config");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        std::fs::write(dir.join(CONFIG_FILE_NAME), "[invalid\n")
            .map_err(|err| format!("write invalid config: {err}"))?;

        let report = evaluate_doctor_core(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        let config_check = report
            .checks
            .iter()
            .find(|check| check.name == "config")
            .ok_or_else(|| "missing config check".to_string())?;
        assert_eq!(config_check.status, DoctorStatus::Fail);
        let evidence = config_check
            .evidence
            .as_deref()
            .ok_or_else(|| "expected evidence for invalid config".to_string())?;

        assert!(
            !evidence.contains("[invalid"),
            "evidence must not echo the config source line: {evidence:?}"
        );
        assert!(
            !evidence.contains('\n'),
            "evidence must be a single line (no source excerpt/caret): {evidence:?}"
        );
        assert!(
            evidence.contains("invalid ripr.toml"),
            "evidence must still name the failure: {evidence:?}"
        );
        assert!(
            evidence.to_lowercase().contains("line"),
            "evidence must still point at a parse location: {evidence:?}"
        );
        Ok(())
    }

    /// Deterministic missing-tool assertion: probing a guaranteed-absent
    /// absolute path must fail closed with actionable evidence, independent
    /// of what happens to be (or not be) on the host's PATH.
    #[test]
    fn doctor_tool_check_fails_closed_for_guaranteed_missing_tool() {
        let missing = std::env::temp_dir().join(format!(
            "ripr-doctor-missing-tool-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let missing_str = missing.to_string_lossy().into_owned();
        let (status, evidence) = doctor_tool_check(&missing_str);
        assert_eq!(status, DoctorStatus::Fail);
        assert!(
            evidence.ends_with("not available"),
            "unexpected evidence for missing tool: {evidence:?}"
        );
    }
}
