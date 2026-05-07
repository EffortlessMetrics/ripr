//! Render a compact agent verification receipt.
//!
//! `ripr agent receipt` consumes the JSON emitted by `ripr agent verify` and
//! narrows it to one seam so an agent can attach a small handoff artifact to a
//! focused test change. It does not run analysis, generate tests, or interpret
//! runtime mutation output.

use serde_json::Value;

pub(crate) const AGENT_RECEIPT_SCHEMA_VERSION: &str = "0.1";

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
) -> Result<String, String> {
    let verify: Value = serde_json::from_str(agent_verify_json)
        .map_err(|err| format!("failed to parse agent verify JSON: {err}"))?;
    let inputs = verify
        .get("inputs")
        .ok_or_else(|| "agent verify JSON is missing `inputs`".to_string())?;
    let before = required_string(inputs, "before", "agent verify inputs")?;
    let after = required_string(inputs, "after", "agent verify inputs")?;
    let seam = find_receipt_seam(&verify, seam_id)?;
    let (remaining_gap, next_recommendation) = receipt_guidance(&seam.change);

    let value = serde_json::json!({
        "schema_version": AGENT_RECEIPT_SCHEMA_VERSION,
        "tool": "ripr",
        "status": "advisory",
        "inputs": {
            "agent_verify_json": agent_verify_path,
            "before": before,
            "after": after
        },
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

    #[test]
    fn agent_receipt_json_selects_changed_seam() -> Result<(), String> {
        let rendered = render_agent_receipt_json(
            agent_verify_json(),
            "target/ripr/workflow/agent-verify.json".to_string(),
            "seam-a",
            Some("tests::pricing_boundary"),
            &["cargo test pricing_boundary".to_string()],
        )?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("receipt JSON should parse: {err}"))?;

        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["seam"]["seam_id"], "seam-a");
        assert_eq!(value["seam"]["before"], "weakly_gripped");
        assert_eq!(value["seam"]["after"], "strongly_gripped");
        assert_eq!(value["seam"]["change"], "improved");
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
        )?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("receipt JSON should parse: {err}"))?;

        assert_eq!(value["seam"]["seam_id"], "seam-c");
        assert_eq!(value["seam"]["grip_class"], "ungripped");
        assert_eq!(value["seam"]["change"], "new");
        assert_eq!(value["test_changed"], Value::Null);
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
            ),
            Err("agent receipt seam_id missing was not found in agent verify JSON".to_string())
        );
    }
}
