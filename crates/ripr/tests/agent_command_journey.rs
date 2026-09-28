//! Executable producer-consumer journey for the `ripr first-action` agent
//! funnel (#4307).
//!
//! The #4306 tests pin the *rendered* command strings. This harness executes
//! the printed commands literally: it runs `ripr first-action` against a real
//! fixture repository, then takes the exact strings from
//! `commands.after_snapshot`, `commands.analysis_outcome`, `commands.verify`,
//! and `commands.receipt` and runs each one through a shell from a foreign
//! working directory (launch directory != workspace root), the way a consuming
//! agent pastes them. The `>` redirect in the printed commands is shell
//! syntax, so a shell is the honest executor; the printed values are
//! POSIX-single-quoted by the command renderer, so the shell is bash
//! (Git Bash supplies it on Windows).
//!
//! Each printed string reaches bash byte for byte: it is written to a script
//! file and the file is executed, not passed through `bash -c`. On Windows
//! the `-c` route sends the text through Rust's argv joining and then MSYS's
//! re-parsing, which rewrites the quoting before bash ever sees it (the same
//! discovery the `shell_arg` round-trip tests in `loop_commands.rs` made), so
//! the file route is the only one that tests the funnel rather than the host.
//! The bash candidate is confirmed by a probe run from the real filesystem
//! because a bare `bash.exe` on PATH is frequently WSL bash, which cannot open
//! drive-letter host paths.
//!
//! Assertions are on-disk and cross-process, never string-shaped: the verify
//! redirect must land `target/ripr/workflow/agent-verify.json`, the analysis
//! outcome must land beside it, the receipt must bind the fresh verify bytes
//! through its `provenance.*.sha256` commitments, and `ripr agent status`
//! must read the persisted receipt back. Reverting the #4306 verify redirect
//! (printing verify instead of writing `agent-verify.json`) fails these tests:
//! the file never appears, and the receipt refuses the missing or stale input.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "common/mod.rs"]
mod common;

use common::fixture_git::fixture_git_ok;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// The single process spawn point for direct (shell-free) ripr invocations.
/// The journey's printed commands run through [`run_in_shell`] instead; this
/// helper is for the surfaces that write their own outputs (`first-action`,
/// `agent status`, and fixture-setup snapshot production, which redirects the
/// child stdout into a file the way the shell redirect would).
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

/// The funnel commands are consumed from a foreign working directory (launch
/// directory != workspace root), so the harness keeps one next to every
/// fixture root. Spawning there is what makes the #3872 anchored redirect
/// targets load-bearing: a relative redirect would write into the launch
/// directory and the journey would catch it.
fn foreign_launch_dir(base: &Path) -> Result<PathBuf, String> {
    let dir = base.join("foreign-launch");
    std::fs::create_dir_all(&dir).map_err(|error| format!("create {}: {error}", dir.display()))?;
    Ok(dir)
}

fn write_fixture_source(root: &Path, relative: &str) -> Result<(), String> {
    let fixture_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/boundary_gap/input");
    let destination = root.join(relative);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    std::fs::copy(fixture_root.join(relative), &destination)
        .map_err(|error| format!("copy fixture {relative} into {}: {error}", root.display()))?;
    Ok(())
}

/// A minimal Cargo package with a real analysis seam, committed to Git so the
/// repo-exposure artifacts carry a concrete repository identity.
fn init_producer_fixture_repo(root: &Path) -> Result<(), String> {
    for relative in ["Cargo.toml", "src/lib.rs", "tests/pricing.rs"] {
        write_fixture_source(root, relative)?;
    }
    fixture_git_ok(root, &["init"]).map_err(|error| format!("fixture git init: {error}"))?;
    // Keep checkouts byte-identical to what was committed (same rule as the
    // cli_smoke producer fixture): a autocrlf rewrite would move artifact
    // bytes between snapshot runs on Windows.
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

/// Repository movement for `agent verify` (#2922 PR A refuses a same-head
/// current pair): the after snapshot is taken on a newer empty commit, so the
/// pair is historical-before/current-after instead of refused.
fn advance_fixture_head(root: &Path, message: &str) -> Result<(), String> {
    commit_fixture(root, message)
}

/// The seam the journey routes on, discovered from the produced before
/// snapshot rather than invented: the receipt refuses a `--seam-id` that the
/// verify output does not carry, so the funnel needs a real analyzer seam.
fn first_snapshot_seam_id(snapshot_path: &Path) -> Result<String, String> {
    let snapshot = read_json(snapshot_path)?;
    snapshot
        .pointer("/seams/0/seam_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            format!(
                "fixture snapshot {} carries no seams; the funnel journey needs a real seam",
                snapshot_path.display()
            )
        })
}

fn write_assistant_proof(path: &Path, seam_id: &str) -> Result<(), String> {
    // The minimal joined assistant-proof projection first-action selects on:
    // a `seam` object. Everything else (classification, discriminator) rides
    // along so the selected action mirrors a real proof report.
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

/// Fixture-setup snapshot production: the same redirect the workflow's
/// `ripr check … repo-exposure-json` command performs, done process-level so
/// setup does not need a shell. The journey's own snapshot step is still
/// executed as a printed shell command.
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
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("parse {} as JSON: {error}", path.display()))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("read {} for digest: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    Ok(format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

/// Render a path the way the shell under test expects to receive it: a
/// Windows path handed to Git Bash loses its backslashes to MSYS argument
/// processing, so the directory separators are spelled forward everywhere.
fn shell_display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// Find a bash that can actually execute a script at a host path. Probing
/// with `bash -c "true"` is not enough on Windows: `bash.exe` on `PATH` is
/// frequently WSL bash, which runs fine but cannot open a drive-letter host
/// path, so every journey step would fail for a reason that has nothing to
/// do with the funnel. Git Bash is preferred and the candidate is confirmed
/// by running a probe script from the real filesystem. Mirrors the discovery
/// the `shell_arg` round-trip tests in `loop_commands.rs` use.
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
    let probe_dir = unique_temp_workspace("journey-shell-probe");
    std::fs::create_dir_all(&probe_dir).ok()?;
    let probe_script = probe_dir.join("probe.sh");
    if std::fs::write(&probe_script, b"printf 'ok\\n'\n").is_err() {
        cleanup(&probe_dir);
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

/// `Ok(Some(bash))` when the journey may run, `Ok(None)` when no usable POSIX
/// shell exists on a local machine (the test skips after printing the
/// notice), and `Err` when it is absent under GitHub Actions, where the
/// strongest executable oracle for the printed funnel must not silently
/// vanish.
fn shell_prerequisite() -> Result<Option<PathBuf>, String> {
    if let Some(bash) = discover_posix_shell() {
        return Ok(Some(bash));
    }
    if std::env::var_os("GITHUB_ACTIONS").is_some() {
        return Err(
            "bash is not usable under GitHub Actions; the command-journey funnel cannot be skipped in CI"
                .to_string(),
        );
    }
    eprintln!(
        "SKIPPED agent_command_journey: no usable `bash` (Git Bash on Windows) was found; the printed funnel commands need a POSIX shell"
    );
    Ok(None)
}

/// The `PATH` handed to the shell: the just-built binary's directory first, so
/// the literal `ripr …` tokens in the printed commands resolve to the binary
/// under test, not to an installed release.
fn shell_path_env() -> Result<std::ffi::OsString, String> {
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_ripr"))
        .parent()
        .ok_or_else(|| "ripr binary path has no parent directory".to_string())?
        .to_path_buf();
    let existing = std::env::var_os("PATH").ok_or_else(|| "PATH is not set".to_string())?;
    std::env::join_paths(std::iter::once(bin_dir).chain(std::env::split_paths(&existing)))
        .map_err(|error| format!("join PATH entries: {error}"))
}

/// Execute one printed command string literally: the exact bytes from the
/// first-action JSON are written to a script file in the launch directory and
/// that file is handed to the confirmed bash, launched from the foreign
/// working directory. Nothing re-derives, reformats, or re-quotes the string.
fn run_in_shell(journey: &Journey, command: &str) -> Result<Output, String> {
    let script = journey.launch_dir.join(format!(
        "journey-{}.sh",
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

/// One journey workspace: a real Git fixture with a before snapshot on the
/// base commit, an empty movement commit on top, and the artifacts the funnel
/// chain reads. `root_arg` is the exact `--root` spelling every command
/// receives, so producer and consumer agree on the recorded root identity.
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

/// Set up the shared fixture and run `ripr first-action` from the foreign
/// launch directory, returning the journey plus the printed funnel commands.
fn start_journey(label: &str, bash: &Path) -> Result<(Journey, Value), String> {
    let root = unique_temp_workspace(label);
    let result = start_journey_at_root(&root, bash);
    if result.is_err() {
        cleanup(&root);
    }
    result
}

fn start_journey_at_root(root: &Path, bash: &Path) -> Result<(Journey, Value), String> {
    std::fs::create_dir_all(root).map_err(|error| format!("create {}: {error}", root.display()))?;
    init_producer_fixture_repo(root)?;

    // The before snapshot is older workflow evidence (produced on the base
    // commit, the way the loop's snapshot step leaves it); the movement commit
    // gives the after snapshot a real head to move to.
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

    // The producer surface itself: first-action writes its JSON report; the
    // journey reads the printed commands from that report file, not from a
    // re-derivation.
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

fn root_arg_of(root: &Path) -> String {
    root.display().to_string()
}

/// Extract one printed command from the first-action report. The string is
/// executed verbatim afterwards; nothing re-derives or reformats it.
fn printed_command(report: &Value, field: &str) -> Result<String, String> {
    report
        .pointer(&format!("/commands/{field}"))
        .and_then(Value::as_str)
        .filter(|command| !command.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("first-action report carries no /commands/{field}:\n{report}"))
}

/// Run the funnel's printed snapshot/verify/receipt commands in order, from
/// the foreign launch directory, through the shell. Returns the receipt the
/// printed receipt command emitted on stdout.
fn execute_funnel(journey: &Journey, report: &Value) -> Result<Value, String> {
    execute_funnel_with_receipt_file(journey, report, None)
}

fn execute_funnel_with_receipt_file(
    journey: &Journey,
    report: &Value,
    receipt_file: Option<&Path>,
) -> Result<Value, String> {
    execute_receipt_steps(
        journey,
        report,
        receipt_file,
        &["after_snapshot", "analysis_outcome", "verify"],
    )
}

fn execute_receipt_steps(
    journey: &Journey,
    report: &Value,
    receipt_file: Option<&Path>,
    steps: &[&str],
) -> Result<Value, String> {
    for field in steps {
        let command = printed_command(report, field)?;
        let output = run_in_shell(journey, &command)?;
        assert_success(
            &output,
            &format!("printed {field} command through a shell: {command}"),
        )?;
    }
    let receipt_command = printed_command(report, "receipt")?;
    let output = run_in_shell(journey, &receipt_command)?;
    assert_success(
        &output,
        &format!("printed receipt command through a shell: {receipt_command}"),
    )?;
    let receipt_text = match receipt_file {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|error| format!("read advertised receipt {}: {error}", path.display()))?,
        None => String::from_utf8(output.stdout.clone())
            .map_err(|error| format!("receipt stdout is not UTF-8: {error}"))?,
    };
    serde_json::from_str(&receipt_text).map_err(|error| {
        format!("receipt output is not one JSON document: {error}\n{receipt_text}")
    })
}

/// The producer-consumer bindings a completed funnel must show: the verify
/// redirect landed, the analysis outcome landed beside it, and the receipt
/// binds the exact fresh verify/before/after bytes and carries the fresh
/// producer-owned analysis outcome.
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

    // The receipt is a fresh producer reading the fresh verify file: its
    // provenance must commit to the exact bytes now on disk.
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
                "receipt {artifact} digest {recorded} does not bind the fresh bytes on disk ({digest})"
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
    // The funnel writes the analysis outcome beside the verify file (#4304);
    // a receipt over a present, complete outcome is the only reading that
    // carries completeness evidence.
    if receipt
        .pointer("/analysis_outcome_status")
        .and_then(Value::as_str)
        != Some("complete")
    {
        return Err(format!(
            "receipt did not read the funnel-written analysis outcome as complete:\n{receipt}"
        ));
    }
    let analysis_text = std::fs::read_to_string(&analysis_path)
        .map_err(|error| format!("read fresh analysis outcome: {error}"))?;
    let analysis: Value = serde_json::from_str(&analysis_text)
        .map_err(|error| format!("parse fresh analysis outcome: {error}"))?;
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
    let text = String::from_utf8(output.stdout.clone())
        .map_err(|error| format!("agent status stdout is not UTF-8: {error}"))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("agent status stdout is not JSON: {error}\n{text}"))
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

/// The canonical completion of the receipt step for `agent status`: the same
/// funnel receipt command with the workflow receipt path as `--out`, which is
/// exactly the command `agent status` itself prescribes for a missing receipt
/// artifact. The typed receipt route is Direct (no redirect), so this runs
/// process-level.
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

fn cleanup(root: &Path) {
    let _ = std::fs::remove_dir_all(root);
}

struct ReviewCardFixture(PathBuf);

impl Drop for ReviewCardFixture {
    fn drop(&mut self) {
        cleanup(&self.0);
    }
}

/// Execute commands obtained from the actual review-card and inherited gate
/// producers, with fresh artifacts and a foreign launch directory.
#[test]
fn review_card_and_gate_commands_persist_fresh_receipt_inputs() -> Result<(), String> {
    let Some(bash) = shell_prerequisite()? else {
        return Ok(());
    };
    let root = unique_temp_workspace("review-card-café space");
    std::fs::create_dir(&root).map_err(|error| format!("claim fixture root: {error}"))?;
    let fixture = ReviewCardFixture(root);
    let (mut journey, setup) = start_journey_at_root(&fixture.0, &bash)?;
    let source = journey.root.join("src/lib.rs");
    let old = std::fs::read_to_string(&source).map_err(|error| error.to_string())?;
    let changed = old.replace(
        "amount >= discount_threshold",
        "amount > discount_threshold",
    );
    if changed == old {
        return Err("fixture change did not alter its predicate".to_string());
    }
    std::fs::write(&source, changed).map_err(|error| error.to_string())?;
    fixture_git_ok(&journey.root, &["add", "src/lib.rs"]).map_err(|error| error.to_string())?;
    commit_fixture(&journey.root, "change the boundary predicate")?;
    let snapshot = run_in_shell(&journey, &printed_command(&setup, "after_snapshot")?)?;
    assert_success(&snapshot, "prepare current after snapshot")?;
    // The card selects HEAD~1 explicitly. Remove the fixture's default-name
    // branch only after preparing its prerequisite snapshots: card/gate steps
    // must keep that selection rather than rediscover main or master.
    fixture_git_ok(&journey.root, &["branch", "-M", "trunk"]).map_err(|error| error.to_string())?;

    let comments_path = journey.root.join("target/ripr/review/comments.json");
    let comments_output = run_ripr(
        &journey.launch_dir,
        &[
            "review-comments",
            "--root",
            &journey.root_arg,
            "--base",
            "HEAD~1",
            "--head",
            "HEAD",
            "--out",
            &comments_path.display().to_string(),
        ],
    )?;
    assert_success(&comments_output, "actual review-comments producer")?;
    let comments = read_json(&comments_path)?;
    let card = comments
        .get("comments")
        .and_then(Value::as_array)
        .and_then(|cards| {
            cards
                .iter()
                .find(|card| card.get("gap_state").and_then(Value::as_str) == Some("actionable"))
        })
        .ok_or_else(|| format!("producer emitted no actionable review card: {comments}"))?;
    journey.seam_id = card
        .get("seam_id")
        .and_then(Value::as_str)
        .ok_or("actionable card has no seam identity")?
        .to_string();
    for name in ["before.repo-exposure.json", "after.repo-exposure.json"] {
        let snapshot = read_json(&workflow_artifact(&journey.root, name))?;
        let contains_selected =
            snapshot
                .get("seams")
                .and_then(Value::as_array)
                .is_some_and(|seams| {
                    seams.iter().any(|seam| {
                        seam.get("seam_id").and_then(Value::as_str)
                            == Some(journey.seam_id.as_str())
                    })
                });
        if !contains_selected {
            return Err(format!("selected review-card seam is absent from {name}"));
        }
    }

    let gate_path = journey.root.join("target/ripr/reports/gate-decision.json");
    let gate_output = run_ripr(
        &journey.launch_dir,
        &[
            "gate",
            "evaluate",
            "--root",
            &journey.root_arg,
            "--pr-guidance",
            &comments_path.display().to_string(),
            "--mode",
            "visible-only",
            "--out",
            &gate_path.display().to_string(),
        ],
    )?;
    assert_success(&gate_output, "actual inherited gate producer")?;
    let gate = read_json(&gate_path)?;
    let route = gate
        .get("decisions")
        .and_then(Value::as_array)
        .and_then(|decisions| {
            decisions.iter().find_map(|decision| {
                let route = decision.get("repair_route")?;
                (route.get("seam_id").and_then(Value::as_str) == Some(journey.seam_id.as_str()))
                    .then_some(route)
            })
        })
        .ok_or_else(|| format!("gate lost the actionable card route: {gate}"))?;
    for (label, guidance, receipt) in [
        (
            "review card",
            card.get("llm_guidance").ok_or("missing guidance")?,
            card.get("receipt_command"),
        ),
        ("gate route", route, route.get("receipt_command")),
    ] {
        let verify_path = workflow_artifact(&journey.root, "agent-verify.json");
        std::fs::write(&verify_path, "{\"decoy\":true}").map_err(|error| error.to_string())?;
        let decoy_digest = sha256_file(&verify_path)?;
        let analysis_path = workflow_artifact(&journey.root, "analysis-outcome.json");
        std::fs::write(&analysis_path, "{\"decoy\":true}").map_err(|error| error.to_string())?;
        let analysis_decoy_digest = sha256_file(&analysis_path)?;
        let receipt_command = receipt
            .and_then(Value::as_str)
            .ok_or("missing receipt command")?;
        if !receipt_command.contains("--out target/ripr/reports/agent-receipt.json") {
            return Err(format!(
                "fixture must name its receipt output explicitly: {receipt_command}"
            ));
        }
        let receipt_path = journey.root.join("target/ripr/reports/agent-receipt.json");
        std::fs::write(&receipt_path, "{\"decoy\":true}").map_err(|error| error.to_string())?;
        let receipt_decoy_digest = sha256_file(&receipt_path)?;
        let funnel = serde_json::json!({"commands": {
            "analysis_outcome": guidance.get("analysis_outcome_command"),
            "verify": guidance.get("verify_command"),
            "receipt": receipt,
        }});
        // A complete outcome in another sibling must not rescue the invalid
        // canonical input. Keep the root and every other printed step intact.
        let outcome_command = printed_command(&funnel, "analysis_outcome")?;
        let canonical_target = "target/ripr/workflow/analysis-outcome.json";
        if outcome_command.matches(canonical_target).count() != 1 {
            return Err(format!(
                "{label} must name exactly one canonical outcome target"
            ));
        }
        let mut wrong_sibling = funnel.clone();
        wrong_sibling["commands"]["analysis_outcome"] =
            serde_json::json!(outcome_command.replacen(
                canonical_target,
                "target/ripr/workflow/wrong-analysis-outcome.json",
                1,
            ));
        std::fs::write(
            workflow_artifact(&journey.root, "wrong-analysis-outcome.json"),
            "{\"decoy\":true}",
        )
        .map_err(|error| error.to_string())?;
        let refused = execute_receipt_steps(
            &journey,
            &wrong_sibling,
            Some(&receipt_path),
            &["analysis_outcome", "verify"],
        )?;
        let wrong_outcome = read_json(&workflow_artifact(
            &journey.root,
            "wrong-analysis-outcome.json",
        ))?;
        if wrong_outcome
            .pointer("/analysis_outcome/analysis_complete")
            .and_then(Value::as_bool)
            != Some(true)
            || sha256_file(&analysis_path)? != analysis_decoy_digest
            || sha256_file(&verify_path)? == decoy_digest
            || sha256_file(&receipt_path)? == receipt_decoy_digest
        {
            return Err(format!(
                "{label} wrong-sibling control did not exercise fresh producers"
            ));
        }
        if refused.get("status").and_then(Value::as_str) != Some("invalid")
            || refused
                .get("analysis_outcome_status")
                .and_then(Value::as_str)
                != Some("invalid")
            || refused.get("analysis_outcome") != Some(&Value::Null)
            || refused
                .get("analysis_outcome_error")
                .and_then(Value::as_str)
                .is_none_or(|error| error.is_empty())
        {
            return Err(format!(
                "{label} accepted an outcome written to the wrong sibling: {refused}"
            ));
        }
        std::fs::write(&verify_path, "{\"decoy\":true}").map_err(|error| error.to_string())?;
        std::fs::write(&receipt_path, "{\"decoy\":true}").map_err(|error| error.to_string())?;
        let receipt = execute_receipt_steps(
            &journey,
            &funnel,
            Some(&receipt_path),
            &["analysis_outcome", "verify"],
        )
        .map_err(|error| format!("{label}: {error}"))?;
        assert_funnel_writes_bind_fresh_artifacts(&journey, &receipt)?;
        if sha256_file(&verify_path)? == decoy_digest {
            return Err(format!("{label} did not replace the decoy verify"));
        }
        if sha256_file(&analysis_path)? == analysis_decoy_digest {
            return Err(format!(
                "{label} did not replace the decoy analysis outcome"
            ));
        }
        if sha256_file(&receipt_path)? == receipt_decoy_digest {
            return Err(format!("{label} did not replace the advertised receipt"));
        }
    }
    Ok(())
}

/// The happy producer-consumer journey: first-action prints the funnel, the
/// printed commands execute literally from a foreign directory, every claimed
/// write exists, the receipt binds the fresh verify bytes, and `agent status`
/// reads the chain back as present, current, and free of stale warnings.
#[test]
fn first_action_funnel_commands_execute_and_bind_the_receipt_to_fresh_verify() -> Result<(), String>
{
    let Some(bash) = shell_prerequisite()? else {
        return Ok(());
    };
    let (journey, report) = start_journey("journey-happy", &bash)?;
    let result = (|| -> Result<(), String> {
        let receipt = execute_funnel(&journey, &report)?;
        assert_funnel_writes_bind_fresh_artifacts(&journey, &receipt)?;

        // Before the receipt is persisted, `agent status` must read the
        // receipt artifact as missing: the printed funnel command emits the
        // receipt on stdout and does not claim a file write.
        let status = agent_status_json(&journey)?;
        if artifact_present(&status, "agent_receipt")? {
            return Err(
                "agent status read a receipt artifact although nothing persisted it".to_string(),
            );
        }
        persist_receipt_for_status(&journey)?;
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
        Ok(())
    })();
    cleanup(&journey.root);
    result
}

/// Hostile matrix (a): a fabricated `agent-verify.json` planted before the
/// funnel runs must be overwritten by the fresh verify redirect, and the
/// receipt must bind the fresh bytes, never the decoy.
#[test]
fn funnel_overwrites_a_planted_decoy_verify_and_binds_the_fresh_bytes() -> Result<(), String> {
    let Some(bash) = shell_prerequisite()? else {
        return Ok(());
    };
    let (journey, report) = start_journey("journey-decoy", &bash)?;
    let result = (|| -> Result<(), String> {
        let verify_path = workflow_artifact(&journey.root, "agent-verify.json");
        // A stale, fabricated verify document in the shape a confused or
        // hostile producer might leave behind: plausible envelope, wrong
        // content, and a seam id the fixture never produced.
        let decoy = serde_json::json!({
            "schema_version": "0.3",
            "tool": "ripr",
            "status": "advisory",
            "inputs": {
                "before": "target/ripr/workflow/before.repo-exposure.json",
                "after": "target/ripr/workflow/after.repo-exposure.json"
            },
            "summary": {"improved": 1, "changed": 0, "regressed": 0, "unchanged": 0, "new": 0, "resolved": 0},
            "changed_seams": [{"seam_id": "seam-a-decoy", "seam_kind": "predicate_boundary", "file": "src/pricing.rs", "line": 42, "before": "weakly_gripped", "after": "strongly_gripped", "change": "improved", "evidence_delta": []}],
            "unchanged_seams": [],
            "new_gaps": [],
            "resolved_gaps": []
        });
        std::fs::write(
            &verify_path,
            serde_json::to_string_pretty(&decoy).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("plant decoy verify: {error}"))?;
        let decoy_digest = sha256_file(&verify_path)?;

        let receipt = execute_funnel(&journey, &report)?;
        let fresh_text = std::fs::read_to_string(&verify_path)
            .map_err(|error| format!("read fresh verify: {error}"))?;
        if fresh_text.contains("seam-a-decoy") {
            return Err(
                "the planted decoy verify remained after the funnel verify redirect".to_string(),
            );
        }
        let fresh = read_json(&verify_path)?;
        if fresh
            .pointer("/artifact_currentness")
            .and_then(Value::as_str)
            .is_none()
        {
            return Err(format!(
                "the verify file after the funnel is not a real agent-verify document:\n{fresh}"
            ));
        }
        assert_funnel_writes_bind_fresh_artifacts(&journey, &receipt)?;
        let recorded = receipt
            .pointer("/provenance/verify_artifact/sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| "receipt provenance is missing verify_artifact.sha256".to_string())?;
        if recorded == decoy_digest.as_str() {
            return Err("the receipt bound the decoy verify bytes".to_string());
        }
        Ok(())
    })();
    cleanup(&journey.root);
    result
}

/// Hostile matrix (b): a receipt persisted by an older run, then a funnel that
/// refreshes `agent-verify.json` but does not touch the persisted receipt,
/// must leave `agent status` warning that the receipt is older than the verify
/// artifact (app/agent_status.rs `push_stale_warning`).
#[test]
fn agent_status_warns_receipt_stale_after_the_funnel_refreshes_verify() -> Result<(), String> {
    let Some(bash) = shell_prerequisite()? else {
        return Ok(());
    };
    let (journey, report) = start_journey("journey-stale", &bash)?;
    let result = (|| -> Result<(), String> {
        // The older run: verify and a persisted receipt over the same
        // snapshot pair, before the funnel refreshes the verify file. The
        // after snapshot exists from setup (produced process-level the same
        // way the before snapshot is), so the older verify has both inputs.
        produce_repo_exposure_snapshot(
            &journey.root,
            &journey.root_arg,
            &workflow_artifact(&journey.root, "after.repo-exposure.json"),
        )?;
        let verify_path = workflow_artifact(&journey.root, "agent-verify.json");
        let older_verify = run_ripr(
            &journey.launch_dir,
            &[
                "agent",
                "verify",
                "--root",
                &journey.root_arg,
                "--before",
                "target/ripr/workflow/before.repo-exposure.json",
                "--after",
                "target/ripr/workflow/after.repo-exposure.json",
                "--json",
            ],
        )?;
        assert_success(&older_verify, "older-run agent verify")?;
        std::fs::write(&verify_path, &older_verify.stdout)
            .map_err(|error| format!("persist older verify: {error}"))?;
        let receipt_path = journey.root.join("target/ripr/reports/agent-receipt.json");
        let older_receipt = run_ripr(
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
                &receipt_path.display().to_string(),
            ],
        )?;
        assert_success(&older_receipt, "older-run agent receipt --out")?;
        if !receipt_path.is_file() {
            return Err("the older run did not persist its receipt".to_string());
        }
        // Establish timestamp ordering explicitly rather than assuming the
        // filesystem can distinguish writes separated by a short sleep.
        let old_time = std::fs::metadata(&receipt_path)
            .and_then(|metadata| metadata.modified())
            .map_err(|error| format!("read receipt timestamp: {error}"))?
            .checked_sub(std::time::Duration::from_secs(10))
            .ok_or_else(|| "receipt timestamp cannot be moved earlier".to_string())?;
        std::fs::File::options()
            .write(true)
            .open(&receipt_path)
            .and_then(|file| file.set_times(std::fs::FileTimes::new().set_modified(old_time)))
            .map_err(|error| format!("set older receipt timestamp: {error}"))?;

        // The funnel refreshes the verify file; the persisted receipt file is
        // not one of its writes, so it now lags the verify artifact.
        let fresh_receipt = execute_funnel(&journey, &report)?;
        assert_funnel_writes_bind_fresh_artifacts(&journey, &fresh_receipt)?;

        let status = agent_status_json(&journey)?;
        let stale = stale_warning_messages(&status);
        let receipt_stale = stale
            .iter()
            .any(|message| message.contains("agent receipt is older than agent verify"));
        if !receipt_stale {
            return Err(format!(
                "agent status did not warn that the persisted receipt is older than the refreshed verify; warnings: {stale:?}"
            ));
        }
        // The verify artifact itself is fresh against both snapshots: the
        // stale warning must name the receipt, not the verify step.
        let verify_stale = stale
            .iter()
            .any(|message| message.contains("agent verify is older than"));
        if verify_stale {
            return Err(format!(
                "agent status flagged the fresh funnel verify as stale; warnings: {stale:?}"
            ));
        }
        Ok(())
    })();
    cleanup(&journey.root);
    result
}

/// Hostile matrix (c): with `target/ripr/workflow/` removed, the verify
/// redirect fails truthfully — non-zero exit, the error names the write path,
/// and no verify file silently appears. The test does not create the
/// directory around the failure: pinning the current behavior is the point.
#[test]
fn funnel_verify_redirect_fails_truthfully_when_the_workflow_dir_is_missing() -> Result<(), String>
{
    let Some(bash) = shell_prerequisite()? else {
        return Ok(());
    };
    let (journey, report) = start_journey("journey-missing-dir", &bash)?;
    let result = (|| -> Result<(), String> {
        std::fs::remove_dir_all(journey.root.join("target"))
            .map_err(|error| format!("remove target dir: {error}"))?;
        let command = printed_command(&report, "verify")?;
        let output = run_in_shell(&journey, &command)?;
        if output.status.success() {
            return Err(format!(
                "the verify redirect succeeded although {} does not exist",
                journey.root.join("target/ripr/workflow").display()
            ));
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("agent-verify.json") {
            return Err(format!(
                "the failed verify redirect did not name its write path; stderr:\n{stderr}"
            ));
        }
        if workflow_artifact(&journey.root, "agent-verify.json").exists() {
            return Err(
                "a missing workflow directory still produced agent-verify.json".to_string(),
            );
        }
        Ok(())
    })();
    cleanup(&journey.root);
    result
}

/// Hostile matrix (d): a workspace root carrying spaces, Unicode, and an
/// apostrophe. The printed commands single-quote every value, so the full
/// funnel — including the receipt binding — must survive the quoting.
#[test]
fn hostile_root_with_spaces_unicode_and_apostrophe_runs_the_full_funnel() -> Result<(), String> {
    let Some(bash) = shell_prerequisite()? else {
        return Ok(());
    };
    let hostile = "ripr jöurney 'quoted root' café";
    let base = unique_temp_workspace("journey-hostile-base");
    std::fs::create_dir_all(&base).map_err(|error| format!("create base: {error}"))?;
    let root = base.join(hostile);
    // start_journey owns everything except the root name, so route through it
    // by building the same fixture under the hostile path.
    let result = (|| {
        std::fs::create_dir_all(&root).map_err(|error| format!("create hostile root: {error}"))?;
        run_funnel_in_hostile_root(&base, &root, &bash)
    })();
    cleanup(&base);
    result
}

fn run_funnel_in_hostile_root(base: &Path, root: &Path, bash: &Path) -> Result<(), String> {
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
        launch_dir: foreign_launch_dir(base)?,
        path_env: shell_path_env()?,
        bash: bash.to_path_buf(),
        seam_id,
        root: root.to_path_buf(),
    };

    let report_path = root.join("target/ripr/reports/first-useful-action.json");
    let report_path_md = root.join("target/ripr/reports/first-useful-action.md");
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
    assert_success(&output, "ripr first-action in the hostile root")?;
    let report = read_json(&report_path)?;
    if report.pointer("/status").and_then(Value::as_str) != Some("actionable") {
        return Err(format!(
            "first-action did not route to an actionable report in the hostile root:\n{report}"
        ));
    }
    let receipt = execute_funnel(&journey, &report)?;
    assert_funnel_writes_bind_fresh_artifacts(&journey, &receipt)?;
    persist_receipt_for_status(&journey)?;
    let status = agent_status_json(&journey)?;
    if !artifact_present(&status, "agent_receipt")? {
        return Err("agent status lost the hostile-root receipt".to_string());
    }
    let stale = stale_warning_messages(&status);
    if !stale.is_empty() {
        return Err(format!(
            "agent status reported stale artifacts in the hostile root: {stale:?}"
        ));
    }
    Ok(())
}
