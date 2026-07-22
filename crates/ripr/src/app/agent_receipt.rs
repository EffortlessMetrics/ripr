//! Validated static agent-verify input for receipt issuance.

use crate::output;
use std::path::{Path, PathBuf};

pub(crate) struct ValidatedAgentReceiptVerify {
    pub(crate) input_paths: output::agent_receipt::AgentReceiptInputPaths,
    pub(crate) verify: serde_json::Value,
}

pub(crate) fn validate_agent_receipt_verify_json(
    root: &Path,
    verify_json: &str,
) -> Result<ValidatedAgentReceiptVerify, String> {
    let verify: serde_json::Value = serde_json::from_str(verify_json).map_err(|err| {
        receipt_verify_input_error("malformed", format!("verify JSON is not valid JSON: {err}"))
    })?;
    let input_paths = output::agent_receipt::agent_receipt_input_paths_from_value(&verify)
        .map_err(|err| receipt_verify_input_error("malformed", err))?;
    let before_path =
        validate_snapshot_path(root, Path::new(&input_paths.before), "receipt --before")
            .map_err(|err| receipt_verify_input_error("path", err))?;
    let after_path = validate_snapshot_path(root, Path::new(&input_paths.after), "receipt --after")
        .map_err(|err| receipt_verify_input_error("path", err))?;
    let before_json = read_snapshot(&before_path, "receipt before")
        .map_err(|err| receipt_verify_input_error("artifact_read", err))?;
    let after_json = read_snapshot(&after_path, "receipt after")
        .map_err(|err| receipt_verify_input_error("artifact_read", err))?;
    let before_identity = crate::agent::artifact::validate_repo_exposure_artifact(
        root,
        &before_json,
        "receipt before",
    )
    .map_err(|err| receipt_verify_input_error("artifact", err))?;
    let after_identity =
        crate::agent::artifact::validate_repo_exposure_artifact(root, &after_json, "receipt after")
            .map_err(|err| receipt_verify_input_error("artifact", err))?;
    if before_identity.base_revision != after_identity.base_revision {
        return Err(receipt_verify_input_error(
            "incomparable_base_revision",
            "agent receipt verify JSON references incomparable artifacts: base revisions differ",
        ));
    }
    if before_identity.input_identity != after_identity.input_identity {
        return Err(receipt_verify_input_error(
            "incomparable_analysis_inputs",
            "agent receipt verify JSON references incomparable artifacts: analysis input identities differ",
        ));
    }
    let artifact_currentness = match (&before_identity.currentness, &after_identity.currentness) {
        (
            crate::agent::artifact::ArtifactCurrentness::Current,
            crate::agent::artifact::ArtifactCurrentness::Current,
        ) => "current",
        (
            crate::agent::artifact::ArtifactCurrentness::Historical,
            crate::agent::artifact::ArtifactCurrentness::Historical,
        ) => "historical_noncurrent",
        _ => "dirty_worktree",
    };
    let report = output::outcome::targeted_test_outcome_report_from_json(
        &before_json,
        &after_json,
        input_paths.before.clone(),
        input_paths.after.clone(),
    )
    .map_err(|err| receipt_verify_input_error("movement_recompute", err))?;
    let canonical = output::outcome::render_agent_verify_json_with_currentness(
        &report,
        Some(artifact_currentness),
    )
    .map_err(|err| receipt_verify_input_error("canonical_render", err))?;
    // Fail-closed on bytes, not on parsed values: the supplied document must be
    // the exact canonical rendering (up to trailing newlines), so hand-authored
    // JSON with identical values but different key order or spacing is rejected.
    let supplied = verify_json.trim_end_matches('\n');
    let canonical_body = canonical.trim_end_matches('\n');
    if supplied != canonical_body {
        return Err(receipt_verify_input_error(
            "not_canonical",
            "agent receipt verify JSON is not canonical output from ripr agent verify; rerun agent verify from the bound artifacts",
        ));
    }
    let verify: serde_json::Value = serde_json::from_str(canonical_body).map_err(|err| {
        receipt_verify_input_error(
            "canonical_render",
            format!("canonical agent verify JSON is not valid JSON: {err}"),
        )
    })?;

    Ok(ValidatedAgentReceiptVerify {
        input_paths,
        verify,
    })
}

fn validate_snapshot_path(root: &Path, path: &Path, flag: &str) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent receipt root {} failed: {err}",
            root.display()
        )
    })?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let candidate = candidate.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent receipt {flag} {} failed: {err}",
            path.display()
        )
    })?;
    if !candidate.starts_with(&root) {
        return Err(format!(
            "agent receipt {flag} {} must stay under root {}",
            path.display(),
            root.display()
        ));
    }
    Ok(candidate)
}

fn read_snapshot(path: &Path, label: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|err| {
        format!(
            "read agent receipt {label} snapshot {} failed: {err}",
            output::outcome::display_path(path)
        )
    })
}

fn receipt_verify_input_error(kind: &str, detail: impl std::fmt::Display) -> String {
    format!("agent receipt verify input [{kind}]: {detail}")
}
