//! #5399: genuine durable attempts, not hand-authored currentness flags.

use super::*;
use crate::app::repair_attempt::{
    BeforeArtifactSource, BeginRepairAttemptOptions, RepairAttemptId, begin_repair_attempt_with,
    edit_cage_policy_from_packet, finish_repair_attempt, load_repair_attempt_manifest,
    receipt_binding_from, retain_terminal_evidence, write_edit_cage_baseline,
};
use std::path::PathBuf;

/// Own only an exclusively created test directory, including setup failures
/// and assertion unwinds. Durable proof receipts live outside this fixture.
struct FixtureRoot(PathBuf);

impl std::ops::Deref for FixtureRoot {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!("remove freshness fixture {}: {error}", self.0.display());
        }
    }
}

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
    crate::testing::fixture_git::fixture_git_ok(root, args)
}

fn write(path: &Path, contents: &str) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|error| format!("write {}: {error}", path.display()))
}

fn snapshot(grip: &str) -> String {
    json!({ "schema_version": "0.3", "scope": "repo", "seams": [{
        "seam_id": "seam:freshness", "kind": "predicate_boundary",
        "file": "src/subject.rs", "line": 1, "grip_class": grip,
        "related_tests": [], "observed_values": [], "missing_discriminators": [],
    }] })
    .to_string()
}

fn prepared(label: &str) -> Result<(FixtureRoot, RepairAttemptId), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("fixture clock: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-mcp-freshness-{label}-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&root).map_err(|error| format!("create {}: {error}", root.display()))?;
    let root = FixtureRoot(root);
    std::fs::create_dir(root.join("tests")).map_err(|error| error.to_string())?;
    git(&root, &["init"])?;
    git(
        &root,
        &["config", "user.email", "ripr-test@example.invalid"],
    )?;
    git(&root, &["config", "user.name", "RIPR Test"])?;
    write(&root.join(".gitignore"), "/target/\n")?;
    write(&root.join("tests/target.rs"), "#[test]\nfn focused() {}\n")?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "--no-gpg-sign", "-m", "prepared"])?;
    let workflow = root.join("target/ripr/workflow");
    std::fs::create_dir_all(&workflow).map_err(|error| error.to_string())?;
    let before = workflow.join("before.json");
    let packet = workflow.join("packet.json");
    let baseline = workflow.join("baseline.json");
    write(&before, &snapshot("weakly_gripped"))?;
    let verify_spec = crate::agent::command_specs::agent_verify_command_spec(
        ".",
        "target/ripr/workflow/before.json",
        "target/ripr/workflow/after.json",
        None,
    );
    let receipt_spec = crate::agent::command_specs::agent_receipt_command_spec(
        ".",
        "target/ripr/workflow/agent-verify.json",
        "seam:freshness",
        None,
    );
    verify_spec.validate().map_err(|error| error.to_string())?;
    receipt_spec.validate().map_err(|error| error.to_string())?;
    let packet_text = json!({
        "seam_id": "seam:freshness",
        "allowed_edit_surface": ["tests/target.rs"],
        "forbidden_files": [],
        "command_specs": { "verify": verify_spec, "receipt": receipt_spec },
    })
    .to_string();
    write(&packet, &packet_text)?;
    let policy = edit_cage_policy_from_packet(&packet_text, "seam:freshness")?;
    write_edit_cage_baseline(&root, &baseline, &policy)?;
    let result = begin_repair_attempt_with(BeginRepairAttemptOptions {
        root: &root,
        root_argument: &root,
        seam_id: "seam:freshness",
        sources: &[
            BeforeArtifactSource {
                role: "before_snapshot",
                path: &before,
            },
            BeforeArtifactSource {
                role: "agent_packet",
                path: &packet,
            },
            BeforeArtifactSource {
                role: "edit_cage_baseline",
                path: &baseline,
            },
        ],
        expected_repository_head: None,
        next_command_suffix: None,
        store: None,
    })?;
    if result.manifest.state != RepairAttemptState::AwaitingEdit {
        return Err("fixture did not publish one awaiting attempt".to_string());
    }
    Ok((root, result.manifest.repair_attempt_id))
}

fn parity(root: &Path, id: &RepairAttemptId, expected: &str) -> Result<(Value, Value), String> {
    let session = WorkspaceSession::default();
    let attempt = session
        .repair_attempt_document(id.as_str(), Some(root), None)
        .map_err(|failure| failure.detail)?;
    let receipt = session
        .receipt_status_document(id.as_str(), Some(root), None)
        .map_err(|failure| failure.detail)?;
    let cli = crate::app::agent_status::build_agent_attempt_status(root, root, None, id)?;
    // Decision assertions come before additive metadata. The old implementation
    // must fail for offering continuation, not merely for a missing new field.
    if matches!(expected, "historical" | "unknown") {
        assert!(
            attempt["next_command"].is_null(),
            "old continuation escaped: {attempt}"
        );
        assert_eq!(attempt["command_routes"], json!([]));
        assert_eq!(
            receipt["status"],
            if expected == "historical" {
                "stale"
            } else {
                "limited"
            }
        );
    }
    for document in [&attempt, &receipt] {
        assert_eq!(document["currentness"]["state"], expected);
        assert_eq!(document["currentness"]["state"], cli.attempt.currentness);
        assert_eq!(
            document["currentness"]["head_current"],
            json!(cli.attempt.head_current)
        );
        assert_eq!(
            document["currentness"]["evidence_head"],
            json!(cli.attempt.evidence_head)
        );
    }
    assert_eq!(attempt["attempt_id"], id.as_str());
    assert_eq!(receipt["attempt"]["attempt_id"], id.as_str());
    Ok((attempt, receipt))
}

fn receipt_document(root: &Path, id: &RepairAttemptId) -> Result<Value, String> {
    WorkspaceSession::default()
        .receipt_status_document(id.as_str(), Some(root), None)
        .map_err(|failure| failure.detail)
}

fn with_unavailable_git<T>(
    root: &Path,
    observe: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    std::fs::rename(root.join(".git"), root.join(".git-unavailable"))
        .map_err(|error| error.to_string())?;
    // Cargo directs TMPDIR under target/, inside the checkout. Without a
    // local discovery barrier, Git could report the enclosing repository's
    // valid HEAD instead of the unreadable fixture HEAD this case requires.
    write(
        &root.join(".git"),
        "gitdir: .git-missing-for-freshness-test\n",
    )?;
    let observed = match crate::agent::artifact::current_git_head(root) {
        Ok(head) => Err(format!(
            "unavailable-HEAD fixture still discovered Git HEAD {head}"
        )),
        Err(_) => observe(),
    };
    // Preserve the discovery-barrier bytes in the fixture until closeout.
    std::fs::rename(root.join(".git"), root.join(".git-unavailable-marker"))
        .map_err(|error| error.to_string())?;
    std::fs::rename(root.join(".git-unavailable"), root.join(".git"))
        .map_err(|error| error.to_string())?;
    observed
}

#[test]
fn durable_currentness_keeps_ordinary_descendant_continuation() -> Result<(), String> {
    let (root, id) = prepared("descendant")?;
    let (before, _) = parity(&root, &id, "current")?;
    assert!(before["next_command"].is_string());
    assert_eq!(before["command_routes"].as_array().map(Vec::len), Some(2));
    write(
        &root.join("tests/target.rs"),
        "#[test]\nfn focused() { assert_eq!(1, 1); }\n",
    )?;
    git(&root, &["add", "tests/target.rs"])?;
    git(&root, &["commit", "--no-gpg-sign", "-m", "focused edit"])?;
    let (after, receipt) = parity(&root, &id, "current")?;
    assert_eq!(after["next_command"], before["next_command"]);
    assert_eq!(after["command_routes"], before["command_routes"]);
    assert_eq!(receipt["status"], "awaiting_edit");
    Ok(())
}

#[test]
fn durable_currentness_refuses_diverged_continuation_without_rewriting_attempt()
-> Result<(), String> {
    let (root, id) = prepared("diverged")?;
    let manifest_path = root.join(format!(
        "target/ripr/repair-attempts/{}/attempt.json",
        id.as_str()
    ));
    let bytes = std::fs::read(&manifest_path).map_err(|error| error.to_string())?;
    git(&root, &["checkout", "--orphan", "other-history"])?;
    git(&root, &["commit", "--no-gpg-sign", "-m", "unrelated root"])?;
    let (attempt, receipt) = parity(&root, &id, "historical")?;
    assert!(attempt["next_command"].is_null());
    assert_eq!(attempt["command_routes"], json!([]));
    assert_eq!(receipt["status"], "stale");
    assert_eq!(receipt["attempt"]["state"], "awaiting_edit");
    assert_eq!(
        std::fs::read(&manifest_path).map_err(|error| error.to_string())?,
        bytes
    );
    assert_eq!(parity(&root, &id, "historical")?, (attempt, receipt));
    Ok(())
}

#[test]
fn durable_currentness_does_not_reuse_recorded_after_admission() -> Result<(), String> {
    let (root, id) = prepared("terminal")?;
    finish_and_issue(&root, &id)?;
    let before = receipt_document(&root, &id)?;
    assert_eq!(before["status"], "improved");
    assert_eq!(before["receipt"]["status"], "issued");
    git(&root, &["add", "tests/target.rs"])?;
    git(&root, &["commit", "--no-gpg-sign", "-m", "later head"])?;
    let (_, receipt) = parity(&root, &id, "historical")?;
    assert_eq!(receipt["currentness"]["after_current"], true);
    assert_eq!(receipt["status"], "stale");
    assert_eq!(receipt["receipt"], before["receipt"]);
    Ok(())
}

fn finish_and_issue(root: &Path, id: &RepairAttemptId) -> Result<(), String> {
    write(
        &root.join("tests/target.rs"),
        "#[test]\nfn focused() { assert_eq!(1, 1); }\n",
    )?;
    let manifest = load_repair_attempt_manifest(root, id)?;
    let packet = find_manifest_artifact_by_role(&manifest, "agent_packet")
        .ok_or_else(|| "fixture lost retained packet".to_string())?;
    let packet_path = root.join(&packet.path);
    let after = finish_repair_attempt(
        root,
        id,
        &packet_path,
        crate::edit_cage::HeadMovement::AdmitDescendantCommits,
    )?;
    assert!(after.current);
    assert_eq!(
        load_repair_attempt_manifest(root, id)?.state,
        RepairAttemptState::ReadyToFinish
    );
    issue_terminal_receipt(root, id, &packet_path)
}

fn issue_terminal_receipt(root: &Path, id: &RepairAttemptId, packet: &Path) -> Result<(), String> {
    use crate::output::agent_receipt::{
        AgentReceiptAnalysisOutcome, AgentReceiptArtifactProvenance, AgentReceiptProvenance,
        render_agent_receipt_value_json,
    };
    let before_path = "target/ripr/workflow/before.json";
    let after_path = "target/ripr/workflow/after.json";
    let verify_path = "target/ripr/workflow/agent-verify.json";
    let receipt_path = "target/ripr/workflow/agent-receipt.json";
    let before =
        std::fs::read_to_string(root.join(before_path)).map_err(|error| error.to_string())?;
    let after = snapshot("strongly_gripped");
    write(&root.join(after_path), &after)?;
    let report = crate::output::outcome::targeted_test_outcome_report_from_json(
        &before,
        &after,
        before_path.to_string(),
        after_path.to_string(),
    )?;
    let digest = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let binding = crate::output::outcome::AgentVerifyArtifactBinding {
        before_content_sha256: digest(before.as_bytes()),
        after_content_sha256: digest(after.as_bytes()),
    };
    let verify = crate::output::outcome::render_agent_verify_json_with_currentness(
        &report,
        Some("current"),
        &binding,
    )?;
    let verify_value: Value = serde_json::from_str(&verify).map_err(|error| error.to_string())?;
    assert_eq!(
        verify_value["changed_seams"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        verify_value["changed_seams"][0]["seam_id"],
        "seam:freshness"
    );
    assert_eq!(verify_value["changed_seams"][0]["change"], "improved");
    write(&root.join(verify_path), &verify)?;
    let artifact = |path: &str, bytes: &[u8]| AgentReceiptArtifactProvenance {
        path: path.to_string(),
        sha256: digest(bytes),
    };
    let outcome = crate::analysis_outcome::AnalysisOutcome::new(
        crate::analysis_outcome::AnalysisOutcomeKind::CompleteWithFindings,
        Default::default(),
        crate::analysis_outcome::AnalysisOutcomeCounts {
            finding_count: 1,
            ..Default::default()
        },
        Vec::new(),
    )?;
    let rendered = render_agent_receipt_value_json(
        &verify_value,
        verify_path.to_string(),
        "seam:freshness",
        None,
        &[],
        AgentReceiptProvenance {
            ripr_version: env!("CARGO_PKG_VERSION").to_string(),
            repo_root: ".".to_string(),
            config_fingerprint: None,
            command_template_version: "fixture".to_string(),
            generated_at: "2026-10-04T00:00:00Z".to_string(),
            workflow_artifact: None,
            before_artifact: artifact(before_path, before.as_bytes()),
            after_artifact: artifact(after_path, after.as_bytes()),
            verify_artifact: artifact(verify_path, verify.as_bytes()),
        },
        AgentReceiptAnalysisOutcome::Present(Box::new(outcome)),
    )?;
    let mut receipt: Value = serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
    // Exactly the CLI producer binding path, before immutable retention.
    receipt["repair_attempt"] =
        receipt_binding_from(root, None, "seam:freshness", packet, Some(id.as_str()))?;
    write(&root.join(receipt_path), &receipt.to_string())?;
    retain_terminal_evidence(
        root,
        id,
        &[
            BeforeArtifactSource {
                role: "agent_verify",
                path: &root.join(verify_path),
            },
            BeforeArtifactSource {
                role: "agent_receipt",
                path: &root.join(receipt_path),
            },
        ],
    )?;
    let manifest = load_repair_attempt_manifest(root, id)?;
    assert!(matches!(
        load_attempt_terminal_receipt(root, &manifest),
        AttemptTerminalReceipt::Issued { .. }
    ));
    Ok(())
}

#[test]
fn durable_currentness_issued_receipt_at_unknown_head_is_limited() -> Result<(), String> {
    let (root, id) = prepared("issued-unknown")?;
    finish_and_issue(&root, &id)?;
    let before = receipt_document(&root, &id)?;
    assert_eq!(before["status"], "improved");
    let (_, receipt) = with_unavailable_git(&root, || parity(&root, &id, "unknown"))?;
    assert_eq!(receipt["receipt"], before["receipt"]);
    Ok(())
}

#[test]
fn durable_currentness_keeps_tampered_terminal_evidence_invalid_at_historical_head()
-> Result<(), String> {
    let (root, id) = prepared("tampered")?;
    finish_and_issue(&root, &id)?;
    let before = receipt_document(&root, &id)?;
    assert_eq!(before["receipt"]["status"], "issued");
    let manifest = load_repair_attempt_manifest(&root, &id)?;
    let verify = find_terminal_artifact_by_role(&manifest, "agent_verify")
        .ok_or_else(|| "fixture lost retained verify".to_string())?;
    // Deliberate negative corruption of this fixture only, never a new
    // producer status or a replacement receipt.
    write(&root.join(&verify.path), "{\"tampered\":true}")?;
    git(&root, &["add", "tests/target.rs"])?;
    git(&root, &["commit", "--no-gpg-sign", "-m", "later head"])?;
    let receipt = WorkspaceSession::default()
        .receipt_status_document(id.as_str(), Some(&root), None)
        .map_err(|failure| failure.detail)?;
    assert_eq!(receipt["status"], "invalid");
    assert_eq!(receipt["receipt"]["status"], "unavailable");
    assert_eq!(receipt["currentness"]["state"], "historical");
    Ok(())
}

#[test]
fn durable_currentness_unknown_head_never_offers_continuation() -> Result<(), String> {
    let (root, id) = prepared("unknown")?;
    let (attempt, receipt) = with_unavailable_git(&root, || parity(&root, &id, "unknown"))?;
    assert!(attempt["next_command"].is_null());
    assert_eq!(attempt["command_routes"], json!([]));
    assert_eq!(receipt["status"], "limited");
    Ok(())
}
