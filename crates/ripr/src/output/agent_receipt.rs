//! Render a compact agent verification receipt.
//!
//! `ripr agent receipt` consumes the JSON emitted by `ripr agent verify` and
//! narrows it to one seam so an agent can attach a small handoff artifact to a
//! focused test change. It does not run analysis, generate tests, or interpret
//! runtime mutation output.

use serde_json::Value;

pub(crate) const AGENT_RECEIPT_SCHEMA_VERSION: &str = "0.2";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReceiptInputPaths {
    pub(crate) before: String,
    pub(crate) after: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReceiptArtifactProvenance {
    pub(crate) path: String,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReceiptProvenance {
    pub(crate) ripr_version: String,
    pub(crate) repo_root: String,
    pub(crate) config_fingerprint: Option<String>,
    pub(crate) command_template_version: String,
    pub(crate) generated_at: String,
    pub(crate) workflow_artifact: Option<AgentReceiptArtifactProvenance>,
    pub(crate) before_artifact: AgentReceiptArtifactProvenance,
    pub(crate) after_artifact: AgentReceiptArtifactProvenance,
    pub(crate) verify_artifact: AgentReceiptArtifactProvenance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AgentReceiptSeam {
    seam_id: String,
    seam_kind: String,
    file: String,
    line: usize,
    before: Option<String>,
    after: Option<String>,
    grip_class: Option<String>,
    change: String,
    evidence_delta: Vec<String>,
}

pub(crate) fn render_agent_receipt_json(
    agent_verify_json: &str,
    agent_verify_path: String,
    seam_id: &str,
    test_changed: Option<&str>,
    commands_run: &[String],
    provenance: AgentReceiptProvenance,
) -> Result<String, String> {
    let verify: Value = serde_json::from_str(agent_verify_json)
        .map_err(|err| format!("failed to parse agent verify JSON: {err}"))?;
    let input_paths = agent_receipt_input_paths_from_value(&verify)?;
    let seam = find_receipt_seam(&verify, seam_id)?;
    let (remaining_gap, next_recommendation) = receipt_guidance(&seam.change);
    let provenance = provenance_json(&provenance, &seam);

    let value = serde_json::json!({
        "schema_version": AGENT_RECEIPT_SCHEMA_VERSION,
        "tool": "ripr",
        "status": "advisory",
        "inputs": {
            "agent_verify_json": agent_verify_path,
            "before": input_paths.before,
            "after": input_paths.after
        },
        "provenance": provenance,
        "seam": {
            "seam_id": seam.seam_id,
            "seam_kind": seam.seam_kind,
            "file": seam.file,
            "line": seam.line,
            "before": seam.before,
            "after": seam.after,
            "grip_class": seam.grip_class,
            "change": seam.change,
            "evidence_delta": seam.evidence_delta
        },
        "test_changed": test_changed,
        "verification": {
            "commands_run": commands_run
        },
        "summary": {
            "remaining_gap": remaining_gap,
            "next_recommendation": next_recommendation
        }
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render agent receipt JSON: {err}"))
}

pub(crate) fn agent_receipt_input_paths(
    agent_verify_json: &str,
) -> Result<AgentReceiptInputPaths, String> {
    let verify: Value = serde_json::from_str(agent_verify_json)
        .map_err(|err| format!("failed to parse agent verify JSON: {err}"))?;
    agent_receipt_input_paths_from_value(&verify)
}

fn agent_receipt_input_paths_from_value(verify: &Value) -> Result<AgentReceiptInputPaths, String> {
    let inputs = verify
        .get("inputs")
        .ok_or_else(|| "agent verify JSON is missing `inputs`".to_string())?;
    Ok(AgentReceiptInputPaths {
        before: required_string(inputs, "before", "agent verify inputs")?,
        after: required_string(inputs, "after", "agent verify inputs")?,
    })
}

fn provenance_json(
    provenance: &AgentReceiptProvenance,
    seam: &AgentReceiptSeam,
) -> serde_json::Value {
    let (before_class, after_class) = receipt_class_pair(seam);
    serde_json::json!({
        "ripr_version": provenance.ripr_version.as_str(),
        "repo_root": provenance.repo_root.as_str(),
        "config_fingerprint": provenance.config_fingerprint.as_deref(),
        "command_template_version": provenance.command_template_version.as_str(),
        "generated_at": provenance.generated_at.as_str(),
        "workflow_artifact": provenance.workflow_artifact.as_ref().map(artifact_provenance_json),
        "before_artifact": artifact_provenance_json(&provenance.before_artifact),
        "after_artifact": artifact_provenance_json(&provenance.after_artifact),
        "verify_artifact": artifact_provenance_json(&provenance.verify_artifact),
        "seam_id": seam.seam_id.as_str(),
        "before_class": before_class,
        "after_class": after_class,
        "movement": seam.change.as_str(),
        "limits": {
            "static_artifact_relationship": true,
            "runtime_mutation_execution": false,
            "runtime_adequacy_claim": false
        }
    })
}

fn artifact_provenance_json(artifact: &AgentReceiptArtifactProvenance) -> serde_json::Value {
    serde_json::json!({
        "path": artifact.path.as_str(),
        "sha256": artifact.sha256.as_str()
    })
}

fn receipt_class_pair(seam: &AgentReceiptSeam) -> (Option<String>, Option<String>) {
    match seam.change.as_str() {
        "new" => (None, seam.grip_class.clone()),
        "resolved" => (seam.grip_class.clone(), None),
        _ => (seam.before.clone(), seam.after.clone()),
    }
}

fn find_receipt_seam(verify: &Value, seam_id: &str) -> Result<AgentReceiptSeam, String> {
    for bucket in ["changed_seams", "unchanged_seams"] {
        for seam in array_field(verify, bucket)? {
            if required_string(seam, "seam_id", bucket)? == seam_id {
                return matched_receipt_seam(seam, seam_id, bucket);
            }
        }
    }

    for bucket in ["new_gaps", "resolved_gaps"] {
        for seam in array_field(verify, bucket)? {
            if required_string(seam, "seam_id", bucket)? == seam_id {
                return one_sided_receipt_seam(seam, seam_id, bucket);
            }
        }
    }

    Err(format!(
        "agent receipt seam_id {seam_id} was not found in agent verify JSON"
    ))
}

fn matched_receipt_seam(
    seam: &Value,
    seam_id: &str,
    bucket: &str,
) -> Result<AgentReceiptSeam, String> {
    Ok(AgentReceiptSeam {
        seam_id: seam_id.to_string(),
        seam_kind: required_string(seam, "seam_kind", bucket)?,
        file: required_string(seam, "file", bucket)?,
        line: required_usize(seam, "line", bucket)?,
        before: Some(required_string(seam, "before", bucket)?),
        after: Some(required_string(seam, "after", bucket)?),
        grip_class: None,
        change: required_string(seam, "change", bucket)?,
        evidence_delta: string_array_field(seam, "evidence_delta"),
    })
}

fn one_sided_receipt_seam(
    seam: &Value,
    seam_id: &str,
    bucket: &str,
) -> Result<AgentReceiptSeam, String> {
    Ok(AgentReceiptSeam {
        seam_id: seam_id.to_string(),
        seam_kind: required_string(seam, "seam_kind", bucket)?,
        file: required_string(seam, "file", bucket)?,
        line: required_usize(seam, "line", bucket)?,
        before: None,
        after: None,
        grip_class: Some(required_string(seam, "grip_class", bucket)?),
        change: required_string(seam, "change", bucket)?,
        evidence_delta: Vec::new(),
    })
}

fn receipt_guidance(change: &str) -> (&'static str, &'static str) {
    match change {
        "improved" => (
            "No remaining static gap is named by this receipt; inspect the current seam packet if review needs final assertion detail.",
            "Keep the focused test and attach this receipt with the agent verify JSON.",
        ),
        "changed" => (
            "Static evidence changed without a higher grip class; inspect the evidence delta and current seam packet.",
            "Strengthen the discriminator named by the seam packet, then rerun agent verify.",
        ),
        "regressed" => (
            "The after snapshot ranks this seam lower than before.",
            "Revisit the targeted test or changed behavior before relying on this patch.",
        ),
        "unchanged" => (
            "Static grip class did not move.",
            "Add or strengthen the missing discriminator named by the seam packet, then rerun agent verify.",
        ),
        "new" => (
            "A new static seam gap is present in the after snapshot.",
            "Run agent brief or agent packet for this seam before merging the change.",
        ),
        "resolved" => (
            "The seam is absent from the after snapshot; this may mean the behavior changed or the gap was resolved.",
            "Confirm the seam disappeared for the intended reason, then keep the before/after artifacts with review evidence.",
        ),
        _ => (
            "Static receipt guidance is unknown for this change bucket.",
            "Inspect the agent verify JSON and current seam packet before relying on this patch.",
        ),
    }
}

fn array_field<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("agent verify JSON is missing `{key}` array"))
}

fn required_string(value: &Value, key: &str, context: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("{context} is missing string field `{key}`"))
}

fn required_usize(value: &Value, key: &str, context: &str) -> Result<usize, String> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|line| usize::try_from(line).ok())
        .ok_or_else(|| format!("{context} is missing numeric field `{key}`"))
}

fn string_array_field(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_verify_json() -> &'static str {
        r#"{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "advisory",
  "inputs": {
    "before": "target/ripr/workflow/before.repo-exposure.json",
    "after": "target/ripr/workflow/after.repo-exposure.json"
  },
  "summary": {
    "improved": 1,
    "changed": 0,
    "regressed": 0,
    "unchanged": 1,
    "new": 1,
    "resolved": 0
  },
  "changed_seams": [
    {
      "seam_id": "seam-a",
      "seam_kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "before": "weakly_gripped",
      "after": "strongly_gripped",
      "change": "improved",
      "evidence_delta": ["missing discriminator no longer reported: threshold equality"]
    }
  ],
  "unchanged_seams": [
    {
      "seam_id": "seam-b",
      "seam_kind": "error_variant",
      "file": "src/auth.rs",
      "line": 9,
      "before": "weakly_gripped",
      "after": "weakly_gripped",
      "change": "unchanged",
      "evidence_delta": []
    }
  ],
  "new_gaps": [
    {
      "seam_id": "seam-c",
      "seam_kind": "call_presence",
      "file": "src/events.rs",
      "line": 12,
      "grip_class": "ungripped",
      "change": "new"
    }
  ],
  "resolved_gaps": []
}"#
    }

    fn fixed_provenance() -> AgentReceiptProvenance {
        AgentReceiptProvenance {
            ripr_version: "0.4.0".to_string(),
            repo_root: ".".to_string(),
            config_fingerprint: Some("fnv1a64:4c94a2f6cfaa5c21".to_string()),
            command_template_version: "0.1".to_string(),
            generated_at: "unix_ms:1778179200000".to_string(),
            workflow_artifact: None,
            before_artifact: AgentReceiptArtifactProvenance {
                path: "target/ripr/workflow/before.repo-exposure.json".to_string(),
                sha256: "sha256:before".to_string(),
            },
            after_artifact: AgentReceiptArtifactProvenance {
                path: "target/ripr/workflow/after.repo-exposure.json".to_string(),
                sha256: "sha256:after".to_string(),
            },
            verify_artifact: AgentReceiptArtifactProvenance {
                path: "target/ripr/workflow/agent-verify.json".to_string(),
                sha256: "sha256:verify".to_string(),
            },
        }
    }

    #[test]
    fn agent_receipt_json_selects_changed_seam() -> Result<(), String> {
        let rendered = render_agent_receipt_json(
            agent_verify_json(),
            "target/ripr/workflow/agent-verify.json".to_string(),
            "seam-a",
            Some("tests::pricing_boundary"),
            &["cargo test pricing_boundary".to_string()],
            fixed_provenance(),
        )?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("receipt JSON should parse: {err}"))?;

        assert_eq!(value["schema_version"], "0.2");
        assert_eq!(value["seam"]["seam_id"], "seam-a");
        assert_eq!(value["seam"]["before"], "weakly_gripped");
        assert_eq!(value["seam"]["after"], "strongly_gripped");
        assert_eq!(value["seam"]["change"], "improved");
        assert_eq!(value["provenance"]["ripr_version"], "0.4.0");
        assert_eq!(value["provenance"]["repo_root"], ".");
        assert_eq!(
            value["provenance"]["config_fingerprint"],
            "fnv1a64:4c94a2f6cfaa5c21"
        );
        assert_eq!(value["provenance"]["command_template_version"], "0.1");
        assert_eq!(value["provenance"]["generated_at"], "unix_ms:1778179200000");
        assert_eq!(value["provenance"]["before_class"], "weakly_gripped");
        assert_eq!(value["provenance"]["after_class"], "strongly_gripped");
        assert_eq!(value["provenance"]["movement"], "improved");
        assert_eq!(
            value["provenance"]["before_artifact"]["sha256"],
            "sha256:before"
        );
        assert_eq!(
            value["provenance"]["limits"]["runtime_mutation_execution"],
            false
        );
        assert_eq!(value["test_changed"], "tests::pricing_boundary");
        assert_eq!(
            value["verification"]["commands_run"][0],
            "cargo test pricing_boundary"
        );
        assert!(
            value["summary"]["next_recommendation"]
                .as_str()
                .unwrap_or_default()
                .contains("attach this receipt")
        );
        Ok(())
    }

    #[test]
    fn agent_receipt_json_selects_new_gap() -> Result<(), String> {
        let rendered = render_agent_receipt_json(
            agent_verify_json(),
            "target/ripr/workflow/agent-verify.json".to_string(),
            "seam-c",
            None,
            &[],
            fixed_provenance(),
        )?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("receipt JSON should parse: {err}"))?;

        assert_eq!(value["seam"]["seam_id"], "seam-c");
        assert_eq!(value["seam"]["grip_class"], "ungripped");
        assert_eq!(value["seam"]["change"], "new");
        assert_eq!(value["provenance"]["before_class"], Value::Null);
        assert_eq!(value["provenance"]["after_class"], "ungripped");
        assert_eq!(value["test_changed"], Value::Null);
        Ok(())
    }

    #[test]
    fn agent_receipt_input_paths_extracts_verify_snapshot_paths() -> Result<(), String> {
        let paths = agent_receipt_input_paths(agent_verify_json())?;

        assert_eq!(
            paths.before,
            "target/ripr/workflow/before.repo-exposure.json"
        );
        assert_eq!(paths.after, "target/ripr/workflow/after.repo-exposure.json");
        Ok(())
    }

    #[test]
    fn agent_receipt_json_errors_when_seam_is_missing() {
        assert_eq!(
            render_agent_receipt_json(
                agent_verify_json(),
                "target/ripr/workflow/agent-verify.json".to_string(),
                "missing",
                None,
                &[],
                fixed_provenance(),
            ),
            Err("agent receipt seam_id missing was not found in agent verify JSON".to_string())
        );
    }
}
