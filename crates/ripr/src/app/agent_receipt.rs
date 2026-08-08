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
    // Fail closed on schema identity before any other check (#2922 PR B):
    // only the current agent-verify schema carries the artifact
    // content-commitment binding the canonical comparison relies on. Older
    // documents are unverifiable-by-shape and newer ones are unknown; both
    // get one bounded typed rejection.
    let schema_version = verify
        .get("schema_version")
        .and_then(serde_json::Value::as_str);
    if schema_version != Some(output::outcome::AGENT_VERIFY_SCHEMA_VERSION) {
        return Err(receipt_verify_input_error(
            "unsupported_schema",
            format!(
                "agent receipt verify JSON has unsupported schema version `{}`; expected `{}` from ripr agent verify",
                schema_version.unwrap_or("<missing>"),
                output::outcome::AGENT_VERIFY_SCHEMA_VERSION
            ),
        ));
    }
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
    if let Err(reason) =
        crate::agent::artifact::validate_comparable_pair(&before_identity, &after_identity)
    {
        let kind = if reason.contains("base revisions") {
            "incomparable_base_revision"
        } else {
            "incomparable_analysis_inputs"
        };
        return Err(receipt_verify_input_error(
            kind,
            format!("agent receipt verify JSON references incomparable artifacts: {reason}"),
        ));
    }
    // The receipt path re-runs the same pair and lineage authority as agent
    // verify (#2922 PR A) so a replayed or fabricated verify JSON cannot
    // bypass ordering or movement requirements.
    if let Err(reason) =
        crate::agent::artifact::validate_pair_lineage(root, &before_identity, &after_identity)
    {
        return Err(receipt_verify_input_error(
            "incomparable_lineage",
            format!("agent receipt verify JSON references incomparable artifacts: {reason}"),
        ));
    }
    if let Err(reason) =
        crate::agent::artifact::validate_verify_movement(&before_identity, &after_identity)
    {
        return Err(receipt_verify_input_error(
            "no_movement",
            format!(
                "agent receipt verify JSON references artifacts without repository movement: {reason}"
            ),
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
    // Recompute the same artifact binding the producer emits (#2922 PR B);
    // the canonical byte comparison below is the single authority that
    // rejects a verify JSON replayed against different or mutated artifact
    // bytes.
    let binding = output::outcome::AgentVerifyArtifactBinding {
        before_content_sha256: before_identity.content_sha256.clone(),
        after_content_sha256: after_identity.content_sha256.clone(),
    };
    let canonical = output::outcome::render_agent_verify_json_with_currentness(
        &report,
        Some(artifact_currentness),
        &binding,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_verify_rejects_unsupported_schema_before_any_io() -> Result<(), String> {
        // The schema gate fires before path, artifact, or Git work, so an
        // unsupported document is rejected for one bounded reason even when
        // nothing else about the input could be read (#2922 PR B).
        for (case, document) in [
            ("missing schema_version", r#"{"tool":"ripr"}"#),
            ("older schema 0.1", r#"{"schema_version":"0.1"}"#),
            ("newer schema", r#"{"schema_version":"9.9"}"#),
            ("non-string schema", r#"{"schema_version":2}"#),
        ] {
            match validate_agent_receipt_verify_json(Path::new("."), document) {
                Err(error) if error.contains("[unsupported_schema]") => {}
                Err(error) => {
                    return Err(format!(
                        "{case}: expected [unsupported_schema], got a different rejection: {error}"
                    ));
                }
                Ok(_) => {
                    return Err(format!(
                        "{case}: unsupported verify schema must be rejected, not validated"
                    ));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn receipt_verify_malformed_json_stays_typed_malformed() -> Result<(), String> {
        match validate_agent_receipt_verify_json(Path::new("."), "{not json") {
            Err(error) if error.contains("[malformed]") => Ok(()),
            Err(error) => Err(format!("expected [malformed], got: {error}")),
            Ok(_) => Err("malformed verify JSON must be rejected".to_string()),
        }
    }
}
