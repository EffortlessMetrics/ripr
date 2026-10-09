use super::{
    BOUNDARY_GAP_SEAM_ID, advance_fixture_head, assert_success, ignore_remove_dir_all,
    init_producer_fixture_repo, renderer_shell_arg, spawn_command, unique_temp_workspace,
};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::process::Output;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const BEFORE: &str = "target/ripr/workflow/before.repo-exposure.json";
const AFTER: &str = "target/ripr/workflow/after.repo-exposure.json";
const VERIFY: &str = "target/ripr/workflow/agent-verify.json";
const RECEIPT: &str = "target/ripr/reports/agent-receipt.json";
const CONTROL: &str = "target/ripr/workflow/control-verify.json";
const DECOY_MARKER: &[u8] = b"independent slash-root verify marker\n";

// Only this case's unique parent is owned here. Tear it down on early errors
// and assertion unwinds as well as the checked success-path cleanup below.
struct FixtureCleanup(PathBuf);

impl Drop for FixtureCleanup {
    fn drop(&mut self) {
        ignore_remove_dir_all(&self.0);
    }
}

fn cli(cwd: &Path, args: &[&str]) -> TestResult<Output> {
    // Use the existing owned, drained, deadline-bounded spawn site. Every CLI
    // and shell invocation consumes the exact executable built for this test.
    Ok(spawn_command(
        env!("CARGO_BIN_EXE_ripr"),
        Some(cwd),
        args,
        &[],
        None,
        Some(b""),
    )?)
}

fn snapshots(root: &Path) -> TestResult<Vec<(std::path::PathBuf, Vec<u8>)>> {
    init_producer_fixture_repo(root)?;
    std::fs::create_dir_all(root.join("target/ripr/workflow"))?;
    let mut custody = Vec::new();
    for relative in [BEFORE, AFTER] {
        if relative == AFTER {
            advance_fixture_head(root, "descended recovery snapshot")?;
        }
        let output = cli(
            root,
            &[
                "check",
                "--root",
                root.to_str().ok_or("fixture root must be UTF-8")?,
                "--base",
                "HEAD",
                "--mode",
                "draft",
                "--format",
                "repo-exposure-json",
            ],
        )?;
        assert_success(&output);
        let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(value["run_status"], "complete", "snapshot setup: {value}");
        assert_eq!(
            value["artifact"]["repository"]["root"].as_str(),
            root.to_str(),
            "snapshot setup must bind the native root"
        );
        assert!(
            value["seams"].as_array().is_some_and(|seams| seams
                .iter()
                .any(|seam| seam["seam_id"] == BOUNDARY_GAP_SEAM_ID)),
            "the real producer must discover the intended seam: {value}"
        );
        let path = root.join(relative);
        std::fs::write(&path, &output.stdout)?;
        custody.push((path, output.stdout));
    }
    Ok(custody)
}

fn receipt(cwd: &Path, root: &Path, verify: &str, out: bool) -> TestResult<Output> {
    let mut args = vec![
        "agent",
        "receipt",
        "--root",
        root.to_str().ok_or("receipt root must be UTF-8")?,
        "--verify-json",
        verify,
        "--seam-id",
        BOUNDARY_GAP_SEAM_ID,
        "--json",
    ];
    if out {
        args.extend(["--out", RECEIPT]);
    }
    cli(cwd, &args)
}

fn missing_hint(cwd: &Path, root: &Path, verify: &str) -> TestResult<String> {
    let output = receipt(cwd, root, verify, true)?;
    assert_eq!(output.status.code(), Some(2), "missing input: {output:?}");
    assert!(
        output.stdout.is_empty(),
        "a refusal must not issue a receipt"
    );
    assert!(!root.join(RECEIPT).exists());
    let error = String::from_utf8(output.stderr)?;
    assert!(error.contains("canonicalize agent receipt --verify-json"));
    Ok(error
        .split_once("; produce it with: ")
        .ok_or("actual CLI refusal must name the workflow producer")?
        .1
        .trim_end_matches(['\r', '\n'])
        .to_string())
}

fn shell(cwd: &Path, custody: &Path, command: &str) -> TestResult<Vec<Vec<u8>>> {
    // Capture actual shell argv separately from redirected JSON, then invoke
    // the real verifier. This function does not decode or rewrite the hint.
    let script = format!(
        "ripr() {{ printf '%s\\000' \"$@\" > {}; {} \"$@\"; }}\n{command}\n",
        renderer_shell_arg(custody.to_str().ok_or("custody path must be UTF-8")?),
        renderer_shell_arg(env!("CARGO_BIN_EXE_ripr")),
    );
    let output = spawn_command("sh", Some(cwd), &["-s"], &[], None, Some(script.as_bytes()))?;
    assert_success(&output);
    assert!(
        output.stdout.is_empty(),
        "verifier stdout belongs in its redirect"
    );
    let bytes = std::fs::read(custody)?;
    assert_eq!(
        bytes.last(),
        Some(&0),
        "argv custody must be NUL terminated"
    );
    Ok(bytes[..bytes.len() - 1]
        .split(|byte| *byte == 0)
        .map(<[u8]>::to_vec)
        .collect())
}

fn assert_argv_root(argv: &[Vec<u8>], expected: &Path) -> TestResult {
    assert_eq!(argv.len(), 9, "exact verifier invocation: {argv:?}");
    for (index, value) in [
        (0, "agent"),
        (1, "verify"),
        (2, "--root"),
        (4, "--before"),
        (5, BEFORE),
        (6, "--after"),
        (7, AFTER),
        (8, "--json"),
    ] {
        assert_eq!(argv[index], value.as_bytes());
    }
    assert_eq!(
        argv[3],
        expected.as_os_str().as_bytes(),
        "the emitted recovery hint must select the exact native root"
    );
    let actual = Path::new(std::ffi::OsStr::from_bytes(&argv[3]));
    let selected = std::fs::metadata(expected)?;
    let opened = std::fs::metadata(actual)?;
    assert_eq!(
        (opened.dev(), opened.ino()),
        (selected.dev(), selected.ino())
    );
    Ok(())
}

fn known_command(root: &Path) -> TestResult<String> {
    Ok(format!(
        "ripr agent verify --root {} --before {BEFORE} --after {AFTER} --json > {}",
        renderer_shell_arg(root.to_str().ok_or("control root must be UTF-8")?),
        renderer_shell_arg(
            root.join(CONTROL)
                .to_str()
                .ok_or("control target must be UTF-8")?
        ),
    ))
}

fn assert_verify(path: &Path, root: &Path) -> TestResult {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("read verifier output {} failed: {error}", path.display()))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(value["schema_version"], "0.3", "genuine verify: {value}");
    assert!(
        value["unchanged_seams"]
            .as_array()
            .is_some_and(|seams| seams
                .iter()
                .any(|seam| seam["seam_id"] == BOUNDARY_GAP_SEAM_ID)),
        "genuine verifier must compare the selected seam: {value}"
    );
    for (key, relative) in [("before", BEFORE), ("after", AFTER)] {
        assert_eq!(value["inputs"][key], relative);
        let input: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join(relative))?)?;
        assert_eq!(
            input["artifact"]["repository"]["root"].as_str(),
            root.to_str()
        );
        assert_eq!(
            value["inputs"][format!("{key}_content_sha256")],
            input["artifact"]["content_sha256"]
        );
    }
    Ok(())
}

/// #6684: an actual missing-file error's command must repair the file this
/// receipt selected. A suffix-only oracle would accept a different checkout.
fn recovery_case(absolute: bool) -> TestResult {
    let parent = unique_temp_workspace("receipt-recovery-native");
    std::fs::create_dir(&parent)?;
    let _cleanup = FixtureCleanup(parent.clone());
    let selected = parent.join("team\\repo 'quoted'");
    let decoy = parent.join("team/repo 'quoted'");
    let foreign = parent.join("foreign");
    for path in [&selected, &decoy, &foreign] {
        std::fs::create_dir_all(path)?;
    }
    let selected = selected.canonicalize()?;
    let decoy = decoy.canonicalize()?;
    assert_ne!(selected, decoy);
    let mut snapshots = snapshots(&selected)?;
    snapshots.extend(self::snapshots(&decoy)?);
    let selected_verify = selected.join(VERIFY);
    let decoy_verify = decoy.join(VERIFY);
    std::fs::write(&decoy_verify, DECOY_MARKER)?;
    let argv_path = parent.join("actual-argv.nul");

    // Establish both real shell/producer routes before the intended RED. The
    // deliberately wrong display-root control must actually write a genuine
    // independent slash-decoy artifact. Distinct control outputs cannot seed
    // or mask the missing default file whose recovery is being tested.
    let wrong = shell(&foreign, &argv_path, &known_command(&decoy)?)?;
    assert_argv_root(&wrong, &decoy)?;
    assert_verify(&decoy.join(CONTROL), &decoy)?;
    assert!(!selected_verify.exists());
    let pilot = shell(&foreign, &argv_path, &known_command(&selected)?)?;
    assert_argv_root(&pilot, &selected)?;
    assert_verify(&selected.join(CONTROL), &selected)?;
    assert!(!selected_verify.exists());
    assert_eq!(std::fs::read(&decoy_verify)?, DECOY_MARKER);

    // Relative is essential: an absolute redirect can conceal a wrong --root.
    // Test the absolute route too, because its own renderer owns that spelling.
    let absolute_verify = selected_verify
        .to_str()
        .ok_or("verify path must be UTF-8")?;
    let verify_input = if absolute { absolute_verify } else { VERIFY };
    {
        assert!(!selected_verify.exists());
        let hint = missing_hint(&foreign, &selected, verify_input)?;
        let argv = shell(&foreign, &argv_path, &hint)?;
        assert_argv_root(&argv, &selected)?;
        assert_verify(&selected_verify, &selected)?;
        assert_eq!(std::fs::read(&decoy_verify)?, DECOY_MARKER);
        assert!(!foreign.join("target").exists());
        for (path, original) in &snapshots {
            assert_eq!(
                &std::fs::read(path)?,
                original,
                "snapshot custody: {path:?}"
            );
        }
        let consumed = receipt(&foreign, &selected, verify_input, true)?;
        assert_success(&consumed);
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(selected.join(RECEIPT))?)?;
        assert_eq!(value["schema_version"], "0.5");
        assert_eq!(value["provenance"]["repo_root"].as_str(), selected.to_str());
        assert_eq!(value["seam"]["seam_id"], BOUNDARY_GAP_SEAM_ID);
        assert_eq!(value["analysis_outcome_status"], "missing");
        assert_eq!(value["status"], "incomplete");
        assert_eq!(value["inputs"]["agent_verify_json"], verify_input);
        assert_eq!(value["provenance"]["verify_artifact"]["path"], verify_input);
        std::fs::remove_file(selected.join(RECEIPT))?;
        std::fs::remove_file(&selected_verify)?;
    }

    // A custom missing path cannot identify a producer snapshot pair. Keep
    // the actual CLI refusal and do not invent a workflow continuation.
    let custom = receipt(&foreign, &selected, "comparisons/verify.json", true)?;
    assert_eq!(custom.status.code(), Some(2));
    assert!(custom.stdout.is_empty());
    let error = String::from_utf8(custom.stderr)?;
    assert!(error.contains("canonicalize agent receipt --verify-json comparisons/verify.json"));
    assert!(!error.contains("produce it with:") && !error.contains(BEFORE));
    assert!(!selected.join(RECEIPT).exists());
    assert!(!selected_verify.exists());
    assert_eq!(std::fs::read(&decoy_verify)?, DECOY_MARKER);
    assert!(!foreign.join("target").exists());
    std::fs::remove_dir_all(parent)?;
    Ok(())
}

#[test]
fn missing_relative_workflow_verify_hint_runs_in_native_root_from_foreign_cwd() -> TestResult {
    recovery_case(false)
}

#[test]
fn missing_absolute_workflow_verify_hint_runs_in_native_root_from_foreign_cwd() -> TestResult {
    recovery_case(true)
}
