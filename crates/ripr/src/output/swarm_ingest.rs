use std::collections::BTreeSet;

use serde_json::Value;

pub(crate) const SWARM_INGEST_SCHEMA_VERSION: &str = "0.1";

#[derive(Clone, Debug, Eq, PartialEq)]
struct SwarmIngestFacts {
    gap_id: Option<String>,
    canonical_gap_id: Option<String>,
    agent_status: Option<String>,
    stop_reason: Option<String>,
    staleness_status: Option<String>,
    edited_files: Vec<String>,
    allowed_files: Vec<String>,
    forbidden_files: Vec<String>,
    edited_forbidden_files: Vec<String>,
    verify_present: bool,
    verify_status: Option<String>,
    verify_exit_code: Option<i64>,
    verify_passed: bool,
    verify_failed: bool,
    receipt_movement: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SwarmIngestClassification {
    state: &'static str,
    reason: &'static str,
    next_action: &'static str,
}

pub(crate) fn render_swarm_ingest_json(
    result_json: &str,
    result_path: &str,
) -> Result<String, String> {
    let value: Value = serde_json::from_str(result_json)
        .map_err(|err| format!("failed to parse swarm ingest result JSON: {err}"))?;
    let facts = swarm_ingest_facts(&value);
    let classification = classify_swarm_result(&facts);
    let rendered = serde_json::json!({
        "schema_version": SWARM_INGEST_SCHEMA_VERSION,
        "tool": "ripr",
        "report": "swarm-ingest",
        "scope": "agent_result",
        "source": "external_agent_result",
        "status": "advisory",
        "inputs": {
            "result": result_path,
        },
        "classification": {
            "state": classification.state,
            "reason": classification.reason,
            "gap_id": facts.gap_id.as_ref(),
            "canonical_gap_id": facts.canonical_gap_id.as_ref(),
        },
        "evidence": {
            "agent_status": facts.agent_status.as_ref(),
            "stop_reason": facts.stop_reason.as_ref(),
            "staleness_status": facts.staleness_status.as_ref(),
            "edited_files": &facts.edited_files,
            "allowed_files": &facts.allowed_files,
            "forbidden_files": &facts.forbidden_files,
            "edited_forbidden_files": &facts.edited_forbidden_files,
            "verify": {
                "present": facts.verify_present,
                "status": facts.verify_status,
                "exit_code": facts.verify_exit_code,
                "passed": facts.verify_passed,
                "failed": facts.verify_failed,
            },
            "receipt": {
                "movement": facts.receipt_movement.as_ref(),
            },
        },
        "safety": {
            "forbidden_edit_flagged": !facts.edited_forbidden_files.is_empty(),
            "requires_human_review": true,
            "trusted_success": false,
        },
        "next_action": {
            "kind": classification.state,
            "summary": classification.next_action,
        },
        "must_not_infer": [
            "do not trust agent-reported success without verify evidence",
            "do not treat missing verify output as closed",
            "do not ignore forbidden production-code edits",
            "do not run providers, generate tests, run mutation testing, or claim runtime proof from ingest",
        ],
    });
    super::json::render_pretty_with_newline(&rendered, "swarm ingest")
}

fn swarm_ingest_facts(value: &Value) -> SwarmIngestFacts {
    let forbidden_files = first_string_array(
        value,
        &[
            &["forbidden_files"],
            &["packet", "forbidden_files"],
            &["queue_packet", "forbidden_files"],
            &["agent_packet", "forbidden_files"],
            &["task", "forbidden_files"],
        ],
    );
    let edited_files = first_string_array(
        value,
        &[
            &["edited_files"],
            &["changed_files"],
            &["attempt", "edited_files"],
            &["attempt", "changed_files"],
            &["result", "edited_files"],
            &["result", "changed_files"],
            &["changes", "edited_files"],
        ],
    );
    let edited_forbidden_files = edited_forbidden_files(&edited_files, &forbidden_files);
    let verify_status = first_string(
        value,
        &[
            &["verify_status"],
            &["verify", "status"],
            &["verification", "status"],
            &["attempt", "verify", "status"],
            &["attempt", "verification", "status"],
            &["result", "verify", "status"],
            &["result", "verification", "status"],
        ],
    );
    let verify_exit_code = first_i64(
        value,
        &[
            &["verify_exit_code"],
            &["verify", "exit_code"],
            &["verification", "exit_code"],
            &["attempt", "verify", "exit_code"],
            &["attempt", "verification", "exit_code"],
            &["result", "verify", "exit_code"],
            &["result", "verification", "exit_code"],
        ],
    );
    let verify_present = verify_status.is_some()
        || verify_exit_code.is_some()
        || first_string(
            value,
            &[
                &["verify", "output_path"],
                &["verify", "stdout"],
                &["verification", "output_path"],
                &["attempt", "verify", "output_path"],
                &["attempt", "verification", "output_path"],
            ],
        )
        .is_some();
    let verify_passed =
        verify_status.as_deref().is_some_and(is_success_status) || verify_exit_code == Some(0);
    let verify_failed = verify_status.as_deref().is_some_and(is_failure_status)
        || verify_exit_code.is_some_and(|code| code != 0);
    SwarmIngestFacts {
        gap_id: first_string(
            value,
            &[
                &["gap_id"],
                &["packet", "gap_id"],
                &["queue_packet", "gap_id"],
                &["agent_packet", "gap_id"],
                &["task", "gap_id"],
                &["result", "gap_id"],
            ],
        ),
        canonical_gap_id: first_string(
            value,
            &[
                &["canonical_gap_id"],
                &["packet", "canonical_gap_id"],
                &["queue_packet", "canonical_gap_id"],
                &["agent_packet", "canonical_gap_id"],
                &["task", "canonical_gap_id"],
                &["result", "canonical_gap_id"],
            ],
        ),
        agent_status: first_string(
            value,
            &[
                &["agent_status"],
                &["attempt", "status"],
                &["result", "status"],
                &["status"],
            ],
        ),
        stop_reason: first_string(
            value,
            &[
                &["stop_reason"],
                &["attempt", "stop_reason"],
                &["result", "stop_reason"],
            ],
        ),
        staleness_status: first_string(
            value,
            &[
                &["staleness_status"],
                &["packet", "staleness_status"],
                &["queue_packet", "staleness_status"],
                &["agent_packet", "staleness_status"],
            ],
        ),
        edited_files,
        allowed_files: first_string_array(
            value,
            &[
                &["allowed_files"],
                &["allowed_edit_surface"],
                &["packet", "allowed_files"],
                &["packet", "allowed_edit_surface"],
                &["queue_packet", "allowed_files"],
                &["queue_packet", "allowed_edit_surface"],
                &["agent_packet", "allowed_files"],
                &["task", "allowed_files"],
            ],
        ),
        forbidden_files,
        edited_forbidden_files,
        verify_present,
        verify_status,
        verify_exit_code,
        verify_passed,
        verify_failed,
        receipt_movement: first_string(
            value,
            &[
                &["receipt_movement"],
                &["receipt", "movement"],
                &["receipt", "provenance", "movement"],
                &["receipt", "static_movement", "state"],
                &["receipt", "summary", "receipt_state"],
                &["agent_receipt", "provenance", "movement"],
                &["agent_receipt", "seam", "change"],
                &["provenance", "movement"],
                &["seam", "change"],
            ],
        ),
    }
}

fn classify_swarm_result(facts: &SwarmIngestFacts) -> SwarmIngestClassification {
    if !facts.edited_forbidden_files.is_empty() {
        return SwarmIngestClassification {
            state: "edited_forbidden_file",
            reason: "Agent result reports edits to files forbidden by the packet.",
            next_action: "Reject or manually review the attempt before using any test repair.",
        };
    }
    if facts
        .staleness_status
        .as_deref()
        .is_some_and(is_stale_status)
    {
        return SwarmIngestClassification {
            state: "stale_packet",
            reason: "Agent result refers to a packet marked stale.",
            next_action: "Refresh the queue and reroute the gap before trusting the attempt.",
        };
    }
    if facts.agent_status.as_deref().is_some_and(is_stopped_status) || facts.stop_reason.is_some() {
        return SwarmIngestClassification {
            state: "stopped_by_agent",
            reason: "Agent stopped before claiming a completed repair.",
            next_action: "Record the stop reason and reroute only if the packet remains actionable.",
        };
    }
    if facts.verify_failed {
        return SwarmIngestClassification {
            state: "verify_failed",
            reason: "Verify evidence reports a failing command or non-zero exit code.",
            next_action: "Inspect verify output before retrying or accepting the repair.",
        };
    }
    if !facts.verify_present {
        return SwarmIngestClassification {
            state: "uncertain",
            reason: "Agent result did not include verify evidence.",
            next_action: "Run the packet verify command and attach the result before judging closure.",
        };
    }
    if !facts.verify_passed {
        return SwarmIngestClassification {
            state: "uncertain",
            reason: "Verify evidence is present but does not prove a passing command.",
            next_action: "Normalize the verify result or rerun the packet verify command.",
        };
    }
    match facts
        .receipt_movement
        .as_deref()
        .map(normalize_state)
        .as_deref()
    {
        Some("closed" | "resolved" | "receipt_movement_resolved") => SwarmIngestClassification {
            state: "closed",
            reason: "Verify passed and receipt movement indicates the gap closed.",
            next_action: "Attach the receipt and keep the focused test repair.",
        },
        Some("improved" | "receipt_movement_improved") => SwarmIngestClassification {
            state: "partially_improved",
            reason: "Verify passed and receipt movement improved, but did not report closure.",
            next_action: "Keep the evidence and decide whether another focused repair is needed.",
        },
        Some("unchanged" | "receipt_movement_unchanged" | "unchanged_after_attempt") => {
            SwarmIngestClassification {
                state: "uncertain",
                reason: "Verify passed but receipt movement stayed unchanged.",
                next_action: "Strengthen the discriminator or reroute the remaining gap.",
            }
        }
        Some("regressed" | "receipt_movement_regressed") => SwarmIngestClassification {
            state: "uncertain",
            reason: "Verify passed but receipt movement regressed.",
            next_action: "Reject or manually inspect the attempt before retrying.",
        },
        _ => SwarmIngestClassification {
            state: "uncertain",
            reason: "Verify passed but no recognized receipt movement was supplied.",
            next_action: "Produce a before/after receipt before judging closure.",
        },
    }
}

fn edited_forbidden_files(edited_files: &[String], forbidden_files: &[String]) -> Vec<String> {
    let forbidden: BTreeSet<_> = forbidden_files
        .iter()
        .map(|file| normalize_path(file))
        .collect();
    dedup(
        edited_files
            .iter()
            .filter(|file| forbidden.contains(&normalize_path(file)))
            .cloned()
            .collect(),
    )
}

fn first_string(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths
        .iter()
        .find_map(|path| path_value(value, path).and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn first_i64(value: &Value, paths: &[&[&str]]) -> Option<i64> {
    paths
        .iter()
        .find_map(|path| path_value(value, path).and_then(Value::as_i64))
}

fn first_string_array(value: &Value, paths: &[&[&str]]) -> Vec<String> {
    paths
        .iter()
        .find_map(|path| {
            let values = string_array_at(value, path);
            (!values.is_empty()).then_some(values)
        })
        .unwrap_or_default()
}

fn string_array_at(value: &Value, path: &[&str]) -> Vec<String> {
    let Some(array) = path_value(value, path).and_then(Value::as_array) else {
        return Vec::new();
    };
    dedup(
        array
            .iter()
            .filter_map(|item| {
                item.as_str()
                    .map(ToOwned::to_owned)
                    .or_else(|| {
                        item.get("path")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                    })
                    .or_else(|| {
                        item.get("file")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                    })
            })
            .collect(),
    )
}

fn path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn dedup(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

fn normalize_state(state: &str) -> String {
    state.trim().to_ascii_lowercase()
}

fn is_success_status(status: &str) -> bool {
    matches!(
        normalize_state(status).as_str(),
        "pass" | "passed" | "success" | "succeeded" | "ok"
    )
}

fn is_failure_status(status: &str) -> bool {
    matches!(
        normalize_state(status).as_str(),
        "fail" | "failed" | "failure" | "error" | "errored"
    )
}

fn is_stale_status(status: &str) -> bool {
    matches!(normalize_state(status).as_str(), "stale" | "stale_packet")
}

fn is_stopped_status(status: &str) -> bool {
    matches!(
        normalize_state(status).as_str(),
        "stopped" | "stopped_by_agent" | "blocked" | "aborted"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_value(json: &str) -> Result<Value, String> {
        let rendered = render_swarm_ingest_json(json, "agent-result.json")?;
        serde_json::from_str(&rendered)
            .map_err(|err| format!("rendered ingest JSON should parse: {err}"))
    }

    #[test]
    fn ingest_flags_forbidden_edits_before_success_claims() -> Result<(), String> {
        let value = render_value(
            r#"{
              "packet": {
                "gap_id": "gap:python:pricing",
                "canonical_gap_id": "python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold",
                "allowed_files": ["tests/test_pricing.py"],
                "forbidden_files": ["app/pricing.py"]
              },
              "attempt": {
                "status": "completed",
                "edited_files": ["tests/test_pricing.py", "app/pricing.py"],
                "verify": {"status": "passed", "exit_code": 0}
              },
              "receipt": {"provenance": {"movement": "resolved"}}
            }"#,
        )?;

        assert_eq!(value["classification"]["state"], "edited_forbidden_file");
        assert_eq!(value["safety"]["forbidden_edit_flagged"], true);
        assert_eq!(
            value["evidence"]["edited_forbidden_files"],
            serde_json::json!(["app/pricing.py"])
        );
        assert_eq!(value["safety"]["trusted_success"], false);
        Ok(())
    }

    #[test]
    fn ingest_requires_verify_before_closure() -> Result<(), String> {
        let value = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:pricing"},
              "attempt": {"status": "completed", "edited_files": ["tests/test_pricing.py"]},
              "receipt": {"provenance": {"movement": "resolved"}}
            }"#,
        )?;

        assert_eq!(value["classification"]["state"], "uncertain");
        assert_eq!(value["evidence"]["verify"]["present"], false);
        assert!(
            value["classification"]["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("verify evidence"))
        );
        Ok(())
    }

    #[test]
    fn ingest_classifies_stopped_failed_improved_and_closed_attempts() -> Result<(), String> {
        let stopped = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:stopped"},
              "attempt": {"status": "stopped", "stop_reason": "expected value is ambiguous"}
            }"#,
        )?;
        assert_eq!(stopped["classification"]["state"], "stopped_by_agent");

        let failed = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:failed"},
              "attempt": {"verify": {"status": "failed", "exit_code": 1}}
            }"#,
        )?;
        assert_eq!(failed["classification"]["state"], "verify_failed");

        let improved = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:improved"},
              "attempt": {"verify": {"status": "passed", "exit_code": 0}},
              "receipt": {"provenance": {"movement": "improved"}}
            }"#,
        )?;
        assert_eq!(improved["classification"]["state"], "partially_improved");

        let closed = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:closed", "staleness_status": "not_evaluated"},
              "attempt": {"verify": {"status": "passed", "exit_code": 0}},
              "receipt": {"provenance": {"movement": "resolved"}}
            }"#,
        )?;
        assert_eq!(closed["classification"]["state"], "closed");

        let stale = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:stale", "staleness_status": "stale"},
              "attempt": {"verify": {"status": "passed", "exit_code": 0}},
              "receipt": {"provenance": {"movement": "resolved"}}
            }"#,
        )?;
        assert_eq!(stale["classification"]["state"], "stale_packet");
        Ok(())
    }
}
