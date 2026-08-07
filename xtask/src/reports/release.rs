use flate2::read::GzDecoder;
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tar::Archive;

const REPORT_WORK_DIR: &str = "target/ripr/release-readiness";
const INSTALL_ROOT: &str = "target/ripr/release-readiness/install";
const PILOT_OUT: &str = "target/ripr/release-readiness/pilot";
const OUTCOME_OUT: &str = "target/ripr/release-readiness/targeted-test-outcome.json";
const AGENT_ANALYSIS_OUTCOME_OUT: &str =
    "target/ripr/release-readiness/agent-analysis-outcome.json";
const AGENT_VERIFY_OUT: &str = "target/ripr/release-readiness/agent-verify.json";
const AGENT_RECEIPT_OUT: &str = "target/ripr/release-readiness/agent-receipt.json";
const BOUNDARY_BEFORE_OUT: &str = "target/ripr/release-readiness/before.repo-exposure.json";
const BOUNDARY_AFTER_OUT: &str = "target/ripr/release-readiness/after.repo-exposure.json";
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

struct PublicCliReceipt<'a> {
    status: &'a str,
    report_dir: &'a Path,
    binary: &'a Path,
    fixture_root: &'a Path,
    base: Option<&'a str>,
    head: Option<&'a str>,
    commands: &'a [Value],
    details: &'a [String],
}

struct PublicCliJourney {
    base: String,
    head: String,
    artifacts: Vec<String>,
    commands: Vec<Value>,
    details: Vec<String>,
}

struct AuthenticRepoExposureFixture {
    root: PathBuf,
    before_commit: String,
    after_commit: String,
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
        publish_dry_run_check(version, crate_version.as_deref(), clean_tree.clone()),
        package_install_check(version, crate_version.as_deref()),
        packaged_cli_journey_check(&installed_binary, crate_version.as_deref()),
        installed_command_surface_check(&installed_binary),
        pilot_fixture_check(&installed_binary),
        outcome_fixture_check(&installed_binary),
        agent_verify_fixture_check(&installed_binary, crate_version.as_deref()),
        agent_receipt_fixture_check(&installed_binary),
        repo_exposure_latency_check(),
        lsp_cockpit_check(),
        github_workflow_check(&installed_binary),
        vsix_packaging_check(),
        extension_version_match_check(version, crate_version.as_deref()),
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

fn package_install_check(version: &str, crate_version: Option<&str>) -> ReleaseReadinessCheck {
    let command = format!(
        "cargo package -p ripr --locked && cargo install --path <external-package-root>/ripr-{version} --locked --root {INSTALL_ROOT} --force && installed ripr doctor --root <external-fixture> --json"
    );
    let Some(crate_version) = crate_version else {
        return readiness_check(
            "package-install",
            "not_run",
            false,
            &command,
            "crate version could not be read; release-prep should run this gate explicitly",
            Vec::new(),
            Vec::new(),
        );
    };
    if crate_version != version {
        return readiness_check(
            "package-install",
            "not_run",
            false,
            &command,
            "requested release version does not match the crate version yet",
            Vec::new(),
            vec![format!(
                "requested version: {version}; crates/ripr version: {crate_version}"
            )],
        );
    }
    // Re-read the worktree immediately before packaging.  The initial status
    // snapshot is shared with the other release checks, but this gate owns a
    // package/install proof whose safety depends on the state at this exact
    // point in time.
    match git_worktree_is_clean() {
        Ok(true) => match run_packaged_install(version, crate_version) {
            Ok(result) if result.success => readiness_check(
                "package-install",
                "pass",
                true,
                &command,
                "packaged crate was extracted, installed, identity-checked, and exercised outside the source checkout",
                result.artifacts,
                result.details,
            ),
            Ok(result) => readiness_check(
                "package-install",
                "fail",
                true,
                &command,
                "packaged crate install or external CLI smoke failed",
                result.artifacts,
                result.details,
            ),
            Err(err) => readiness_check(
                "package-install",
                "fail",
                true,
                &command,
                "packaged crate install could not run",
                vec![crate::normalize_path(&installed_ripr_binary())],
                vec![err],
            ),
        },
        Ok(false) => readiness_check(
            "package-install",
            "fail",
            true,
            &command,
            "dirty tree at package/install proof time; package/install proof is not trustworthy",
            Vec::new(),
            Vec::new(),
        ),
        Err(err) => readiness_check(
            "package-install",
            "fail",
            true,
            &command,
            "git worktree state could not be verified before package/install proof",
            Vec::new(),
            vec![err],
        ),
    }
}

struct PackageInstallResult {
    success: bool,
    artifacts: Vec<String>,
    details: Vec<String>,
}

fn run_packaged_install(
    version: &str,
    crate_version: &str,
) -> Result<PackageInstallResult, String> {
    let package_dir = external_package_root()?;
    let extracted_root = package_dir.join(format!("ripr-{version}"));
    let archive = Path::new("target/package").join(format!("ripr-{version}.crate"));
    let workspace_binary =
        Path::new("target/debug").join(format!("ripr{}", std::env::consts::EXE_SUFFIX));
    let installed_binary = installed_ripr_binary();
    let install_root = std::env::current_dir()
        .map_err(|err| format!("read current directory for install root failed: {err}"))?
        .join(INSTALL_ROOT);
    let fixture_root = external_doctor_fixture_root()?;
    let mut artifacts = vec![
        crate::normalize_path(&archive),
        crate::normalize_path(&installed_binary),
    ];
    let mut details = Vec::new();

    let _ = fs::remove_dir_all(&package_dir);
    let _ = fs::remove_dir_all(&install_root);
    let _ = fs::remove_dir_all(&fixture_root);

    let package = run_command("cargo", &["package", "-p", "ripr", "--locked"])
        .map_err(|err| format!("cargo package could not run: {err}"))?;
    details.extend(command_details(&package));
    if !package.success {
        return Ok(PackageInstallResult {
            success: false,
            artifacts,
            details,
        });
    }
    if !archive.is_file() {
        details.push(format!(
            "missing package archive: {}",
            crate::normalize_path(&archive)
        ));
        return Ok(PackageInstallResult {
            success: false,
            artifacts,
            details,
        });
    }
    let archive_digest = crate::reports::release_server::sha256_file(&archive)?;
    details.push(format!("package archive sha256: {archive_digest}"));
    if let Err(err) = extract_packaged_crate(&archive, &package_dir, version) {
        let cleanup = fs::remove_dir_all(&package_dir);
        if let Err(cleanup_err) = cleanup {
            return Err(format!(
                "{err}; package extraction cleanup failed: {cleanup_err}"
            ));
        }
        return Err(err);
    }
    if !extracted_root.is_dir() {
        details.push(format!(
            "missing extracted package root: {}",
            crate::normalize_path(&extracted_root)
        ));
        return Ok(PackageInstallResult {
            success: false,
            artifacts,
            details,
        });
    }
    artifacts.push(crate::normalize_path(&extracted_root));

    let build = run_command("cargo", &["build", "-p", "ripr", "--locked"])
        .map_err(|err| format!("workspace binary build could not run: {err}"))?;
    details.extend(command_details(&build));
    if !build.success || !workspace_binary.is_file() {
        details.push(format!(
            "missing workspace binary: {}",
            crate::normalize_path(&workspace_binary)
        ));
        let _ = fs::remove_dir_all(&package_dir);
        return Ok(PackageInstallResult {
            success: false,
            artifacts,
            details,
        });
    }
    let workspace_digest = crate::reports::release_server::sha256_file(&workspace_binary)?;
    details.push(format!("workspace binary sha256: {workspace_digest}"));

    let install_args = vec![
        "install".to_string(),
        "--path".to_string(),
        ".".to_string(),
        "--locked".to_string(),
        "--root".to_string(),
        crate::normalize_path(&install_root),
        "--force".to_string(),
    ];
    let install = crate::run::capture_output_in_dir(
        "cargo",
        &install_args,
        &extracted_root,
        "cargo install from packaged crate",
    )
    .map_err(|err| format!("cargo install from packaged crate could not run: {err}"))?;
    let install_result = CommandResult {
        status: install.status.code(),
        success: install.status.success(),
        stdout: install.stdout,
        stderr: install.stderr,
    };
    details.extend(command_details(&install_result));
    if !install_result.success || !installed_binary.is_file() {
        details.push(format!(
            "missing installed binary: {}",
            crate::normalize_path(&installed_binary)
        ));
        let _ = fs::remove_dir_all(&package_dir);
        return Ok(PackageInstallResult {
            success: false,
            artifacts,
            details,
        });
    }
    let installed_binary = fs::canonicalize(&installed_binary).map_err(|err| {
        format!("canonicalize installed binary for external execution failed: {err}")
    })?;
    let installed_digest = crate::reports::release_server::sha256_file(&installed_binary)?;
    details.push(format!("installed binary sha256: {installed_digest}"));
    if let Err(err) = validate_binary_identity(&workspace_digest, &installed_digest) {
        details.push(err);
        return Ok(PackageInstallResult {
            success: false,
            artifacts,
            details,
        });
    }

    let version_result = run_command_path(&installed_binary, &["--version"])
        .map_err(|err| format!("installed ripr --version could not run: {err}"))?;
    details.extend(command_details(&version_result));
    if let Err(err) = validate_installed_version(
        version_result.success,
        &version_result.stdout,
        crate_version,
    ) {
        details.push(err);
        return Ok(PackageInstallResult {
            success: false,
            artifacts,
            details,
        });
    }

    fs::remove_dir_all(&package_dir)
        .map_err(|err| format!("remove extracted package after install failed: {err}"))?;
    let fixture = create_external_doctor_fixture(&fixture_root)?;
    let doctor_args = vec![
        "doctor".to_string(),
        "--root".to_string(),
        crate::normalize_path(&fixture),
        "--json".to_string(),
    ];
    let doctor = match crate::run::capture_output_in_dir(
        &installed_binary.to_string_lossy(),
        &doctor_args,
        &fixture,
        "installed ripr doctor",
    ) {
        Ok(output) => output,
        Err(err) => {
            let cleanup = fs::remove_dir_all(&fixture_root);
            details.push(format!("installed ripr doctor could not run: {err}"));
            if let Err(cleanup_err) = cleanup {
                details.push(format!(
                    "external doctor fixture cleanup failed: {cleanup_err}"
                ));
            }
            return Ok(PackageInstallResult {
                success: false,
                artifacts,
                details,
            });
        }
    };
    let doctor_result = CommandResult {
        status: doctor.status.code(),
        success: doctor.status.success(),
        stdout: doctor.stdout,
        stderr: doctor.stderr,
    };
    details.extend(command_details(&doctor_result));
    let doctor_json: Value = match serde_json::from_str(&doctor_result.stdout) {
        Ok(value) => value,
        Err(err) => {
            let cleanup = fs::remove_dir_all(&fixture_root);
            details.push(format!(
                "installed ripr doctor emitted malformed JSON: {err}"
            ));
            if let Err(cleanup_err) = cleanup {
                details.push(format!(
                    "external doctor fixture cleanup failed: {cleanup_err}"
                ));
            }
            return Ok(PackageInstallResult {
                success: false,
                artifacts,
                details,
            });
        }
    };
    if let Err(err) = validate_doctor_result(doctor_result.success, &doctor_json) {
        details.push(err);
        if let Err(cleanup_err) = fs::remove_dir_all(&fixture_root) {
            details.push(format!(
                "external doctor fixture cleanup failed: {cleanup_err}"
            ));
        }
        return Ok(PackageInstallResult {
            success: false,
            artifacts,
            details,
        });
    }

    fs::remove_dir_all(&fixture_root)
        .map_err(|err| format!("remove external doctor fixture failed: {err}"))?;
    details.push(format!(
        "external doctor fixture cleaned: {}",
        crate::normalize_path(&fixture_root)
    ));
    Ok(PackageInstallResult {
        success: true,
        artifacts,
        details,
    })
}

fn extract_packaged_crate(archive: &Path, destination: &Path, version: &str) -> Result<(), String> {
    let file = fs::File::open(archive).map_err(|err| {
        format!(
            "open package archive {} failed: {err}",
            crate::normalize_path(archive)
        )
    })?;
    let decoder = GzDecoder::new(file);
    let mut tar = Archive::new(decoder);
    let expected_root = PathBuf::from(format!("ripr-{version}"));
    fs::create_dir_all(destination)
        .map_err(|err| format!("create package extraction directory failed: {err}"))?;
    for (index, entry_result) in tar
        .entries()
        .map_err(|err| format!("read package archive entries failed: {err}"))?
        .enumerate()
    {
        let mut entry =
            entry_result.map_err(|err| format!("read package entry {index} failed: {err}"))?;
        let enclosed = entry
            .path()
            .map_err(|err| format!("read package entry {index} path failed: {err}"))?
            .into_owned();
        validate_package_entry(&enclosed, entry.header().entry_type(), &expected_root)?;
        let output = destination.join(&enclosed);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "create package parent {} failed: {err}",
                    crate::normalize_path(parent)
                )
            })?;
        }
        entry.unpack(&output).map_err(|err| {
            format!(
                "extract package entry {} failed: {err}",
                crate::normalize_path(&output)
            )
        })?;
    }
    Ok(())
}

fn validate_package_entry(
    enclosed: &Path,
    entry_type: tar::EntryType,
    expected_root: &Path,
) -> Result<(), String> {
    if !enclosed.starts_with(expected_root) {
        return Err(format!(
            "package entry {:?} is outside expected root {expected_root:?}",
            enclosed
        ));
    }
    if enclosed.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return Err(format!(
            "package entry {:?} escapes extraction root",
            enclosed
        ));
    }
    if entry_type.is_symlink() || entry_type.is_hard_link() {
        return Err(format!("package entry {:?} is a link", enclosed));
    }
    Ok(())
}

fn validate_binary_identity(workspace_digest: &str, installed_digest: &str) -> Result<(), String> {
    if installed_digest == workspace_digest {
        return Err("installed binary unexpectedly matches workspace binary digest".to_string());
    }
    Ok(())
}

fn validate_installed_version(success: bool, stdout: &str, version: &str) -> Result<(), String> {
    if !success || !stdout.contains(&format!("ripr {version}")) {
        return Err(
            "installed binary version output did not identify the packaged crate version"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_doctor_result(success: bool, doctor_json: &Value) -> Result<(), String> {
    if !success || doctor_json.get("status").and_then(Value::as_str) != Some("pass") {
        return Err(
            "installed ripr doctor did not report pass for the external fixture".to_string(),
        );
    }
    Ok(())
}

fn external_doctor_fixture_root() -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock is before Unix epoch: {err}"))?
        .as_nanos();
    Ok(release_temp_root()?.join(format!(
        "ripr-release-doctor-{}-{stamp}",
        std::process::id()
    )))
}

fn external_package_root() -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock is before Unix epoch: {err}"))?
        .as_nanos();
    Ok(release_temp_root()?.join(format!(
        "ripr-release-package-{}-{stamp}",
        std::process::id()
    )))
}

fn external_cli_fixture_root() -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock is before Unix epoch: {err}"))?
        .as_nanos();
    Ok(release_temp_root()?.join(format!(
        "ripr-release-cli-fixture with spaces-{}-{stamp}",
        std::process::id()
    )))
}

fn release_temp_root() -> Result<PathBuf, String> {
    let configured = std::env::temp_dir();
    let current = std::env::current_dir()
        .map_err(|err| format!("read current directory for release fixture failed: {err}"))?;
    let current = fs::canonicalize(&current).map_err(|err| {
        format!("canonicalize current directory for release fixture failed: {err}")
    })?;
    let mut candidate = fs::canonicalize(&configured).map_err(|err| {
        format!("canonicalize temporary directory for release fixture failed: {err}")
    })?;
    for _ in 0..64 {
        if candidate != current && !candidate.starts_with(&current) {
            return Ok(candidate);
        }
        candidate = candidate
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "configured temporary directory has no external parent".to_string())?;
    }
    Err("could not find an external release fixture directory within 64 parent steps".to_string())
}

fn create_external_doctor_fixture(root: &Path) -> Result<PathBuf, String> {
    let source = root.join("src");
    fs::create_dir_all(&source)
        .map_err(|err| format!("create external doctor fixture failed: {err}"))?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"release-doctor-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .map_err(|err| format!("write external doctor fixture manifest failed: {err}"))?;
    fs::write(
        source.join("lib.rs"),
        "pub fn fixture_marker() -> &'static str { \"ok\" }\n",
    )
    .map_err(|err| format!("write external doctor fixture source failed: {err}"))?;
    Ok(root.to_path_buf())
}

fn run_git_output_in_dir(root: &Path, args: &[&str]) -> Result<String, String> {
    let mut owned = vec![
        "-c".to_string(),
        "core.hooksPath=".to_string(),
        "-c".to_string(),
        "init.templateDir=".to_string(),
        "-c".to_string(),
        "commit.template=".to_string(),
    ];
    owned.extend(
        args.iter()
            .map(|arg| (*arg).to_string())
            .collect::<Vec<_>>(),
    );
    let output = crate::run::capture_output_in_dir_with_envs(
        "git",
        &owned,
        root,
        "external CLI fixture git command",
        &[("GIT_CONFIG_NOSYSTEM", "1")],
        &["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"],
    )?;
    let result = CommandResult {
        status: output.status.code(),
        success: output.status.success(),
        stdout: output.stdout,
        stderr: output.stderr,
    };
    if !result.success {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            command_details(&result).join("; ")
        ));
    }
    Ok(result.stdout.trim().to_string())
}

fn create_external_cli_fixture(root: &Path) -> Result<(String, String), String> {
    fs::create_dir_all(root.join("src"))
        .map_err(|err| format!("create external CLI fixture source failed: {err}"))?;
    fs::create_dir_all(root.join("tests"))
        .map_err(|err| format!("create external CLI fixture tests failed: {err}"))?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"external-cli-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .map_err(|err| format!("write external CLI fixture manifest failed: {err}"))?;
    fs::write(
        root.join("src/lib.rs"),
        "pub fn classify(value: i32) -> i32 {\n    if value > 0 { 1 } else { 0 }\n}\n",
    )
    .map_err(|err| format!("write external CLI fixture base source failed: {err}"))?;
    fs::write(
        root.join("tests/behavior.rs"),
        "#[test]\nfn positive_value_is_classified() {\n    assert_eq!(external_cli_fixture::classify(1), 1);\n}\n",
    )
    .map_err(|err| format!("write external CLI fixture test failed: {err}"))?;
    run_git_output_in_dir(root, &["init", "--quiet"])?;
    run_git_output_in_dir(
        root,
        &["config", "user.email", "ripr-release@example.invalid"],
    )?;
    run_git_output_in_dir(root, &["config", "user.name", "RIPR Release Fixture"])?;
    run_git_output_in_dir(root, &["add", "."])?;
    run_git_output_in_dir(root, &["commit", "--no-gpg-sign", "--quiet", "-m", "base"])?;
    let base = run_git_output_in_dir(root, &["rev-parse", "HEAD"])?;

    fs::write(
        root.join("src/lib.rs"),
        "pub fn classify(value: i32) -> i32 {\n    if value >= 0 { 1 } else { 0 }\n}\n",
    )
    .map_err(|err| format!("write external CLI fixture head source failed: {err}"))?;
    run_git_output_in_dir(root, &["add", "."])?;
    run_git_output_in_dir(root, &["commit", "--no-gpg-sign", "--quiet", "-m", "head"])?;
    let head = run_git_output_in_dir(root, &["rev-parse", "HEAD"])?;
    Ok((base, head))
}

fn first_finding_id(value: &Value) -> Option<String> {
    value
        .get("findings")
        .and_then(Value::as_array)
        .and_then(|findings| {
            findings.iter().find_map(|finding| {
                finding
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
        })
}

fn record_journey_command(
    commands: &mut Vec<Value>,
    name: &str,
    args: &[String],
    result: &CommandResult,
) {
    let argv = args
        .iter()
        .map(|arg| {
            if Path::new(arg).is_absolute() {
                crate::normalize_path(Path::new(arg))
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>();
    commands.push(json!({
        "name": name,
        "argv": argv,
        "status": if result.success { "pass" } else { "fail" },
        "exit_code": result.status,
        "stdout_preview": result.stdout.lines().take(3).collect::<Vec<_>>(),
        "stderr_preview": result.stderr.lines().take(3).collect::<Vec<_>>(),
    }));
}

fn write_public_cli_receipt(receipt: &PublicCliReceipt<'_>) -> Result<PathBuf, String> {
    let path = receipt.report_dir.join(format!(
        "packaged-cli-journey-{}.json",
        std::env::consts::OS
    ));
    let value = json!({
        "schema_version": "0.1",
        "kind": "ripr_packaged_cli_journey",
        "status": receipt.status,
        "platform": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
        "binary": crate::normalize_path(receipt.binary),
        "fixture_root": crate::normalize_path(receipt.fixture_root),
        "base_sha": receipt.base,
        "head_sha": receipt.head,
        "commands": receipt.commands,
        "details": receipt.details,
        "non_publication": true,
    });
    let text = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("render packaged CLI receipt failed: {err}"))?;
    fs::write(&path, format!("{text}\n"))
        .map_err(|err| format!("write packaged CLI receipt failed: {err}"))?;
    Ok(path)
}

fn packaged_cli_journey_check(binary: &Path, crate_version: Option<&str>) -> ReleaseReadinessCheck {
    let command = format!(
        "{bin} --version; {bin} check --root <external-fixture> --base <base> --json --write-artifact <check-artifact>; {bin} diff --root <external-fixture> --base <base> --head <head> --json; {bin} explain --root <external-fixture> --from <check-artifact> <finding>; {bin} context --root <external-fixture> --from <check-artifact> --at <finding> --json; {bin} pilot --root <external-fixture> --out <pilot-out> --timeout-ms 30000",
        bin = crate::normalize_path(binary),
    );
    let binary = match fs::canonicalize(binary) {
        Ok(path) => path,
        Err(err) => {
            return readiness_check(
                "packaged-cli-journey",
                "fail",
                true,
                &command,
                "installed binary could not be canonicalized for external CLI journey",
                Vec::new(),
                vec![err.to_string()],
            );
        }
    };
    let report_dir = match std::env::current_dir() {
        Ok(current_dir) => current_dir.join(REPORT_WORK_DIR),
        Err(err) => {
            return readiness_check(
                "packaged-cli-journey",
                "fail",
                true,
                &command,
                "readiness report directory could not be determined",
                Vec::new(),
                vec![err.to_string()],
            );
        }
    };
    if let Err(err) = clear_packaged_cli_artifacts(&report_dir) {
        return readiness_check(
            "packaged-cli-journey",
            "fail",
            true,
            &command,
            "stale packaged CLI artifacts could not be cleared",
            Vec::new(),
            vec![err],
        );
    }
    let fixture_root = match external_cli_fixture_root() {
        Ok(path) => path,
        Err(err) => {
            return readiness_check(
                "packaged-cli-journey",
                "fail",
                true,
                &command,
                "external CLI fixture root could not be created",
                Vec::new(),
                vec![err],
            );
        }
    };
    let mut commands = Vec::new();
    let mut details = vec![format!("platform: {}", std::env::consts::OS)];
    let journey = (|| -> Result<PublicCliJourney, String> {
        let expected_version = crate_version.ok_or_else(|| {
            "crate version could not be read for packaged CLI journey".to_string()
        })?;
        let version_args = vec!["--version".to_string()];
        let version = run_command_in_dir(
            &binary,
            &version_args,
            &std::env::current_dir().map_err(|err| {
                format!("read current directory for packaged CLI version failed: {err}")
            })?,
            "installed packaged CLI version",
        )?;
        record_journey_command(&mut commands, "version", &version_args, &version);
        validate_installed_version(version.success, &version.stdout, expected_version)?;
        details.push(format!("packaged version: {expected_version}"));
        let (base, head) = create_external_cli_fixture(&fixture_root)?;
        details.push(format!("base sha: {base}"));
        details.push(format!("head sha: {head}"));
        let binary_text = binary.to_string_lossy().into_owned();
        let root_text = fixture_root.to_string_lossy().into_owned();
        let check_artifact = report_dir.join("packaged-cli-check.json");
        let check_artifact_text = check_artifact.to_string_lossy().into_owned();
        let check_args = vec![
            "check".to_string(),
            "--root".to_string(),
            root_text.clone(),
            "--base".to_string(),
            base.clone(),
            "--json".to_string(),
            "--write-artifact".to_string(),
            check_artifact_text.clone(),
        ];
        let check = run_command_in_dir(
            &binary_text,
            &check_args,
            &fixture_root,
            "installed packaged CLI check",
        )?;
        record_journey_command(&mut commands, "check", &check_args, &check);
        if !check.success {
            return Err(format!(
                "packaged CLI check failed: {}",
                command_details(&check).join("; ")
            ));
        }
        let check_json: Value = serde_json::from_str(&check.stdout)
            .map_err(|err| format!("packaged CLI check emitted malformed JSON: {err}"))?;
        if check_json.get("base").and_then(Value::as_str) != Some(base.as_str()) {
            return Err(
                "packaged CLI check did not retain the requested base identity".to_string(),
            );
        }
        if check_json
            .get("analysis_outcome")
            .and_then(|outcome| outcome.get("analysis_complete"))
            != Some(&Value::Bool(true))
        {
            return Err("packaged CLI check did not report complete analysis".to_string());
        }
        let finding = first_finding_id(&check_json)
            .ok_or_else(|| "packaged CLI check produced no finding selector".to_string())?;
        if !check_artifact.is_file() {
            return Err(format!(
                "packaged CLI check did not write artifact {}",
                crate::normalize_path(&check_artifact)
            ));
        }
        let diff_args = vec![
            "diff".to_string(),
            "--root".to_string(),
            root_text.clone(),
            "--base".to_string(),
            base.clone(),
            "--head".to_string(),
            head.clone(),
            "--json".to_string(),
        ];
        let diff = run_command_in_dir(
            &binary_text,
            &diff_args,
            &fixture_root,
            "installed packaged CLI exact diff",
        )?;
        record_journey_command(&mut commands, "diff", &diff_args, &diff);
        if !diff.success {
            return Err(format!(
                "packaged CLI exact diff failed: {}",
                command_details(&diff).join("; ")
            ));
        }
        let diff_json: Value = serde_json::from_str(&diff.stdout)
            .map_err(|err| format!("packaged CLI exact diff emitted malformed JSON: {err}"))?;
        if diff_json.get("base").and_then(Value::as_str) != Some(base.as_str())
            || diff_json.get("head").and_then(Value::as_str) != Some(head.as_str())
        {
            return Err(
                "packaged CLI exact diff did not retain both revision identities".to_string(),
            );
        }
        let explain_args = vec![
            "explain".to_string(),
            "--root".to_string(),
            root_text.clone(),
            "--from".to_string(),
            check_artifact_text.clone(),
            finding.clone(),
        ];
        let explain = run_command_in_dir(
            &binary_text,
            &explain_args,
            &fixture_root,
            "installed packaged CLI explain",
        )?;
        record_journey_command(&mut commands, "explain", &explain_args, &explain);
        if !explain.success || explain.stdout.trim().is_empty() {
            return Err("packaged CLI explain failed or emitted no output".to_string());
        }
        if !explain.stdout.contains(&finding) {
            return Err(
                "packaged CLI explain did not retain the requested finding identity".to_string(),
            );
        }
        let context_args = vec![
            "context".to_string(),
            "--root".to_string(),
            root_text.clone(),
            "--from".to_string(),
            check_artifact_text,
            "--at".to_string(),
            finding.clone(),
            "--json".to_string(),
        ];
        let context = run_command_in_dir(
            &binary_text,
            &context_args,
            &fixture_root,
            "installed packaged CLI context",
        )?;
        record_journey_command(&mut commands, "context", &context_args, &context);
        if !context.success {
            return Err("packaged CLI context failed".to_string());
        }
        let context_json: Value = serde_json::from_str(&context.stdout)
            .map_err(|err| format!("packaged CLI context emitted malformed JSON: {err}"))?;
        if context_json
            .get("probe")
            .and_then(|probe| probe.get("id"))
            .and_then(Value::as_str)
            != Some(finding.as_str())
        {
            return Err(
                "packaged CLI context did not retain the requested finding identity".to_string(),
            );
        }
        let pilot_out = fixture_root.join("pilot-output");
        let pilot_out_text = pilot_out.to_string_lossy().into_owned();
        let pilot_args = vec![
            "pilot".to_string(),
            "--root".to_string(),
            root_text,
            "--out".to_string(),
            pilot_out_text,
            "--timeout-ms".to_string(),
            "30000".to_string(),
        ];
        let pilot = run_command_in_dir(
            &binary_text,
            &pilot_args,
            &fixture_root,
            "installed packaged CLI pilot",
        )?;
        record_journey_command(&mut commands, "pilot", &pilot_args, &pilot);
        if !pilot.success {
            return Err("packaged CLI pilot failed".to_string());
        }
        let packet = pilot_out.join("agent-seam-packets.json");
        let summary = pilot_out.join("pilot-summary.json");
        for path in [&packet, &summary] {
            if !path.is_file() {
                return Err(format!(
                    "packaged CLI pilot did not write {}",
                    crate::normalize_path(path)
                ));
            }
            let text = fs::read_to_string(path)
                .map_err(|err| format!("read packaged CLI pilot artifact failed: {err}"))?;
            let value: Value = serde_json::from_str(&text)
                .map_err(|err| format!("packaged CLI pilot artifact is malformed JSON: {err}"))?;
            if path == &packet {
                let run_status =
                    value
                        .get("run_status")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            "packaged CLI pilot packet artifact omitted run_status".to_string()
                        })?;
                if run_status != "complete" {
                    return Err(format!(
                        "packaged CLI pilot did not complete: run_status={run_status}"
                    ));
                }
                let packets_total = value
                    .get("packets_total")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| {
                        "packaged CLI pilot packet artifact omitted numeric packets_total"
                            .to_string()
                    })?;
                details.push(format!("pilot run status: {run_status}"));
                details.push(format!("pilot packets: {packets_total}"));
            } else {
                let status = value
                    .get("status")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "packaged CLI pilot summary omitted status".to_string())?;
                if status != "complete" {
                    return Err(format!(
                        "packaged CLI pilot summary was not complete: status={status}"
                    ));
                }
                details.push(format!("pilot summary status: {status}"));
            }
        }
        let retained_packet = report_dir.join("packaged-cli-agent-seam-packets.json");
        let retained_summary = report_dir.join("packaged-cli-pilot-summary.json");
        fs::copy(&packet, &retained_packet)
            .map_err(|err| format!("retain packaged CLI packet failed: {err}"))?;
        fs::copy(&summary, &retained_summary)
            .map_err(|err| format!("retain packaged CLI summary failed: {err}"))?;
        Ok(PublicCliJourney {
            base,
            head,
            artifacts: vec![
                crate::normalize_path(&check_artifact),
                crate::normalize_path(&retained_packet),
                crate::normalize_path(&retained_summary),
            ],
            commands: std::mem::take(&mut commands),
            details: std::mem::take(&mut details),
        })
    })();
    let cleanup = if fixture_root.exists() {
        fs::remove_dir_all(&fixture_root)
    } else {
        Ok(())
    };
    match (journey, cleanup) {
        (Ok(journey), Ok(())) => match write_public_cli_receipt(&PublicCliReceipt {
            status: "qualified",
            report_dir: &report_dir,
            binary: &binary,
            fixture_root: &fixture_root,
            base: Some(&journey.base),
            head: Some(&journey.head),
            commands: &journey.commands,
            details: &journey.details,
        }) {
            Ok(receipt) => {
                let mut artifacts = vec![crate::normalize_path(&receipt)];
                artifacts.extend(journey.artifacts);
                readiness_check(
                    "packaged-cli-journey",
                    "pass",
                    true,
                    &command,
                    "installed packaged CLI completed external check, exact diff, explain/context, and pilot packet journey",
                    artifacts,
                    journey.details,
                )
            }
            Err(err) => readiness_check(
                "packaged-cli-journey",
                "fail",
                true,
                &command,
                "packaged CLI journey passed but receipt writing failed",
                Vec::new(),
                vec![err],
            ),
        },
        (Ok(journey), Err(err)) => readiness_check(
            "packaged-cli-journey",
            "fail",
            true,
            &command,
            "packaged CLI journey passed but external fixture cleanup failed",
            Vec::new(),
            {
                let mut details = journey.details;
                details.push(format!("cleanup error: {err}"));
                details
            },
        ),
        (Err(err), Ok(())) => readiness_check(
            "packaged-cli-journey",
            "fail",
            true,
            &command,
            "installed packaged CLI external journey failed",
            Vec::new(),
            {
                details.push(err);
                details
            },
        ),
        (Err(err), Err(cleanup_err)) => readiness_check(
            "packaged-cli-journey",
            "fail",
            true,
            &command,
            "installed packaged CLI journey and fixture cleanup failed",
            Vec::new(),
            {
                details.push(err);
                details.push(format!("cleanup error: {cleanup_err}"));
                details
            },
        ),
    }
}

fn clear_packaged_cli_artifacts(report_dir: &Path) -> Result<(), String> {
    let names = [
        format!("packaged-cli-journey-{}.json", std::env::consts::OS),
        "packaged-cli-check.json".to_string(),
        "packaged-cli-agent-seam-packets.json".to_string(),
        "packaged-cli-pilot-summary.json".to_string(),
    ];
    for name in names {
        let path = report_dir.join(name);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(format!(
                    "remove stale packaged CLI artifact {} failed: {err}",
                    crate::normalize_path(&path)
                ));
            }
        }
    }
    Ok(())
}

/// Commands the bounded first screen (`ripr --help`) must keep offering.
///
/// Only the first actions belong here. This is the screen a first-time reader
/// sees, so the gate holds it to "can a new user start?" — not to the full
/// catalog. `repository_first_screen_offers_the_first_run_commands` pins these
/// against the shipped help text so a release cannot be cut on a first screen
/// that dropped its own entry point.
const FIRST_SCREEN_NEEDLES: &[&str] = &["ripr doctor", "ripr check", "ripr first-pr"];

/// Release-loop commands the exhaustive reference (`ripr help --all`) must list.
///
/// These used to be grepped out of `--help`. #1613 moved the catalog behind
/// `help --all`, which is now the surface that promises completeness — and
/// `help_all_documents_every_public_command` binds that text to the parser's own
/// command list, so this needle set is checking a claim something else keeps
/// honest rather than checking prose in isolation.
const RELEASE_LOOP_NEEDLES: &[&str] = &[
    "ripr pilot",
    "ripr outcome",
    "ripr first-pr",
    "ripr calibrate cargo-mutants",
    "ripr agent verify",
    "ripr agent receipt",
];

fn installed_command_surface_check(binary: &Path) -> ReleaseReadinessCheck {
    let binary_path = crate::normalize_path(binary);
    let command = format!(
        "{binary_path} --version && {binary_path} --help && {binary_path} help --all && {binary_path} first-pr --help"
    );
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
    let help_all = match run_command_path(binary, &["help", "--all"]) {
        Ok(result) if result.success => result,
        Ok(result) => {
            return readiness_check(
                "installed-command-surface",
                "fail",
                true,
                &command,
                "installed binary help --all failed",
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
                "installed ripr help --all could not run",
                vec![crate::normalize_path(binary)],
                vec![err],
            );
        }
    };
    // Two surfaces, two claims. `--help` is the bounded first screen, so it is
    // held only to the commands a first run needs; the full release-loop
    // catalog moved behind `help --all` (#1613) and is checked there. Grepping
    // `--help` for the whole catalog would fail the release on a deliberate
    // presentation change, and grepping only `help --all` would let the first
    // screen lose its first actions without any gate noticing.
    let mut missing = missing_required_needles(&help.stdout, FIRST_SCREEN_NEEDLES);
    missing.extend(missing_required_needles(
        &help_all.stdout,
        RELEASE_LOOP_NEEDLES,
    ));
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

fn agent_verify_fixture_check(
    binary: &Path,
    _producer_version: Option<&str>,
) -> ReleaseReadinessCheck {
    let command = format!(
        "{} check --root . --mode draft --format repo-exposure-json; agent verify; agent receipt",
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
    let binary = match absolute_installed_binary(binary) {
        Ok(path) => path,
        Err(err) => {
            return readiness_check(
                "agent-verify-boundary-fixture",
                "fail",
                true,
                &command,
                "installed binary path is not usable inside the external fixture",
                Vec::new(),
                vec![err],
            );
        }
    };
    match run_authentic_repo_exposure_journey(&binary) {
        Ok(details) => readiness_check(
            "agent-verify-boundary-fixture",
            "pass",
            true,
            &command,
            "installed RIPR produced and verified authentic before/after repo-exposure artifacts",
            vec![
                BOUNDARY_BEFORE_OUT.to_string(),
                BOUNDARY_AFTER_OUT.to_string(),
                AGENT_ANALYSIS_OUTCOME_OUT.to_string(),
                AGENT_VERIFY_OUT.to_string(),
                AGENT_RECEIPT_OUT.to_string(),
            ],
            details,
        ),
        Err(err) => readiness_check(
            "agent-verify-boundary-fixture",
            "fail",
            true,
            &command,
            "installed RIPR could not complete the authentic before/after producer journey",
            vec![
                BOUNDARY_BEFORE_OUT.to_string(),
                BOUNDARY_AFTER_OUT.to_string(),
                AGENT_ANALYSIS_OUTCOME_OUT.to_string(),
                AGENT_VERIFY_OUT.to_string(),
                AGENT_RECEIPT_OUT.to_string(),
            ],
            vec![err],
        ),
    }
}

/// The authentic producer journey spawns the installed binary with the
/// external fixture as its working directory, so the checkout-relative
/// `installed_ripr_binary()` path would not resolve there. Resolve it to an
/// absolute path before the journey begins.
fn absolute_installed_binary(binary: &Path) -> Result<PathBuf, String> {
    let resolved = fs::canonicalize(binary)
        .map_err(|err| format!("canonicalize installed binary failed: {err}"))?;
    if !resolved.is_absolute() {
        return Err(format!(
            "installed binary did not resolve to an absolute path: {}",
            crate::normalize_path(&resolved)
        ));
    }
    Ok(resolved)
}

fn run_authentic_repo_exposure_journey(binary: &Path) -> Result<Vec<String>, String> {
    let fixture = create_authentic_repo_exposure_fixture()?;
    let result = run_authentic_repo_exposure_journey_in_fixture(binary, &fixture);
    let cleanup = fs::remove_dir_all(&fixture.root);
    let mut details = match result {
        Ok(details) => details,
        Err(error) => return Err(format!("{error}; fixture cleanup: {cleanup:?}")),
    };
    cleanup.map_err(|err| format!("remove authentic fixture failed: {err}"))?;
    details.push(format!(
        "external producer fixture cleaned: {}",
        crate::normalize_path(&fixture.root)
    ));
    Ok(details)
}

fn create_authentic_repo_exposure_fixture() -> Result<AuthenticRepoExposureFixture, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock is before Unix epoch: {err}"))?
        .as_nanos();
    let root = release_temp_root()?.join(format!(
        "ripr-release-exposure-{}-{stamp}",
        std::process::id()
    ));
    let result = (|| {
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create authentic fixture source failed: {err}"))?;
        fs::create_dir_all(root.join("tests"))
            .map_err(|err| format!("create authentic fixture tests failed: {err}"))?;
        for relative in ["Cargo.toml", "src/lib.rs", "tests/pricing.rs"] {
            let source = Path::new("fixtures/boundary_gap/input").join(relative);
            let destination = root.join(relative);
            fs::copy(&source, &destination).map_err(|err| {
                format!(
                    "copy authentic fixture {} to {} failed: {err}",
                    crate::normalize_path(&source),
                    crate::normalize_path(&destination)
                )
            })?;
        }
        run_fixture_git_command(&root, &["init", "--quiet", "--template="], "initialize")?;
        run_fixture_git_command(
            &root,
            &["config", "user.name", "RIPR Release Fixture"],
            "configure user name",
        )?;
        run_fixture_git_command(
            &root,
            &["config", "user.email", "release-fixture@example.invalid"],
            "configure user email",
        )?;
        run_fixture_git_command(
            &root,
            &["config", "commit.gpgSign", "false"],
            "disable signing",
        )?;
        run_fixture_git_command(
            &root,
            &["-c", "core.hooksPath=", "add", "."],
            "stage before state",
        )?;
        run_fixture_git_command(
            &root,
            &["-c", "core.hooksPath=", "commit", "-m", "fixture before"],
            "commit before state",
        )?;
        let before_commit = fixture_head(&root)?;

        let tests_path = root.join("tests/pricing.rs");
        let mut tests = fs::OpenOptions::new()
            .append(true)
            .open(&tests_path)
            .map_err(|err| format!("open authentic fixture tests for update failed: {err}"))?;
        writeln!(tests)
            .map_err(|err| format!("write authentic fixture separator failed: {err}"))?;
        writeln!(tests, "#[test]")
            .map_err(|err| format!("write authentic fixture test failed: {err}"))?;
        writeln!(tests, "fn equality_boundary_discounts() {{")
            .map_err(|err| format!("write authentic fixture test body failed: {err}"))?;
        writeln!(tests, "    assert_eq!(discounted_total(100, 100), 90);")
            .map_err(|err| format!("write authentic fixture assertion failed: {err}"))?;
        writeln!(tests, "}}")
            .map_err(|err| format!("write authentic fixture test close failed: {err}"))?;
        drop(tests);
        run_fixture_git_command(
            &root,
            &["-c", "core.hooksPath=", "add", "."],
            "stage after state",
        )?;
        run_fixture_git_command(
            &root,
            &["-c", "core.hooksPath=", "commit", "-m", "fixture after"],
            "commit after state",
        )?;
        let after_commit = fixture_head(&root)?;
        if before_commit == after_commit {
            return Err("authentic fixture before and after commits are identical".to_string());
        }
        let ancestor = run_fixture_git_command(
            &root,
            &["merge-base", "--is-ancestor", &before_commit, &after_commit],
            "compare fixture commits",
        )?;
        if !ancestor.success {
            return Err(
                "authentic fixture after commit is not descended from before commit".to_string(),
            );
        }
        Ok(AuthenticRepoExposureFixture {
            root: root.clone(),
            before_commit,
            after_commit,
        })
    })();
    result.inspect_err(|_error| {
        let _ = fs::remove_dir_all(&root);
    })
}

fn run_authentic_repo_exposure_journey_in_fixture(
    binary: &Path,
    fixture: &AuthenticRepoExposureFixture,
) -> Result<Vec<String>, String> {
    let before_name = "before.repo-exposure.json";
    let after_name = "after.repo-exposure.json";
    checkout_fixture_commit(&fixture.root, &fixture.before_commit)?;
    let _before = run_producer_check(binary, &fixture.root, before_name)?;
    checkout_fixture_commit(&fixture.root, &fixture.after_commit)?;
    let _after = run_producer_check(binary, &fixture.root, after_name)?;
    validate_authentic_artifact(
        &fixture.root.join(before_name),
        &fixture.before_commit,
        "before",
    )?;
    validate_authentic_artifact(
        &fixture.root.join(after_name),
        &fixture.after_commit,
        "after",
    )?;
    let before_value = read_json_value(&fixture.root.join(before_name))?;
    let after_value = read_json_value(&fixture.root.join(after_name))?;
    let before_input = artifact_string(&before_value, &["artifact", "analysis", "input_identity"])?;
    let after_input = artifact_string(&after_value, &["artifact", "analysis", "input_identity"])?;
    let before_snapshot = artifact_string(&before_value, &["artifact", "snapshot_identity"])?;
    let after_snapshot = artifact_string(&after_value, &["artifact", "snapshot_identity"])?;
    if before_input != after_input || before_snapshot == after_snapshot {
        return Err(
            "authentic producer did not preserve comparable input identity and distinguish before/after snapshot identities"
                .to_string(),
        );
    }
    run_analysis_outcome_check(binary, &fixture.root)?;
    let verify_args = vec![
        "agent".to_string(),
        "verify".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--before".to_string(),
        before_name.to_string(),
        "--after".to_string(),
        after_name.to_string(),
        "--json".to_string(),
    ];
    let verify = run_command_in_dir(
        binary,
        &verify_args,
        &fixture.root,
        "authentic agent verify",
    )?;
    if !verify.success {
        return Err(format!(
            "authentic agent verify failed: {}",
            command_details(&verify).join("; ")
        ));
    }
    fs::write(fixture.root.join("agent-verify.json"), &verify.stdout)
        .map_err(|err| format!("write authentic agent verify artifact failed: {err}"))?;
    let receipt_args = vec![
        "agent".to_string(),
        "receipt".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--verify-json".to_string(),
        "agent-verify.json".to_string(),
        "--seam-id".to_string(),
        BOUNDARY_GAP_SEAM_ID.to_string(),
        "--json".to_string(),
        "--out".to_string(),
        "agent-receipt.json".to_string(),
    ];
    let receipt = run_command_in_dir(
        binary,
        &receipt_args,
        &fixture.root,
        "authentic agent receipt",
    )?;
    if !receipt.success || !fixture.root.join("agent-receipt.json").is_file() {
        return Err(format!(
            "authentic agent receipt failed: {}",
            command_details(&receipt).join("; ")
        ));
    }
    for (source, destination) in [
        (before_name, BOUNDARY_BEFORE_OUT),
        (after_name, BOUNDARY_AFTER_OUT),
        ("analysis-outcome.json", AGENT_ANALYSIS_OUTCOME_OUT),
        ("agent-verify.json", AGENT_VERIFY_OUT),
        ("agent-receipt.json", AGENT_RECEIPT_OUT),
    ] {
        fs::copy(fixture.root.join(source), destination).map_err(|err| {
            format!("retain authentic artifact {source} at {destination} failed: {err}")
        })?;
    }
    Ok(vec![
        format!("authentic fixture before commit: {}", fixture.before_commit),
        format!("authentic fixture after commit: {}", fixture.after_commit),
        format!("before input identity: {before_input}"),
        format!("after input identity: {after_input}"),
        format!("before snapshot identity: {before_snapshot}"),
        format!("after snapshot identity: {after_snapshot}"),
        "producer artifacts, canonical analysis outcome, verify output, and receipt retained under target/ripr/release-readiness".to_string(),
    ])
}

fn run_producer_check(binary: &Path, root: &Path, artifact_name: &str) -> Result<Value, String> {
    let args = vec![
        "check".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--mode".to_string(),
        "draft".to_string(),
        "--format".to_string(),
        "repo-exposure-json".to_string(),
    ];
    let result = run_command_in_dir(binary, &args, root, "authentic repo-exposure producer")?;
    if !result.success {
        return Err(format!(
            "producer check failed: {}",
            command_details(&result).join("; ")
        ));
    }
    let value: Value = serde_json::from_str(&result.stdout)
        .map_err(|err| format!("producer emitted malformed repo-exposure JSON: {err}"))?;
    fs::write(root.join(artifact_name), &result.stdout)
        .map_err(|err| format!("write producer artifact {artifact_name} failed: {err}"))?;
    Ok(value)
}

fn run_analysis_outcome_check(binary: &Path, root: &Path) -> Result<(), String> {
    let args = vec![
        "check".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--mode".to_string(),
        "draft".to_string(),
        "--format".to_string(),
        "json".to_string(),
    ];
    let result = run_command_in_dir(binary, &args, root, "authentic analysis outcome producer")?;
    if !result.success {
        return Err(format!(
            "analysis outcome producer failed: {}",
            command_details(&result).join("; ")
        ));
    }
    let _: Value = serde_json::from_str(&result.stdout)
        .map_err(|err| format!("analysis outcome producer emitted malformed JSON: {err}"))?;
    fs::write(root.join("analysis-outcome.json"), result.stdout)
        .map_err(|err| format!("write analysis outcome artifact failed: {err}"))
}

fn validate_authentic_artifact(
    path: &Path,
    expected_head: &str,
    label: &str,
) -> Result<(), String> {
    let value = read_json_value(path)?;
    if value.get("run_status").and_then(Value::as_str) != Some("complete") {
        return Err(format!("authentic {label} artifact is not complete"));
    }
    if artifact_string(&value, &["artifact", "producer", "tool"])? != "ripr" {
        return Err(format!("authentic {label} artifact producer is not ripr"));
    }
    if artifact_string(&value, &["artifact", "repository", "head"])? != expected_head {
        return Err(format!(
            "authentic {label} artifact head does not match fixture commit"
        ));
    }
    if value
        .get("seams")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
    {
        return Err(format!(
            "authentic {label} artifact contains no analyzed seams"
        ));
    }
    Ok(())
}

fn artifact_string<'a>(value: &'a Value, path: &[&str]) -> Result<&'a str, String> {
    let mut current = value;
    for key in path {
        current = current
            .get(*key)
            .ok_or_else(|| format!("artifact is missing {}", path.join(".")))?;
    }
    current
        .as_str()
        .ok_or_else(|| format!("artifact {} is not a string", path.join(".")))
}

fn checkout_fixture_commit(root: &Path, commit: &str) -> Result<(), String> {
    let result = run_fixture_git_command(
        root,
        &["checkout", "--quiet", "--detach", commit],
        "checkout fixture commit",
    )?;
    if !result.success {
        return Err(format!(
            "checkout fixture commit failed: {}",
            command_details(&result).join("; ")
        ));
    }
    Ok(())
}

fn fixture_head(root: &Path) -> Result<String, String> {
    let result = run_fixture_git_command(root, &["rev-parse", "HEAD"], "read fixture HEAD")?;
    if !result.success {
        return Err(format!(
            "read fixture HEAD failed: {}",
            command_details(&result).join("; ")
        ));
    }
    let head = result.stdout.trim();
    if head.len() != 40 || !head.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("fixture HEAD is not a full commit SHA: {head}"));
    }
    Ok(head.to_string())
}

fn run_fixture_git_command(
    root: &Path,
    args: &[&str],
    operation: &str,
) -> Result<CommandResult, String> {
    let args = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    let result = run_command_in_dir("git", &args, root, operation)?;
    if !result.success {
        return Err(format!(
            "{operation} failed: {}",
            command_details(&result).join("; ")
        ));
    }
    Ok(result)
}

fn run_command_in_dir(
    program: impl AsRef<Path>,
    args: &[String],
    cwd: &Path,
    operation: &str,
) -> Result<CommandResult, String> {
    let program = program.as_ref().to_string_lossy().into_owned();
    let output = crate::run::capture_output_in_dir(&program, args, cwd, operation)?;
    Ok(CommandResult {
        status: output.status.code(),
        success: output.status.success(),
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn agent_receipt_fixture_check(binary: &Path) -> ReleaseReadinessCheck {
    let command = format!(
        "{} agent receipt (executed inside the authentic external fixture)",
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
            vec![
                AGENT_ANALYSIS_OUTCOME_OUT.to_string(),
                AGENT_VERIFY_OUT.to_string(),
                AGENT_RECEIPT_OUT.to_string(),
            ],
            vec![format!("missing prerequisite: {AGENT_VERIFY_OUT}")],
        );
    }
    if !Path::new(AGENT_ANALYSIS_OUTCOME_OUT).exists() {
        return readiness_check(
            "agent-receipt-boundary-fixture",
            "fail",
            true,
            &command,
            "canonical analysis-outcome artifact is missing",
            vec![
                AGENT_ANALYSIS_OUTCOME_OUT.to_string(),
                AGENT_VERIFY_OUT.to_string(),
                AGENT_RECEIPT_OUT.to_string(),
            ],
            vec![format!(
                "missing prerequisite: {AGENT_ANALYSIS_OUTCOME_OUT}"
            )],
        );
    }
    match read_json_value(Path::new(AGENT_RECEIPT_OUT)) {
        Ok(value)
            if value.get("analysis_outcome_status").and_then(Value::as_str) == Some("complete") =>
        {
            readiness_check(
                "agent-receipt-boundary-fixture",
                "pass",
                true,
                &command,
                "authentic fixture receipt was produced by the installed RIPR binary",
                vec![
                    AGENT_ANALYSIS_OUTCOME_OUT.to_string(),
                    AGENT_VERIFY_OUT.to_string(),
                    AGENT_RECEIPT_OUT.to_string(),
                ],
                Vec::new(),
            )
        }
        Ok(value) => readiness_check(
            "agent-receipt-boundary-fixture",
            "fail",
            true,
            &command,
            "authentic fixture receipt does not retain complete analysis-outcome evidence",
            vec![
                AGENT_ANALYSIS_OUTCOME_OUT.to_string(),
                AGENT_VERIFY_OUT.to_string(),
                AGENT_RECEIPT_OUT.to_string(),
            ],
            vec![format!(
                "analysis_outcome_status: {}",
                value
                    .get("analysis_outcome_status")
                    .and_then(Value::as_str)
                    .unwrap_or("missing")
            )],
        ),
        Err(err) => readiness_check(
            "agent-receipt-boundary-fixture",
            "fail",
            true,
            &command,
            "authentic fixture receipt is missing or malformed",
            vec![
                AGENT_ANALYSIS_OUTCOME_OUT.to_string(),
                AGENT_VERIFY_OUT.to_string(),
                AGENT_RECEIPT_OUT.to_string(),
            ],
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

/// One editor manifest version, named so a mismatch says which file is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EditorVersion {
    /// Repo-relative path, plus the JSON pointer when a file carries the version
    /// in more than one place.
    source: String,
    value: Option<String>,
}

/// Fail-closed guard against the VS Code extension version silently drifting
/// from the version actually being released. When `editors/vscode/package.json`
/// lags, `vsce` embeds the stale version into the VSIX and the marketplace
/// publish fails with "vX already exists" — the failure mode that left the
/// extension two releases behind through 0.8.0/0.9.0 (#1283). Any read failure
/// or mismatch is a fail; only exact agreement passes.
///
/// The referent is the **requested** release version, not the crate version.
/// Comparing the extension to the crate made the gate vacuous on exactly the run
/// that matters: with the workspace at 0.10.0 and the extension at 0.10.0,
/// `release-readiness --version 0.11.0` reported "matches the crate version"
/// while both lagged the release being prepared — and because a
/// requested/crate mismatch downgrades `package-list` and `publish-dry-run` to
/// `not_run` (non-required), `release_readiness_status` returned `warn` and the
/// whole command exited 0. `docs/RELEASE.md` lists "the version in the root
/// `Cargo.toml` is correct" as a *precondition* of this gate, so a disagreement
/// here is a real not-ready state rather than a legitimate pre-bump one.
///
/// `package-lock.json` is checked too, in both places npm keeps the version.
/// `docs/RELEASE.md` states it as a precondition and `publish-extension.yml`
/// runs `npm ci`, which hard-fails when the lock disagrees with `package.json`.
fn extension_version_match_check(
    version: &str,
    crate_version: Option<&str>,
) -> ReleaseReadinessCheck {
    let package_json = Path::new("editors/vscode/package.json");
    let lock_json = Path::new("editors/vscode/package-lock.json");
    let editors = vec![
        EditorVersion {
            source: "editors/vscode/package.json version".to_string(),
            value: json_string_at(package_json, &["version"]),
        },
        EditorVersion {
            source: "editors/vscode/package-lock.json version".to_string(),
            value: json_string_at(lock_json, &["version"]),
        },
        EditorVersion {
            source: "editors/vscode/package-lock.json packages.\"\".version".to_string(),
            value: json_string_at(lock_json, &["packages", "", "version"]),
        },
    ];
    extension_version_check_from(version, crate_version, &editors)
}

/// Read a string at a JSON pointer path, or `None` if any step is absent.
fn json_string_at(path: &Path, pointer: &[&str]) -> Option<String> {
    let mut value = read_json_value(path).ok()?;
    for key in pointer {
        value = value.get(key)?.clone();
    }
    value.as_str().map(str::to_string)
}

/// Pure comparison split out from the file reads so it is testable without a
/// working-directory dependency.
fn extension_version_check_from(
    version: &str,
    crate_version: Option<&str>,
    editors: &[EditorVersion],
) -> ReleaseReadinessCheck {
    let command = "compare the requested release version to the workspace package version (Cargo.toml) and the editor manifests";
    let sources = vec![
        "Cargo.toml".to_string(),
        "editors/vscode/package.json".to_string(),
        "editors/vscode/package-lock.json".to_string(),
    ];

    let Some(krate) = crate_version else {
        return readiness_check(
            "extension-version-match",
            "fail",
            true,
            command,
            "could not read the workspace package version",
            Vec::new(),
            vec![
                "workspace package version unreadable via crates/ripr/Cargo.toml -> Cargo.toml [workspace.package]"
                    .to_string(),
            ],
        );
    };

    let mut problems = Vec::new();
    if krate != version {
        problems.push(format!(
            "workspace package version {krate} != requested release version {version} (bump [workspace.package] version in Cargo.toml)"
        ));
    }
    for editor in editors {
        match &editor.value {
            None => problems.push(format!("{} is unreadable", editor.source)),
            Some(found) if found != version => problems.push(format!(
                "{} is {found} != requested release version {version}",
                editor.source
            )),
            Some(_) => {}
        }
    }

    if problems.is_empty() {
        readiness_check(
            "extension-version-match",
            "pass",
            true,
            command,
            "the workspace package version and both editor manifests match the requested release version",
            sources,
            Vec::new(),
        )
    } else {
        readiness_check(
            "extension-version-match",
            "fail",
            true,
            command,
            "the release version is not consistently declared; the marketplace publish would fail or republish a stale version",
            Vec::new(),
            problems,
        )
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
        EditorVersion, FIRST_SCREEN_NEEDLES, PackageVersion, RELEASE_LOOP_NEEDLES,
        ReleaseReadinessCheck, ReleaseReadinessReport, extension_version_check_from,
        extract_packaged_crate, missing_required_needles, package_version,
        parse_release_readiness_args, read_crate_version, readiness_check, release_readiness_json,
        release_readiness_markdown, release_readiness_status, validate_binary_identity,
        validate_doctor_result, validate_installed_version, validate_package_entry,
        vsix_start_current_repair_command_present,
    };
    use serde_json::Value;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn packaged_crate_extraction_rejects_entry_outside_package_root() -> Result<(), String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("clock error: {err}"))?
            .as_nanos();
        let archive = std::env::temp_dir().join(format!("ripr-package-{stamp}.crate"));
        let destination = std::env::temp_dir().join(format!("ripr-package-extract-{stamp}"));
        let file = fs::File::create(&archive)
            .map_err(|err| format!("create test archive failed: {err}"))?;
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut writer = tar::Builder::new(encoder);
        let payload = b"must not extract";
        let mut header = tar::Header::new_gnu();
        header.set_size(payload.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        writer
            .append_data(&mut header, "other/escape", &payload[..])
            .map_err(|err| format!("write outside-root entry failed: {err}"))?;
        let encoder = writer
            .into_inner()
            .map_err(|err| format!("finish test archive failed: {err}"))?;
        encoder
            .finish()
            .map_err(|err| format!("finish compressed test archive failed: {err}"))?;

        let result = extract_packaged_crate(&archive, &destination, "0.1.0");
        let _ = fs::remove_file(&archive);
        let _ = fs::remove_dir_all(&destination);
        if result.is_ok() {
            return Err(
                "package extraction accepted an entry outside the package root".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn packaged_crate_extraction_rejects_symlink_and_hardlink_entries() -> Result<(), String> {
        for (kind, entry_type) in [
            ("symlink", tar::EntryType::symlink()),
            ("hardlink", tar::EntryType::hard_link()),
        ] {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|err| format!("clock error: {err}"))?
                .as_nanos();
            let archive = std::env::temp_dir().join(format!("ripr-package-{kind}-{stamp}.crate"));
            let destination =
                std::env::temp_dir().join(format!("ripr-package-extract-{kind}-{stamp}"));
            let file = fs::File::create(&archive)
                .map_err(|err| format!("create {kind} test archive failed: {err}"))?;
            let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            let mut writer = tar::Builder::new(encoder);
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(entry_type);
            header.set_mode(0o644);
            writer
                .append_link(&mut header, format!("ripr-0.1.0/{kind}"), "target")
                .map_err(|err| format!("write {kind} entry failed: {err}"))?;
            let encoder = writer
                .into_inner()
                .map_err(|err| format!("finish {kind} archive failed: {err}"))?;
            encoder
                .finish()
                .map_err(|err| format!("finish compressed {kind} archive failed: {err}"))?;

            let result = extract_packaged_crate(&archive, &destination, "0.1.0");
            let _ = fs::remove_file(&archive);
            let _ = fs::remove_dir_all(&destination);
            if result.is_ok() {
                return Err(format!("package extraction accepted a {kind} entry"));
            }
        }
        Ok(())
    }

    #[test]
    fn packaged_crate_entry_validation_rejects_traversal_components() -> Result<(), String> {
        let expected_root = Path::new("ripr-0.1.0");
        let regular = tar::EntryType::Regular;
        for path in [Path::new("ripr-0.1.0/foo/../escape"), Path::new("/escape")] {
            if validate_package_entry(path, regular, expected_root).is_ok() {
                return Err(format!("package entry validation accepted {path:?}"));
            }
        }
        Ok(())
    }

    #[test]
    fn packaged_install_validates_identity_version_and_doctor_status() -> Result<(), String> {
        if validate_binary_identity("same", "same").is_ok() {
            return Err("matching workspace and installed digests were accepted".to_string());
        }
        validate_binary_identity("workspace", "installed")?;

        if validate_installed_version(true, "ripr 0.9.0\n", "0.10.0").is_ok() {
            return Err("wrong installed version was accepted".to_string());
        }
        validate_installed_version(true, "ripr 0.10.0\n", "0.10.0")?;
        if validate_installed_version(false, "ripr 0.10.0\n", "0.10.0").is_ok() {
            return Err("failed version command was accepted".to_string());
        }

        let pass = serde_json::json!({"status": "pass"});
        validate_doctor_result(true, &pass)?;
        if validate_doctor_result(false, &pass).is_ok() {
            return Err("failed doctor command was accepted".to_string());
        }
        let warn = serde_json::json!({"status": "warn"});
        if validate_doctor_result(true, &warn).is_ok() {
            return Err("non-pass doctor status was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn authentic_journey_binary_resolves_to_absolute_path() -> Result<(), String> {
        let resolved = super::absolute_installed_binary(Path::new("Cargo.toml"))?;
        if !resolved.is_absolute() {
            return Err(format!(
                "resolved binary path {} is not absolute",
                resolved.display()
            ));
        }
        if !resolved.ends_with("Cargo.toml") {
            return Err(format!(
                "resolved binary path {} lost its file name",
                resolved.display()
            ));
        }
        Ok(())
    }

    #[test]
    fn release_fixture_root_is_canonical_and_external() -> Result<(), String> {
        let root = super::release_temp_root()?;
        let current = fs::canonicalize(
            std::env::current_dir()
                .map_err(|err| format!("read current directory failed: {err}"))?,
        )
        .map_err(|err| format!("canonicalize current directory failed: {err}"))?;
        if root == current || root.starts_with(&current) {
            return Err(format!(
                "release fixture root {} is not external to {}",
                root.display(),
                current.display()
            ));
        }
        Ok(())
    }

    /// Build the three editor manifest readings the real check collects.
    fn editors(package_json: Option<&str>, lock: Option<&str>) -> Vec<EditorVersion> {
        vec![
            EditorVersion {
                source: "editors/vscode/package.json version".to_string(),
                value: package_json.map(str::to_string),
            },
            EditorVersion {
                source: "editors/vscode/package-lock.json version".to_string(),
                value: lock.map(str::to_string),
            },
            EditorVersion {
                source: "editors/vscode/package-lock.json packages.\"\".version".to_string(),
                value: lock.map(str::to_string),
            },
        ]
    }

    #[test]
    fn extension_version_check_fails_closed_on_drift() -> Result<(), String> {
        let matched = extension_version_check_from(
            "0.10.0",
            Some("0.10.0"),
            &editors(Some("0.10.0"), Some("0.10.0")),
        );
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
            let check =
                extension_version_check_from("0.10.0", krate, &editors(ext, Some("0.10.0")));
            if check.status != "fail" || !check.required {
                return Err(format!(
                    "expected required fail for {label}, got {}/{}",
                    check.status, check.required
                ));
            }
        }
        Ok(())
    }

    /// The defect this check was rewritten to close: everything agreed with the
    /// crate, nothing agreed with the version being released, and the gate said
    /// pass. Comparing against `crate_version` alone made the lens vacuous on the
    /// one run that matters — `--version <next>` before the bump landed.
    #[test]
    fn extension_version_check_fails_when_everything_lags_the_requested_release()
    -> Result<(), String> {
        let check = extension_version_check_from(
            "0.11.0",
            Some("0.10.0"),
            &editors(Some("0.10.0"), Some("0.10.0")),
        );
        if check.status != "fail" || !check.required {
            return Err(format!(
                "a release requested at 0.11.0 with everything at 0.10.0 must fail, got {}/{}",
                check.status, check.required
            ));
        }
        // The report has to name the root cause, not just the symptom: the
        // workspace version is the thing a human has to bump first.
        if !check
            .details
            .iter()
            .any(|detail| detail.contains("[workspace.package]"))
        {
            return Err(format!(
                "expected the workspace bump to be named in details, got {:?}",
                check.details
            ));
        }
        Ok(())
    }

    /// `npm ci` hard-fails when the lock disagrees with `package.json`, and
    /// `publish-extension.yml` runs it — so a lock-only lag breaks the publish
    /// even though `package.json` looks correct. Both places npm keeps the
    /// version are checked.
    #[test]
    fn extension_version_check_catches_a_lock_only_lag() -> Result<(), String> {
        let check = extension_version_check_from(
            "0.11.0",
            Some("0.11.0"),
            &editors(Some("0.11.0"), Some("0.10.0")),
        );
        if check.status != "fail" || !check.required {
            return Err(format!(
                "a stale package-lock.json must fail, got {}/{}",
                check.status, check.required
            ));
        }
        if !check
            .details
            .iter()
            .any(|detail| detail.contains("package-lock.json"))
        {
            return Err(format!(
                "expected package-lock.json to be named, got {:?}",
                check.details
            ));
        }
        // `package.json` agrees with the request, so it must not be blamed.
        if check
            .details
            .iter()
            .any(|detail| detail.starts_with("editors/vscode/package.json"))
        {
            return Err(format!(
                "package.json matches and must not be reported, got {:?}",
                check.details
            ));
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

        // The lock is a release precondition in its own right: `npm ci` in
        // publish-extension.yml hard-fails when it disagrees with package.json,
        // so a lock-only lag breaks the publish while package.json looks fine.
        let lock_json = root.join("editors/vscode/package-lock.json");
        let lock_text = fs::read_to_string(&lock_json)
            .map_err(|err| format!("failed to read {}: {err}", lock_json.display()))?;
        let lock: Value = serde_json::from_str(&lock_text)
            .map_err(|err| format!("failed to parse {}: {err}", lock_json.display()))?;
        for pointer in [vec!["version"], vec!["packages", "", "version"]] {
            let mut found = &lock;
            for key in &pointer {
                found = found
                    .get(key)
                    .ok_or_else(|| format!("package-lock.json has no {}", pointer.join(".")))?;
            }
            let value = found.as_str().ok_or_else(|| {
                format!("package-lock.json {} is not a string", pointer.join("."))
            })?;
            if value != crate_version {
                return Err(format!(
                    "package-lock.json {} is {value} != workspace version {crate_version}",
                    pointer.join(".")
                ));
            }
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
        // The two needle sets are checked against different surfaces, so a
        // needle in the wrong set silently weakens the gate. `first-pr` is the
        // deliberate overlap: it is both a first action and a release-loop step.
        for needle in FIRST_SCREEN_NEEDLES {
            if RELEASE_LOOP_NEEDLES.contains(needle) && *needle != "ripr first-pr" {
                return Err(format!(
                    "{needle} is in both needle sets; decide which surface owns it"
                ));
            }
        }
        let missing_first_pr = missing_required_needles(help, &["ripr first-pr", "--receipts-dir"]);
        if missing_first_pr != ["--receipts-dir".to_string()] {
            return Err(format!("unexpected missing needles: {missing_first_pr:?}"));
        }
        Ok(())
    }

    /// Bind the needle sets to the help text this repository actually ships.
    ///
    /// `installed-command-surface` only runs in the on-demand release lane
    /// (`policy/ci-lane-whitelist.toml`, `posture = "on_demand_release"`), so
    /// nothing at PR time notices when a help change stops satisfying it. That is
    /// exactly how #1613's help rework broke this gate: moving the catalog behind
    /// `help --all` emptied five of six needles out of `--help`, and every PR
    /// check stayed green. This test reads the shipped constants directly, so the
    /// break surfaces in `cargo test` instead of mid-release.
    #[test]
    fn repository_help_surfaces_satisfy_command_surface_needles() -> Result<(), String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| "xtask manifest has no parent directory".to_string())?
            .to_path_buf();
        let path = root.join("crates/ripr/src/cli/help/overview.rs");
        let text = crate::read_text_lossy(&path)
            .map_err(|err| format!("read {}: {err}", path.display()))?;

        let first_screen = raw_string_const(&text, "HELP")
            .ok_or_else(|| format!("could not locate HELP in {}", path.display()))?;
        let full_reference = raw_string_const(&text, "HELP_ALL")
            .ok_or_else(|| format!("could not locate HELP_ALL in {}", path.display()))?;

        let missing_first = missing_required_needles(first_screen, FIRST_SCREEN_NEEDLES);
        if !missing_first.is_empty() {
            return Err(format!(
                "the `ripr --help` first screen no longer offers {missing_first:?}; \
                 either restore them or move them to RELEASE_LOOP_NEEDLES"
            ));
        }

        let missing_all = missing_required_needles(full_reference, RELEASE_LOOP_NEEDLES);
        if !missing_all.is_empty() {
            return Err(format!(
                "`ripr help --all` no longer documents {missing_all:?}; \
                 the release gate would fail on this help text"
            ));
        }
        Ok(())
    }

    /// Extract the body of `const <name>: &str = r#"..."#;` from Rust source.
    ///
    /// Matches on `const <name>:` so `HELP` does not also match `HELP_ALL`.
    fn raw_string_const<'a>(text: &'a str, name: &str) -> Option<&'a str> {
        let anchor = format!("const {name}:");
        let after_name = text.find(&anchor)? + anchor.len();
        let rest = text.get(after_name..)?;
        let open = rest.find("r#\"")? + "r#\"".len();
        let body = rest.get(open..)?;
        let close = body.find("\"#")?;
        body.get(..close)
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
