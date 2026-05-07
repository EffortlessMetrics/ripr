use serde_json::Value;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const AGENT_STATUS_SCHEMA_VERSION: &str = "0.1";

const BEFORE_SNAPSHOT_PATH: &str = "target/ripr/workflow/before.repo-exposure.json";
const AFTER_SNAPSHOT_PATH: &str = "target/ripr/workflow/after.repo-exposure.json";
const AGENT_BRIEF_PATH: &str = "target/ripr/workflow/agent-brief.json";
const AGENT_PACKET_PATH: &str = "target/ripr/workflow/agent-packet.json";
const AGENT_VERIFY_PATH: &str = "target/ripr/workflow/agent-verify.json";
const AGENT_RECEIPT_PATH: &str = "target/ripr/reports/agent-receipt.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AgentStatusArtifactDef {
    name: &'static str,
    label: &'static str,
    path: &'static str,
}

const ARTIFACTS: &[AgentStatusArtifactDef] = &[
    AgentStatusArtifactDef {
        name: "before_snapshot",
        label: "before snapshot",
        path: BEFORE_SNAPSHOT_PATH,
    },
    AgentStatusArtifactDef {
        name: "after_snapshot",
        label: "after snapshot",
        path: AFTER_SNAPSHOT_PATH,
    },
    AgentStatusArtifactDef {
        name: "agent_brief",
        label: "agent brief",
        path: AGENT_BRIEF_PATH,
    },
    AgentStatusArtifactDef {
        name: "agent_packet",
        label: "agent packet",
        path: AGENT_PACKET_PATH,
    },
    AgentStatusArtifactDef {
        name: "agent_verify",
        label: "agent verify",
        path: AGENT_VERIFY_PATH,
    },
    AgentStatusArtifactDef {
        name: "agent_receipt",
        label: "agent receipt",
        path: AGENT_RECEIPT_PATH,
    },
];

const MISSING_COMMAND_ORDER: &[&str] = &[
    "before_snapshot",
    "agent_packet",
    "agent_brief",
    "after_snapshot",
    "agent_verify",
    "agent_receipt",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentStatusReport {
    pub(crate) root: String,
    pub(crate) seam: Option<AgentStatusSeam>,
    pub(crate) artifacts: Vec<AgentStatusArtifact>,
    pub(crate) missing_commands: Vec<AgentStatusCommand>,
    pub(crate) warnings: Vec<AgentStatusWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentStatusSeam {
    pub(crate) seam_id: String,
    pub(crate) source: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentStatusArtifact {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) path: String,
    pub(crate) present: bool,
    pub(crate) bytes: Option<u64>,
    pub(crate) modified: Option<SystemTime>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentStatusCommand {
    pub(crate) step: String,
    pub(crate) artifact: String,
    pub(crate) reason: String,
    pub(crate) command: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentStatusWarning {
    pub(crate) kind: String,
    pub(crate) artifact: String,
    pub(crate) message: String,
}

impl AgentStatusReport {
    fn status(&self) -> &'static str {
        if self.artifacts.iter().any(|artifact| !artifact.present) {
            "incomplete"
        } else if self.warnings.is_empty() {
            "complete"
        } else {
            "warning"
        }
    }
}

pub(crate) fn build_agent_status_report(root: &Path, root_argument: &Path) -> AgentStatusReport {
    let artifacts = ARTIFACTS
        .iter()
        .map(|artifact| inspect_artifact(root, artifact))
        .collect::<Vec<_>>();
    let mut warnings = Vec::new();
    let seam = recover_seam_id(root, &artifacts, &mut warnings);
    warnings.extend(stale_warnings(&artifacts));
    let missing_commands = missing_commands(root_argument, seam.as_ref(), &artifacts);

    AgentStatusReport {
        root: report_path(root_argument),
        seam,
        artifacts,
        missing_commands,
        warnings,
    }
}

pub(crate) fn render_agent_status_json(report: &AgentStatusReport) -> Result<String, String> {
    let next_command = report
        .missing_commands
        .first()
        .map(agent_status_command_json);
    let value = serde_json::json!({
        "schema_version": AGENT_STATUS_SCHEMA_VERSION,
        "tool": "ripr",
        "status": report.status(),
        "root": report.root,
        "seam": report.seam.as_ref().map(agent_status_seam_json),
        "artifacts": report.artifacts.iter().map(agent_status_artifact_json).collect::<Vec<_>>(),
        "missing_commands": report.missing_commands.iter().map(agent_status_command_json).collect::<Vec<_>>(),
        "next_command": next_command,
        "warnings": report.warnings.iter().map(agent_status_warning_json).collect::<Vec<_>>()
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render agent status JSON: {err}"))
}

fn agent_status_seam_json(seam: &AgentStatusSeam) -> Value {
    serde_json::json!({
        "seam_id": seam.seam_id,
        "source": seam.source
    })
}

fn agent_status_artifact_json(artifact: &AgentStatusArtifact) -> Value {
    serde_json::json!({
        "name": artifact.name,
        "label": artifact.label,
        "path": artifact.path,
        "required": true,
        "state": if artifact.present { "present" } else { "missing" },
        "bytes": artifact.bytes,
        "modified_unix_ms": modified_unix_ms(artifact.modified)
    })
}

fn agent_status_command_json(command: &AgentStatusCommand) -> Value {
    serde_json::json!({
        "step": command.step,
        "artifact": command.artifact,
        "reason": command.reason,
        "command": command.command
    })
}

fn agent_status_warning_json(warning: &AgentStatusWarning) -> Value {
    serde_json::json!({
        "kind": warning.kind,
        "artifact": warning.artifact,
        "message": warning.message
    })
}

fn inspect_artifact(root: &Path, artifact: &AgentStatusArtifactDef) -> AgentStatusArtifact {
    let path = root.join(artifact.path);
    match std::fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => AgentStatusArtifact {
            name: artifact.name.to_string(),
            label: artifact.label.to_string(),
            path: artifact.path.to_string(),
            present: true,
            bytes: Some(metadata.len()),
            modified: metadata.modified().ok(),
        },
        _ => AgentStatusArtifact {
            name: artifact.name.to_string(),
            label: artifact.label.to_string(),
            path: artifact.path.to_string(),
            present: false,
            bytes: None,
            modified: None,
        },
    }
}

fn recover_seam_id(
    root: &Path,
    artifacts: &[AgentStatusArtifact],
    warnings: &mut Vec<AgentStatusWarning>,
) -> Option<AgentStatusSeam> {
    for (artifact_name, source) in [
        ("agent_receipt", "agent_receipt"),
        ("agent_verify", "agent_verify"),
        ("agent_packet", "agent_packet"),
        ("agent_brief", "agent_brief"),
    ] {
        let Some(artifact) = artifact_by_name(artifacts, artifact_name).filter(|a| a.present)
        else {
            continue;
        };
        let path = root.join(&artifact.path);
        let Some(value) = read_json_artifact(&path, artifact, warnings) else {
            continue;
        };
        if let Some(seam_id) = seam_id_from_source(&value, source) {
            return Some(AgentStatusSeam {
                seam_id,
                source: source.to_string(),
            });
        }
    }
    None
}

fn read_json_artifact(
    path: &Path,
    artifact: &AgentStatusArtifact,
    warnings: &mut Vec<AgentStatusWarning>,
) -> Option<Value> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            warnings.push(AgentStatusWarning {
                kind: "artifact_json_unreadable".to_string(),
                artifact: artifact.name.clone(),
                message: format!("could not read {}: {err}", artifact.path),
            });
            return None;
        }
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => Some(value),
        Err(err) => {
            warnings.push(AgentStatusWarning {
                kind: "artifact_json_unreadable".to_string(),
                artifact: artifact.name.clone(),
                message: format!("could not parse {} as JSON: {err}", artifact.path),
            });
            None
        }
    }
}

fn seam_id_from_source(value: &Value, source: &str) -> Option<String> {
    match source {
        "agent_receipt" => value
            .get("seam")
            .and_then(|seam| string_field(seam, "seam_id")),
        "agent_verify" => seam_id_from_verify(value),
        "agent_packet" => value
            .get("packets")
            .and_then(Value::as_array)
            .and_then(|packets| packets.first())
            .and_then(|packet| string_field(packet, "seam_id")),
        "agent_brief" => value
            .get("working_set")
            .and_then(|working_set| string_field(working_set, "seam_id"))
            .or_else(|| {
                value
                    .get("top_seams")
                    .and_then(Value::as_array)
                    .and_then(|seams| seams.first())
                    .and_then(|seam| string_field(seam, "seam_id"))
            }),
        _ => None,
    }
}

fn seam_id_from_verify(value: &Value) -> Option<String> {
    for bucket in [
        "changed_seams",
        "unchanged_seams",
        "new_gaps",
        "resolved_gaps",
    ] {
        let Some(seam_id) = value
            .get(bucket)
            .and_then(Value::as_array)
            .and_then(|seams| seams.first())
            .and_then(|seam| string_field(seam, "seam_id"))
        else {
            continue;
        };
        return Some(seam_id);
    }
    None
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn stale_warnings(artifacts: &[AgentStatusArtifact]) -> Vec<AgentStatusWarning> {
    let mut warnings = Vec::new();
    push_stale_warning(
        artifacts,
        "agent_verify",
        "before_snapshot",
        "agent verify is older than the before snapshot; rerun verify after refreshing snapshots",
        &mut warnings,
    );
    push_stale_warning(
        artifacts,
        "agent_verify",
        "after_snapshot",
        "agent verify is older than the after snapshot; rerun verify after refreshing snapshots",
        &mut warnings,
    );
    push_stale_warning(
        artifacts,
        "agent_receipt",
        "agent_verify",
        "agent receipt is older than agent verify; rerun receipt after refreshing verify",
        &mut warnings,
    );
    warnings
}

fn push_stale_warning(
    artifacts: &[AgentStatusArtifact],
    stale_candidate: &str,
    newer_input: &str,
    message: &str,
    warnings: &mut Vec<AgentStatusWarning>,
) {
    let Some(candidate) = artifact_by_name(artifacts, stale_candidate).filter(|a| a.present) else {
        return;
    };
    let Some(input) = artifact_by_name(artifacts, newer_input).filter(|a| a.present) else {
        return;
    };
    let (Some(candidate_modified), Some(input_modified)) = (candidate.modified, input.modified)
    else {
        return;
    };
    if candidate_modified.duration_since(input_modified).is_err() {
        warnings.push(AgentStatusWarning {
            kind: "stale_artifact".to_string(),
            artifact: candidate.name.clone(),
            message: message.to_string(),
        });
    }
}

fn missing_commands(
    root_argument: &Path,
    seam: Option<&AgentStatusSeam>,
    artifacts: &[AgentStatusArtifact],
) -> Vec<AgentStatusCommand> {
    let mut commands = Vec::new();
    for artifact_name in MISSING_COMMAND_ORDER {
        let Some(artifact) = artifact_by_name(artifacts, artifact_name) else {
            continue;
        };
        if artifact.present {
            continue;
        }
        commands.push(AgentStatusCommand {
            step: artifact.name.clone(),
            artifact: artifact.path.clone(),
            reason: format!("{} artifact is missing", artifact.label),
            command: command_for_missing_artifact(root_argument, seam, artifact),
        });
    }
    commands
}

fn command_for_missing_artifact(
    root_argument: &Path,
    seam: Option<&AgentStatusSeam>,
    artifact: &AgentStatusArtifact,
) -> String {
    let root = shell_arg(&report_path(root_argument));
    let seam_id = seam
        .map(|seam| seam.seam_id.as_str())
        .unwrap_or("<seam-id>");
    match artifact.name.as_str() {
        "before_snapshot" => format!(
            "ripr check --root {root} --mode draft --format repo-exposure-json > {BEFORE_SNAPSHOT_PATH}"
        ),
        "after_snapshot" => format!(
            "ripr check --root {root} --mode draft --format repo-exposure-json > {AFTER_SNAPSHOT_PATH}"
        ),
        "agent_packet" => format!(
            "ripr agent packet --root {root} --seam-id {} --json > {AGENT_PACKET_PATH}",
            shell_arg(seam_id)
        ),
        "agent_brief" => format!(
            "ripr agent brief --root {root} --seam-id {} --json > {AGENT_BRIEF_PATH}",
            shell_arg(seam_id)
        ),
        "agent_verify" => format!(
            "ripr agent verify --root {root} --before {BEFORE_SNAPSHOT_PATH} --after {AFTER_SNAPSHOT_PATH} --json > {AGENT_VERIFY_PATH}"
        ),
        "agent_receipt" => format!(
            "ripr agent receipt --root {root} --verify-json {AGENT_VERIFY_PATH} --seam-id {} --json --out {AGENT_RECEIPT_PATH}",
            shell_arg(seam_id)
        ),
        _ => String::new(),
    }
}

fn artifact_by_name<'a>(
    artifacts: &'a [AgentStatusArtifact],
    name: &str,
) -> Option<&'a AgentStatusArtifact> {
    artifacts.iter().find(|artifact| artifact.name == name)
}

fn report_path(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.is_empty() {
        ".".to_string()
    } else {
        text
    }
}

fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '\\' | '_' | '-' | ':'))
    {
        return value.to_string();
    }
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn modified_unix_ms(time: Option<SystemTime>) -> Option<u64> {
    let millis = time?.duration_since(UNIX_EPOCH).ok()?.as_millis();
    u64::try_from(millis).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    fn unique_agent_status_test_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "ripr-agent-status-{label}-{}-{stamp}",
            std::process::id()
        ))
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

    fn artifact(name: &str, present: bool, modified: Option<SystemTime>) -> AgentStatusArtifact {
        AgentStatusArtifact {
            name: name.to_string(),
            label: name.replace('_', " "),
            path: format!("target/ripr/{name}.json"),
            present,
            bytes: Some(1),
            modified,
        }
    }

    #[test]
    fn agent_status_reports_missing_artifacts_and_next_commands() -> Result<(), String> {
        let root = unique_agent_status_test_dir("missing");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;

        let report = build_agent_status_report(&root, Path::new("."));
        let rendered = render_agent_status_json(&report)?;
        let value: Value =
            serde_json::from_str(&rendered).map_err(|err| format!("parse status JSON: {err}"))?;

        assert_eq!(value["schema_version"], AGENT_STATUS_SCHEMA_VERSION);
        assert_eq!(value["status"], "incomplete");
        assert_eq!(value["seam"], Value::Null);
        assert_eq!(value["artifacts"].as_array().map(Vec::len), Some(6));
        assert_eq!(value["missing_commands"].as_array().map(Vec::len), Some(6));
        assert_eq!(value["next_command"]["step"], "before_snapshot");
        assert_eq!(
            value["next_command"]["command"],
            "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json"
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_status_reports_complete_when_all_artifacts_are_present() -> Result<(), String> {
        let root = unique_agent_status_test_dir("complete");
        write_file(&root.join(BEFORE_SNAPSHOT_PATH), "{}")?;
        write_file(&root.join(AFTER_SNAPSHOT_PATH), "{}")?;
        write_file(&root.join(AGENT_BRIEF_PATH), "{}")?;
        write_file(&root.join(AGENT_PACKET_PATH), "{}")?;
        write_file(
            &root.join(AGENT_VERIFY_PATH),
            r#"{
  "changed_seams": [],
  "unchanged_seams": [{"seam_id": "from-verify"}],
  "new_gaps": [],
  "resolved_gaps": []
}"#,
        )?;
        write_file(
            &root.join(AGENT_RECEIPT_PATH),
            r#"{"seam":{"seam_id":"from-receipt"}}"#,
        )?;

        let report = build_agent_status_report(&root, Path::new("."));
        let rendered = render_agent_status_json(&report)?;
        let value: Value =
            serde_json::from_str(&rendered).map_err(|err| format!("parse status JSON: {err}"))?;

        assert_eq!(value["status"], "complete");
        assert_eq!(value["seam"]["seam_id"], "from-receipt");
        assert_eq!(value["seam"]["source"], "agent_receipt");
        assert_eq!(value["missing_commands"].as_array().map(Vec::len), Some(0));
        assert_eq!(value["next_command"], Value::Null);
        assert_eq!(
            value["artifacts"][0]["state"],
            serde_json::Value::String("present".to_string())
        );
        assert!(value["artifacts"][0]["bytes"].as_u64().is_some());
        assert!(value["artifacts"][0]["modified_unix_ms"].as_u64().is_some());

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_status_recovers_seam_id_from_receipt() -> Result<(), String> {
        let root = unique_agent_status_test_dir("receipt");
        write_file(
            &root.join(AGENT_RECEIPT_PATH),
            r#"{"seam":{"seam_id":"67fc764ba37d77bd"}}"#,
        )?;

        let report = build_agent_status_report(&root, Path::new("repo root"));
        let seam = report
            .seam
            .as_ref()
            .ok_or_else(|| "expected recovered seam".to_string())?;

        assert_eq!(seam.seam_id, "67fc764ba37d77bd");
        assert_eq!(seam.source, "agent_receipt");
        assert!(report.missing_commands.iter().any(|command| {
            command.step == "agent_packet"
                && command.command
                    == "ripr agent packet --root \"repo root\" --seam-id 67fc764ba37d77bd --json > target/ripr/workflow/agent-packet.json"
        }));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_status_warns_for_malformed_present_json() -> Result<(), String> {
        let root = unique_agent_status_test_dir("malformed");
        write_file(&root.join(AGENT_RECEIPT_PATH), "{not json")?;

        let report = build_agent_status_report(&root, Path::new("."));

        assert!(report.seam.is_none());
        assert!(report.warnings.iter().any(|warning| {
            warning.kind == "artifact_json_unreadable" && warning.artifact == "agent_receipt"
        }));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_status_recovers_seam_id_from_verify_packet_or_brief() {
        let verify: Value = serde_json::json!({
            "changed_seams": [],
            "unchanged_seams": [{"seam_id": "from-verify"}],
            "new_gaps": [],
            "resolved_gaps": []
        });
        let packet: Value = serde_json::json!({
            "packets": [{"seam_id": "from-packet"}]
        });
        let brief: Value = serde_json::json!({
            "working_set": {"seam_id": "from-brief"},
            "top_seams": [{"seam_id": "from-top-seam"}]
        });

        assert_eq!(
            seam_id_from_source(&verify, "agent_verify"),
            Some("from-verify".to_string())
        );
        assert_eq!(
            seam_id_from_source(&packet, "agent_packet"),
            Some("from-packet".to_string())
        );
        assert_eq!(
            seam_id_from_source(&brief, "agent_brief"),
            Some("from-brief".to_string())
        );
    }

    #[test]
    fn agent_status_recovers_brief_top_seam_without_working_set_seam() {
        let brief: Value = serde_json::json!({
            "working_set": {"seam_id": null},
            "top_seams": [{"seam_id": "from-top-seam"}]
        });

        assert_eq!(
            seam_id_from_source(&brief, "agent_brief"),
            Some("from-top-seam".to_string())
        );
    }

    #[test]
    fn agent_status_warns_when_verify_or_receipt_look_stale() {
        let early = UNIX_EPOCH + Duration::from_secs(1);
        let middle = UNIX_EPOCH + Duration::from_secs(2);
        let late = UNIX_EPOCH + Duration::from_secs(3);
        let warnings = stale_warnings(&[
            artifact("before_snapshot", true, Some(late)),
            artifact("after_snapshot", true, Some(late)),
            artifact("agent_verify", true, Some(middle)),
            artifact("agent_receipt", true, Some(early)),
        ]);

        assert_eq!(warnings.len(), 3);
        assert!(warnings.iter().any(|warning| {
            warning.artifact == "agent_verify"
                && warning.message.contains("older than the before snapshot")
        }));
        assert!(warnings.iter().any(|warning| {
            warning.artifact == "agent_receipt"
                && warning.message.contains("older than agent verify")
        }));
    }

    #[test]
    fn agent_status_emits_all_missing_command_templates() {
        let artifacts = ARTIFACTS
            .iter()
            .map(|def| AgentStatusArtifact {
                name: def.name.to_string(),
                label: def.label.to_string(),
                path: def.path.to_string(),
                present: false,
                bytes: None,
                modified: None,
            })
            .collect::<Vec<_>>();
        let seam = AgentStatusSeam {
            seam_id: "seam-a".to_string(),
            source: "agent_verify".to_string(),
        };
        let commands = missing_commands(Path::new("."), Some(&seam), &artifacts);

        assert_eq!(commands.len(), 6);
        assert!(commands.iter().any(|command| {
            command.step == "after_snapshot"
                && command.command
                    == "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json"
        }));
        assert!(commands.iter().any(|command| {
            command.step == "agent_brief"
                && command.command
                    == "ripr agent brief --root . --seam-id seam-a --json > target/ripr/workflow/agent-brief.json"
        }));
        assert!(commands.iter().any(|command| {
            command.step == "agent_verify"
                && command.command
                    == "ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json > target/ripr/workflow/agent-verify.json"
        }));
        assert!(commands.iter().any(|command| {
            command.step == "agent_receipt"
                && command.command
                    == "ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id seam-a --json --out target/ripr/reports/agent-receipt.json"
        }));
    }

    #[test]
    fn agent_status_quotes_paths_with_spaces() {
        assert_eq!(shell_arg("repo root"), "\"repo root\"");
        assert_eq!(shell_arg("target/ripr/workflow"), "target/ripr/workflow");
    }
}
