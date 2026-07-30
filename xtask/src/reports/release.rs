use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

const REPORT_WORK_DIR: &str = "target/ripr/release-readiness";
const INSTALL_ROOT: &str = "target/ripr/release-readiness/install";
const PILOT_OUT: &str = "target/ripr/release-readiness/pilot";
const OUTCOME_OUT: &str = "target/ripr/release-readiness/targeted-test-outcome.json";
const AGENT_VERIFY_OUT: &str = "target/ripr/release-readiness/agent-verify.json";
const AGENT_RECEIPT_OUT: &str = "target/ripr/release-readiness/agent-receipt.json";
const BOUNDARY_GAP_SEAM_ID: &str = "67fc764ba37d77bd";
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
    let crate_version =
        read_crate_version(Path::new("crates/ripr/Cargo.toml"), Path::new("Cargo.toml"));
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
        agent_receipt_fixture_check(&installed_binary),
        repo_exposure_latency_check(),
        lsp_cockpit_check(),
        github_workflow_check(&installed_binary),
        vsix_packaging_check(),
        extension_version_match_check(crate_version.as_deref()),
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
    let binary_path = crate::normalize_path(binary);
    let command =
        format!("{binary_path} --version && {binary_path} --help && {binary_path} first-pr --help");
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
    let version = match run_command_path(binary, &["--version"]) {
        Ok(result) if result.success => result,
        Ok(result) => {
            return readiness_check(
                "installed-command-surface",
                "fail",
                true,
                &command,
                "installed ripr --version failed",
                vec![crate::normalize_path(binary)],
                command_details(&result),
            );
        }
        Err(err) => {
            return readiness_check(
                "installed-command-surface",
                "fail",
                true,
                &command,
                "installed ripr --version could not run",
                vec![crate::normalize_path(binary)],
                vec![err],
            );
        }
    };
    let help = match run_command_path(binary, &["--help"]) {
        Ok(result) if result.success => result,
        Ok(result) => {
            return readiness_check(
                "installed-command-surface",
                "fail",
                true,
                &command,
                "installed binary help failed",
                vec![crate::normalize_path(binary)],
                command_details(&result),
            );
        }
        Err(err) => {
            return readiness_check(
                "installed-command-surface",
                "fail",
                true,
                &command,
                "installed binary help could not run",
                vec![crate::normalize_path(binary)],
                vec![err],
            );
        }
    };
    let first_pr_help = match run_command_path(binary, &["first-pr", "--help"]) {
        Ok(result) if result.success => result,
        Ok(result) => {
            return readiness_check(
                "installed-command-surface",
                "fail",
                true,
                &command,
                "installed ripr first-pr --help failed",
                vec![crate::normalize_path(binary)],
                command_details(&result),
            );
        }
        Err(err) => {
            return readiness_check(
                "installed-command-surface",
                "fail",
                true,
                &command,
                "installed ripr first-pr --help could not run",
                vec![crate::normalize_path(binary)],
                vec![err],
            );
        }
    };
    let mut missing = missing_required_needles(
        &help.stdout,
        &[
            "ripr pilot",
            "ripr outcome",
            "ripr first-pr",
            "ripr calibrate cargo-mutants",
            "ripr agent verify",
            "ripr agent receipt",
        ],
    );
    missing.extend(missing_required_needles(
        &first_pr_help.stdout,
        &[
            "Create the start-here packet",
            "usage: ripr first-pr",
            "--gap-ledger",
            "--receipts-dir",
            "--out-dir",
        ],
    ));
    if missing.is_empty() {
        readiness_check(
            "installed-command-surface",
            "pass",
            true,
            &command,
            "installed binary exposes public release-loop and first-run commands",
            vec![crate::normalize_path(binary)],
            command_details(&version),
        )
    } else {
        readiness_check(
            "installed-command-surface",
            "fail",
            true,
            &command,
            "installed binary is missing expected public loop or first-run commands",
            vec![crate::normalize_path(binary)],
            vec![format!("missing: {}", missing.join(", "))],
        )
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

fn agent_receipt_fixture_check(binary: &Path) -> ReleaseReadinessCheck {
    let command = format!(
        "{} agent receipt --root . --verify-json {AGENT_VERIFY_OUT} --seam-id {BOUNDARY_GAP_SEAM_ID} --json --out {AGENT_RECEIPT_OUT}",
        crate::normalize_path(binary)
    );
    if !binary.exists() {
        return readiness_check(
            "agent-receipt-boundary-fixture",
            "fail",
            true,
            &command,
            "installed binary is missing",
            Vec::new(),
            Vec::new(),
        );
    }
    if !Path::new(AGENT_VERIFY_OUT).exists() {
        return readiness_check(
            "agent-receipt-boundary-fixture",
            "fail",
            true,
            &command,
            "agent verify artifact is missing",
            vec![AGENT_VERIFY_OUT.to_string(), AGENT_RECEIPT_OUT.to_string()],
            vec![format!("missing prerequisite: {AGENT_VERIFY_OUT}")],
        );
    }
    let _ = fs::remove_file(AGENT_RECEIPT_OUT);
    match run_command_path(
        binary,
        &[
            "agent",
            "receipt",
            "--root",
            ".",
            "--verify-json",
            AGENT_VERIFY_OUT,
            "--seam-id",
            BOUNDARY_GAP_SEAM_ID,
            "--json",
            "--out",
            AGENT_RECEIPT_OUT,
        ],
    ) {
        Ok(result) if result.success && Path::new(AGENT_RECEIPT_OUT).exists() => readiness_check(
            "agent-receipt-boundary-fixture",
            "pass",
            true,
            &command,
            "ripr agent receipt wrote a focused boundary-gap receipt",
            vec![AGENT_VERIFY_OUT.to_string(), AGENT_RECEIPT_OUT.to_string()],
            Vec::new(),
        ),
        Ok(result) => readiness_check(
            "agent-receipt-boundary-fixture",
            "fail",
            true,
            &command,
            "ripr agent receipt failed or did not write its artifact",
            vec![AGENT_VERIFY_OUT.to_string(), AGENT_RECEIPT_OUT.to_string()],
            command_details(&result),
        ),
        Err(err) => readiness_check(
            "agent-receipt-boundary-fixture",
            "fail",
            true,
            &command,
            "ripr agent receipt could not run",
            vec![AGENT_VERIFY_OUT.to_string(), AGENT_RECEIPT_OUT.to_string()],
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
                "ripr agent start",
                "ripr agent status",
                "ripr agent review-summary",
                "ripr reports gap-ledger",
                "ripr first-pr",
                "#### First-run status",
                "missing_start_here",
                "cat target/ripr/reports/start-here.md",
                "target/ripr/reports/gap-decision-ledger.json",
                "target/ripr/reports/start-here.md",
                "target/ripr/pilot",
                "target/ripr/workflow",
                "target/ripr/reports",
                "target/ripr/workflow/agent-status.md",
                "target/ripr/workflow/agent-review-summary.md",
                "target/ripr/reports/agent-receipt.json",
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
                    "generated GitHub workflow is advisory and starts with first-run repair guidance",
                    vec![".github/workflows/ripr.yml (dry-run)".to_string()],
                    Vec::new(),
                )
            } else {
                readiness_check(
                    "github-workflow-defaults",
                    "fail",
                    true,
                    &command,
                    "generated GitHub workflow is missing expected advisory first-run artifacts",
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
    if !vsix_start_current_repair_command_present(package_json) {
        missing.push(
            "editors/vscode/package.json contributes.commands ripr.startCurrentRepair".to_string(),
        );
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

/// Fail-closed guard against the VS Code extension version silently drifting
/// from the crate version. When `editors/vscode/package.json` lags the crate,
/// `vsce` embeds the stale version into the VSIX and the marketplace publish
/// fails with "vX already exists" — the failure mode that left the extension two
/// releases behind through 0.8.0/0.9.0 (#1283). Any read failure or mismatch is
/// a fail; only an exact match passes.
fn extension_version_match_check(crate_version: Option<&str>) -> ReleaseReadinessCheck {
    let package_json = Path::new("editors/vscode/package.json");
    let ext_version = read_json_value(package_json).ok().and_then(|value| {
        value
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    extension_version_check_from(ext_version.as_deref(), crate_version)
}

/// Pure comparison split out from the file read so it is testable without a
/// working-directory dependency.
fn extension_version_check_from(
    ext_version: Option<&str>,
    crate_version: Option<&str>,
) -> ReleaseReadinessCheck {
    let command =
        "compare editors/vscode/package.json version to the workspace package version (Cargo.toml)";
    match (ext_version, crate_version) {
        (Some(ext), Some(krate)) if ext == krate => readiness_check(
            "extension-version-match",
            "pass",
            true,
            command,
            "VS Code extension version matches the crate version",
            vec!["editors/vscode/package.json".to_string()],
            Vec::new(),
        ),
        (Some(ext), Some(krate)) => readiness_check(
            "extension-version-match",
            "fail",
            true,
            command,
            "VS Code extension version does not match the crate version; the marketplace publish would fail or republish a stale version",
            Vec::new(),
            vec![format!(
                "editors/vscode/package.json version {ext} != crate version {krate} (bump editors/vscode/package.json + package-lock.json)"
            )],
        ),
        (None, _) => readiness_check(
            "extension-version-match",
            "fail",
            true,
            command,
            "could not read the VS Code extension version",
            Vec::new(),
            vec!["editors/vscode/package.json version field unreadable".to_string()],
        ),
        (_, None) => readiness_check(
            "extension-version-match",
            "fail",
            true,
            command,
            "could not read the crate version",
            Vec::new(),
            vec![
                "workspace package version unreadable via crates/ripr/Cargo.toml -> Cargo.toml [workspace.package]"
                    .to_string(),
            ],
        ),
    }
}

fn missing_required_needles(text: &str, required: &[&str]) -> Vec<String> {
    required
        .iter()
        .filter(|needle| !text.contains(**needle))
        .map(|needle| (*needle).to_string())
        .collect()
}

fn vsix_start_current_repair_command_present(package_json: &Path) -> bool {
    read_json_value(package_json)
        .ok()
        .and_then(|value| {
            value
                .get("contributes")
                .and_then(|contributes| contributes.get("commands"))
                .and_then(Value::as_array)
                .map(|commands| {
                    commands.iter().any(|command| {
                        command.get("command").and_then(Value::as_str)
                            == Some("ripr.startCurrentRepair")
                            && command.get("title").and_then(Value::as_str)
                                == Some("ripr: Start Current Repair")
                    })
                })
        })
        .unwrap_or(false)
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
    out.push_str("\nThis report records the release surface from repo artifacts. It does not run mutation testing, enable CI blocking, change analyzer classifications, or expand LSP behavior.\n");
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

/// How a manifest's `[package]` declares its version.
#[derive(Debug, PartialEq, Eq)]
enum PackageVersion {
    /// `version = "0.10.0"` — the version is written in this manifest.
    Literal(String),
    /// `version.workspace = true` — the version comes from the workspace root.
    Inherited,
}

/// Read the effective package version for `manifest`, resolving Cargo's
/// `version.workspace = true` inheritance against `workspace_manifest`.
///
/// Inheritance has to be resolved rather than scanned past, because the two
/// obvious shortcuts both produce a *wrong version* instead of an error, and
/// every caller here reports a verdict on whatever it is handed:
///
/// - a bare `strip_prefix("version")` scan reads `version.workspace = true` as
///   the literal version `true`;
/// - skipping the inherited line and continuing reads the first `version = "…"`
///   of the next `[dependencies]` entry as the crate's own version.
///
/// `extension-version-match` claims it compared the extension to the crate
/// version, so this must return the real version or `None` — never a value it
/// merely found nearby.
fn read_crate_version(manifest: &Path, workspace_manifest: &Path) -> Option<String> {
    let text = crate::read_text_lossy(manifest).ok()?;
    match package_version(&text, "package")? {
        PackageVersion::Literal(version) => Some(version),
        PackageVersion::Inherited => {
            let workspace_text = crate::read_text_lossy(workspace_manifest).ok()?;
            match package_version(&workspace_text, "workspace.package")? {
                PackageVersion::Literal(version) => Some(version),
                // A workspace root that itself inherits has nowhere left to
                // look; report unreadable rather than guess.
                PackageVersion::Inherited => None,
            }
        }
    }
}

/// Extract the `version` key from one top-level table of a manifest.
///
/// Scoped to `[{section}]` so a `version = "…"` belonging to a dependency table
/// can never be mistaken for the package's own version.
///
/// This reads the `version` key of a Cargo manifest table, not arbitrary TOML:
/// it handles the spellings Cargo actually accepts for that key — either quote
/// style, both inheritance forms, and trailing comments on values and section
/// headers — and returns `None` for anything else. That boundary matters
/// because the guarantee callers rely on is "a real version or `None`", so an
/// unrecognized spelling has to read as unreadable rather than as whatever text
/// happened to follow the `=`.
fn package_version(text: &str, section: &str) -> Option<PackageVersion> {
    let header = format!("[{section}]");
    let mut in_section = false;
    for line in text.lines() {
        let trimmed = strip_toml_comment(line).trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('[') {
            in_section = trimmed == header;
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            // `version = { workspace = true }` and `version.workspace = true`
            // are the same declaration in two spellings; Cargo accepts both.
            "version" if inline_table_inherits_workspace(value) => {
                return Some(PackageVersion::Inherited);
            }
            "version" => {
                let version = unquote_toml_string(value)?;
                if !version.is_empty() {
                    return Some(PackageVersion::Literal(version.to_string()));
                }
            }
            "version.workspace" if value == "true" => return Some(PackageVersion::Inherited),
            _ => {}
        }
    }
    None
}

/// Drop a trailing `#` comment that begins outside a quoted string.
///
/// Without this, `version = "0.10.0" # bump` reads as the version
/// `0.10.0" # bump`, and a `[package] # note` header stops matching its table.
fn strip_toml_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (index, ch) in line.char_indices() {
        match (quote, ch) {
            (None, '"' | '\'') => quote = Some(ch),
            (Some(open), _) if ch == open => quote = None,
            (None, '#') => return &line[..index],
            _ => {}
        }
    }
    line
}

/// Return the contents of a quoted TOML string, or `None` when `value` is not
/// one. An unquoted or malformed value is unreadable, not a bare version.
fn unquote_toml_string(value: &str) -> Option<&str> {
    for quote in ['"', '\''] {
        if let Some(rest) = value.strip_prefix(quote) {
            return rest.strip_suffix(quote);
        }
    }
    None
}

/// Whether an inline table declares workspace inheritance
/// (`{ workspace = true }`).
fn inline_table_inherits_workspace(value: &str) -> bool {
    let Some(inner) = value.strip_prefix('{').and_then(|v| v.strip_suffix('}')) else {
        return false;
    };
    inner.split(',').any(|entry| {
        let Some((key, entry_value)) = entry.split_once('=') else {
            return false;
        };
        key.trim() == "workspace" && entry_value.trim() == "true"
    })
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
        PackageVersion, ReleaseReadinessCheck, ReleaseReadinessReport,
        extension_version_check_from, missing_required_needles, package_version,
        parse_release_readiness_args, read_crate_version, readiness_check, release_readiness_json,
        release_readiness_markdown, release_readiness_status,
        vsix_start_current_repair_command_present,
    };
    use serde_json::Value;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn extension_version_check_fails_closed_on_drift() -> Result<(), String> {
        let matched = extension_version_check_from(Some("0.10.0"), Some("0.10.0"));
        if matched.status != "pass" || !matched.required {
            return Err(format!(
                "expected pass+required on match, got {}/{}",
                matched.status, matched.required
            ));
        }
        for (ext, krate, label) in [
            (Some("0.8.0"), Some("0.10.0"), "stale extension"),
            (None, Some("0.10.0"), "unreadable extension"),
            (Some("0.10.0"), None, "unreadable crate"),
        ] {
            let check = extension_version_check_from(ext, krate);
            if check.status != "fail" || !check.required {
                return Err(format!(
                    "expected required fail for {label}, got {}/{}",
                    check.status, check.required
                ));
            }
        }
        Ok(())
    }

    /// `version.workspace = true` must not be read as the literal version
    /// `true`, and a dependency's `version` must not stand in for the package's
    /// own. Both shortcuts hand `extension-version-match` a wrong version that
    /// it would then report a verdict on (#2711).
    #[test]
    fn package_version_distinguishes_literal_inherited_and_dependency() -> Result<(), String> {
        let literal = "[package]\nname = \"ripr\"\nversion = \"0.10.0\"\n";
        if package_version(literal, "package")
            != Some(PackageVersion::Literal("0.10.0".to_string()))
        {
            return Err("expected a literal package version".to_string());
        }

        let inherited = "[package]\nname = \"ripr\"\nversion.workspace = true\n\n[dependencies]\nserde = { version = \"1.0.9\" }\nzip = \"2\"\n";
        if package_version(inherited, "package") != Some(PackageVersion::Inherited) {
            return Err("expected an inherited package version".to_string());
        }

        // The dependency table must be invisible to a `[package]` lookup, so a
        // manifest with no package version reads as unreadable, not as `1.0.9`.
        let no_package_version =
            "[package]\nname = \"ripr\"\n\n[dependencies]\nserde = \"1.0.9\"\n";
        if package_version(no_package_version, "package").is_some() {
            return Err("dependency version leaked into the package lookup".to_string());
        }
        Ok(())
    }

    /// The remaining ways a manifest can fail to declare a usable version. Each
    /// must read as unreadable, because every caller reports a verdict on
    /// whatever it receives — an empty or bogus version would be published as
    /// confidently as a real one.
    #[test]
    fn package_version_rejects_malformed_and_non_inherited_declarations() -> Result<(), String> {
        let cases: &[(&str, &str)] = &[
            (
                "[package]\nname = \"ripr\"\nversion = \"\"\n",
                "an empty version string",
            ),
            (
                "[package]\nname = \"ripr\"\nversion.workspace = false\n",
                "an explicitly non-inherited version",
            ),
            (
                "[package]\nname = \"ripr\"\nedition = \"2024\"\n",
                "a package table with no version key",
            ),
            (
                "[workspace.package]\nversion = \"0.10.0\"\n",
                "a version declared only in another table",
            ),
        ];
        for (manifest, label) in cases {
            if let Some(found) = package_version(manifest, "package") {
                return Err(format!("expected unreadable for {label}, got {found:?}"));
            }
        }

        // The workspace lookup is the same scanner against a different table,
        // so confirm it actually resolves the table it claims to.
        let workspace = "[workspace]\nmembers = [\"crates/ripr\"]\n\n[workspace.package]\nversion = \"0.11.0\"\n";
        if package_version(workspace, "workspace.package")
            != Some(PackageVersion::Literal("0.11.0".to_string()))
        {
            return Err("expected the workspace table version".to_string());
        }
        Ok(())
    }

    /// The manifest spellings Cargo accepts that a naive line scan gets wrong.
    /// Each of these is valid TOML, so reading them loosely would return a
    /// corrupted version — `0.10.0" # bump` — while still satisfying the
    /// "returns Some" happy path. Raised in review on #2711.
    #[test]
    fn package_version_handles_comments_and_quote_styles() -> Result<(), String> {
        let cases: &[(&str, PackageVersion, &str)] = &[
            (
                "[package]\nversion = \"0.10.0\" # bump me at release\n",
                PackageVersion::Literal("0.10.0".to_string()),
                "a trailing comment after a double-quoted version",
            ),
            (
                "[package]\nversion = '0.10.0'\n",
                PackageVersion::Literal("0.10.0".to_string()),
                "a single-quoted version",
            ),
            (
                "[package] # the package table\nversion = \"0.10.0\"\n",
                PackageVersion::Literal("0.10.0".to_string()),
                "a trailing comment on the section header",
            ),
            (
                "[package]\nversion = { workspace = true }\n",
                PackageVersion::Inherited,
                "the inline-table inheritance spelling",
            ),
            (
                "[package]\nversion.workspace = true # inherited\n",
                PackageVersion::Inherited,
                "a trailing comment after inherited",
            ),
        ];
        for (manifest, expected, label) in cases {
            let found = package_version(manifest, "package");
            if found.as_ref() != Some(expected) {
                return Err(format!("expected {expected:?} for {label}, got {found:?}"));
            }
        }

        // A `#` inside the quoted value is part of the version, not a comment.
        let hashed = "[package]\nversion = \"0.10.0+build#7\"\n";
        if package_version(hashed, "package")
            != Some(PackageVersion::Literal("0.10.0+build#7".to_string()))
        {
            return Err("a quoted '#' must not be treated as a comment".to_string());
        }

        // An unquoted value is not a version; it must read as unreadable.
        let unquoted = "[package]\nversion = 0.10.0\n";
        if let Some(found) = package_version(unquoted, "package") {
            return Err(format!(
                "expected unreadable for an unquoted version, got {found:?}"
            ));
        }

        // A commented-out version leaves the table with no version at all.
        let commented_out = "[package]\n# version = \"0.9.0\"\nedition = \"2024\"\n";
        if let Some(found) = package_version(commented_out, "package") {
            return Err(format!(
                "expected unreadable when the version is commented out, got {found:?}"
            ));
        }
        Ok(())
    }

    /// A workspace root that itself defers, and a member manifest that cannot be
    /// read at all, both have nowhere left to look. Neither may fall back to a
    /// guess: inheritance must terminate in a real version or in `None`.
    #[test]
    fn read_crate_version_terminates_instead_of_guessing() -> Result<(), String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("clock error: {err}"))?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-version-terminate-{stamp}"));
        fs::create_dir_all(&dir).map_err(|err| format!("failed to create dir: {err}"))?;

        let member = dir.join("member-Cargo.toml");
        let deferring_workspace = dir.join("deferring-Cargo.toml");
        let write = |path: &std::path::Path, text: &str| -> Result<(), String> {
            fs::write(path, text)
                .map_err(|err| format!("failed to write {}: {err}", path.display()))
        };
        write(
            &member,
            "[package]\nname = \"ripr\"\nversion.workspace = true\n",
        )?;
        // A workspace root that also says `version.workspace = true`.
        write(
            &deferring_workspace,
            "[workspace.package]\nversion.workspace = true\n",
        )?;

        let deferred_forever = read_crate_version(&member, &deferring_workspace);
        let absent_member =
            read_crate_version(&dir.join("absent-Cargo.toml"), &deferring_workspace);
        fs::remove_dir_all(&dir).map_err(|err| format!("failed to remove dir: {err}"))?;

        if deferred_forever.is_some() {
            return Err(format!(
                "expected unreadable when the workspace also defers, got {deferred_forever:?}"
            ));
        }
        if absent_member.is_some() {
            return Err(format!(
                "expected unreadable when the member manifest is absent, got {absent_member:?}"
            ));
        }
        Ok(())
    }

    /// The release gate's version claim has to survive the indirection it now
    /// depends on: reading `crates/ripr/Cargo.toml` must resolve through the
    /// workspace root, and must yield `None` rather than a guess when it cannot.
    #[test]
    fn read_crate_version_resolves_workspace_inheritance() -> Result<(), String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("clock error: {err}"))?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-version-inherit-{stamp}"));
        fs::create_dir_all(&dir).map_err(|err| format!("failed to create dir: {err}"))?;

        let member = dir.join("member-Cargo.toml");
        let workspace = dir.join("workspace-Cargo.toml");
        let workspace_without_version = dir.join("workspace-no-version-Cargo.toml");
        let write = |path: &std::path::Path, text: &str| -> Result<(), String> {
            fs::write(path, text)
                .map_err(|err| format!("failed to write {}: {err}", path.display()))
        };
        write(
            &member,
            "[package]\nname = \"ripr\"\nversion.workspace = true\n\n[dependencies]\nserde = \"1.0.9\"\n",
        )?;
        write(
            &workspace,
            "[workspace]\nmembers = [\"crates/ripr\"]\n\n[workspace.package]\nversion = \"0.10.0\"\nedition = \"2024\"\n",
        )?;
        write(
            &workspace_without_version,
            "[workspace]\nmembers = [\"crates/ripr\"]\n\n[workspace.package]\nedition = \"2024\"\n",
        )?;

        let resolved = read_crate_version(&member, &workspace);
        let unresolvable = read_crate_version(&member, &workspace_without_version);
        let missing_workspace = read_crate_version(&member, &dir.join("absent-Cargo.toml"));
        fs::remove_dir_all(&dir).map_err(|err| format!("failed to remove dir: {err}"))?;

        if resolved.as_deref() != Some("0.10.0") {
            return Err(format!(
                "expected 0.10.0 through inheritance, got {resolved:?}"
            ));
        }
        if unresolvable.is_some() {
            return Err(format!(
                "expected unreadable when the workspace declares no version, got {unresolvable:?}"
            ));
        }
        if missing_workspace.is_some() {
            return Err(format!(
                "expected unreadable when the workspace manifest is absent, got {missing_workspace:?}"
            ));
        }
        Ok(())
    }

    /// Binds the gate to this repository's real manifests: whatever the release
    /// version is, the checked-in extension manifest must already agree with it.
    /// This is the check that would have caught #1283 at PR time.
    #[test]
    fn repository_extension_version_matches_workspace_version() -> Result<(), String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| "xtask manifest has no parent directory".to_string())?
            .to_path_buf();
        let crate_version = read_crate_version(
            &root.join("crates/ripr/Cargo.toml"),
            &root.join("Cargo.toml"),
        )
        .ok_or_else(|| "workspace package version unreadable".to_string())?;
        let package_json = root.join("editors/vscode/package.json");
        let text = fs::read_to_string(&package_json)
            .map_err(|err| format!("failed to read {}: {err}", package_json.display()))?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|err| format!("failed to parse {}: {err}", package_json.display()))?;
        let ext_version = value
            .get("version")
            .and_then(Value::as_str)
            .ok_or_else(|| "extension package.json has no version".to_string())?;
        if ext_version != crate_version {
            return Err(format!(
                "editors/vscode/package.json version {ext_version} != workspace version {crate_version}"
            ));
        }
        Ok(())
    }

    #[test]
    fn release_readiness_args_parse_version() -> Result<(), String> {
        let args = vec!["--version".to_string(), "0.5.0".to_string()];
        let parsed = parse_release_readiness_args(&args)?;
        if parsed.version != "0.5.0" {
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
    fn release_readiness_command_surface_needles_include_first_run() -> Result<(), String> {
        let help = "ripr pilot\nripr outcome\nripr first-pr\nripr agent verify";
        let missing = missing_required_needles(
            help,
            &[
                "ripr pilot",
                "ripr outcome",
                "ripr first-pr",
                "ripr agent verify",
            ],
        );
        if !missing.is_empty() {
            return Err(format!(
                "expected all first-run needles present: {missing:?}"
            ));
        }
        let missing_first_pr = missing_required_needles(help, &["ripr first-pr", "--receipts-dir"]);
        if missing_first_pr != ["--receipts-dir".to_string()] {
            return Err(format!("unexpected missing needles: {missing_first_pr:?}"));
        }
        Ok(())
    }

    #[test]
    fn vsix_manifest_declares_start_current_repair_command() -> Result<(), String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("clock error: {err}"))?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ripr-vsix-command-{stamp}.json"));
        fs::write(
            &path,
            r#"{
              "contributes": {
                "commands": [
                  {
                    "command": "ripr.startCurrentRepair",
                    "title": "ripr: Start Current Repair"
                  }
                ]
              }
            }"#,
        )
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
        let present = vsix_start_current_repair_command_present(&path);
        fs::remove_file(&path)
            .map_err(|err| format!("failed to remove {}: {err}", path.display()))?;
        if !present {
            return Err("expected start current repair command to be detected".to_string());
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
            version: "0.8.0".to_string(),
            status: "pass".to_string(),
            checks,
            next_commands: vec!["cargo xtask release-readiness --version 0.8.0".to_string()],
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
