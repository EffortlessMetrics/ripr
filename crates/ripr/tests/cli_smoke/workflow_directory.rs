use super::{
    BOUNDARY_GAP_SEAM_ID, advance_fixture_head, assert_success, ignore_remove_dir_all,
    init_producer_fixture_repo, renderer_shell_arg, run_git, spawn_command, unique_temp_workspace,
};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::process::Output;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const BEFORE: &str = "before.repo-exposure.json";
const AFTER: &str = "after.repo-exposure.json";
const VERIFY: &str = "agent-verify.json";
const RECEIPT: &str = "target/ripr/reports/agent-receipt.json";

// Arm only after exclusive creation: cleanup owns no pre-existing directory.
struct FixtureCleanup(PathBuf);

impl Drop for FixtureCleanup {
    fn drop(&mut self) {
        ignore_remove_dir_all(&self.0);
    }
}

fn text(path: &Path) -> TestResult<&str> {
    path.to_str()
        .ok_or_else(|| "fixture path must be UTF-8".into())
}

fn cli(cwd: &Path, args: &[&str]) -> TestResult<Output> {
    Ok(spawn_command(
        env!("CARGO_BIN_EXE_ripr"),
        Some(cwd),
        args,
        &[],
        None,
        Some(b""),
    )?)
}

fn read_json(path: &Path) -> TestResult<serde_json::Value> {
    let bytes = std::fs::read(path).map_err(|error| {
        format!(
            "read native workflow artifact {} failed: {error}",
            path.display()
        )
    })?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn start(cwd: &Path, root: &Path, out: &Path) -> TestResult<serde_json::Value> {
    let output = cli(
        cwd,
        &[
            "agent",
            "start",
            "--root",
            text(root)?,
            "--seam-id",
            BOUNDARY_GAP_SEAM_ID,
            "--out",
            text(out)?,
            "--json",
        ],
    )?;
    assert_success(&output);
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let native = root.join(out);
    assert_eq!(
        envelope["workflow"]["workflow_manifest"],
        text(&native.join("workflow.json"))?
    );
    let value = read_json(&native.join("workflow.json"))?;
    assert_eq!(value["seam"]["seam_id"], BOUNDARY_GAP_SEAM_ID);
    Ok(value)
}

fn command<'a>(manifest: &'a serde_json::Value, step: &str) -> TestResult<&'a str> {
    manifest["commands"]
        .as_array()
        .and_then(|commands| commands.iter().find(|item| item["step"] == step))
        .and_then(|item| item["command"].as_str())
        .ok_or_else(|| format!("real workflow must produce {step}: {manifest}").into())
}

fn shell(cwd: &Path, custody: &Path, command: &str) -> TestResult<Vec<Vec<u8>>> {
    // No token decoding or rewriting: sh executes the exact producer line.
    // NUL argv custody is separate from its genuine redirected JSON output.
    let script = format!(
        "ripr() {{ printf '%s\\000' \"$@\" > {}; {} \"$@\"; }}\n{command}\n",
        renderer_shell_arg(text(custody)?),
        renderer_shell_arg(env!("CARGO_BIN_EXE_ripr")),
    );
    let output = spawn_command("sh", Some(cwd), &["-s"], &[], None, Some(script.as_bytes()))?;
    assert_success(&output);
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

fn exact_argv(actual: &[Vec<u8>], expected: &[&str]) {
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|word| word.as_bytes().to_vec())
            .collect::<Vec<_>>()
    );
}

fn native_identity(actual: &[u8], expected: &Path) -> TestResult {
    assert_eq!(actual, expected.as_os_str().as_bytes(), "native path bytes");
    let opened = std::fs::metadata(Path::new(std::ffi::OsStr::from_bytes(actual)))?;
    let selected = std::fs::metadata(expected)?;
    assert_eq!(
        (opened.dev(), opened.ino()),
        (selected.dev(), selected.ino())
    );
    Ok(())
}

fn snapshot(path: &Path, root: &Path) -> TestResult<Vec<u8>> {
    let value = read_json(path)?;
    assert_eq!(value["run_status"], "complete");
    assert_eq!(value["artifact"]["repository"]["root"], text(root)?);
    assert!(value["seams"].as_array().is_some_and(|seams| {
        seams
            .iter()
            .any(|seam| seam["seam_id"] == BOUNDARY_GAP_SEAM_ID)
    }));
    Ok(std::fs::read(path)?)
}

fn verify(path: &Path, directory: &Path) -> TestResult {
    let value = read_json(path)?;
    assert_eq!(value["schema_version"], "0.3");
    assert!(value["unchanged_seams"].as_array().is_some_and(|seams| {
        seams
            .iter()
            .any(|seam| seam["seam_id"] == BOUNDARY_GAP_SEAM_ID)
    }));
    for (key, file) in [("before", BEFORE), ("after", AFTER)] {
        let input = directory.join(file);
        assert_eq!(value["inputs"][key], text(&input)?);
        assert_eq!(
            value["inputs"][format!("{key}_content_sha256")],
            read_json(&input)?["artifact"]["content_sha256"]
        );
    }
    Ok(())
}

fn receipt(value: &serde_json::Value, root: &Path, input: &Path) -> TestResult {
    assert_eq!(value["schema_version"], "0.5");
    assert_eq!(value["status"], "incomplete");
    assert_eq!(value["analysis_outcome_status"], "missing");
    assert_eq!(value["seam"]["seam_id"], BOUNDARY_GAP_SEAM_ID);
    assert_eq!(value["provenance"]["repo_root"], text(root)?);
    assert_eq!(value["inputs"]["agent_verify_json"], text(input)?);
    assert_eq!(value["provenance"]["verify_artifact"]["path"], text(input)?);
    Ok(())
}

/// #6809: valid evidence and mutual verify/receipt agreement do not establish
/// that a generated workflow consumed the physical directory selected at start.
fn workflow_case(absolute: bool) -> TestResult {
    let parent = unique_temp_workspace("workflow-native-directory");
    std::fs::create_dir(&parent)?;
    let _cleanup = FixtureCleanup(parent.clone());
    let root = parent.join("repo");
    let foreign = parent.join("foreign");
    std::fs::create_dir(&foreign)?;
    init_producer_fixture_repo(&root)?;
    // Generated check commands use the ordinary default base, not a rewritten
    // test command with --base injected. Establish that genuine prerequisite.
    run_git(&root, &["update-ref", "refs/remotes/origin/main", "HEAD"])?;
    let root = root.canonicalize()?;
    let relative = Path::new("target/ripr/work\\flow 'quoted'");
    let selected = root.join(relative);
    let decoy = root.join("target/ripr/work/flow 'quoted'");
    let argv_path = parent.join("actual-argv.nul");
    let source_custody = ["Cargo.toml", "src/lib.rs", "tests/pricing.rs"]
        .into_iter()
        .map(|file| Ok((root.join(file), std::fs::read(root.join(file))?)))
        .collect::<TestResult<Vec<_>>>()?;

    // Strong wrong-location control: real producer commands successfully make
    // a genuine verifier and receipt in the slash-decoy before the intended RED.
    let alternate = start(&foreign, &root, &decoy)?;
    shell(
        &foreign,
        &argv_path,
        command(&alternate, "before_snapshot")?,
    )?;
    snapshot(&decoy.join(BEFORE), &root)?;
    advance_fixture_head(&root, "descended decoy snapshot")?;
    shell(&foreign, &argv_path, command(&alternate, "after_snapshot")?)?;
    snapshot(&decoy.join(AFTER), &root)?;
    let control_argv = shell(&foreign, &argv_path, command(&alternate, "agent_verify")?)?;
    verify(&decoy.join(VERIFY), &decoy)?;
    native_identity(&control_argv[5], &decoy.join(BEFORE))?;
    let control = cli(
        &foreign,
        &[
            "agent",
            "receipt",
            "--root",
            text(&root)?,
            "--verify-json",
            text(&decoy.join(VERIFY))?,
            "--seam-id",
            BOUNDARY_GAP_SEAM_ID,
            "--json",
        ],
    )?;
    assert_success(&control);
    receipt(
        &serde_json::from_slice(&control.stdout)?,
        &root,
        &decoy.join(VERIFY),
    )?;
    assert!(!selected.exists());
    assert!(!root.join(RECEIPT).exists());
    let decoy_custody = [
        "workflow.json",
        "commands.md",
        "agent-brief.json",
        BEFORE,
        AFTER,
        VERIFY,
    ]
    .into_iter()
    .map(|file| Ok((decoy.join(file), std::fs::read(decoy.join(file))?)))
    .collect::<TestResult<Vec<_>>>()?;

    let out = if absolute {
        selected.as_path()
    } else {
        relative
    };
    let manifest = start(&foreign, &root, out)?;
    assert_ne!(
        (
            std::fs::metadata(&selected)?.dev(),
            std::fs::metadata(&selected)?.ino()
        ),
        (
            std::fs::metadata(&decoy)?.dev(),
            std::fs::metadata(&decoy)?.ino()
        )
    );
    let before_argv = shell(&foreign, &argv_path, command(&manifest, "before_snapshot")?)?;
    // Intended behavioral RED: the shell ran successfully, but its native
    // snapshot must exist at the selected physical output, never just a suffix.
    let before_bytes = snapshot(&selected.join(BEFORE), &root)?;
    exact_argv(
        &before_argv,
        &[
            "check",
            "--root",
            text(&root)?,
            "--mode",
            "draft",
            "--format",
            "repo-exposure-json",
        ],
    );
    advance_fixture_head(&root, "descended selected snapshot")?;
    shell(&foreign, &argv_path, command(&manifest, "after_snapshot")?)?;
    let after_bytes = snapshot(&selected.join(AFTER), &root)?;
    let verify_argv = shell(&foreign, &argv_path, command(&manifest, "agent_verify")?)?;
    exact_argv(
        &verify_argv,
        &[
            "agent",
            "verify",
            "--root",
            text(&root)?,
            "--before",
            text(&selected.join(BEFORE))?,
            "--after",
            text(&selected.join(AFTER))?,
            "--json",
        ],
    );
    native_identity(&verify_argv[3], &root)?;
    native_identity(&verify_argv[5], &selected.join(BEFORE))?;
    native_identity(&verify_argv[7], &selected.join(AFTER))?;
    verify(&selected.join(VERIFY), &selected)?;
    let verify_bytes = std::fs::read(selected.join(VERIFY))?;
    let receipt_argv = shell(&foreign, &argv_path, command(&manifest, "agent_receipt")?)?;
    exact_argv(
        &receipt_argv,
        &[
            "agent",
            "receipt",
            "--root",
            text(&root)?,
            "--verify-json",
            text(&selected.join(VERIFY))?,
            "--seam-id",
            BOUNDARY_GAP_SEAM_ID,
            "--json",
            "--out",
            RECEIPT,
        ],
    );
    native_identity(&receipt_argv[5], &selected.join(VERIFY))?;
    receipt(
        &read_json(&root.join(RECEIPT))?,
        &root,
        &selected.join(VERIFY),
    )?;

    // Metadata, inventory and continuation must describe the same physical
    // producer files. Regeneration exercises Paths.out_dir, not only the join.
    let regenerated = shell(
        &foreign,
        &argv_path,
        command(&manifest, "workflow_manifest")?,
    )?;
    exact_argv(
        &regenerated,
        &[
            "agent",
            "start",
            "--root",
            text(&root)?,
            "--seam-id",
            BOUNDARY_GAP_SEAM_ID,
            "--out",
            text(&selected)?,
        ],
    );
    native_identity(&regenerated[7], &selected)?;
    let refreshed = read_json(&selected.join("workflow.json"))?;
    assert_eq!(refreshed["out_dir"], text(&selected)?);
    for (key, file) in [
        ("workflow_manifest", "workflow.json"),
        ("commands_markdown", "commands.md"),
        ("agent_brief", "agent-brief.json"),
    ] {
        assert_eq!(refreshed["outputs"][key], text(&selected.join(file))?);
    }
    for (name, file) in [
        ("before_snapshot", BEFORE),
        ("after_snapshot", AFTER),
        ("agent_verify", VERIFY),
    ] {
        let artifact = refreshed["artifacts"]
            .as_array()
            .and_then(|items| items.iter().find(|item| item["name"] == name))
            .ok_or("expected workflow artifact")?;
        assert_eq!(artifact["path"], text(&selected.join(file))?);
        assert_eq!(artifact["state"], "present");
    }
    assert_eq!(std::fs::read(selected.join(BEFORE))?, before_bytes);
    assert_eq!(std::fs::read(selected.join(AFTER))?, after_bytes);
    assert_eq!(std::fs::read(selected.join(VERIFY))?, verify_bytes);
    for (path, bytes) in source_custody.into_iter().chain(decoy_custody) {
        assert_eq!(
            std::fs::read(&path)?,
            bytes,
            "immutable source/decoy: {path:?}"
        );
    }
    assert!(!foreign.join("target").exists());
    let missing = selected.join("missing-verify.json");
    let refused = cli(
        &foreign,
        &[
            "agent",
            "receipt",
            "--root",
            text(&root)?,
            "--verify-json",
            text(&missing)?,
            "--seam-id",
            BOUNDARY_GAP_SEAM_ID,
            "--json",
        ],
    )?;
    assert_eq!(refused.status.code(), Some(2));
    assert!(refused.stdout.is_empty());
    assert!(!String::from_utf8(refused.stderr)?.contains("produce it with:"));
    std::fs::remove_dir_all(parent)?;
    Ok(())
}

#[test]
fn relative_custom_workflow_directory_keeps_native_producer_and_receipt_identity() -> TestResult {
    workflow_case(false)
}

#[test]
fn absolute_custom_workflow_directory_keeps_native_producer_and_receipt_identity() -> TestResult {
    workflow_case(true)
}
