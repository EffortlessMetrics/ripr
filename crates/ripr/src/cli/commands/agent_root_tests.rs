//! #5744: real snapshot/verify writers feed the unchanged receipt admission.

use super::*;
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};

struct OwnedRoot(PathBuf);

impl Drop for OwnedRoot {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!(
                "remove owned CLI root fixture {}: {error}",
                self.0.display()
            );
        }
    }
}

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
    crate::testing::fixture_git::fixture_git_ok(root, args)
}

#[test]
fn cli_snapshot_verify_absolute_inputs_retain_literal_unix_root() -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let parent = std::env::temp_dir().join(format!("ripr-cli-root-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&parent).map_err(|error| error.to_string())?;
    let _owned = OwnedRoot(parent.clone());
    let root = parent.join("team\\repo 'quoted'");
    std::fs::create_dir(&root).map_err(|error| error.to_string())?;
    std::fs::create_dir(root.join("src")).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"literal_root_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn identity(value: bool) -> bool { value }\n",
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join(".gitignore"), "/target/\n").map_err(|error| error.to_string())?;
    git(&root, &["init"])?;
    git(&root, &["config", "user.name", "RIPR test"])?;
    git(
        &root,
        &["config", "user.email", "ripr-test@example.invalid"],
    )?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "--no-gpg-sign", "-m", "before"])?;
    let before = root.join("target/identity/before.json");
    let after = root.join("target/identity/after.json");
    write_agent_repo_exposure_snapshot(&root, &before)?;
    // Real descendant movement, with identical producer-consumed manifests/config.
    std::fs::write(root.join("README.md"), "fixture revision movement\n")
        .map_err(|error| error.to_string())?;
    git(&root, &["add", "README.md"])?;
    git(&root, &["commit", "--no-gpg-sign", "-m", "after"])?;
    write_agent_repo_exposure_snapshot(&root, &after)?;
    let before_bytes = std::fs::read(&before).map_err(|error| error.to_string())?;
    let after_bytes = std::fs::read(&after).map_err(|error| error.to_string())?;
    let decoy_parent = parent.join("team");
    std::fs::create_dir(&decoy_parent).map_err(|error| error.to_string())?;
    let decoy = decoy_parent.join("repo 'quoted'");
    git(
        &parent,
        &[
            "clone",
            "--quiet",
            "--no-hardlinks",
            root.to_str()
                .ok_or_else(|| "fixture root is not UTF-8".to_string())?,
            decoy
                .to_str()
                .ok_or_else(|| "fixture decoy is not UTF-8".to_string())?,
        ],
    )?;
    let selected_meta = std::fs::metadata(&root).map_err(|error| error.to_string())?;
    let decoy_meta = std::fs::metadata(&decoy).map_err(|error| error.to_string())?;
    assert_ne!(
        (selected_meta.dev(), selected_meta.ino()),
        (decoy_meta.dev(), decoy_meta.ino())
    );
    assert_eq!(
        crate::agent::artifact::current_git_head(&root)?,
        crate::agent::artifact::current_git_head(&decoy)?
    );
    let decoy_snapshots = decoy.join("target/identity");
    std::fs::create_dir_all(&decoy_snapshots).map_err(|error| error.to_string())?;
    std::fs::copy(&before, decoy_snapshots.join("before.json"))
        .map_err(|error| error.to_string())?;
    std::fs::copy(&after, decoy_snapshots.join("after.json")).map_err(|error| error.to_string())?;
    let verify = render_agent_verify(&AgentVerifyOptions {
        root: root.clone(),
        before: before.clone(),
        after: after.clone(),
        json: true,
    })?;
    let value: serde_json::Value =
        serde_json::from_str(&verify).map_err(|error| error.to_string())?;
    assert_eq!(
        value["inputs"]["before"].as_str().map(str::as_bytes),
        Some(before.as_os_str().as_bytes()),
        "verify before input lost selected Unix root"
    );
    assert_eq!(
        value["inputs"]["after"].as_str().map(str::as_bytes),
        Some(after.as_os_str().as_bytes()),
        "verify after input lost selected Unix root"
    );
    app::agent_receipt::validate_agent_receipt_verify_json(&root, &verify)?;
    let foreign_refusal = app::agent_receipt::validate_agent_receipt_verify_json(&decoy, &verify)
        .err()
        .ok_or_else(|| "verify was admitted in slash-path decoy".to_string())?;
    assert!(foreign_refusal.contains("must stay under root"));
    let relative = render_agent_verify(&AgentVerifyOptions {
        root: root.clone(),
        before: PathBuf::from("target/identity/before.json"),
        after: PathBuf::from("target/identity/after.json"),
        json: true,
    })?;
    app::agent_receipt::validate_agent_receipt_verify_json(&root, &relative)?;
    assert_eq!(
        std::fs::read(&before).map_err(|error| error.to_string())?,
        before_bytes
    );
    assert_eq!(
        std::fs::read(&after).map_err(|error| error.to_string())?,
        after_bytes
    );
    Ok(())
}

fn first_action_for_receipt(
    root: &Path,
    receipt_path: &Path,
    receipt: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let receipt_json =
        serde_json::to_string(receipt).map_err(|error| format!("serialize receipt: {error}"))?;
    let report = crate::output::first_useful_action::build_first_useful_action_report(
        crate::output::first_useful_action::FirstUsefulActionInput {
            root: root.to_string_lossy().to_string(),
            generated_at: "2026-10-04T00:00:00Z".to_string(),
            pr_guidance_path: None,
            assistant_proof_path: None,
            gap_ledger_path: None,
            ledger_path: None,
            baseline_delta_path: None,
            receipt_path: Some(receipt_path.to_string_lossy().to_string()),
            gate_decision_path: None,
            coverage_frontier_path: None,
            editor_context_path: None,
            pr_guidance_json: None,
            assistant_proof_json: None,
            gap_ledger_json: None,
            ledger_json: None,
            baseline_delta_json: None,
            receipt_json: Some(Ok(receipt_json)),
            gate_decision_json: None,
            coverage_frontier_json: None,
            editor_context_json: None,
        },
    );
    let rendered = crate::output::first_useful_action::render_first_useful_action_json(&report)?;
    serde_json::from_str(&rendered).map_err(|error| format!("parse rendered first action: {error}"))
}

/// #6313: issue a real CLI receipt from an eligible same-HEAD dirty pair and
/// send it through first-action's actual provenance/currentness admission.
/// The prior descendant-head witness is deliberately ineligible here.
#[test]
fn cli_receipt_first_action_reopens_literal_unix_root_and_refuses_decoy() -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let parent = std::env::temp_dir().join(format!(
        "ripr-first-action-root-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&parent).map_err(|error| error.to_string())?;
    let _owned = OwnedRoot(parent.clone());
    let root = parent.join("team\\repo 'quoted'");
    std::fs::create_dir(&root).map_err(|error| error.to_string())?;
    std::fs::create_dir(root.join("src")).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"first_action_root_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn identity(value: bool) -> bool { value }\n",
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join(".gitignore"), "/target/\n").map_err(|error| error.to_string())?;
    git(&root, &["init"])?;
    git(&root, &["config", "user.name", "RIPR test"])?;
    git(
        &root,
        &["config", "user.email", "ripr-test@example.invalid"],
    )?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "--no-gpg-sign", "-m", "baseline"])?;
    git(&root, &["checkout", "-B", "main"])?;
    git(&root, &["checkout", "-b", "feature"])?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn identity(value: bool) -> bool { if value { true } else { false } }\n",
    )
    .map_err(|error| error.to_string())?;
    git(&root, &["add", "src/lib.rs"])?;
    git(
        &root,
        &["commit", "--no-gpg-sign", "-m", "branch under review"],
    )?;
    // Keep the same HEAD while changing the worktree between the two real
    // snapshot writers; both artifacts must be dirty, not historical/current.
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn identity(value: bool) -> bool { if value { true } else { false } } // dirty review context\n",
    )
    .map_err(|error| error.to_string())?;
    let before = root.join("target/ripr/workflow/before.repo-exposure.json");
    let after = root.join("target/ripr/workflow/after.repo-exposure.json");
    write_agent_repo_exposure_snapshot(&root, &before)?;
    std::fs::create_dir_all(root.join("tests")).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("tests/focused_test.rs"),
        "use first_action_root_fixture::identity;\n#[test]\nfn focused() { assert!(identity(true)); }\n",
    )
    .map_err(|error| error.to_string())?;
    write_agent_repo_exposure_snapshot(&root, &after)?;
    let verify_path = root.join("target/ripr/workflow/agent-verify.json");
    let verify_text = render_agent_verify(&AgentVerifyOptions {
        root: root.clone(),
        before: before.clone(),
        after: after.clone(),
        json: true,
    })?;
    let verify: serde_json::Value =
        serde_json::from_str(&verify_text).map_err(|error| error.to_string())?;
    assert_eq!(verify["artifact_currentness"], "dirty_both");
    let seam_id = ["changed_seams", "unchanged_seams"]
        .into_iter()
        .find_map(|bucket| {
            verify[bucket]
                .as_array()
                .and_then(|seams| seams.first())
                .and_then(|seam| seam["seam_id"].as_str())
        })
        .ok_or("the real verify pair did not produce a routable seam")?
        .to_string();
    std::fs::write(&verify_path, &verify_text).map_err(|error| error.to_string())?;
    let analyzed_diff = crate::analysis::load_diff(&root, Some("main"), None, None)?;
    assert!(
        analyzed_diff.contains("src/lib.rs"),
        "complete outcome must analyze the committed source change"
    );
    write_agent_analysis_outcome(&root)?;
    let analysis_path = root.join("target/ripr/workflow/analysis-outcome.json");
    let analysis_text = std::fs::read_to_string(&analysis_path).map_err(|e| e.to_string())?;
    let analysis: serde_json::Value =
        serde_json::from_str(&analysis_text).map_err(|e| e.to_string())?;
    assert_eq!(analysis["analysis_outcome"]["analysis_complete"], true);
    let receipt_path = root.join("target/ripr/workflow/agent-receipt.json");
    run_agent_receipt(AgentReceiptOptions {
        root: root.clone(),
        verify_json: verify_path.clone(),
        seam_id,
        attempt_id: None,
        test_changed: Some("tests/focused_test.rs".to_string()),
        commands_run: Vec::new(),
        json: true,
        out: Some(receipt_path.clone()),
    })?;
    let receipt_text = std::fs::read_to_string(&receipt_path).map_err(|e| e.to_string())?;
    let receipt: serde_json::Value =
        serde_json::from_str(&receipt_text).map_err(|e| e.to_string())?;
    assert_eq!(receipt["analysis_outcome_status"], "complete");
    let movement = receipt["provenance"]["movement"]
        .as_str()
        .ok_or("issued receipt omitted movement")?;
    let expected_status = match movement {
        "improved" | "resolved" => "already_improved",
        "unchanged" => "unchanged_after_attempt",
        other => return Err(format!("issued receipt movement is not routable: {other}")),
    };

    // Clone the exact committed HEAD into a separately initialized slash-path
    // directory and copy the authentic immutable evidence, so reopening a
    // normalized locator cannot accidentally look like a missing file only.
    let decoy_parent = parent.join("team");
    std::fs::create_dir(&decoy_parent).map_err(|error| error.to_string())?;
    let decoy = decoy_parent.join("repo 'quoted'");
    git(
        &parent,
        &[
            "clone",
            "--quiet",
            "--no-hardlinks",
            root.to_str().ok_or("fixture root is not UTF-8")?,
            decoy.to_str().ok_or("fixture decoy is not UTF-8")?,
        ],
    )?;
    assert_eq!(
        crate::agent::artifact::current_git_head(&root)?,
        crate::agent::artifact::current_git_head(&decoy)?
    );
    let selected_meta = std::fs::metadata(&root).map_err(|e| e.to_string())?;
    let decoy_meta = std::fs::metadata(&decoy).map_err(|e| e.to_string())?;
    assert_ne!(
        (selected_meta.dev(), selected_meta.ino()),
        (decoy_meta.dev(), decoy_meta.ino())
    );
    let decoy_workflow = decoy.join("target/ripr/workflow");
    std::fs::create_dir_all(&decoy_workflow).map_err(|e| e.to_string())?;
    for name in [
        "before.repo-exposure.json",
        "after.repo-exposure.json",
        "agent-verify.json",
        "analysis-outcome.json",
        "agent-receipt.json",
    ] {
        std::fs::copy(
            root.join("target/ripr/workflow").join(name),
            decoy_workflow.join(name),
        )
        .map_err(|e| e.to_string())?;
    }
    std::fs::write(decoy.join("identity"), "different repository\n").map_err(|e| e.to_string())?;
    let retained = [
        before.as_path(),
        after.as_path(),
        verify_path.as_path(),
        analysis_path.as_path(),
        receipt_path.as_path(),
    ]
    .map(|path| std::fs::read(path).map_err(|e| e.to_string()))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;

    let report = first_action_for_receipt(&root, &receipt_path, &receipt)?;
    let warnings = report["warnings"].to_string();
    if report["status"] == "missing_required_artifact" && !warnings.contains("receipt repo_root") {
        return Err(format!(
            "first-action did not reach the inherited root-provenance failure: {warnings}"
        ));
    }
    assert_eq!(
        report["status"], expected_status,
        "eligible authentic receipt failed first-action: {warnings}"
    );
    for (field, expected) in [
        ("repo_root", root.as_path()),
        ("before_artifact", before.as_path()),
        ("after_artifact", after.as_path()),
        ("verify_artifact", verify_path.as_path()),
    ] {
        let actual = if field == "repo_root" {
            receipt["provenance"][field].as_str()
        } else {
            receipt["provenance"][field]["path"].as_str()
        };
        assert_eq!(
            actual.map(str::as_bytes),
            Some(expected.as_os_str().as_bytes()),
            "issued receipt {field} lost native Unix identity"
        );
    }
    assert_eq!(
        receipt["inputs"]["agent_verify_json"]
            .as_str()
            .map(str::as_bytes),
        Some(verify_path.as_os_str().as_bytes()),
        "issued receipt verify locator lost native Unix identity"
    );

    let foreign =
        first_action_for_receipt(&decoy, &decoy_workflow.join("agent-receipt.json"), &receipt)?;
    assert_eq!(foreign["status"], "missing_required_artifact");
    let mut changed_locator = receipt.clone();
    changed_locator["provenance"]["before_artifact"]["path"] = serde_json::Value::String(
        decoy_workflow
            .join("before.repo-exposure.json")
            .to_string_lossy()
            .to_string(),
    );
    assert_eq!(
        first_action_for_receipt(&root, &receipt_path, &changed_locator)?["status"],
        "missing_required_artifact",
        "foreign before locator was admitted"
    );
    let mut changed_digest = receipt.clone();
    changed_digest["provenance"]["before_artifact"]["sha256"] =
        serde_json::Value::String("0".repeat(64));
    assert_eq!(
        first_action_for_receipt(&root, &receipt_path, &changed_digest)?["status"],
        "missing_required_artifact",
        "changed content digest was admitted"
    );
    let mut changed_verify = receipt.clone();
    changed_verify["inputs"]["agent_verify_json"] = serde_json::Value::String(
        decoy_workflow
            .join("agent-verify.json")
            .to_string_lossy()
            .to_string(),
    );
    assert_eq!(
        first_action_for_receipt(&root, &receipt_path, &changed_verify)?["status"],
        "missing_required_artifact",
        "foreign verify locator was admitted"
    );
    for (path, bytes) in [
        before.as_path(),
        after.as_path(),
        verify_path.as_path(),
        analysis_path.as_path(),
        receipt_path.as_path(),
    ]
    .into_iter()
    .zip(retained)
    {
        assert_eq!(std::fs::read(path).map_err(|e| e.to_string())?, bytes);
    }
    assert_eq!(
        std::fs::read(decoy_workflow.join("agent-verify.json")).map_err(|e| e.to_string())?,
        verify_text.as_bytes()
    );
    assert_eq!(
        std::fs::read(decoy_workflow.join("agent-receipt.json")).map_err(|e| e.to_string())?,
        receipt_text.as_bytes()
    );
    assert_eq!(
        std::fs::read(decoy.join("identity")).map_err(|e| e.to_string())?,
        b"different repository\n"
    );
    Ok(())
}
