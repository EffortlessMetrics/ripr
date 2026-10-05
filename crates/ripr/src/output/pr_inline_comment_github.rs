//! The GitHub side of inline-comment publishing (#5409).
//!
//! The generated workflow used to carry two jq programs around its `gh api`
//! calls: one turned the paginated review comments into
//! `existing-comments.json`, the other turned the publish plan into PATCH
//! and review requests. Both live here now, so the workflow holds only the
//! token and the `gh api` loop. Nothing here reads a token or the network.
//!
//! Each function mirrors the retired program's output; the tests below and
//! `tests/generated_review_workflow.rs` hold it to that program's results.

use serde_json::{Map, Value, json};

/// The marker the publish step appends to every card it posts.
const DEDUPE_MARKER: &str = "<!-- ripr:dedupe=";
const COMPACT_MARKER: &str = " presentation=compact-v1 -->";
const CARD_OPEN: &str = "<details><summary>Full RIPR repair card</summary>\n\n";
const CARD_CLOSE: &str = "\n\n</details>";
const LEGACY_PRESENTATION: &str = "__ripr_legacy_presentation__";
const UNREADABLE_PRESENTATION: &str = "__ripr_compact_presentation_unreadable__";
/// The only author whose marked comments count: the workflow posts with
/// `github.token`. Anyone can type the marker.
const WORKFLOW_BOT_LOGIN: &str = "github-actions[bot]";

/// `existing-comments.json` from the pages `gh api --paginate --slurp`
/// returns for a pull request's review comments.
pub(crate) fn existing_comments(pages: &Value) -> Value {
    let comments: Vec<Value> = values(pages)
        .flat_map(values)
        .filter_map(existing_comment)
        .collect();
    json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "kind": "pr_inline_comment_existing_comments",
        "comments": comments,
    })
}

fn existing_comment(comment: &Value) -> Option<Value> {
    let user = comment.get("user");
    let login = user.and_then(|user| user.get("login"));
    let kind = user.and_then(|user| user.get("type"));
    if login != Some(&json!(WORKFLOW_BOT_LOGIN)) || kind != Some(&json!("Bot")) {
        return None;
    }
    let body = match comment.get("body") {
        None | Some(Value::Null) => "",
        Some(Value::String(body)) => body.as_str(),
        Some(_) => return None,
    };
    if !body.contains(DEDUPE_MARKER) {
        return None;
    }
    let dedupe_key = dedupe_key(body)?;
    let field = |name: &str| comment.get(name).cloned().unwrap_or(Value::Null);
    let line = match field("line") {
        Value::Null | Value::Bool(false) => field("original_line"),
        line => line,
    };
    let side = match field("side") {
        Value::Null | Value::Bool(false) => json!("RIGHT"),
        side => side,
    };
    let card = if body.contains(COMPACT_MARKER) {
        full_card(body).unwrap_or(UNREADABLE_PRESENTATION)
    } else {
        LEGACY_PRESENTATION
    };
    Some(json!({
        "comment_id": field("id"),
        "dedupe_key": dedupe_key,
        "path": field("path"),
        "line": line,
        "side": side,
        "body": card,
        "outdated": field("position").is_null() && field("line").is_null(),
    }))
}

/// The key in the first `<!-- ripr:dedupe=KEY[ presentation=P] -->` marker
/// that closes on its own line, as
/// `capture("<!-- ripr:dedupe=(?<key>.*?)(?: presentation=[^ ]+)? -->")`
/// reads it: the shortest key, and a presentation tag is not part of it.
fn dedupe_key(body: &str) -> Option<&str> {
    for (start, _) in body.match_indices(DEDUPE_MARKER) {
        let key_start = start + DEDUPE_MARKER.len();
        let rest = &body[key_start..];
        let line_len = rest.find('\n').unwrap_or(rest.len());
        let ends = rest[..line_len]
            .char_indices()
            .map(|(at, _)| at)
            .chain(std::iter::once(line_len));
        for end in ends {
            if marker_closes(&rest[end..]) {
                return Some(&rest[..end]);
            }
        }
    }
    None
}

/// Whether `text` starts with `(?: presentation=[^ ]+)? -->`.
fn marker_closes(text: &str) -> bool {
    if let Some(tag) = text.strip_prefix(" presentation=") {
        let run = tag.find(' ').unwrap_or(tag.len());
        if run > 0 && tag[run..].starts_with(" -->") {
            return true;
        }
    }
    text.starts_with(" -->")
}

/// The card inside the compact `<details>` block: from the first opening
/// to the last closing, as the retired `(?<card>.*)` with flag `m` did.
fn full_card(body: &str) -> Option<&str> {
    let open = body.find(CARD_OPEN)? + CARD_OPEN.len();
    let close = body.rfind(CARD_CLOSE)?;
    (close >= open).then(|| &body[open..close])
}

/// One `gh api` call the publish step makes.
#[derive(Debug, PartialEq)]
pub(crate) struct PublishRequest {
    pub(crate) method: &'static str,
    /// The endpoint after `repos/OWNER/REPO/`.
    pub(crate) endpoint: String,
    pub(crate) payload: Value,
    /// Printed after the call succeeds.
    pub(crate) message: String,
}

/// What the publish step does with one plan.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct PublishRequests {
    /// Printed before any call: why nothing is published, or which cards
    /// are already current.
    pub(crate) notes: Vec<String>,
    /// PATCHes first, then at most one review POST, in the order the
    /// retired step made them.
    pub(crate) requests: Vec<PublishRequest>,
}

/// The requests for `comment-publish-plan.json` on pull request
/// `pull_request` at `head_sha`.
pub(crate) fn publish_requests(
    plan: &Value,
    pull_request: &str,
    head_sha: &str,
) -> PublishRequests {
    let mut out = PublishRequests::default();
    if plan.pointer("/summary/safe_to_publish") != Some(&Value::Bool(true)) {
        out.notes.push(
            "RIPR inline comments were not published because the publish plan is not safe."
                .to_string(),
        );
        for blocked in values_at(plan, "blocked") {
            let line = format!(
                "- {}: {}",
                interpolate(blocked.get("blocked_reason")),
                interpolate(blocked.get("message"))
            );
            out.notes.push(fold_lines(&line));
        }
        return out;
    }

    let publishable: Vec<(&Value, String)> = values_at(plan, "operations")
        .filter(|operation| operation.get("safe_to_publish") == Some(&Value::Bool(true)))
        .filter(|operation| {
            matches!(
                operation.get("operation").and_then(Value::as_str),
                Some("create" | "update" | "keep")
            )
        })
        .map(|operation| (operation, compact_body(operation)))
        .collect();
    let with_operation = |name: &'static str| {
        publishable.iter().filter(move |(operation, _)| {
            operation.get("operation").and_then(Value::as_str) == Some(name)
        })
    };
    let create_count = with_operation("create").count();
    let additional = summary_count(plan, "summary_only") + cap_skipped(plan);
    let suppressed = summary_count(plan, "suppressed");

    for (operation, body) in with_operation("update") {
        let Some(comment_id) = operation.get("existing_comment_id").and_then(Value::as_u64) else {
            out.notes.push(format!(
                "Skipped a RIPR inline comment update without a numeric comment id: {}",
                fold_lines(&interpolate(operation.get("dedupe_key")))
            ));
            continue;
        };
        out.requests.push(PublishRequest {
            method: "PATCH",
            endpoint: format!("pulls/comments/{comment_id}"),
            payload: json!({ "body": body }),
            message: format!(
                "Updated RIPR inline comment: {}",
                fold_lines(&interpolate(operation.get("dedupe_key")))
            ),
        });
    }

    // Count the PATCHes actually queued: a skipped update must neither
    // trigger a summary review nor be reported as done.
    let update_count = out.requests.len();
    if create_count > 0 || (update_count > 0 && (additional > 0 || suppressed > 0)) {
        let mut payload = Map::new();
        payload.insert(
            "body".to_string(),
            json!(review_body(plan, additional, suppressed)),
        );
        payload.insert("event".to_string(), json!("COMMENT"));
        payload.insert("commit_id".to_string(), json!(head_sha));
        if create_count > 0 {
            let comments: Vec<Value> = with_operation("create")
                .map(|(operation, body)| {
                    let placement = |name: &str| {
                        operation
                            .pointer(&format!("/placement/{name}"))
                            .cloned()
                            .unwrap_or(Value::Null)
                    };
                    let side = match placement("side") {
                        Value::Null | Value::Bool(false) => json!("RIGHT"),
                        side => side,
                    };
                    json!({
                        "path": placement("path"),
                        "line": placement("line"),
                        "side": side,
                        "body": body,
                    })
                })
                .collect();
            payload.insert("comments".to_string(), Value::Array(comments));
        }
        out.requests.push(PublishRequest {
            method: "POST",
            endpoint: format!("pulls/{pull_request}/reviews"),
            payload: Value::Object(payload),
            message: if create_count > 0 {
                format!("Created one RIPR review with {create_count} inline comment(s).")
            } else {
                format!(
                    "Created one RIPR review summary after {update_count} inline comment update(s)."
                )
            },
        });
    }

    for (operation, _) in with_operation("keep") {
        out.notes.push(format!(
            "RIPR inline comment already current: {}",
            fold_lines(&interpolate(operation.get("dedupe_key")))
        ));
    }
    out
}

/// The review's top-level text.
fn review_body(plan: &Value, additional: u64, suppressed: u64) -> String {
    let inline = summary_count(plan, "publishable");
    let plural = |count: u64| if count == 1 { "" } else { "s" };
    let mut body = format!(
        "RIPR surfaced {inline} line-placed recommendation{}.",
        plural(inline)
    );
    if additional > 0 {
        body.push_str(&format!(
            "\n\n{additional} additional recommendation{} remain in the generated `target/ripr/review/comments.json` and `target/ripr/review/comments.md` artifacts.",
            plural(additional)
        ));
    }
    if suppressed > 0 {
        body.push_str(&format!(
            "\n\n{suppressed} suppressed recommendation{} remain visible there with reasons.",
            plural(suppressed)
        ));
    }
    body.push_str("\n\nAdvisory static evidence only; gate authority remains separate.");
    body
}

/// The published body: a one-line lead, the next command, the full card
/// folded under `<details>`, and the dedupe marker the capture reads back.
fn compact_body(operation: &Value) -> String {
    let full = operation
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let dedupe_key = interpolate(operation.get("dedupe_key"));
    let gap = full
        .strip_prefix("### ripr gap: ")
        .and_then(|rest| non_empty_line(rest))
        .unwrap_or("repairable gap");
    let repair = full
        .match_indices("\nRepair:\n")
        .find_map(|(at, heading)| non_empty_line(&full[at + heading.len()..]))
        .unwrap_or("Follow the bounded repair route in the RIPR artifact.");
    let next = match code_span_line(full, "Start the repair") {
        Some(start) => format!("Start the repair: {start}"),
        None => format!(
            "Verify: {}",
            code_span_line(full, "Verify").unwrap_or("`ripr agent verify`")
        ),
    };
    format!(
        "**ripr: {gap}** — {repair}\n\n{next}\n\n<details><summary>Full RIPR repair card</summary>\n\n{full}\n\n</details>\n\n<!-- ripr:dedupe={dedupe_key} presentation=compact-v1 -->"
    )
}

/// `[^\n]+` at the start of `text`.
fn non_empty_line(text: &str) -> Option<&str> {
    let line = &text[..text.find('\n').unwrap_or(text.len())];
    (!line.is_empty()).then_some(line)
}

/// The first `\nLABEL:\n` line that is one code span: a fence of N
/// backticks, content that neither starts nor ends with a backtick, and
/// the same N-backtick fence closing the line.
fn code_span_line<'a>(text: &'a str, label: &str) -> Option<&'a str> {
    let heading = format!("\n{label}:\n");
    text.match_indices(&heading).find_map(|(at, _)| {
        let rest = &text[at + heading.len()..];
        let line = &rest[..rest.find('\n').unwrap_or(rest.len())];
        is_code_span(line).then_some(line)
    })
}

fn is_code_span(line: &str) -> bool {
    let fence = line.len() - line.trim_start_matches('`').len();
    let tail = line.len() - line.trim_end_matches('`').len();
    // The content is at least one character, so the fences cannot overlap.
    fence > 0 && tail == fence && line.len() > 2 * fence
}

fn values(value: &Value) -> Box<dyn Iterator<Item = &Value> + '_> {
    match value {
        Value::Array(items) => Box::new(items.iter()),
        Value::Object(map) => Box::new(map.values()),
        _ => Box::new(std::iter::empty()),
    }
}

fn values_at<'a>(value: &'a Value, key: &str) -> Box<dyn Iterator<Item = &'a Value> + 'a> {
    value.get(key).map_or_else(
        || Box::new(std::iter::empty()) as Box<dyn Iterator<Item = &Value>>,
        values,
    )
}

fn summary_count(plan: &Value, name: &str) -> u64 {
    plan.pointer(&format!("/summary/{name}"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

/// Comments the plan skipped for the inline cap or body size: they stay
/// in the artifacts and count as additional recommendations.
fn cap_skipped(plan: &Value) -> u64 {
    values_at(plan, "skipped")
        .filter(|skipped| {
            matches!(
                skipped.get("skip_reason").and_then(Value::as_str),
                Some("inline_comment_cap_reached" | "comment_body_too_large")
            )
        })
        .count() as u64
}

/// A value as jq's `"\(.)"` prints it: strings raw, anything else as JSON.
fn interpolate(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
        None => "null".to_string(),
    }
}

/// Folds CR and LF to spaces, so a repository path or key cannot start a
/// line that GitHub reads as a workflow command.
fn fold_lines(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests;
