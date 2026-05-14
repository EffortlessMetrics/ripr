use crate::agent::loop_commands::{
    WORKFLOW_AFTER_SNAPSHOT_ARTIFACT, WORKFLOW_AGENT_BRIEF_ARTIFACT,
    WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT, agent_brief_command, agent_verify_command, display_path,
};
use crate::app::Mode;
use crate::app::agent_brief::{
    AgentBriefResolvedWorkingSet, AgentBriefSelectedSeam, AgentBriefSelection,
    AgentBriefWhyNowReason,
};
use crate::config::RiprConfig;
use crate::output::agent_seam_packets;
use crate::output::gap_decision_ledger::{GapRecord, GapRepairRoute};
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::path::Path;

pub(crate) const REVIEW_COMMENTS_SCHEMA_VERSION: &str = "0.1";
pub(crate) const DEFAULT_REVIEW_MAX_INLINE_COMMENTS: usize = 3;
pub(crate) const DEFAULT_REVIEW_MAX_SUMMARY_ITEMS: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReviewPlacement {
    path: String,
    line: usize,
    mode: &'static str,
}

pub(crate) fn render_review_comments_json(
    root: &Path,
    base: &str,
    head: &str,
    mode: &Mode,
    config: &RiprConfig,
    working_set: &AgentBriefResolvedWorkingSet,
    selection: &AgentBriefSelection<'_>,
) -> Result<String, String> {
    let mut comments = Vec::new();
    let mut summary_only = Vec::new();
    let mut suppressed = Vec::new();
    let changed_test_paths = changed_test_paths(working_set);
    let actionable = selection
        .top_seams
        .iter()
        .filter(|selected| {
            selected.why_now.reason != AgentBriefWhyNowReason::RepoActionableFallback
        })
        .collect::<Vec<_>>();
    let suppressed_repo_fallback = !selection.top_seams.is_empty() && actionable.is_empty();
    let mut warnings = if suppressed_repo_fallback {
        selection
            .warnings
            .iter()
            .filter(|warning| !warning.contains("omitted by the brief cap"))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        selection.warnings.clone()
    };
    if suppressed_repo_fallback {
        warnings.push(
            "repo-actionable fallback seams were suppressed because PR guidance requires a changed working-set match"
                .to_string(),
        );
    }

    for selected in actionable.iter().take(DEFAULT_REVIEW_MAX_SUMMARY_ITEMS) {
        let recommendation = review_recommendation_json(root, mode, config, selected);
        let recommended_test = agent_seam_packets::recommended_test_for(selected.seam);
        if changed_test_paths
            .iter()
            .any(|path| path == &normalize_path_text(Path::new(&recommended_test.file)))
        {
            suppressed.push(suppressed_json(
                selected,
                "nearby_test_changed",
                "A nearby recommended test file changed in this pull request.",
            ));
            continue;
        }

        match placement_for(selected, working_set) {
            Some(placement) if comments.len() < DEFAULT_REVIEW_MAX_INLINE_COMMENTS => {
                let mut comment = recommendation;
                comment["placement"] = placement_json(&placement);
                comments.push(comment);
            }
            Some(placement) => {
                let mut item = recommendation;
                item["placement"] = placement_json(&placement);
                item["summary_reason"] = json!("inline comment cap reached");
                summary_only.push(item);
            }
            None => {
                let mut item = recommendation;
                item["placement"] = Value::Null;
                item["summary_reason"] =
                    json!("no safe changed-line placement was available for this seam");
                summary_only.push(item);
            }
        }
    }

    if actionable.len() > DEFAULT_REVIEW_MAX_SUMMARY_ITEMS {
        for selected in actionable.iter().skip(DEFAULT_REVIEW_MAX_SUMMARY_ITEMS) {
            suppressed.push(suppressed_json(
                selected,
                "summary_cap",
                "The PR guidance summary item cap was reached.",
            ));
        }
    }

    let value = json!({
        "schema_version": REVIEW_COMMENTS_SCHEMA_VERSION,
        "tool": "ripr",
        "status": "advisory",
        "root": display_path(root),
        "base": base,
        "head": head,
        "mode": mode.as_str(),
        "limits": {
            "max_inline_comments": DEFAULT_REVIEW_MAX_INLINE_COMMENTS,
            "max_summary_items": DEFAULT_REVIEW_MAX_SUMMARY_ITEMS,
        },
        "summary": {
            "comments": comments.len(),
            "summary_only": summary_only.len(),
            "suppressed": suppressed.len(),
            "unchanged_tests": changed_test_paths.is_empty(),
        },
        "comments": comments,
        "summary_only": summary_only,
        "suppressed": suppressed,
        "warnings": warnings,
        "limits_note": "Advisory static evidence only; no automatic edits, generated tests, runtime mutation execution, or CI blocking.",
    });

    serde_json::to_string_pretty(&value)
        .map_err(|err| format!("failed to render review comments JSON: {err}"))
}

pub(crate) fn render_gap_record_review_comments_json(
    root: &Path,
    base: &str,
    head: &str,
    mode: &Mode,
    gap_ledger_path: &str,
    records: &[GapRecord],
) -> Result<String, String> {
    let mut comments = Vec::new();
    let mut summary_only = Vec::new();
    let mut suppressed = Vec::new();
    let mut seen_dedupe = BTreeSet::new();

    for record in records {
        let comment = match gap_record_comment_json(root, gap_ledger_path, record, &mut seen_dedupe)
        {
            Ok(comment) => comment,
            Err(suppressed_item) => {
                suppressed.push(suppressed_item);
                continue;
            }
        };
        if comments.len() < DEFAULT_REVIEW_MAX_INLINE_COMMENTS {
            comments.push(comment);
        } else if summary_only.len() < DEFAULT_REVIEW_MAX_SUMMARY_ITEMS {
            let mut item = comment;
            item["summary_reason"] = json!("inline comment cap reached");
            summary_only.push(item);
        } else {
            suppressed.push(gap_record_cap_suppressed_json(record));
        }
    }

    let value = json!({
        "schema_version": REVIEW_COMMENTS_SCHEMA_VERSION,
        "tool": "ripr",
        "status": "advisory",
        "root": display_path(root),
        "base": base,
        "head": head,
        "mode": mode.as_str(),
        "inputs": {
            "gap_ledger": gap_ledger_path
        },
        "limits": {
            "max_inline_comments": DEFAULT_REVIEW_MAX_INLINE_COMMENTS,
            "max_summary_items": DEFAULT_REVIEW_MAX_SUMMARY_ITEMS,
        },
        "summary": {
            "comments": comments.len(),
            "summary_only": summary_only.len(),
            "suppressed": suppressed.len(),
            "unchanged_tests": true,
        },
        "comments": comments,
        "summary_only": summary_only,
        "suppressed": suppressed,
        "warnings": [],
        "limits_note": "Advisory static evidence only; gap-ledger repair cards do not edit source, generate tests, run mutation testing, or change CI/gate authority.",
    });

    serde_json::to_string_pretty(&value)
        .map_err(|err| format!("failed to render gap-ledger review comments JSON: {err}"))
}

pub(crate) fn render_review_comments_markdown(
    root: &Path,
    base: &str,
    head: &str,
    mode: &Mode,
    config: &RiprConfig,
    working_set: &AgentBriefResolvedWorkingSet,
    selection: &AgentBriefSelection<'_>,
) -> String {
    let Ok(rendered) =
        render_review_comments_json(root, base, head, mode, config, working_set, selection)
    else {
        return "# RIPR PR Guidance\n\nUnable to render PR guidance.\n".to_string();
    };
    let Ok(value) = serde_json::from_str::<Value>(&rendered) else {
        return "# RIPR PR Guidance\n\nUnable to parse rendered PR guidance.\n".to_string();
    };

    render_review_comments_markdown_value(root, base, head, mode, &value)
}

pub(crate) fn render_gap_record_review_comments_markdown(
    root: &Path,
    base: &str,
    head: &str,
    mode: &Mode,
    gap_ledger_path: &str,
    records: &[GapRecord],
) -> String {
    let Ok(rendered) =
        render_gap_record_review_comments_json(root, base, head, mode, gap_ledger_path, records)
    else {
        return "# RIPR PR Guidance\n\nUnable to render gap-ledger PR guidance.\n".to_string();
    };
    let Ok(value) = serde_json::from_str::<Value>(&rendered) else {
        return "# RIPR PR Guidance\n\nUnable to parse rendered gap-ledger PR guidance.\n"
            .to_string();
    };
    render_review_comments_markdown_value(root, base, head, mode, &value)
}

fn render_review_comments_markdown_value(
    root: &Path,
    base: &str,
    head: &str,
    mode: &Mode,
    value: &Value,
) -> String {
    let summary = value.get("summary").and_then(Value::as_object);
    let comments = summary
        .and_then(|summary| summary.get("comments"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let summary_only = summary
        .and_then(|summary| summary.get("summary_only"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let suppressed = summary
        .and_then(|summary| summary.get("suppressed"))
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let mut lines = vec![
        "# RIPR PR Guidance".to_string(),
        String::new(),
        format!("- root: {}", display_path(root)),
        format!("- base: {base}"),
        format!("- head: {head}"),
        format!("- mode: {}", mode.as_str()),
        format!("- line annotations: {comments}"),
        format!("- summary-only recommendations: {summary_only}"),
        format!("- suppressed recommendations: {suppressed}"),
        String::new(),
        "Advisory static evidence only. RIPR does not edit source, generate tests, run mutation testing, or make CI blocking by default.".to_string(),
        String::new(),
    ];

    push_markdown_items(&mut lines, "Line Annotations", value.get("comments"));
    push_markdown_items(
        &mut lines,
        "Summary-Only Recommendations",
        value.get("summary_only"),
    );
    push_suppressed_items(&mut lines, value.get("suppressed"));
    lines.push(String::new());
    lines.join("\n")
}

fn gap_record_comment_json(
    root: &Path,
    gap_ledger_path: &str,
    record: &GapRecord,
    seen_dedupe: &mut BTreeSet<String>,
) -> Result<Value, Value> {
    let Some(projection) = record.projection_eligibility.get("pr_comment") else {
        return Err(gap_record_suppressed_json(
            record,
            "not_pr_comment_eligible",
            "missing_pr_comment_projection",
        ));
    };
    if !projection.eligible {
        return Err(gap_record_suppressed_json(
            record,
            "not_pr_comment_eligible",
            &projection.reason,
        ));
    }
    if record.scope != "pr_local" || record.repairability != "repairable" {
        return Err(gap_record_suppressed_json(
            record,
            "not_pr_local_repairable",
            "PR comments require a PR-local repairable gap.",
        ));
    }
    if matches!(
        record.policy_state.as_str(),
        "suppressed" | "waived" | "resolved"
    ) {
        return Err(gap_record_suppressed_json(
            record,
            "policy_state_not_commentable",
            "Suppressed, waived, or resolved gaps are not PR-comment repair cards.",
        ));
    }
    let Some(anchor) = record.anchor.as_ref() else {
        return Err(gap_record_suppressed_json(
            record,
            "missing_anchor",
            "PR comments require a stable anchor.",
        ));
    };
    let Some(file) = anchor
        .file
        .as_deref()
        .map(str::trim)
        .filter(|file| !file.is_empty())
    else {
        return Err(gap_record_suppressed_json(
            record,
            "missing_anchor",
            "PR comments require an anchor file.",
        ));
    };
    let Some(line) = anchor.line else {
        return Err(gap_record_suppressed_json(
            record,
            "missing_anchor",
            "PR comments require an anchor line.",
        ));
    };
    let Some(dedupe) = anchor
        .dedupe_fingerprint
        .as_deref()
        .map(str::trim)
        .filter(|dedupe| !dedupe.is_empty())
    else {
        return Err(gap_record_suppressed_json(
            record,
            "missing_dedupe_fingerprint",
            "PR comments require a dedupe fingerprint.",
        ));
    };
    if file.is_empty() || dedupe.is_empty() {
        return Err(gap_record_suppressed_json(
            record,
            "missing_anchor",
            "PR comments require a non-empty anchor and dedupe fingerprint.",
        ));
    }
    if !seen_dedupe.insert(dedupe.to_string()) {
        return Err(gap_record_suppressed_json(
            record,
            "duplicate_dedupe_fingerprint",
            "A previous GapRecord already emitted this PR comment dedupe key.",
        ));
    }
    let Some(repair_route) = record.repair_route.as_ref() else {
        return Err(gap_record_suppressed_json(
            record,
            "missing_repair_route",
            "PR comments require a repair route.",
        ));
    };
    if record.verification_commands.is_empty() {
        return Err(gap_record_suppressed_json(
            record,
            "missing_verification_command",
            "PR comments require a verification command.",
        ));
    }

    let gap_id = gap_record_id(record);
    let repair_text = repair_text(repair_route);
    let why = repair_why(record, repair_route);
    let verify_command = record.verification_commands[0].clone();

    Ok(json!({
        "id": format!("ripr-review-{gap_id}"),
        "source": "gap_decision_ledger",
        "gap_id": gap_id.as_str(),
        "canonical_gap_id": non_empty(&record.canonical_gap_id),
        "gap_kind": record.kind.as_str(),
        "language": record.language.as_str(),
        "language_status": record.language_status.as_str(),
        "seam_id": gap_id.as_str(),
        "dedupe_key": dedupe,
        "placement": {
            "path": file,
            "line": line,
            "side": "RIGHT",
            "mode": "gap_record_anchor",
        },
        "kind": record.kind.as_str(),
        "grip_class": record.evidence_class.as_str(),
        "severity": "warning",
        "reason": why,
        "missing_discriminator": repair_route.changed_behavior.as_deref().or(repair_route.assertion_shape.as_deref()),
        "suggested_test": {
            "intent": repair_intent(&repair_route.route_kind),
            "candidate_values": [],
            "assertion_shape": repair_route.assertion_shape.as_deref(),
            "assertion_kind": repair_route.route_kind.as_str(),
            "recommended_file": repair_route.target_file.as_deref().or(repair_route.related_test.as_deref()),
            "recommended_name": repair_route.related_test.as_deref(),
            "near_test": repair_route.related_test.as_deref(),
        },
        "repair_card": {
            "gap_kind": record.kind.as_str(),
            "changed_behavior": repair_route.changed_behavior.as_deref(),
            "why_this_matters": why,
            "repair": repair_text,
            "repair_route": repair_route,
            "evidence_ids": &record.evidence_ids,
            "verification_commands": &record.verification_commands,
            "verify_command": verify_command,
            "source_artifact": gap_ledger_path,
            "authority_boundary": record.authority_boundary.as_str(),
        },
        "llm_guidance": {
            "prompt": repair_prompt(repair_route, &verify_command),
            "command": format!("ripr first-action --root {} --gap-ledger {}", display_path(root), gap_ledger_path),
            "verify_command": verify_command,
        },
    }))
}

fn gap_record_suppressed_json(record: &GapRecord, reason: &str, message: &str) -> Value {
    json!({
        "gap_id": gap_record_id(record),
        "file": record.anchor.as_ref().and_then(|anchor| anchor.file.clone()),
        "line": record.anchor.as_ref().and_then(|anchor| anchor.line),
        "reason": reason,
        "message": message,
    })
}

fn gap_record_cap_suppressed_json(record: &GapRecord) -> Value {
    json!({
        "gap_id": gap_record_id(record),
        "file": record.anchor.as_ref().and_then(|anchor| anchor.file.clone()),
        "line": record.anchor.as_ref().and_then(|anchor| anchor.line),
        "reason": "summary_cap",
        "message": "The PR guidance summary item cap was reached.",
    })
}

fn gap_record_id(record: &GapRecord) -> String {
    non_empty(&record.gap_id)
        .or_else(|| non_empty(&record.canonical_gap_id))
        .unwrap_or_else(|| "unknown-gap".to_string())
}

fn non_empty(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_string())
}

fn repair_text(route: &GapRepairRoute) -> String {
    route
        .assertion_shape
        .clone()
        .or_else(|| route.changed_behavior.clone())
        .unwrap_or_else(|| format!("Follow repair route `{}`.", route.route_kind))
}

fn repair_why(record: &GapRecord, route: &GapRepairRoute) -> String {
    if let Some(changed) = route.changed_behavior.as_deref() {
        return format!(
            "Changed behavior `{changed}` has a repairable {} gap.",
            record.kind
        );
    }
    if let Some(assertion) = route.assertion_shape.as_deref() {
        return format!(
            "This PR-local {} gap needs a focused repair: {assertion}.",
            record.kind
        );
    }
    format!(
        "This PR-local {} gap has a bounded repair route.",
        record.kind
    )
}

fn repair_intent(route_kind: &str) -> &'static str {
    match route_kind {
        "AddBoundaryAssertion" => "Add a boundary assertion.",
        "AddErrorAssertion" => "Add an error-path assertion.",
        "AddValueAssertion" => "Add an exact value assertion.",
        "AddSideEffectObserver" => "Add a side-effect observer.",
        "AddOutputGolden" => "Add or update output-contract golden evidence.",
        _ => "Add the focused proof named by the repair route.",
    }
}

fn repair_prompt(route: &GapRepairRoute, verify_command: &str) -> String {
    let repair = repair_text(route);
    format!(
        "{repair} Do not change production behavior unless the existing tests prove it is necessary. Verify with `{verify_command}`."
    )
}

fn review_recommendation_json(
    root: &Path,
    _mode: &Mode,
    config: &RiprConfig,
    selected: &AgentBriefSelectedSeam<'_>,
) -> Value {
    let entry = selected.seam;
    let seam = &entry.seam;
    let missing = agent_seam_packets::missing_discriminator_records_for(entry);
    let recommended = agent_seam_packets::recommended_test_for(entry);
    let nearest = agent_seam_packets::nearest_strong_test_to_imitate(&entry.evidence);
    let candidate_values = agent_seam_packets::candidate_values_for(entry, &missing);
    let assertion_shape =
        agent_seam_packets::assertion_shape_for(seam.kind(), seam.owner(), &entry.evidence);
    let seam_id = seam.id().as_str();
    let root_display = display_path(root);
    let missing_value = missing.first().map(|record| record.value.clone());

    json!({
        "id": format!("ripr-review-{seam_id}"),
        "seam_id": seam_id,
        "dedupe_key": format!("ripr:{seam_id}:{}:{}", display_path(seam.file()), seam.display_line()),
        "kind": seam.kind().as_str(),
        "grip_class": entry.class.as_str(),
        "severity": config.severity().for_seam(entry.class).as_str(),
        "owner": seam.owner(),
        "seam": {
            "file": display_path(seam.file()),
            "line": seam.display_line(),
            "expression": seam.expression(),
        },
        "reason": reason_for(selected, missing_value.as_deref()),
        "missing_discriminator": missing_value,
        "suggested_test": {
            "intent": suggested_test_intent(assertion_shape.kind),
            "candidate_values": candidate_values.iter().map(|record| record.value.clone()).collect::<Vec<_>>(),
            "assertion_shape": assertion_shape.example,
            "assertion_kind": assertion_shape.kind,
            "recommended_file": recommended.file,
            "recommended_name": recommended.name,
            "near_test": nearest.map(|test| test.test_name.clone()),
        },
        "llm_guidance": {
            "prompt": llm_prompt(&recommended.file, nearest.map(|test| test.test_name.as_str()), missing_value.as_deref()),
            "command": agent_brief_command(&root_display, seam_id, WORKFLOW_AGENT_BRIEF_ARTIFACT),
            "verify_command": agent_verify_command(
                &root_display,
                WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
                WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
                None,
            ),
        },
    })
}

fn placement_for(
    selected: &AgentBriefSelectedSeam<'_>,
    working_set: &AgentBriefResolvedWorkingSet,
) -> Option<ReviewPlacement> {
    let seam = &selected.seam.seam;
    let seam_file = normalize_path_text(seam.file());
    let production_lines = working_set
        .changed_lines
        .iter()
        .filter(|line| !is_test_like_path(&line.file))
        .collect::<Vec<_>>();

    if production_lines.iter().any(|line| {
        normalize_path_text(&line.file) == seam_file && line.line == seam.display_line()
    }) {
        return Some(ReviewPlacement {
            path: seam_file,
            line: seam.display_line(),
            mode: "exact_seam_line",
        });
    }

    let owner_line = working_set
        .changed_owners
        .iter()
        .filter(|owner| normalize_path_text(&owner.file) == seam_file)
        .filter(|owner| owner.owner == seam.owner())
        .filter(|owner| !is_test_like_path(&owner.file))
        .min_by(|left, right| nearest_line_ordering(left.line, right.line, seam.display_line()));
    if let Some(owner) = owner_line {
        return Some(ReviewPlacement {
            path: seam_file,
            line: owner.line,
            mode: "owner_function_changed_line",
        });
    }

    production_lines
        .iter()
        .filter(|line| normalize_path_text(&line.file) == seam_file)
        .min_by(|left, right| nearest_line_ordering(left.line, right.line, seam.display_line()))
        .map(|line| ReviewPlacement {
            path: seam_file,
            line: line.line,
            mode: "same_file_changed_line",
        })
}

fn placement_json(placement: &ReviewPlacement) -> Value {
    json!({
        "path": placement.path,
        "line": placement.line,
        "side": "RIGHT",
        "mode": placement.mode,
    })
}

fn suppressed_json(selected: &AgentBriefSelectedSeam<'_>, reason: &str, message: &str) -> Value {
    let seam = &selected.seam.seam;
    json!({
        "seam_id": seam.id().as_str(),
        "file": display_path(seam.file()),
        "line": seam.display_line(),
        "reason": reason,
        "message": message,
    })
}

fn changed_test_paths(working_set: &AgentBriefResolvedWorkingSet) -> Vec<String> {
    let mut paths = working_set
        .changed_lines
        .iter()
        .filter(|line| is_test_like_path(&line.file))
        .map(|line| normalize_path_text(&line.file))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn is_test_like_path(path: &Path) -> bool {
    let text = normalize_path_text(path);
    text.starts_with("tests/")
        || text.contains("/tests/")
        || text.ends_with("_test.rs")
        || text.ends_with("_tests.rs")
}

fn normalize_path_text(path: &Path) -> String {
    display_path(path)
        .trim_start_matches("./")
        .replace('\\', "/")
}

fn nearest_line_ordering(left: usize, right: usize, target: usize) -> Ordering {
    let left_distance = left.abs_diff(target);
    let right_distance = right.abs_diff(target);
    left_distance
        .cmp(&right_distance)
        .then_with(|| left.cmp(&right))
}

fn reason_for(selected: &AgentBriefSelectedSeam<'_>, missing: Option<&str>) -> String {
    if let Some(missing) = missing {
        return format!("Static evidence names missing discriminator `{missing}` for this seam.");
    }
    format!(
        "Static evidence class is {}; a focused test can strengthen the named seam.",
        selected.seam.class.as_str()
    )
}

fn suggested_test_intent(assertion_kind: &str) -> &'static str {
    match assertion_kind {
        "exact_error_variant" => "Add an exact error-variant test.",
        "side_effect_observer" => "Add a side-effect observer test.",
        "call_expectation" => "Add a call-observation test.",
        _ => "Add one focused discriminator test.",
    }
}

fn llm_prompt(recommended_file: &str, near_test: Option<&str>, missing: Option<&str>) -> String {
    let target = missing.unwrap_or("the missing discriminator named by the seam packet");
    let near = near_test
        .map(|test| format!(" near {test}"))
        .unwrap_or_default();
    format!(
        "Write one focused Rust test for {target}. Place it in {recommended_file}{near}. Do not change production code. Preserve existing fixture style. Verify with ripr agent verify."
    )
}

fn push_markdown_items(lines: &mut Vec<String>, heading: &str, value: Option<&Value>) {
    lines.push(format!("## {heading}"));
    lines.push(String::new());
    let items = value.and_then(Value::as_array);
    let Some(items) = items.filter(|items| !items.is_empty()) else {
        lines.push("- None.".to_string());
        lines.push(String::new());
        return;
    };

    for item in items {
        let seam_id = string_field(item, "seam_id").unwrap_or("unknown");
        let reason = string_field(item, "reason").unwrap_or("No reason available.");
        let command = item
            .get("llm_guidance")
            .and_then(|guidance| string_field(guidance, "command"))
            .unwrap_or("ripr agent brief --root . --seam-id <id> --json");
        lines.push(format!("- `{seam_id}`: {reason}"));
        lines.push(format!("  - command: `{command}`"));
    }
    lines.push(String::new());
}

fn push_suppressed_items(lines: &mut Vec<String>, value: Option<&Value>) {
    lines.push("## Suppressed".to_string());
    lines.push(String::new());
    let items = value.and_then(Value::as_array);
    let Some(items) = items.filter(|items| !items.is_empty()) else {
        lines.push("- None.".to_string());
        return;
    };
    for item in items {
        let seam_id = string_field(item, "seam_id").unwrap_or("unknown");
        let reason = string_field(item, "reason").unwrap_or("unknown");
        lines.push(format!("- `{seam_id}`: {reason}"));
    }
}

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ClassifiedSeam;
    use crate::analysis::seams::{
        ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind,
    };
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, RelationConfidence, RelationReason, TestGripEvidence,
    };
    use crate::app::agent_brief::{
        AgentBriefChangedOwner, AgentBriefLine, AgentBriefResolvedWorkingSet,
        AgentBriefSelectedSeam, AgentBriefSelection, AgentBriefWhyNow, AgentBriefWhyNowConfidence,
        AgentBriefWhyNowReason,
    };
    use crate::domain::{Confidence, OracleKind, OracleStrength, StageEvidence, StageState};
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, "test stage")
    }

    fn classified(line: usize) -> ClassifiedSeam {
        let seam = RepoSeam::new(
            "src/pricing.rs",
            "pricing::discounted_total",
            SeamKind::PredicateBoundary,
            line * 10,
            line,
            "amount >= discount_threshold",
            RequiredDiscriminator::BoundaryValue {
                description: "amount == discount_threshold".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        let seam_id = seam.id().clone();
        ClassifiedSeam {
            seam,
            class: SeamGripClass::WeaklyGripped,
            evidence: TestGripEvidence {
                seam_id,
                related_tests: vec![RelatedTestGrip {
                    test_name: "above_threshold_gets_discount".to_string(),
                    file: PathBuf::from("tests/pricing.rs"),
                    line: 12,
                    oracle_kind: OracleKind::ExactValue,
                    oracle_strength: OracleStrength::Strong,
                    evidence_summary: "exact returned value assertion".to_string(),
                    relation_reason: RelationReason::DirectOwnerCall,
                    relation_confidence: RelationConfidence::High,
                }],
                reach: stage(StageState::Yes),
                activate: stage(StageState::Yes),
                propagate: stage(StageState::Yes),
                observe: stage(StageState::Yes),
                discriminate: stage(StageState::Weak),
                observed_values: Vec::new(),
                missing_discriminators: Vec::new(),
            },
        }
    }

    fn selection<'a>(seams: &'a [ClassifiedSeam]) -> AgentBriefSelection<'a> {
        AgentBriefSelection {
            requested: 10,
            returned: seams.len(),
            default: 10,
            hard_cap: 10,
            top_seams: seams
                .iter()
                .map(|seam| AgentBriefSelectedSeam {
                    seam,
                    why_now: AgentBriefWhyNow {
                        reason: AgentBriefWhyNowReason::ChangedLineIntersectsSeam,
                        confidence: AgentBriefWhyNowConfidence::High,
                        evidence: "changed seam line".to_string(),
                    },
                })
                .collect(),
            warnings: Vec::new(),
        }
    }

    fn render_value(
        working_set: &AgentBriefResolvedWorkingSet,
        seams: &[ClassifiedSeam],
    ) -> Result<Value, String> {
        let rendered = render_review_comments_json(
            Path::new("."),
            "main",
            "HEAD",
            &Mode::Draft,
            &RiprConfig::default(),
            working_set,
            &selection(seams),
        )?;
        serde_json::from_str(&rendered).map_err(|err| format!("parse review comments JSON: {err}"))
    }

    fn render_value_with_selection(
        working_set: &AgentBriefResolvedWorkingSet,
        selection: &AgentBriefSelection<'_>,
    ) -> Result<Value, String> {
        let rendered = render_review_comments_json(
            Path::new("."),
            "main",
            "HEAD",
            &Mode::Draft,
            &RiprConfig::default(),
            working_set,
            selection,
        )?;
        serde_json::from_str(&rendered).map_err(|err| format!("parse review comments JSON: {err}"))
    }

    fn render_markdown(
        working_set: &AgentBriefResolvedWorkingSet,
        seams: &[ClassifiedSeam],
    ) -> String {
        render_review_comments_markdown(
            Path::new("."),
            "main",
            "HEAD",
            &Mode::Draft,
            &RiprConfig::default(),
            working_set,
            &selection(seams),
        )
    }

    fn render_markdown_with_selection(
        working_set: &AgentBriefResolvedWorkingSet,
        selection: &AgentBriefSelection<'_>,
    ) -> String {
        render_review_comments_markdown(
            Path::new("."),
            "main",
            "HEAD",
            &Mode::Draft,
            &RiprConfig::default(),
            working_set,
            selection,
        )
    }

    fn gap_record_review_comments_fixture() -> &'static str {
        r#"
{
  "records": [
    {
      "gap_id": "gap:pr:pricing:threshold-boundary",
      "canonical_gap_id": "gap:rust:pricing:discount:threshold-boundary",
      "kind": "MissingBoundaryAssertion",
      "language": "rust",
      "language_status": "stable",
      "scope": "pr_local",
      "evidence_class": "predicate_boundary",
      "gap_state": "actionable",
      "policy_state": "new",
      "repairability": "repairable",
      "evidence_ids": [
        "evidence:pricing-threshold-reached",
        "evidence:related-test-no-boundary-assertion"
      ],
      "anchor": {
        "file": "src/pricing.rs",
        "line": 42,
        "owner": "pricing::discount",
        "dedupe_fingerprint": "gap:rust:pricing:discount:threshold-boundary"
      },
      "repair_route": {
        "route_kind": "AddBoundaryAssertion",
        "target_file": "tests/pricing.rs",
        "related_test": "tests/pricing.rs::discount_above_threshold",
        "assertion_shape": "assert_eq!(discount(100, 100), 90)",
        "changed_behavior": "amount == discount_threshold"
      },
      "verification_commands": [
        "cargo xtask fixtures boundary_gap",
        "cargo xtask goldens check"
      ],
      "projection_eligibility": {
        "pr_comment": {
          "eligible": true,
          "reason": "stable_anchor_and_repair_route"
        }
      },
      "authority_boundary": "gate_decision_artifact_only"
    },
    {
      "gap_id": "gap:duplicate",
      "kind": "MissingBoundaryAssertion",
      "language": "rust",
      "language_status": "stable",
      "scope": "pr_local",
      "evidence_class": "predicate_boundary",
      "gap_state": "actionable",
      "policy_state": "new",
      "repairability": "repairable",
      "anchor": {
        "file": "src/pricing.rs",
        "line": 42,
        "dedupe_fingerprint": "gap:rust:pricing:discount:threshold-boundary"
      },
      "repair_route": {
        "route_kind": "AddBoundaryAssertion",
        "assertion_shape": "assert_eq!(discount(100, 100), 90)"
      },
      "verification_commands": [
        "cargo xtask fixtures boundary_gap"
      ],
      "projection_eligibility": {
        "pr_comment": {
          "eligible": true,
          "reason": "stable_anchor_and_repair_route"
        }
      }
    },
    {
      "gap_id": "gap:preview",
      "kind": "StaticLimitation",
      "language": "typescript",
      "language_status": "preview",
      "scope": "pr_local",
      "evidence_class": "static_unknown",
      "gap_state": "static_limit",
      "policy_state": "not_policy_targeted",
      "repairability": "analyzer_limitation",
      "projection_eligibility": {
        "pr_comment": {
          "eligible": false,
          "reason": "preview_static_limit_not_repair_card"
        }
      }
    }
  ]
}
"#
    }

    fn eligible_gap_record_json(gap_id: &str, dedupe: &str) -> Value {
        serde_json::json!({
            "gap_id": gap_id,
            "kind": "MissingBoundaryAssertion",
            "language": "rust",
            "language_status": "stable",
            "scope": "pr_local",
            "evidence_class": "predicate_boundary",
            "gap_state": "actionable",
            "policy_state": "new",
            "repairability": "repairable",
            "anchor": {
                "file": "src/pricing.rs",
                "line": 42,
                "dedupe_fingerprint": dedupe
            },
            "repair_route": {
                "route_kind": "AddBoundaryAssertion",
                "target_file": "tests/pricing.rs",
                "assertion_shape": "assert_eq!(discount(100, 100), 90)",
                "changed_behavior": "amount == threshold"
            },
            "verification_commands": [
                "cargo xtask fixtures boundary_gap"
            ],
            "projection_eligibility": {
                "pr_comment": {
                    "eligible": true,
                    "reason": "stable_anchor_and_repair_route"
                }
            }
        })
    }

    fn pr_guidance_fixture(case: &str, file: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/boundary_gap/expected/pr-guidance")
            .join(case)
            .join(file)
    }

    fn recommendation_calibration_fixture(file: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/boundary_gap/expected/recommendation-calibration")
            .join(file)
    }

    fn assert_json_fixture(case: &str, value: &Value) -> Result<(), String> {
        let rendered = format!(
            "{}\n",
            serde_json::to_string_pretty(value)
                .map_err(|err| format!("render {case} JSON fixture: {err}"))?
        );
        assert_text_fixture(case, "comments.json", &rendered)
    }

    fn assert_markdown_fixture(case: &str, rendered: &str) -> Result<(), String> {
        assert_text_fixture(case, "comments.md", &format!("{rendered}\n"))
    }

    fn assert_text_fixture(case: &str, file: &str, rendered: &str) -> Result<(), String> {
        let path = pr_guidance_fixture(case, file);
        let expected = fs::read_to_string(&path)
            .map_err(|err| format!("read fixture {}: {err}", path.display()))?;
        assert_eq!(
            expected, rendered,
            "PR guidance fixture drift for {case}/{file}"
        );
        Ok(())
    }

    #[test]
    fn review_comments_places_exact_changed_seam_line() -> Result<(), String> {
        let seams = [classified(88)];
        let working_set = AgentBriefResolvedWorkingSet::base(
            "main",
            vec![AgentBriefLine::new("src/pricing.rs", 88)],
        );

        let value = render_value(&working_set, &seams)?;
        assert_eq!(value["summary"]["comments"], 1);
        assert_eq!(value["comments"][0]["placement"]["mode"], "exact_seam_line");
        assert_eq!(
            value["comments"][0]["llm_guidance"]["verify_command"],
            "ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json"
        );
        Ok(())
    }

    #[test]
    fn review_comments_places_owner_function_changed_line() -> Result<(), String> {
        let seams = [classified(88)];
        let working_set = AgentBriefResolvedWorkingSet::base(
            "main",
            vec![AgentBriefLine::new("src/pricing.rs", 70)],
        )
        .with_changed_owners(vec![AgentBriefChangedOwner::new(
            "src/pricing.rs",
            70,
            "pricing::discounted_total",
        )]);

        let value = render_value(&working_set, &seams)?;
        assert_eq!(value["summary"]["comments"], 1);
        assert_eq!(
            value["comments"][0]["placement"]["mode"],
            "owner_function_changed_line"
        );
        assert_eq!(value["comments"][0]["placement"]["line"], 70);
        Ok(())
    }

    #[test]
    fn review_comments_places_nearest_same_file_changed_line() -> Result<(), String> {
        let seams = [classified(88)];
        let working_set = AgentBriefResolvedWorkingSet::base(
            "main",
            vec![
                AgentBriefLine::new("src/pricing.rs", 60),
                AgentBriefLine::new("src/pricing.rs", 92),
            ],
        );

        let value = render_value(&working_set, &seams)?;
        assert_eq!(value["summary"]["comments"], 1);
        assert_eq!(
            value["comments"][0]["placement"]["mode"],
            "same_file_changed_line"
        );
        assert_eq!(value["comments"][0]["placement"]["line"], 92);
        Ok(())
    }

    #[test]
    fn review_comments_caps_inline_and_summary_items() -> Result<(), String> {
        let seams = (1..=12)
            .map(|index| classified(index * 10))
            .collect::<Vec<_>>();
        let changed_lines = seams
            .iter()
            .map(|seam| AgentBriefLine::new("src/pricing.rs", seam.seam.display_line()))
            .collect::<Vec<_>>();
        let working_set = AgentBriefResolvedWorkingSet::base("main", changed_lines);

        let value = render_value(&working_set, &seams)?;
        assert_eq!(value["summary"]["comments"], 3);
        assert_eq!(value["summary"]["summary_only"], 7);
        assert_eq!(value["summary"]["suppressed"], 2);
        assert_eq!(
            value["summary_only"][0]["summary_reason"],
            "inline comment cap reached"
        );
        assert_eq!(value["suppressed"][0]["reason"], "summary_cap");
        Ok(())
    }

    #[test]
    fn review_comments_falls_back_to_summary_only_without_safe_line() -> Result<(), String> {
        let seams = [classified(88)];
        let working_set = AgentBriefResolvedWorkingSet::files(vec![PathBuf::from("src/other.rs")]);

        let value = render_value(&working_set, &seams)?;
        assert_eq!(value["summary"]["comments"], 0);
        assert_eq!(value["summary"]["summary_only"], 1);
        assert!(value["summary_only"][0]["placement"].is_null());
        Ok(())
    }

    #[test]
    fn review_comments_suppresses_repo_actionable_fallback() -> Result<(), String> {
        let seams = [classified(88)];
        let working_set = AgentBriefResolvedWorkingSet::files(vec![PathBuf::from("src/other.rs")]);
        let fallback_selection = AgentBriefSelection {
            requested: 10,
            returned: 1,
            default: 10,
            hard_cap: 10,
            top_seams: vec![AgentBriefSelectedSeam {
                seam: &seams[0],
                why_now: AgentBriefWhyNow {
                    reason: AgentBriefWhyNowReason::RepoActionableFallback,
                    confidence: AgentBriefWhyNowConfidence::Low,
                    evidence: "no working-set seam matched".to_string(),
                },
            }],
            warnings: Vec::new(),
        };

        let rendered = render_review_comments_json(
            Path::new("."),
            "main",
            "HEAD",
            &Mode::Draft,
            &RiprConfig::default(),
            &working_set,
            &fallback_selection,
        )?;
        let value: Value =
            serde_json::from_str(&rendered).map_err(|err| format!("parse JSON: {err}"))?;
        assert_eq!(value["summary"]["comments"], 0);
        assert_eq!(value["summary"]["summary_only"], 0);
        assert!(
            value["warnings"][0]
                .as_str()
                .is_some_and(|warning| warning.contains("fallback seams were suppressed"))
        );
        Ok(())
    }

    #[test]
    fn review_comments_suppresses_when_recommended_test_changed() -> Result<(), String> {
        let seams = [classified(88)];
        let working_set = AgentBriefResolvedWorkingSet::base(
            "main",
            vec![
                AgentBriefLine::new("src/pricing.rs", 88),
                AgentBriefLine::new("tests/pricing.rs", 12),
            ],
        );

        let value = render_value(&working_set, &seams)?;
        assert_eq!(value["summary"]["comments"], 0);
        assert_eq!(value["summary"]["suppressed"], 1);
        assert_eq!(value["suppressed"][0]["reason"], "nearby_test_changed");
        assert_eq!(value["summary"]["unchanged_tests"], false);
        let rendered = render_review_comments_markdown(
            Path::new("."),
            "main",
            "HEAD",
            &Mode::Draft,
            &RiprConfig::default(),
            &working_set,
            &selection(&seams),
        );
        assert!(rendered.contains("nearby_test_changed"));
        Ok(())
    }

    #[test]
    fn review_comments_markdown_names_static_boundaries() {
        let seams = [classified(88)];
        let working_set = AgentBriefResolvedWorkingSet::base(
            "main",
            vec![AgentBriefLine::new("src/pricing.rs", 88)],
        );
        let rendered = render_review_comments_markdown(
            Path::new("."),
            "main",
            "HEAD",
            &Mode::Draft,
            &RiprConfig::default(),
            &working_set,
            &selection(&seams),
        );

        assert!(rendered.contains("# RIPR PR Guidance"));
        assert!(rendered.contains("Advisory static evidence only"));
        assert!(rendered.contains("ripr agent brief"));
    }

    #[test]
    fn review_comments_gap_ledger_renders_only_eligible_repair_cards() -> Result<(), String> {
        let records = crate::output::gap_decision_ledger::parse_gap_records_json(
            gap_record_review_comments_fixture(),
        )?;

        let rendered = render_gap_record_review_comments_json(
            Path::new("."),
            "main",
            "HEAD",
            &Mode::Draft,
            "target/ripr/reports/gap-decision-ledger.json",
            &records,
        )?;
        let value: Value =
            serde_json::from_str(&rendered).map_err(|err| format!("parse JSON: {err}"))?;

        assert_eq!(value["summary"]["comments"], 1);
        assert_eq!(value["summary"]["suppressed"], 2);
        assert_eq!(value["comments"][0]["source"], "gap_decision_ledger");
        assert_eq!(
            value["comments"][0]["placement"]["mode"],
            "gap_record_anchor"
        );
        assert_eq!(
            value["comments"][0]["repair_card"]["verify_command"],
            "cargo xtask fixtures boundary_gap"
        );
        assert_eq!(
            value["comments"][0]["suggested_test"]["candidate_values"],
            Value::Array(Vec::new())
        );
        assert!(
            value["comments"][0].get("confidence").is_none(),
            "repair cards should not expose generic confidence optics"
        );
        assert_eq!(
            value["suppressed"][0]["reason"],
            "duplicate_dedupe_fingerprint"
        );
        assert_eq!(value["suppressed"][1]["reason"], "not_pr_comment_eligible");

        let markdown = render_gap_record_review_comments_markdown(
            Path::new("."),
            "main",
            "HEAD",
            &Mode::Draft,
            "target/ripr/reports/gap-decision-ledger.json",
            &records,
        );
        assert!(markdown.contains("gap:pr:pricing:threshold-boundary"));
        assert!(markdown.contains("command: `ripr first-action"));
        Ok(())
    }

    #[test]
    fn review_comments_gap_ledger_reports_ineligible_records_and_caps() -> Result<(), String> {
        let mut record_values = Vec::new();
        for index in 0..14 {
            record_values.push(eligible_gap_record_json(
                &format!("gap:eligible:{index}"),
                &format!("dedupe:{index}"),
            ));
        }

        let mut missing_projection =
            eligible_gap_record_json("gap:missing-projection", "dedupe:missing-projection");
        missing_projection
            .as_object_mut()
            .ok_or("missing_projection should be an object")?
            .remove("projection_eligibility");
        record_values.push(missing_projection);

        let mut repo_scope = eligible_gap_record_json("gap:repo-scope", "dedupe:repo-scope");
        repo_scope["scope"] = serde_json::json!("repo");
        record_values.push(repo_scope);

        let mut waived = eligible_gap_record_json("gap:waived", "dedupe:waived");
        waived["policy_state"] = serde_json::json!("waived");
        record_values.push(waived);

        let mut missing_anchor =
            eligible_gap_record_json("gap:missing-anchor", "dedupe:missing-anchor");
        missing_anchor
            .as_object_mut()
            .ok_or("missing_anchor should be an object")?
            .remove("anchor");
        record_values.push(missing_anchor);

        let mut missing_file = eligible_gap_record_json("gap:missing-file", "dedupe:missing-file");
        missing_file["anchor"]["file"] = serde_json::json!(" ");
        record_values.push(missing_file);

        let mut missing_line = eligible_gap_record_json("gap:missing-line", "dedupe:missing-line");
        missing_line["anchor"]
            .as_object_mut()
            .ok_or("missing_line anchor should be an object")?
            .remove("line");
        record_values.push(missing_line);

        let mut missing_dedupe =
            eligible_gap_record_json("gap:missing-dedupe", "dedupe:missing-dedupe");
        missing_dedupe["anchor"]
            .as_object_mut()
            .ok_or("missing_dedupe anchor should be an object")?
            .remove("dedupe_fingerprint");
        record_values.push(missing_dedupe);

        let mut missing_route =
            eligible_gap_record_json("gap:missing-route", "dedupe:missing-route");
        missing_route
            .as_object_mut()
            .ok_or("missing_route should be an object")?
            .remove("repair_route");
        record_values.push(missing_route);

        let mut missing_verify =
            eligible_gap_record_json("gap:missing-verify", "dedupe:missing-verify");
        missing_verify["verification_commands"] = serde_json::json!([]);
        record_values.push(missing_verify);

        let records_json = serde_json::json!({ "records": record_values }).to_string();
        let records = crate::output::gap_decision_ledger::parse_gap_records_json(&records_json)?;
        let rendered = render_gap_record_review_comments_json(
            Path::new("."),
            "main",
            "HEAD",
            &Mode::Draft,
            "target/ripr/reports/gap-decision-ledger.json",
            &records,
        )?;
        let value: Value =
            serde_json::from_str(&rendered).map_err(|err| format!("parse JSON: {err}"))?;

        assert_eq!(value["summary"]["comments"], 3);
        assert_eq!(value["summary"]["summary_only"], 10);
        let suppressed = value["suppressed"]
            .as_array()
            .ok_or("suppressed should be an array")?;
        let reasons: Vec<&str> = suppressed
            .iter()
            .filter_map(|item| item["reason"].as_str())
            .collect();
        for expected in [
            "summary_cap",
            "not_pr_comment_eligible",
            "not_pr_local_repairable",
            "policy_state_not_commentable",
            "missing_anchor",
            "missing_dedupe_fingerprint",
            "missing_repair_route",
            "missing_verification_command",
        ] {
            assert!(
                reasons.contains(&expected),
                "suppressed reasons should contain {expected}: {reasons:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn review_comments_gap_ledger_repair_helpers_cover_route_language() {
        let route = crate::output::gap_decision_ledger::GapRepairRoute {
            route_kind: "AddOutputGolden".to_string(),
            target_file: None,
            target_line: None,
            related_test: None,
            assertion_shape: None,
            changed_behavior: None,
            stop_conditions: Vec::new(),
        };
        assert_eq!(
            repair_text(&route),
            "Follow repair route `AddOutputGolden`."
        );
        assert_eq!(
            repair_intent("AddErrorAssertion"),
            "Add an error-path assertion."
        );
        assert_eq!(
            repair_intent("AddValueAssertion"),
            "Add an exact value assertion."
        );
        assert_eq!(
            repair_intent("AddSideEffectObserver"),
            "Add a side-effect observer."
        );
        assert_eq!(
            repair_intent("AddOutputGolden"),
            "Add or update output-contract golden evidence."
        );
        assert_eq!(
            repair_intent("UnsupportedRoute"),
            "Add the focused proof named by the repair route."
        );
        assert!(
            repair_prompt(&route, "cargo xtask goldens check")
                .contains("cargo xtask goldens check")
        );
    }

    #[test]
    fn review_comments_pr_guidance_fixtures_pin_required_cases() -> Result<(), String> {
        let exact_seams = [classified(88)];
        let exact = AgentBriefResolvedWorkingSet::base(
            "main",
            vec![AgentBriefLine::new("src/pricing.rs", 88)],
        );
        assert_json_fixture("exact-line", &render_value(&exact, &exact_seams)?)?;
        assert_markdown_fixture("exact-line", &render_markdown(&exact, &exact_seams))?;

        let owner = AgentBriefResolvedWorkingSet::base(
            "main",
            vec![AgentBriefLine::new("src/pricing.rs", 70)],
        )
        .with_changed_owners(vec![AgentBriefChangedOwner::new(
            "src/pricing.rs",
            70,
            "pricing::discounted_total",
        )]);
        assert_json_fixture("owner-function-line", &render_value(&owner, &exact_seams)?)?;
        assert_markdown_fixture(
            "owner-function-line",
            &render_markdown(&owner, &exact_seams),
        )?;

        let same_file = AgentBriefResolvedWorkingSet::base(
            "main",
            vec![
                AgentBriefLine::new("src/pricing.rs", 60),
                AgentBriefLine::new("src/pricing.rs", 92),
            ],
        );
        assert_json_fixture("same-file-line", &render_value(&same_file, &exact_seams)?)?;
        assert_markdown_fixture("same-file-line", &render_markdown(&same_file, &exact_seams))?;

        let summary_only = AgentBriefResolvedWorkingSet::files(vec![PathBuf::from("src/other.rs")]);
        assert_json_fixture("summary-only", &render_value(&summary_only, &exact_seams)?)?;
        assert_markdown_fixture(
            "summary-only",
            &render_markdown(&summary_only, &exact_seams),
        )?;

        let capped_seams = (1..=12)
            .map(|index| classified(index * 10))
            .collect::<Vec<_>>();
        let capped_lines = capped_seams
            .iter()
            .map(|seam| AgentBriefLine::new("src/pricing.rs", seam.seam.display_line()))
            .collect::<Vec<_>>();
        let capped = AgentBriefResolvedWorkingSet::base("main", capped_lines);
        assert_json_fixture("capped", &render_value(&capped, &capped_seams)?)?;
        assert_markdown_fixture("capped", &render_markdown(&capped, &capped_seams))?;

        let changed_test = AgentBriefResolvedWorkingSet::base(
            "main",
            vec![
                AgentBriefLine::new("src/pricing.rs", 88),
                AgentBriefLine::new("tests/pricing.rs", 12),
            ],
        );
        assert_json_fixture(
            "changed-test-skip",
            &render_value(&changed_test, &exact_seams)?,
        )?;
        assert_markdown_fixture(
            "changed-test-skip",
            &render_markdown(&changed_test, &exact_seams),
        )?;

        let configured_off_selection = AgentBriefSelection {
            requested: 10,
            returned: 0,
            default: 10,
            hard_cap: 10,
            top_seams: Vec::new(),
            warnings: vec![format!(
                "seam {} at src/pricing.rs:88 is configured off for weakly_gripped seams and is not included in agent brief results",
                exact_seams[0].seam.id().as_str()
            )],
        };
        assert_json_fixture(
            "configured-off",
            &render_value_with_selection(&exact, &configured_off_selection)?,
        )?;
        assert_markdown_fixture(
            "configured-off",
            &render_markdown_with_selection(&exact, &configured_off_selection),
        )?;

        Ok(())
    }

    #[test]
    fn recommendation_calibration_fixture_expectations_pin_required_cases() -> Result<(), String> {
        let expectations_path = recommendation_calibration_fixture("expectations.json");
        let expectations_text = fs::read_to_string(&expectations_path)
            .map_err(|err| format!("read {}: {err}", expectations_path.display()))?;
        let expectations: Value = serde_json::from_str(&expectations_text)
            .map_err(|err| format!("parse {}: {err}", expectations_path.display()))?;

        assert_eq!(expectations["schema_version"], "0.1");
        assert_eq!(expectations["status"], "advisory");
        assert_eq!(expectations["spec"], "RIPR-SPEC-0013");

        let cases = expectations["cases"]
            .as_array()
            .ok_or("recommendation calibration expectations cases should be an array")?;
        assert_eq!(cases.len(), 10);

        let mut seen = std::collections::BTreeSet::new();
        for case in cases {
            let id = case["id"]
                .as_str()
                .ok_or("recommendation calibration case should have id")?;
            let scenario = case["scenario"]
                .as_str()
                .ok_or("recommendation calibration case should have scenario")?;
            let outcome = case["expected"]["outcome"]
                .as_str()
                .ok_or("recommendation calibration case should have expected outcome")?;
            let placement_quality = case["expected"]["placement_quality"]
                .as_str()
                .ok_or("recommendation calibration case should have placement quality")?;
            seen.insert((id.to_string(), scenario.to_string()));

            assert!(
                [
                    "useful",
                    "noisy",
                    "wrong_line",
                    "already_covered",
                    "wrong_target",
                    "summary_only_correct",
                    "suppressed_correctly",
                    "unknown",
                ]
                .contains(&outcome),
                "{} uses unsupported outcome {}",
                id,
                outcome
            );
            assert!(
                [
                    "correct",
                    "wrong_line",
                    "summary_only_expected",
                    "not_placeable",
                    "unknown",
                ]
                .contains(&placement_quality),
                "{} uses unsupported placement quality {}",
                id,
                placement_quality
            );
            assert_recommendation_calibration_source_exists(case)?;
        }

        for (id, scenario) in [
            ("useful_exact_line_boundary", "useful recommendation"),
            ("noisy_owner_fallback", "noisy recommendation"),
            ("wrong_line_same_file_fallback", "wrong-line placement"),
            ("already_covered_visible", "already-covered seam"),
            (
                "summary_only_expected_boundary",
                "correct summary-only fallback",
            ),
            ("suppression_configured_off", "suppression correctness"),
            (
                "generated_migration_exclusion",
                "generated/migration exclusion",
            ),
            ("macro_heavy_summary_only", "macro-heavy code"),
            ("trait_generic_wrong_target", "trait/generic boundary"),
            ("async_error_boundary_useful", "async/error boundary"),
        ] {
            assert!(
                seen.contains(&(id.to_string(), scenario.to_string())),
                "missing recommendation calibration case {} ({})",
                id,
                scenario
            );
        }

        Ok(())
    }

    #[test]
    fn recommendation_calibration_outcome_receipts_pin_required_labels() -> Result<(), String> {
        let receipt_cases = [
            ("useful.json", "useful"),
            ("noisy.json", "noisy"),
            ("wrong-line.json", "wrong_line"),
            ("already-covered.json", "already_covered"),
            ("wrong-target.json", "wrong_target"),
            ("summary-only-correct.json", "summary_only_correct"),
            ("suppressed-correctly.json", "suppressed_correctly"),
        ];

        let mut seen = std::collections::BTreeSet::new();
        for (file, expected_label) in receipt_cases {
            let relative_path = format!("outcome-receipts/{file}");
            let receipt_path = recommendation_calibration_fixture(&relative_path);
            let receipt_text = fs::read_to_string(&receipt_path)
                .map_err(|err| format!("read {}: {err}", receipt_path.display()))?;
            let receipt: Value = serde_json::from_str(&receipt_text)
                .map_err(|err| format!("parse {}: {err}", receipt_path.display()))?;

            assert_eq!(receipt["schema_version"], "0.1");
            assert_eq!(receipt["tool"], "ripr");
            assert_eq!(receipt["kind"], "review_guidance_outcome_receipt");
            assert_eq!(receipt["status"], "advisory");
            assert_eq!(receipt["spec"], "RIPR-SPEC-0013");

            let label = receipt["outcome"]["label"]
                .as_str()
                .ok_or("outcome receipt should have outcome label")?;
            assert_eq!(label, expected_label);
            seen.insert(label.to_string());

            let seam_id = receipt["guidance"]["seam_id"]
                .as_str()
                .ok_or("outcome receipt should name a guidance seam id")?;
            assert!(
                !seam_id.is_empty(),
                "{file} should name the source guidance seam"
            );
            let source = receipt["outcome"]["source"]
                .as_str()
                .ok_or("outcome receipt should name a local outcome source")?;
            assert!(
                ["fixture", "reviewer", "agent", "ci_artifact", "unknown"].contains(&source),
                "{file} uses unsupported outcome source {source}"
            );
            let placement_quality = receipt["placement"]["quality"]
                .as_str()
                .ok_or("outcome receipt should have placement quality")?;
            assert!(
                [
                    "correct",
                    "wrong_line",
                    "summary_only_expected",
                    "not_placeable",
                    "unknown",
                ]
                .contains(&placement_quality),
                "{file} uses unsupported placement quality {placement_quality}"
            );
            let target_quality = receipt["suggested_test"]["target_quality"]
                .as_str()
                .ok_or("outcome receipt should have suggested-test target quality")?;
            assert!(
                ["correct", "wrong_target", "not_applicable", "unknown"].contains(&target_quality),
                "{file} uses unsupported suggested-test target quality {target_quality}"
            );
            for key in [
                "telemetry",
                "external_service",
                "source_edits",
                "generated_tests",
                "runtime_mutation_execution",
                "ci_blocking",
            ] {
                assert_eq!(
                    receipt["limits"][key], false,
                    "{file} should keep {key} disabled"
                );
            }
        }

        for required in [
            "useful",
            "noisy",
            "wrong_line",
            "already_covered",
            "wrong_target",
            "summary_only_correct",
            "suppressed_correctly",
        ] {
            assert!(
                seen.contains(required),
                "missing recommendation calibration outcome receipt for {required}"
            );
        }

        Ok(())
    }

    fn assert_recommendation_calibration_source_exists(case: &Value) -> Result<(), String> {
        let artifact = case["source_artifact"]
            .as_str()
            .ok_or("recommendation calibration case should have source_artifact")?;
        let collection = case["source_collection"]
            .as_str()
            .ok_or("recommendation calibration case should have source_collection")?;
        let item_id = case["source_item_id"].as_str();
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(artifact);
        let text =
            fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|err| format!("parse source artifact {}: {err}", path.display()))?;

        match collection {
            "comments" | "summary_only" | "suppressed" => {
                let entries = value[collection]
                    .as_array()
                    .ok_or_else(|| format!("{artifact} should contain array {collection}"))?;
                let Some(item_id) = item_id else {
                    return Err(format!(
                        "{artifact} {collection} expectation is missing item id"
                    ));
                };
                assert!(
                    entries.iter().any(|entry| entry["id"] == item_id
                        || entry["seam_id"] == case["expected"]["seam_id"]),
                    "{} {} should contain {}",
                    artifact,
                    collection,
                    item_id
                );
            }
            "warnings" => {
                let warnings = value["warnings"]
                    .as_array()
                    .ok_or_else(|| format!("{artifact} should contain warnings array"))?;
                let seam_id = case["expected"]["seam_id"]
                    .as_str()
                    .ok_or("warning expectation should have seam_id")?;
                assert!(
                    warnings
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|warning| warning.contains(seam_id)),
                    "{} warnings should mention {}",
                    artifact,
                    seam_id
                );
            }
            other => return Err(format!("unsupported source_collection {other}")),
        }

        Ok(())
    }
}
