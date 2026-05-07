use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

const REPORT_WORK_DIR: &str = "target/ripr/release-readiness";
const INSTALL_ROOT: &str = "target/ripr/release-readiness/install";
const PILOT_OUT: &str = "target/ripr/release-readiness/pilot";
const OUTCOME_OUT: &str = "target/ripr/release-readiness/targeted-test-outcome.json";
const AGENT_VERIFY_OUT: &str = "target/ripr/release-readiness/agent-verify.json";
const BEFORE_EXPOSURE: &str =
    "fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json";
const AFTER_EXPOSURE: &str =
    "fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json";

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReleaseReadinessArgs {
    version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReleaseReadinessReport {
    version: String,
    status: String,
    checks: Vec<ReleaseReadinessCheck>,
    next_commands: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReleaseReadinessCheck {
    id: &'static str,
    status: String,
    required: bool,
    command: String,
    summary: String,
    artifacts: Vec<String>,
    details: Vec<String>,
}

#[derive(Clone, Debug)]
struct CommandResult {
    status: Option<i32>,
    success: bool,
    stdout: String,
    stderr: String,
}

pub(crate) fn release_readiness(args: &[String]) -> Result<(), String> {
    let args = parse_release_readiness_args(args)?;
    fs::create_dir_all(REPORT_WORK_DIR)
        .map_err(|err| format!("failed to create {REPORT_WORK_DIR}: {err}"))?;
    let report = build_release_readiness_report(&args.version);
    let json = release_readiness_json(&report)?;
    crate::write_report("release-readiness.json", &json)?;
    crate::write_report("release-readiness.md", &release_readiness_markdown(&report))?;
    if report.status == "fail" {
        return Err(
            "release readiness failed; see target/ripr/reports/release-readiness.md".to_string(),
        );
    }
    Ok(())
}

fn parse_release_readiness_args(args: &[String]) -> Result<ReleaseReadinessArgs, String> {
    let mut version: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--version" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(release_readiness_usage());
                };
                version = Some(value.clone());
                index += 2;
            }
            "--help" | "-h" => return Err(release_readiness_usage()),
            other => {
                return Err(format!(
                    "unknown release-readiness argument {other:?}\n{}",
                    release_readiness_usage()
                ));
            }
        }
    }
    let Some(version) = version else {
        return Err(release_readiness_usage());
    };
    if version.trim().is_empty() {
        return Err(release_readiness_usage());
    }
    Ok(ReleaseReadinessArgs { version })
}

fn release_readiness_usage() -> String {
    "Usage: cargo xtask release-readiness --version <version>".to_string()
}

fn build_release_readiness_report(version: &str) -> ReleaseReadinessReport {
    let crate_version = read_crate_version(Path::new("crates/ripr/Cargo.toml"));
    let clean_tree = git_worktree_is_clean();
    let installed_binary = installed_ripr_binary();
    let checks = vec![
        package_list_check(version, crate_version.as_deref(), clean_tree.clone()),
        publish_dry_run_check(version, crate_version.as_deref(), clean_tree),
        path_install_check(),
        installed_command_surface_check(&installed_binary),
        pilot_fixture_check(&installed_binary),
        outcome_fixture_check(&installed_binary),
        agent_verify_fixture_check(&installed_binary),
        repo_exposure_latency_check(),
        lsp_cockpit_check(),
        github_workflow_check(&installed_binary),
        vsix_packaging_check(),
        known_limits_docs_check(),
    ];
    let status = release_readiness_status(&checks).to_string();
    let next_commands = release_readiness_next_commands(&checks, version);
    ReleaseReadinessReport {
        version: version.to_string(),
        status,
        checks,
        next_commands,
    }
}

fn package_list_check(
    version: &str,
    crate_version: Option<&str>,
    clean_tree: Result<bool, String>,
) -> ReleaseReadinessCheck {
    release_gate_check(
        "package-list",
        "cargo package -p ripr --list",
        version,
        crate_version,
        clean_tree,
        || run_command("cargo", &["package", "-p", "ripr", "--list"]),
    )
}

fn publish_dry_run_check(
    version: &str,
    crate_version: Option<&str>,
    clean_tree: Result<bool, String>,
) -> ReleaseReadinessCheck {
    release_gate_check(
        "publish-dry-run",
        "cargo publish -p ripr --dry-run",
        version,
        crate_version,
        clean_tree,
        || run_command("cargo", &["publish", "-p", "ripr", "--dry-run"]),
    )
}

fn release_gate_check<F>(
    id: &'static str,
    command: &str,
    version: &str,
    crate_version: Option<&str>,
    clean_tree: Result<bool, String>,
    run: F,
) -> ReleaseReadinessCheck
where
    F: FnOnce() -> Result<CommandResult, String>,
{
    let Some(crate_version) = crate_version else {
        return readiness_check(
            id,
            "not_run",
            false,
            command,
            "crate version could not be read; release-prep should run this gate explicitly",
            Vec::new(),
            Vec::new(),
        );
    };
    if crate_version != version {
        return readiness_check(
            id,
            "not_run",
            false,
            command,
            "requested release version does not match the crate version yet",
            Vec::new(),
            vec![format!(
                "requested version: {version}; crates/ripr version: {crate_version}"
            )],
        );
    }
    match clean_tree {
        Ok(true) => match run() {
            Ok(result) if result.success => readiness_check(
                id,
                "pass",
                true,
                command,
                "release gate passed",
                Vec::new(),
                command_details(&result),
            ),
            Ok(result) => readiness_check(
                id,
                "fail",
                true,
                command,
                "release gate failed",
                Vec::new(),
                command_details(&result),
            ),
            Err(err) => readiness_check(
                id,
                "fail",
                true,
                command,
                "release gate could not run",
                Vec::new(),
                vec![err],
            ),
        },
        Ok(false) => readiness_check(
            id,
            "not_run",
            false,
            command,
            "dirty tree; release-prep should rerun this on the committed version bump",
            Vec::new(),
            Vec::new(),
        ),
        Err(err) => readiness_check(
            id,
            "not_run",
            false,
            command,
            "git worktree state could not be verified",
            Vec::new(),
            vec![err],
        ),
    }
}

fn path_install_check() -> ReleaseReadinessCheck {
    let command =
        format!("cargo install --path crates/ripr --locked --root {INSTALL_ROOT} --force");
    match run_command(
        "cargo",
        &[
            "install",
            "--path",
            "crates/ripr",
            "--locked",
            "--root",
            INSTALL_ROOT,
            "--force",
        ],
    ) {
        Ok(result) if result.success => readiness_check(
            "path-install",
            "pass",
            true,
            &command,
            "path-installed ripr binary is available",
            vec![crate::normalize_path(&installed_ripr_binary())],
            command_details(&result),
        ),
        Ok(result) => readiness_check(
            "path-install",
            "fail",
            true,
            &command,
            "path install failed",
            vec![crate::normalize_path(&installed_ripr_binary())],
            command_details(&result),
        ),
        Err(err) => readiness_check(
            "path-install",
            "fail",
            true,
            &command,
            "path install could not run",
            vec![crate::normalize_path(&installed_ripr_binary())],
            vec![err],
        ),
    }
}

fn installed_command_surface_check(binary: &Path) -> ReleaseReadinessCheck {
    let command = format!("{} --help", crate::normalize_path(binary));
    if !binary.exists() {
        return readiness_check(
            "installed-command-surface",
            "fail",
            true,
            &command,
            "installed ripr binary is missing",
            vec![crate::normalize_path(binary)],
            Vec::new(),
        );
    }
    match run_command_path(binary, &["--help"]) {
        Ok(result) if result.success => {
            let required = [
                "ripr pilot",
                "ripr outcome",
                "ripr calibrate cargo-mutants",
                "ripr agent verify",
            ];
            let missing = required
                .iter()
                .filter(|needle| !result.stdout.contains(**needle))
                .map(|needle| (*needle).to_string())
                .collect::<Vec<_>>();
            if missing.is_empty() {
                readiness_check(
                    "installed-command-surface",
                    "pass",
                    true,
                    &command,
                    "installed binary exposes the 0.4 public loop commands",
                    vec![crate::normalize_path(binary)],
                    Vec::new(),
                )
            } else {
                readiness_check(
                    "installed-command-surface",
                    "fail",
                    true,
                    &command,
                    "installed binary is missing expected public loop commands",
                    vec![crate::normalize_path(binary)],
                    vec![format!("missing: {}", missing.join(", "))],
                )
            }
        }
        Ok(result) => readiness_check(
            "installed-command-surface",
            "fail",
            true,
            &command,
            "installed binary help failed",
            vec![crate::normalize_path(binary)],
            command_details(&result),
        ),
        Err(err) => readiness_check(
            "installed-command-surface",
            "fail",
            true,
            &command,
            "installed binary help could not run",
            vec![crate::normalize_path(binary)],
            vec![err],
        ),
    }
}

fn pilot_fixture_check(binary: &Path) -> ReleaseReadinessCheck {
    let command = format!(
        "{} pilot --root fixtures/boundary_gap/input --out {PILOT_OUT} --timeout-ms 30000",
        crate::normalize_path(binary)
    );
    if !binary.exists() {
        return readiness_check(
            "pilot-boundary-fixture",
            "fail",
            true,
            &command,
            "installed binary is missing",
            Vec::new(),
            Vec::new(),
        );
    }
    let _ = fs::remove_dir_all(PILOT_OUT);
    match run_command_path(
        binary,
        &[
            "pilot",
            "--root",
            "fixtures/boundary_gap/input",
            "--out",
            PILOT_OUT,
            "--timeout-ms",
            "30000",
        ],
    ) {
        Ok(result) if result.success => {
            let artifacts = [
                format!("{PILOT_OUT}/repo-exposure.json"),
                format!("{PILOT_OUT}/repo-exposure.md"),
                format!("{PILOT_OUT}/agent-seam-packets.json"),
                format!("{PILOT_OUT}/pilot-summary.json"),
                format!("{PILOT_OUT}/pilot-summary.md"),
            ];
            let missing = artifacts
                .iter()
                .filter(|path| !Path::new(path.as_str()).exists())
                .cloned()
                .collect::<Vec<_>>();
            if missing.is_empty() {
                readiness_check(
                    "pilot-boundary-fixture",
                    "pass",
                    true,
                    &command,
                    "ripr pilot completed on the boundary-gap fixture",
                    artifacts.to_vec(),
                    Vec::new(),
                )
            } else {
                readiness_check(
                    "pilot-boundary-fixture",
                    "fail",
                    true,
                    &command,
                    "ripr pilot completed but expected artifacts are missing",
                    artifacts.to_vec(),
                    vec![format!("missing: {}", missing.join(", "))],
                )
            }
        }
        Ok(result) => readiness_check(
            "pilot-boundary-fixture",
            "fail",
            true,
            &command,
            "ripr pilot failed on the boundary-gap fixture",
            Vec::new(),
            command_details(&result),
        ),
        Err(err) => readiness_check(
            "pilot-boundary-fixture",
            "fail",
            true,
            &command,
            "ripr pilot could not run",
            Vec::new(),
            vec![err],
        ),
    }
}

fn outcome_fixture_check(binary: &Path) -> ReleaseReadinessCheck {
    let command = format!(
        "{} outcome --before {BEFORE_EXPOSURE} --after {AFTER_EXPOSURE} --format json --out {OUTCOME_OUT}",
        crate::normalize_path(binary)
    );
    if !binary.exists() {
        return readiness_check(
            "outcome-boundary-fixture",
            "fail",
            true,
            &command,
            "installed binary is missing",
            Vec::new(),
            Vec::new(),
        );
    }
    let _ = fs::remove_file(OUTCOME_OUT);
    match run_command_path(
        binary,
        &[
            "outcome",
            "--before",
            BEFORE_EXPOSURE,
            "--after",
            AFTER_EXPOSURE,
            "--format",
            "json",
            "--out",
            OUTCOME_OUT,
        ],
    ) {
        Ok(result) if result.success && Path::new(OUTCOME_OUT).exists() => readiness_check(
            "outcome-boundary-fixture",
            "pass",
            true,
            &command,
            "ripr outcome compared checked before/after snapshots",
            vec![OUTCOME_OUT.to_string()],
            Vec::new(),
        ),
        Ok(result) => readiness_check(
            "outcome-boundary-fixture",
            "fail",
            true,
            &command,
            "ripr outcome failed or did not write its artifact",
            vec![OUTCOME_OUT.to_string()],
            command_details(&result),
        ),
        Err(err) => readiness_check(
            "outcome-boundary-fixture",
            "fail",
            true,
            &command,
            "ripr outcome could not run",
            vec![OUTCOME_OUT.to_string()],
            vec![err],
        ),
    }
}

fn agent_verify_fixture_check(binary: &Path) -> ReleaseReadinessCheck {
    let command = format!(
        "{} agent verify --root . --before {BEFORE_EXPOSURE} --after {AFTER_EXPOSURE} --json > {AGENT_VERIFY_OUT}",
        crate::normalize_path(binary)
    );
    if !binary.exists() {
        return readiness_check(
            "agent-verify-boundary-fixture",
            "fail",
            true,
            &command,
            "installed binary is missing",
            Vec::new(),
            Vec::new(),
        );
    }
    let _ = fs::remove_file(AGENT_VERIFY_OUT);
    match run_command_path(
        binary,
        &[
            "agent",
            "verify",
            "--root",
            ".",
            "--before",
            BEFORE_EXPOSURE,
            "--after",
            AFTER_EXPOSURE,
            "--json",
        ],
    ) {
        Ok(result) if result.success => match fs::write(AGENT_VERIFY_OUT, &result.stdout) {
            Ok(()) => readiness_check(
                "agent-verify-boundary-fixture",
                "pass",
                true,
                &command,
                "ripr agent verify compared checked before/after snapshots",
                vec![AGENT_VERIFY_OUT.to_string()],
                Vec::new(),
            ),
            Err(err) => readiness_check(
                "agent-verify-boundary-fixture",
                "fail",
                true,
                &command,
                "ripr agent verify passed but artifact write failed",
                vec![AGENT_VERIFY_OUT.to_string()],
                vec![format!("failed to write {AGENT_VERIFY_OUT}: {err}")],
            ),
        },
        Ok(result) => readiness_check(
            "agent-verify-boundary-fixture",
            "fail",
            true,
            &command,
            "ripr agent verify failed on checked snapshots",
            vec![AGENT_VERIFY_OUT.to_string()],
            command_details(&result),
        ),
        Err(err) => readiness_check(
            "agent-verify-boundary-fixture",
            "fail",
            true,
            &command,
            "ripr agent verify could not run",
            vec![AGENT_VERIFY_OUT.to_string()],
            vec![err],
        ),
    }
}

fn repo_exposure_latency_check() -> ReleaseReadinessCheck {
    let command = "cargo xtask repo-exposure-latency-report";
    let artifact = "target/ripr/reports/repo-exposure-latency.json";
    match crate::repo_exposure_latency_report_impl() {
        Ok(()) => match read_json_status(Path::new(artifact)) {
            Ok(status) if status == "pass" => readiness_check(
                "repo-exposure-latency",
                "pass",
                true,
                command,
                "repo-exposure latency report exists and passes",
                vec![
                    artifact.to_string(),
                    "target/ripr/reports/repo-exposure-latency.md".to_string(),
                ],
                Vec::new(),
            ),
            Ok(status) => readiness_check(
                "repo-exposure-latency",
                "warn",
                false,
                command,
                "repo-exposure latency report exists but is not passing",
                vec![
                    artifact.to_string(),
                    "target/ripr/reports/repo-exposure-latency.md".to_string(),
                ],
                vec![format!("report status: {status}")],
            ),
            Err(err) => readiness_check(
                "repo-exposure-latency",
                "fail",
                true,
                command,
                "repo-exposure latency report could not be read",
                vec![artifact.to_string()],
                vec![err],
            ),
        },
        Err(err) => readiness_check(
            "repo-exposure-latency",
            "fail",
            true,
            command,
            "repo-exposure latency report command failed",
            vec![artifact.to_string()],
            vec![err],
        ),
    }
}

fn lsp_cockpit_check() -> ReleaseReadinessCheck {
    let command = "cargo xtask lsp-cockpit-report";
    let artifact = "target/ripr/reports/lsp-cockpit.json";
    match crate::lsp_cockpit_report_impl() {
        Ok(()) => match read_json_status(Path::new(artifact)) {
            Ok(status) if status == "pass" => readiness_check(
                "lsp-cockpit",
                "pass",
                true,
                command,
                "LSP cockpit report passes",
                vec![
                    artifact.to_string(),
                    "target/ripr/reports/lsp-cockpit.md".to_string(),
                ],
                Vec::new(),
            ),
            Ok(status) => readiness_check(
                "lsp-cockpit",
                "fail",
                true,
                command,
                "LSP cockpit report is not passing",
                vec![artifact.to_string()],
                vec![format!("report status: {status}")],
            ),
            Err(err) => readiness_check(
                "lsp-cockpit",
                "fail",
                true,
                command,
                "LSP cockpit report could not be read",
                vec![artifact.to_string()],
                vec![err],
            ),
        },
        Err(err) => readiness_check(
            "lsp-cockpit",
            "fail",
            true,
            command,
            "LSP cockpit report command failed",
            vec![artifact.to_string()],
            vec![err],
        ),
    }
}

fn github_workflow_check(binary: &Path) -> ReleaseReadinessCheck {
    let command = format!(
        "{} init --ci github --dry-run",
        crate::normalize_path(binary)
    );
    if !binary.exists() {
        return readiness_check(
            "github-workflow-defaults",
            "fail",
            true,
            &command,
            "installed binary is missing",
            Vec::new(),
            Vec::new(),
        );
    }
    match run_command_path(binary, &["init", "--ci", "github", "--dry-run"]) {
        Ok(result) if result.success => {
            let required = [
                "continue-on-error: true",
                "ripr pilot",
                "target/ripr/pilot",
                "target/ripr/reports",
                "RIPR_UPLOAD_SARIF",
                "actions/upload-artifact",
            ];
            let missing = required
                .iter()
                .filter(|needle| !result.stdout.contains(**needle))
                .map(|needle| (*needle).to_string())
                .collect::<Vec<_>>();
            if missing.is_empty() {
                readiness_check(
                    "github-workflow-defaults",
                    "pass",
                    true,
                    &command,
                    "generated GitHub workflow is advisory and includes pilot/report artifacts",
                    vec![".github/workflows/ripr.yml (dry-run)".to_string()],
                    Vec::new(),
                )
            } else {
                readiness_check(
                    "github-workflow-defaults",
                    "fail",
                    true,
                    &command,
                    "generated GitHub workflow is missing expected advisory artifacts",
                    vec![".github/workflows/ripr.yml (dry-run)".to_string()],
                    vec![format!("missing: {}", missing.join(", "))],
                )
            }
        }
        Ok(result) => readiness_check(
            "github-workflow-defaults",
            "fail",
            true,
            &command,
            "generated GitHub workflow dry-run failed",
            Vec::new(),
            command_details(&result),
        ),
        Err(err) => readiness_check(
            "github-workflow-defaults",
            "fail",
            true,
            &command,
            "generated GitHub workflow dry-run could not run",
            Vec::new(),
            vec![err],
        ),
    }
}

fn vsix_packaging_check() -> ReleaseReadinessCheck {
    let package_json = Path::new("editors/vscode/package.json");
    let release_doc = Path::new("docs/RELEASE_MARKETPLACE.md");
    let icon = Path::new("editors/vscode/icon.png");
    let command = "npm --prefix editors/vscode run package";
    let mut missing = Vec::new();
    for path in [package_json, release_doc, icon] {
        if !path.exists() {
            missing.push(crate::normalize_path(path));
        }
    }
    let script_present = read_json_value(package_json)
        .ok()
        .and_then(|value| {
            value
                .get("scripts")
                .and_then(|scripts| scripts.get("package"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .is_some();
    if !script_present {
        missing.push("editors/vscode/package.json scripts.package".to_string());
    }
    let doc_mentions_package = crate::read_text_lossy(release_doc)
        .map(|text| text.contains("npm run package") || text.contains("vsce"))
        .unwrap_or(false);
    if !doc_mentions_package {
        missing.push("docs/RELEASE_MARKETPLACE.md package instructions".to_string());
    }
    if missing.is_empty() {
        readiness_check(
            "vsix-packaging-path",
            "pass",
            true,
            command,
            "VSIX package path exists and is documented",
            vec![
                "editors/vscode/package.json".to_string(),
                "editors/vscode/package-lock.json".to_string(),
                "docs/RELEASE_MARKETPLACE.md".to_string(),
            ],
            Vec::new(),
        )
    } else {
        readiness_check(
            "vsix-packaging-path",
            "fail",
            true,
            command,
            "VSIX package path is incomplete",
            Vec::new(),
            vec![format!("missing: {}", missing.join(", "))],
        )
    }
}

fn known_limits_docs_check() -> ReleaseReadinessCheck {
    let command = "cargo xtask markdown-links";
    let docs = [
        "docs/INSTALLATION_VERIFICATION.md",
        "docs/QUICKSTART.md",
        "docs/EDITOR_EXTENSION.md",
    ];
    let mut missing = docs
        .iter()
        .filter(|path| !Path::new(path).exists())
        .map(|path| (*path).to_string())
        .collect::<Vec<_>>();
    let all_text = docs
        .iter()
        .filter_map(|path| crate::read_text_lossy(Path::new(path)).ok())
        .collect::<Vec<_>>()
        .join("\n");
    for needle in ["runtime mutation", "CI blocking", "unsaved-buffer"] {
        if !all_text.contains(needle) {
            missing.push(format!("known-limit text: {needle}"));
        }
    }
    if missing.is_empty() {
        readiness_check(
            "known-limits-docs",
            "pass",
            true,
            command,
            "known limits are documented for install, editor, and quickstart paths",
            docs.iter().map(|path| (*path).to_string()).collect(),
            Vec::new(),
        )
    } else {
        readiness_check(
            "known-limits-docs",
            "fail",
            true,
            command,
            "known limits docs are incomplete",
            docs.iter().map(|path| (*path).to_string()).collect(),
            vec![format!("missing: {}", missing.join(", "))],
        )
    }
}

fn readiness_check(
    id: &'static str,
    status: &str,
    required: bool,
    command: &str,
    summary: &str,
    artifacts: Vec<String>,
    details: Vec<String>,
) -> ReleaseReadinessCheck {
    ReleaseReadinessCheck {
        id,
        status: status.to_string(),
        required,
        command: command.to_string(),
        summary: summary.to_string(),
        artifacts,
        details,
    }
}

fn release_readiness_status(checks: &[ReleaseReadinessCheck]) -> &'static str {
    if checks
        .iter()
        .any(|check| check.required && check.status == "fail")
    {
        return "fail";
    }
    if checks
        .iter()
        .any(|check| check.status == "warn" || check.status == "not_run")
    {
        return "warn";
    }
    "pass"
}

fn release_readiness_next_commands(checks: &[ReleaseReadinessCheck], version: &str) -> Vec<String> {
    let mut out = checks
        .iter()
        .filter(|check| check.status != "pass")
        .map(|check| check.command.clone())
        .collect::<Vec<_>>();
    if out.is_empty() {
        out.push(format!("cargo xtask release-readiness --version {version}"));
    }
    out
}

fn release_readiness_json(report: &ReleaseReadinessReport) -> Result<String, String> {
    let value = json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "report": "release-readiness",
        "version": report.version,
        "status": report.status,
        "checks": report.checks.iter().map(release_readiness_check_json).collect::<Vec<_>>(),
        "next_commands": report.next_commands,
    });
    serde_json::to_string_pretty(&value)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|err| format!("failed to render release-readiness JSON: {err}"))
}

fn release_readiness_check_json(check: &ReleaseReadinessCheck) -> Value {
    json!({
        "id": check.id,
        "status": check.status,
        "required": check.required,
        "command": check.command,
        "summary": check.summary,
        "artifacts": check.artifacts,
        "details": check.details,
    })
}

fn release_readiness_markdown(report: &ReleaseReadinessReport) -> String {
    let mut out = String::new();
    out.push_str("# ripr release readiness\n\n");
    out.push_str(&format!("- version: `{}`\n", report.version));
    out.push_str(&format!("- status: `{}`\n\n", report.status));
    out.push_str("| Check | Status | Required | Summary |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for check in &report.checks {
        out.push_str(&format!(
            "| `{}` | `{}` | {} | {} |\n",
            check.id,
            check.status,
            if check.required { "yes" } else { "no" },
            md_escape_inline(&check.summary)
        ));
    }
    out.push_str("\n## Details\n\n");
    for check in &report.checks {
        out.push_str(&format!("### `{}`\n\n", check.id));
        out.push_str(&format!("- status: `{}`\n", check.status));
        out.push_str(&format!(
            "- command: `{}`\n",
            md_escape_inline(&check.command)
        ));
        if !check.artifacts.is_empty() {
            out.push_str("- artifacts:\n");
            for artifact in &check.artifacts {
                out.push_str(&format!("  - `{}`\n", md_escape_inline(artifact)));
            }
        }
        if !check.details.is_empty() {
            out.push_str("- details:\n");
            for detail in &check.details {
                out.push_str(&format!("  - {}\n", md_escape_inline(detail)));
            }
        }
        out.push('\n');
    }
    out.push_str("## Next Commands\n\n");
    for command in &report.next_commands {
        out.push_str(&format!("- `{}`\n", md_escape_inline(command)));
    }
    out.push_str("\nThis report records the 0.4 release surface from repo artifacts. It does not run mutation testing, enable CI blocking, change analyzer classifications, or expand LSP behavior.\n");
    out
}

fn md_escape_inline(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn command_details(result: &CommandResult) -> Vec<String> {
    let mut details = Vec::new();
    details.push(match result.status {
        Some(code) => format!("exit code: {code}"),
        None => "exit code: unavailable".to_string(),
    });
    push_trimmed_detail(&mut details, "stdout", &result.stdout);
    push_trimmed_detail(&mut details, "stderr", &result.stderr);
    details
}

fn push_trimmed_detail(details: &mut Vec<String>, label: &str, text: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }
    let first_line = if let Some(line) = trimmed.lines().next() {
        line
    } else {
        trimmed
    };
    details.push(format!("{label}: {first_line}"));
}

fn read_crate_version(path: &Path) -> Option<String> {
    let text = crate::read_text_lossy(path).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("version") else {
            continue;
        };
        let Some((_, value)) = rest.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn git_worktree_is_clean() -> Result<bool, String> {
    let result = run_command("git", &["status", "--porcelain"])?;
    if !result.success {
        return Err(command_details(&result).join("; "));
    }
    Ok(result.stdout.trim().is_empty())
}

fn installed_ripr_binary() -> PathBuf {
    Path::new(INSTALL_ROOT)
        .join("bin")
        .join(format!("ripr{}", std::env::consts::EXE_SUFFIX))
}

fn read_json_status(path: &Path) -> Result<String, String> {
    let value = read_json_value(path)?;
    value
        .get("status")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("{} is missing status", crate::normalize_path(path)))
}

fn read_json_value(path: &Path) -> Result<Value, String> {
    let text = crate::read_text_lossy(path)?;
    serde_json::from_str(&text).map_err(|err| {
        format!(
            "failed to parse {} as JSON: {err}",
            crate::normalize_path(path)
        )
    })
}

fn run_command(program: &str, args: &[&str]) -> Result<CommandResult, String> {
    let output =
        crate::run::capture_output(program, args, &format!("{program} {}", args.join(" ")))?;
    Ok(CommandResult {
        status: output.status.code(),
        success: output.status.success(),
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn run_command_path(program: &Path, args: &[&str]) -> Result<CommandResult, String> {
    let program_text = program.to_string_lossy().into_owned();
    run_command(&program_text, args)
}

#[cfg(test)]
mod tests {
    use super::{
        ReleaseReadinessCheck, ReleaseReadinessReport, parse_release_readiness_args,
        readiness_check, release_readiness_json, release_readiness_markdown,
        release_readiness_status,
    };
    use serde_json::Value;

    #[test]
    fn release_readiness_args_parse_version() -> Result<(), String> {
        let args = vec!["--version".to_string(), "0.4.0".to_string()];
        let parsed = parse_release_readiness_args(&args)?;
        if parsed.version != "0.4.0" {
            return Err(format!("unexpected version {}", parsed.version));
        }
        Ok(())
    }

    #[test]
    fn release_readiness_args_require_version() -> Result<(), String> {
        let parsed = parse_release_readiness_args(&[]);
        match parsed {
            Err(message) if message.contains("--version") => Ok(()),
            Err(message) => Err(format!("unexpected error: {message}")),
            Ok(_) => Err("expected missing version error".to_string()),
        }
    }

    #[test]
    fn release_readiness_status_warns_for_not_run_but_fails_required_failures() -> Result<(), String>
    {
        let pass = readiness_check("pass", "pass", true, "cmd", "ok", Vec::new(), Vec::new());
        let not_run = readiness_check(
            "package",
            "not_run",
            false,
            "cargo package",
            "dirty tree",
            Vec::new(),
            Vec::new(),
        );
        let warn_status = release_readiness_status(&[pass.clone(), not_run.clone()]);
        if warn_status != "warn" {
            return Err(format!("expected warn status, got {warn_status}"));
        }
        let failure = readiness_check("fail", "fail", true, "cmd", "bad", Vec::new(), Vec::new());
        let fail_status = release_readiness_status(&[pass, not_run, failure]);
        if fail_status != "fail" {
            return Err(format!("expected fail status, got {fail_status}"));
        }
        Ok(())
    }

    #[test]
    fn release_readiness_json_and_markdown_are_structured() -> Result<(), String> {
        let checks: Vec<ReleaseReadinessCheck> = vec![readiness_check(
            "installed-command-surface",
            "pass",
            true,
            "target/ripr/release-readiness/install/bin/ripr --help",
            "installed binary exposes commands",
            vec!["target/ripr/release-readiness/install/bin/ripr".to_string()],
            Vec::new(),
        )];
        let report = ReleaseReadinessReport {
            version: "0.4.0".to_string(),
            status: "pass".to_string(),
            checks,
            next_commands: vec!["cargo xtask release-readiness --version 0.4.0".to_string()],
        };
        let json_text = release_readiness_json(&report)?;
        let value: Value = serde_json::from_str(&json_text)
            .map_err(|err| format!("release readiness JSON parse failed: {err}"))?;
        if value["report"] != "release-readiness" {
            return Err("expected release-readiness report id".to_string());
        }
        let markdown = release_readiness_markdown(&report);
        if !markdown.contains("# ripr release readiness") {
            return Err("expected release readiness markdown heading".to_string());
        }
        if !markdown.contains("installed-command-surface") {
            return Err("expected check id in markdown".to_string());
        }
        Ok(())
    }
}
