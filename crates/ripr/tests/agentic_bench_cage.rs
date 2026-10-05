//! Benchmark B6: production-routed edit-cage (`benchmarks/agentic/edit-cage`).
//!
//! The harness runs `ripr agent repair --phase before/after` through the
//! worktree-built binary, the B1 pattern. `crates/ripr/src/edit_cage` stays
//! `pub(crate)`; this suite never names it and never reimplements production
//! matching, digest, or capture. Oracles are CLI exit, stderr/stdout, the
//! attempt manifest the command already wrote, the typed attempt-status
//! document, and byte-identity of a retained attempt record across a
//! superseding finish.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "common/mod.rs"]
mod common;

use common::fixture_git::fixture_git_ok;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Production `edit_cage::MAX_CAPTURE_FILE_BYTES`. Expected-value pin of the
/// published constant so an over-budget file exercises the real capture path;
/// this suite does not reimplement capture.
const PRODUCTION_MAX_CAPTURE_FILE_BYTES: u64 = 16 * 1024 * 1024;

const IN_SURFACE_TEST: &str = r#"
#[test]
fn equality_boundary_discounts() {
    assert_eq!(discounted_total(100, 100), 90);
}
"#;

fn run_ripr(current_dir: &Path, args: &[&str]) -> Result<Output, String> {
    Command::new(env!("CARGO_BIN_EXE_ripr"))
        .current_dir(current_dir)
        .args(args)
        .output()
        .map_err(|error| format!("spawn ripr {args:?} in {}: {error}", current_dir.display()))
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

fn fixture_input() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks/agentic/edit-cage/input")
}

fn copy_fixture_file(root: &Path, relative: &str) -> Result<(), String> {
    let destination = root.join(relative);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::copy(fixture_input().join(relative), &destination)
        .map_err(|error| format!("copy fixture {relative} into {}: {error}", root.display()))?;
    Ok(())
}

fn init_fixture_repo(root: &Path) -> Result<(), String> {
    for relative in ["Cargo.toml", "src/lib.rs", "tests/pricing.rs"] {
        copy_fixture_file(root, relative)?;
    }
    fs::write(root.join(".gitignore"), "/target/\n")
        .map_err(|error| format!("write .gitignore: {error}"))?;
    fixture_git_ok(root, &["init"]).map_err(|error| format!("fixture git init: {error}"))?;
    fixture_git_ok(root, &["config", "core.autocrlf", "false"])
        .map_err(|error| format!("fixture git config autocrlf: {error}"))?;
    fixture_git_ok(
        root,
        &[
            "add",
            "Cargo.toml",
            "src/lib.rs",
            "tests/pricing.rs",
            ".gitignore",
        ],
    )
    .map_err(|error| format!("fixture git add: {error}"))?;
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
            "fixture",
        ],
    )
    .map_err(|error| format!("fixture commit in {}: {error}", root.display()))
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {} as JSON: {error}", path.display()))
}

fn parse_stdout_json(output: &Output) -> Result<Value, String> {
    serde_json::from_slice(&output.stdout).map_err(|error| {
        format!(
            "stdout is not JSON: {error}\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn discover_seam_id(root: &Path, root_arg: &str) -> Result<String, String> {
    let output = run_ripr(
        root,
        &[
            "check",
            "--root",
            root_arg,
            "--mode",
            "draft",
            "--format",
            "repo-exposure-json",
        ],
    )?;
    if !output.status.success() {
        return Err(format!(
            "setup repo-exposure failed with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let snapshot: Value = parse_stdout_json(&output)?;
    snapshot
        .pointer("/seams/0/seam_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            format!("fixture snapshot carries no seams; B6 needs a real seam:\n{snapshot}")
        })
}

struct Journey {
    root: PathBuf,
    root_arg: String,
    seam_id: String,
    attempt_id: String,
}

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn add_in_surface_test(root: &Path) -> Result<(), String> {
    let path = root.join("tests/pricing.rs");
    let mut tests =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    tests.push_str(IN_SURFACE_TEST);
    fs::write(&path, tests).map_err(|error| format!("write {}: {error}", path.display()))
}

fn edit_out_of_surface(root: &Path) -> Result<(), String> {
    let path = root.join("src/lib.rs");
    let mut source =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    source.push_str("\n// out-of-surface production edit\n");
    fs::write(&path, source).map_err(|error| format!("write {}: {error}", path.display()))
}

fn plant_over_budget_file(root: &Path) -> Result<(), String> {
    let path = root.join("oversized.bin");
    let file =
        fs::File::create(&path).map_err(|error| format!("create {}: {error}", path.display()))?;
    file.set_len(PRODUCTION_MAX_CAPTURE_FILE_BYTES.saturating_add(1))
        .map_err(|error| format!("size {}: {error}", path.display()))
}

fn attempt_dir(root: &Path, attempt_id: &str) -> PathBuf {
    root.join("target/ripr/repair-attempts").join(attempt_id)
}

fn attempt_manifest(root: &Path, attempt_id: &str) -> Result<Value, String> {
    read_json(&attempt_dir(root, attempt_id).join("attempt.json"))
}

fn artifact_path(root: &Path, manifest: &Value, role: &str) -> Result<PathBuf, String> {
    let artifacts = manifest
        .pointer("/artifacts")
        .and_then(Value::as_array)
        .ok_or_else(|| "attempt manifest carries no artifacts array".to_string())?;
    let entry = artifacts
        .iter()
        .find(|artifact| artifact.get("role").and_then(Value::as_str) == Some(role))
        .ok_or_else(|| format!("attempt manifest has no {role} artifact"))?;
    let relative = entry
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{role} artifact has no path"))?;
    Ok(root.join(relative))
}

fn compatibility_receipt_path(root: &Path) -> PathBuf {
    root.join("target/ripr/reports/agent-receipt.json")
}

fn attempt_local_receipt_path(root: &Path, attempt_id: &str) -> PathBuf {
    attempt_dir(root, attempt_id)
        .join("artifacts")
        .join("agent-receipt.json")
}

/// Fail-closed cases must not write the compatibility projection or retain an
/// attempt-local copy. Production keeps the global file as a one-slot
/// projection (`target/ripr/reports/agent-receipt.json`) and copies a
/// successful receipt beside the attempt; this suite only checks those
/// production locations.
fn assert_no_receipt_written(root: &Path, attempt_id: &str, context: &str) -> Result<(), String> {
    let global = compatibility_receipt_path(root);
    if global.is_file() {
        return Err(format!(
            "{context}: wrote the compatibility receipt at {}",
            global.display()
        ));
    }
    let local = attempt_local_receipt_path(root, attempt_id);
    if local.is_file() {
        return Err(format!(
            "{context}: retained an attempt-local receipt at {}",
            local.display()
        ));
    }
    let manifest_path = attempt_dir(root, attempt_id).join("attempt.json");
    if let Ok(manifest) = read_json(&manifest_path) {
        for pointer in ["/artifacts", "/terminal_artifacts"] {
            let Some(artifacts) = manifest.pointer(pointer).and_then(Value::as_array) else {
                continue;
            };
            if artifacts.iter().any(|artifact| {
                artifact.get("role").and_then(Value::as_str) == Some("agent_receipt")
            }) {
                return Err(format!(
                    "{context}: attempt manifest lists an agent_receipt under {pointer}:\n{manifest}"
                ));
            }
        }
    }
    Ok(())
}

fn assert_exit_code(output: &Output, expected: i32, context: &str) -> Result<(), String> {
    if output.status.code() == Some(expected) {
        return Ok(());
    }
    Err(format!(
        "{context}: want exit {expected}, got {:?}\n{}",
        output.status.code(),
        combined_output(output)
    ))
}

fn start_before(label: &str) -> Result<(Journey, Fixture), String> {
    start_before_after(label, |_| Ok(()))
}

fn start_before_after(
    label: &str,
    after_init: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(Journey, Fixture), String> {
    let root = unique_temp_workspace(label);
    fs::create_dir_all(&root).map_err(|error| format!("create {}: {error}", root.display()))?;
    let owned = Fixture(root.clone());
    init_fixture_repo(&root)?;
    after_init(&root)?;
    let root_arg = root.display().to_string();
    let seam_id = discover_seam_id(&root, &root_arg)?;
    let output = run_ripr(
        &root,
        &[
            "agent",
            "repair",
            "--json",
            "--root",
            &root_arg,
            "--seam-id",
            &seam_id,
            "--phase",
            "before",
        ],
    )?;
    if !output.status.success() {
        return Err(format!(
            "agent repair --phase before failed with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let document = parse_stdout_json(&output)?;
    let attempt_id = document
        .pointer("/repair_attempt/attempt_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!("before-phase stdout is missing repair_attempt.attempt_id:\n{document}")
        })?
        .to_owned();
    if attempt_id.is_empty() {
        return Err("before-phase published an empty attempt id".to_string());
    }
    Ok((
        Journey {
            root,
            root_arg,
            seam_id,
            attempt_id,
        },
        owned,
    ))
}

fn run_after(journey: &Journey) -> Result<Output, String> {
    run_after_attempt(&journey.root, &journey.root_arg, &journey.attempt_id)
}

fn run_after_attempt(root: &Path, root_arg: &str, attempt_id: &str) -> Result<Output, String> {
    run_ripr(
        root,
        &[
            "agent",
            "repair",
            "--json",
            "--root",
            root_arg,
            "--attempt",
            attempt_id,
            "--phase",
            "after",
        ],
    )
}

/// Runs a before phase against an already-prepared fixture root and returns
/// the minted attempt id. The supersession-bytes case needs two attempts on
/// one root; `start_before` always builds a fresh root.
fn run_before_phase(root: &Path, root_arg: &str, seam_id: &str) -> Result<String, String> {
    let output = run_ripr(
        root,
        &[
            "agent",
            "repair",
            "--json",
            "--root",
            root_arg,
            "--seam-id",
            seam_id,
            "--phase",
            "before",
        ],
    )?;
    if !output.status.success() {
        return Err(format!(
            "agent repair --phase before failed with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let document = parse_stdout_json(&output)?;
    let attempt_id = document
        .pointer("/repair_attempt/attempt_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!("before-phase stdout is missing repair_attempt.attempt_id:\n{document}")
        })?
        .to_owned();
    if attempt_id.is_empty() {
        return Err("before-phase published an empty attempt id".to_string());
    }
    Ok(attempt_id)
}

/// Reads the typed attempt-status document for one exact attempt. The oracle
/// binds production's versioned `agent_attempt_status` shape; it computes no
/// state machine of its own.
fn run_attempt_status(root: &Path, root_arg: &str, attempt_id: &str) -> Result<Value, String> {
    let output = run_ripr(
        root,
        &[
            "agent",
            "status",
            "--json",
            "--root",
            root_arg,
            "--attempt",
            attempt_id,
        ],
    )?;
    if !output.status.success() {
        return Err(format!(
            "agent status --attempt failed with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    parse_stdout_json(&output)
}

fn json_str<'a>(value: &'a Value, pointer: &str, label: &str) -> Result<&'a str, String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label} carries no {pointer}:\n{value}"))
}

fn run_receipt(journey: &Journey) -> Result<Output, String> {
    run_ripr(
        &journey.root,
        &[
            "agent",
            "receipt",
            "--json",
            "--root",
            &journey.root_arg,
            "--attempt",
            &journey.attempt_id,
            "--seam-id",
            &journey.seam_id,
            "--verify-json",
            "target/ripr/workflow/agent-verify.json",
        ],
    )
}

fn verdict_status(root: &Path, attempt_id: &str) -> Result<String, String> {
    let manifest = attempt_manifest(root, attempt_id)?;
    manifest
        .pointer("/after/verdict/status")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("attempt {attempt_id} has no after.verdict.status:\n{manifest}"))
}

fn combined_output(output: &Output) -> String {
    format!(
        "status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn bench_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks/agentic/edit-cage/manifest.json")
}

/// The committed B6 manifest is the runner's identity document. Schema and
/// oracle command are checked here; path-matching and fixture digests stay
/// with `cargo xtask agentic-bench` (the production owner).
#[test]
fn b6_manifest_names_the_production_oracle() -> Result<(), String> {
    let manifest = read_json(&bench_manifest())?;
    if manifest.get("schema_version").and_then(Value::as_str)
        != Some("ripr-agentic-bench-manifest-v1")
    {
        return Err(format!("B6 manifest schema drifted:\n{manifest}"));
    }
    if manifest.get("bench").and_then(Value::as_str) != Some("edit-cage")
        || manifest.get("bench_index").and_then(Value::as_str) != Some("B6")
    {
        return Err(format!("B6 manifest identity drifted:\n{manifest}"));
    }
    if manifest.get("oracle_command").and_then(Value::as_str)
        != Some("cargo test -p ripr --test agentic_bench_cage")
    {
        return Err(format!("B6 oracle_command drifted:\n{manifest}"));
    }
    Ok(())
}

/// Out-of-surface production edit fails closed: violated verdict, CLI
/// refusal, no receipt. Production evaluates the cage during finish, then the
/// receipt path refuses (`not receipt-ready`). That refusal is after the
/// verify render, so `--json` prints the bare verify document and the process
/// exits 2 (`CommandError::Failure`). It is not the pre-verify exit-3
/// `repair_after_refusal` path (diverged HEAD, input drift, no-movement
/// verify, replaced trust manifest).
#[test]
fn b6_out_of_surface_edit_fails_closed() -> Result<(), String> {
    let (journey, _owned) = start_before("agentic-b6-out-of-surface")?;
    add_in_surface_test(&journey.root)?;
    edit_out_of_surface(&journey.root)?;
    let after = run_after(&journey)?;
    assert_exit_code(&after, 2, "out-of-surface after-phase")?;
    let status = verdict_status(&journey.root, &journey.attempt_id)?;
    if status != "violated" {
        return Err(format!(
            "out-of-surface edit verdict is {status}, want violated"
        ));
    }
    // #6286: pin the exact violation the production cage records, not just
    // the `violated` status. The labels come from production; the oracle pins
    // only their spelling and location.
    let manifest = attempt_manifest(&journey.root, &journey.attempt_id)?;
    if json_str(&manifest, "/state", "failed attempt manifest")? != "failed" {
        return Err(format!(
            "out-of-surface attempt must record state failed:\n{manifest}"
        ));
    }
    let violations = manifest
        .pointer("/after/verdict/violations")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("failed attempt manifest carries no violations:\n{manifest}"))?;
    if !violations.iter().any(|violation| {
        violation.pointer("/path").and_then(Value::as_str) == Some("src/lib.rs")
            && violation.pointer("/kind").and_then(Value::as_str) == Some("forbidden_path")
    }) {
        return Err(format!(
            "violations must name src/lib.rs as forbidden_path: {violations:?}"
        ));
    }
    if violations.iter().any(|violation| {
        violation.pointer("/path").and_then(Value::as_str) == Some("tests/pricing.rs")
    }) {
        return Err(format!(
            "the selected test target must not be a violation: {violations:?}"
        ));
    }
    let joined = combined_output(&after);
    if !joined.contains("src/lib.rs") {
        return Err(format!(
            "CLI refusal did not name the out-of-surface path:\n{joined}"
        ));
    }
    if !joined.contains("OutsideAllowedSurface")
        && !joined.contains("ForbiddenPath")
        && !joined.contains("forbidden")
        && !joined.contains("outside")
    {
        return Err(format!(
            "CLI refusal did not name the out-of-surface kind:\n{joined}"
        ));
    }
    if !joined.contains("not receipt-ready") {
        return Err(format!(
            "CLI refusal did not use the production receipt-ready refusal:\n{joined}"
        ));
    }
    if let Ok(document) = parse_stdout_json(&after) {
        let kind = document.get("kind").and_then(Value::as_str);
        if kind == Some("repair_after_result") {
            return Err(format!(
                "out-of-surface refusal printed a success after-result envelope:\n{document}"
            ));
        }
        if kind == Some("repair_after_refusal") {
            return Err(format!(
                "out-of-surface cage refusal is post-verify exit 2, not typed repair_after_refusal:\n{document}"
            ));
        }
    }
    assert_no_receipt_written(&journey.root, &journey.attempt_id, "out-of-surface edit")?;
    // #6286: the typed CLI surface for the refusal. Production's versioned
    // attempt-status document reports the failed state and the one recovery
    // step; the after phase itself exits 2 (only pre-verify named refusals
    // are exit-3 typed), so this document is the typed refusal the oracle
    // binds.
    let attempt_status = run_attempt_status(&journey.root, &journey.root_arg, &journey.attempt_id)?;
    if json_str(&attempt_status, "/kind", "attempt status")? != "agent_attempt_status" {
        return Err(format!(
            "attempt status must be the typed agent_attempt_status document:\n{attempt_status}"
        ));
    }
    if json_str(&attempt_status, "/attempt/state", "attempt status")? != "failed" {
        return Err(format!(
            "attempt status must report the failed state:\n{attempt_status}"
        ));
    }
    if json_str(&attempt_status, "/next_action/step", "attempt status")? != "repair_attempt_before"
    {
        return Err(format!(
            "attempt status must route recovery through a fresh before phase:\n{attempt_status}"
        ));
    }
    let command = json_str(&attempt_status, "/next_action/command", "attempt status")?;
    if !command.contains("--phase before") || !command.contains(journey.seam_id.as_str()) {
        return Err(format!(
            "attempt status recovery command must name the before phase for seam {}:\n{attempt_status}",
            journey.seam_id
        ));
    }
    Ok(())
}

/// A dirty production premise refuses the before phase itself (#5262). The
/// loop pins the repository at its before phase, so an uncommitted
/// production change used to pass the edit cage silently and only refuse the
/// receipt after the whole attempt was consumed. Now the before phase names
/// the paths and the commit-first recovery, and no attempt or workflow
/// artifact exists. The boundary is production content, not worktree
/// cleanliness: a dirty focused test file inside the allowed surface still
/// starts and completes the loop, because the loop expects the test edit.
#[test]
fn b6_dirty_production_premise_refuses_the_before_phase_and_a_dirty_test_still_runs()
-> Result<(), String> {
    let root = unique_temp_workspace("agentic-b6-dirty-premise");
    fs::create_dir_all(&root).map_err(|error| format!("create {}: {error}", root.display()))?;
    let _owned = Fixture(root.clone());
    init_fixture_repo(&root)?;
    let root_arg = root.display().to_string();

    // Uncommitted production change before the loop starts.
    edit_out_of_surface(&root)?;
    let seam_id = discover_seam_id(&root, &root_arg)?;
    let before = run_ripr(
        &root,
        &[
            "agent",
            "repair",
            "--json",
            "--root",
            &root_arg,
            "--seam-id",
            &seam_id,
            "--phase",
            "before",
        ],
    )?;
    assert_exit_code(&before, 2, "dirty-premise before-phase")?;
    let text = combined_output(&before);
    for fragment in [
        "repair attempt cannot start",
        "src/lib.rs",
        "to recover:",
        "git commit -- src/lib.rs",
        "No workflow was prepared and no repair attempt was started.",
    ] {
        if !text.contains(fragment) {
            return Err(format!(
                "dirty-premise refusal must name `{fragment}`:\n{text}"
            ));
        }
    }
    let attempts = root.join("target/ripr/repair-attempts");
    let published = if attempts.is_dir() {
        fs::read_dir(&attempts)
            .map_err(|error| format!("read {}: {error}", attempts.display()))?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("repair-attempt-")
            })
            .count()
    } else {
        0
    };
    if published != 0 {
        return Err(format!(
            "a dirty-premise refusal published {published} attempt directories"
        ));
    }
    if root.join("target/ripr/workflow").exists() {
        return Err("a dirty-premise refusal wrote workflow artifacts".to_string());
    }

    // The recovery the refusal names: commit the production change, and the
    // same seam starts.
    fixture_git_ok(&root, &["add", "src/lib.rs"])
        .map_err(|error| format!("git add src/lib.rs: {error}"))?;
    fixture_git_ok(
        &root,
        &[
            "-c",
            "user.name=RIPR test",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-qm",
            "committed production premise",
        ],
    )
    .map_err(|error| format!("git commit src/lib.rs: {error}"))?;
    let committed = run_ripr(
        &root,
        &[
            "agent",
            "repair",
            "--json",
            "--root",
            &root_arg,
            "--seam-id",
            &seam_id,
            "--phase",
            "before",
        ],
    )?;
    if !committed.status.success() {
        return Err(format!(
            "before phase must start once the production premise is committed:\n{}",
            combined_output(&committed)
        ));
    }
    let attempt_id = parse_stdout_json(&committed)?
        .pointer("/repair_attempt/attempt_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            format!(
                "committed-premise before-phase stdout is missing repair_attempt.attempt_id:\n{}",
                String::from_utf8_lossy(&committed.stdout)
            )
        })?;

    // Boundary control on the same loop: a dirty focused test file inside
    // the allowed surface does not refuse the before phase. The dirty edit
    // adds an unrelated passing test so the seam's gap still exists and the
    // packet still renders (a gap-closing test would make the seam
    // strongly_gripped and refuse a fresh before phase at the packet gate,
    // which is the gap rule, not the premise rule). The loop starts, and
    // closing the gap mid-loop completes the after phase compliantly.
    append_test_fn(
        &root,
        "unrelated_smoke_still_passes",
        "assert_eq!(discounted_total(10, 100), 10);",
    )?;
    let dirty_test_before = run_ripr(
        &root,
        &[
            "agent",
            "repair",
            "--json",
            "--root",
            &root_arg,
            "--seam-id",
            &seam_id,
            "--phase",
            "before",
        ],
    )?;
    if !dirty_test_before.status.success() {
        return Err(format!(
            "a dirty focused test file must not refuse the before phase:\n{}",
            combined_output(&dirty_test_before)
        ));
    }
    let test_attempt_id = parse_stdout_json(&dirty_test_before)?
        .pointer("/repair_attempt/attempt_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            format!(
                "dirty-test before-phase stdout is missing repair_attempt.attempt_id:\n{}",
                String::from_utf8_lossy(&dirty_test_before.stdout)
            )
        })?;
    // The mid-loop edit closes the gap with the equality test. It must not
    // duplicate an existing fn name: repeated `add_in_surface_test` appends
    // the same fn twice and would fail compilation instead of proving the
    // boundary.
    append_test_fn(
        &root,
        "equality_boundary_discounts_closes_the_gap",
        "assert_eq!(discounted_total(100, 100), 90);",
    )?;
    let after = run_ripr(
        &root,
        &[
            "agent",
            "repair",
            "--json",
            "--root",
            &root_arg,
            "--attempt",
            &test_attempt_id,
            "--phase",
            "after",
        ],
    )?;
    if !after.status.success() {
        return Err(format!(
            "the loop must complete when only the allowed test surface was dirty and edited:\n{}",
            combined_output(&after)
        ));
    }
    let status = verdict_status(&root, &test_attempt_id)?;
    if status != "compliant" {
        return Err(format!(
            "dirty-test-premise loop verdict is {status}, want compliant"
        ));
    }
    if attempt_id.is_empty() {
        return Err("committed-premise attempt id was empty".to_string());
    }
    Ok(())
}

/// Appends one distinct `#[test]` fn to the fixture's focused test file.
fn append_test_fn(root: &Path, name: &str, body: &str) -> Result<(), String> {
    let path = root.join("tests/pricing.rs");
    let mut tests =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    tests.push_str(&format!("#[test]\nfn {name}() {{\n    {body}\n}}\n"));
    fs::write(&path, tests).map_err(|error| format!("write {}: {error}", path.display()))
}

/// Production reread: a finished attempt's receipt must not replay after the
/// snapshot moves. Production has no `superseded` cage token; the expected-value
/// oracles are the CLI refusals `tampered or stale` and `already finished`.
#[test]
fn b6_superseded_snapshot_never_replays_a_stale_verdict() -> Result<(), String> {
    let (journey, _owned) = start_before("agentic-b6-superseded")?;
    add_in_surface_test(&journey.root)?;
    let after = run_after(&journey)?;
    if !after.status.success() {
        return Err(format!(
            "in-surface after-phase must succeed so the stale-reread control has a receipt:\n{}",
            combined_output(&after)
        ));
    }
    let status = verdict_status(&journey.root, &journey.attempt_id)?;
    if status != "compliant" {
        return Err(format!(
            "in-surface finish verdict is {status}, want compliant"
        ));
    }
    let reread = run_receipt(&journey)?;
    if !reread.status.success() {
        return Err(format!(
            "production receipt reread of an unchanged finished attempt failed:\n{}",
            combined_output(&reread)
        ));
    }

    edit_out_of_surface(&journey.root)?;
    let stale = run_receipt(&journey)?;
    if stale.status.success() {
        return Err(format!(
            "receipt replayed a stale compliant verdict after the tree moved:\n{}",
            combined_output(&stale)
        ));
    }
    let stale_text = combined_output(&stale);
    // Production reread re-evaluates the cage and compares it to the stored
    // after verdict. There is no `superseded` cage token; the refusal string
    // is the expected-value oracle for the original B6 supersession ledger.
    if !stale_text.contains("tampered or stale") {
        return Err(format!(
            "stale receipt refusal did not use the production binding vocabulary:\n{stale_text}"
        ));
    }

    let replay = run_after(&journey)?;
    if replay.status.success() {
        return Err(format!(
            "after-phase replayed a finished attempt:\n{}",
            combined_output(&replay)
        ));
    }
    let replay_text = combined_output(&replay);
    if !replay_text.contains("already finished") && !replay_text.contains("terminal") {
        return Err(format!(
            "finished-attempt refusal did not name already-finished/terminal:\n{replay_text}"
        ));
    }
    Ok(())
}

/// A superseding finish never replays the first attempt's verdict (#6286).
/// Two attempts finish against one fixture: the second finish replaces the
/// one-slot workflow receipt with its own verdict binding, while the first
/// attempt's record stays byte-identical and its retained receipt keeps
/// binding its own attempt. The oracle reads production's own manifests and
/// receipts and computes no digests of its own.
#[test]
fn b6_superseding_finish_keeps_first_attempt_byte_identical() -> Result<(), String> {
    let (journey_a, _owned) = start_before("agentic-b6-supersession-bytes")?;
    let attempt_b = run_before_phase(&journey_a.root, &journey_a.root_arg, &journey_a.seam_id)?;
    if journey_a.attempt_id == attempt_b {
        return Err("the two before phases minted the same attempt id".to_string());
    }
    append_test_fn(
        &journey_a.root,
        "equality_boundary_discounts",
        "assert_eq!(discounted_total(100, 100), 90);",
    )?;
    let after_a = run_after(&journey_a)?;
    if !after_a.status.success() {
        return Err(format!(
            "first after phase failed:\n{}",
            combined_output(&after_a)
        ));
    }
    let manifest_a = attempt_manifest(&journey_a.root, &journey_a.attempt_id)?;
    if json_str(&manifest_a, "/state", "first attempt manifest")? != "ready_to_finish" {
        return Err(format!(
            "first attempt must finish ready_to_finish:\n{manifest_a}"
        ));
    }
    let manifest_path_a = attempt_dir(&journey_a.root, &journey_a.attempt_id).join("attempt.json");
    let manifest_a_bytes = fs::read(&manifest_path_a)
        .map_err(|error| format!("read {}: {error}", manifest_path_a.display()))?;
    let first_receipt = read_json(&compatibility_receipt_path(&journey_a.root))?;
    if json_str(
        &first_receipt,
        "/repair_attempt/attempt_id",
        "first receipt",
    )? != journey_a.attempt_id.as_str()
    {
        return Err(format!(
            "the workflow receipt must bind the first finished attempt:\n{first_receipt}"
        ));
    }

    append_test_fn(
        &journey_a.root,
        "equality_boundary_second_case",
        "assert_eq!(discounted_total(50, 50), 40);",
    )?;
    let after_b = run_after_attempt(&journey_a.root, &journey_a.root_arg, &attempt_b)?;
    if !after_b.status.success() {
        return Err(format!(
            "second after phase failed:\n{}",
            combined_output(&after_b)
        ));
    }
    let manifest_b = attempt_manifest(&journey_a.root, &attempt_b)?;
    if json_str(&manifest_b, "/state", "second attempt manifest")? != "ready_to_finish" {
        return Err(format!(
            "second attempt must finish ready_to_finish:\n{manifest_b}"
        ));
    }

    // The superseding finish replaces the projection with its own verdict
    // binding; it must not replay the first attempt's verdict or touch the
    // first attempt's record.
    let superseded = read_json(&compatibility_receipt_path(&journey_a.root))?;
    if json_str(
        &superseded,
        "/repair_attempt/attempt_id",
        "superseded receipt",
    )? != attempt_b.as_str()
    {
        return Err(format!(
            "the workflow receipt must bind the superseding attempt:\n{superseded}"
        ));
    }
    let bound_delta = json_str(
        &superseded,
        "/repair_attempt/delta_sha256",
        "superseded receipt",
    )?;
    let verdict_delta = json_str(
        &manifest_b,
        "/after/delta_sha256",
        "second attempt manifest",
    )?;
    if bound_delta != verdict_delta {
        return Err(format!(
            "the superseding receipt must bind attempt B's own verdict digest, not a replay: receipt {bound_delta}, manifest {verdict_delta}"
        ));
    }
    if fs::read(&manifest_path_a)
        .map_err(|error| format!("reread {}: {error}", manifest_path_a.display()))?
        != manifest_a_bytes
    {
        return Err(
            "the superseding finish must not rewrite the first attempt's record".to_string(),
        );
    }
    let retained_a = read_json(&attempt_local_receipt_path(
        &journey_a.root,
        &journey_a.attempt_id,
    ))?;
    if json_str(
        &retained_a,
        "/repair_attempt/attempt_id",
        "retained first receipt",
    )? != journey_a.attempt_id.as_str()
    {
        return Err(format!(
            "the first attempt's retained receipt must keep binding its own attempt:\n{retained_a}"
        ));
    }
    Ok(())
}

/// Over-budget capture fails closed: a file one byte over the production
/// per-file bound is planted before `--phase before` so both snapshots see
/// `WorktreeIdentity::Other`. Production `evaluate_edit_cage` then records
/// `incomparable` (`!delta.comparable`); an in-surface test edit cannot earn
/// a receipt. The numeric bound is an expected-value pin of
/// `MAX_CAPTURE_FILE_BYTES`; this suite does not reimplement capture.
#[test]
fn b6_over_budget_capture_fails_closed() -> Result<(), String> {
    let (journey, _owned) = start_before_after("agentic-b6-over-budget", plant_over_budget_file)?;
    add_in_surface_test(&journey.root)?;
    let after = run_after(&journey)?;
    assert_exit_code(&after, 2, "over-budget after-phase")?;
    let status = verdict_status(&journey.root, &journey.attempt_id)?;
    if status != "incomparable" {
        return Err(format!(
            "over-budget capture verdict is {status}, want incomparable\n{}",
            combined_output(&after)
        ));
    }
    assert_no_receipt_written(&journey.root, &journey.attempt_id, "over-budget capture")?;
    Ok(())
}

/// Invalid retained packet or attempt manifest is rejected before any receipt.
#[test]
fn b6_invalid_packet_and_manifest_are_rejected_before_any_receipt() -> Result<(), String> {
    let (packet_journey, _packet_owned) = start_before("agentic-b6-invalid-packet")?;
    let packet_manifest = attempt_manifest(&packet_journey.root, &packet_journey.attempt_id)?;
    let packet_path = artifact_path(&packet_journey.root, &packet_manifest, "agent_packet")?;
    fs::write(&packet_path, "{not a repair packet")
        .map_err(|error| format!("corrupt retained packet: {error}"))?;
    add_in_surface_test(&packet_journey.root)?;
    let after = run_after(&packet_journey)?;
    assert_exit_code(&after, 2, "invalid packet after-phase")?;
    assert_no_receipt_written(
        &packet_journey.root,
        &packet_journey.attempt_id,
        "invalid packet",
    )?;
    // #6286: the binding fails before the durable finish: the attempt stays
    // awaiting its edit with no recorded verdict.
    let retained = attempt_manifest(&packet_journey.root, &packet_journey.attempt_id)?;
    if json_str(&retained, "/state", "packet-corrupted manifest")? != "awaiting_edit" {
        return Err(format!(
            "a packet rejected before the finish must leave the attempt awaiting_edit:\n{retained}"
        ));
    }
    if retained
        .pointer("/after")
        .is_some_and(|after| !after.is_null())
    {
        return Err(format!(
            "a packet rejected before the finish must record no after verdict:\n{retained}"
        ));
    }

    let (manifest_journey, _manifest_owned) = start_before("agentic-b6-invalid-manifest")?;
    let manifest_path =
        attempt_dir(&manifest_journey.root, &manifest_journey.attempt_id).join("attempt.json");
    fs::write(&manifest_path, "{not an attempt manifest")
        .map_err(|error| format!("corrupt attempt manifest: {error}"))?;
    add_in_surface_test(&manifest_journey.root)?;
    let after = run_after(&manifest_journey)?;
    assert_exit_code(&after, 2, "invalid attempt manifest after-phase")?;
    assert_no_receipt_written(
        &manifest_journey.root,
        &manifest_journey.attempt_id,
        "invalid attempt manifest",
    )?;
    Ok(())
}
