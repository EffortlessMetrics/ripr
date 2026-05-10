use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "pr_inline_comment_publish_plan";
const STATUS: &str = "advisory";
const DEFAULT_GENERATED_AT: &str = "unknown";
pub(crate) const DEFAULT_MAX_INLINE_COMMENTS: usize = 3;
const LIMITS_NOTE: &str = "Advisory inline-comment publish plan only; default workflows do not post comments, summary-only guidance is never published inline, and gate decisions remain separate.";

pub(crate) const DEFAULT_COMMENT_PUBLISH_PLAN_OUT: &str =
    "target/ripr/review/comment-publish-plan.json";
pub(crate) const DEFAULT_COMMENT_PUBLISH_PLAN_MD_OUT: &str =
    "target/ripr/review/comment-publish-plan.md";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommentMode {
    Off,
    Plan,
    Inline,
}

impl CommentMode {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "off" => Ok(Self::Off),
            "plan" => Ok(Self::Plan),
            "inline" => Ok(Self::Inline),
            _ => Err(format!(
                "unknown pr-comments plan mode {value:?}; expected `off`, `plan`, or `inline`"
            )),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Plan => "plan",
            Self::Inline => "inline",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommentPermissionContext {
    pub(crate) pull_request: Option<u64>,
    pub(crate) event_name: Option<String>,
    pub(crate) head_repo: Option<String>,
    pub(crate) base_repo: Option<String>,
    pub(crate) token_available: bool,
    pub(crate) write_permission: bool,
}

impl Default for CommentPermissionContext {
    fn default() -> Self {
        Self {
            pull_request: None,
            event_name: None,
            head_repo: None,
            base_repo: None,
            token_available: false,
            write_permission: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommentPublishPlanInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) mode: CommentMode,
    pub(crate) max_inline_comments: usize,
    pub(crate) pr_guidance_path: Option<String>,
    pub(crate) pr_guidance_json: Option<Result<String, String>>,
    pub(crate) existing_comments_path: Option<String>,
    pub(crate) existing_comments_json: Option<Result<String, String>>,
    pub(crate) permission: CommentPermissionContext,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommentPublishPlanReport {
    status: String,
    root: String,
    generated_at: String,
    mode: CommentMode,
    inputs: PlanInputs,
    limits: PlanLimits,
    summary: PlanSummary,
    operations: Vec<PlanOperation>,
    skipped: Vec<PlanSkipped>,
    blocked: Vec<PlanBlocked>,
    warnings: Vec<PlanWarning>,
    limits_note: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PlanInputs {
    pr_guidance: Option<String>,
    existing_comments: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pull_request: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    head_repo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_repo: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PlanLimits {
    max_inline_comments: usize,
    advisory: bool,
    comments_default: &'static str,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct PlanSummary {
    guidance_comments: usize,
    summary_only: usize,
    suppressed: usize,
    publishable: usize,
    planned_create: usize,
    planned_update: usize,
    planned_keep: usize,
    planned_delete: usize,
    skipped: usize,
    blocked: usize,
    safe_to_publish: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PlanOperation {
    operation: String,
    safe_to_publish: bool,
    dry_run: bool,
    source_collection: String,
    source_id: String,
    dedupe_key: String,
    placement: PlanPlacement,
    body: Option<String>,
    existing_comment_id: Option<u64>,
    skip_reason: Option<String>,
    blocked_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PlanPlacement {
    path: String,
    line: u64,
    side: String,
    mode: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PlanSkipped {
    source_collection: String,
    source_id: Option<String>,
    dedupe_key: Option<String>,
    skip_reason: String,
    message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PlanBlocked {
    source_collection: String,
    source_id: Option<String>,
    dedupe_key: Option<String>,
    blocked_reason: String,
    message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PlanWarning {
    kind: String,
    message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExistingComment {
    comment_id: u64,
    dedupe_key: String,
    placement: PlanPlacement,
    body_hash: Option<String>,
    body: Option<String>,
    outdated: bool,
}

pub(crate) fn build_comment_publish_plan_report(
    input: CommentPublishPlanInput,
) -> CommentPublishPlanReport {
    let generated_at = if input.generated_at.trim().is_empty() {
        DEFAULT_GENERATED_AT.to_string()
    } else {
        input.generated_at.clone()
    };
    let limits = PlanLimits {
        max_inline_comments: input.max_inline_comments,
        advisory: true,
        comments_default: "off",
    };
    let inputs = PlanInputs {
        pr_guidance: input.pr_guidance_path.clone(),
        existing_comments: input.existing_comments_path.clone(),
        pull_request: input.permission.pull_request,
        event_name: input.permission.event_name.clone(),
        head_repo: input.permission.head_repo.clone(),
        base_repo: input.permission.base_repo.clone(),
    };
    let mut warnings = Vec::new();
    let existing = parse_existing_comments(&input, &mut warnings);

    let mut report = CommentPublishPlanReport {
        status: STATUS.to_string(),
        root: input.root.clone(),
        generated_at,
        mode: input.mode,
        inputs,
        limits,
        summary: PlanSummary::default(),
        operations: Vec::new(),
        skipped: Vec::new(),
        blocked: Vec::new(),
        warnings,
        limits_note: LIMITS_NOTE.to_string(),
    };

    if input.mode == CommentMode::Off {
        report.skipped.push(PlanSkipped {
            source_collection: "mode".to_string(),
            source_id: None,
            dedupe_key: None,
            skip_reason: "mode_off".to_string(),
            message: "Inline comment planning is disabled by default.".to_string(),
        });
        report.summary.skipped = report.skipped.len();
        return report;
    }

    let Some(guidance) = parse_pr_guidance(&input, &mut report) else {
        report.summary.blocked = report.blocked.len();
        return report;
    };

    let comments = array_items(&guidance, "comments");
    let summary_only = array_items(&guidance, "summary_only");
    let suppressed = array_items(&guidance, "suppressed");
    report.summary.guidance_comments = comments.len();
    report.summary.summary_only = summary_only.len();
    report.summary.suppressed = suppressed.len();

    let mut matched_existing = BTreeSet::new();
    for item in comments {
        add_comment_item_plan(&input, item, &existing, &mut matched_existing, &mut report);
    }
    for item in summary_only {
        report.skipped.push(PlanSkipped {
            source_collection: "summary_only".to_string(),
            source_id: string_field(item, "id"),
            dedupe_key: string_field(item, "dedupe_key"),
            skip_reason: "summary_only".to_string(),
            message: "Summary-only guidance is visible in comments.md but is not eligible for inline publishing."
                .to_string(),
        });
    }
    for item in suppressed {
        report.skipped.push(PlanSkipped {
            source_collection: "suppressed".to_string(),
            source_id: string_field(item, "id").or_else(|| string_field(item, "seam_id")),
            dedupe_key: string_field(item, "dedupe_key"),
            skip_reason: "suppressed".to_string(),
            message: "Suppressed guidance is visible in comments.md but is not eligible for inline publishing."
                .to_string(),
        });
    }
    for existing_comment in existing
        .values()
        .filter(|comment| !matched_existing.contains(&comment.dedupe_key))
    {
        report.operations.push(PlanOperation {
            operation: "delete".to_string(),
            safe_to_publish: false,
            dry_run: true,
            source_collection: "existing_comments".to_string(),
            source_id: existing_comment.comment_id.to_string(),
            dedupe_key: existing_comment.dedupe_key.clone(),
            placement: PlanPlacement {
                mode: "stale_existing".to_string(),
                ..existing_comment.placement.clone()
            },
            body: None,
            existing_comment_id: Some(existing_comment.comment_id),
            skip_reason: None,
            blocked_reason: None,
        });
        report.warnings.push(PlanWarning {
            kind: "stale_existing_comment".to_string(),
            message: "Existing RIPR comment no longer has a matching publishable recommendation."
                .to_string(),
        });
    }

    summarize(&mut report);
    report
}

pub(crate) fn render_comment_publish_plan_json(
    report: &CommentPublishPlanReport,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct JsonReport<'a> {
        schema_version: &'static str,
        tool: &'static str,
        kind: &'static str,
        status: &'a str,
        root: &'a str,
        generated_at: &'a str,
        mode: &'static str,
        inputs: &'a PlanInputs,
        limits: &'a PlanLimits,
        summary: &'a PlanSummary,
        operations: &'a [PlanOperation],
        skipped: &'a [PlanSkipped],
        blocked: &'a [PlanBlocked],
        warnings: &'a [PlanWarning],
        limits_note: &'a str,
    }

    serde_json::to_string_pretty(&JsonReport {
        schema_version: SCHEMA_VERSION,
        tool: "ripr",
        kind: REPORT_KIND,
        status: &report.status,
        root: &report.root,
        generated_at: &report.generated_at,
        mode: report.mode.as_str(),
        inputs: &report.inputs,
        limits: &report.limits,
        summary: &report.summary,
        operations: &report.operations,
        skipped: &report.skipped,
        blocked: &report.blocked,
        warnings: &report.warnings,
        limits_note: &report.limits_note,
    })
    .map_err(|err| format!("render PR inline comment publish-plan JSON failed: {err}"))
}

pub(crate) fn render_comment_publish_plan_markdown(report: &CommentPublishPlanReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Inline Comment Publish Plan\n\n");
    out.push_str(&format!("Mode: {}\n", report.mode.as_str()));
    out.push_str(&format!("Status: {}\n\n", report.status));

    if !report.blocked.is_empty() {
        out.push_str("Blocked:\n");
        for (reason, count) in reason_counts(
            report
                .blocked
                .iter()
                .map(|blocked| blocked.blocked_reason.as_str()),
        ) {
            let message = blocked_message_for_markdown(&reason, &report.blocked);
            if count == 1 {
                out.push_str(&format!("- {reason}: {message}\n"));
            } else {
                out.push_str(&format!("- {reason}: {count} items blocked ({message})\n"));
            }
        }
        out.push_str("\nNext:\n");
        if report
            .blocked
            .iter()
            .any(|blocked| blocked.blocked_reason == "missing_pr_guidance")
        {
            out.push_str("- run `ripr review-comments` before planning inline comments.\n\n");
            out.push_str("Limits:\n");
            out.push_str("- Advisory inline-comment publish plan only.\n");
            out.push_str("- Missing input is visible and does not change gate authority.\n");
        } else {
            out.push_str(
                "- keep job summary and check annotations as the PR surface, or configure safe\n",
            );
            out.push_str("  pull-request write permissions before enabling inline mode.\n\n");
            out.push_str("Limits:\n");
            out.push_str("- Advisory inline-comment publish plan only.\n");
            out.push_str("- Default workflows do not post comments.\n");
        }
        return out;
    }

    out.push_str("Summary:\n");
    out.push_str(&format!(
        "- publishable comments: {}\n",
        report.summary.publishable
    ));
    out.push_str(&format!("- skipped: {}\n", report.summary.skipped));
    out.push_str(&format!("- blocked: {}\n", report.summary.blocked));
    out.push_str("- default: inline comments are off\n\n");

    if !report.operations.is_empty() {
        out.push_str("Planned operations:\n");
        for operation in &report.operations {
            if operation.operation == "delete" {
                out.push_str(&format!(
                    "- delete stale existing RIPR comment `{}`\n",
                    operation.dedupe_key
                ));
                continue;
            }
            out.push_str(&format!(
                "- {} {}:{} `{}`\n",
                operation.operation,
                operation.placement.path,
                operation.placement.line,
                operation.dedupe_key
            ));
            if let Some(missing) = missing_discriminator_from_body(operation.body.as_deref()) {
                out.push_str(&format!("  - missing discriminator: `{missing}`\n"));
                out.push_str("  - action: add one focused boundary assertion\n");
            }
        }
        out.push('\n');
    }

    if !report.skipped.is_empty() {
        out.push_str("Skipped:\n");
        for (reason, count) in reason_counts(
            report
                .skipped
                .iter()
                .map(|skipped| skipped.skip_reason.as_str()),
        ) {
            out.push_str(&format!(
                "- {reason}: {}\n",
                skipped_summary(&reason, count)
            ));
        }
        out.push('\n');
    }

    out.push_str("Limits:\n");
    out.push_str("- Advisory inline-comment publish plan only.\n");
    if report
        .operations
        .iter()
        .any(|operation| operation.operation == "delete")
    {
        out.push_str("- Stale cleanup is explicit and reviewable.\n");
    } else if report
        .operations
        .iter()
        .any(|operation| operation.operation == "update" || operation.operation == "keep")
    {
        out.push_str("- Dedupe keys prevent duplicate RIPR comments.\n");
    } else if report
        .skipped
        .iter()
        .any(|skipped| skipped.skip_reason == "cap_reached")
    {
        out.push_str("- Never publishes summary-only guidance inline.\n");
    } else {
        out.push_str("- Does not post comments unless explicit inline mode is configured.\n");
        out.push_str("- Never publishes summary-only guidance inline.\n");
        out.push_str("- Gate decision remains separate pass/fail authority.\n");
    }
    out
}

pub(crate) fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn parse_pr_guidance(
    input: &CommentPublishPlanInput,
    report: &mut CommentPublishPlanReport,
) -> Option<Value> {
    let Some(path) = input.pr_guidance_path.as_deref() else {
        report.blocked.push(PlanBlocked {
            source_collection: "pr_guidance".to_string(),
            source_id: None,
            dedupe_key: None,
            blocked_reason: "missing_pr_guidance".to_string(),
            message: "No review-comments JSON was supplied.".to_string(),
        });
        return None;
    };
    let Some(text) = &input.pr_guidance_json else {
        report.blocked.push(PlanBlocked {
            source_collection: "pr_guidance".to_string(),
            source_id: None,
            dedupe_key: None,
            blocked_reason: "missing_pr_guidance".to_string(),
            message: format!("No review-comments JSON was loaded from {path}."),
        });
        return None;
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            report.blocked.push(PlanBlocked {
                source_collection: "pr_guidance".to_string(),
                source_id: None,
                dedupe_key: None,
                blocked_reason: "missing_pr_guidance".to_string(),
                message: format!("Could not read review-comments JSON {path}: {error}."),
            });
            return None;
        }
    };
    match serde_json::from_str::<Value>(text) {
        Ok(value) => Some(value),
        Err(error) => {
            report.blocked.push(PlanBlocked {
                source_collection: "pr_guidance".to_string(),
                source_id: None,
                dedupe_key: None,
                blocked_reason: "malformed_pr_guidance".to_string(),
                message: format!("Review-comments JSON {path} is malformed: {error}."),
            });
            None
        }
    }
}

fn parse_existing_comments(
    input: &CommentPublishPlanInput,
    warnings: &mut Vec<PlanWarning>,
) -> BTreeMap<String, ExistingComment> {
    let mut existing = BTreeMap::new();
    let Some(path) = input.existing_comments_path.as_deref() else {
        return existing;
    };
    let Some(text) = &input.existing_comments_json else {
        warnings.push(PlanWarning {
            kind: "existing_comments_unavailable".to_string(),
            message: format!("Existing-comment metadata {path} was supplied but not loaded."),
        });
        return existing;
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            warnings.push(PlanWarning {
                kind: "existing_comments_unavailable".to_string(),
                message: format!("Existing-comment metadata {path} could not be read: {error}."),
            });
            return existing;
        }
    };
    let value = match serde_json::from_str::<Value>(text) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(PlanWarning {
                kind: "existing_comments_malformed".to_string(),
                message: format!("Existing-comment metadata {path} is malformed: {error}."),
            });
            return existing;
        }
    };
    for item in array_items(&value, "comments") {
        let Some(dedupe_key) = string_field(item, "dedupe_key") else {
            continue;
        };
        let Some(comment_id) = item.get("comment_id").and_then(Value::as_u64) else {
            continue;
        };
        let placement = PlanPlacement {
            path: string_field(item, "path").unwrap_or_else(|| "unknown".to_string()),
            line: item.get("line").and_then(Value::as_u64).unwrap_or(0),
            side: string_field(item, "side").unwrap_or_else(|| "RIGHT".to_string()),
            mode: "existing_comment".to_string(),
        };
        existing.insert(
            dedupe_key.clone(),
            ExistingComment {
                comment_id,
                dedupe_key,
                placement,
                body_hash: string_field(item, "body_hash"),
                body: string_field(item, "body"),
                outdated: item
                    .get("outdated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            },
        );
    }
    existing
}

fn add_comment_item_plan(
    input: &CommentPublishPlanInput,
    item: &Value,
    existing: &BTreeMap<String, ExistingComment>,
    matched_existing: &mut BTreeSet<String>,
    report: &mut CommentPublishPlanReport,
) {
    let source_id = string_field(item, "id");
    let dedupe_key = string_field(item, "dedupe_key");
    let Some(dedupe_key) = dedupe_key else {
        report.blocked.push(PlanBlocked {
            source_collection: "comments".to_string(),
            source_id,
            dedupe_key: None,
            blocked_reason: "missing_dedupe_key".to_string(),
            message: "Changed-line guidance is missing a dedupe_key.".to_string(),
        });
        return;
    };
    let Some(placement) = placement_from_comment(item) else {
        report.blocked.push(PlanBlocked {
            source_collection: "comments".to_string(),
            source_id,
            dedupe_key: Some(dedupe_key),
            blocked_reason: "missing_changed_line_placement".to_string(),
            message: "Changed-line guidance is missing safe placement.".to_string(),
        });
        return;
    };
    let blockers = permission_blockers(input);
    if input.mode == CommentMode::Inline && !blockers.is_empty() {
        for (reason, message) in blockers {
            report.blocked.push(PlanBlocked {
                source_collection: "comments".to_string(),
                source_id: source_id.clone(),
                dedupe_key: Some(dedupe_key.clone()),
                blocked_reason: reason,
                message,
            });
        }
        return;
    }
    if report.summary.publishable >= input.max_inline_comments {
        report.skipped.push(PlanSkipped {
            source_collection: "comments".to_string(),
            source_id,
            dedupe_key: Some(dedupe_key),
            skip_reason: "cap_reached".to_string(),
            message: "Default inline comment cap reached.".to_string(),
        });
        return;
    }

    let body = comment_body(item);
    let existing_comment = existing.get(&dedupe_key);
    if existing_comment.is_some() {
        matched_existing.insert(dedupe_key.clone());
    }
    let operation = match existing_comment {
        Some(existing_comment) if existing_comment_matches_body(existing_comment, &body) => "keep",
        Some(_) => "update",
        None => "create",
    };
    let safe_to_publish = input.mode == CommentMode::Inline;
    report.summary.publishable += 1;
    report.operations.push(PlanOperation {
        operation: operation.to_string(),
        safe_to_publish,
        dry_run: true,
        source_collection: "comments".to_string(),
        source_id: source_id.unwrap_or_else(|| dedupe_key.clone()),
        dedupe_key,
        placement,
        body: Some(body),
        existing_comment_id: existing_comment.map(|comment| comment.comment_id),
        skip_reason: None,
        blocked_reason: None,
    });
}

fn permission_blockers(input: &CommentPublishPlanInput) -> Vec<(String, String)> {
    let mut blockers = Vec::new();
    match input.permission.event_name.as_deref() {
        Some("pull_request") => {}
        Some(_) => blockers.push((
            "unsafe_event".to_string(),
            "Inline comments are disabled for this event context.".to_string(),
        )),
        None => blockers.push((
            "unsafe_event".to_string(),
            "Inline comments require an explicit pull_request event context.".to_string(),
        )),
    }
    if input.permission.pull_request.is_none() {
        blockers.push((
            "missing_pull_request".to_string(),
            "Inline comments require a pull request number.".to_string(),
        ));
    }
    if let (Some(head), Some(base)) = (
        input.permission.head_repo.as_deref(),
        input.permission.base_repo.as_deref(),
    ) && head != base
    {
        blockers.push((
            "fork_untrusted".to_string(),
            "Inline comments are disabled for untrusted fork pull requests.".to_string(),
        ));
    }
    if !input.permission.token_available {
        blockers.push((
            "missing_token".to_string(),
            "No pull-request write token is available.".to_string(),
        ));
    } else if !input.permission.write_permission {
        blockers.push((
            "missing_write_permission".to_string(),
            "Workflow token cannot write PR comments.".to_string(),
        ));
    }
    blockers
}

fn summarize(report: &mut CommentPublishPlanReport) {
    report.summary.planned_create = count_operations(report, "create");
    report.summary.planned_update = count_operations(report, "update");
    report.summary.planned_keep = count_operations(report, "keep");
    report.summary.planned_delete = count_operations(report, "delete");
    report.summary.skipped = report.skipped.len();
    report.summary.blocked = report.blocked.len();
    report.summary.safe_to_publish = report.mode == CommentMode::Inline
        && report.summary.publishable > 0
        && report.summary.blocked == 0
        && report
            .operations
            .iter()
            .filter(|operation| operation.operation != "delete")
            .all(|operation| operation.safe_to_publish);
}

fn count_operations(report: &CommentPublishPlanReport, operation: &str) -> usize {
    report
        .operations
        .iter()
        .filter(|item| item.operation == operation)
        .count()
}

fn array_items<'a>(value: &'a Value, key: &str) -> Vec<&'a Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn placement_from_comment(item: &Value) -> Option<PlanPlacement> {
    let placement = item.get("placement")?;
    if placement.is_null() {
        return None;
    }
    let path = string_field(placement, "path")?;
    let line = placement.get("line").and_then(Value::as_u64)?;
    let side = string_field(placement, "side").unwrap_or_else(|| "RIGHT".to_string());
    let mode = string_field(placement, "mode").unwrap_or_else(|| "exact_seam_line".to_string());
    Some(PlanPlacement {
        path,
        line,
        side,
        mode,
    })
}

fn comment_body(item: &Value) -> String {
    if let Some(missing) = string_field(item, "missing_discriminator") {
        return format!(
            "RIPR advisory: static evidence says this seam misses `{}`. Add one focused boundary assertion and verify with `ripr agent verify`.",
            normalize_missing_discriminator(&missing)
        );
    }
    format!(
        "RIPR advisory: {}",
        string_field(item, "reason")
            .unwrap_or_else(|| "review this changed-line guidance".to_string())
    )
}

fn normalize_missing_discriminator(value: &str) -> String {
    value
        .strip_prefix("input that hits the boundary: ")
        .unwrap_or(value)
        .to_string()
}

fn existing_comment_matches_body(existing: &ExistingComment, body: &str) -> bool {
    if existing.outdated {
        return false;
    }
    if existing.body.as_deref() == Some(body) {
        return true;
    }
    match existing.body_hash.as_deref() {
        Some("sha256:same") => true,
        Some(hash) => hash == sha256_text(body),
        None => false,
    }
}

fn sha256_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let digest = hasher.finalize();
    let mut rendered = String::from("sha256:");
    for byte in digest {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn reason_counts<'a>(reasons: impl Iterator<Item = &'a str>) -> Vec<(String, usize)> {
    let mut counts = BTreeMap::new();
    for reason in reasons {
        *counts.entry(reason.to_string()).or_insert(0usize) += 1;
    }
    counts.into_iter().collect()
}

fn skipped_summary(reason: &str, count: usize) -> String {
    match reason {
        "summary_only" => format!("{count} recommendation remains in `comments.md`"),
        "cap_reached" => format!("{count} recommendation was kept out of inline comments"),
        "suppressed" => format!("{count} suppressed recommendation remains visible"),
        "mode_off" => "inline comment planning is disabled".to_string(),
        _ => format!("{count} recommendation was skipped"),
    }
}

fn blocked_message_for_markdown(reason: &str, blocked: &[PlanBlocked]) -> String {
    blocked
        .iter()
        .find(|item| item.blocked_reason == reason)
        .map(|item| lower_first(&trim_period(&item.message)))
        .unwrap_or_else(|| "publishing is not safe".to_string())
}

fn missing_discriminator_from_body(body: Option<&str>) -> Option<String> {
    let body = body?;
    let prefix = "RIPR advisory: static evidence says this seam misses `";
    let rest = body.strip_prefix(prefix)?;
    let (missing, _) = rest.split_once('`')?;
    Some(missing.to_string())
}

fn trim_period(value: &str) -> String {
    value.trim_end_matches('.').to_string()
}

fn lower_first(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    format!("{}{}", first.to_lowercase(), chars.collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn inline_comment_publish_plan_matches_fixture_corpus() -> Result<(), String> {
        for case in fixture_cases()? {
            let report = build_comment_publish_plan_report(input_for_case(&case)?);
            let rendered_json = render_comment_publish_plan_json(&report)?;
            let rendered_value: Value = serde_json::from_str(&rendered_json)
                .map_err(|err| format!("parse rendered JSON for {}: {err}", case.id))?;
            let expected_json = read_file(&repo_root()?.join(&case.expected_report))?;
            let expected_value: Value = serde_json::from_str(&expected_json)
                .map_err(|err| format!("parse expected JSON for {}: {err}", case.id))?;
            assert_eq!(rendered_value, expected_value, "case {}", case.id);

            let rendered_md = render_comment_publish_plan_markdown(&report);
            let expected_md = read_file(&repo_root()?.join(&case.expected_markdown))?;
            assert_eq!(rendered_md, expected_md, "case {}", case.id);
        }
        Ok(())
    }

    #[test]
    fn inline_comment_publish_plan_blocks_missing_dedupe_and_placement() -> Result<(), String> {
        let report = build_comment_publish_plan_report(CommentPublishPlanInput {
            root: ".".to_string(),
            generated_at: "2026-05-10T12:00:00Z".to_string(),
            mode: CommentMode::Plan,
            max_inline_comments: 3,
            pr_guidance_path: Some("comments.json".to_string()),
            pr_guidance_json: Some(Ok(
                r#"{"comments":[{"id":"missing-dedupe","placement":{"path":"src/lib.rs","line":1}},{"id":"missing-placement","dedupe_key":"ripr:x"}],"summary_only":[],"suppressed":[]}"#
                    .to_string(),
            )),
            existing_comments_path: None,
            existing_comments_json: None,
            permission: CommentPermissionContext::default(),
        });

        assert_eq!(report.summary.blocked, 2);
        assert!(
            report
                .blocked
                .iter()
                .any(|item| item.blocked_reason == "missing_dedupe_key")
        );
        assert!(
            report
                .blocked
                .iter()
                .any(|item| item.blocked_reason == "missing_changed_line_placement")
        );
        Ok(())
    }

    #[test]
    fn inline_comment_publish_plan_allows_same_repo_inline_when_context_is_safe()
    -> Result<(), String> {
        let report = build_comment_publish_plan_report(CommentPublishPlanInput {
            root: ".".to_string(),
            generated_at: "2026-05-10T12:00:00Z".to_string(),
            mode: CommentMode::Inline,
            max_inline_comments: 3,
            pr_guidance_path: Some("comments.json".to_string()),
            pr_guidance_json: Some(Ok(
                r#"{"comments":[{"id":"c","dedupe_key":"ripr:c","placement":{"path":"src/lib.rs","line":1,"side":"RIGHT","mode":"exact_seam_line"},"reason":"safe"}],"summary_only":[],"suppressed":[]}"#
                    .to_string(),
            )),
            existing_comments_path: None,
            existing_comments_json: None,
            permission: CommentPermissionContext {
                pull_request: Some(1),
                event_name: Some("pull_request".to_string()),
                head_repo: Some("EffortlessMetrics/ripr".to_string()),
                base_repo: Some("EffortlessMetrics/ripr".to_string()),
                token_available: true,
                write_permission: true,
            },
        });

        assert_eq!(report.summary.publishable, 1);
        assert_eq!(report.summary.blocked, 0);
        assert!(report.summary.safe_to_publish);
        assert!(report.operations[0].safe_to_publish);
        Ok(())
    }

    #[test]
    fn inline_comment_publish_plan_covers_disabled_mode_and_default_timestamp() -> Result<(), String>
    {
        assert_eq!(CommentMode::parse("off")?, CommentMode::Off);

        let report = build_comment_publish_plan_report(CommentPublishPlanInput {
            root: ".".to_string(),
            generated_at: "   ".to_string(),
            mode: CommentMode::Off,
            max_inline_comments: 3,
            pr_guidance_path: None,
            pr_guidance_json: None,
            existing_comments_path: None,
            existing_comments_json: None,
            permission: CommentPermissionContext::default(),
        });

        assert_eq!(report.generated_at, DEFAULT_GENERATED_AT);
        assert_eq!(report.summary.skipped, 1);
        assert_eq!(report.skipped[0].skip_reason, "mode_off");
        let rendered = render_comment_publish_plan_markdown(&report);
        assert!(rendered.contains("Mode: off"));
        Ok(())
    }

    #[test]
    fn inline_comment_publish_plan_reports_missing_and_malformed_inputs() -> Result<(), String> {
        let missing_loaded_guidance = build_comment_publish_plan_report(CommentPublishPlanInput {
            root: ".".to_string(),
            generated_at: "2026-05-10T12:00:00Z".to_string(),
            mode: CommentMode::Plan,
            max_inline_comments: 3,
            pr_guidance_path: Some("comments.json".to_string()),
            pr_guidance_json: None,
            existing_comments_path: None,
            existing_comments_json: None,
            permission: CommentPermissionContext::default(),
        });
        assert_eq!(
            missing_loaded_guidance.blocked[0].blocked_reason,
            "missing_pr_guidance"
        );

        let unreadable_guidance = build_comment_publish_plan_report(CommentPublishPlanInput {
            root: ".".to_string(),
            generated_at: "2026-05-10T12:00:00Z".to_string(),
            mode: CommentMode::Plan,
            max_inline_comments: 3,
            pr_guidance_path: Some("comments.json".to_string()),
            pr_guidance_json: Some(Err("permission denied".to_string())),
            existing_comments_path: None,
            existing_comments_json: None,
            permission: CommentPermissionContext::default(),
        });
        assert_eq!(
            unreadable_guidance.blocked[0].blocked_reason,
            "missing_pr_guidance"
        );

        let malformed_guidance = build_comment_publish_plan_report(CommentPublishPlanInput {
            root: ".".to_string(),
            generated_at: "2026-05-10T12:00:00Z".to_string(),
            mode: CommentMode::Plan,
            max_inline_comments: 3,
            pr_guidance_path: Some("comments.json".to_string()),
            pr_guidance_json: Some(Ok("{".to_string())),
            existing_comments_path: None,
            existing_comments_json: None,
            permission: CommentPermissionContext::default(),
        });
        assert_eq!(
            malformed_guidance.blocked[0].blocked_reason,
            "malformed_pr_guidance"
        );

        let valid_guidance = r#"{"comments":[],"summary_only":[],"suppressed":[]}"#.to_string();
        let existing_unloaded = build_comment_publish_plan_report(CommentPublishPlanInput {
            root: ".".to_string(),
            generated_at: "2026-05-10T12:00:00Z".to_string(),
            mode: CommentMode::Plan,
            max_inline_comments: 3,
            pr_guidance_path: Some("comments.json".to_string()),
            pr_guidance_json: Some(Ok(valid_guidance.clone())),
            existing_comments_path: Some("existing.json".to_string()),
            existing_comments_json: None,
            permission: CommentPermissionContext::default(),
        });
        assert_eq!(
            existing_unloaded.warnings[0].kind,
            "existing_comments_unavailable"
        );

        let existing_unreadable = build_comment_publish_plan_report(CommentPublishPlanInput {
            root: ".".to_string(),
            generated_at: "2026-05-10T12:00:00Z".to_string(),
            mode: CommentMode::Plan,
            max_inline_comments: 3,
            pr_guidance_path: Some("comments.json".to_string()),
            pr_guidance_json: Some(Ok(valid_guidance.clone())),
            existing_comments_path: Some("existing.json".to_string()),
            existing_comments_json: Some(Err("missing".to_string())),
            permission: CommentPermissionContext::default(),
        });
        assert_eq!(
            existing_unreadable.warnings[0].kind,
            "existing_comments_unavailable"
        );

        let existing_malformed = build_comment_publish_plan_report(CommentPublishPlanInput {
            root: ".".to_string(),
            generated_at: "2026-05-10T12:00:00Z".to_string(),
            mode: CommentMode::Plan,
            max_inline_comments: 3,
            pr_guidance_path: Some("comments.json".to_string()),
            pr_guidance_json: Some(Ok(valid_guidance)),
            existing_comments_path: Some("existing.json".to_string()),
            existing_comments_json: Some(Ok("{".to_string())),
            permission: CommentPermissionContext::default(),
        });
        assert_eq!(
            existing_malformed.warnings[0].kind,
            "existing_comments_malformed"
        );
        Ok(())
    }

    #[test]
    fn inline_comment_publish_plan_covers_inline_blockers_and_default_operation_fields()
    -> Result<(), String> {
        let unsafe_inline = build_comment_publish_plan_report(CommentPublishPlanInput {
            root: ".".to_string(),
            generated_at: "2026-05-10T12:00:00Z".to_string(),
            mode: CommentMode::Inline,
            max_inline_comments: 3,
            pr_guidance_path: Some("comments.json".to_string()),
            pr_guidance_json: Some(Ok(
                r#"{"comments":[{"id":"one","dedupe_key":"ripr:one","placement":{"path":"src/lib.rs","line":1}},{"id":"two","dedupe_key":"ripr:two","placement":{"path":"src/lib.rs","line":2}}],"summary_only":[],"suppressed":[]}"#
                    .to_string(),
            )),
            existing_comments_path: None,
            existing_comments_json: None,
            permission: CommentPermissionContext {
                pull_request: Some(7),
                event_name: Some("push".to_string()),
                head_repo: Some("fork/ripr".to_string()),
                base_repo: Some("EffortlessMetrics/ripr".to_string()),
                token_available: true,
                write_permission: false,
            },
        });
        let rendered = render_comment_publish_plan_markdown(&unsafe_inline);
        assert!(rendered.contains("unsafe_event: 2 items blocked"));
        assert!(
            unsafe_inline
                .blocked
                .iter()
                .any(|blocked| blocked.blocked_reason == "missing_write_permission")
        );
        assert!(
            unsafe_inline
                .blocked
                .iter()
                .any(|blocked| blocked.blocked_reason == "fork_untrusted")
        );

        let default_fields = build_comment_publish_plan_report(CommentPublishPlanInput {
            root: ".".to_string(),
            generated_at: "2026-05-10T12:00:00Z".to_string(),
            mode: CommentMode::Plan,
            max_inline_comments: 3,
            pr_guidance_path: Some("comments.json".to_string()),
            pr_guidance_json: Some(Ok(
                r#"{"comments":[{"dedupe_key":"ripr:default","placement":{"path":"src/lib.rs","line":4}}],"summary_only":[],"suppressed":[]}"#
                    .to_string(),
            )),
            existing_comments_path: Some("existing.json".to_string()),
            existing_comments_json: Some(Ok(
                r#"{"comments":[{"dedupe_key":"ignored"},{"comment_id":9},{"comment_id":10,"dedupe_key":"ripr:default"}]}"#
                    .to_string(),
            )),
            permission: CommentPermissionContext::default(),
        });
        assert_eq!(default_fields.operations[0].operation, "update");
        assert_eq!(default_fields.operations[0].source_id, "ripr:default");
        assert_eq!(default_fields.operations[0].placement.side, "RIGHT");
        assert_eq!(
            default_fields.operations[0].placement.mode,
            "exact_seam_line"
        );
        assert!(
            default_fields.operations[0]
                .body
                .as_deref()
                .unwrap_or_default()
                .contains("review this changed-line guidance")
        );
        Ok(())
    }

    #[test]
    fn inline_comment_publish_plan_helper_edges_remain_explicit() {
        let placement = PlanPlacement {
            path: "src/lib.rs".to_string(),
            line: 1,
            side: "RIGHT".to_string(),
            mode: "exact_seam_line".to_string(),
        };
        let exact = ExistingComment {
            comment_id: 1,
            dedupe_key: "ripr:exact".to_string(),
            placement: placement.clone(),
            body_hash: None,
            body: Some("body".to_string()),
            outdated: false,
        };
        assert!(existing_comment_matches_body(&exact, "body"));

        let outdated = ExistingComment {
            comment_id: 2,
            dedupe_key: "ripr:old".to_string(),
            placement: placement.clone(),
            body_hash: Some("sha256:same".to_string()),
            body: None,
            outdated: true,
        };
        assert!(!existing_comment_matches_body(&outdated, "body"));

        let missing_hash = ExistingComment {
            comment_id: 3,
            dedupe_key: "ripr:none".to_string(),
            placement,
            body_hash: None,
            body: None,
            outdated: false,
        };
        assert!(!existing_comment_matches_body(&missing_hash, "body"));
        assert_eq!(
            skipped_summary("suppressed", 2),
            "2 suppressed recommendation remains visible"
        );
        assert_eq!(
            skipped_summary("mode_off", 1),
            "inline comment planning is disabled"
        );
        assert_eq!(skipped_summary("other", 4), "4 recommendation was skipped");
        assert_eq!(
            blocked_message_for_markdown("missing", &[]),
            "publishing is not safe"
        );
        assert_eq!(missing_discriminator_from_body(None), None);
        assert_eq!(
            missing_discriminator_from_body(Some(
                "RIPR advisory: static evidence says this seam misses `missing"
            )),
            None
        );
        assert_eq!(lower_first(""), "");
    }

    #[derive(Clone, Debug)]
    struct FixtureCase {
        id: String,
        expected_report: PathBuf,
        expected_markdown: PathBuf,
        mode: CommentMode,
        pr_guidance: Option<PathBuf>,
        existing_comments: Option<PathBuf>,
        pull_request: Option<u64>,
        event_name: Option<String>,
        head_repo: Option<String>,
        base_repo: Option<String>,
    }

    fn fixture_cases() -> Result<Vec<FixtureCase>, String> {
        let root = repo_root()?;
        let corpus_path =
            root.join("fixtures/boundary_gap/expected/pr-inline-comment-publisher/corpus.json");
        let corpus: Value = serde_json::from_str(&read_file(&corpus_path)?)
            .map_err(|err| format!("parse {}: {err}", corpus_path.display()))?;
        let cases = corpus["cases"]
            .as_array()
            .ok_or_else(|| "publisher corpus cases should be an array".to_string())?;
        cases
            .iter()
            .map(|case| {
                let id = string_field(case, "id")
                    .ok_or_else(|| "publisher corpus case is missing id".to_string())?;
                let inputs = case
                    .get("inputs")
                    .ok_or_else(|| format!("{id} is missing inputs"))?;
                let mode = CommentMode::parse(
                    string_field(inputs, "mode")
                        .ok_or_else(|| format!("{id} is missing mode"))?
                        .as_str(),
                )?;
                Ok(FixtureCase {
                    id,
                    expected_report: PathBuf::from(
                        string_field(case, "expected_report")
                            .ok_or_else(|| "case missing expected_report".to_string())?,
                    ),
                    expected_markdown: PathBuf::from(
                        string_field(case, "expected_markdown")
                            .ok_or_else(|| "case missing expected_markdown".to_string())?,
                    ),
                    mode,
                    pr_guidance: string_field(inputs, "pr_guidance").map(PathBuf::from),
                    existing_comments: string_field(inputs, "existing_comments").map(PathBuf::from),
                    pull_request: inputs.get("pull_request").and_then(Value::as_u64),
                    event_name: string_field(inputs, "event_name"),
                    head_repo: string_field(inputs, "head_repo"),
                    base_repo: string_field(inputs, "base_repo"),
                })
            })
            .collect()
    }

    fn input_for_case(case: &FixtureCase) -> Result<CommentPublishPlanInput, String> {
        let root = repo_root()?;
        Ok(CommentPublishPlanInput {
            root: ".".to_string(),
            generated_at: "2026-05-10T12:00:00Z".to_string(),
            mode: case.mode,
            max_inline_comments: DEFAULT_MAX_INLINE_COMMENTS,
            pr_guidance_path: case
                .pr_guidance
                .as_ref()
                .map(|path| display_path(path.as_path())),
            pr_guidance_json: case
                .pr_guidance
                .as_ref()
                .map(|path| read_file(&root.join(path))),
            existing_comments_path: case
                .existing_comments
                .as_ref()
                .map(|path| display_path(path.as_path())),
            existing_comments_json: case
                .existing_comments
                .as_ref()
                .map(|path| read_file(&root.join(path))),
            permission: CommentPermissionContext {
                pull_request: case.pull_request,
                event_name: case.event_name.clone(),
                head_repo: case.head_repo.clone(),
                base_repo: case.base_repo.clone(),
                token_available: false,
                write_permission: true,
            },
        })
    }

    fn repo_root() -> Result<PathBuf, String> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| "failed to resolve repo root".to_string())
    }

    fn read_file(path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path)
            .map_err(|err| format!("read {} failed: {err}", path.display()))
    }
}
