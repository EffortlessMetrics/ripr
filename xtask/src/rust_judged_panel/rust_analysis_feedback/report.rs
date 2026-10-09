//! One DTO, then JSON and Markdown. No overall analyzer score.

use serde_json::{Value, json};

use super::schema::{
    FeedbackRow, count_by_direction, count_by_family, count_by_reduction, count_by_semantic_owner,
    count_by_status, count_new_owners, is_analyzer_defect,
};
use super::{ValidatedBundle, sha256_bytes};

pub(crate) struct RenderedFeedback {
    pub(crate) json: String,
    pub(crate) markdown: String,
    pub(crate) row_count: usize,
    pub(crate) analyzer_defect_count: usize,
}

#[derive(Clone, Debug)]
struct FeedbackReport {
    kind: &'static str,
    authority: String,
    judgments_sha256: String,
    calibration_status: String,
    rows: Vec<FeedbackRow>,
}

pub(crate) fn render(bundle: &ValidatedBundle) -> Result<RenderedFeedback, String> {
    let report = FeedbackReport {
        kind: "rust_judged_panel_feedback_report",
        authority: bundle.ledger.authority.clone(),
        judgments_sha256: bundle.judgments_sha256.clone(),
        calibration_status: bundle.ledger.calibration.status.clone(),
        rows: bundle.ledger.rows.clone(),
    };
    render_report(&report, bundle.facts.len())
}

fn render_report(report: &FeedbackReport, judged_count: usize) -> Result<RenderedFeedback, String> {
    let mut rows = report.rows.clone();
    rows.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    let directions = count_by_direction(&rows);
    let statuses = count_by_status(&rows);
    let reductions = count_by_reduction(&rows);
    let families = count_by_family(&rows);
    let owners = count_by_semantic_owner(&rows);
    let defect_count = rows
        .iter()
        .filter(|row| is_analyzer_defect(&row.failure_direction))
        .count();
    let unowned = rows
        .iter()
        .filter(|row| {
            is_analyzer_defect(&row.failure_direction)
                && row.owner.designated == "unowned_no_github_mutation"
        })
        .count();
    let existing_owner = rows
        .iter()
        .filter(|row| row.owner.existing.is_some())
        .count();
    let dto = json!({
        "schema_version": "0.1",
        "kind": report.kind,
        "authority": report.authority,
        "release_judgments_sha256": report.judgments_sha256,
        "calibration_status": report.calibration_status,
        "counts": {
            "judged_cases": judged_count,
            "rows": rows.len(),
            "analyzer_defects": defect_count,
            "by_failure_direction": directions,
            "by_status": statuses,
            "by_reduction": reductions,
            "by_analyzer_family": families,
            "by_semantic_owner": owners,
            "existing_owner": existing_owner,
            "new_owner": count_new_owners(&rows),
            "unowned_no_github_mutation": unowned,
        },
        "non_claims": [
            "No overall analyzer score.",
            "No support-tier, release, or publication claim.",
            "Runtime calibration remains #4795 metadata and does not set static class.",
            "This ledger does not mutate GitHub or repair the analyzer."
        ],
        "rows": rows.iter().map(row_json).collect::<Vec<_>>(),
    });
    let json_text = serde_json::to_string_pretty(&dto).map_err(|error| error.to_string())?;
    let markdown = render_markdown(&dto, &json_text)?;
    Ok(RenderedFeedback {
        json: json_text,
        markdown,
        row_count: rows.len(),
        analyzer_defect_count: defect_count,
    })
}

fn row_json(row: &FeedbackRow) -> Value {
    json!({
        "feedback_id": row.feedback_id,
        "case_id": row.case_id,
        "behavior_identity": row.behavior_identity,
        "failure_direction": row.failure_direction,
        "status": row.status,
        "reduction": row.reduction.disposition,
        "analyzer_family": row.analyzer_family,
        "semantic_owner": row.semantic_owner,
        "designated_owner": row.owner.designated,
        "existing_owner": row.owner.existing,
        "target_identity": row.target_identity,
        "merged_implementation": row.repair.merged_implementation,
        "original_case_replay": row.repair.original_case_replay,
        "notes": row.notes,
    })
}

fn render_markdown(dto: &Value, json_text: &str) -> Result<String, String> {
    let counts = &dto["counts"];
    let mut body = String::from("# Rust judged-panel feedback ledger\n\n");
    body.push_str(
        "Authority: `#4796`. Judgments: `#3806`. Calibration: `#4795` (not absorbed).\n\n",
    );
    body.push_str(&format!(
        "Rows: {}. Analyzer defects: {}. Judged cases: {}.\n\n",
        counts["rows"], counts["analyzer_defects"], counts["judged_cases"]
    ));
    append_count_section(
        &mut body,
        "Counts by failure direction",
        &counts["by_failure_direction"],
    );
    append_count_section(
        &mut body,
        "Counts by lifecycle status",
        &counts["by_status"],
    );
    append_count_section(
        &mut body,
        "Counts by analyzer family",
        &counts["by_analyzer_family"],
    );
    append_count_section(&mut body, "Counts by reduction", &counts["by_reduction"]);
    append_count_section(
        &mut body,
        "Counts by semantic owner",
        &counts["by_semantic_owner"],
    );
    body.push_str("## Rows\n\n");
    if let Some(rows) = dto["rows"].as_array() {
        for row in rows {
            body.push_str(&format!(
                "- `{}` ({}) direction=`{}` status=`{}` owner=`{}` family=`{}`\n",
                row["case_id"].as_str().unwrap_or("?"),
                row["feedback_id"].as_str().unwrap_or("?"),
                row["failure_direction"].as_str().unwrap_or("?"),
                row["status"].as_str().unwrap_or("?"),
                row["designated_owner"].as_str().unwrap_or("?"),
                row["analyzer_family"].as_str().unwrap_or("?"),
            ));
        }
    }
    body.push_str("\n## Non-claims\n\n");
    if let Some(items) = dto["non_claims"].as_array() {
        for item in items {
            body.push_str(&format!("- {}\n", item.as_str().unwrap_or("")));
        }
    }
    body.push_str(&format!(
        "\nReport digest: `{}`\n",
        sha256_bytes(json_text.as_bytes())
    ));
    Ok(body)
}

fn append_count_section(body: &mut String, heading: &str, value: &Value) {
    body.push_str(&format!("## {heading}\n\n"));
    if let Some(map) = value.as_object() {
        for (key, count) in map {
            body.push_str(&format!("- `{key}`: {count}\n"));
        }
    }
    body.push('\n');
}

#[cfg(test)]
pub(crate) fn render_from_rows_for_test(
    rows: Vec<FeedbackRow>,
    judged_count: usize,
    judgments_sha256: &str,
) -> Result<RenderedFeedback, String> {
    render_report(
        &FeedbackReport {
            kind: "rust_judged_panel_feedback_report",
            authority: "EffortlessMetrics/ripr-swarm#4796".into(),
            judgments_sha256: judgments_sha256.into(),
            calibration_status: "not_run".into(),
            rows,
        },
        judged_count,
    )
}
