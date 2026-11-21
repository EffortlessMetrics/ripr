use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::RustJudgedPanelManifest;
use super::subject::{self, ReplaySubject, RepositoryState};
use crate::run::{
    TimedBytesOutput, capture_bytes_in_dir_with_timeout, capture_process_output, tool_build_timeout,
};

const DEFAULT_OUTPUT: &str = "target/ripr/rust-judged-panel";
const RUN_TIMEOUT: Duration = Duration::from_mins(2);
const BUILD_ENV_REMOVE: [&str; 10] = [
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET",
    "CARGO_ENCODED_RUSTFLAGS",
    "RUSTFLAGS",
    "RUSTDOCFLAGS",
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RIPR_BIN",
    "RIPR_CACHE_DIR",
];
const RUN_ENV_REMOVE: [&str; 5] = [
    "RIPR_BIN",
    "RIPR_CONFIG",
    "RIPR_CACHE_DIR",
    "HTTP_PROXY",
    "HTTPS_PROXY",
];
static RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceIdentity {
    head: String,
    tree: String,
    cargo_lock_sha256: String,
    cargo_toml_sha256: String,
    dirty: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BuildIdentity {
    command: Vec<String>,
    package: String,
    profile: String,
    features: Vec<String>,
    locked: bool,
    offline: bool,
    cargo_version: String,
    rustc_verbose_version: String,
    host_target: String,
    cargo_home: Option<String>,
    executed_binary_path: String,
    retained_binary_path: String,
    binary_sha256: String,
    binary_bytes: u64,
    binary_version: String,
    build_stdout_sha256: String,
    build_stderr_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExecutionPlan {
    argv: Vec<String>,
    root: String,
    base: String,
    head: String,
    tree: String,
    mode: String,
    format: String,
    config_path: String,
    config_sha256: String,
    diff_path: String,
    diff_sha256: String,
    executed_diff_identity: String,
    subject_inputs: Vec<InputDigest>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InputDigest {
    role: String,
    source_path: String,
    repository_path: String,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawEvidence {
    stdout_path: String,
    stdout_sha256: String,
    stdout_bytes: u64,
    stderr_path: String,
    stderr_sha256: String,
    stderr_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CaseReceipt {
    schema_version: String,
    kind: String,
    case_id: String,
    subject_id: String,
    expected_direction: String,
    source_head: String,
    source_tree: String,
    binary_sha256: String,
    repository_base: String,
    repository_head: String,
    repository_tree: String,
    plan: ExecutionPlan,
    disposition: String,
    exit_code: Option<i32>,
    timed_out: bool,
    duration_ms: u128,
    analyzer_input_identity: Option<String>,
    raw: RawEvidence,
    error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct IndexEntry {
    case_id: String,
    expected_direction: String,
    receipt_path: String,
    receipt_sha256: String,
    stdout_sha256: String,
    stderr_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct HostRunIndex {
    schema_version: String,
    kind: String,
    publication_state: String,
    run_id: String,
    source: SourceIdentity,
    build: BuildIdentity,
    cases: Vec<IndexEntry>,
    non_claims: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CurrentRun {
    schema_version: String,
    kind: String,
    run_id: String,
    index_path: String,
    index_sha256: String,
}

#[derive(Clone, Debug)]
pub(super) struct ValidatedHostRun {
    pub(super) current_ref: String,
    pub(super) current_sha256: String,
    pub(super) index_ref: String,
    pub(super) index_sha256: String,
    pub(super) run_id: String,
    pub(super) source_head: String,
    pub(super) source_tree: String,
    pub(super) cargo_lock_sha256: String,
    pub(super) cargo_toml_sha256: String,
    pub(super) profile: String,
    pub(super) features: Vec<String>,
    pub(super) host_target: String,
    pub(super) binary_sha256: String,
    pub(super) binary_version: String,
    pub(super) cases: Vec<ValidatedHostCase>,
}

#[derive(Clone, Debug)]
pub(super) struct ValidatedHostCase {
    pub(super) case_id: String,
    pub(super) subject_id: String,
    pub(super) expected_direction: String,
    pub(super) repository_base: String,
    pub(super) repository_head: String,
    pub(super) repository_tree: String,
    pub(super) argv: Vec<String>,
    pub(super) mode: String,
    pub(super) format: String,
    pub(super) config_path: String,
    pub(super) config_sha256: String,
    pub(super) diff_path: String,
    pub(super) diff_sha256: String,
    pub(super) executed_diff_identity: String,
    pub(super) subject_inputs: Vec<ValidatedInputDigest>,
    pub(super) disposition: String,
    pub(super) analyzer_input_identity: String,
    pub(super) receipt_ref: String,
    pub(super) receipt_sha256: String,
    pub(super) stdout_ref: String,
    pub(super) stdout_sha256: String,
    pub(super) stderr_ref: String,
    pub(super) stderr_sha256: String,
    pub(super) stdout: Vec<u8>,
    pub(super) materialized_root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct ValidatedInputDigest {
    pub(super) role: String,
    pub(super) source_path: String,
    pub(super) repository_path: String,
    pub(super) sha256: String,
}

struct RunLock(PathBuf);

impl Drop for RunLock {
    fn drop(&mut self) {
        let _result = fs::remove_file(&self.0);
    }
}

pub(super) fn run(
    root: &Path,
    manifest: &RustJudgedPanelManifest,
    output: Option<&str>,
) -> Result<(), String> {
    let output_text = output.unwrap_or(DEFAULT_OUTPUT);
    let output_root = confined_output(root, output_text)?;
    fs::create_dir_all(&output_root).map_err(|error| {
        format!(
            "create host-run output `{}`: {error}",
            output_root.display()
        )
    })?;
    let _lock = acquire_lock(&output_root)?;

    let repository = subject::repository_state(root)?;
    if repository.dirty {
        return Err(
            "authoritative Rust judged-panel replay requires a clean build-source worktree"
                .to_string(),
        );
    }
    let source = source_identity(root, &repository)?;
    let run_id = run_id(&source);
    let attempt = output_root.join(format!(".staging-{run_id}"));
    if attempt.exists() {
        return Err(format!(
            "host-run staging path already exists: `{}`; remove only after confirming no live owner",
            attempt.display()
        ));
    }
    fs::create_dir_all(&attempt)
        .map_err(|error| format!("create host-run staging `{}`: {error}", attempt.display()))?;

    let subjects = subject::materialize_for_replay(root, &attempt.join("subjects"), manifest)?;
    let build = build_fresh_binary(root, &attempt)?;
    let binary = attempt.join(&build.retained_binary_path);
    validate_binary_unchanged(&binary, &build.binary_sha256)?;

    let mut receipts = Vec::new();
    for selected in &subjects {
        let receipt = execute_case(root, &attempt, selected, &source, &build, &binary)?;
        validate_binary_unchanged(&binary, &build.binary_sha256)?;
        let accepted = matches!(receipt.disposition.as_str(), "complete" | "typed_limited");
        receipts.push(receipt);
        if !accepted {
            return Err(format!(
                "Rust judged-panel case `{}` ended as `{}`; non-authoritative raw receipt retained at `{}`",
                selected.case_id,
                receipts
                    .last()
                    .map_or("unknown", |receipt| receipt.disposition.as_str()),
                attempt.display()
            ));
        }
    }

    let after = subject::repository_state(root)?;
    validate_source_unchanged(root, &source, &after)?;

    let final_run =
        publish_complete_generation(&output_root, &attempt, &run_id, source, build, &receipts)?;
    let current = read_strict_json::<CurrentRun>(&output_root.join("current.json"), "current run")?;
    validate_current(&output_root, &current)?;
    println!(
        "Rust judged-panel host run complete: cases={} run={} current={}",
        receipts.len(),
        final_run.display(),
        output_root.join("current.json").display()
    );
    Ok(())
}

#[cfg(test)]
pub(super) fn write_test_host_run(
    root: &Path,
    subjects: &[super::subject::PacketSubject],
) -> Result<String, String> {
    let output_root = root.join("target/ripr/rust-judged-panel");
    let attempt = output_root.join(".staging-run-fixture");
    fs::create_dir_all(&attempt).map_err(|error| error.to_string())?;

    let source = SourceIdentity {
        head: "1".repeat(40),
        tree: "2".repeat(40),
        cargo_lock_sha256: format!("sha256:{}", "3".repeat(64)),
        cargo_toml_sha256: format!("sha256:{}", "4".repeat(64)),
        dirty: false,
    };
    let binary = b"fixture ripr binary\n";
    let build_stdout = b"fixture build stdout\n";
    let build_stderr = b"fixture build stderr\n";
    fs::create_dir_all(attempt.join("build-target/debug")).map_err(|error| error.to_string())?;
    fs::write(attempt.join("build-target/debug/ripr"), binary)
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(attempt.join("build")).map_err(|error| error.to_string())?;
    fs::write(attempt.join("build/stdout.bin"), build_stdout).map_err(|error| error.to_string())?;
    fs::write(attempt.join("build/stderr.bin"), build_stderr).map_err(|error| error.to_string())?;
    let build = BuildIdentity {
        command: vec![
            "cargo".to_string(),
            "build".to_string(),
            "-p".to_string(),
            "ripr".to_string(),
            "--locked".to_string(),
            "--offline".to_string(),
            "--target-dir".to_string(),
            "build-target".to_string(),
        ],
        package: "ripr".to_string(),
        profile: "dev".to_string(),
        features: vec!["default".to_string()],
        locked: true,
        offline: true,
        cargo_version: "cargo fixture".to_string(),
        rustc_verbose_version: "rustc fixture".to_string(),
        host_target: "fixture-host".to_string(),
        cargo_home: None,
        executed_binary_path: "build-target/debug/ripr".to_string(),
        retained_binary_path: "build-target/debug/ripr".to_string(),
        binary_sha256: sha256_bytes(binary),
        binary_bytes: binary.len() as u64,
        binary_version: "ripr 0.12.0-fixture".to_string(),
        build_stdout_sha256: sha256_bytes(build_stdout),
        build_stderr_sha256: sha256_bytes(build_stderr),
    };
    let run_id = "run-fixture";
    let mut receipts = Vec::with_capacity(subjects.len());
    for subject in subjects {
        let case_root = attempt.join("subjects").join(&subject.case_id);
        fs::create_dir_all(case_root.join("src")).map_err(|error| error.to_string())?;
        fs::write(
            case_root.join("src/lib.rs"),
            format!(
                "\nfn {}() {{ {} ; }}\n",
                subject.owner, subject.changed_behavior
            ),
        )
        .map_err(|error| error.to_string())?;

        let case_dir = attempt.join("cases").join(&subject.case_id);
        fs::create_dir_all(&case_dir).map_err(|error| error.to_string())?;
        let stdout = serde_json::json!({
            "root": format!("target/ripr/rust-judged-panel/.staging-{run_id}/subjects/{}", subject.case_id),
            "analysis_outcome": {"analysis_complete": true, "outcome": {"kind": "complete_with_findings", "limitations": []}},
            "findings": [{
                "id": format!("fixture-{}", subject.case_id),
                "classification": subject.expected_classification,
                "probe": {
                    "family": if subject.behavior_family == "predicate_boundary" { "predicate" } else { "return_value" },
                    "file": format!("target/ripr/rust-judged-panel/.staging-{run_id}/subjects/{}/{}", subject.case_id, subject.anchor_file),
                    "line": subject.anchor_line,
                    "expression": subject.changed_behavior,
                },
                "missing": subject.expected_missing,
                "recommended_next_step": subject.expected_recommendation,
                "static_limit_kind": subject.expected_static_limit_kind,
            }]
        });
        let stdout = serde_json::to_vec(&stdout).map_err(|error| error.to_string())?;
        let stderr = b"fixture stderr\n";
        fs::write(case_dir.join("stdout.bin"), &stdout).map_err(|error| error.to_string())?;
        fs::write(case_dir.join("stderr.bin"), stderr).map_err(|error| error.to_string())?;
        let subject_inputs = [
            ("cargo_toml", &subject.cargo_toml),
            ("cargo_lock", &subject.cargo_lock),
            ("config", &subject.config),
            ("source_before", &subject.source_before),
            ("source_after", &subject.source_after),
            ("diff", &subject.diff),
        ]
        .into_iter()
        .map(|(role, file)| InputDigest {
            role: role.to_string(),
            source_path: file.source_path.clone(),
            repository_path: file.repository_path.clone(),
            sha256: file.sha256.clone(),
        })
        .chain(subject.tests.iter().map(|file| InputDigest {
            role: "test".to_string(),
            source_path: file.source_path.clone(),
            repository_path: file.repository_path.clone(),
            sha256: file.sha256.clone(),
        }))
        .collect::<Vec<_>>();
        let executed_diff_identity =
            super::subject::executed_diff_identity(&case_root, &subject.expected_base)?;
        let receipt = CaseReceipt {
            schema_version: "0.1".to_string(),
            kind: "rust_judged_panel_host_run_receipt".to_string(),
            case_id: subject.case_id.clone(),
            subject_id: subject.subject_id.clone(),
            expected_direction: subject.expected_direction.clone(),
            source_head: source.head.clone(),
            source_tree: source.tree.clone(),
            binary_sha256: build.binary_sha256.clone(),
            repository_base: subject.expected_base.clone(),
            repository_head: subject.expected_head.clone(),
            repository_tree: subject.expected_tree.clone(),
            plan: ExecutionPlan {
                argv: vec![
                    "check".to_string(),
                    "--root".to_string(),
                    "<materialized-subject>".to_string(),
                    "--base".to_string(),
                    subject.expected_base.clone(),
                    "--mode".to_string(),
                    "draft".to_string(),
                    "--json".to_string(),
                ],
                root: "<materialized-subject>".to_string(),
                base: subject.expected_base.clone(),
                head: subject.expected_head.clone(),
                tree: subject.expected_tree.clone(),
                mode: "draft".to_string(),
                format: "json".to_string(),
                config_path: subject.config.repository_path.clone(),
                config_sha256: subject.config.sha256.clone(),
                diff_path: subject.diff.source_path.clone(),
                diff_sha256: subject.diff.sha256.clone(),
                executed_diff_identity: executed_diff_identity.clone(),
                subject_inputs,
            },
            disposition: "complete".to_string(),
            exit_code: Some(0),
            timed_out: false,
            duration_ms: 1,
            analyzer_input_identity: Some(executed_diff_identity),
            raw: RawEvidence {
                stdout_path: format!("cases/{}/stdout.bin", subject.case_id),
                stdout_sha256: sha256_bytes(&stdout),
                stdout_bytes: stdout.len() as u64,
                stderr_path: format!("cases/{}/stderr.bin", subject.case_id),
                stderr_sha256: sha256_bytes(stderr),
                stderr_bytes: stderr.len() as u64,
            },
            error: None,
        };
        atomic_write(&case_dir.join("receipt.json"), &pretty_json(&receipt)?)?;
        receipts.push(receipt);
    }
    publish_complete_generation(&output_root, &attempt, run_id, source, build, &receipts)?;
    Ok("target/ripr/rust-judged-panel/current.json".to_string())
}

fn source_identity(root: &Path, state: &RepositoryState) -> Result<SourceIdentity, String> {
    Ok(SourceIdentity {
        head: state.head.clone(),
        tree: state.tree.clone(),
        cargo_lock_sha256: sha256_file(&root.join("Cargo.lock"))?,
        cargo_toml_sha256: sha256_file(&root.join("Cargo.toml"))?,
        dirty: state.dirty,
    })
}

fn run_id(source: &SourceIdentity) -> String {
    let sequence = RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let head = source.head.get(..12).unwrap_or(source.head.as_str());
    format!("{head}-{}-{sequence}", std::process::id())
}

fn build_fresh_binary(root: &Path, attempt: &Path) -> Result<BuildIdentity, String> {
    let target = attempt.join("build-target");
    if target.exists() {
        return Err(format!(
            "fresh build target already exists: `{}`",
            target.display()
        ));
    }
    let command = vec![
        "build".to_string(),
        "-p".to_string(),
        "ripr".to_string(),
        "--locked".to_string(),
        "--offline".to_string(),
        "--target-dir".to_string(),
        target.display().to_string(),
    ];
    let output = capture_bytes_in_dir_with_timeout(
        Path::new("cargo"),
        &command,
        root,
        &[("CARGO_NET_OFFLINE", "true")],
        &BUILD_ENV_REMOVE,
        tool_build_timeout()?,
        "fresh locked offline RIPR build",
    )?;
    let build_dir = attempt.join("build");
    fs::create_dir_all(&build_dir)
        .map_err(|error| format!("create build receipt directory: {error}"))?;
    atomic_write(&build_dir.join("stdout.bin"), &output.stdout)?;
    atomic_write(&build_dir.join("stderr.bin"), &output.stderr)?;
    if output.timed_out || !output.status.is_some_and(|status| status.success()) {
        return Err(format!(
            "fresh locked offline RIPR build did not complete successfully; raw build output retained at `{}`",
            build_dir.display()
        ));
    }
    let binary = target
        .join("debug")
        .join(if cfg!(windows) { "ripr.exe" } else { "ripr" });
    if !binary.is_file() {
        return Err(format!(
            "fresh build did not produce `{}`",
            binary.display()
        ));
    }
    let binary_sha256 = sha256_file(&binary)?;
    let binary_bytes = fs::metadata(&binary)
        .map_err(|error| format!("read built binary metadata: {error}"))?
        .len();
    let binary_version = successful_text_command(&binary, &["--version"], "RIPR version")?;
    let expected_version = workspace_version(root)?;
    if !binary_version
        .split_whitespace()
        .any(|part| part == expected_version.as_str())
    {
        return Err(format!(
            "fresh RIPR version `{binary_version}` does not bind workspace version `{expected_version}`"
        ));
    }
    let cargo_version =
        successful_text_command(Path::new("cargo"), &["--version"], "Cargo version")?;
    let rustc_verbose_version =
        successful_text_command(Path::new("rustc"), &["-vV"], "rustc verbose version")?;
    let host_target = rustc_verbose_version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or_else(|| "rustc -vV did not report a host target".to_string())?
        .to_string();
    Ok(BuildIdentity {
        command: std::iter::once("cargo".to_string())
            .chain(command)
            .collect(),
        package: "ripr".to_string(),
        profile: "dev".to_string(),
        features: vec!["default".to_string()],
        locked: true,
        offline: true,
        cargo_version,
        rustc_verbose_version,
        host_target,
        cargo_home: std::env::var_os("CARGO_HOME")
            .map(|value| value.to_string_lossy().into_owned()),
        executed_binary_path: binary.display().to_string(),
        retained_binary_path: format!(
            "build-target/debug/{}",
            if cfg!(windows) { "ripr.exe" } else { "ripr" }
        ),
        binary_sha256,
        binary_bytes,
        binary_version,
        build_stdout_sha256: sha256_bytes(&output.stdout),
        build_stderr_sha256: sha256_bytes(&output.stderr),
    })
}

fn execute_case(
    root: &Path,
    attempt: &Path,
    subject: &ReplaySubject,
    source: &SourceIdentity,
    build: &BuildIdentity,
    binary: &Path,
) -> Result<CaseReceipt, String> {
    let case_dir = attempt.join("cases").join(&subject.case_id);
    fs::create_dir_all(&case_dir)
        .map_err(|error| format!("create case receipt directory: {error}"))?;
    let cache = attempt.join("cache").join(&subject.case_id);
    fs::create_dir_all(&cache).map_err(|error| format!("create isolated RIPR cache: {error}"))?;
    let argv = vec![
        "check".to_string(),
        "--root".to_string(),
        subject.root.display().to_string(),
        "--base".to_string(),
        subject.base.clone(),
        "--mode".to_string(),
        "draft".to_string(),
        "--json".to_string(),
    ];
    let executed_diff_identity = subject::executed_diff_identity(&subject.root, &subject.base)?;
    let plan = ExecutionPlan {
        argv: vec![
            "check".to_string(),
            "--root".to_string(),
            "<materialized-subject>".to_string(),
            "--base".to_string(),
            subject.base.clone(),
            "--mode".to_string(),
            "draft".to_string(),
            "--json".to_string(),
        ],
        root: "<materialized-subject>".to_string(),
        base: subject.base.clone(),
        head: subject.head.clone(),
        tree: subject.tree.clone(),
        mode: "draft".to_string(),
        format: "json".to_string(),
        config_path: subject.config.repository_path.clone(),
        config_sha256: subject.config.sha256.clone(),
        diff_path: subject.diff.source_path.clone(),
        diff_sha256: subject.diff.sha256.clone(),
        executed_diff_identity,
        subject_inputs: subject_input_digests(subject),
    };
    let cache_text = cache.display().to_string();
    let observation = capture_bytes_in_dir_with_timeout(
        binary,
        &argv,
        root,
        &[("RIPR_CACHE_DIR", cache_text.as_str())],
        &RUN_ENV_REMOVE,
        RUN_TIMEOUT,
        &format!("Rust judged-panel replay `{}`", subject.case_id),
    );
    let (output, spawn_error) = match observation {
        Ok(output) => (output, None),
        Err(error) => (
            TimedBytesOutput {
                status: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
                duration: Duration::ZERO,
                timed_out: false,
            },
            Some(error),
        ),
    };
    atomic_write(&case_dir.join("stdout.bin"), &output.stdout)?;
    atomic_write(&case_dir.join("stderr.bin"), &output.stderr)?;
    let mut output_envelope = classify_output(&output, spawn_error.as_deref());
    let observed_diff_identity = subject::executed_diff_identity(&subject.root, &subject.base)?;
    if observed_diff_identity != plan.executed_diff_identity {
        output_envelope = envelope(
            "input_drift",
            output_envelope.input_identity,
            Some(format!(
                "materialized diff identity drifted while executing `{}`",
                subject.case_id
            )),
        );
    }
    let raw = RawEvidence {
        stdout_path: format!("cases/{}/stdout.bin", subject.case_id),
        stdout_sha256: sha256_bytes(&output.stdout),
        stdout_bytes: output.stdout.len() as u64,
        stderr_path: format!("cases/{}/stderr.bin", subject.case_id),
        stderr_sha256: sha256_bytes(&output.stderr),
        stderr_bytes: output.stderr.len() as u64,
    };
    let receipt = CaseReceipt {
        schema_version: "0.1".to_string(),
        kind: "rust_judged_panel_host_run_receipt".to_string(),
        case_id: subject.case_id.clone(),
        subject_id: subject.subject_id.clone(),
        expected_direction: subject.expected_direction.clone(),
        source_head: source.head.clone(),
        source_tree: source.tree.clone(),
        binary_sha256: build.binary_sha256.clone(),
        repository_base: subject.base.clone(),
        repository_head: subject.head.clone(),
        repository_tree: subject.tree.clone(),
        plan,
        disposition: output_envelope.disposition,
        exit_code: output.status.and_then(|status| status.code()),
        timed_out: output.timed_out,
        duration_ms: output.duration.as_millis(),
        analyzer_input_identity: output_envelope.input_identity,
        raw,
        error: output_envelope.error,
    };
    atomic_write(&case_dir.join("receipt.json"), &pretty_json(&receipt)?)?;
    validate_case_receipt(attempt, &receipt, source, build)?;
    Ok(receipt)
}

fn subject_input_digests(subject: &ReplaySubject) -> Vec<InputDigest> {
    let mut inputs = vec![
        input_digest("cargo_toml", &subject.cargo_toml),
        input_digest("cargo_lock", &subject.cargo_lock),
        input_digest("config", &subject.config),
        input_digest("source_before", &subject.source_before),
        input_digest("source_after", &subject.source_after),
        input_digest("diff", &subject.diff),
    ];
    inputs.extend(subject.tests.iter().map(|test| input_digest("test", test)));
    inputs
}

fn input_digest(role: &str, file: &subject::ReplaySubjectFile) -> InputDigest {
    InputDigest {
        role: role.to_string(),
        source_path: file.source_path.clone(),
        repository_path: file.repository_path.clone(),
        sha256: file.sha256.clone(),
    }
}

struct Envelope {
    disposition: String,
    input_identity: Option<String>,
    error: Option<String>,
}

fn classify_output(output: &TimedBytesOutput, spawn_error: Option<&str>) -> Envelope {
    if let Some(error) = spawn_error {
        return envelope("spawn_failed", None, Some(error.to_string()));
    }
    if output.timed_out {
        return envelope(
            "timed_out",
            None,
            Some("configured deadline elapsed".to_string()),
        );
    }
    if !output.status.is_some_and(|status| status.success()) {
        return envelope(
            "nonzero_exit",
            None,
            Some("RIPR exited unsuccessfully".to_string()),
        );
    }
    classify_successful_stdout(&output.stdout)
}

fn classify_successful_stdout(stdout: &[u8]) -> Envelope {
    let text = match std::str::from_utf8(stdout) {
        Ok(text) => text,
        Err(error) => return envelope("malformed_output", None, Some(error.to_string())),
    };
    let value = match super::parse_json_without_duplicate_keys(text) {
        Ok(value) => value,
        Err(error) => return envelope("malformed_output", None, Some(error.to_string())),
    };
    let complete = value
        .pointer("/analysis_outcome/analysis_complete")
        .and_then(Value::as_bool);
    let input_identity = value
        .pointer("/analysis_outcome/outcome/identity/input_identity")
        .and_then(Value::as_str)
        .filter(|identity| !identity.trim().is_empty())
        .map(ToString::to_string);
    if complete != Some(true) || input_identity.is_none() {
        return envelope(
            "incomplete_output",
            input_identity,
            Some("analysis_complete=true and a non-empty input identity are required".to_string()),
        );
    }
    let limited = value
        .pointer("/analysis_outcome/outcome/limitations")
        .and_then(Value::as_array)
        .is_some_and(|limitations| !limitations.is_empty());
    envelope(
        if limited { "typed_limited" } else { "complete" },
        input_identity,
        None,
    )
}

fn envelope(disposition: &str, input_identity: Option<String>, error: Option<String>) -> Envelope {
    Envelope {
        disposition: disposition.to_string(),
        input_identity,
        error,
    }
}

fn publish_complete_generation(
    output_root: &Path,
    attempt: &Path,
    run_id: &str,
    source: SourceIdentity,
    build: BuildIdentity,
    receipts: &[CaseReceipt],
) -> Result<PathBuf, String> {
    validate_case_set(receipts)?;
    let mut cases = Vec::new();
    for receipt in receipts {
        validate_case_receipt(attempt, receipt, &source, &build)?;
        let receipt_path = format!("cases/{}/receipt.json", receipt.case_id);
        cases.push(IndexEntry {
            case_id: receipt.case_id.clone(),
            expected_direction: receipt.expected_direction.clone(),
            receipt_sha256: sha256_file(&attempt.join(&receipt_path))?,
            receipt_path,
            stdout_sha256: receipt.raw.stdout_sha256.clone(),
            stderr_sha256: receipt.raw.stderr_sha256.clone(),
        });
    }
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    let index = HostRunIndex {
        schema_version: "0.1".to_string(),
        kind: "rust_judged_panel_host_run_index".to_string(),
        publication_state: "complete".to_string(),
        run_id: run_id.to_string(),
        source,
        build,
        cases,
        non_claims: vec![
            "host-bound raw execution evidence only".to_string(),
            "no portable semantic packet or judgment".to_string(),
            "no rate, gate, badge, or support claim".to_string(),
        ],
    };
    let index_bytes = pretty_json(&index)?;
    atomic_write(&attempt.join("run-index.json"), &index_bytes)?;
    validate_generation(attempt)?;

    let runs = output_root.join("runs");
    fs::create_dir_all(&runs).map_err(|error| format!("create immutable run root: {error}"))?;
    let final_run = runs.join(run_id);
    if final_run.exists() {
        return Err(format!(
            "immutable host run already exists: `{}`",
            final_run.display()
        ));
    }
    fs::rename(attempt, &final_run).map_err(|error| {
        format!(
            "publish immutable host run `{}`: {error}",
            final_run.display()
        )
    })?;
    let current = CurrentRun {
        schema_version: "0.1".to_string(),
        kind: "rust_judged_panel_current_host_run".to_string(),
        run_id: run_id.to_string(),
        index_path: format!("runs/{run_id}/run-index.json"),
        index_sha256: sha256_file(&final_run.join("run-index.json"))?,
    };
    atomic_write(&output_root.join("current.json"), &pretty_json(&current)?)?;
    Ok(final_run)
}

fn validate_case_set(receipts: &[CaseReceipt]) -> Result<(), String> {
    let expected = BTreeSet::from([
        "rust-boundary-exact-equality-should-stay-quiet",
        "rust-boundary-missing-equality-should-gap",
        "rust-macro-wrapped-reach-should-limit",
    ]);
    let actual = receipts
        .iter()
        .map(|receipt| receipt.case_id.as_str())
        .collect::<BTreeSet<_>>();
    let directions_match = receipts.iter().all(|receipt| {
        let expected = if receipt.case_id.ends_with("should-gap") {
            "should_gap"
        } else if receipt.case_id.ends_with("should-stay-quiet") {
            "should_stay_quiet"
        } else if receipt.case_id.ends_with("should-limit") {
            "should_limit"
        } else {
            ""
        };
        receipt.expected_direction == expected
    });
    if receipts.len() == 3 && actual == expected && directions_match {
        Ok(())
    } else {
        Err(format!(
            "host run requires exactly the three canonical cases; got {} receipt(s): {}",
            receipts.len(),
            actual.into_iter().collect::<Vec<_>>().join(",")
        ))
    }
}

fn validate_case_receipt(
    run_root: &Path,
    receipt: &CaseReceipt,
    source: &SourceIdentity,
    build: &BuildIdentity,
) -> Result<(), String> {
    for (label, actual, expected) in [
        (
            "source head",
            receipt.source_head.as_str(),
            source.head.as_str(),
        ),
        (
            "source tree",
            receipt.source_tree.as_str(),
            source.tree.as_str(),
        ),
        (
            "binary digest",
            receipt.binary_sha256.as_str(),
            build.binary_sha256.as_str(),
        ),
    ] {
        if actual != expected {
            return Err(format!(
                "host receipt {label} mismatch: expected {expected}, got {actual}"
            ));
        }
    }
    for (label, path, expected, bytes) in [
        (
            "stdout",
            receipt.raw.stdout_path.as_str(),
            receipt.raw.stdout_sha256.as_str(),
            receipt.raw.stdout_bytes,
        ),
        (
            "stderr",
            receipt.raw.stderr_path.as_str(),
            receipt.raw.stderr_sha256.as_str(),
            receipt.raw.stderr_bytes,
        ),
    ] {
        safe_relative(path)?;
        let path = run_root.join(path);
        ensure_confined(run_root, &path, &format!("host raw {label}"))?;
        let actual = sha256_file(&path)?;
        let actual_bytes = fs::metadata(&path)
            .map_err(|error| format!("read raw {label} metadata: {error}"))?
            .len();
        if actual != expected || actual_bytes != bytes {
            return Err(format!("host receipt raw {label} identity mismatch"));
        }
    }
    let materialized_root = run_root.join("subjects").join(&receipt.case_id);
    ensure_confined(run_root, &materialized_root, "materialized subject")?;
    let executed_diff_identity =
        subject::executed_diff_identity(&materialized_root, &receipt.repository_base)?;
    if receipt.plan.executed_diff_identity != executed_diff_identity {
        return Err(format!(
            "host receipt `{}` executed diff identity does not match retained materialized Git bytes",
            receipt.case_id
        ));
    }
    let publishable = matches!(receipt.disposition.as_str(), "complete" | "typed_limited");
    let analyzer_identity = receipt.analyzer_input_identity.as_deref();
    if (publishable && analyzer_identity != Some(executed_diff_identity.as_str()))
        || analyzer_identity.is_some_and(|identity| identity != executed_diff_identity)
    {
        return Err(format!(
            "host receipt `{}` analyzer input identity does not match executed diff identity",
            receipt.case_id
        ));
    }
    if receipt.plan.base != receipt.repository_base
        || receipt.plan.head != receipt.repository_head
        || receipt.plan.tree != receipt.repository_tree
        || receipt.plan.argv.is_empty()
        || receipt.plan.config_sha256.trim().is_empty()
        || receipt.plan.diff_sha256.trim().is_empty()
        || receipt.plan.executed_diff_identity.trim().is_empty()
        || receipt.plan.subject_inputs.len() < 7
        || receipt.plan.subject_inputs.iter().any(|input| {
            input.role.trim().is_empty()
                || input.source_path.trim().is_empty()
                || input.repository_path.trim().is_empty()
                || input.sha256.trim().is_empty()
        })
    {
        return Err(format!(
            "host receipt `{}` has a stale typed plan",
            receipt.case_id
        ));
    }
    Ok(())
}

fn validate_generation(run_root: &Path) -> Result<(), String> {
    let index: HostRunIndex = read_strict_json(&run_root.join("run-index.json"), "host run index")?;
    if index.publication_state != "complete" || index.cases.len() != 3 {
        return Err("host run index is not a complete three-case generation".to_string());
    }
    if index.source.dirty {
        return Err("host run index cannot publish dirty build source".to_string());
    }
    validate_build_identity(run_root, &index.build)?;
    let mut seen = BTreeSet::new();
    for entry in &index.cases {
        if !seen.insert(entry.case_id.as_str()) {
            return Err(format!("host run index duplicates `{}`", entry.case_id));
        }
        safe_relative(&entry.receipt_path)?;
        let expected_receipt = format!("cases/{}/receipt.json", entry.case_id);
        if entry.receipt_path != expected_receipt {
            return Err(format!(
                "host run index receipt path for `{}` is not `{expected_receipt}`",
                entry.case_id
            ));
        }
        let receipt_path = run_root.join(&entry.receipt_path);
        ensure_confined(run_root, &receipt_path, "host receipt")?;
        if sha256_file(&receipt_path)? != entry.receipt_sha256 {
            return Err(format!(
                "host run index receipt digest mismatch for `{}`",
                entry.case_id
            ));
        }
        let receipt: CaseReceipt = read_strict_json(&receipt_path, "case receipt")?;
        validate_case_receipt(run_root, &receipt, &index.source, &index.build)?;
    }
    Ok(())
}

fn validate_build_identity(run_root: &Path, build: &BuildIdentity) -> Result<(), String> {
    let expected_target = Path::new(&build.executed_binary_path)
        .parent()
        .and_then(Path::parent)
        .map(|path| path.display().to_string());
    let command_matches = build.command.len() == 8
        && build.command.first().is_some_and(|value| value == "cargo")
        && build.command.get(1).is_some_and(|value| value == "build")
        && build.command.get(2).is_some_and(|value| value == "-p")
        && build.command.get(3).is_some_and(|value| value == "ripr")
        && build
            .command
            .get(4)
            .is_some_and(|value| value == "--locked")
        && build
            .command
            .get(5)
            .is_some_and(|value| value == "--offline")
        && build
            .command
            .get(6)
            .is_some_and(|value| value == "--target-dir")
        && build.command.get(7) == expected_target.as_ref();
    if build.package != "ripr"
        || build.profile != "dev"
        || build.features != ["default"]
        || !build.locked
        || !build.offline
        || !command_matches
    {
        return Err(
            "host run build identity is not the owned locked/offline dev build".to_string(),
        );
    }
    safe_relative(&build.retained_binary_path)?;
    let binary = run_root.join(&build.retained_binary_path);
    ensure_confined(run_root, &binary, "retained binary")?;
    let actual_bytes = fs::metadata(&binary)
        .map_err(|error| format!("read retained RIPR binary metadata: {error}"))?
        .len();
    if sha256_file(&binary)? != build.binary_sha256 || actual_bytes != build.binary_bytes {
        return Err("retained RIPR binary identity mismatch".to_string());
    }
    for (path, expected) in [
        ("build/stdout.bin", build.build_stdout_sha256.as_str()),
        ("build/stderr.bin", build.build_stderr_sha256.as_str()),
    ] {
        ensure_confined(run_root, &run_root.join(path), "build raw evidence")?;
        if sha256_file(&run_root.join(path))? != expected {
            return Err(format!("host run build raw digest mismatch for `{path}`"));
        }
    }
    Ok(())
}

fn validate_current(output_root: &Path, current: &CurrentRun) -> Result<(), String> {
    if current.run_id.is_empty()
        || current.run_id.contains('/')
        || current.run_id.contains('\\')
        || current.run_id.contains(':')
    {
        return Err("current host-run run id is not a single safe path component".to_string());
    }
    safe_relative(&current.index_path)?;
    let expected_path = format!("runs/{}/run-index.json", current.run_id);
    if current.index_path != expected_path {
        return Err(format!(
            "current host-run index path must be `{expected_path}`"
        ));
    }
    let path = output_root.join(&current.index_path);
    ensure_confined(output_root, &path, "host run index")?;
    if sha256_file(&path)? != current.index_sha256 {
        return Err("current host-run index digest mismatch".to_string());
    }
    let run_root = path
        .parent()
        .ok_or_else(|| "current host-run index has no run parent".to_string())?;
    validate_generation(run_root)
}

pub(super) fn load_validated_current(
    root: &Path,
    host_current: &str,
) -> Result<ValidatedHostRun, String> {
    let relative = Path::new(host_current);
    safe_relative(host_current)?;
    if super::normalize_path(relative) != host_current
        || relative.components().take(2).collect::<Vec<_>>()
            != [
                Component::Normal(OsStr::new("target")),
                Component::Normal(OsStr::new("ripr")),
            ]
    {
        return Err(
            "host current must be a normalized repository-relative path under `target/ripr/`"
                .to_string(),
        );
    }
    let current_path = root.join(relative);
    let root_canonical =
        fs::canonicalize(root).map_err(|error| format!("canonicalize repository root: {error}"))?;
    let current_canonical = fs::canonicalize(&current_path)
        .map_err(|error| format!("resolve host current `{host_current}`: {error}"))?;
    if !current_canonical.starts_with(&root_canonical) {
        return Err("host current escapes the repository through a link".to_string());
    }
    let output_root = current_path
        .parent()
        .ok_or_else(|| "host current has no output root".to_string())?;
    let current: CurrentRun = read_strict_json(&current_path, "current host run")?;
    if current.schema_version != "0.1" || current.kind != "rust_judged_panel_current_host_run" {
        return Err("host current has an unsupported schema or kind".to_string());
    }
    validate_current(output_root, &current)?;

    let index_path = output_root.join(&current.index_path);
    let index: HostRunIndex = read_strict_json(&index_path, "host run index")?;
    if index.schema_version != "0.1"
        || index.kind != "rust_judged_panel_host_run_index"
        || index.publication_state != "complete"
        || index.run_id != current.run_id
    {
        return Err("host current and run index authority do not agree".to_string());
    }
    let run_root = index_path
        .parent()
        .ok_or_else(|| "host run index has no generation root".to_string())?;
    let output_relative = relative
        .parent()
        .ok_or_else(|| "host current has no relative output root".to_string())?;
    let run_relative = Path::new(&current.index_path)
        .parent()
        .ok_or_else(|| "host run index has no relative generation root".to_string())?;
    let mut cases = Vec::new();
    for entry in &index.cases {
        let receipt_path = run_root.join(&entry.receipt_path);
        ensure_confined(run_root, &receipt_path, "host receipt")?;
        let receipt: CaseReceipt = read_strict_json(&receipt_path, "case receipt")?;
        if receipt.schema_version != "0.1"
            || receipt.kind != "rust_judged_panel_host_run_receipt"
            || receipt.case_id != entry.case_id
            || receipt.expected_direction != entry.expected_direction
            || receipt.raw.stdout_sha256 != entry.stdout_sha256
            || receipt.raw.stderr_sha256 != entry.stderr_sha256
        {
            return Err(format!(
                "host run index and receipt authority do not agree for `{}`",
                entry.case_id
            ));
        }
        if !matches!(receipt.disposition.as_str(), "complete" | "typed_limited")
            || receipt.timed_out
            || receipt.exit_code != Some(0)
            || receipt.error.is_some()
        {
            return Err(format!(
                "host receipt `{}` is not a publishable completed analysis",
                receipt.case_id
            ));
        }
        let analyzer_input_identity = receipt.analyzer_input_identity.clone().ok_or_else(|| {
            format!(
                "host receipt `{}` lacks analyzer input identity",
                receipt.case_id
            )
        })?;
        safe_relative(&receipt.raw.stdout_path)?;
        safe_relative(&receipt.raw.stderr_path)?;
        let stdout_path = run_root.join(&receipt.raw.stdout_path);
        let stderr_path = run_root.join(&receipt.raw.stderr_path);
        ensure_confined(run_root, &stdout_path, "host stdout")?;
        ensure_confined(run_root, &stderr_path, "host stderr")?;
        let stdout = fs::read(&stdout_path).map_err(|error| {
            format!(
                "read host stdout for `{}` at `{}`: {error}",
                receipt.case_id,
                stdout_path.display()
            )
        })?;
        let receipt_ref =
            super::normalize_path(&output_relative.join(run_relative).join(&entry.receipt_path));
        let stdout_ref = super::normalize_path(
            &output_relative
                .join(run_relative)
                .join(&receipt.raw.stdout_path),
        );
        let stderr_ref = super::normalize_path(
            &output_relative
                .join(run_relative)
                .join(&receipt.raw.stderr_path),
        );
        for value in [&receipt_ref, &stdout_ref, &stderr_ref] {
            safe_relative(value)?;
        }
        cases.push(ValidatedHostCase {
            case_id: receipt.case_id.clone(),
            subject_id: receipt.subject_id.clone(),
            expected_direction: receipt.expected_direction.clone(),
            repository_base: receipt.repository_base.clone(),
            repository_head: receipt.repository_head.clone(),
            repository_tree: receipt.repository_tree.clone(),
            argv: receipt.plan.argv.clone(),
            mode: receipt.plan.mode.clone(),
            format: receipt.plan.format.clone(),
            config_path: receipt.plan.config_path.clone(),
            config_sha256: receipt.plan.config_sha256.clone(),
            diff_path: receipt.plan.diff_path.clone(),
            diff_sha256: receipt.plan.diff_sha256.clone(),
            executed_diff_identity: receipt.plan.executed_diff_identity.clone(),
            subject_inputs: receipt
                .plan
                .subject_inputs
                .iter()
                .map(|input| ValidatedInputDigest {
                    role: input.role.clone(),
                    source_path: input.source_path.clone(),
                    repository_path: input.repository_path.clone(),
                    sha256: input.sha256.clone(),
                })
                .collect(),
            disposition: receipt.disposition.clone(),
            analyzer_input_identity,
            receipt_ref,
            receipt_sha256: entry.receipt_sha256.clone(),
            stdout_ref,
            stdout_sha256: receipt.raw.stdout_sha256.clone(),
            stderr_ref,
            stderr_sha256: receipt.raw.stderr_sha256.clone(),
            stdout,
            materialized_root: run_root.join("subjects").join(&receipt.case_id),
        });
    }
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    Ok(ValidatedHostRun {
        current_ref: host_current.to_string(),
        current_sha256: sha256_file(&current_path)?,
        index_ref: super::normalize_path(&output_relative.join(&current.index_path)),
        index_sha256: current.index_sha256,
        run_id: index.run_id,
        source_head: index.source.head,
        source_tree: index.source.tree,
        cargo_lock_sha256: index.source.cargo_lock_sha256,
        cargo_toml_sha256: index.source.cargo_toml_sha256,
        profile: index.build.profile,
        features: index.build.features,
        host_target: index.build.host_target,
        binary_sha256: index.build.binary_sha256,
        binary_version: index.build.binary_version,
        cases,
    })
}

fn validate_binary_unchanged(path: &Path, expected: &str) -> Result<(), String> {
    let actual = sha256_file(path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "fresh RIPR binary changed after build: expected {expected}, got {actual}"
        ))
    }
}

fn validate_source_unchanged(
    root: &Path,
    expected: &SourceIdentity,
    actual: &RepositoryState,
) -> Result<(), String> {
    let current = source_identity(root, actual)?;
    if &current == expected {
        Ok(())
    } else {
        Err("build source changed during the host-run transaction".to_string())
    }
}

fn acquire_lock(output_root: &Path) -> Result<RunLock, String> {
    let path = output_root.join("replay.lock");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            format!(
                "another replay or stale lock owns `{}`: {error}",
                path.display()
            )
        })?;
    if let Err(error) = writeln!(file, "pid={}", std::process::id()) {
        let _result = fs::remove_file(&path);
        return Err(format!("write replay lock `{}`: {error}", path.display()));
    }
    Ok(RunLock(path))
}

fn confined_output(root: &Path, value: &str) -> Result<PathBuf, String> {
    safe_relative(value)?;
    if value.contains("//") || value.contains(':') {
        return Err(format!("host-run output `{value}` is not normalized"));
    }
    let components = Path::new(value)
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>();
    if components.get(0..2) != Some(&["target", "ripr"]) {
        return Err("host-run output must remain under `target/ripr/`".to_string());
    }
    let root_canonical =
        fs::canonicalize(root).map_err(|error| format!("canonicalize repository root: {error}"))?;
    let output = root.join(value);
    let mut ancestor = output.as_path();
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| "host-run output has no existing ancestor".to_string())?;
    }
    let ancestor_canonical = fs::canonicalize(ancestor)
        .map_err(|error| format!("canonicalize host-run output ancestor: {error}"))?;
    if !ancestor_canonical.starts_with(&root_canonical) {
        return Err("host-run output escapes the repository through a link".to_string());
    }
    Ok(output)
}

fn successful_text_command(program: &Path, args: &[&str], label: &str) -> Result<String, String> {
    let owned = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    let bytes = capture_process_output(&program.display().to_string(), &owned, &[])
        .map_err(|error| format!("{label}: {}", error.message))?;
    String::from_utf8(bytes)
        .map(|text| text.trim().to_string())
        .map_err(|error| format!("{label} was not UTF-8: {error}"))
}

fn workspace_version(root: &Path) -> Result<String, String> {
    let body = fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|error| format!("read workspace Cargo.toml: {error}"))?;
    let value: toml::Value =
        toml::from_str(&body).map_err(|error| format!("parse workspace Cargo.toml: {error}"))?;
    value
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| "workspace package version is missing".to_string())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("output `{}` has no parent", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create `{}`: {error}", parent.display()))?;
    let sequence = RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("receipt"),
        std::process::id(),
        sequence
    ));
    fs::write(&temp, bytes).map_err(|error| format!("write `{}`: {error}", temp.display()))?;
    fs::rename(&temp, path).map_err(|error| format!("publish `{}`: {error}", path.display()))
}

fn read_strict_json<T: for<'de> Deserialize<'de>>(path: &Path, label: &str) -> Result<T, String> {
    let body = fs::read_to_string(path)
        .map_err(|error| format!("read {label} `{}`: {error}", path.display()))?;
    let value = super::parse_json_without_duplicate_keys(&body)
        .map_err(|error| format!("parse {label} `{}`: {error}", path.display()))?;
    serde_json::from_value(value)
        .map_err(|error| format!("parse {label} `{}`: {error}", path.display()))
}

fn pretty_json<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("serialize host-run JSON: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("read digest input `{}`: {error}", path.display()))?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn safe_relative(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty()
        || value.contains('\\')
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "path `{value}` must be normalized repository-relative text"
        ));
    }
    Ok(())
}

fn ensure_confined(root: &Path, path: &Path, label: &str) -> Result<(), String> {
    let root = fs::canonicalize(root)
        .map_err(|error| format!("canonicalize {label} root `{}`: {error}", root.display()))?;
    let resolved = fs::canonicalize(path)
        .map_err(|error| format!("canonicalize {label} `{}`: {error}", path.display()))?;
    if resolved.starts_with(&root) {
        Ok(())
    } else {
        Err(format!("{label} escapes its retained generation"))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::{
        BuildIdentity, CaseReceipt, CurrentRun, ExecutionPlan, InputDigest, RawEvidence,
        SourceIdentity, acquire_lock, classify_successful_stdout, confined_output,
        publish_complete_generation, sha256_bytes, subject, validate_binary_unchanged,
        validate_case_receipt, validate_case_set, validate_current,
    };

    fn repository_root() -> Result<PathBuf, String> {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "xtask manifest must have a repository parent".to_string())
    }

    fn scratch(label: &str) -> Result<PathBuf, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let path = repository_root()?
            .join("target/ripr/rust-judged-panel-host-tests")
            .join(format!("{label}-{nonce}"));
        fs::create_dir_all(&path).map_err(|error| error.to_string())?;
        Ok(path)
    }

    fn source() -> SourceIdentity {
        SourceIdentity {
            head: "1".repeat(40),
            tree: "2".repeat(40),
            cargo_lock_sha256: format!("sha256:{}", "3".repeat(64)),
            cargo_toml_sha256: format!("sha256:{}", "4".repeat(64)),
            dirty: false,
        }
    }

    fn build() -> BuildIdentity {
        BuildIdentity {
            command: vec!["cargo".to_string(), "build".to_string()],
            package: "ripr".to_string(),
            profile: "dev".to_string(),
            features: vec!["default".to_string()],
            locked: true,
            offline: true,
            cargo_version: "cargo fixture".to_string(),
            rustc_verbose_version: "rustc fixture".to_string(),
            host_target: "fixture-host".to_string(),
            cargo_home: None,
            executed_binary_path: "fixture".to_string(),
            retained_binary_path: "build-target/debug/ripr".to_string(),
            binary_sha256: format!("sha256:{}", "5".repeat(64)),
            binary_bytes: 1,
            binary_version: "ripr 0.10.0".to_string(),
            build_stdout_sha256: sha256_bytes(b""),
            build_stderr_sha256: sha256_bytes(b""),
        }
    }

    fn receipt(root: &Path, case_id: &str, direction: &str) -> Result<CaseReceipt, String> {
        let identities = subject::materialize_diff_fixture(&root.join("subjects").join(case_id))?;
        let case_dir = root.join("cases").join(case_id);
        fs::create_dir_all(&case_dir).map_err(|error| error.to_string())?;
        fs::write(case_dir.join("stdout.bin"), b"out").map_err(|error| error.to_string())?;
        fs::write(case_dir.join("stderr.bin"), b"err").map_err(|error| error.to_string())?;
        Ok(CaseReceipt {
            schema_version: "0.1".to_string(),
            kind: "rust_judged_panel_host_run_receipt".to_string(),
            case_id: case_id.to_string(),
            subject_id: "fixture".to_string(),
            expected_direction: direction.to_string(),
            source_head: source().head,
            source_tree: source().tree,
            binary_sha256: build().binary_sha256,
            repository_base: identities.0.clone(),
            repository_head: identities.1.clone(),
            repository_tree: identities.2.clone(),
            plan: ExecutionPlan {
                argv: vec!["check".to_string()],
                root: "<materialized-subject>".to_string(),
                base: identities.0,
                head: identities.1,
                tree: identities.2,
                mode: "draft".to_string(),
                format: "json".to_string(),
                config_path: "ripr.toml".to_string(),
                config_sha256: "config".to_string(),
                diff_path: "diff.patch".to_string(),
                diff_sha256: "diff".to_string(),
                executed_diff_identity: identities.3.clone(),
                subject_inputs: vec![
                    InputDigest {
                        role: "fixture".to_string(),
                        source_path: "fixture/source".to_string(),
                        repository_path: "fixture/repository".to_string(),
                        sha256: "sha256:fixture".to_string(),
                    };
                    7
                ],
            },
            disposition: "complete".to_string(),
            exit_code: Some(0),
            timed_out: false,
            duration_ms: Duration::ZERO.as_millis(),
            analyzer_input_identity: Some(identities.3),
            raw: RawEvidence {
                stdout_path: format!("cases/{case_id}/stdout.bin"),
                stdout_sha256: sha256_bytes(b"out"),
                stdout_bytes: 3,
                stderr_path: format!("cases/{case_id}/stderr.bin"),
                stderr_sha256: sha256_bytes(b"err"),
                stderr_bytes: 3,
            },
            error: None,
        })
    }

    #[test]
    fn concurrent_writer_cannot_take_live_lock() -> Result<(), String> {
        let root = scratch("lock")?;
        let first = acquire_lock(&root)?;
        let rejected = acquire_lock(&root).is_err();
        drop(first);
        fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        if rejected {
            Ok(())
        } else {
            Err("second writer acquired a live replay lock".to_string())
        }
    }

    #[test]
    fn output_is_confined_to_target_ripr() -> Result<(), String> {
        let root = repository_root()?;
        for value in [
            "../escape".to_string(),
            format!("{}:/escape", char::from(b'C')),
            "target//ripr/run".to_string(),
            "target/ripr/../escape".to_string(),
        ] {
            if confined_output(&root, &value).is_ok() {
                return Err(format!("unsafe host-run output was accepted: {value}"));
            }
        }
        confined_output(&root, "target/ripr/rust-judged-panel").map(|_| ())
    }

    #[test]
    fn current_pointer_must_join_exact_run_index() -> Result<(), String> {
        let root = scratch("current-join")?;
        let current = CurrentRun {
            schema_version: "0.1".to_string(),
            kind: "rust_judged_panel_current_host_run".to_string(),
            run_id: "run-a".to_string(),
            index_path: "runs/run-b/run-index.json".to_string(),
            index_sha256: "sha256:index".to_string(),
        };
        let rejected = validate_current(&root, &current).is_err();
        fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        if rejected {
            Ok(())
        } else {
            Err("current pointer accepted a mismatched generation path".to_string())
        }
    }

    #[test]
    fn missing_or_duplicate_case_cannot_form_complete_set() -> Result<(), String> {
        let root = scratch("case-set")?;
        let gap = receipt(
            &root,
            "rust-boundary-missing-equality-should-gap",
            "should_gap",
        )?;
        let quiet = receipt(
            &root,
            "rust-boundary-exact-equality-should-stay-quiet",
            "should_stay_quiet",
        )?;
        if validate_case_set(&[gap.clone(), quiet.clone()]).is_ok()
            || validate_case_set(&[gap.clone(), gap, quiet]).is_ok()
        {
            return Err("partial or duplicate host-run case set was accepted".to_string());
        }
        fs::remove_dir_all(&root).map_err(|error| error.to_string())
    }

    #[test]
    fn raw_or_typed_identity_tamper_is_rejected() -> Result<(), String> {
        let root = scratch("tamper")?;
        let mut value = receipt(
            &root,
            "rust-boundary-missing-equality-should-gap",
            "should_gap",
        )?;
        validate_case_receipt(&root, &value, &source(), &build())?;
        fs::write(root.join(&value.raw.stdout_path), b"changed")
            .map_err(|error| error.to_string())?;
        if validate_case_receipt(&root, &value, &source(), &build()).is_ok() {
            return Err("raw output tamper was accepted".to_string());
        }
        fs::write(root.join(&value.raw.stdout_path), b"out").map_err(|error| error.to_string())?;
        value.plan.base = "d".repeat(40);
        if validate_case_receipt(&root, &value, &source(), &build()).is_ok() {
            return Err("typed plan tamper was accepted".to_string());
        }
        fs::remove_dir_all(&root).map_err(|error| error.to_string())
    }

    #[test]
    fn stale_or_missing_analyzer_identity_is_rejected_against_materialized_diff()
    -> Result<(), String> {
        let root = scratch("stale-analyzer-input")?;
        let mut value = receipt(
            &root,
            "rust-boundary-missing-equality-should-gap",
            "should_gap",
        )?;
        validate_case_receipt(&root, &value, &source(), &build())?;
        value.analyzer_input_identity = Some("sha256:stale-output".to_string());
        let stale_rejected = validate_case_receipt(&root, &value, &source(), &build()).is_err();
        value.analyzer_input_identity = None;
        let missing_rejected = validate_case_receipt(&root, &value, &source(), &build()).is_err();
        fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        if stale_rejected && missing_rejected {
            Ok(())
        } else {
            Err(format!(
                "analyzer input identity rejection failed: stale={stale_rejected} missing={missing_rejected}"
            ))
        }
    }

    #[test]
    fn changed_built_binary_is_rejected() -> Result<(), String> {
        let root = scratch("binary")?;
        let binary = root.join("ripr");
        fs::write(&binary, b"first").map_err(|error| error.to_string())?;
        let expected = sha256_bytes(b"first");
        validate_binary_unchanged(&binary, &expected)?;
        fs::write(&binary, b"second").map_err(|error| error.to_string())?;
        let rejected = validate_binary_unchanged(&binary, &expected).is_err();
        fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        if rejected {
            Ok(())
        } else {
            Err("changed built binary was accepted".to_string())
        }
    }

    #[test]
    fn successful_output_distinguishes_malformed_incomplete_and_limited() -> Result<(), String> {
        let malformed = classify_successful_stdout(b"not-json");
        let incomplete = classify_successful_stdout(
            br#"{"analysis_outcome":{"analysis_complete":false,"outcome":{"identity":{"input_identity":"sha256:fixture"},"limitations":[]}}}"#,
        );
        let limited = classify_successful_stdout(
            br#"{"analysis_outcome":{"analysis_complete":true,"outcome":{"identity":{"input_identity":"sha256:fixture"},"limitations":[{"kind":"macro"}]}}}"#,
        );
        if malformed.disposition != "malformed_output"
            || incomplete.disposition != "incomplete_output"
            || limited.disposition != "typed_limited"
        {
            Err(format!(
                "wrong dispositions: malformed={} incomplete={} limited={}",
                malformed.disposition, incomplete.disposition, limited.disposition
            ))
        } else {
            Ok(())
        }
    }

    #[test]
    fn failure_after_two_cases_preserves_previous_current() -> Result<(), String> {
        let root = scratch("partial-current")?;
        let attempt = root.join("attempt");
        fs::create_dir_all(&attempt).map_err(|error| error.to_string())?;
        let current = root.join("current.json");
        fs::write(&current, b"previous-current").map_err(|error| error.to_string())?;
        let before = fs::read(&current).map_err(|error| error.to_string())?;
        let receipts = vec![
            receipt(
                &attempt,
                "rust-boundary-missing-equality-should-gap",
                "should_gap",
            )?,
            receipt(
                &attempt,
                "rust-boundary-exact-equality-should-stay-quiet",
                "should_stay_quiet",
            )?,
        ];
        if publish_complete_generation(&root, &attempt, "fixture-run", source(), build(), &receipts)
            .is_ok()
        {
            return Err("two-case generation was published".to_string());
        }
        let after = fs::read(&current).map_err(|error| error.to_string())?;
        let attempt_preserved = attempt.exists();
        fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        if before == after && attempt_preserved {
            Ok(())
        } else {
            Err("partial generation changed current or lost diagnostic staging".to_string())
        }
    }
}
