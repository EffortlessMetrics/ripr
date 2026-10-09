//! Benchmark B1: agent-funnel E2E (`benchmarks/agentic/agent-funnel`).
//!
//! The harness runs `ripr first-action` against a real Git fixture, then
//! executes the printed `after_snapshot`, `analysis_outcome`, `verify`, and
//! `receipt` commands byte for byte through bash from a foreign working
//! directory (launch directory != workspace root), the way a consuming agent
//! pastes them. The `>` redirect in the printed commands is shell syntax, so
//! a shell is the honest executor; each printed string reaches bash
//! byte for byte via a script file, never through `bash -c` (whose argv
//! joining and MSYS re-parsing on Windows rewrite the quoting before bash
//! ever sees it).
//!
//! The oracle is on-disk and byte-oriented, never string-shaped: the verify
//! redirect must land `target/ripr/workflow/agent-verify.json`, the analysis
//! outcome must land beside it, the receipt must bind the exact fresh bytes
//! through its `provenance.*.sha256` commitments (sha256 over file bytes,
//! compared as bytes/digests), and `ripr agent status` must re-read the
//! persisted receipt chain as present and current.
//!
//! Two anti-gaming twins prove the oracle is load-bearing: a mutated
//! stdout-only verify (redirect stripped, exit still 0) must FAIL the
//! oracle, and a planted stale `agent-verify.json` must be overwritten by
//! fresh bytes the receipt binds instead of the decoy digest.
//!
//! The happy path writes a `ripr-agent-funnel-benchmark-v1` receipt to
//! `target/ripr/reports/agent-funnel-benchmark.json` (revision, runner
//! class, analyzer version, named checks, claim boundary), mirroring the
//! `targeted-rerun-benchmark` receipt pattern.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "common/mod.rs"]
mod common;

use common::fixture_git::fixture_git_ok;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn run_ripr(current_dir: &Path, args: &[&str]) -> Result<Output, String> {
    Command::new(env!("CARGO_BIN_EXE_ripr"))
        .current_dir(current_dir)
        .args(args)
        .output()
        .map_err(|error| format!("spawn ripr {args:?} in {}: {error}", current_dir.display()))
}

fn assert_success(output: &Output, label: &str) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{label} failed with {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    ))
}

fn unique_temp_workspace(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ripr-{label}-{stamp}-{pid}-{counter}"))
}

fn foreign_launch_dir(base: &Path) -> Result<PathBuf, String> {
    let dir = base.join("foreign-launch");
    std::fs::create_dir_all(&dir).map_err(|error| format!("create {}: {error}", dir.display()))?;
    Ok(dir)
}

fn write_fixture_source(root: &Path, relative: &str) -> Result<(), String> {
    let fixture_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks/agentic/agent-funnel/input");
    let destination = root.join(relative);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    std::fs::copy(fixture_root.join(relative), &destination)
        .map_err(|error| format!("copy fixture {relative} into {}: {error}", root.display()))?;
    Ok(())
}

fn init_producer_fixture_repo(root: &Path) -> Result<(), String> {
    for relative in ["Cargo.toml", "src/lib.rs", "tests/pricing.rs"] {
        write_fixture_source(root, relative)?;
    }
    fixture_git_ok(root, &["init"]).map_err(|error| format!("fixture git init: {error}"))?;
    fixture_git_ok(root, &["config", "core.autocrlf", "false"])
        .map_err(|error| format!("fixture git config: {error}"))?;
    fixture_git_ok(
        root,
        &["add", "Cargo.toml", "src/lib.rs", "tests/pricing.rs"],
    )
    .map_err(|error| format!("fixture git add: {error}"))?;
    commit_fixture(root, "fixture")
}

fn commit_fixture(root: &Path, message: &str) -> Result<(), String> {
    fixture_git_ok(
        root,
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            message,
        ],
    )
    .map_err(|error| format!("fixture commit in {}: {error}", root.display()))
}

fn advance_fixture_head(root: &Path, message: &str) -> Result<(), String> {
    commit_fixture(root, message)
}

fn first_snapshot_seam_id(snapshot_path: &Path) -> Result<String, String> {
    let snapshot = read_json(snapshot_path)?;
    snapshot
        .pointer("/seams/0/seam_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            format!(
                "fixture snapshot {} carries no seams; the funnel needs a real seam",
                snapshot_path.display()
            )
        })
}

fn write_assistant_proof(path: &Path, seam_id: &str) -> Result<(), String> {
    let proof = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "seam": {
            "seam_id": seam_id,
            "seam_kind": "predicate_boundary",
            "path": "src/lib.rs",
            "line": 2,
            "grip_class": "weakly_exposed",
            "missing_discriminator": "assert_eq!(discounted_total(100, 100), 90)"
        }
    });
    std::fs::write(
        path,
        serde_json::to_string_pretty(&proof).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("write assistant proof {}: {error}", path.display()))
}

fn produce_repo_exposure_snapshot(
    root: &Path,
    root_arg: &str,
    destination: &Path,
) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let file = std::fs::File::create(destination)
        .map_err(|error| format!("create {}: {error}", destination.display()))?;
    let output = Command::new(env!("CARGO_BIN_EXE_ripr"))
        .args([
            "check",
            "--root",
            root_arg,
            "--mode",
            "draft",
            "--format",
            "repo-exposure-json",
        ])
        .current_dir(root)
        .stdout(Stdio::from(file))
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("spawn setup snapshot check in {}: {error}", root.display()))?;
    assert_success(
        &output,
        &format!("setup repo-exposure snapshot for {}", root.display()),
    )
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = read_bytes(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {} as JSON: {error}", path.display()))
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn sha256_file(path: &Path) -> Result<String, String> {
    Ok(sha256_bytes(&read_bytes(path)?))
}

fn shell_display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn discover_posix_shell() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        for prefix_var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(prefix) = std::env::var_os(prefix_var) {
                candidates.push(PathBuf::from(prefix).join("Git/bin/bash.exe"));
            }
        }
        candidates.push(PathBuf::from("bash.exe"));
    } else {
        candidates.push(PathBuf::from("/bin/bash"));
        candidates.push(PathBuf::from("bash"));
    }
    let probe_dir = unique_temp_workspace("funnel-shell-probe");
    std::fs::create_dir_all(&probe_dir).ok()?;
    let probe_script = probe_dir.join("probe.sh");
    if std::fs::write(&probe_script, b"printf 'ok\\n'\n").is_err() {
        let _ = std::fs::remove_dir_all(&probe_dir);
        return None;
    }
    let found = candidates.into_iter().find(|candidate| {
        std::process::Command::new(candidate)
            .arg(shell_display_path(&probe_script))
            .output()
            .is_ok_and(|output| {
                output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "ok"
            })
    });
    let _ = std::fs::remove_dir_all(&probe_dir);
    found
}

fn shell_prerequisite() -> Result<Option<PathBuf>, String> {
    if let Some(bash) = discover_posix_shell() {
        return Ok(Some(bash));
    }
    if std::env::var_os("GITHUB_ACTIONS").is_some() {
        return Err(
            "bash is not usable under GitHub Actions; the agent-funnel benchmark cannot be skipped in CI"
                .to_string(),
        );
    }
    eprintln!(
        "SKIPPED agentic_bench_agent: no usable `bash` (Git Bash on Windows) was found; the printed funnel commands need a POSIX shell"
    );
    Ok(None)
}

fn shell_path_env() -> Result<std::ffi::OsString, String> {
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_ripr"))
        .parent()
        .ok_or_else(|| "ripr binary path has no parent directory".to_string())?
        .to_path_buf();
    let existing = std::env::var_os("PATH").ok_or_else(|| "PATH is not set".to_string())?;
    std::env::join_paths(std::iter::once(bin_dir).chain(std::env::split_paths(&existing)))
        .map_err(|error| format!("join PATH entries: {error}"))
}

fn run_in_shell(journey: &Journey, command: &str) -> Result<Output, String> {
    let script = journey.launch_dir.join(format!(
        "funnel-{}.sh",
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&script, command.as_bytes())
        .map_err(|error| format!("write script for printed command: {error}"))?;
    Command::new(&journey.bash)
        .arg(shell_display_path(&script))
        .current_dir(&journey.launch_dir)
        .env("PATH", &journey.path_env)
        .output()
        .map_err(|error| format!("spawn shell for printed command: {error}"))
}

struct Journey {
    root: PathBuf,
    root_arg: String,
    launch_dir: PathBuf,
    path_env: std::ffi::OsString,
    bash: PathBuf,
    seam_id: String,
}

fn workflow_artifact(root: &Path, relative: &str) -> PathBuf {
    root.join("target/ripr/workflow").join(relative)
}

fn root_arg_of(root: &Path) -> String {
    root.display().to_string()
}

fn start_journey(label: &str, bash: &Path) -> Result<(Journey, Value), String> {
    let root = unique_temp_workspace(label);
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("create {}: {error}", root.display()))?;
    let result = start_journey_at_root(&root, bash);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&root);
    }
    result
}

fn start_journey_at_root(root: &Path, bash: &Path) -> Result<(Journey, Value), String> {
    init_producer_fixture_repo(root)?;
    produce_repo_exposure_snapshot(
        root,
        &root_arg_of(root),
        &workflow_artifact(root, "before.repo-exposure.json"),
    )?;
    advance_fixture_head(root, "movement for the after snapshot")?;

    let seam_id = first_snapshot_seam_id(&workflow_artifact(root, "before.repo-exposure.json"))?;
    std::fs::create_dir_all(root.join("target/ripr/reports"))
        .map_err(|error| format!("create reports dir: {error}"))?;
    let proof_path = root.join("target/ripr/reports/test-oracle-assistant-proof.json");
    write_assistant_proof(&proof_path, &seam_id)?;

    let journey = Journey {
        root_arg: root_arg_of(root),
        launch_dir: foreign_launch_dir(root)?,
        path_env: shell_path_env()?,
        bash: bash.to_path_buf(),
        seam_id,
        root: root.to_path_buf(),
    };

    let report_path = journey
        .root
        .join("target/ripr/reports/first-useful-action.json");
    let report_path_md = journey
        .root
        .join("target/ripr/reports/first-useful-action.md");
    let output = run_ripr(
        &journey.launch_dir,
        &[
            "first-action",
            "--root",
            &journey.root_arg,
            "--assistant-proof",
            &proof_path.display().to_string(),
            "--out",
            &report_path.display().to_string(),
            "--out-md",
            &report_path_md.display().to_string(),
        ],
    )?;
    assert_success(&output, "ripr first-action")?;
    let report = read_json(&report_path)?;
    if report.pointer("/status").and_then(Value::as_str) != Some("actionable") {
        return Err(format!(
            "first-action did not route to an actionable report:\n{report}"
        ));
    }
    Ok((journey, report))
}

fn printed_command(report: &Value, field: &str) -> Result<String, String> {
    report
        .pointer(&format!("/commands/{field}"))
        .and_then(Value::as_str)
        .filter(|command| !command.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("first-action report carries no /commands/{field}:\n{report}"))
}

fn run_printed_step(journey: &Journey, report: &Value, field: &str) -> Result<(), String> {
    let command = printed_command(report, field)?;
    let output = run_in_shell(journey, &command)?;
    assert_success(
        &output,
        &format!("printed {field} command through a shell: {command}"),
    )
}

fn run_printed_receipt_stdout(journey: &Journey, report: &Value) -> Result<Value, String> {
    let command = printed_command(report, "receipt")?;
    let output = run_in_shell(journey, &command)?;
    assert_success(
        &output,
        &format!("printed receipt command through a shell: {command}"),
    )?;
    serde_json::from_slice(&output.stdout).map_err(|error| {
        format!(
            "receipt output is not one JSON document: {error}\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

/// Mutate a printed verify command into its stdout-only twin: the shell
/// redirect is stripped, so the verify document is produced but never lands
/// on disk. Fails closed when the printed shape carries no redirect.
fn strip_shell_redirect(command: &str) -> Result<String, String> {
    match command.rfind(" > ") {
        Some(index) => Ok(command[..index].to_string()),
        None => Err(format!(
            "printed verify command carries no shell redirect to mutate: {command}"
        )),
    }
}

/// The byte oracle: both funnel writes exist on disk, and the receipt binds
/// the exact bytes now on disk through its sha256 provenance commitments.
/// Every comparison below is over file bytes or their digests, never over
/// command-output text or substring matching.
fn assert_funnel_writes_bind_fresh_artifacts(
    journey: &Journey,
    receipt: &Value,
) -> Result<(), String> {
    let verify_path = workflow_artifact(&journey.root, "agent-verify.json");
    let analysis_path = workflow_artifact(&journey.root, "analysis-outcome.json");
    for (label, path) in [
        ("agent-verify.json", &verify_path),
        ("analysis-outcome.json", &analysis_path),
    ] {
        if !path.is_file() {
            return Err(format!(
                "the funnel claimed to write {label} but {} is missing",
                path.display()
            ));
        }
    }

    let expected = [
        ("verify_artifact", sha256_file(&verify_path)?),
        (
            "before_artifact",
            sha256_file(&workflow_artifact(
                &journey.root,
                "before.repo-exposure.json",
            ))?,
        ),
        (
            "after_artifact",
            sha256_file(&workflow_artifact(
                &journey.root,
                "after.repo-exposure.json",
            ))?,
        ),
    ];
    for (artifact, digest) in expected {
        let recorded = receipt
            .pointer(&format!("/provenance/{artifact}/sha256"))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("receipt provenance is missing {artifact}.sha256"))?;
        if recorded != digest {
            return Err(format!(
                "receipt {artifact} digest {recorded} does not bind the bytes on disk ({digest})"
            ));
        }
    }
    let recorded_seam = receipt
        .pointer("/seam/seam_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "receipt carries no seam.seam_id".to_string())?;
    if recorded_seam != journey.seam_id {
        return Err(format!(
            "receipt routed seam {recorded_seam}, expected the fixture seam {}",
            journey.seam_id
        ));
    }
    if receipt
        .pointer("/analysis_outcome_status")
        .and_then(Value::as_str)
        != Some("complete")
    {
        return Err(format!(
            "receipt did not read the funnel-written analysis outcome as complete:\n{receipt}"
        ));
    }
    let analysis = read_json(&analysis_path)?;
    let written_outcome = analysis
        .pointer("/analysis_outcome/outcome")
        .filter(|outcome| outcome.is_object())
        .ok_or("fresh analysis artifact carries no typed outcome")?;
    if receipt.pointer("/analysis_outcome/outcome") != Some(written_outcome) {
        return Err(
            "receipt did not carry the outcome written by the advertised command".to_string(),
        );
    }
    if receipt.pointer("/status").and_then(Value::as_str) != Some("advisory") {
        return Err(format!(
            "receipt over the complete funnel chain is not advisory:\n{receipt}"
        ));
    }
    Ok(())
}

fn agent_status_json(journey: &Journey) -> Result<Value, String> {
    let output = run_ripr(
        &journey.launch_dir,
        &["agent", "status", "--root", &journey.root_arg, "--json"],
    )?;
    assert_success(&output, "ripr agent status --json")?;
    serde_json::from_slice(&output.stdout).map_err(|error| {
        format!(
            "agent status stdout is not JSON: {error}\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn artifact_present(status: &Value, name: &str) -> Result<bool, String> {
    let entry = status
        .pointer("/artifacts")
        .and_then(Value::as_array)
        .ok_or_else(|| "agent status carries no artifacts array".to_string())?
        .iter()
        .find(|artifact| artifact.get("name").and_then(Value::as_str) == Some(name))
        .ok_or_else(|| format!("agent status has no {name} artifact entry"))?;
    Ok(entry.get("state").and_then(Value::as_str) == Some("present"))
}

fn stale_warning_messages(status: &Value) -> Vec<String> {
    status
        .pointer("/warnings")
        .and_then(Value::as_array)
        .map(|warnings| {
            warnings
                .iter()
                .filter(|warning| {
                    warning.get("kind").and_then(Value::as_str) == Some("stale_artifact")
                })
                .filter_map(|warning| {
                    warning
                        .get("message")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn persist_receipt_for_status(journey: &Journey) -> Result<(), String> {
    let out_path = journey.root.join("target/ripr/reports/agent-receipt.json");
    let output = run_ripr(
        &journey.launch_dir,
        &[
            "agent",
            "receipt",
            "--root",
            &journey.root_arg,
            "--verify-json",
            "target/ripr/workflow/agent-verify.json",
            "--seam-id",
            &journey.seam_id,
            "--json",
            "--out",
            &out_path.display().to_string(),
        ],
    )?;
    assert_success(
        &output,
        "ripr agent receipt --out (status-prescribed completion)",
    )?;
    if !out_path.is_file() {
        return Err(format!(
            "agent receipt --out did not write {}",
            out_path.display()
        ));
    }
    Ok(())
}

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn benchmark_check(name: &str, detail: &str) -> Value {
    serde_json::json!({"name": name, "state": "pass", "detail": detail})
}

fn git_revision() -> String {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&workspace)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unavailable".to_string())
}

fn runner_class() -> String {
    std::env::var("RUNNER_NAME")
        .or_else(|_| std::env::var("GITHUB_RUNNER_NAME"))
        .unwrap_or_else(|_| format!("local-{}-{}", std::env::consts::OS, std::env::consts::ARCH))
}

fn analyzer_version() -> String {
    Command::new(env!("CARGO_BIN_EXE_ripr"))
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unavailable".to_string())
}

fn write_benchmark_receipt(
    status: &str,
    checks: &[Value],
    error: Option<&str>,
) -> Result<PathBuf, String> {
    let receipt = serde_json::json!({
        "schema_version": "ripr-agent-funnel-benchmark-v1",
        "tool": "ripr",
        "report": "agent-funnel-benchmark",
        "status": status,
        "benchmark": "B1",
        "fixture": "benchmarks/agentic/agent-funnel/input",
        "revision": git_revision(),
        "runner_class": runner_class(),
        "analyzer_version": analyzer_version(),
        "checks": checks,
        "error": error,
        "claim_boundary": "Named-fixture static funnel behavior only; this does not claim runtime mutation behavior, coverage adequacy, or universal latency."
    });
    let text = serde_json::to_string_pretty(&receipt)
        .map_err(|error| format!("serialize agent-funnel receipt: {error}"))?;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/ripr/reports/agent-funnel-benchmark.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    std::fs::write(&path, format!("{text}\n"))
        .map_err(|error| format!("write {}: {error}", path.display()))?;
    Ok(path)
}

/// B1 happy path: the printed funnel executes from a foreign directory,
/// both writes land on disk, the receipt binds the fresh bytes, and
/// `agent status` re-reads the persisted chain as present and current.
#[test]
fn b1_agent_funnel_happy_path_binds_receipt_to_fresh_verify() -> Result<(), String> {
    let Some(bash) = shell_prerequisite()? else {
        return Ok(());
    };
    let (journey, report) = start_journey("agent-funnel-b1", &bash)?;
    let _owned = Fixture(journey.root.clone());
    let mut checks = Vec::new();
    let outcome: Result<(), String> = (|| {
        for field in ["after_snapshot", "analysis_outcome", "verify"] {
            run_printed_step(&journey, &report, field)?;
        }
        let receipt = run_printed_receipt_stdout(&journey, &report)?;
        checks.push(benchmark_check(
            "printed_funnel_executes",
            "after_snapshot, analysis_outcome, verify, and receipt ran byte for byte from a foreign cwd",
        ));
        assert_funnel_writes_bind_fresh_artifacts(&journey, &receipt)?;
        checks.push(benchmark_check(
            "receipt_binds_fresh_bytes",
            "agent-verify.json and analysis-outcome.json exist; provenance sha256 equals the bytes on disk",
        ));

        let status = agent_status_json(&journey)?;
        if artifact_present(&status, "agent_receipt")? {
            return Err(
                "agent status read a receipt artifact although nothing persisted it".to_string(),
            );
        }
        checks.push(benchmark_check(
            "status_requires_persisted_receipt",
            "agent status reads the receipt as missing before the --out completion",
        ));
        persist_receipt_for_status(&journey)?;
        checks.push(benchmark_check(
            "receipt_persisted",
            "agent receipt --out wrote target/ripr/reports/agent-receipt.json",
        ));
        let status = agent_status_json(&journey)?;
        for name in [
            "before_snapshot",
            "after_snapshot",
            "analysis_outcome",
            "agent_verify",
            "agent_receipt",
        ] {
            if !artifact_present(&status, name)? {
                return Err(format!(
                    "agent status reads {name} as missing after the funnel"
                ));
            }
        }
        let stale = stale_warning_messages(&status);
        if !stale.is_empty() {
            return Err(format!(
                "agent status reported stale artifacts for a fresh funnel chain: {stale:?}"
            ));
        }
        checks.push(benchmark_check(
            "status_present_and_current",
            "agent status re-reads all five artifacts as present with no stale warnings",
        ));
        Ok(())
    })();
    let failed = outcome.as_ref().err();
    let receipt_path = write_benchmark_receipt(
        if failed.is_none() { "pass" } else { "fail" },
        &checks,
        failed.map(String::as_str),
    )?;
    println!(
        "B1 agent-funnel: {} checks passed, receipt at {}",
        checks.len(),
        receipt_path.display()
    );
    outcome
}

/// Anti-gaming twin 1: the printed verify redirect is mutated away, so the
/// verify document goes to stdout only. The command still exits 0 with a
/// real document on stdout (both asserted as controls), but the oracle must
/// FAIL: no `agent-verify.json` reaches the disk, and the receipt step must
/// refuse the missing input. Exit-status grading would pass this twin.
#[test]
fn b1_anti_gaming_stdout_only_verify_fails_the_oracle() -> Result<(), String> {
    let Some(bash) = shell_prerequisite()? else {
        return Ok(());
    };
    let (journey, report) = start_journey("agent-funnel-stdout", &bash)?;
    let _owned = Fixture(journey.root.clone());

    for field in ["after_snapshot", "analysis_outcome"] {
        run_printed_step(&journey, &report, field)?;
    }
    let printed = printed_command(&report, "verify")?;
    let mutated = strip_shell_redirect(&printed)?;
    if !mutated.contains("agent verify") || mutated == printed {
        return Err("the redirect mutation mangled the verify command".to_string());
    }
    let output = run_in_shell(&journey, &mutated)?;
    assert_success(&output, "mutated stdout-only verify (control: it must run)")?;
    let stdout_doc: Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        format!("mutated verify stdout is not a JSON document (control): {error}")
    })?;
    if stdout_doc
        .pointer("/artifact_currentness")
        .and_then(Value::as_str)
        .is_none()
    {
        return Err("mutated verify stdout is not a real agent-verify document".to_string());
    }
    let verify_path = workflow_artifact(&journey.root, "agent-verify.json");
    if verify_path.exists() {
        return Err("the stdout-only verify still wrote agent-verify.json".to_string());
    }

    let receipt_command = printed_command(&report, "receipt")?;
    let receipt_output = run_in_shell(&journey, &receipt_command)?;
    if receipt_output.status.success() {
        return Err(format!(
            "the receipt step accepted a missing agent-verify.json:\n{}",
            String::from_utf8_lossy(&receipt_output.stdout)
        ));
    }
    println!("B1 anti-gaming twin 1: stdout-only verify failed the oracle as required");
    Ok(())
}

/// Anti-gaming twin 2: a fabricated `agent-verify.json` is planted before
/// the funnel runs. The funnel must overwrite it — raw byte inequality —
/// and the receipt must bind the fresh bytes, never the decoy digest.
#[test]
fn b1_anti_gaming_stale_verify_bytes_are_rejected() -> Result<(), String> {
    let Some(bash) = shell_prerequisite()? else {
        return Ok(());
    };
    let (journey, report) = start_journey("agent-funnel-stale", &bash)?;
    let _owned = Fixture(journey.root.clone());

    let verify_path = workflow_artifact(&journey.root, "agent-verify.json");
    let decoy = serde_json::json!({
        "schema_version": "0.3",
        "tool": "ripr",
        "status": "advisory",
        "inputs": {
            "before": "target/ripr/workflow/before.repo-exposure.json",
            "after": "target/ripr/workflow/after.repo-exposure.json"
        },
        "summary": {"improved": 1, "changed": 0, "regressed": 0, "unchanged": 0, "new": 0, "resolved": 0},
        "changed_seams": [{"seam_id": "seam-b1-decoy", "seam_kind": "predicate_boundary", "file": "src/lib.rs", "line": 42, "before": "weakly_gripped", "after": "strongly_gripped", "change": "improved", "evidence_delta": []}],
        "unchanged_seams": [],
        "new_gaps": [],
        "resolved_gaps": []
    });
    let decoy_bytes = serde_json::to_vec_pretty(&decoy).map_err(|error| error.to_string())?;
    std::fs::write(&verify_path, &decoy_bytes)
        .map_err(|error| format!("plant decoy verify: {error}"))?;
    let decoy_digest = sha256_bytes(&decoy_bytes);

    for field in ["after_snapshot", "analysis_outcome", "verify"] {
        run_printed_step(&journey, &report, field)?;
    }
    let receipt = run_printed_receipt_stdout(&journey, &report)?;

    let fresh_bytes = read_bytes(&verify_path)?;
    if fresh_bytes == decoy_bytes {
        return Err("the planted decoy verify bytes persisted through the funnel".to_string());
    }
    let fresh: Value = serde_json::from_slice(&fresh_bytes)
        .map_err(|error| format!("fresh verify bytes are not JSON: {error}"))?;
    if fresh
        .pointer("/artifact_currentness")
        .and_then(Value::as_str)
        .is_none()
    {
        return Err(format!(
            "the verify bytes after the funnel are not a real agent-verify document:\n{fresh}"
        ));
    }
    assert_funnel_writes_bind_fresh_artifacts(&journey, &receipt)?;
    let recorded = receipt
        .pointer("/provenance/verify_artifact/sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "receipt provenance is missing verify_artifact.sha256".to_string())?;
    if recorded == decoy_digest {
        return Err("the receipt bound the stale decoy verify bytes".to_string());
    }
    println!("B1 anti-gaming twin 2: stale verify bytes rejected, fresh bytes bound");
    Ok(())
}
