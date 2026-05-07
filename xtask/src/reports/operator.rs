use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "../../../crates/ripr/src/agent/loop_commands.rs"]
mod loop_commands;

const TOP_WEAK_SEAMS_LIMIT: usize = 5;
const BEFORE_SNAPSHOT_COMMAND: &str = "ripr pilot --out target/ripr/pilot";
const TARGETED_OUTCOME_ARTIFACT: &str = "target/ripr/reports/targeted-test-outcome.json";
const RECEIPT_SEAM_ID_PLACEHOLDER: &str = "__RIPR_SEAM_ID_PLACEHOLDER__:unset";

#[derive(Clone, Debug)]
struct OperatorArtifact {
    name: String,
    path: String,
    state: String,
    status: String,
    command: String,
    required: bool,
    summary: String,
    value: Option<Value>,
}

#[derive(Clone, Debug)]
struct OperatorCockpitReport {
    status: String,
    inputs: Vec<OperatorInputSummary>,
    top_weak_seams: Vec<OperatorWeakSeam>,
    surface_alignment: Vec<OperatorSurfaceAlignment>,
    next_commands: Vec<OperatorNextCommand>,
}

#[derive(Clone, Debug)]
struct OperatorInputSummary {
    name: String,
    path: String,
    state: String,
    status: String,
    command: String,
    required: bool,
    summary: String,
}

#[derive(Clone, Debug)]
struct OperatorWeakSeam {
    seam_id: String,
    seam_kind: String,
    file: String,
    line: usize,
    owner: String,
    expression: String,
    grip_class: String,
    why_it_matters: String,
    suggested_next_targeted_test: String,
    best_related_test: Option<OperatorRelatedTest>,
}

#[derive(Clone, Debug)]
struct OperatorRelatedTest {
    name: String,
    file: String,
    line: usize,
    oracle_strength: String,
}

#[derive(Clone, Debug)]
struct OperatorSurfaceAlignment {
    surface: String,
    state: String,
    status: String,
    agreement: String,
    signal: String,
    command: String,
}

#[derive(Clone, Debug)]
struct OperatorNextCommand {
    command: String,
    reason: String,
}

pub(crate) fn operator_cockpit_report() -> Result<(), String> {
    let report = build_operator_cockpit_report_at(&reports_dir());
    crate::write_report("operator-cockpit.json", &operator_cockpit_json(&report)?)?;
    crate::write_report("operator-cockpit.md", &operator_cockpit_markdown(&report))
}

fn build_operator_cockpit_report_at(report_dir: &Path) -> OperatorCockpitReport {
    keep_shared_loop_templates_reachable();
    let repo_exposure = read_artifact(
        report_dir,
        "repo exposure",
        "repo-exposure.json",
        "cargo xtask repo-exposure-report",
        true,
    );
    let lsp = read_artifact(
        report_dir,
        "LSP cockpit",
        "lsp-cockpit.json",
        "cargo xtask lsp-cockpit-report",
        true,
    );
    let sarif = read_artifact(
        report_dir,
        "SARIF policy",
        "sarif-policy.json",
        "cargo xtask sarif-policy --current target/ripr/workflow/current.repo-sarif.json",
        true,
    );
    let badge = read_first_artifact(
        report_dir,
        "badge status",
        &["repo-ripr-badge.json", "ripr-badge.json"],
        "cargo xtask repo-badge-artifacts",
        true,
    );
    let targeted_outcome = read_artifact(
        report_dir,
        "targeted-test outcome",
        "targeted-test-outcome.json",
        &targeted_outcome_command(),
        true,
    );
    let before_snapshot = read_workspace_artifact(
        report_dir,
        "before snapshot",
        loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
        BEFORE_SNAPSHOT_COMMAND,
        true,
    );
    let after_snapshot = read_workspace_artifact(
        report_dir,
        "after snapshot",
        loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
        &after_snapshot_command(),
        true,
    );
    let agent_verify = read_workspace_artifact(
        report_dir,
        "agent verify",
        loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT,
        &agent_verify_command(),
        true,
    );
    let agent_receipt = read_workspace_artifact(
        report_dir,
        "agent receipt",
        loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT,
        &receipt_command_for(None),
        true,
    );
    let calibration = read_artifact(
        report_dir,
        "mutation calibration",
        "mutation-calibration.json",
        "cargo xtask mutation-calibration . --mutants-json target/mutants/outcomes.json --repo-exposure-json target/ripr/reports/repo-exposure.json",
        false,
    );

    let artifacts = vec![
        repo_exposure,
        lsp,
        before_snapshot,
        after_snapshot,
        agent_verify,
        agent_receipt,
        sarif,
        badge,
        targeted_outcome,
        calibration,
    ];
    let top_weak_seams = artifacts
        .iter()
        .find(|artifact| artifact.name == "repo exposure")
        .and_then(|artifact| artifact.value.as_ref())
        .map(top_weak_seams)
        .unwrap_or_default();
    let surface_alignment = artifacts
        .iter()
        .map(|artifact| artifact_surface_alignment(artifact, &top_weak_seams))
        .collect::<Vec<_>>();
    let next_commands = operator_next_commands(&artifacts, &top_weak_seams);
    let status = operator_status(&artifacts, &top_weak_seams, &surface_alignment).to_string();
    let inputs = artifacts.iter().map(artifact_input_summary).collect();

    OperatorCockpitReport {
        status,
        inputs,
        top_weak_seams,
        surface_alignment,
        next_commands,
    }
}

fn keep_shared_loop_templates_reachable() {
    let workflow_artifacts =
        loop_commands::agent_workflow_artifacts(Path::new("target/ripr/workflow"));
    let workflow_commands =
        loop_commands::agent_workflow_commands(".", "draft", "seam-id", &workflow_artifacts);

    drop((
        loop_commands::WORKFLOW_BEFORE_SNAPSHOT_FILE,
        loop_commands::WORKFLOW_AFTER_SNAPSHOT_FILE,
        loop_commands::WORKFLOW_AGENT_PACKET_FILE,
        loop_commands::WORKFLOW_AGENT_BRIEF_FILE,
        loop_commands::WORKFLOW_AGENT_VERIFY_FILE,
        loop_commands::WORKFLOW_AGENT_WORKFLOW_JSON_FILE,
        loop_commands::WORKFLOW_AGENT_WORKFLOW_MARKDOWN_FILE,
        loop_commands::WORKFLOW_AGENT_STATUS_FILE,
        loop_commands::WORKFLOW_AGENT_REVIEW_SUMMARY_FILE,
        loop_commands::WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
        loop_commands::WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
        loop_commands::WORKFLOW_AGENT_SEAM_PACKETS_ARTIFACT,
        loop_commands::WORKFLOW_AGENT_PACKET_ARTIFACT,
        loop_commands::WORKFLOW_AGENT_BRIEF_ARTIFACT,
        loop_commands::WORKFLOW_AGENT_VERIFY_ARTIFACT,
        loop_commands::WORKFLOW_AGENT_RECEIPT_ARTIFACT,
        loop_commands::WORKFLOW_AGENT_STATUS_ARTIFACT,
        loop_commands::WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT,
        loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
        loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
        loop_commands::EDITOR_AGENT_PACKET_ARTIFACT,
        loop_commands::EDITOR_AGENT_BRIEF_ARTIFACT,
        loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT,
        loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT,
        loop_commands::agent_seam_packets_command(
            ".",
            "draft",
            loop_commands::WORKFLOW_AGENT_SEAM_PACKETS_ARTIFACT,
        ),
        loop_commands::agent_packet_command(
            ".",
            "seam-id",
            loop_commands::WORKFLOW_AGENT_PACKET_ARTIFACT,
        ),
        loop_commands::agent_brief_command(
            ".",
            "seam-id",
            loop_commands::WORKFLOW_AGENT_BRIEF_ARTIFACT,
        ),
        loop_commands::agent_status_command(
            ".",
            Some(loop_commands::WORKFLOW_AGENT_STATUS_ARTIFACT),
        ),
        loop_commands::agent_review_summary_command(
            ".",
            Some(loop_commands::WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT),
        ),
        workflow_artifacts.before_snapshot,
        workflow_artifacts.after_snapshot,
        workflow_artifacts.agent_packet,
        workflow_artifacts.agent_brief,
        workflow_artifacts.agent_verify,
        workflow_artifacts.agent_receipt,
        workflow_artifacts.agent_status,
        workflow_artifacts.review_summary,
        workflow_commands.before_snapshot,
        workflow_commands.agent_packet,
        workflow_commands.agent_brief,
        workflow_commands.after_snapshot,
        workflow_commands.agent_verify,
        workflow_commands.agent_receipt,
        workflow_commands.agent_status,
        workflow_commands.review_summary,
        loop_commands::display_path(Path::new(".")),
        loop_commands::shell_path(Path::new(".")),
    ));
}

fn read_workspace_artifact(
    report_dir: &Path,
    name: &str,
    relative_path: &str,
    command: &str,
    required: bool,
) -> OperatorArtifact {
    read_artifact_path(
        workspace_artifact_path(report_dir, relative_path),
        name,
        command,
        required,
    )
}

fn read_artifact(
    report_dir: &Path,
    name: &str,
    file: &str,
    command: &str,
    required: bool,
) -> OperatorArtifact {
    read_artifact_path(report_dir.join(file), name, command, required)
}

fn read_first_artifact(
    report_dir: &Path,
    name: &str,
    files: &[&str],
    command: &str,
    required: bool,
) -> OperatorArtifact {
    for file in files {
        let path = report_dir.join(file);
        if path.exists() {
            return read_artifact_path(path, name, command, required);
        }
    }
    let file = files.first().copied().unwrap_or("unknown.json");
    read_artifact_path(report_dir.join(file), name, command, required)
}

fn read_artifact_path(
    path: PathBuf,
    name: &str,
    command: &str,
    required: bool,
) -> OperatorArtifact {
    let normalized = normalize_path(&path);
    if !path.exists() {
        return OperatorArtifact {
            name: name.to_string(),
            path: normalized,
            state: if required {
                "missing".to_string()
            } else {
                "optional_missing".to_string()
            },
            status: if required {
                "missing".to_string()
            } else {
                "optional".to_string()
            },
            command: command.to_string(),
            required,
            summary: if required {
                "Report has not been generated yet.".to_string()
            } else {
                "Optional calibration report has not been generated.".to_string()
            },
            value: None,
        };
    }

    match fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(value) => {
                let status = value
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("present")
                    .to_string();
                let summary = artifact_summary(name, &value);
                OperatorArtifact {
                    name: name.to_string(),
                    path: normalized,
                    state: "present".to_string(),
                    status,
                    command: command.to_string(),
                    required,
                    summary,
                    value: Some(value),
                }
            }
            Err(err) => OperatorArtifact {
                name: name.to_string(),
                path: normalized,
                state: "invalid_json".to_string(),
                status: "warn".to_string(),
                command: command.to_string(),
                required,
                summary: format!("Could not parse report JSON: {err}."),
                value: None,
            },
        },
        Err(err) => OperatorArtifact {
            name: name.to_string(),
            path: normalized,
            state: "unreadable".to_string(),
            status: "warn".to_string(),
            command: command.to_string(),
            required,
            summary: format!("Could not read report: {err}."),
            value: None,
        },
    }
}

fn artifact_summary(name: &str, value: &Value) -> String {
    match name {
        "repo exposure" => repo_exposure_summary(value),
        "before snapshot" | "after snapshot" => repo_exposure_summary(value),
        "LSP cockpit" => lsp_summary(value),
        "SARIF policy" => sarif_summary(value),
        "badge status" => badge_summary(value),
        "targeted-test outcome" => targeted_outcome_summary(value),
        "agent verify" => agent_verify_summary(value),
        "agent receipt" => agent_receipt_summary(value),
        "mutation calibration" => calibration_summary(value),
        _ => "Report is present.".to_string(),
    }
}

fn repo_exposure_summary(value: &Value) -> String {
    let metrics = value.get("metrics").and_then(Value::as_object);
    let seams_total = metrics
        .and_then(|metrics| usize_field(metrics, "seams_total"))
        .unwrap_or(0);
    let weakly_gripped = metrics
        .and_then(|metrics| usize_field(metrics, "weakly_gripped"))
        .unwrap_or(0);
    let ungripped = metrics
        .and_then(|metrics| usize_field(metrics, "ungripped"))
        .unwrap_or(0);
    let reachable = metrics
        .and_then(|metrics| usize_field(metrics, "reachable_unrevealed"))
        .unwrap_or(0);
    format!(
        "{seams_total} seams; {weakly_gripped} weakly_gripped, {ungripped} ungripped, {reachable} reachable_unrevealed."
    )
}

fn lsp_summary(value: &Value) -> String {
    let fixtures = value
        .get("fixtures")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let uncovered = value
        .get("vscode_e2e")
        .and_then(|vscode| vscode.get("uncovered_contributed_commands"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    format!("{fixtures} LSP fixture reports; {uncovered} uncovered contributed VS Code commands.")
}

fn sarif_summary(value: &Value) -> String {
    let new_results = value
        .get("new_results_total")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mode = value
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    format!("{new_results} new configured-threshold SARIF results in {mode} mode.")
}

fn badge_summary(value: &Value) -> String {
    let message = value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let counts = value.get("counts").and_then(Value::as_object);
    let analyzed = counts
        .and_then(|counts| usize_field(counts, "analyzed_seams"))
        .or_else(|| counts.and_then(|counts| usize_field(counts, "analyzed_findings")))
        .unwrap_or(0);
    format!("Badge headline {message}; analyzed surface count {analyzed}.")
}

fn targeted_outcome_summary(value: &Value) -> String {
    let summary = value.get("summary").and_then(Value::as_object);
    let moved = summary
        .and_then(|summary| usize_field(summary, "moved"))
        .unwrap_or(0);
    let regressed = summary
        .and_then(|summary| usize_field(summary, "regressed"))
        .unwrap_or(0);
    let unchanged = summary
        .and_then(|summary| usize_field(summary, "unchanged"))
        .unwrap_or(0);
    format!("{moved} moved, {regressed} regressed, {unchanged} unchanged seams.")
}

fn agent_verify_summary(value: &Value) -> String {
    let summary = value.get("summary").and_then(Value::as_object);
    let improved = summary
        .and_then(|summary| usize_field(summary, "improved"))
        .unwrap_or(0);
    let changed = summary
        .and_then(|summary| usize_field(summary, "changed"))
        .unwrap_or(0);
    let regressed = summary
        .and_then(|summary| usize_field(summary, "regressed"))
        .unwrap_or(0);
    let unchanged = summary
        .and_then(|summary| usize_field(summary, "unchanged"))
        .unwrap_or(0);
    format!(
        "{improved} improved, {changed} changed, {regressed} regressed, {unchanged} unchanged seams."
    )
}

fn agent_receipt_summary(value: &Value) -> String {
    let seam = value.get("seam").and_then(Value::as_object);
    let seam_id = seam
        .and_then(|seam| seam.get("seam_id"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let change = seam
        .and_then(|seam| seam.get("change"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let before = seam
        .and_then(|seam| seam.get("before"))
        .and_then(Value::as_str)
        .unwrap_or("n/a");
    let after = seam
        .and_then(|seam| seam.get("after"))
        .and_then(Value::as_str)
        .unwrap_or("n/a");
    let remaining_gap = value
        .get("summary")
        .and_then(|summary| summary.get("remaining_gap"))
        .and_then(Value::as_str)
        .unwrap_or("No receipt summary was available.");
    format!("Receipt for seam {seam_id}: {change}; before {before}, after {after}. {remaining_gap}")
}

fn calibration_summary(value: &Value) -> String {
    let metrics = value.get("metrics").and_then(Value::as_object);
    let matched = metrics
        .and_then(|metrics| usize_field(metrics, "matched_total"))
        .unwrap_or(0);
    let ambiguous = metrics
        .and_then(|metrics| usize_field(metrics, "ambiguous_file_line_total"))
        .unwrap_or(0);
    format!("{matched} matched runtime records; {ambiguous} ambiguous file/line joins.")
}

fn artifact_input_summary(artifact: &OperatorArtifact) -> OperatorInputSummary {
    OperatorInputSummary {
        name: artifact.name.clone(),
        path: artifact.path.clone(),
        state: artifact.state.clone(),
        status: artifact.status.clone(),
        command: artifact.command.clone(),
        required: artifact.required,
        summary: artifact.summary.clone(),
    }
}

fn artifact_surface_alignment(
    artifact: &OperatorArtifact,
    top_weak_seams: &[OperatorWeakSeam],
) -> OperatorSurfaceAlignment {
    let agreement = artifact_agreement(artifact, top_weak_seams);
    OperatorSurfaceAlignment {
        surface: artifact.name.clone(),
        state: artifact.state.clone(),
        status: artifact.status.clone(),
        agreement,
        signal: artifact.summary.clone(),
        command: artifact.command.clone(),
    }
}

fn artifact_agreement(artifact: &OperatorArtifact, top_weak_seams: &[OperatorWeakSeam]) -> String {
    if artifact.state == "missing" || artifact.state == "optional_missing" {
        return "not_available".to_string();
    }
    if artifact.state != "present" {
        return "needs_regeneration".to_string();
    }
    match artifact.name.as_str() {
        "repo exposure" => {
            if top_weak_seams.is_empty() {
                "no_headline_weak_seams".to_string()
            } else {
                "actionable_seams_visible".to_string()
            }
        }
        "before snapshot" => "before_snapshot_available".to_string(),
        "after snapshot" => "after_snapshot_available".to_string(),
        "agent verify" => "agent_verify_counts_available".to_string(),
        "agent receipt" => "agent_receipt_available".to_string(),
        "LSP cockpit" if artifact.status == "pass" => "editor_contract_green".to_string(),
        "LSP cockpit" => "editor_contract_needs_review".to_string(),
        "SARIF policy" if artifact.status == "pass" => "policy_agrees_clean".to_string(),
        "SARIF policy" if artifact.status == "new_results" => {
            "policy_reports_new_results".to_string()
        }
        "SARIF policy" if artifact.status == "advisory_missing_baseline" => {
            "policy_advisory_missing_baseline".to_string()
        }
        "badge status" => "badge_artifact_present".to_string(),
        "targeted-test outcome" => "targeted_outcome_artifact_present".to_string(),
        "mutation calibration" => "calibration_artifact_present".to_string(),
        _ => "present".to_string(),
    }
}

fn operator_status(
    artifacts: &[OperatorArtifact],
    top_weak_seams: &[OperatorWeakSeam],
    alignment: &[OperatorSurfaceAlignment],
) -> &'static str {
    if artifacts
        .iter()
        .any(|artifact| artifact.required && artifact.state != "present")
    {
        return "warn";
    }
    if alignment.iter().any(|surface| {
        surface.status == "fail"
            || surface.status == "failed"
            || surface.status == "warn"
            || surface.status == "warning"
            || surface.status == "new_results"
            || surface.status == "advisory_missing_baseline"
            || surface.agreement == "needs_regeneration"
            || surface.agreement == "editor_contract_needs_review"
    }) {
        return "warn";
    }
    if !top_weak_seams.is_empty() {
        return "warn";
    }
    "pass"
}

fn operator_next_commands(
    artifacts: &[OperatorArtifact],
    top_weak_seams: &[OperatorWeakSeam],
) -> Vec<OperatorNextCommand> {
    let mut commands = Vec::new();
    let mut seen = BTreeSet::new();
    for artifact in artifacts {
        if !artifact.required || artifact.state == "present" {
            continue;
        }
        let command = missing_artifact_command(artifact, top_weak_seams);
        push_next_command(
            &mut commands,
            &mut seen,
            &command,
            &format!("Generate the missing {} input.", artifact.name),
        );
    }

    if !top_weak_seams.is_empty() {
        push_next_command(
            &mut commands,
            &mut seen,
            BEFORE_SNAPSHOT_COMMAND,
            "Open the top actionable seam packet and write one focused targeted test.",
        );
        push_next_command(
            &mut commands,
            &mut seen,
            &after_snapshot_command(),
            "After adding the targeted test, capture the after repo-exposure snapshot.",
        );
        push_next_command(
            &mut commands,
            &mut seen,
            &agent_verify_command(),
            "Compare the before and after static evidence snapshots for the agent loop.",
        );
        push_next_command(
            &mut commands,
            &mut seen,
            &receipt_command_for(top_weak_seams.first().map(|seam| seam.seam_id.as_str())),
            "Write a focused receipt for the top seam after agent verify completes.",
        );
        push_next_command(
            &mut commands,
            &mut seen,
            &targeted_outcome_command(),
            "Compare the before and after static evidence snapshots.",
        );
    } else if commands.is_empty() {
        push_next_command(
            &mut commands,
            &mut seen,
            "cargo xtask reports index",
            "Refresh the report index after the cockpit inputs are current.",
        );
    }
    commands
}

fn missing_artifact_command(
    artifact: &OperatorArtifact,
    top_weak_seams: &[OperatorWeakSeam],
) -> String {
    if artifact.name == "agent receipt" {
        return receipt_command_for(top_weak_seams.first().map(|seam| seam.seam_id.as_str()));
    }
    artifact.command.clone()
}

fn receipt_command_for(seam_id: Option<&str>) -> String {
    let seam_id = seam_id.unwrap_or(RECEIPT_SEAM_ID_PLACEHOLDER);
    loop_commands::agent_receipt_command(
        ".",
        loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT,
        seam_id,
        Some(loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT),
    )
    .replace(RECEIPT_SEAM_ID_PLACEHOLDER, "<seam-id>")
}

fn after_snapshot_command() -> String {
    loop_commands::check_repo_exposure_command(
        ".",
        "draft",
        loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
    )
}

fn agent_verify_command() -> String {
    loop_commands::agent_verify_command(
        ".",
        loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
        loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
        Some(loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT),
    )
}

fn targeted_outcome_command() -> String {
    loop_commands::outcome_command(
        loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
        loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
        Some(TARGETED_OUTCOME_ARTIFACT),
    )
}

fn push_next_command(
    commands: &mut Vec<OperatorNextCommand>,
    seen: &mut BTreeSet<String>,
    command: &str,
    reason: &str,
) {
    if seen.insert(command.to_string()) {
        commands.push(OperatorNextCommand {
            command: command.to_string(),
            reason: reason.to_string(),
        });
    }
}

fn top_weak_seams(value: &Value) -> Vec<OperatorWeakSeam> {
    let mut seams = value
        .get("seams")
        .and_then(Value::as_array)
        .map(|seams| seams.iter().filter_map(parse_weak_seam).collect::<Vec<_>>())
        .unwrap_or_default();
    seams.sort_by(|left, right| {
        weak_seam_rank(&left.grip_class)
            .cmp(&weak_seam_rank(&right.grip_class))
            .then_with(|| left.file.cmp(&right.file))
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.seam_id.cmp(&right.seam_id))
    });
    seams.truncate(TOP_WEAK_SEAMS_LIMIT);
    seams
}

fn parse_weak_seam(value: &Value) -> Option<OperatorWeakSeam> {
    let grip_class = value.get("grip_class")?.as_str()?.to_string();
    if !is_operator_attention_class(&grip_class) {
        return None;
    }
    if value
        .get("headline_eligible")
        .and_then(Value::as_bool)
        .is_some_and(|eligible| !eligible)
    {
        return None;
    }

    let seam_id = string_field(value, "seam_id")?;
    let seam_kind = string_field(value, "kind").unwrap_or_else(|| "unknown".to_string());
    let file = string_field(value, "file").unwrap_or_else(|| "unknown".to_string());
    let line = usize_value(value.get("line")).unwrap_or(0);
    let owner = string_field(value, "owner").unwrap_or_else(|| "unknown".to_string());
    let expression = string_field(value, "expression").unwrap_or_default();
    let best_related_test = value
        .get("related_tests")
        .and_then(Value::as_array)
        .and_then(|tests| tests.first())
        .and_then(parse_related_test);
    let (why_it_matters, suggested_next_targeted_test) =
        weak_seam_guidance(value, &grip_class, &seam_kind, &owner, &best_related_test);

    Some(OperatorWeakSeam {
        seam_id,
        seam_kind,
        file,
        line,
        owner,
        expression,
        grip_class,
        why_it_matters,
        suggested_next_targeted_test,
        best_related_test,
    })
}

fn weak_seam_guidance(
    value: &Value,
    grip_class: &str,
    seam_kind: &str,
    owner: &str,
    best_related_test: &Option<OperatorRelatedTest>,
) -> (String, String) {
    if let Some((missing_value, reason)) = first_missing_discriminator(value) {
        return (
            reason.clone(),
            format!(
                "Add a focused {seam_kind} test for `{owner}` that exercises `{missing_value}` and asserts the observable result."
            ),
        );
    }

    if let Some(test) = best_related_test {
        return (
            format!(
                "RIPR can relate `{}` to this seam, but the current static grip is `{grip_class}`.",
                test.name
            ),
            format!(
                "Add or strengthen a focused assertion near `{}` for `{owner}`.",
                test.name
            ),
        );
    }

    (
        format!("Static evidence for this {seam_kind} seam is `{grip_class}`."),
        format!(
            "Add one focused test that reaches `{owner}` and asserts the changed behavior directly."
        ),
    )
}

fn first_missing_discriminator(value: &Value) -> Option<(String, String)> {
    let discriminators = value.get("missing_discriminators")?.as_array()?;
    let first = discriminators.first()?;
    let object = first.as_object()?;
    let missing_value = object
        .get("value")
        .and_then(Value::as_str)
        .unwrap_or("missing discriminator")
        .to_string();
    let reason = object
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("RIPR found a missing discriminator for this seam.")
        .to_string();
    Some((missing_value, reason))
}

fn parse_related_test(value: &Value) -> Option<OperatorRelatedTest> {
    Some(OperatorRelatedTest {
        name: string_field(value, "name")?,
        file: string_field(value, "file").unwrap_or_else(|| "unknown".to_string()),
        line: usize_value(value.get("line")).unwrap_or(0),
        oracle_strength: string_field(value, "oracle_strength")
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

fn is_operator_attention_class(class: &str) -> bool {
    matches!(
        class,
        "weakly_gripped"
            | "ungripped"
            | "reachable_unrevealed"
            | "activation_unknown"
            | "propagation_unknown"
            | "observation_unknown"
            | "discrimination_unknown"
    )
}

fn weak_seam_rank(class: &str) -> usize {
    match class {
        "weakly_gripped" => 0,
        "ungripped" => 1,
        "reachable_unrevealed" => 2,
        "activation_unknown" => 3,
        "propagation_unknown" => 4,
        "observation_unknown" => 5,
        "discrimination_unknown" => 6,
        _ => 7,
    }
}

fn operator_cockpit_json(report: &OperatorCockpitReport) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "status": report.status,
        "inputs": report.inputs.iter().map(input_json).collect::<Vec<_>>(),
        "top_weak_seams": report.top_weak_seams.iter().map(weak_seam_json).collect::<Vec<_>>(),
        "surface_alignment": report.surface_alignment.iter().map(surface_alignment_json).collect::<Vec<_>>(),
        "next_commands": report.next_commands.iter().map(next_command_json).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render operator cockpit JSON: {err}"))
}

fn input_json(input: &OperatorInputSummary) -> Value {
    serde_json::json!({
        "name": input.name,
        "path": input.path,
        "state": input.state,
        "status": input.status,
        "command": input.command,
        "required": input.required,
        "summary": input.summary,
    })
}

fn weak_seam_json(seam: &OperatorWeakSeam) -> Value {
    serde_json::json!({
        "seam_id": seam.seam_id,
        "seam_kind": seam.seam_kind,
        "file": seam.file,
        "line": seam.line,
        "owner": seam.owner,
        "expression": seam.expression,
        "grip_class": seam.grip_class,
        "why_it_matters": seam.why_it_matters,
        "suggested_next_targeted_test": seam.suggested_next_targeted_test,
        "best_related_test": seam.best_related_test.as_ref().map(related_test_json),
    })
}

fn related_test_json(test: &OperatorRelatedTest) -> Value {
    serde_json::json!({
        "name": test.name,
        "file": test.file,
        "line": test.line,
        "oracle_strength": test.oracle_strength,
    })
}

fn surface_alignment_json(surface: &OperatorSurfaceAlignment) -> Value {
    serde_json::json!({
        "surface": surface.surface,
        "state": surface.state,
        "status": surface.status,
        "agreement": surface.agreement,
        "signal": surface.signal,
        "command": surface.command,
    })
}

fn next_command_json(command: &OperatorNextCommand) -> Value {
    serde_json::json!({
        "command": command.command,
        "reason": command.reason,
    })
}

fn operator_cockpit_markdown(report: &OperatorCockpitReport) -> String {
    let mut out = String::new();
    out.push_str("# ripr operator cockpit\n\n");
    out.push_str(&format!("Status: {}\n\n", report.status));
    push_top_weak_seams_markdown(&mut out, &report.top_weak_seams);
    push_surface_alignment_markdown(&mut out, &report.surface_alignment);
    push_inputs_markdown(&mut out, &report.inputs);
    push_next_commands_markdown(&mut out, &report.next_commands);
    out.push_str("\nThis cockpit joins existing reports. It does not rerun analysis, mutate tests, or change static classifications.\n");
    out
}

fn push_top_weak_seams_markdown(out: &mut String, seams: &[OperatorWeakSeam]) {
    out.push_str("## Top Weak Seams\n\n");
    if seams.is_empty() {
        out.push_str("No headline weak seams were available from `repo-exposure.json`.\n\n");
        return;
    }
    for seam in seams {
        out.push_str(&format!(
            "- `{}` `{}` {}:{} `{}`\n",
            md_escape(&seam.seam_id),
            md_escape(&seam.grip_class),
            md_escape(&seam.file),
            seam.line,
            md_escape(&seam.seam_kind)
        ));
        out.push_str(&format!("  - why: {}\n", md_escape(&seam.why_it_matters)));
        out.push_str(&format!(
            "  - next targeted test: {}\n",
            md_escape(&seam.suggested_next_targeted_test)
        ));
        if let Some(test) = &seam.best_related_test {
            out.push_str(&format!(
                "  - best related test: `{}` {}:{} ({})\n",
                md_escape(&test.name),
                md_escape(&test.file),
                test.line,
                md_escape(&test.oracle_strength)
            ));
        }
    }
    out.push('\n');
}

fn push_surface_alignment_markdown(out: &mut String, surfaces: &[OperatorSurfaceAlignment]) {
    out.push_str("## Surface Alignment\n\n");
    out.push_str("| Surface | State | Status | Agreement | Signal |\n");
    out.push_str("| --- | --- | --- | --- | --- |\n");
    for surface in surfaces {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            md_escape(&surface.surface),
            md_escape(&surface.state),
            md_escape(&surface.status),
            md_escape(&surface.agreement),
            md_escape(&surface.signal)
        ));
    }
    out.push('\n');
}

fn push_inputs_markdown(out: &mut String, inputs: &[OperatorInputSummary]) {
    out.push_str("## Inputs\n\n");
    out.push_str("| Report | Required | State | Path |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for input in inputs {
        out.push_str(&format!(
            "| {} | {} | {} | `{}` |\n",
            md_escape(&input.name),
            input.required,
            md_escape(&input.state),
            md_escape(&input.path)
        ));
    }
    out.push('\n');
}

fn push_next_commands_markdown(out: &mut String, commands: &[OperatorNextCommand]) {
    out.push_str("## Next Commands\n\n");
    if commands.is_empty() {
        out.push_str("None.\n");
        return;
    }
    for command in commands {
        out.push_str(&format!(
            "- `{}`\n  - {}\n",
            md_escape(&command.command),
            md_escape(&command.reason)
        ));
    }
}

fn reports_dir() -> PathBuf {
    Path::new("target").join("ripr").join("reports")
}

fn workspace_artifact_path(report_dir: &Path, relative_path: &str) -> PathBuf {
    workspace_root_for_report_dir(report_dir).join(relative_path)
}

fn workspace_root_for_report_dir(report_dir: &Path) -> PathBuf {
    let reports = report_dir.file_name().and_then(|name| name.to_str());
    let ripr = report_dir
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str());
    let target = report_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|name| name.to_str());
    if reports == Some("reports")
        && ripr == Some("ripr")
        && target == Some("target")
        && let Some(root) = report_dir
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
    {
        return root.to_path_buf();
    }
    report_dir.to_path_buf()
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn md_escape(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn usize_field(object: &serde_json::Map<String, Value>, key: &str) -> Option<usize> {
    usize_value(object.get(key))
}

fn usize_value(value: Option<&Value>) -> Option<usize> {
    value
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn operator_cockpit_reports_missing_inputs_with_commands() -> Result<(), String> {
        let report =
            build_operator_cockpit_report_at(Path::new("target/ripr/missing-operator-test"));
        assert_eq!(report.status, "warn");
        assert!(report.inputs.iter().any(|input| {
            input.name == "repo exposure"
                && input.state == "missing"
                && input.command == "cargo xtask repo-exposure-report"
        }));
        assert!(report.next_commands.iter().any(|command| {
            command.command == "cargo xtask repo-exposure-report"
                && command.reason.contains("repo exposure")
        }));
        assert!(report.next_commands.iter().any(|command| {
            command.command == BEFORE_SNAPSHOT_COMMAND && command.reason.contains("before snapshot")
        }));
        assert!(report.next_commands.iter().any(|command| {
            command.command == after_snapshot_command() && command.reason.contains("after snapshot")
        }));
        assert!(report.next_commands.iter().any(|command| {
            command.command == agent_verify_command() && command.reason.contains("agent verify")
        }));
        assert!(report.next_commands.iter().any(|command| {
            command.command == receipt_command_for(None) && command.reason.contains("agent receipt")
        }));

        let json = operator_cockpit_json(&report)?;
        let value: Value = serde_json::from_str(&json)
            .map_err(|err| format!("operator cockpit JSON should parse: {err}"))?;
        assert_eq!(value.get("status").and_then(Value::as_str), Some("warn"));
        assert!(operator_cockpit_markdown(&report).contains("cargo xtask repo-exposure-report"));
        Ok(())
    }

    #[test]
    fn receipt_command_placeholder_does_not_rewrite_real_seam_ids() {
        let command = receipt_command_for(Some("RIPR_SEAM_ID_PLACEHOLDER"));

        assert!(command.contains("--seam-id RIPR_SEAM_ID_PLACEHOLDER"));
        assert!(!command.contains("<seam-id>"));
    }

    #[test]
    fn operator_cockpit_json_and_markdown_are_structured() -> Result<(), String> {
        let root = temp_report_dir("structured")?;
        let dir = root.join("target/ripr/reports");
        fs::create_dir_all(&dir)
            .map_err(|err| format!("failed to create {}: {err}", dir.display()))?;
        let before_snapshot =
            workspace_artifact_path(&dir, loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT);
        let after_snapshot =
            workspace_artifact_path(&dir, loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT);
        let agent_verify =
            workspace_artifact_path(&dir, loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT);
        let agent_receipt =
            workspace_artifact_path(&dir, loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT);
        write_json(
            &dir,
            "repo-exposure.json",
            &serde_json::json!({
                "schema_version": "0.2",
                "scope": "repo",
                "metrics": {
                    "seams_total": 2,
                    "weakly_gripped": 1,
                    "ungripped": 0,
                    "reachable_unrevealed": 0
                },
                "seams": [
                    {
                        "seam_id": "67fc764ba37d77bd",
                        "kind": "predicate_boundary",
                        "file": "src/lib.rs",
                        "line": 42,
                        "owner": "src/lib.rs::discounted_total",
                        "expression": "amount >= discount_threshold",
                        "grip_class": "weakly_gripped",
                        "headline_eligible": true,
                        "evidence": {
                            "reach": "yes",
                            "activate": "yes",
                            "propagate": "yes",
                            "observe": "yes",
                            "discriminate": "weak"
                        },
                        "related_tests": [
                            {
                                "name": "below_threshold_has_no_discount",
                                "file": "tests/pricing.rs",
                                "line": 12,
                                "oracle_strength": "strong"
                            }
                        ],
                        "missing_discriminators": [
                            {
                                "value": "discount_threshold (equality boundary)",
                                "reason": "observed values do not include the equality-boundary case for this predicate"
                            }
                        ]
                    }
                ]
            }),
        )?;
        write_json_path(
            &before_snapshot,
            &serde_json::json!({
                "schema_version": "0.2",
                "scope": "repo",
                "metrics": {
                    "seams_total": 1,
                    "weakly_gripped": 1,
                    "ungripped": 0,
                    "reachable_unrevealed": 0
                },
                "seams": []
            }),
        )?;
        write_json_path(
            &after_snapshot,
            &serde_json::json!({
                "schema_version": "0.2",
                "scope": "repo",
                "metrics": {
                    "seams_total": 1,
                    "weakly_gripped": 0,
                    "ungripped": 0,
                    "reachable_unrevealed": 0
                },
                "seams": []
            }),
        )?;
        write_json_path(
            &agent_verify,
            &serde_json::json!({
                "schema_version": "0.1",
                "tool": "ripr",
                "status": "advisory",
                "inputs": {
                    "before": loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
                    "after": loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT
                },
                "summary": {
                    "improved": 1,
                    "changed": 0,
                    "regressed": 0,
                    "unchanged": 1,
                    "new": 0,
                    "resolved": 0
                },
                "changed_seams": [],
                "unchanged_seams": [],
                "new_gaps": [],
                "resolved_gaps": []
            }),
        )?;
        write_json_path(
            &agent_receipt,
            &serde_json::json!({
                "schema_version": "0.1",
                "tool": "ripr",
                "status": "advisory",
                "inputs": {
                    "agent_verify_json": loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT,
                    "before": loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
                    "after": loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT
                },
                "seam": {
                    "seam_id": "67fc764ba37d77bd",
                    "seam_kind": "predicate_boundary",
                    "file": "src/lib.rs",
                    "line": 42,
                    "before": "weakly_gripped",
                    "after": "strongly_gripped",
                    "grip_class": null,
                    "change": "improved",
                    "evidence_delta": []
                },
                "summary": {
                    "remaining_gap": "No remaining static gap is named by this receipt.",
                    "next_recommendation": "Keep the focused test and attach this receipt with the agent verify JSON."
                }
            }),
        )?;
        write_json(
            &dir,
            "lsp-cockpit.json",
            &serde_json::json!({
                "schema_version": "0.1",
                "status": "pass",
                "fixtures": [{"fixture": "boundary_gap"}],
                "vscode_e2e": {"uncovered_contributed_commands": []}
            }),
        )?;
        write_json(
            &dir,
            "sarif-policy.json",
            &serde_json::json!({
                "schema_version": "0.1",
                "status": "pass",
                "mode": "advisory",
                "new_results_total": 0
            }),
        )?;
        write_json(
            &dir,
            "repo-ripr-badge.json",
            &serde_json::json!({
                "schema_version": "0.3",
                "status": "warn",
                "message": "1",
                "counts": {
                    "unsuppressed_exposure_gaps": 1,
                    "unknowns": 0,
                    "analyzed_seams": 2
                }
            }),
        )?;
        write_json(
            &dir,
            "targeted-test-outcome.json",
            &serde_json::json!({
                "schema_version": "0.1",
                "status": "advisory",
                "summary": {
                    "moved": 1,
                    "regressed": 0,
                    "unchanged": 1,
                    "new": 0,
                    "removed": 0
                }
            }),
        )?;

        let report = build_operator_cockpit_report_at(&dir);
        assert_eq!(report.status, "warn");
        assert_eq!(report.top_weak_seams.len(), 1);
        assert_eq!(report.top_weak_seams[0].seam_id, "67fc764ba37d77bd");
        assert_eq!(
            report.top_weak_seams[0]
                .best_related_test
                .as_ref()
                .map(|test| test.name.as_str()),
            Some("below_threshold_has_no_discount")
        );
        assert!(report.inputs.iter().any(|input| {
            input.name == "agent verify"
                && input.state == "present"
                && input.summary == "1 improved, 0 changed, 0 regressed, 1 unchanged seams."
        }));
        assert!(report.inputs.iter().any(|input| {
            input.name == "agent receipt"
                && input.state == "present"
                && input.summary.contains("Receipt for seam 67fc764ba37d77bd")
        }));

        let json = operator_cockpit_json(&report)?;
        let value: Value = serde_json::from_str(&json)
            .map_err(|err| format!("operator cockpit JSON should parse: {err}"))?;
        let seams = value
            .get("top_weak_seams")
            .and_then(Value::as_array)
            .ok_or_else(|| "operator cockpit JSON should include top_weak_seams".to_string())?;
        let first = seams
            .first()
            .ok_or_else(|| "operator cockpit should include one weak seam".to_string())?;
        assert_eq!(
            first
                .get("suggested_next_targeted_test")
                .and_then(Value::as_str),
            Some(
                "Add a focused predicate_boundary test for `src/lib.rs::discounted_total` that exercises `discount_threshold (equality boundary)` and asserts the observable result."
            )
        );
        let markdown = operator_cockpit_markdown(&report);
        assert!(markdown.contains("## Surface Alignment"));
        assert!(markdown.contains("discount_threshold (equality boundary)"));
        assert!(markdown.contains("1 improved, 0 changed, 0 regressed, 1 unchanged seams."));
        assert!(markdown.contains("agent_verify_counts_available"));
        assert!(markdown.contains("agent_receipt_available"));
        Ok(())
    }

    #[test]
    fn operator_cockpit_matches_editor_agent_loop_fixture() -> Result<(), String> {
        let root = temp_report_dir("editor-agent-loop")?;
        let dir = root.join("target/ripr/reports");
        fs::create_dir_all(&dir)
            .map_err(|err| format!("failed to create {}: {err}", dir.display()))?;
        let repo = repo_root();
        let fixture = repo.join("fixtures/boundary_gap/expected/editor-agent-loop");
        let calibration = repo.join("fixtures/boundary_gap/calibration");

        copy_file(
            &calibration.join("before-targeted-test.repo-exposure.json"),
            &dir.join("repo-exposure.json"),
        )?;
        copy_file(
            &calibration.join("before-targeted-test.repo-exposure.json"),
            &workspace_artifact_path(&dir, loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT),
        )?;
        copy_file(
            &calibration.join("after-targeted-test.repo-exposure.json"),
            &workspace_artifact_path(&dir, loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT),
        )?;
        copy_file(
            &calibration.join("targeted-test-outcome.json"),
            &dir.join("targeted-test-outcome.json"),
        )?;
        copy_file(
            &fixture.join("agent-verify.json"),
            &workspace_artifact_path(&dir, loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT),
        )?;
        copy_file(
            &fixture.join("agent-receipt.json"),
            &workspace_artifact_path(&dir, loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT),
        )?;
        write_json(
            &dir,
            "lsp-cockpit.json",
            &serde_json::json!({
                "schema_version": "0.1",
                "status": "pass",
                "fixtures": [{"fixture": "boundary_gap"}],
                "vscode_e2e": {"uncovered_contributed_commands": []}
            }),
        )?;

        let report = build_operator_cockpit_report_at(&dir);
        let json = normalize_fixture_output(&operator_cockpit_json(&report)?, &root);
        let markdown = normalize_fixture_output(&operator_cockpit_markdown(&report), &root);
        let expected_json = fs::read_to_string(fixture.join("operator-cockpit.json"))
            .map_err(|err| format!("failed to read operator cockpit fixture JSON: {err}"))?;
        let expected_markdown = fs::read_to_string(fixture.join("operator-cockpit.md"))
            .map_err(|err| format!("failed to read operator cockpit fixture Markdown: {err}"))?;

        assert_eq!(json, normalize_newlines(&expected_json));
        assert_eq!(markdown, normalize_newlines(&expected_markdown));
        Ok(())
    }

    fn temp_report_dir(label: &str) -> Result<PathBuf, String> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before unix epoch: {err}"))?;
        let dir = std::env::temp_dir().join(format!(
            "ripr-operator-cockpit-{label}-{}-{}",
            std::process::id(),
            duration.as_nanos()
        ));
        fs::create_dir_all(&dir)
            .map_err(|err| format!("failed to create {}: {err}", dir.display()))?;
        Ok(dir)
    }

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn write_json(dir: &Path, file: &str, value: &Value) -> Result<(), String> {
        write_json_path(&dir.join(file), value)
    }

    fn write_json_path(path: &Path, value: &Value) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }
        let rendered = serde_json::to_string_pretty(value)
            .map_err(|err| format!("failed to render test JSON: {err}"))?;
        fs::write(path, rendered)
            .map_err(|err| format!("failed to write test report {}: {err}", path.display()))
    }

    fn copy_file(source: &Path, destination: &Path) -> Result<(), String> {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }
        fs::copy(source, destination).map_err(|err| {
            format!(
                "failed to copy {} to {}: {err}",
                source.display(),
                destination.display()
            )
        })?;
        Ok(())
    }

    fn normalize_fixture_output(text: &str, root: &Path) -> String {
        let root_prefix = format!("{}/", normalize_path(root));
        normalize_newlines(text).replace(&root_prefix, "")
    }

    fn normalize_newlines(text: &str) -> String {
        text.replace("\r\n", "\n")
    }
}
