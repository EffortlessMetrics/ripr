use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "report_packet_index";

pub(crate) const DEFAULT_REPORT_PACKET_INDEX_OUT: &str = "target/ripr/reports/index.json";
pub(crate) const DEFAULT_REPORT_PACKET_INDEX_MD_OUT: &str = "target/ripr/reports/index.md";

const LIMITS: &[&str] = &[
    "Advisory report-packet index only.",
    "Does not rerun analysis.",
    "Does not edit source or generate tests.",
    "Does not call providers.",
    "Does not run mutation testing.",
    "Does not publish inline comments.",
    "Does not change default CI blocking.",
    "Gate decision remains pass/fail authority when configured.",
];

const MARKDOWN_LIMITS: &[&str] = &[
    "Advisory report-packet index only.",
    "Does not rerun analysis.",
    "Does not run mutation testing.",
    "Does not edit source or generate tests.",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReportPacketIndexInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) reports_dir: PathBuf,
    pub(crate) review_dir: PathBuf,
    pub(crate) receipts_dir: PathBuf,
    pub(crate) workflow_dir: PathBuf,
    pub(crate) agent_dir: PathBuf,
    pub(crate) pilot_dir: PathBuf,
    pub(crate) ci_dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ReportPacketIndexReport {
    status: String,
    root: String,
    generated_at: String,
    inputs: IndexInputs,
    summary: IndexSummary,
    groups: Vec<IndexGroup>,
    missing_expected: Vec<MissingExpected>,
    warnings: Vec<IndexWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct IndexInputs {
    reports_dir: String,
    review_dir: String,
    receipts_dir: String,
    workflow_dir: String,
    agent_dir: String,
    pilot_dir: String,
    ci_dir: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct IndexSummary {
    entries: usize,
    available: usize,
    missing_expected: usize,
    warnings: usize,
    failures: usize,
    start_here: Option<String>,
    gate_authority: Option<String>,
    advisory: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct IndexGroup {
    group: String,
    label: String,
    summary: String,
    entries: Vec<IndexEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct IndexEntry {
    id: String,
    label: String,
    kind: String,
    path: String,
    json_path: Option<String>,
    status: String,
    available: bool,
    required: bool,
    authority: bool,
    description: String,
    next_command: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct MissingExpected {
    id: String,
    label: String,
    group: String,
    path: String,
    required: bool,
    reason: String,
    next_command: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct IndexWarning {
    kind: String,
    message: String,
    source_artifact: Option<String>,
}

#[derive(Clone, Debug)]
struct ArtifactSpec {
    id: &'static str,
    label: &'static str,
    group: &'static str,
    kind: &'static str,
    path: PathBuf,
    json_path: Option<PathBuf>,
    required: bool,
    authority: bool,
    description: &'static str,
    default_status: &'static str,
    next_command: Option<&'static str>,
}

pub(crate) fn build_report_packet_index_report(
    input: ReportPacketIndexInput,
) -> ReportPacketIndexReport {
    let specs = artifact_specs(&input);
    let mut entries = Vec::new();
    for spec in &specs {
        if artifact_available(spec) {
            entries.push(entry_from_spec(spec, true, status_for_spec(spec)));
        }
    }

    let missing_expected = missing_expected_surfaces(&specs);
    for missing in &missing_expected {
        if let Some(spec) = specs.iter().find(|spec| spec.id == missing.id) {
            entries.push(entry_from_spec(spec, false, "missing".to_string()));
        }
    }

    let warnings = missing_expected
        .iter()
        .map(|missing| IndexWarning {
            kind: "missing_expected".to_string(),
            message: format!("{} is missing.", missing.label),
            source_artifact: None,
        })
        .collect::<Vec<_>>();

    let failures = entries
        .iter()
        .filter(|entry| matches!(entry.status.as_str(), "fail" | "blocked"))
        .count();
    let status = if entries.is_empty() {
        "incomplete"
    } else if failures > 0 {
        "fail"
    } else if !missing_expected.is_empty()
        || entries
            .iter()
            .any(|entry| matches!(entry.status.as_str(), "warn" | "incomplete" | "unreadable"))
    {
        "warn"
    } else {
        "pass"
    }
    .to_string();

    let start_here = entries
        .iter()
        .find(|entry| entry.id == "pr_review_front_panel" && entry.available)
        .map(|entry| entry.path.clone());
    let gate_authority = entries
        .iter()
        .find(|entry| entry.id == "gate_decision" && entry.available)
        .map(|entry| entry.path.clone());
    let groups = group_entries(entries);
    let entry_count = groups.iter().map(|group| group.entries.len()).sum();
    let available = groups
        .iter()
        .flat_map(|group| group.entries.iter())
        .filter(|entry| entry.available)
        .count();
    let summary = IndexSummary {
        entries: entry_count,
        available,
        missing_expected: missing_expected.len(),
        warnings: warnings.len(),
        failures,
        start_here,
        gate_authority,
        advisory: true,
    };

    ReportPacketIndexReport {
        status,
        root: input.root,
        generated_at: input.generated_at,
        inputs: IndexInputs {
            reports_dir: display_path(&input.reports_dir),
            review_dir: display_path(&input.review_dir),
            receipts_dir: display_path(&input.receipts_dir),
            workflow_dir: display_path(&input.workflow_dir),
            agent_dir: display_path(&input.agent_dir),
            pilot_dir: display_path(&input.pilot_dir),
            ci_dir: display_path(&input.ci_dir),
        },
        summary,
        groups,
        missing_expected,
        warnings,
    }
}

pub(crate) fn render_report_packet_index_json(
    report: &ReportPacketIndexReport,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct JsonReport<'a> {
        schema_version: &'static str,
        tool: &'static str,
        kind: &'static str,
        status: &'a str,
        root: &'a str,
        generated_at: &'a str,
        inputs: &'a IndexInputs,
        summary: &'a IndexSummary,
        groups: &'a [IndexGroup],
        missing_expected: &'a [MissingExpected],
        warnings: &'a [IndexWarning],
        limits: Vec<&'static str>,
    }

    serde_json::to_string_pretty(&JsonReport {
        schema_version: SCHEMA_VERSION,
        tool: "ripr",
        kind: REPORT_KIND,
        status: &report.status,
        root: &report.root,
        generated_at: &report.generated_at,
        inputs: &report.inputs,
        summary: &report.summary,
        groups: &report.groups,
        missing_expected: &report.missing_expected,
        warnings: &report.warnings,
        limits: LIMITS.to_vec(),
    })
    .map_err(|err| format!("render report-packet index JSON failed: {err}"))
}

pub(crate) fn render_report_packet_index_markdown(report: &ReportPacketIndexReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Report Packet Index\n\n");
    out.push_str(&format!("Status: {}\n\n", report.status));

    if let Some(start_here) = &report.summary.start_here {
        out.push_str("Start here:\n");
        out.push_str(&format!("- PR review front panel: {start_here}\n"));
        if let Some(gate_authority) = &report.summary.gate_authority {
            out.push_str(&format!("- Gate authority: {gate_authority}\n"));
        }
        out.push('\n');
    } else {
        out.push_str("Next: generate the PR review front panel before using the packet index as the first-screen PR story.\n\n");
    }

    out.push_str("Packet summary:\n");
    out.push_str(&format!(
        "- Available artifacts: {}\n",
        report.summary.available
    ));
    out.push_str(&format!(
        "- Missing expected artifacts: {}\n",
        report.summary.missing_expected
    ));
    out.push_str(&format!("- Warnings: {}\n", report.summary.warnings));
    out.push_str(&format!("- Failures: {}\n\n", report.summary.failures));

    for group in &report.groups {
        out.push_str(&format!("{}:\n", group.label));
        for entry in &group.entries {
            if entry.available {
                out.push_str(&format!("- {}: {}\n", entry.label, entry.path));
            } else {
                out.push_str(&format!("- {}: missing\n", entry.label));
                if let Some(next_command) = &entry.next_command {
                    out.push_str(&format!("  - next: `{next_command}`\n"));
                }
            }
            if entry.authority {
                out.push_str(
                    "- authority: gate decision controls configured pass/fail, not this index\n",
                );
            }
        }
        out.push('\n');
    }

    if !report.missing_expected.is_empty() {
        out.push_str("Missing expected:\n");
        for missing in &report.missing_expected {
            out.push_str(&format!("- {}: {}\n", missing.label, missing.reason));
            if let Some(next_command) = &missing.next_command {
                out.push_str(&format!("  - next: `{next_command}`\n"));
            }
        }
        out.push('\n');
    }

    out.push_str("Limits:\n");
    for limit in MARKDOWN_LIMITS {
        out.push_str(&format!("- {limit}\n"));
    }
    out
}

fn artifact_specs(input: &ReportPacketIndexInput) -> Vec<ArtifactSpec> {
    let reports = &input.reports_dir;
    let review = &input.review_dir;
    vec![
        ArtifactSpec {
            id: "pr_review_front_panel",
            label: "PR review front panel",
            group: "start_here",
            kind: "markdown",
            path: reports.join("pr-review-front-panel.md"),
            json_path: Some(reports.join("pr-review-front-panel.json")),
            required: true,
            authority: false,
            description: "First-screen PR review story.",
            default_status: "available",
            next_command: Some(
                "ripr pr-review front-panel --out target/ripr/reports/pr-review-front-panel.json --out-md target/ripr/reports/pr-review-front-panel.md",
            ),
        },
        ArtifactSpec {
            id: "first_useful_action",
            label: "First useful action",
            group: "pr_review_story",
            kind: "markdown",
            path: reports.join("first-useful-action.md"),
            json_path: Some(reports.join("first-useful-action.json")),
            required: false,
            authority: false,
            description: "One advisory next action.",
            default_status: "available",
            next_command: None,
        },
        ArtifactSpec {
            id: "review_comments",
            label: "Review guidance",
            group: "pr_review_story",
            kind: "markdown",
            path: review.join("comments.md"),
            json_path: Some(review.join("comments.json")),
            required: false,
            authority: false,
            description: "Bounded PR guidance comments.",
            default_status: "available",
            next_command: None,
        },
        ArtifactSpec {
            id: "assistant_proof",
            label: "Assistant proof",
            group: "repair_agent_handoff",
            kind: "markdown",
            path: reports.join("test-oracle-assistant-proof.md"),
            json_path: Some(reports.join("test-oracle-assistant-proof.json")),
            required: false,
            authority: false,
            description: "Joined repair proof packet.",
            default_status: "available",
            next_command: Some(
                "ripr assistant-loop proof --out target/ripr/reports/test-oracle-assistant-proof.json --out-md target/ripr/reports/test-oracle-assistant-proof.md",
            ),
        },
        ArtifactSpec {
            id: "assistant_loop_health",
            label: "Assistant loop health",
            group: "repair_agent_handoff",
            kind: "markdown",
            path: reports.join("assistant-loop-health.md"),
            json_path: Some(reports.join("assistant-loop-health.json")),
            required: false,
            authority: false,
            description: "Repair loop health over assistant proof inputs.",
            default_status: "available",
            next_command: Some(
                "ripr assistant-loop health --proof target/ripr/reports/test-oracle-assistant-proof.json --out target/ripr/reports/assistant-loop-health.json --out-md target/ripr/reports/assistant-loop-health.md",
            ),
        },
        ArtifactSpec {
            id: "pr_evidence_ledger",
            label: "PR evidence ledger",
            group: "evidence_movement",
            kind: "markdown",
            path: reports.join("pr-evidence-ledger.md"),
            json_path: Some(reports.join("pr-evidence-ledger.json")),
            required: false,
            authority: false,
            description: "PR-local movement and policy state ledger.",
            default_status: "available",
            next_command: None,
        },
        ArtifactSpec {
            id: "baseline_debt_delta",
            label: "Baseline debt delta",
            group: "evidence_movement",
            kind: "markdown",
            path: reports.join("baseline-debt-delta.md"),
            json_path: Some(reports.join("baseline-debt-delta.json")),
            required: false,
            authority: false,
            description: "Baseline still-present, resolved, and new debt.",
            default_status: "available",
            next_command: None,
        },
        ArtifactSpec {
            id: "ripr_zero_status",
            label: "RIPR Zero status",
            group: "evidence_movement",
            kind: "markdown",
            path: reports.join("ripr-zero-status.md"),
            json_path: Some(reports.join("ripr-zero-status.json")),
            required: false,
            authority: false,
            description: "Movement toward RIPR Zero.",
            default_status: "available",
            next_command: None,
        },
        ArtifactSpec {
            id: "gate_decision",
            label: "Gate decision",
            group: "policy_gates",
            kind: "markdown",
            path: reports.join("gate-decision.md"),
            json_path: Some(reports.join("gate-decision.json")),
            required: false,
            authority: true,
            description: "Configured gate pass/fail authority.",
            default_status: "pass",
            next_command: None,
        },
        ArtifactSpec {
            id: "recommendation_calibration",
            label: "Recommendation calibration",
            group: "calibration",
            kind: "markdown",
            path: reports.join("recommendation-calibration.md"),
            json_path: Some(reports.join("recommendation-calibration.json")),
            required: false,
            authority: false,
            description: "Recommendation quality evidence.",
            default_status: "available",
            next_command: None,
        },
        ArtifactSpec {
            id: "mutation_calibration",
            label: "Mutation calibration",
            group: "calibration",
            kind: "markdown",
            path: reports.join("mutation-calibration.md"),
            json_path: Some(reports.join("mutation-calibration.json")),
            required: false,
            authority: false,
            description: "Imported runtime calibration context.",
            default_status: "available",
            next_command: None,
        },
        ArtifactSpec {
            id: "coverage_grip_frontier",
            label: "Coverage/grip frontier",
            group: "calibration",
            kind: "markdown",
            path: reports.join("coverage-grip-frontier.md"),
            json_path: Some(reports.join("coverage-grip-frontier.json")),
            required: false,
            authority: false,
            description: "Coverage delta and static grip movement context; not runtime confirmation.",
            default_status: "available",
            next_command: None,
        },
        ArtifactSpec {
            id: "agent_receipt",
            label: "Agent receipt",
            group: "validation_receipts",
            kind: "json",
            path: reports.join("agent-receipt.json"),
            json_path: Some(reports.join("agent-receipt.json")),
            required: false,
            authority: false,
            description: "Focused repair receipt.",
            default_status: "available",
            next_command: Some("ripr agent receipt --out target/ripr/reports/agent-receipt.json"),
        },
        ArtifactSpec {
            id: "pr_summary",
            label: "PR summary",
            group: "validation_receipts",
            kind: "markdown",
            path: reports.join("pr-summary.md"),
            json_path: None,
            required: false,
            authority: false,
            description: "Local reviewer packet.",
            default_status: "available",
            next_command: Some("cargo xtask pr-summary"),
        },
        ArtifactSpec {
            id: "check_pr",
            label: "Check PR",
            group: "validation_receipts",
            kind: "markdown",
            path: reports.join("check-pr.md"),
            json_path: None,
            required: false,
            authority: false,
            description: "Local review-ready gate receipt.",
            default_status: "pass",
            next_command: Some("cargo xtask check-pr"),
        },
        ArtifactSpec {
            id: "sarif",
            label: "SARIF output",
            group: "sarif_badges",
            kind: "json",
            path: reports.join("ripr.sarif.json"),
            json_path: Some(reports.join("ripr.sarif.json")),
            required: false,
            authority: false,
            description: "Code scanning projection.",
            default_status: "available",
            next_command: None,
        },
        ArtifactSpec {
            id: "badge",
            label: "Badge output",
            group: "sarif_badges",
            kind: "json",
            path: reports.join("ripr-badge.json"),
            json_path: Some(reports.join("ripr-badge.json")),
            required: false,
            authority: false,
            description: "Badge JSON output.",
            default_status: "available",
            next_command: None,
        },
    ]
}

fn artifact_available(spec: &ArtifactSpec) -> bool {
    spec.path.exists() || spec.json_path.as_ref().is_some_and(|path| path.exists())
}

fn status_for_spec(spec: &ArtifactSpec) -> String {
    if spec.id == "gate_decision" {
        return read_json_status(spec)
            .map(|status| gate_status(&status))
            .unwrap_or_else(|| spec.default_status.to_string());
    }
    if spec.id == "pr_review_front_panel" {
        return read_json_status(spec)
            .map(|status| front_panel_status(&status))
            .unwrap_or_else(|| spec.default_status.to_string());
    }
    if spec.id == "check_pr" {
        return read_markdown_status(&spec.path).unwrap_or_else(|| spec.default_status.to_string());
    }
    spec.default_status.to_string()
}

fn gate_status(status: &str) -> String {
    match status {
        "blocked" => "blocked".to_string(),
        "config_error" | "fail" | "failure" => "fail".to_string(),
        "acknowledged" => "acknowledged".to_string(),
        "suppressed" => "suppressed".to_string(),
        "warn" => "warn".to_string(),
        "incomplete" => "incomplete".to_string(),
        _ => "pass".to_string(),
    }
}

fn front_panel_status(status: &str) -> String {
    match status {
        "blocked" => "blocked".to_string(),
        "fail" | "config_error" => "fail".to_string(),
        "warn" => "warn".to_string(),
        "incomplete" => "incomplete".to_string(),
        _ => "available".to_string(),
    }
}

fn read_json_status(spec: &ArtifactSpec) -> Option<String> {
    let path = spec.json_path.as_ref()?;
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    value
        .get("decision")
        .or_else(|| value.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn read_markdown_status(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let Some(status) = line.strip_prefix("Status:") else {
            continue;
        };
        return Some(gate_status(status.trim()));
    }
    None
}

fn entry_from_spec(spec: &ArtifactSpec, available: bool, status: String) -> IndexEntry {
    IndexEntry {
        id: spec.id.to_string(),
        label: spec.label.to_string(),
        kind: spec.kind.to_string(),
        path: display_path(&spec.path),
        json_path: spec.json_path.as_ref().map(|path| display_path(path)),
        status,
        available,
        required: spec.required,
        authority: spec.authority,
        description: spec.description.to_string(),
        next_command: if available {
            None
        } else {
            spec.next_command.map(str::to_string)
        },
    }
}

fn missing_expected_surfaces(specs: &[ArtifactSpec]) -> Vec<MissingExpected> {
    let has_any_review_story = is_available(specs, "first_useful_action")
        || is_available(specs, "review_comments")
        || is_available(specs, "assistant_proof")
        || is_available(specs, "gate_decision");
    let mut missing = Vec::new();

    if has_any_review_story && !is_available(specs, "pr_review_front_panel") {
        push_missing(
            &mut missing,
            specs,
            "pr_review_front_panel",
            "not_generated",
        );
    }
    if (is_available(specs, "first_useful_action") || is_available(specs, "review_comments"))
        && !is_available(specs, "assistant_proof")
    {
        push_missing(
            &mut missing,
            specs,
            "assistant_proof",
            "missing_required_input",
        );
    }
    if is_available(specs, "assistant_proof")
        && is_available(specs, "assistant_loop_health")
        && !is_available(specs, "agent_receipt")
        && !is_available(specs, "check_pr")
    {
        push_missing(&mut missing, specs, "agent_receipt", "not_generated");
        push_missing(&mut missing, specs, "check_pr", "not_generated");
    }
    missing
}

fn push_missing(
    missing: &mut Vec<MissingExpected>,
    specs: &[ArtifactSpec],
    id: &str,
    reason: &str,
) {
    if let Some(spec) = specs.iter().find(|spec| spec.id == id) {
        missing.push(MissingExpected {
            id: spec.id.to_string(),
            label: spec.label.to_string(),
            group: spec.group.to_string(),
            path: display_path(&spec.path),
            required: spec.required,
            reason: reason.to_string(),
            next_command: spec.next_command.map(str::to_string),
        });
    }
}

fn is_available(specs: &[ArtifactSpec], id: &str) -> bool {
    specs
        .iter()
        .find(|spec| spec.id == id)
        .is_some_and(artifact_available)
}

fn group_entries(entries: Vec<IndexEntry>) -> Vec<IndexGroup> {
    let order = [
        "start_here",
        "pr_review_story",
        "repair_agent_handoff",
        "evidence_movement",
        "policy_gates",
        "calibration",
        "validation_receipts",
        "sarif_badges",
        "local_context",
    ];
    let mut groups = Vec::new();
    for group_name in order {
        let group_entries = entries
            .iter()
            .filter(|entry| group_for_entry(entry.id.as_str()) == group_name)
            .cloned()
            .collect::<Vec<_>>();
        if group_entries.is_empty() {
            continue;
        }
        groups.push(IndexGroup {
            group: group_name.to_string(),
            label: group_label(group_name).to_string(),
            summary: group_summary(group_name).to_string(),
            entries: group_entries,
        });
    }
    groups
}

fn group_for_entry(id: &str) -> &'static str {
    match id {
        "pr_review_front_panel" => "start_here",
        "first_useful_action" | "review_comments" => "pr_review_story",
        "assistant_proof" | "assistant_loop_health" => "repair_agent_handoff",
        "pr_evidence_ledger" | "baseline_debt_delta" | "ripr_zero_status" => "evidence_movement",
        "gate_decision" => "policy_gates",
        "recommendation_calibration" | "mutation_calibration" | "coverage_grip_frontier" => {
            "calibration"
        }
        "agent_receipt" | "pr_summary" | "check_pr" => "validation_receipts",
        "sarif" | "badge" => "sarif_badges",
        _ => "local_context",
    }
}

fn group_label(group: &str) -> &'static str {
    match group {
        "start_here" => "Start here",
        "pr_review_story" => "PR review story",
        "repair_agent_handoff" => "Repair and agent handoff",
        "evidence_movement" => "Evidence movement",
        "policy_gates" => "Policy and gates",
        "calibration" => "Calibration",
        "validation_receipts" => "Validation receipts",
        "sarif_badges" => "SARIF and badges",
        _ => "Local context",
    }
}

fn group_summary(group: &str) -> &'static str {
    match group {
        "start_here" => "Reviewer-first PR story.",
        "pr_review_story" => "Guidance and first useful action.",
        "repair_agent_handoff" => "Proof, health, and repair routing.",
        "evidence_movement" => "Debt delta and PR movement.",
        "policy_gates" => "Configured pass/fail authority.",
        "calibration" => "Recommendation and coverage/grip context.",
        "validation_receipts" => "Receipts and local readiness reports.",
        "sarif_badges" => "Code scanning and badge surfaces.",
        _ => "Repo-local context.",
    }
}

pub(crate) fn display_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    for marker in ["target/ripr/", "target/ci/"] {
        if let Some(index) = normalized.find(marker) {
            return normalized[index..].to_string();
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::path::{Path, PathBuf};

    #[test]
    fn report_packet_index_reports_missing_front_panel() -> Result<(), String> {
        let root = temp_root("missing-front-panel")?;
        write(
            &root.join("target/ripr/reports/first-useful-action.md"),
            "Status: pass\n",
        )?;
        write(&root.join("target/ripr/review/comments.md"), "comments\n")?;
        write(
            &root.join("target/ripr/reports/test-oracle-assistant-proof.md"),
            "Status: pass\n",
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));

        assert_eq!(report.status, "warn");
        assert_eq!(report.summary.start_here, None);
        assert_eq!(report.summary.missing_expected, 1);
        assert!(
            render_report_packet_index_markdown(&report)
                .contains("PR review front panel: not_generated")
        );
        Ok(())
    }

    #[test]
    fn report_packet_index_preserves_blocked_gate_authority() -> Result<(), String> {
        let root = temp_root("blocked-gate")?;
        write(
            &root.join("target/ripr/reports/pr-review-front-panel.md"),
            "Status: blocked\n",
        )?;
        write(
            &root.join("target/ripr/reports/pr-review-front-panel.json"),
            r#"{"status":"blocked"}"#,
        )?;
        write(
            &root.join("target/ripr/reports/gate-decision.md"),
            "Status: blocked\n",
        )?;
        write(
            &root.join("target/ripr/reports/gate-decision.json"),
            r#"{"decision":"blocked"}"#,
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));

        assert_eq!(report.status, "fail");
        assert_eq!(
            report.summary.gate_authority.as_deref(),
            Some("target/ripr/reports/gate-decision.md")
        );
        assert!(render_report_packet_index_json(&report)?.contains("\"authority\": true"));
        Ok(())
    }

    #[test]
    fn report_packet_index_groups_coverage_grip_as_calibration() -> Result<(), String> {
        let root = temp_root("coverage-grip")?;
        write(
            &root.join("target/ripr/reports/pr-review-front-panel.md"),
            "Status: pass\n",
        )?;
        write(
            &root.join("target/ripr/reports/coverage-grip-frontier.md"),
            "Status: pass\n",
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        let rendered: Value = serde_json::from_str(&render_report_packet_index_json(&report)?)
            .map_err(|err| format!("parse rendered index JSON failed: {err}"))?;
        let groups = rendered
            .get("groups")
            .and_then(Value::as_array)
            .ok_or_else(|| "groups missing".to_string())?;
        let calibration = groups.iter().any(|group| {
            group.get("group").and_then(Value::as_str) == Some("calibration")
                && group
                    .get("entries")
                    .and_then(Value::as_array)
                    .is_some_and(|entries| {
                        entries.iter().any(|entry| {
                            entry.get("id").and_then(Value::as_str)
                                == Some("coverage_grip_frontier")
                        })
                    })
        });
        assert!(calibration);
        Ok(())
    }

    fn input_for_root(root: &Path) -> ReportPacketIndexInput {
        ReportPacketIndexInput {
            root: ".".to_string(),
            generated_at: "2026-05-10T12:00:00Z".to_string(),
            reports_dir: root.join("target/ripr/reports"),
            review_dir: root.join("target/ripr/review"),
            receipts_dir: root.join("target/ripr/receipts"),
            workflow_dir: root.join("target/ripr/workflow"),
            agent_dir: root.join("target/ripr/agent"),
            pilot_dir: root.join("target/ripr/pilot"),
            ci_dir: root.join("target/ci"),
        }
    }

    fn temp_root(name: &str) -> Result<PathBuf, String> {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| format!("system clock before unix epoch: {err}"))?
            .as_millis();
        let root = std::env::temp_dir().join(format!("ripr-report-packet-index-{name}-{millis}"));
        std::fs::create_dir_all(&root).map_err(|err| {
            format!(
                "create temp report-packet index root {} failed: {err}",
                root.display()
            )
        })?;
        Ok(root)
    }

    fn write(path: &Path, text: &str) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
        }
        std::fs::write(path, text).map_err(|err| format!("write {} failed: {err}", path.display()))
    }

    // ── gate_status variants ─────────────────────────────────────────────────

    #[test]
    fn gate_status_blocked_returns_blocked() -> Result<(), String> {
        assert_eq!(gate_status("blocked"), "blocked");
        Ok(())
    }

    #[test]
    fn gate_status_config_error_and_fail_and_failure_return_fail() -> Result<(), String> {
        assert_eq!(gate_status("config_error"), "fail");
        assert_eq!(gate_status("fail"), "fail");
        assert_eq!(gate_status("failure"), "fail");
        Ok(())
    }

    #[test]
    fn gate_status_acknowledged_suppressed_warn_incomplete() -> Result<(), String> {
        assert_eq!(gate_status("acknowledged"), "acknowledged");
        assert_eq!(gate_status("suppressed"), "suppressed");
        assert_eq!(gate_status("warn"), "warn");
        assert_eq!(gate_status("incomplete"), "incomplete");
        Ok(())
    }

    #[test]
    fn gate_status_unknown_falls_through_to_pass() -> Result<(), String> {
        assert_eq!(gate_status("pass"), "pass");
        assert_eq!(gate_status("anything_else"), "pass");
        Ok(())
    }

    // ── front_panel_status variants ──────────────────────────────────────────

    #[test]
    fn front_panel_status_blocked_returns_blocked() -> Result<(), String> {
        assert_eq!(front_panel_status("blocked"), "blocked");
        Ok(())
    }

    #[test]
    fn front_panel_status_fail_and_config_error_return_fail() -> Result<(), String> {
        assert_eq!(front_panel_status("fail"), "fail");
        assert_eq!(front_panel_status("config_error"), "fail");
        Ok(())
    }

    #[test]
    fn front_panel_status_warn_and_incomplete() -> Result<(), String> {
        assert_eq!(front_panel_status("warn"), "warn");
        assert_eq!(front_panel_status("incomplete"), "incomplete");
        Ok(())
    }

    #[test]
    fn front_panel_status_unknown_returns_available() -> Result<(), String> {
        assert_eq!(front_panel_status("pass"), "available");
        assert_eq!(front_panel_status("ok"), "available");
        Ok(())
    }

    // ── read_json_status: decision vs status field ───────────────────────────

    #[test]
    fn read_json_status_reads_decision_field_first() -> Result<(), String> {
        let root = temp_root("json-decision")?;
        let path = root.join("target/ripr/reports/gate-decision.json");
        write(&path, r#"{"decision":"blocked"}"#)?;
        let spec = ArtifactSpec {
            id: "gate_decision",
            label: "Gate decision",
            group: "policy_gates",
            kind: "markdown",
            path: root.join("target/ripr/reports/gate-decision.md"),
            json_path: Some(path),
            required: false,
            authority: true,
            description: "desc",
            default_status: "pass",
            next_command: None,
        };
        let status = read_json_status(&spec);
        let Some(s) = status else {
            return Err("expected Some status from read_json_status".to_string());
        };
        assert_eq!(s, "blocked");
        Ok(())
    }

    #[test]
    fn read_json_status_falls_back_to_status_field() -> Result<(), String> {
        let root = temp_root("json-status-field")?;
        let path = root.join("target/ripr/reports/front-panel.json");
        write(&path, r#"{"status":"warn"}"#)?;
        let spec = ArtifactSpec {
            id: "pr_review_front_panel",
            label: "PR review front panel",
            group: "start_here",
            kind: "markdown",
            path: root.join("target/ripr/reports/pr-review-front-panel.md"),
            json_path: Some(path),
            required: true,
            authority: false,
            description: "desc",
            default_status: "available",
            next_command: None,
        };
        let status = read_json_status(&spec);
        let Some(s) = status else {
            return Err("expected Some status from status field".to_string());
        };
        assert_eq!(s, "warn");
        Ok(())
    }

    #[test]
    fn read_json_status_returns_none_for_missing_file() -> Result<(), String> {
        let root = temp_root("json-missing")?;
        let spec = ArtifactSpec {
            id: "gate_decision",
            label: "Gate decision",
            group: "policy_gates",
            kind: "markdown",
            path: root.join("target/ripr/reports/gate-decision.md"),
            json_path: Some(root.join("target/ripr/reports/nonexistent.json")),
            required: false,
            authority: true,
            description: "desc",
            default_status: "pass",
            next_command: None,
        };
        assert_eq!(read_json_status(&spec), None);
        Ok(())
    }

    #[test]
    fn read_json_status_returns_none_when_no_json_path() -> Result<(), String> {
        let root = temp_root("json-no-path")?;
        let spec = ArtifactSpec {
            id: "pr_summary",
            label: "PR summary",
            group: "validation_receipts",
            kind: "markdown",
            path: root.join("target/ripr/reports/pr-summary.md"),
            json_path: None,
            required: false,
            authority: false,
            description: "desc",
            default_status: "available",
            next_command: None,
        };
        assert_eq!(read_json_status(&spec), None);
        Ok(())
    }

    // ── read_markdown_status ────────────────────────────────────────────────

    #[test]
    fn read_markdown_status_parses_status_line() -> Result<(), String> {
        let root = temp_root("md-status")?;
        let path = root.join("target/ripr/reports/check-pr.md");
        write(&path, "# Check PR\n\nStatus: pass\n\nSome other text\n")?;
        let status = read_markdown_status(&path);
        let Some(s) = status else {
            return Err("expected Some status from markdown".to_string());
        };
        assert_eq!(s, "pass");
        Ok(())
    }

    #[test]
    fn read_markdown_status_maps_fail_through_gate_status() -> Result<(), String> {
        let root = temp_root("md-status-fail")?;
        let path = root.join("target/ripr/reports/check-pr.md");
        write(&path, "Status: fail\n")?;
        let status = read_markdown_status(&path);
        let Some(s) = status else {
            return Err("expected Some from fail markdown".to_string());
        };
        assert_eq!(s, "fail");
        Ok(())
    }

    #[test]
    fn read_markdown_status_returns_none_for_missing_file() -> Result<(), String> {
        let root = temp_root("md-missing")?;
        let path = root.join("target/ripr/reports/nonexistent.md");
        assert_eq!(read_markdown_status(&path), None);
        Ok(())
    }

    #[test]
    fn read_markdown_status_returns_none_when_no_status_line() -> Result<(), String> {
        let root = temp_root("md-no-status")?;
        let path = root.join("target/ripr/reports/check-pr.md");
        write(&path, "# Check PR\n\nNo status line here.\n")?;
        assert_eq!(read_markdown_status(&path), None);
        Ok(())
    }

    // ── status_for_spec check_pr path ────────────────────────────────────────

    #[test]
    fn status_for_spec_check_pr_reads_markdown() -> Result<(), String> {
        let root = temp_root("check-pr-status")?;
        let md_path = root.join("target/ripr/reports/check-pr.md");
        write(&md_path, "Status: pass\n")?;
        let spec = ArtifactSpec {
            id: "check_pr",
            label: "Check PR",
            group: "validation_receipts",
            kind: "markdown",
            path: md_path,
            json_path: None,
            required: false,
            authority: false,
            description: "desc",
            default_status: "pass",
            next_command: Some("cargo xtask check-pr"),
        };
        assert_eq!(status_for_spec(&spec), "pass");
        Ok(())
    }

    #[test]
    fn status_for_spec_check_pr_falls_back_to_default() -> Result<(), String> {
        let root = temp_root("check-pr-default")?;
        let spec = ArtifactSpec {
            id: "check_pr",
            label: "Check PR",
            group: "validation_receipts",
            kind: "markdown",
            path: root.join("nonexistent.md"),
            json_path: None,
            required: false,
            authority: false,
            description: "desc",
            default_status: "pass",
            next_command: None,
        };
        assert_eq!(status_for_spec(&spec), "pass");
        Ok(())
    }

    #[test]
    fn status_for_spec_gate_decision_falls_back_to_default() -> Result<(), String> {
        let root = temp_root("gate-default")?;
        let spec = ArtifactSpec {
            id: "gate_decision",
            label: "Gate decision",
            group: "policy_gates",
            kind: "markdown",
            path: root.join("nonexistent.md"),
            json_path: Some(root.join("nonexistent.json")),
            required: false,
            authority: true,
            description: "desc",
            default_status: "pass",
            next_command: None,
        };
        assert_eq!(status_for_spec(&spec), "pass");
        Ok(())
    }

    #[test]
    fn status_for_spec_other_id_returns_default_status() -> Result<(), String> {
        let root = temp_root("spec-default")?;
        let spec = ArtifactSpec {
            id: "sarif",
            label: "SARIF output",
            group: "sarif_badges",
            kind: "json",
            path: root.join("ripr.sarif.json"),
            json_path: Some(root.join("ripr.sarif.json")),
            required: false,
            authority: false,
            description: "desc",
            default_status: "available",
            next_command: None,
        };
        assert_eq!(status_for_spec(&spec), "available");
        Ok(())
    }

    // ── group_for_entry: all groups ──────────────────────────────────────────

    #[test]
    fn group_for_entry_covers_all_known_ids() -> Result<(), String> {
        let cases: &[(&str, &str)] = &[
            ("pr_review_front_panel", "start_here"),
            ("first_useful_action", "pr_review_story"),
            ("review_comments", "pr_review_story"),
            ("assistant_proof", "repair_agent_handoff"),
            ("assistant_loop_health", "repair_agent_handoff"),
            ("pr_evidence_ledger", "evidence_movement"),
            ("baseline_debt_delta", "evidence_movement"),
            ("ripr_zero_status", "evidence_movement"),
            ("gate_decision", "policy_gates"),
            ("recommendation_calibration", "calibration"),
            ("mutation_calibration", "calibration"),
            ("coverage_grip_frontier", "calibration"),
            ("agent_receipt", "validation_receipts"),
            ("pr_summary", "validation_receipts"),
            ("check_pr", "validation_receipts"),
            ("sarif", "sarif_badges"),
            ("badge", "sarif_badges"),
            ("unknown_artifact", "local_context"),
        ];
        for (id, expected_group) in cases {
            let actual = group_for_entry(id);
            if actual != *expected_group {
                return Err(format!(
                    "group_for_entry({id:?}) = {actual:?}, want {expected_group:?}"
                ));
            }
        }
        Ok(())
    }

    // ── group_label and group_summary: all variants ──────────────────────────

    #[test]
    fn group_label_covers_all_known_groups() -> Result<(), String> {
        let cases: &[(&str, &str)] = &[
            ("start_here", "Start here"),
            ("pr_review_story", "PR review story"),
            ("repair_agent_handoff", "Repair and agent handoff"),
            ("evidence_movement", "Evidence movement"),
            ("policy_gates", "Policy and gates"),
            ("calibration", "Calibration"),
            ("validation_receipts", "Validation receipts"),
            ("sarif_badges", "SARIF and badges"),
            ("local_context", "Local context"),
            ("unknown", "Local context"),
        ];
        for (group, expected) in cases {
            let actual = group_label(group);
            if actual != *expected {
                return Err(format!(
                    "group_label({group:?}) = {actual:?}, want {expected:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn group_summary_covers_all_known_groups() -> Result<(), String> {
        let cases: &[(&str, &str)] = &[
            ("start_here", "Reviewer-first PR story."),
            ("pr_review_story", "Guidance and first useful action."),
            ("repair_agent_handoff", "Proof, health, and repair routing."),
            ("evidence_movement", "Debt delta and PR movement."),
            ("policy_gates", "Configured pass/fail authority."),
            ("calibration", "Recommendation and coverage/grip context."),
            (
                "validation_receipts",
                "Receipts and local readiness reports.",
            ),
            ("sarif_badges", "Code scanning and badge surfaces."),
            ("local_context", "Repo-local context."),
            ("unknown", "Repo-local context."),
        ];
        for (group, expected) in cases {
            let actual = group_summary(group);
            if actual != *expected {
                return Err(format!(
                    "group_summary({group:?}) = {actual:?}, want {expected:?}"
                ));
            }
        }
        Ok(())
    }

    // ── display_path: target/ci/ marker ─────────────────────────────────────

    #[test]
    fn display_path_strips_ci_prefix() -> Result<(), String> {
        let path = PathBuf::from("/some/absolute/path/target/ci/reports/foo.json");
        assert_eq!(display_path(&path), "target/ci/reports/foo.json");
        Ok(())
    }

    #[test]
    fn display_path_strips_ripr_prefix() -> Result<(), String> {
        let path = PathBuf::from("/project/target/ripr/reports/index.json");
        assert_eq!(display_path(&path), "target/ripr/reports/index.json");
        Ok(())
    }

    #[test]
    fn display_path_returns_normalized_when_no_marker() -> Result<(), String> {
        let path = PathBuf::from("/absolute/other/path/foo.json");
        assert_eq!(display_path(&path), "/absolute/other/path/foo.json");
        Ok(())
    }

    // ── entry_from_spec: available vs not-available (next_command) ───────────

    #[test]
    fn entry_from_spec_available_clears_next_command() -> Result<(), String> {
        let root = temp_root("entry-available")?;
        let spec = ArtifactSpec {
            id: "agent_receipt",
            label: "Agent receipt",
            group: "validation_receipts",
            kind: "json",
            path: root.join("agent-receipt.json"),
            json_path: Some(root.join("agent-receipt.json")),
            required: false,
            authority: false,
            description: "desc",
            default_status: "available",
            next_command: Some("ripr agent receipt --out agent-receipt.json"),
        };
        let entry = entry_from_spec(&spec, true, "available".to_string());
        assert_eq!(entry.next_command, None);
        assert!(entry.available);
        Ok(())
    }

    #[test]
    fn entry_from_spec_not_available_preserves_next_command() -> Result<(), String> {
        let root = temp_root("entry-missing")?;
        let spec = ArtifactSpec {
            id: "agent_receipt",
            label: "Agent receipt",
            group: "validation_receipts",
            kind: "json",
            path: root.join("agent-receipt.json"),
            json_path: Some(root.join("agent-receipt.json")),
            required: false,
            authority: false,
            description: "desc",
            default_status: "available",
            next_command: Some("ripr agent receipt --out agent-receipt.json"),
        };
        let entry = entry_from_spec(&spec, false, "missing".to_string());
        let Some(cmd) = &entry.next_command else {
            return Err("expected next_command to be Some when not available".to_string());
        };
        assert_eq!(cmd, "ripr agent receipt --out agent-receipt.json");
        assert!(!entry.available);
        Ok(())
    }

    // ── missing_expected_surfaces: various combos ────────────────────────────

    #[test]
    fn missing_expected_no_review_story_means_no_missing() -> Result<(), String> {
        let root = temp_root("missing-no-review")?;
        // Only sarif/badge artifacts present — no review story artifacts
        write(&root.join("target/ripr/reports/ripr.sarif.json"), "{}")?;
        let report = build_report_packet_index_report(input_for_root(&root));
        assert_eq!(report.summary.missing_expected, 0);
        Ok(())
    }

    #[test]
    fn missing_expected_first_useful_action_triggers_assistant_proof_missing() -> Result<(), String>
    {
        let root = temp_root("missing-assistant-proof")?;
        // first_useful_action present but no assistant_proof
        write(
            &root.join("target/ripr/reports/first-useful-action.md"),
            "content\n",
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        let has_assistant_proof_missing = report
            .missing_expected
            .iter()
            .any(|m| m.id == "assistant_proof");
        assert!(
            has_assistant_proof_missing,
            "expected assistant_proof in missing_expected"
        );
        Ok(())
    }

    #[test]
    fn missing_expected_assistant_proof_and_health_trigger_agent_receipt_and_check_pr()
    -> Result<(), String> {
        let root = temp_root("missing-agent-receipt")?;
        // assistant_proof + assistant_loop_health present but no agent_receipt or check_pr
        write(
            &root.join("target/ripr/reports/test-oracle-assistant-proof.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/assistant-loop-health.md"),
            "content\n",
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        let has_agent_receipt = report
            .missing_expected
            .iter()
            .any(|m| m.id == "agent_receipt");
        let has_check_pr = report.missing_expected.iter().any(|m| m.id == "check_pr");
        assert!(
            has_agent_receipt,
            "expected agent_receipt in missing_expected"
        );
        assert!(has_check_pr, "expected check_pr in missing_expected");
        Ok(())
    }

    #[test]
    fn missing_expected_with_agent_receipt_suppresses_agent_receipt_missing() -> Result<(), String>
    {
        let root = temp_root("agent-receipt-present")?;
        write(
            &root.join("target/ripr/reports/test-oracle-assistant-proof.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/assistant-loop-health.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/agent-receipt.json"),
            r#"{"status":"available"}"#,
        )?;
        write(
            &root.join("target/ripr/reports/check-pr.md"),
            "Status: pass\n",
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        let has_agent_receipt = report
            .missing_expected
            .iter()
            .any(|m| m.id == "agent_receipt");
        assert!(
            !has_agent_receipt,
            "agent_receipt should not appear in missing when present"
        );
        Ok(())
    }

    // ── build_report overall status variants ─────────────────────────────────

    #[test]
    fn build_report_status_incomplete_when_no_artifacts() -> Result<(), String> {
        let root = temp_root("status-incomplete")?;
        let report = build_report_packet_index_report(input_for_root(&root));
        assert_eq!(report.status, "incomplete");
        Ok(())
    }

    #[test]
    fn build_report_status_pass_when_all_available_no_issues() -> Result<(), String> {
        let root = temp_root("status-pass")?;
        write(
            &root.join("target/ripr/reports/pr-review-front-panel.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/pr-review-front-panel.json"),
            r#"{"status":"pass"}"#,
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        assert_eq!(report.status, "pass");
        Ok(())
    }

    #[test]
    fn build_report_status_fail_when_gate_is_failed() -> Result<(), String> {
        let root = temp_root("status-fail")?;
        write(
            &root.join("target/ripr/reports/gate-decision.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/gate-decision.json"),
            r#"{"decision":"fail"}"#,
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        assert_eq!(report.status, "fail");
        Ok(())
    }

    #[test]
    fn build_report_warns_when_artifact_has_warn_status() -> Result<(), String> {
        let root = temp_root("status-warn-entry")?;
        write(
            &root.join("target/ripr/reports/pr-review-front-panel.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/pr-review-front-panel.json"),
            r#"{"status":"warn"}"#,
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        assert_eq!(report.status, "warn");
        Ok(())
    }

    // ── markdown rendering: start_here with gate_authority ───────────────────

    #[test]
    fn markdown_renders_start_here_with_gate_authority() -> Result<(), String> {
        let root = temp_root("md-start-gate")?;
        write(
            &root.join("target/ripr/reports/pr-review-front-panel.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/pr-review-front-panel.json"),
            r#"{"status":"pass"}"#,
        )?;
        write(
            &root.join("target/ripr/reports/gate-decision.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/gate-decision.json"),
            r#"{"decision":"pass"}"#,
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        let md = render_report_packet_index_markdown(&report);
        assert!(
            md.contains("Gate authority:"),
            "expected gate authority line in markdown: {md}"
        );
        assert!(
            md.contains("Start here:"),
            "expected start here in markdown: {md}"
        );
        Ok(())
    }

    #[test]
    fn markdown_renders_authority_note_for_gate_entry() -> Result<(), String> {
        let root = temp_root("md-authority-note")?;
        write(
            &root.join("target/ripr/reports/gate-decision.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/gate-decision.json"),
            r#"{"decision":"pass"}"#,
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        let md = render_report_packet_index_markdown(&report);
        assert!(
            md.contains("authority: gate decision controls"),
            "expected authority note in markdown: {md}"
        );
        Ok(())
    }

    #[test]
    fn markdown_renders_next_command_for_missing_entry() -> Result<(), String> {
        let root = temp_root("md-next-cmd")?;
        // Make front_useful_action present but not pr_review_front_panel (triggers warning)
        write(
            &root.join("target/ripr/reports/first-useful-action.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/review/comments.md"),
            "comments\n",
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        let md = render_report_packet_index_markdown(&report);
        // The missing entry for pr_review_front_panel should show a next: command
        assert!(
            md.contains("next: `ripr pr-review"),
            "expected next command hint in markdown: {md}"
        );
        Ok(())
    }

    #[test]
    fn markdown_renders_missing_expected_section() -> Result<(), String> {
        let root = temp_root("md-missing-expected")?;
        write(
            &root.join("target/ripr/reports/first-useful-action.md"),
            "content\n",
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        let md = render_report_packet_index_markdown(&report);
        assert!(
            md.contains("Missing expected:"),
            "expected Missing expected section in markdown: {md}"
        );
        Ok(())
    }

    // ── JSON render includes schema metadata ─────────────────────────────────

    #[test]
    fn json_render_includes_schema_version_and_kind() -> Result<(), String> {
        let root = temp_root("json-schema")?;
        write(
            &root.join("target/ripr/reports/pr-review-front-panel.md"),
            "content\n",
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        let json = render_report_packet_index_json(&report)?;
        let value: Value =
            serde_json::from_str(&json).map_err(|err| format!("parse json failed: {err}"))?;
        let schema_version = value
            .get("schema_version")
            .and_then(Value::as_str)
            .ok_or_else(|| "schema_version missing".to_string())?;
        assert_eq!(schema_version, "0.1");
        let kind = value
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "kind missing".to_string())?;
        assert_eq!(kind, "report_packet_index");
        Ok(())
    }

    // ── artifact_available: json_path fallback ───────────────────────────────

    #[test]
    fn artifact_available_true_when_only_json_path_exists() -> Result<(), String> {
        let root = temp_root("avail-json")?;
        let json_path = root.join("target/ripr/reports/front-panel.json");
        write(&json_path, r#"{"status":"pass"}"#)?;
        let spec = ArtifactSpec {
            id: "pr_review_front_panel",
            label: "PR review front panel",
            group: "start_here",
            kind: "markdown",
            path: root.join("target/ripr/reports/nonexistent.md"),
            json_path: Some(json_path),
            required: true,
            authority: false,
            description: "desc",
            default_status: "available",
            next_command: None,
        };
        assert!(artifact_available(&spec));
        Ok(())
    }

    #[test]
    fn artifact_available_false_when_neither_path_exists() -> Result<(), String> {
        let root = temp_root("avail-none")?;
        let spec = ArtifactSpec {
            id: "pr_review_front_panel",
            label: "PR review front panel",
            group: "start_here",
            kind: "markdown",
            path: root.join("nonexistent.md"),
            json_path: Some(root.join("nonexistent.json")),
            required: true,
            authority: false,
            description: "desc",
            default_status: "available",
            next_command: None,
        };
        assert!(!artifact_available(&spec));
        Ok(())
    }

    // ── warnings list populated from missing_expected ─────────────────────────

    #[test]
    fn warnings_list_populated_for_missing_front_panel() -> Result<(), String> {
        let root = temp_root("warnings")?;
        write(
            &root.join("target/ripr/reports/first-useful-action.md"),
            "content\n",
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        assert!(
            !report.warnings.is_empty(),
            "expected warnings to be non-empty"
        );
        let has_missing_kind = report.warnings.iter().any(|w| w.kind == "missing_expected");
        assert!(has_missing_kind, "expected missing_expected warning kind");
        Ok(())
    }

    // ── gate_decision: acknowledged and suppressed in full pipeline ──────────

    #[test]
    fn gate_decision_acknowledged_produces_pass_overall_status() -> Result<(), String> {
        let root = temp_root("gate-acknowledged")?;
        write(
            &root.join("target/ripr/reports/gate-decision.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/gate-decision.json"),
            r#"{"decision":"acknowledged"}"#,
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        // acknowledged is not "fail" or "blocked", so overall status should not be fail
        assert_ne!(report.status, "fail");
        // The gate entry itself should have status "acknowledged"
        let gate_entry = report
            .groups
            .iter()
            .flat_map(|g| g.entries.iter())
            .find(|e| e.id == "gate_decision");
        let Some(entry) = gate_entry else {
            return Err("expected gate_decision entry".to_string());
        };
        assert_eq!(entry.status, "acknowledged");
        Ok(())
    }

    #[test]
    fn gate_decision_suppressed_produces_pass_overall_status() -> Result<(), String> {
        let root = temp_root("gate-suppressed")?;
        write(
            &root.join("target/ripr/reports/gate-decision.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/gate-decision.json"),
            r#"{"decision":"suppressed"}"#,
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        assert_ne!(report.status, "fail");
        let gate_entry = report
            .groups
            .iter()
            .flat_map(|g| g.entries.iter())
            .find(|e| e.id == "gate_decision");
        let Some(entry) = gate_entry else {
            return Err("expected gate_decision entry".to_string());
        };
        assert_eq!(entry.status, "suppressed");
        Ok(())
    }

    #[test]
    fn gate_decision_incomplete_produces_warn_overall_status() -> Result<(), String> {
        let root = temp_root("gate-incomplete")?;
        write(
            &root.join("target/ripr/reports/gate-decision.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/gate-decision.json"),
            r#"{"decision":"incomplete"}"#,
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        let gate_entry = report
            .groups
            .iter()
            .flat_map(|g| g.entries.iter())
            .find(|e| e.id == "gate_decision");
        let Some(entry) = gate_entry else {
            return Err("expected gate_decision entry".to_string());
        };
        assert_eq!(entry.status, "incomplete");
        assert_eq!(report.status, "warn");
        Ok(())
    }

    // ── front_panel: warn and incomplete in full pipeline ────────────────────

    #[test]
    fn front_panel_incomplete_json_status_produces_warn_overall() -> Result<(), String> {
        let root = temp_root("front-panel-incomplete")?;
        write(
            &root.join("target/ripr/reports/pr-review-front-panel.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/pr-review-front-panel.json"),
            r#"{"status":"incomplete"}"#,
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        let entry = report
            .groups
            .iter()
            .flat_map(|g| g.entries.iter())
            .find(|e| e.id == "pr_review_front_panel");
        let Some(e) = entry else {
            return Err("expected pr_review_front_panel entry".to_string());
        };
        assert_eq!(e.status, "incomplete");
        assert_eq!(report.status, "warn");
        Ok(())
    }

    #[test]
    fn front_panel_config_error_json_status_produces_fail_overall() -> Result<(), String> {
        let root = temp_root("front-panel-config-error")?;
        write(
            &root.join("target/ripr/reports/pr-review-front-panel.md"),
            "content\n",
        )?;
        write(
            &root.join("target/ripr/reports/pr-review-front-panel.json"),
            r#"{"status":"config_error"}"#,
        )?;
        let report = build_report_packet_index_report(input_for_root(&root));
        let entry = report
            .groups
            .iter()
            .flat_map(|g| g.entries.iter())
            .find(|e| e.id == "pr_review_front_panel");
        let Some(e) = entry else {
            return Err("expected pr_review_front_panel entry".to_string());
        };
        assert_eq!(e.status, "fail");
        assert_eq!(report.status, "fail");
        Ok(())
    }
}
