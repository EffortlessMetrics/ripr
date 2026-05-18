mod artifacts;
mod json;
mod markdown;
mod receipt;
mod report;
mod types;
mod util;

pub(crate) use json::render_agent_review_summary_json;
pub(crate) use markdown::render_agent_review_summary_markdown;
pub(crate) use report::build_agent_review_summary_report;
#[cfg(test)]
mod tests {
    use super::artifacts::{
        LSP_COCKPIT_ARTIFACT, OPERATOR_COCKPIT_ARTIFACT, REPO_EXPOSURE_ARTIFACT,
    };
    use super::types::AGENT_REVIEW_SUMMARY_SCHEMA_VERSION;
    use super::*;
    use crate::agent::loop_commands::{
        WORKFLOW_AFTER_SNAPSHOT_ARTIFACT, WORKFLOW_AGENT_BRIEF_ARTIFACT,
        WORKFLOW_AGENT_PACKET_ARTIFACT, WORKFLOW_AGENT_RECEIPT_ARTIFACT,
        WORKFLOW_AGENT_VERIFY_ARTIFACT, WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
        WORKFLOW_MANIFEST_ARTIFACT,
    };
    use serde_json::Value;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_agent_review_summary_test_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "ripr-agent-review-summary-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn fixture_value(relative_path: &str) -> Result<Value, String> {
        let text = std::fs::read_to_string(workspace_root().join(relative_path))
            .map_err(|err| format!("read fixture {relative_path}: {err}"))?;
        serde_json::from_str(&text).map_err(|err| format!("parse fixture {relative_path}: {err}"))
    }

    fn write_file(path: &Path, text: &str) -> Result<(), String> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|err| format!("create parent: {err}"))?;
        }
        std::fs::write(path, text).map_err(|err| format!("write {}: {err}", path.display()))
    }

    fn write_complete_artifacts(root: &Path) -> Result<(), String> {
        write_file(&root.join(WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AFTER_SNAPSHOT_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AGENT_BRIEF_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AGENT_PACKET_ARTIFACT), "{}")?;
        write_file(
            &root.join(WORKFLOW_AGENT_VERIFY_ARTIFACT),
            r#"{"changed_seams":[{"seam_id":"seam-a"}],"unchanged_seams":[],"new_gaps":[],"resolved_gaps":[]}"#,
        )?;
        write_file(
            &root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
            r#"{
  "schema_version": "0.3",
  "tool": "ripr",
  "status": "advisory",
  "provenance": {
    "before_class": "weakly_gripped",
    "after_class": "strongly_gripped",
    "movement": "improved",
    "verify_artifact": {
      "path": "target/ripr/workflow/agent-verify.json",
      "sha256": "sha256:verify"
    }
  },
  "seam": {
    "seam_id": "seam-a",
    "file": "src/lib.rs",
    "line": 42,
    "seam_kind": "predicate_boundary",
    "before": "weakly_gripped",
    "after": "strongly_gripped",
    "change": "improved",
    "grip_class": "strongly_gripped"
  },
  "summary": {
    "remaining_gap": "No remaining static gap is named by this receipt.",
    "next_recommendation": "Keep the focused test and attach the receipt.",
    "next_action": {
      "kind": "improved",
      "summary": "Static grip improved.",
      "recommended_action": "Keep the focused test and include this receipt in review.",
      "safe_to_merge": false
    }
  }
}"#,
        )?;
        write_file(
            &root.join(WORKFLOW_MANIFEST_ARTIFACT),
            r#"{"status":"ready","seam":{"seam_id":"seam-a","file":"src/lib.rs","line":42,"seam_kind":"predicate_boundary"}}"#,
        )?;
        write_file(
            &root.join(REPO_EXPOSURE_ARTIFACT),
            r#"{"status":"ready","metrics":{"seams_total":2,"weakly_gripped":1}}"#,
        )?;
        write_file(
            &root.join(OPERATOR_COCKPIT_ARTIFACT),
            r#"{"status":"ready","top_weak_seams":[{"seam_id":"seam-a"}],"next_commands":[]}"#,
        )?;
        write_file(&root.join(LSP_COCKPIT_ARTIFACT), r#"{"status":"ready"}"#)?;
        Ok(())
    }

    struct ReviewFixtureCase<'a> {
        name: &'a str,
        seam_id: &'a str,
        movement: &'a str,
        before: &'a str,
        after: &'a str,
        grip_class: &'a str,
        action_kind: &'a str,
        action_summary: &'a str,
        action_recommendation: &'a str,
    }

    fn write_review_summary_case_artifacts(
        root: &Path,
        case: &ReviewFixtureCase<'_>,
    ) -> Result<(), String> {
        write_file(&root.join(WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AFTER_SNAPSHOT_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AGENT_BRIEF_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AGENT_PACKET_ARTIFACT), "{}")?;
        write_file(
            &root.join(WORKFLOW_AGENT_VERIFY_ARTIFACT),
            &serde_json::to_string_pretty(&serde_json::json!({
                "changed_seams": [{"seam_id": case.seam_id}],
                "unchanged_seams": [],
                "new_gaps": [],
                "resolved_gaps": []
            }))
            .map_err(|err| format!("render verify fixture: {err}"))?,
        )?;
        write_file(
            &root.join(WORKFLOW_MANIFEST_ARTIFACT),
            &serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": "0.1",
                "tool": "ripr",
                "status": "ready",
                "seam": {
                    "seam_id": case.seam_id,
                    "file": "src/pricing.rs",
                    "line": 42,
                    "seam_kind": "predicate_boundary"
                }
            }))
            .map_err(|err| format!("render workflow fixture: {err}"))?,
        )?;
        write_file(
            &root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
            &serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": "0.3",
                "tool": "ripr",
                "status": "advisory",
                "provenance": {
                    "before_class": case.before,
                    "after_class": case.after,
                    "movement": case.movement,
                    "verify_artifact": {
                        "path": WORKFLOW_AGENT_VERIFY_ARTIFACT,
                        "sha256": "sha256:verify"
                    }
                },
                "seam": {
                    "seam_id": case.seam_id,
                    "file": "src/pricing.rs",
                    "line": 42,
                    "seam_kind": "predicate_boundary",
                    "before": case.before,
                    "after": case.after,
                    "change": case.movement,
                    "grip_class": case.grip_class
                },
                "summary": {
                    "remaining_gap": "Fixture-controlled static review state.",
                    "next_recommendation": case.action_recommendation,
                    "next_action": {
                        "kind": case.action_kind,
                        "summary": case.action_summary,
                        "recommended_action": case.action_recommendation,
                        "safe_to_merge": false
                    }
                }
            }))
            .map_err(|err| format!("render receipt fixture: {err}"))?,
        )?;
        Ok(())
    }

    fn assert_review_summary_matches_fixture(
        root: &Path,
        root_argument: &Path,
        case_name: &str,
    ) -> Result<(), String> {
        let report = build_agent_review_summary_report(root, root_argument);
        let rendered = render_agent_review_summary_json(&report)?;
        let actual: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse rendered review summary: {err}"))?;
        let fixture_path =
            format!("fixtures/boundary_gap/expected/llm-work-loop/{case_name}/review-summary.json");
        assert_eq!(
            actual,
            fixture_value(&fixture_path)?,
            "{case_name} fixture drifted"
        );
        Ok(())
    }

    #[test]
    fn agent_review_summary_joins_status_receipt_cockpit_repo_and_lsp() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("joined");
        write_complete_artifacts(&root)?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse review summary JSON: {err}"))?;

        assert_eq!(value["schema_version"], AGENT_REVIEW_SUMMARY_SCHEMA_VERSION);
        assert_eq!(value["status"], "ready");
        assert_eq!(value["target_seam"]["seam_id"], "seam-a");
        assert_eq!(value["static_movement"]["state"], "improved");
        assert_eq!(
            value["static_movement"]["next_action"]["recommended_action"],
            "Keep the focused test and include this receipt in review."
        );
        assert!(
            value["surfaces"]
                .as_array()
                .ok_or_else(|| "expected surfaces".to_string())?
                .iter()
                .any(|surface| surface["name"] == "operator_cockpit"
                    && surface["state"] == "present")
        );
        assert!(
            value["ci_artifacts"]
                .as_array()
                .ok_or_else(|| "expected ci artifacts".to_string())?
                .iter()
                .any(|artifact| artifact["name"] == "agent_receipt"
                    && artifact["state"] == "present")
        );
        assert_eq!(value["next_command"], Value::Null);

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_reports_missing_receipt_with_next_command() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("missing-receipt");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse review summary JSON: {err}"))?;

        assert_eq!(value["status"], "incomplete");
        assert_eq!(value["static_movement"]["state"], "missing_artifact");
        assert_eq!(value["next_command"]["step"], "before_snapshot");
        assert_eq!(
            value["static_movement"]["next_action"]["recommended_action"],
            "Run the next command listed by agent status."
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_llm_work_loop_review_summary_fixtures_pin_core_states() -> Result<(), String> {
        let cases = [
            ReviewFixtureCase {
                name: "happy",
                seam_id: "seam-happy",
                movement: "improved",
                before: "weakly_gripped",
                after: "strongly_gripped",
                grip_class: "strongly_gripped",
                action_kind: "improved",
                action_summary: "Static grip improved.",
                action_recommendation: "Keep the focused test and include this receipt in review.",
            },
            ReviewFixtureCase {
                name: "unchanged",
                seam_id: "seam-unchanged",
                movement: "unchanged",
                before: "weakly_gripped",
                after: "weakly_gripped",
                grip_class: "weakly_gripped",
                action_kind: "unchanged",
                action_summary: "Static grip did not improve.",
                action_recommendation: "Add the missing discriminator or stronger assertion named by the packet.",
            },
            ReviewFixtureCase {
                name: "regressed",
                seam_id: "seam-regressed",
                movement: "regressed",
                before: "weakly_gripped",
                after: "ungripped",
                grip_class: "ungripped",
                action_kind: "regressed",
                action_summary: "Static grip regressed.",
                action_recommendation: "Revisit the test or code change before merge.",
            },
        ];

        for case in cases {
            let root = unique_agent_review_summary_test_dir(case.name);
            write_review_summary_case_artifacts(&root, &case)?;
            assert_review_summary_matches_fixture(&root, Path::new("."), case.name)?;
            std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        }
        Ok(())
    }

    #[test]
    fn agent_llm_work_loop_review_summary_fixture_pins_missing_artifact() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("missing-artifact");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        assert_review_summary_matches_fixture(&root, Path::new("."), "missing-artifact")?;
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_llm_work_loop_review_summary_fixture_pins_stale_artifact() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("stale-artifact");
        let case = ReviewFixtureCase {
            name: "stale-artifact",
            seam_id: "seam-stale",
            movement: "unchanged",
            before: "weakly_gripped",
            after: "weakly_gripped",
            grip_class: "weakly_gripped",
            action_kind: "unchanged",
            action_summary: "Static grip did not improve.",
            action_recommendation: "Add the missing discriminator or stronger assertion named by the packet.",
        };
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join(WORKFLOW_AGENT_BRIEF_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AGENT_PACKET_ARTIFACT), "{}")?;
        write_file(
            &root.join(WORKFLOW_MANIFEST_ARTIFACT),
            r#"{"schema_version":"0.1","tool":"ripr","status":"ready","seam":{"seam_id":"seam-stale","file":"src/pricing.rs","line":42,"seam_kind":"predicate_boundary"}}"#,
        )?;
        write_file(
            &root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
            r#"{"schema_version":"0.3","tool":"ripr","status":"advisory","provenance":{"before_class":"weakly_gripped","after_class":"weakly_gripped","movement":"unchanged","verify_artifact":{"path":"target/ripr/workflow/agent-verify.json","sha256":"sha256:verify"}},"seam":{"seam_id":"seam-stale","file":"src/pricing.rs","line":42,"seam_kind":"predicate_boundary","before":"weakly_gripped","after":"weakly_gripped","change":"unchanged","grip_class":"weakly_gripped"},"summary":{"remaining_gap":"Fixture-controlled static review state.","next_recommendation":"Add the missing discriminator or stronger assertion named by the packet.","next_action":{"kind":"unchanged","summary":"Static grip did not improve.","recommended_action":"Add the missing discriminator or stronger assertion named by the packet.","safe_to_merge":false}}}"#,
        )?;
        std::thread::sleep(std::time::Duration::from_millis(25));
        write_file(
            &root.join(WORKFLOW_AGENT_VERIFY_ARTIFACT),
            &serde_json::to_string_pretty(&serde_json::json!({
                "changed_seams": [{"seam_id": case.seam_id}],
                "unchanged_seams": [],
                "new_gaps": [],
                "resolved_gaps": []
            }))
            .map_err(|err| format!("render verify fixture: {err}"))?,
        )?;
        std::thread::sleep(std::time::Duration::from_millis(25));
        write_file(&root.join(WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AFTER_SNAPSHOT_ARTIFACT), "{}")?;
        assert_review_summary_matches_fixture(&root, Path::new("."), "stale-artifact")?;
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_llm_work_loop_review_summary_fixtures_pin_path_arguments() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("path-arguments");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;

        assert_review_summary_matches_fixture(&root, Path::new("repo root"), "path-with-spaces")?;
        assert_review_summary_matches_fixture(
            &root,
            Path::new("repo\\root"),
            "windows-separators",
        )?;
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_markdown_names_review_focus_and_limits() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("markdown");
        write_complete_artifacts(&root)?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_markdown(&report);

        assert!(rendered.contains("# RIPR Agent Review Summary"));
        assert!(rendered.contains("Target seam: seam-a"));
        assert!(rendered.contains("Movement: improved"));
        assert!(rendered.contains("Static artifact relationship only."));
        assert!(rendered.contains("No runtime mutation execution."));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_warns_for_invalid_optional_surface() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("invalid-surface");
        write_complete_artifacts(&root)?;
        write_file(&root.join(OPERATOR_COCKPIT_ARTIFACT), "{")?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse review summary JSON: {err}"))?;

        assert_eq!(value["status"], "warning");
        assert!(
            value["surfaces"]
                .as_array()
                .ok_or_else(|| "expected surfaces".to_string())?
                .iter()
                .any(|surface| surface["name"] == "operator_cockpit"
                    && surface["state"] == "invalid_json"
                    && surface["summary"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("could not be parsed as JSON"))
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_recovers_target_from_workflow() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("workflow-target");
        write_file(
            &root.join(WORKFLOW_MANIFEST_ARTIFACT),
            r#"{"status":"ready","seam":{"seam_id":"workflow-seam","file":"src/workflow.rs","line":7,"seam_kind":"branch"}}"#,
        )?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse review summary JSON: {err}"))?;

        assert_eq!(value["status"], "incomplete");
        assert_eq!(value["target_seam"]["seam_id"], "workflow-seam");
        assert_eq!(value["target_seam"]["source"], "agent_workflow");
        assert_eq!(value["target_seam"]["file"], "src/workflow.rs");
        assert_eq!(value["target_seam"]["line"], 7);
        assert_eq!(value["target_seam"]["seam_kind"], "branch");
        assert_eq!(value["static_movement"]["state"], "missing_artifact");

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_recovers_target_from_status_verify() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("status-target");
        write_file(
            &root.join(WORKFLOW_AGENT_VERIFY_ARTIFACT),
            r#"{"changed_seams":[],"unchanged_seams":[],"new_gaps":[{"seam_id":"verify-seam"}],"resolved_gaps":[]}"#,
        )?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse review summary JSON: {err}"))?;

        assert_eq!(value["target_seam"]["seam_id"], "verify-seam");
        assert_eq!(value["target_seam"]["source"], "agent_verify");
        assert_eq!(value["next_command"]["step"], "before_snapshot");
        assert!(
            value["next_command"]["command"]
                .as_str()
                .unwrap_or_default()
                .contains("repo-exposure-json")
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_treats_lsp_cockpit_as_optional() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("optional-lsp");
        write_complete_artifacts(&root)?;
        std::fs::remove_file(root.join(LSP_COCKPIT_ARTIFACT))
            .map_err(|err| format!("remove lsp cockpit: {err}"))?;
        write_file(
            &root.join(REPO_EXPOSURE_ARTIFACT),
            r#"{"status":"ready","summary":{"total_seams":3,"weakly_exposed":2}}"#,
        )?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse review summary JSON: {err}"))?;

        assert_eq!(value["status"], "ready");
        assert!(
            value["surfaces"]
                .as_array()
                .ok_or_else(|| "expected surfaces".to_string())?
                .iter()
                .any(|surface| surface["name"] == "lsp_cockpit"
                    && surface["state"] == "optional_missing")
        );
        assert!(
            value["surfaces"]
                .as_array()
                .ok_or_else(|| "expected surfaces".to_string())?
                .iter()
                .any(|surface| surface["name"] == "repo_exposure"
                    && surface["summary"]
                        == "Repo exposure artifact lists 3 seams and 2 weak seams.")
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_handles_receipt_without_next_action() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("no-next-action");
        write_complete_artifacts(&root)?;
        write_file(
            &root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
            r#"{"seam":{"seam_id":"seam-without-next","change":"unchanged"}}"#,
        )?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_json(&report)?;
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse review summary JSON: {err}"))?;

        assert_eq!(value["status"], "ready");
        assert_eq!(value["target_seam"]["seam_id"], "seam-without-next");
        assert_eq!(value["static_movement"]["state"], "unchanged");
        assert_eq!(value["static_movement"]["before_class"], Value::Null);
        assert_eq!(value["static_movement"]["verify_artifact"], Value::Null);
        assert_eq!(
            value["reviewer_summary"]["remaining"],
            "No next action was recovered from the available artifacts."
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_review_summary_markdown_includes_next_command_when_incomplete() -> Result<(), String> {
        let root = unique_agent_review_summary_test_dir("markdown-next-command");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;

        let report = build_agent_review_summary_report(&root, Path::new("."));
        let rendered = render_agent_review_summary_markdown(&report);

        assert!(rendered.contains("Target seam: unknown"));
        assert!(rendered.contains("Next command:"));
        assert!(rendered.contains("ripr check --root . --mode draft --format repo-exposure-json"));
        assert!(rendered.contains("No generated tests."));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }
}
