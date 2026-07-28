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
use std::time::{Duration, Instant};

/// The single source of truth for which tools doctor probes for availability.
/// Both the evaluation (which actually spawns each tool to check it) and the
/// human-readable projection (which reads the resulting checks back out of
/// the report) iterate this list, so there is exactly one place that names
/// the probed tools.
pub(crate) const DOCTOR_TOOLS: [&str; 3] = ["git", "cargo", "rustc"];

const MINIMUM_RUSTC_VERSION: &str = env!("CARGO_PKG_RUST_VERSION");

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RustcVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

impl std::fmt::Display for RustcVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

fn parse_rustc_version(output: &str) -> Option<RustcVersion> {
    let version_token = output
        .trim_start()
        .strip_prefix("rustc ")?
        .split_whitespace()
        .next()?;
    let mut components = version_token.split('.');
    let major = components.next()?.parse().ok()?;
    let minor = components.next()?.parse().ok()?;
    let patch = components.next()?.split(['-', '+']).next()?.parse().ok()?;
    Some(RustcVersion {
        major,
        minor,
        patch,
    })
}

fn minimum_rustc_version() -> Option<RustcVersion> {
    let mut components = MINIMUM_RUSTC_VERSION.split('.');
    let major = components.next()?.parse().ok()?;
    let minor = components.next()?.parse().ok()?;
    let patch = components
        .next()
        .unwrap_or("0")
        .split(['-', '+'])
        .next()?
        .parse()
        .ok()?;
    Some(RustcVersion {
        major,
        minor,
        patch,
    })
}

fn validate_rustc_version(output: &str) -> Result<(), String> {
    let minimum = minimum_rustc_version().ok_or_else(|| {
        format!(
            "declared package rust-version `{MINIMUM_RUSTC_VERSION}` could not be parsed; update Cargo.toml"
        )
    })?;
    let version = parse_rustc_version(output).ok_or_else(|| {
        format!(
            "rustc version could not be parsed from `{}`; install Rust {minimum}+",
            output.trim()
        )
    })?;
    if version < minimum {
        return Err(format!(
            "rustc {version} is below the minimum supported Rust version {minimum}; run `rustup update stable` or install Rust {minimum}+"
        ));
    }
    Ok(())
}

/// How long a tool probe may run before it is terminated (#2183 review): a
/// broken or malicious shim must not hang `ripr doctor` forever.
///
/// Residual, documented (#2183 review): the deadline terminates the spawned
/// process itself, not a whole process tree. A shim that *detaches*
/// (double-fork/setsid) work can leave that work running after the probe
/// returns. Process-group termination needs either `unsafe` (forbidden in
/// this crate) or a new dependency, and doctor returns bounded regardless —
/// so the bounded-probe contract is honored while the detached-descendant
/// case is accepted here rather than hidden.
const DOCTOR_TOOL_TIMEOUT: Duration = Duration::from_secs(5);

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
    /// Enabled language wire strings from the effective config (#2072):
    /// the typed surface the generated CI consumes instead of parsing the
    /// human "Enabled languages:" line.
    pub(crate) languages: Vec<String>,
}

impl DoctorReport {
    pub(crate) const SCHEMA_VERSION: &'static str = "0.2";

    pub(crate) fn new(root: &str) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            tool: "ripr",
            root: root.to_string(),
            status: DoctorStatus::Pass,
            checks: Vec::new(),
            sections: Vec::new(),
            languages: Vec::new(),
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
        let (status, evidence) = doctor_tool_check_for_root(tool, root);
        report.add_check(&format!("tool_{tool}"), status, Some(evidence));
    }
    // Typed language surface for generated CI (#2072): mirror exactly the
    // effective enabled set the human projection prints.
    if let Ok(config) = &config {
        report.languages = config
            .languages()
            .enabled()
            .iter()
            .map(|language| language.as_str().to_string())
            .collect();
    }
    DoctorCoreEvaluation { report, config }
}

/// Probe a single tool's availability via `<tool> --version`.
/// Probe a tool that must NOT load project configuration (#2183 review,
/// CWE-829): `yarn --version` run in a repository checkout executes
/// repo-controlled code when the repo pins `.yarnrc.yml`/`yarnPath`, so
/// merely running `ripr doctor` in a hostile checkout would execute it.
/// The probe runs from the OS temp dir with config resolution disabled.
pub(crate) fn doctor_tool_check_isolated(tool: &str) -> (DoctorStatus, String) {
    let mut command = doctor_tool_command(tool);
    command
        .current_dir(std::env::temp_dir())
        .env("YARN_IGNORE_PATH", "1");
    doctor_tool_check_with_command(tool, command, DOCTOR_TOOL_TIMEOUT, None).into_public()
}

fn doctor_tool_command(tool: &str) -> std::process::Command {
    std::process::Command::new(tool)
}

pub(crate) fn doctor_tool_check(tool: &str) -> (DoctorStatus, String) {
    doctor_tool_check_with_timeout(tool, DOCTOR_TOOL_TIMEOUT)
}

fn doctor_tool_check_for_root(tool: &str, root: &Path) -> (DoctorStatus, String) {
    if tool == "rustc" {
        doctor_tool_check_with_timeout_result_at(tool, DOCTOR_TOOL_TIMEOUT, Some(root))
            .into_public()
    } else {
        doctor_tool_check(tool)
    }
}

fn doctor_tool_check_with_timeout(tool: &str, timeout: Duration) -> (DoctorStatus, String) {
    doctor_tool_check_with_timeout_result(tool, timeout).into_public()
}

fn doctor_tool_check_with_timeout_result(tool: &str, timeout: Duration) -> DoctorToolCheckResult {
    doctor_tool_check_with_timeout_result_at(tool, timeout, None)
}

fn doctor_tool_check_with_timeout_result_at(
    tool: &str,
    timeout: Duration,
    root: Option<&Path>,
) -> DoctorToolCheckResult {
    doctor_tool_check_with_command(tool, doctor_tool_command(tool), timeout, root)
}

fn doctor_tool_check_with_command(
    tool: &str,
    mut command: std::process::Command,
    timeout: Duration,
    root: Option<&Path>,
) -> DoctorToolCheckResult {
    command.arg("--version");
    if let Some(root) = root {
        command.current_dir(root);
    }
    match run_doctor_tool(command, timeout) {
        Ok(output) if output.status.success() => doctor_tool_check_success(tool, &output.stdout),
        Err(DoctorToolRunError::TimedOut) => {
            DoctorToolCheckResult::failure(doctor_timeout_evidence(tool, timeout))
        }
        Err(DoctorToolRunError::Spawn(kind)) => doctor_spawn_failure(tool, kind),
        _ => DoctorToolCheckResult::failure(format!("{tool} not available")),
    }
}

fn doctor_tool_check_success(tool: &str, stdout: &[u8]) -> DoctorToolCheckResult {
    let evidence = String::from_utf8_lossy(stdout).trim().to_string();
    if tool != "rustc" {
        return DoctorToolCheckResult::pass(evidence);
    }
    match validate_rustc_version(&evidence) {
        Ok(()) => DoctorToolCheckResult::pass(evidence),
        Err(error) => DoctorToolCheckResult::failure(error),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct DoctorToolCheckResult {
    status: DoctorStatus,
    evidence: String,
    retryable_launch_failure: bool,
}

impl DoctorToolCheckResult {
    fn pass(evidence: String) -> Self {
        Self {
            status: DoctorStatus::Pass,
            evidence,
            retryable_launch_failure: false,
        }
    }

    fn failure(evidence: String) -> Self {
        Self {
            status: DoctorStatus::Fail,
            evidence,
            retryable_launch_failure: false,
        }
    }

    fn into_public(self) -> (DoctorStatus, String) {
        (self.status, self.evidence)
    }
}

fn doctor_spawn_failure(tool: &str, kind: std::io::ErrorKind) -> DoctorToolCheckResult {
    DoctorToolCheckResult {
        status: DoctorStatus::Fail,
        evidence: if kind == std::io::ErrorKind::NotFound {
            format!("{tool} not available")
        } else {
            format!("{tool} could not be launched: {kind:?}")
        },
        retryable_launch_failure: doctor_spawn_failure_is_retryable(kind),
    }
}

fn doctor_spawn_failure_is_retryable(kind: std::io::ErrorKind) -> bool {
    // The transient launch-failure class (#2242): resource exhaustion or
    // exec races under load. Observed in the wild as both WouldBlock and
    // ExecutableFileBusy under full-suite parallelism. NotFound (missing
    // tool) and PermissionDenied (not executable) are persistent and must
    // fail immediately.
    matches!(
        kind,
        std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::ExecutableFileBusy
            | std::io::ErrorKind::OutOfMemory
    )
}

fn doctor_timeout_evidence(tool: &str, timeout: Duration) -> String {
    let milliseconds = timeout.as_millis();
    if milliseconds < 1_000 || !milliseconds.is_multiple_of(1_000) {
        format!("{tool} timed out after {milliseconds}ms")
    } else {
        format!("{tool} timed out after {}s", timeout.as_secs())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DoctorToolRunError {
    Spawn(std::io::ErrorKind),
    Wait,
    TimedOut,
}

fn run_doctor_tool(
    mut command: std::process::Command,
    timeout: Duration,
) -> Result<std::process::Output, DoctorToolRunError> {
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|err| DoctorToolRunError::Spawn(err.kind()))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|_err| DoctorToolRunError::Wait);
            }
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(DoctorToolRunError::TimedOut);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(DoctorToolRunError::Wait);
            }
        }
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
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_test_dir(label: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "ripr-output-doctor-{label}-{}-{stamp}-{}",
            std::process::id(),
            TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
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
    fn rustc_version_check_fails_below_msrv_and_passes_supported_versions() -> Result<(), String> {
        let cases = [
            (
                "rustc 1.80.0 (abc 2024-01-01)",
                DoctorStatus::Fail,
                "below the minimum supported Rust version",
            ),
            ("rustc 1.95.0 (abc 2026-04-14)", DoctorStatus::Pass, ""),
            (
                "rustc 1.96.1-nightly (abc 2026-05-01)",
                DoctorStatus::Pass,
                "",
            ),
        ];
        for (evidence, expected_status, expected_fragment) in cases {
            let result = doctor_tool_check_success("rustc", evidence.as_bytes());
            if result.status != expected_status {
                return Err(format!(
                    "unexpected status for {evidence:?}: {:?}",
                    result.status
                ));
            }
            if !result.evidence.contains(expected_fragment) {
                return Err(format!(
                    "missing expected evidence for {evidence:?}: {:?}",
                    result.evidence
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn rustc_version_check_fails_closed_for_malformed_output() -> Result<(), String> {
        let result = doctor_tool_check_success("rustc", b"rustc unavailable");
        if result.status != DoctorStatus::Fail {
            return Err(format!(
                "malformed rustc output unexpectedly passed: {result:?}"
            ));
        }
        if !result.evidence.contains("could not be parsed") {
            return Err(format!(
                "unexpected malformed-output evidence: {:?}",
                result.evidence
            ));
        }
        Ok(())
    }

    #[test]
    fn rustc_version_parser_covers_invalid_prefix_and_components() {
        for output in [
            "",
            "cargo 1.95.0",
            "rustc",
            "rustc ",
            "rustc x.95.0",
            "rustc 1.x.0",
            "rustc 1.95",
            "rustc 1.95.x",
            "rustc 1.95.-nightly",
        ] {
            assert!(
                parse_rustc_version(output).is_none(),
                "malformed rustc output unexpectedly parsed: {output:?}"
            );
        }
        assert!(parse_rustc_version("rustc 1.95.0-nightly").is_some());
        assert_eq!(
            doctor_tool_check_success("cargo", b"cargo 1.95.0").status,
            DoctorStatus::Pass
        );
    }

    #[test]
    fn rustc_doctor_probes_apply_the_version_gate() {
        let (status, evidence) = doctor_tool_check("rustc");
        assert_eq!(status, DoctorStatus::Pass, "{evidence}");
        assert!(evidence.starts_with("rustc "), "{evidence}");

        let (isolated_status, isolated_evidence) = doctor_tool_check_isolated("rustc");
        assert_eq!(isolated_status, DoctorStatus::Pass, "{isolated_evidence}");
        assert!(
            isolated_evidence.starts_with("rustc "),
            "{isolated_evidence}"
        );
    }

    #[test]
    fn rustc_doctor_probe_from_selected_root_applies_the_version_gate() {
        let (status, evidence) = doctor_tool_check_for_root("rustc", &std::env::temp_dir());
        assert_eq!(status, DoctorStatus::Pass, "{evidence}");
        assert!(evidence.starts_with("rustc "), "{evidence}");
    }

    #[cfg(unix)]
    #[test]
    fn doctor_rustc_probe_uses_selected_root() -> Result<(), String> {
        let dir = unique_test_dir("selected-root-rustc");
        let selected_root = dir.join("selected-root");
        std::fs::create_dir_all(&selected_root).map_err(|err| format!("create root: {err}"))?;
        let shim = publish_doctor_test_tool(
            &dir,
            "rustc-root-probe",
            "#!/bin/sh\ncase \"$PWD\" in\n  *selected-root) printf 'rustc 1.94.0 (target-root)\\n' ;;\n  *) printf 'rustc 1.96.0 (caller-root)\\n' ;;\nesac\n",
        )?;

        let result = doctor_tool_check_with_command(
            "rustc",
            doctor_tool_command(
                shim.to_str()
                    .ok_or_else(|| "shim path is not UTF-8".to_string())?,
            ),
            DOCTOR_TOOL_TIMEOUT,
            Some(&selected_root),
        );
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result.status, DoctorStatus::Fail);
        assert!(
            result
                .evidence
                .contains("below the minimum supported Rust version")
        );
        Ok(())
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
    fn doctor_timeout_evidence_preserves_fractional_durations() {
        assert_eq!(
            doctor_timeout_evidence("probe", std::time::Duration::from_millis(250)),
            "probe timed out after 250ms"
        );
        assert_eq!(
            doctor_timeout_evidence("probe", std::time::Duration::from_millis(1_500)),
            "probe timed out after 1500ms"
        );
        assert_eq!(
            doctor_timeout_evidence("probe", std::time::Duration::from_secs(5)),
            "probe timed out after 5s"
        );
    }

    #[test]
    fn doctor_json_carries_enabled_languages() -> Result<(), String> {
        // #2072: generated CI consumes the typed languages surface.
        // Hermetic: a temp root with a configured enabled list, so the
        // test proves config propagation, not just built-in defaults
        // (#2182 review).
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root =
            std::env::temp_dir().join(format!("ripr-doctor-lang-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"probe\"\n")
            .map_err(|err| format!("write Cargo.toml: {err}"))?;
        std::fs::write(
            root.join(crate::config::CONFIG_FILE_NAME),
            "[languages]\nenabled = [\"rust\", \"python\"]\n",
        )
        .map_err(|err| format!("write config: {err}"))?;

        let report = evaluate_doctor_core(&root);
        let json = report.render_json()?;
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        let parsed: serde_json::Value =
            serde_json::from_str(&json).map_err(|e| format!("invalid JSON: {e}"))?;
        let languages = parsed["languages"]
            .as_array()
            .ok_or_else(|| "languages must be an array".to_string())?;
        assert!(
            languages.iter().any(|language| language == "rust"),
            "configured set must include rust: {languages:?}"
        );
        assert!(
            languages.iter().any(|language| language == "python"),
            "configured set must include python: {languages:?}"
        );
        Ok(())
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
        assert_eq!(parsed["schema_version"], "0.2");
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

    /// Publish a test executable only after its writable file descriptor is
    /// closed, so the pathname handed to `exec` can never be concurrently open
    /// for writing by this fixture.
    #[cfg(unix)]
    fn publish_doctor_test_tool(
        directory: &Path,
        name: &str,
        script: &str,
    ) -> Result<std::path::PathBuf, String> {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;

        let published = directory.join(name);
        let staged = directory.join(format!(".{name}.staged"));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)
            .map_err(|err| format!("create staged tool: {err}"))?;
        file.write_all(script.as_bytes())
            .map_err(|err| format!("write staged tool: {err}"))?;
        file.set_permissions(std::fs::Permissions::from_mode(0o755))
            .map_err(|err| format!("chmod staged tool: {err}"))?;
        file.sync_all()
            .map_err(|err| format!("sync staged tool: {err}"))?;
        drop(file);
        std::fs::rename(&staged, &published)
            .map_err(|err| format!("publish staged tool: {err}"))?;
        Ok(published)
    }

    /// Probe a just-published shim, retrying only a retryable launch failure.
    ///
    /// Atomic publication (#2242/#2378) removes the writer this process holds,
    /// but it cannot remove host-level exec contention. `ETXTBSY`
    /// (`ExecutableFileBusy`) is raised when *any* process holds the file open
    /// for writing: under full-suite parallelism another thread can `fork`
    /// while some writable descriptor is open, and that descriptor keeps the
    /// file un-executable until the child reaches its own `exec`. `FD_CLOEXEC`
    /// closes the descriptor *at* exec — it does not close the fork/exec
    /// window.
    ///
    /// Every test that publishes a tool and immediately executes it is exposed
    /// to this, so the retry lives here rather than at one call site. The
    /// bound is deliberate: only `retryable_launch_failure` is retried, so a
    /// real verdict — a timeout, a non-zero exit, a tool that genuinely did not
    /// execute — is never retried into a pass.
    #[cfg(unix)]
    fn probe_published_tool(tool: &str, timeout: Duration) -> (DoctorStatus, String) {
        let mut launch_attempt = 0usize;
        loop {
            launch_attempt += 1;
            let outcome = doctor_tool_check_with_timeout_result(tool, timeout);
            let retryable_launch_failure = outcome.retryable_launch_failure;
            let result = outcome.into_public();
            if launch_attempt >= 3 || !retryable_launch_failure {
                break result;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    #[cfg(unix)]
    #[test]
    fn doctor_test_tool_is_closed_before_its_executable_path_is_published() -> Result<(), String> {
        let dir = unique_test_dir("atomic-publish");
        std::fs::create_dir(&dir).map_err(|err| format!("create dir: {err}"))?;
        let shim = publish_doctor_test_tool(&dir, "ripr-atomic-probe-tool", "#!/bin/sh\nexit 0\n")?;
        let staged = dir.join(".ripr-atomic-probe-tool.staged");
        if staged.exists() {
            return Err("staged tool remained after atomic publication".to_string());
        }
        let shim_text = shim.to_str().ok_or("shim path is not utf-8")?;
        // #2441: this test publishes and immediately execs, exactly like its
        // sibling below, so it needs the same bounded launch retry. Without it
        // the test failed intermittently on CI with `ExecutableFileBusy` while
        // proving nothing about the publication contract it exists to check.
        let (status, evidence) = probe_published_tool(shim_text, Duration::from_secs(1));
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove dir: {err}"))?;
        if status != DoctorStatus::Pass {
            return Err(format!(
                "atomically published tool did not execute: {evidence}"
            ));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn doctor_tool_check_times_out_a_hanging_tool_100_times() -> Result<(), String> {
        // #2183 review: a shim that blocks forever must not hang doctor;
        // the probe terminates it at the deadline and names the timeout. Run
        // the materialization and probe repeatedly to catch visibility and
        // cleanup races under full-suite parallelism.
        for attempt in 0..100 {
            let dir = unique_test_dir("timeout");
            std::fs::create_dir(&dir).map_err(|err| format!("create dir: {err}"))?;
            let sleep = ["/usr/bin/sleep", "/bin/sleep"]
                .into_iter()
                .find(|candidate| std::path::Path::new(candidate).is_file())
                .ok_or("no portable Unix sleep utility found")?;
            let script = format!("#!/bin/sh\nexec {sleep} 60\n");
            let shim = publish_doctor_test_tool(&dir, "ripr-hanging-probe-tool", &script)?;

            let start = std::time::Instant::now();
            let shim_text = shim.to_str().ok_or("shim path is not utf-8")?;
            // #2242/#2378: publish the executable pathname only after the
            // writer is closed. The bounded retry for independent host-level
            // exec contention now lives in `probe_published_tool`, shared with
            // the atomic-publication test (#2441); a real timeout result is
            // still never retried.
            let (status, evidence) = probe_published_tool(shim_text, Duration::from_millis(250));
            let elapsed = start.elapsed();

            std::fs::remove_dir_all(&dir).map_err(|err| format!("remove dir: {err}"))?;
            if status != DoctorStatus::Fail {
                return Err(format!(
                    "attempt {attempt}: hanging tool unexpectedly passed"
                ));
            }
            if evidence != format!("{shim_text} timed out after 250ms") {
                return Err(format!(
                    "attempt {attempt}: expected a 250ms timeout, got: {evidence}"
                ));
            }
            if elapsed >= std::time::Duration::from_secs(30) {
                return Err(format!(
                    "attempt {attempt}: hanging tool was not terminated: {elapsed:?}"
                ));
            }
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn doctor_tool_check_names_non_missing_launch_failure() -> Result<(), String> {
        let dir = unique_test_dir("launch-failure");
        std::fs::create_dir(&dir).map_err(|err| format!("create directory tool: {err}"))?;
        let tool = dir.to_str().ok_or("directory path is not utf-8")?;
        let (status, evidence) = doctor_tool_check(tool);
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove directory tool: {err}"))?;
        if status != DoctorStatus::Fail {
            return Err("directory tool unexpectedly passed".to_string());
        }
        if !evidence.contains("could not be launched") {
            return Err(format!("launch failure was misclassified: {evidence}"));
        }
        Ok(())
    }

    #[test]
    fn doctor_spawn_failure_retry_policy_covers_transient_kinds() -> Result<(), String> {
        for (kind, expected) in [
            (std::io::ErrorKind::WouldBlock, true),
            (std::io::ErrorKind::ExecutableFileBusy, true),
            (std::io::ErrorKind::OutOfMemory, true),
            (std::io::ErrorKind::PermissionDenied, false),
            (std::io::ErrorKind::NotFound, false),
        ] {
            if doctor_spawn_failure_is_retryable(kind) != expected {
                return Err(format!("unexpected retry policy for {kind:?}"));
            }
        }
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

    #[test]
    fn doctor_tool_runner_times_out_and_reaps_child() -> Result<(), String> {
        let mut command = doctor_tool_command(if cfg!(windows) { "powershell" } else { "sh" });
        #[cfg(windows)]
        command.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 5"]);
        #[cfg(not(windows))]
        command.args(["-c", "sleep 5"]);

        match run_doctor_tool(command, Duration::from_millis(20)) {
            Err(DoctorToolRunError::TimedOut) => Ok(()),
            Err(error) => Err(format!("expected timeout, got {error:?}")),
            Ok(_) => Err("timed-out tool unexpectedly completed".into()),
        }
    }
}
