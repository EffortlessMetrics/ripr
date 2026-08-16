//! Shared validation for the producer-owned diff analysis artifact.
//!
//! Consumers may project this artifact, but they must not reconstruct its
//! completeness or identity from findings, packets, or exit status.

use crate::analysis_outcome::AnalysisOutcome;
use crate::app::CHECK_OUTPUT_SCHEMA_VERSION;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AnalysisOutcomeArtifactError {
    Missing(String),
    Invalid(String),
}

impl AnalysisOutcomeArtifactError {
    pub(crate) fn status(&self) -> &'static str {
        match self {
            Self::Missing(_) => "missing",
            Self::Invalid(_) => "invalid",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AnalysisOutcomeUnavailableStatus {
    Missing,
    Invalid,
}

impl AnalysisOutcomeUnavailableStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AnalysisOutcomeProjection {
    pub(crate) status: &'static str,
    pub(crate) outcome: Value,
    pub(crate) error: Option<String>,
}

pub(crate) fn analysis_outcome_projection(
    analysis_outcome: Option<&AnalysisOutcome>,
    required: bool,
) -> AnalysisOutcomeProjection {
    let Some(analysis_outcome) = analysis_outcome else {
        return if required {
            AnalysisOutcomeProjection {
                status: "missing",
                outcome: Value::Null,
                error: Some(
                    "diff-scoped agent packet requires the producer AnalysisOutcome artifact"
                        .to_string(),
                ),
            }
        } else {
            AnalysisOutcomeProjection {
                status: "not_applicable",
                outcome: Value::Null,
                error: None,
            }
        };
    };

    let outcome_value = match serde_json::to_value(analysis_outcome) {
        Ok(value) => value,
        Err(error) => {
            return AnalysisOutcomeProjection {
                status: "invalid",
                outcome: Value::Null,
                error: Some(format!(
                    "serialize producer AnalysisOutcome failed: {error}"
                )),
            };
        }
    };
    let digest = match analysis_outcome.semantic_digest() {
        Ok(digest) => digest,
        Err(error) => {
            return AnalysisOutcomeProjection {
                status: "invalid",
                outcome: Value::Null,
                error: Some(format!(
                    "compute producer AnalysisOutcome digest failed: {error}"
                )),
            };
        }
    };

    AnalysisOutcomeProjection {
        status: if analysis_outcome.kind.is_complete() {
            "complete"
        } else {
            "incomplete"
        },
        outcome: serde_json::json!({
            "analysis_complete": analysis_outcome.kind.is_complete(),
            "outcome": outcome_value,
            "semantic_digest": digest,
        }),
        error: None,
    }
}

pub(crate) fn unavailable_analysis_outcome_projection(
    status: AnalysisOutcomeUnavailableStatus,
    reason: &str,
) -> AnalysisOutcomeProjection {
    AnalysisOutcomeProjection {
        status: status.as_str(),
        outcome: Value::Null,
        error: Some(reason.to_string()),
    }
}

impl std::fmt::Display for AnalysisOutcomeArtifactError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(reason) | Self::Invalid(reason) => formatter.write_str(reason),
        }
    }
}

pub(crate) fn read_analysis_outcome_artifact_at(
    root: &Path,
    root_display: &str,
    path: &Path,
) -> Result<AnalysisOutcome, AnalysisOutcomeArtifactError> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        let message = format!(
            "Analysis outcome artifact {} is unavailable: {error}.",
            path.display()
        );
        if error.kind() == std::io::ErrorKind::NotFound {
            AnalysisOutcomeArtifactError::Missing(message)
        } else {
            AnalysisOutcomeArtifactError::Invalid(message)
        }
    })?;
    let value = serde_json::from_str::<Value>(&text).map_err(|error| {
        AnalysisOutcomeArtifactError::Invalid(format!(
            "Analysis outcome artifact {} is malformed JSON: {error}.",
            path.display()
        ))
    })?;
    validate_analysis_outcome_artifact(root, root_display, &value)
}

pub(crate) fn analysis_outcome_artifact_path_for_verify(
    verify_path: &Path,
) -> Result<std::path::PathBuf, String> {
    verify_path
        .parent()
        .map(|parent| parent.join("analysis-outcome.json"))
        .ok_or_else(|| "agent verify path has no parent directory".to_string())
}

pub(crate) fn validate_analysis_outcome_artifact(
    root: &Path,
    root_display: &str,
    value: &Value,
) -> Result<AnalysisOutcome, AnalysisOutcomeArtifactError> {
    if value.get("tool").and_then(Value::as_str) != Some("ripr") {
        return Err(invalid(
            "Analysis outcome artifact has an unknown producer tool.",
        ));
    }
    if value.get("schema_version").and_then(Value::as_str) != Some(CHECK_OUTPUT_SCHEMA_VERSION) {
        return Err(invalid(format!(
            "Analysis outcome artifact has unsupported schema_version; expected {}.",
            CHECK_OUTPUT_SCHEMA_VERSION
        )));
    }
    if value.get("root").and_then(Value::as_str) != Some(root_display) {
        return Err(invalid(
            "Analysis outcome artifact root does not match the review root.",
        ));
    }
    let Some(envelope) = value.get("analysis_outcome") else {
        return Err(invalid(
            "Analysis outcome artifact is missing the analysis_outcome envelope.",
        ));
    };
    let Some(declared_complete) = envelope.get("analysis_complete").and_then(Value::as_bool) else {
        return Err(invalid(
            "Analysis outcome artifact is missing boolean analysis_complete.",
        ));
    };
    let Some(outcome) = envelope.get("outcome") else {
        return Err(invalid(
            "Analysis outcome artifact is missing the typed outcome.",
        ));
    };
    let outcome = serde_json::from_value::<AnalysisOutcome>(outcome.clone()).map_err(|error| {
        invalid(format!(
            "Analysis outcome artifact has an invalid typed outcome: {error}."
        ))
    })?;
    if declared_complete != outcome.kind.is_complete() {
        return Err(invalid(
            "Analysis outcome artifact completeness disagrees with its typed outcome.",
        ));
    }
    let declared_base = value.get("base").and_then(Value::as_str);
    if declared_base.is_some_and(|base| base.starts_with('-')) {
        return Err(invalid(
            "Analysis outcome artifact base must not begin with '-'.",
        ));
    }
    if outcome.identity.base_revision.as_deref() != declared_base {
        return Err(invalid(
            "Analysis outcome artifact base does not match its typed identity.",
        ));
    }
    if outcome.kind.is_complete()
        && !matches!(
            outcome.kind,
            crate::analysis_outcome::AnalysisOutcomeKind::NoScope
        )
    {
        let diff =
            crate::analysis::load_diff(root, declared_base, None, None).map_err(|error| {
                invalid(format!(
                    "Current analysis input could not be established: {error}."
                ))
            })?;
        let digest = Sha256::digest(diff.as_bytes());
        let expected_input_identity = format!("sha256:{}", digest_hex(digest.as_ref()));
        if outcome.identity.input_identity.as_deref() != Some(expected_input_identity.as_str()) {
            return Err(invalid(
                "Analysis outcome artifact input identity does not match the current diff.",
            ));
        }
    }
    Ok(outcome)
}

fn invalid(reason: impl Into<String>) -> AnalysisOutcomeArtifactError {
    AnalysisOutcomeArtifactError::Invalid(reason.into())
}

fn digest_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::loop_commands::WORKFLOW_ANALYSIS_OUTCOME_ARTIFACT;
    use crate::analysis_outcome::{
        AnalysisIdentity, AnalysisLimitation, AnalysisLimitationKind, AnalysisOutcomeCounts,
        AnalysisOutcomeKind, AnalysisRecovery, AnalysisRecoveryKind, AnalysisStage,
    };

    fn workspace_root() -> Result<std::path::PathBuf, String> {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| "expected workspace root above crate manifest".to_string())
    }

    fn complete_outcome(kind: AnalysisOutcomeKind) -> Result<AnalysisOutcome, String> {
        let mut outcome = AnalysisOutcome::new(
            kind,
            AnalysisIdentity::default(),
            AnalysisOutcomeCounts {
                changed_file_count: 1,
                changed_line_count: 1,
                candidate_line_count: 1,
                probe_count: 1,
                finding_count: if kind == AnalysisOutcomeKind::CompleteWithFindings {
                    1
                } else {
                    0
                },
            },
            Vec::new(),
        )?;
        let diff = crate::analysis::load_diff(&workspace_root()?, Some("HEAD"), None, None)?;
        let digest = Sha256::digest(diff.as_bytes());
        outcome.identity.base_revision = Some("HEAD".to_string());
        outcome.identity.input_identity = Some(format!("sha256:{}", digest_hex(digest.as_ref())));
        Ok(outcome)
    }

    fn incomplete_outcome(kind: AnalysisOutcomeKind) -> Result<AnalysisOutcome, String> {
        AnalysisOutcome::new(
            kind,
            AnalysisIdentity::default(),
            AnalysisOutcomeCounts {
                changed_file_count: 1,
                changed_line_count: 1,
                candidate_line_count: 1,
                probe_count: 0,
                finding_count: 0,
            },
            vec![AnalysisLimitation::new(
                AnalysisLimitationKind::ProducerFailure,
                AnalysisStage::AnalysisPipeline,
                AnalysisRecovery::new(AnalysisRecoveryKind::InspectFailure, "inspect the failure")?,
            )],
        )
    }

    fn artifact(outcome: &AnalysisOutcome, declared_complete: bool) -> Result<Value, String> {
        let root = workspace_root()?;
        Ok(serde_json::json!({
            "schema_version": "0.2",
            "tool": "ripr",
            "root": root.display().to_string(),
            "base": outcome.identity.base_revision.as_deref(),
            "analysis_outcome": {
                "analysis_complete": declared_complete,
                "outcome": outcome
            }
        }))
    }

    #[test]
    fn validates_complete_findings_and_zero_outcomes_without_count_inference() -> Result<(), String>
    {
        for outcome in [
            complete_outcome(AnalysisOutcomeKind::CompleteWithFindings)?,
            complete_outcome(AnalysisOutcomeKind::CompleteNoFindings)?,
        ] {
            let root = workspace_root()?;
            let declared_complete = outcome.kind.is_complete();
            assert_eq!(
                validate_analysis_outcome_artifact(
                    &root,
                    &root.display().to_string(),
                    &artifact(&outcome, declared_complete)?
                ),
                Ok(outcome)
            );
        }
        Ok(())
    }

    #[test]
    fn preserves_incomplete_and_unsupported_kinds() -> Result<(), String> {
        for kind in [
            AnalysisOutcomeKind::PartialWithLimitations,
            AnalysisOutcomeKind::UnsupportedInput,
        ] {
            let outcome = incomplete_outcome(kind)?;
            let root = workspace_root()?;
            assert_eq!(
                validate_analysis_outcome_artifact(
                    &root,
                    &root.display().to_string(),
                    &artifact(&outcome, false)?
                ),
                Ok(outcome)
            );
        }
        Ok(())
    }

    #[test]
    fn rejects_missing_envelope_and_declared_completeness_disagreement() -> Result<(), String> {
        let outcome = complete_outcome(AnalysisOutcomeKind::CompleteNoFindings)?;
        let root = workspace_root()?;
        let root_display = root.display().to_string();
        let mut missing = artifact(&outcome, true)?;
        missing
            .as_object_mut()
            .ok_or_else(|| "fixture should be an object".to_string())?
            .remove("analysis_outcome");
        assert_eq!(
            validate_analysis_outcome_artifact(&root, &root_display, &missing),
            Err(AnalysisOutcomeArtifactError::Invalid(
                "Analysis outcome artifact is missing the analysis_outcome envelope.".to_string()
            ))
        );
        assert_eq!(
            validate_analysis_outcome_artifact(&root, &root_display, &artifact(&outcome, false)?,),
            Err(AnalysisOutcomeArtifactError::Invalid(
                "Analysis outcome artifact completeness disagrees with its typed outcome."
                    .to_string()
            ))
        );
        // Use an absolute path that cannot accidentally resolve back into the
        // checkout. Hosted CI places `CARGO_TARGET_DIR` outside the checkout,
        // while local Cargo commonly creates an in-tree `target`; using the
        // relative `target` path would therefore exercise different Git
        // discovery behavior in the two environments.
        let invalid_root = std::env::temp_dir().join(format!(
            "ripr-analysis-outcome-invalid-root-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&invalid_root);
        assert!(matches!(
            validate_analysis_outcome_artifact(
                &invalid_root,
                &root_display,
                &artifact(&outcome, true)?,
            ),
            Err(AnalysisOutcomeArtifactError::Invalid(reason))
                if reason.contains("Current analysis input could not be established")
        ));
        Ok(())
    }

    #[test]
    fn rejects_invalid_producer_metadata_and_identity() -> Result<(), String> {
        let outcome = complete_outcome(AnalysisOutcomeKind::CompleteNoFindings)?;
        let root = workspace_root()?;
        let root_display = root.display().to_string();

        let mut unknown_tool = artifact(&outcome, true)?;
        unknown_tool["tool"] = Value::String("other".to_string());
        assert_eq!(
            validate_analysis_outcome_artifact(&root, &root_display, &unknown_tool),
            Err(AnalysisOutcomeArtifactError::Invalid(
                "Analysis outcome artifact has an unknown producer tool.".to_string()
            ))
        );

        let mut wrong_schema = artifact(&outcome, true)?;
        wrong_schema["schema_version"] = Value::String("0.1".to_string());
        assert!(matches!(
            validate_analysis_outcome_artifact(&root, &root_display, &wrong_schema),
            Err(AnalysisOutcomeArtifactError::Invalid(reason))
                if reason.contains("unsupported schema_version")
        ));

        let mut wrong_root = artifact(&outcome, true)?;
        wrong_root["root"] = Value::String("other-root".to_string());
        assert_eq!(
            validate_analysis_outcome_artifact(&root, &root_display, &wrong_root),
            Err(AnalysisOutcomeArtifactError::Invalid(
                "Analysis outcome artifact root does not match the review root.".to_string()
            ))
        );

        let mut missing_complete = artifact(&outcome, true)?;
        missing_complete["analysis_outcome"]
            .as_object_mut()
            .ok_or_else(|| "expected outcome envelope".to_string())?
            .remove("analysis_complete");
        assert!(matches!(
            validate_analysis_outcome_artifact(&root, &root_display, &missing_complete),
            Err(AnalysisOutcomeArtifactError::Invalid(reason))
                if reason.contains("missing boolean analysis_complete")
        ));

        let mut missing_outcome = artifact(&outcome, true)?;
        missing_outcome["analysis_outcome"]
            .as_object_mut()
            .ok_or_else(|| "expected outcome envelope".to_string())?
            .remove("outcome");
        assert!(matches!(
            validate_analysis_outcome_artifact(&root, &root_display, &missing_outcome),
            Err(AnalysisOutcomeArtifactError::Invalid(reason))
                if reason.contains("missing the typed outcome")
        ));

        let mut invalid_typed = artifact(&outcome, true)?;
        invalid_typed["analysis_outcome"]["outcome"]["kind"] =
            Value::String("not-a-kind".to_string());
        assert!(matches!(
            validate_analysis_outcome_artifact(&root, &root_display, &invalid_typed),
            Err(AnalysisOutcomeArtifactError::Invalid(reason))
                if reason.contains("invalid typed outcome")
        ));

        let mut wrong_base = artifact(&outcome, true)?;
        wrong_base["base"] = Value::String("origin/main".to_string());
        assert_eq!(
            validate_analysis_outcome_artifact(&root, &root_display, &wrong_base),
            Err(AnalysisOutcomeArtifactError::Invalid(
                "Analysis outcome artifact base does not match its typed identity.".to_string()
            ))
        );

        let mut option_like_base = artifact(&outcome, true)?;
        option_like_base["base"] = Value::String("--output".to_string());
        option_like_base["analysis_outcome"]["outcome"]["identity"]["base_revision"] =
            Value::String("--output".to_string());
        assert_eq!(
            validate_analysis_outcome_artifact(&root, &root_display, &option_like_base),
            Err(AnalysisOutcomeArtifactError::Invalid(
                "Analysis outcome artifact base must not begin with '-'.".to_string()
            ))
        );

        let mut wrong_identity = artifact(&outcome, true)?;
        wrong_identity["analysis_outcome"]["outcome"]["identity"]["input_identity"] =
            Value::String("sha256:wrong".to_string());
        assert_eq!(
            validate_analysis_outcome_artifact(&root, &root_display, &wrong_identity),
            Err(AnalysisOutcomeArtifactError::Invalid(
                "Analysis outcome artifact input identity does not match the current diff."
                    .to_string()
            ))
        );
        Ok(())
    }

    #[test]
    fn classifies_non_not_found_artifact_io_as_invalid() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-analysis-outcome-io-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| format!("clock before epoch: {error}"))?
                .as_nanos()
        ));
        let artifact_path = root.join(WORKFLOW_ANALYSIS_OUTCOME_ARTIFACT);
        std::fs::create_dir_all(&artifact_path)
            .map_err(|error| format!("create directory artifact: {error}"))?;
        let result =
            read_analysis_outcome_artifact_at(&root, &root.display().to_string(), &artifact_path);
        std::fs::remove_dir_all(&root).map_err(|error| format!("remove test root: {error}"))?;
        assert!(matches!(
            result,
            Err(AnalysisOutcomeArtifactError::Invalid(reason))
                if reason.contains("is unavailable")
        ));
        Ok(())
    }

    #[test]
    fn derives_analysis_outcome_path_from_custom_verify_directory() -> Result<(), String> {
        assert_eq!(
            analysis_outcome_artifact_path_for_verify(Path::new(
                "target/custom/agent-verify.json"
            ))?,
            Path::new("target/custom/analysis-outcome.json")
        );
        Ok(())
    }
}
