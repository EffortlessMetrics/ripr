use crate::reports::pr_evidence_summary::io::file_state;
use crate::reports::pr_evidence_summary::model::JsonInput;
use crate::reports::pr_evidence_summary::util::{
    md_escape, string_field, summary_bool, summary_string_or_null, summary_u64,
};
use serde_json::Value;
use std::path::Path;

pub(super) struct SummaryRenderInput<'a> {
    pub(super) repo: &'a Path,
    pub(super) pr_evidence_json: &'a str,
    pub(super) review_comments_json: &'a str,
    pub(super) start_here_json: &'a str,
    pub(super) pr_evidence_md: &'a str,
    pub(super) review_comments_md: &'a str,
    pub(super) start_here_md: &'a str,
    pub(super) pr_summary_md: &'a str,
    pub(super) pr_evidence: &'a JsonInput,
    pub(super) review_comments: &'a JsonInput,
    pub(super) start_here: &'a JsonInput,
}

pub(super) fn render_pr_evidence_summary(input: &SummaryRenderInput<'_>) -> String {
    let pr_value = input.pr_evidence.value.as_ref();
    let review_value = input.review_comments.value.as_ref();
    let start_here_value = input.start_here.value.as_ref();

    let mut out = String::new();
    out.push_str("# PR Evidence Summary\n\n");
    render_start_here(&mut out, start_here_value, input.start_here);
    render_fast_gate(
        &mut out,
        pr_value,
        review_value,
        input.pr_evidence,
        input.review_comments,
    );
    render_ripr(&mut out, pr_value, review_value);
    render_targeted_mutation(&mut out, pr_value);
    render_artifacts(&mut out, input);
    out.push_str(
        "\n_This summary is generated from diff-scoped artifacts. Do not copy it into public badge state._\n",
    );
    out
}

fn render_start_here(out: &mut String, start_here_value: Option<&Value>, start_here: &JsonInput) {
    out.push_str("## Start Here\n\n");
    out.push_str(&format!("- start-here JSON: {}\n", start_here.state));
    out.push_str(&format!(
        "- status: `{}`\n",
        value_string(start_here_value, &["status"])
    ));
    out.push_str(&format!(
        "- selected state: `{}`\n",
        value_string(start_here_value, &["selected", "state"])
    ));

    match value_string(start_here_value, &["selected", "state"]).as_str() {
        "top_gap" => render_start_here_top_gap(out, start_here_value),
        "missing_artifact" => render_start_here_missing(out, start_here_value),
        "no_action" | "empty_diff" => render_start_here_no_action(out, start_here_value),
        "not_available" => {
            out.push_str(
                "- next: generate `target/ripr/reports/start-here.json` before relying on a top repair unit.\n",
            );
        }
        _ => render_start_here_blocked(out, start_here_value),
    }

    render_start_here_limits(out, start_here_value);
    out.push_str(
        "- boundary: static advisory evidence only; gate decision remains separate pass/fail authority when configured.\n\n",
    );
}

fn render_start_here_top_gap(out: &mut String, start_here_value: Option<&Value>) {
    out.push_str(&format!(
        "- canonical gap: `{}`\n",
        first_available_string(
            start_here_value,
            &[&["selected", "canonical_gap_id"], &["selected", "gap_id"]]
        )
    ));
    out.push_str(&format!(
        "- language: `{}` ({})\n",
        value_string(start_here_value, &["selected", "language"]),
        value_string(start_here_value, &["selected", "language_status"])
    ));
    out.push_str(&format!(
        "- top gap: `{}`\n",
        value_string(start_here_value, &["selected", "kind"])
    ));
    out.push_str(&format!(
        "- changed behavior: `{}`\n",
        value_string(start_here_value, &["selected", "changed_behavior"])
    ));
    out.push_str(&format!(
        "- missing discriminator: `{}`\n",
        value_string(start_here_value, &["selected", "missing_discriminator"])
    ));
    out.push_str(&format!(
        "- focused proof intent: `{}`\n",
        value_string(start_here_value, &["selected", "focused_proof_intent"])
    ));
    out.push_str(&format!(
        "- repair route: `{}`\n",
        value_string(start_here_value, &["selected", "repair", "route"])
    ));
    out.push_str(&format!(
        "- repair target: `{}`\n",
        value_string(start_here_value, &["selected", "repair", "target_file"])
    ));
    out.push_str(&format!(
        "- related test: `{}`\n",
        value_string(start_here_value, &["selected", "repair", "related_test"])
    ));
    out.push_str(&format!(
        "- static limit: `{}`\n",
        first_available_string_or(
            start_here_value,
            &[
                &["selected", "static_limit_kind"],
                &["selected", "static_limit_detail"]
            ],
            "none"
        )
    ));
    out.push_str(&format!(
        "- verify: `{}`\n",
        value_string(start_here_value, &["selected", "verify_command"])
    ));
    out.push_str(&format!(
        "- receipt: `{}`\n",
        value_string(start_here_value, &["selected", "receipt_command"])
    ));
    out.push_str(&format!(
        "- receipt state: `{}`\n",
        value_string(start_here_value, &["selected", "receipt_state"])
    ));
}

fn render_start_here_missing(out: &mut String, start_here_value: Option<&Value>) {
    out.push_str(&format!(
        "- missing artifact: `{}`\n",
        value_string(start_here_value, &["selected", "artifact", "path"])
    ));
    out.push_str(&format!(
        "- next command: `{}`\n",
        value_string(start_here_value, &["selected", "regeneration_command"])
    ));
}

fn render_start_here_no_action(out: &mut String, start_here_value: Option<&Value>) {
    out.push_str(&format!(
        "- reason: `{}`\n",
        value_string(start_here_value, &["selected", "reason"])
    ));
    out.push_str("- no-action is not runtime, coverage, mutation, gate, or merge adequacy.\n");
}

fn render_start_here_blocked(out: &mut String, start_here_value: Option<&Value>) {
    out.push_str(&format!(
        "- blocked reason: `{}`\n",
        value_string(start_here_value, &["selected", "message"])
    ));
    out.push_str(&format!(
        "- next command: `{}`\n",
        value_string(start_here_value, &["selected", "next_command"])
    ));
}

fn render_start_here_limits(out: &mut String, start_here_value: Option<&Value>) {
    let Some(limits) = start_here_value
        .and_then(|value| value.get("limits"))
        .and_then(Value::as_array)
    else {
        return;
    };
    if limits.is_empty() {
        return;
    }
    out.push_str("- limits: ");
    let rendered = limits
        .iter()
        .filter_map(Value::as_str)
        .map(md_escape)
        .collect::<Vec<_>>()
        .join("; ");
    out.push_str(&rendered);
    out.push('\n');
}

fn render_fast_gate(
    out: &mut String,
    pr_value: Option<&Value>,
    review_value: Option<&Value>,
    pr_evidence: &JsonInput,
    review_comments: &JsonInput,
) {
    out.push_str("## Fast Gate\n\n");
    out.push_str(&format!("- PR evidence JSON: {}\n", pr_evidence.state));
    out.push_str(&format!(
        "- review guidance JSON: {}\n",
        review_comments.state
    ));
    out.push_str(&format!(
        "- PR evidence status: `{}`\n",
        string_field(pr_value, "status")
    ));
    out.push_str(&format!(
        "- review guidance status: `{}`\n",
        string_field(review_value, "status")
    ));
    out.push_str(&format!("- base: `{}`\n", string_field(pr_value, "base")));
    out.push_str(&format!("- head: `{}`\n", string_field(pr_value, "head")));
    out.push_str(&format!(
        "- changed files: {}\n\n",
        summary_u64(pr_value, "changed_files")
    ));
}

fn render_ripr(out: &mut String, pr_value: Option<&Value>, review_value: Option<&Value>) {
    out.push_str("## RIPR\n\n");
    out.push_str(&format!(
        "- changed-line comments: {}\n",
        summary_u64(review_value.or(pr_value), "comments")
    ));
    out.push_str(&format!(
        "- summary-only guidance: {}\n",
        summary_u64(review_value.or(pr_value), "summary_only")
    ));
    out.push_str(&format!(
        "- suppressed guidance: {}\n",
        summary_u64(review_value.or(pr_value), "suppressed")
    ));
    out.push_str(&format!(
        "- weakly_exposed: {}\n",
        summary_u64(pr_value, "weakly_exposed")
    ));
    out.push_str(&format!(
        "- reachable_unrevealed: {}\n",
        summary_u64(pr_value, "reachable_unrevealed")
    ));
    out.push_str(&format!(
        "- no_static_path: {}\n",
        summary_u64(pr_value, "no_static_path")
    ));
    out.push_str(&format!(
        "- severe gaps: {}\n\n",
        summary_u64(pr_value, "severe_gaps")
    ));
}

fn render_targeted_mutation(out: &mut String, pr_value: Option<&Value>) {
    out.push_str("## Targeted Mutation\n\n");
    out.push_str(&format!(
        "- requires_targeted_mutation: {}\n",
        summary_bool(pr_value, "requires_targeted_mutation")
    ));
    out.push_str(&format!(
        "- ripr_severe_gap: {}\n",
        summary_bool(pr_value, "ripr_severe_gap")
    ));
    out.push_str(&format!(
        "- routing_reason: `{}`\n\n",
        summary_string_or_null(pr_value, "routing_reason")
    ));
}

fn render_artifacts(out: &mut String, input: &SummaryRenderInput<'_>) {
    out.push_str("## Artifacts\n\n");
    out.push_str("| Artifact | Path | State |\n");
    out.push_str("| --- | --- | --- |\n");
    out.push_str(&format!(
        "| PR evidence JSON | `{}` | {} |\n",
        input.pr_evidence_json, input.pr_evidence.state
    ));
    out.push_str(&format!(
        "| PR evidence Markdown | `{}` | {} |\n",
        input.pr_evidence_md,
        file_state(input.repo, input.pr_evidence_md)
    ));
    out.push_str(&format!(
        "| Review guidance JSON | `{}` | {} |\n",
        input.review_comments_json, input.review_comments.state
    ));
    out.push_str(&format!(
        "| Review guidance Markdown | `{}` | {} |\n",
        input.review_comments_md,
        file_state(input.repo, input.review_comments_md)
    ));
    out.push_str(&format!(
        "| Start-here JSON | `{}` | {} |\n",
        input.start_here_json, input.start_here.state
    ));
    out.push_str(&format!(
        "| Start-here Markdown | `{}` | {} |\n",
        input.start_here_md,
        file_state(input.repo, input.start_here_md)
    ));
    out.push_str(&format!(
        "| PR evidence summary Markdown | `{}` | generated |\n",
        input.pr_summary_md
    ));
}

fn first_available_string(value: Option<&Value>, paths: &[&[&str]]) -> String {
    first_available_string_or(value, paths, "not_available")
}

fn first_available_string_or(value: Option<&Value>, paths: &[&[&str]], fallback: &str) -> String {
    paths
        .iter()
        .find_map(|path| {
            let value = value_at_path(value, path)?;
            if value.is_null() {
                return None;
            }
            value.as_str().map(md_escape)
        })
        .unwrap_or_else(|| fallback.to_string())
}

fn value_string(value: Option<&Value>, path: &[&str]) -> String {
    let Some(value) = value_at_path(value, path) else {
        return "not_available".to_string();
    };
    if value.is_null() {
        "not_available".to_string()
    } else {
        value
            .as_str()
            .map(md_escape)
            .unwrap_or_else(|| "invalid".to_string())
    }
}

fn value_at_path<'a>(value: Option<&'a Value>, path: &[&str]) -> Option<&'a Value> {
    let mut current = value?;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}
