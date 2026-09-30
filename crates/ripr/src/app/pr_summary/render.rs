use super::io::file_state;
use super::model::JsonInput;
use super::util::{string_field, summary_bool, summary_string_or_null, summary_u64};
use crate::output::first_pr::{ProofPathLabels, REPAIR_AFTER_PHASE_LABEL, REPAIR_AFTER_PHASE_STEP};
use crate::output::markdown::{
    COMMAND_SHELL_DISCLOSURE, PowershellForm, code_span, inline_prose, powershell_form,
    table_code_span,
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
        "- status: {}\n",
        value_code(start_here_value, &["status"])
    ));
    out.push_str(&format!(
        "- selected state: {}\n",
        value_code(start_here_value, &["selected", "state"])
    ));

    match value_raw(start_here_value, &["selected", "state"]).as_str() {
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
        "- canonical gap: {}\n",
        code_span(&first_available_string(
            start_here_value,
            &[&["selected", "canonical_gap_id"], &["selected", "gap_id"]]
        ))
    ));
    out.push_str(&format!(
        "- language: {} ({})\n",
        value_code(start_here_value, &["selected", "language"]),
        inline_prose(&value_raw(
            start_here_value,
            &["selected", "language_status"]
        ))
    ));
    out.push_str(&format!(
        "- top gap: {}\n",
        value_code(start_here_value, &["selected", "kind"])
    ));
    out.push_str(&format!(
        "- changed behavior: {}\n",
        value_code(start_here_value, &["selected", "changed_behavior"])
    ));
    out.push_str(&format!(
        "- missing discriminator: {}\n",
        value_code(start_here_value, &["selected", "missing_discriminator"])
    ));
    out.push_str(&format!(
        "- focused proof intent: {}\n",
        value_code(start_here_value, &["selected", "focused_proof_intent"])
    ));
    out.push_str(&format!(
        "- repair route: {}\n",
        value_code(start_here_value, &["selected", "repair", "route"])
    ));
    out.push_str(&format!(
        "- repair target: {}\n",
        value_code(start_here_value, &["selected", "repair", "target_file"])
    ));
    out.push_str(&format!(
        "- related test: {}\n",
        value_code(start_here_value, &["selected", "repair", "related_test"])
    ));
    out.push_str(&format!(
        "- static limit: {}\n",
        code_span(&first_available_string_or(
            start_here_value,
            &[
                &["selected", "static_limit_kind"],
                &["selected", "static_limit_detail"]
            ],
            "none"
        ))
    ));
    let repair_command = start_here_value
        .and_then(|value| value.pointer("/selected/repair_command"))
        .and_then(Value::as_str)
        .filter(|command| !command.trim().is_empty());
    let labels = push_repair_transaction(out, repair_command);
    out.push_str(&format!(
        "- {}: {}\n",
        labels.verify,
        value_code(start_here_value, &["selected", "verify_command"])
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        labels.receipt,
        value_code(start_here_value, &["selected", "receipt_command"])
    ));
    out.push_str(&format!(
        "- receipt state: {}\n",
        value_code(start_here_value, &["selected", "receipt_state"])
    ));
}

/// Lower-case proof-path labels for this summary's lower-case bullets.
struct SummaryProofLabels {
    verify: String,
    receipt: String,
}

/// Render a carried repair start as the lead of the proof path (#3906):
/// the start, then its after phase, which runs verify and writes the
/// receipt. Returns the verify and receipt labels from the shared selector
/// (F60-14): the manual alternative beside a start, otherwise steps that run
/// after the test edit. JSON fields are unchanged.
fn push_repair_transaction(out: &mut String, repair_command: Option<&str>) -> SummaryProofLabels {
    if let Some(command) = repair_command {
        out.push_str(&format!("- start repair: {}\n", code_span(command)));
        out.push_str(&format!(
            "- {}: {REPAIR_AFTER_PHASE_STEP}\n",
            REPAIR_AFTER_PHASE_LABEL.to_lowercase()
        ));
    }
    let labels = ProofPathLabels::for_repair_start(repair_command.is_some());
    SummaryProofLabels {
        verify: labels.verify.to_lowercase(),
        receipt: labels.receipt.to_lowercase(),
    }
}

fn render_start_here_missing(out: &mut String, start_here_value: Option<&Value>) {
    out.push_str(&format!(
        "- missing artifact: {}\n",
        value_code(start_here_value, &["selected", "artifact", "path"])
    ));
    push_next_command_pair(out, start_here_value, &["selected", "regeneration_command"]);
}

fn render_start_here_no_action(out: &mut String, start_here_value: Option<&Value>) {
    out.push_str(&format!(
        "- reason: {}\n",
        value_code(start_here_value, &["selected", "reason"])
    ));
    out.push_str("- no-action is not runtime, coverage, mutation, gate, or merge adequacy.\n");
}

fn render_start_here_blocked(out: &mut String, start_here_value: Option<&Value>) {
    out.push_str(&format!(
        "- blocked reason: {}\n",
        value_code(start_here_value, &["selected", "message"])
    ));
    push_next_command_pair(out, start_here_value, &["selected", "next_command"]);
}

/// Present one Start Here next command for both shells (#4950).
///
/// The bash bullet stays authoritative and byte-identical; the shared
/// [`powershell_form`] classification then renders the same outcome
/// first-pr's pairing prints for these packet fields (#2628): the guarded
/// BOM-free UTF-8 write twin when PowerShell needs a different form, the
/// runs-unchanged note when it does not, and the explicit unavailable
/// disclosure for a compound command instead of an invalid or invented
/// translation. A missing or empty field keeps its `not_available` line and
/// gains no shell outcome.
fn push_next_command_pair(out: &mut String, start_here_value: Option<&Value>, path: &[&str]) {
    out.push_str(&format!(
        "- next command: {}\n",
        value_code(start_here_value, path)
    ));
    let Some(command) = start_here_value
        .and_then(|value| value_at_path(Some(value), path))
        .and_then(Value::as_str)
        .filter(|command| !command.trim().is_empty())
    else {
        return;
    };
    match powershell_form(command) {
        PowershellForm::Translated(line) => {
            out.push_str(&format!(
                "- next command (PowerShell): {}\n",
                code_span(&line)
            ));
        }
        PowershellForm::SameAsBash => {
            out.push_str(
                "- next command runs unchanged in Bash and PowerShell; cmd.exe is not supported.\n",
            );
        }
        PowershellForm::Unavailable => {
            out.push_str(&format!(
                "- {}: {}\n",
                crate::output::markdown::POWERSHELL_UNAVAILABLE_DISCLOSURE,
                code_span(command)
            ));
        }
    }
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
        .map(inline_prose)
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
        "- PR evidence status: {}\n",
        string_field(pr_value, "status")
    ));
    out.push_str(&format!(
        "- review guidance status: {}\n",
        string_field(review_value, "status")
    ));
    out.push_str(&format!("- base: {}\n", string_field(pr_value, "base")));
    out.push_str(&format!("- head: {}\n", string_field(pr_value, "head")));
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
        "- routing_reason: {}\n\n",
        summary_string_or_null(pr_value, "routing_reason")
    ));
}

fn render_artifacts(out: &mut String, input: &SummaryRenderInput<'_>) {
    out.push_str("## Artifacts\n\n");
    out.push_str("| Artifact | Path | State |\n");
    out.push_str("| --- | --- | --- |\n");
    out.push_str(&format!(
        "| PR evidence JSON | {} | {} |\n",
        table_code_span(input.pr_evidence_json),
        input.pr_evidence.state
    ));
    out.push_str(&format!(
        "| PR evidence Markdown | {} | {} |\n",
        table_code_span(input.pr_evidence_md),
        file_state(input.repo, input.pr_evidence_md)
    ));
    out.push_str(&format!(
        "| Review guidance JSON | {} | {} |\n",
        table_code_span(input.review_comments_json),
        input.review_comments.state
    ));
    out.push_str(&format!(
        "| Review guidance Markdown | {} | {} |\n",
        table_code_span(input.review_comments_md),
        file_state(input.repo, input.review_comments_md)
    ));
    out.push_str(&format!(
        "| Start-here JSON | {} | {} |\n",
        table_code_span(input.start_here_json),
        input.start_here.state
    ));
    out.push_str(&format!(
        "| Start-here Markdown | {} | {} |\n",
        table_code_span(input.start_here_md),
        file_state(input.repo, input.start_here_md)
    ));
    out.push_str(&format!(
        "| PR evidence summary Markdown | {} | generated |\n",
        table_code_span(input.pr_summary_md)
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
            value.as_str().map(str::to_string)
        })
        .unwrap_or_else(|| fallback.to_string())
}

/// The string at `path` as one code span; see [`value_raw`].
fn value_code(value: Option<&Value>, path: &[&str]) -> String {
    code_span(&value_raw(value, path))
}

/// The string at `path`, `not_available` when absent or null, or `invalid`.
fn value_raw(value: Option<&Value>, path: &[&str]) -> String {
    let Some(value) = value_at_path(value, path) else {
        return "not_available".to_string();
    };
    if value.is_null() {
        "not_available".to_string()
    } else {
        value
            .as_str()
            .map(str::to_string)
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

/// Render the compact Markdown panel from the computed summary struct.
pub fn render_evidence_summary_md(s: &super::model::PrEvidenceSummaryJson) -> String {
    use super::model::{NullableU64, U64OrNotAvailable};
    let mut out = String::new();
    out.push_str("# PR Evidence Summary v1\n\n");
    out.push_str(&format!("**Run Status**: {}\n\n", code_span(&s.run_status)));
    out.push_str(&format!(
        "**Analysis Complete**: {}\n\n",
        code_span(
            &s.analysis_complete
                .map_or_else(|| "not_available".to_string(), |value| value.to_string())
        )
    ));
    if let Some(outcome) = &s.analysis_outcome {
        let kind = outcome
            .pointer("/outcome/kind")
            .and_then(Value::as_str)
            .unwrap_or("not_available");
        out.push_str(&format!("**Analysis Outcome**: {}\n\n", code_span(kind)));
        if let Some(limitations) = outcome
            .pointer("/outcome/limitations")
            .and_then(Value::as_array)
        {
            out.push_str("### Analysis Limitations\n\n");
            for limitation in limitations {
                let limitation_kind = limitation
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or("not_available");
                let recovery = limitation
                    .pointer("/recovery/kind")
                    .and_then(Value::as_str)
                    .unwrap_or("not_available");
                out.push_str(&format!(
                    "- {}; recovery: {}\n",
                    code_span(limitation_kind),
                    code_span(recovery)
                ));
            }
            out.push('\n');
        }
    }

    let surfaces = match &s.changed_surfaces {
        U64OrNotAvailable::Value(n) => n.to_string(),
        U64OrNotAvailable::NotAvailable => "not_available".to_string(),
    };
    out.push_str(&format!("**Changed Surfaces**: {surfaces}\n\n"));

    out.push_str("## Gaps\n\n");
    let fmt_u64 = |v: &U64OrNotAvailable| -> String {
        match v {
            U64OrNotAvailable::Value(n) => n.to_string(),
            U64OrNotAvailable::NotAvailable => "not_available".to_string(),
        }
    };
    let fmt_nullable = |v: &NullableU64| -> String {
        match v {
            NullableU64::Value(n) => n.to_string(),
            NullableU64::Null => "null".to_string(),
        }
    };
    out.push_str(&format!(
        "- total actionable: {}\n",
        fmt_u64(&s.gaps.total_actionable)
    ));
    out.push_str(&format!(
        "- total static limitations: {}\n",
        fmt_u64(&s.gaps.total_static_limitation)
    ));
    out.push_str(&format!(
        "- new actionable: {}\n",
        fmt_nullable(&s.gaps.new_actionable)
    ));
    out.push_str(&format!("- resolved: {}\n", fmt_nullable(&s.gaps.resolved)));
    out.push_str(&format!(
        "- regressed: {}\n",
        fmt_nullable(&s.gaps.regressed)
    ));
    if let Some(note) = &s.gaps.gap_delta_note {
        out.push_str(&format!("- delta note: {}\n", inline_prose(note)));
    }
    out.push('\n');

    out.push_str("## Limitations\n\n");
    // `none` is a finding and `not_available` is the absence of one. A run
    // whose repo-exposure artifact was never read has established neither, and
    // `empty_state_line` is the single owner of which of the two this is.
    let limitations = s.limitations.entries();
    if limitations.is_empty() {
        out.push_str(s.limitations.empty_state_line());
    } else {
        for lim in limitations {
            out.push_str(&format!(
                "- {}: {}\n",
                code_span(&lim.category),
                inline_prose(&lim.repair_route)
            ));
        }
    }
    out.push('\n');

    let missing_receipts = fmt_u64(&s.missing_receipts);
    out.push_str(&format!("**Missing Receipts**: {missing_receipts}\n\n"));

    out.push_str("## Receipt Status\n\n");
    out.push_str(&format!(
        "- receipts present: {}\n",
        fmt_u64(&s.receipt_status.receipts_present)
    ));
    out.push_str(&format!(
        "- missing receipts: {}\n",
        fmt_u64(&s.receipt_status.missing_receipts)
    ));
    out.push_str(&format!(
        "- orphan receipts: {}\n",
        fmt_u64(&s.receipt_status.orphan_receipts)
    ));
    out.push_str(&format!(
        "- stale receipts: {}\n",
        fmt_u64(&s.receipt_status.stale_receipts)
    ));
    out.push_str(&format!(
        "- gap mismatch receipts: {}\n",
        fmt_u64(&s.receipt_status.gap_mismatch_receipts)
    ));
    out.push_str(&format!(
        "- verify failed receipts: {}\n",
        fmt_u64(&s.receipt_status.verify_failed_receipts)
    ));
    out.push('\n');

    out.push_str("## Top Repair\n\n");
    if let Some(repair) = &s.top_repair {
        out.push_str(&format!(
            "- canonical gap: {}\n",
            code_span(&repair.canonical_gap_id)
        ));
        out.push_str(&format!("- language: {}\n", code_span(&repair.language)));
        out.push_str(&format!(
            "- repair kind: {}\n",
            code_span(&repair.repair_kind)
        ));
        out.push_str(&format!("- target: {}\n", code_span(&repair.target)));
        let labels = push_repair_transaction(&mut out, repair.repair_command.as_deref());
        out.push_str(&format!(
            "- {}: {}\n",
            labels.verify,
            code_span(&repair.verify_command)
        ));
        out.push_str(&format!(
            "- {}: {}\n",
            labels.receipt,
            code_span(&repair.receipt_command)
        ));
        out.push_str(&format!(
            "- receipt state: {}\n",
            code_span(&repair.receipt_state)
        ));
    } else {
        let state = s.top_repair_state.as_deref().unwrap_or("missing_artifact");
        out.push_str(&format!("- state: {}\n", code_span(state)));
    }
    out.push('\n');

    out.push_str("## Top Limitation\n\n");
    if let Some(lim) = &s.top_limitation {
        out.push_str(&format!("- category: {}\n", code_span(&lim.category)));
        out.push_str(&format!(
            "- repair route: {}\n",
            code_span(&lim.repair_route)
        ));
        out.push_str(&format!(
            "- why not actionable: {}\n",
            inline_prose(&lim.why_not_actionable)
        ));
    } else {
        // Derived from `limitations`, so it inherits that field's state and
        // asks the same owner: with nothing read, there is no top limitation
        // and no absence of one either.
        out.push_str(s.limitations.empty_state_line());
    }
    out.push('\n');

    out.push_str("## Local Reproduction Commands\n\n");
    out.push_str(COMMAND_SHELL_DISCLOSURE);
    for cmd in &s.local_reproduction_commands {
        out.push_str(&format!("```bash\n{cmd}\n```\n\n"));
        match powershell_form(cmd) {
            PowershellForm::Translated(line) => {
                out.push_str(&format!("```powershell\n{line}\n```\n\n"));
            }
            PowershellForm::SameAsBash => {}
            PowershellForm::Unavailable => out.push_str(&format!(
                "{}: {}\n\n",
                crate::output::markdown::POWERSHELL_UNAVAILABLE_DISCLOSURE,
                code_span(cmd)
            )),
        }
    }

    out.push_str(
        "_This summary composes existing RIPR artifacts. \
        It is static advisory evidence only; \
        run status, gap counts, and receipt state are read from repo artifacts, \
        not computed by this command._\n",
    );

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::first_pr::{
        MANUAL_RECEIPT_LABEL, MANUAL_VERIFY_LABEL, RECEIPT_AFTER_VERIFY_LABEL,
        VERIFY_AFTER_EDIT_LABEL,
    };

    /// #3906: the legacy start-here section shows the carried repair start
    /// before verify, and nothing when start-here carries none.
    #[test]
    fn start_here_top_gap_leads_with_the_carried_repair_start() -> Result<(), String> {
        let command = "ripr agent repair --root crates/pricing --seam-id seam-b --phase before";
        let mut start_here = serde_json::json!({
            "selected": {
                "state": "top_gap",
                "seam_id": "seam-b",
                "verify_command": "ripr agent verify --root . --json",
                "repair_command": command
            }
        });
        let mut with = String::new();
        render_start_here_top_gap(&mut with, Some(&start_here));
        let start = with
            .find(&format!("- start repair: `{command}`\n"))
            .ok_or_else(|| format!("missing start repair line:\n{with}"))?;
        // #3906 (F60-14): the after phase follows the start, then the
        // manual verify and receipt, under the shared labels.
        let after = with
            .find(&format!(
                "- {}: {REPAIR_AFTER_PHASE_STEP}\n",
                REPAIR_AFTER_PHASE_LABEL.to_lowercase()
            ))
            .ok_or_else(|| format!("missing after-phase line:\n{with}"))?;
        let verify = with
            .find(&format!("- {}: `", MANUAL_VERIFY_LABEL.to_lowercase()))
            .ok_or_else(|| format!("missing manual verify line:\n{with}"))?;
        let receipt = with
            .find(&format!("- {}: `", MANUAL_RECEIPT_LABEL.to_lowercase()))
            .ok_or_else(|| format!("missing manual receipt line:\n{with}"))?;
        if !(start < after && after < verify && verify < receipt) {
            return Err(format!(
                "the repair transaction must read in order:\n{with}"
            ));
        }
        if with.contains("- verify: `") || with.contains("- receipt: `") {
            return Err(format!(
                "verify and receipt must not be peer steps:\n{with}"
            ));
        }

        if let Some(selected) = start_here
            .get_mut("selected")
            .and_then(Value::as_object_mut)
        {
            selected.remove("repair_command");
        }
        let mut without = String::new();
        render_start_here_top_gap(&mut without, Some(&start_here));
        if without.contains("start repair") || without.contains("agent repair") {
            return Err(format!("no repair start without the field:\n{without}"));
        }
        if without.contains(&format!("- {}: ", REPAIR_AFTER_PHASE_LABEL.to_lowercase()))
            || without.contains("without a repair attempt")
            || !without.contains(&format!("- {}: `", VERIFY_AFTER_EDIT_LABEL.to_lowercase()))
            || !without.contains(&format!(
                "- {}: `",
                RECEIPT_AFTER_VERIFY_LABEL.to_lowercase()
            ))
        {
            return Err(format!(
                "without a start, verify and receipt run after the test edit:\n{without}"
            ));
        }
        Ok(())
    }

    /// The redirect command from #4950's reproduction: a `>` redirect whose
    /// bash form writes UTF-16 under Windows PowerShell 5.1, so the twin is
    /// the guarded BOM-free UTF-8 write.
    const REDIRECT_COMMAND: &str = "ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/repo-exposure.json";

    fn missing_artifact_start_here(command: &str) -> Value {
        serde_json::json!({
            "selected": {
                "state": "missing_artifact",
                "artifact": {"path": "target/ripr/reports/repo-exposure.json"},
                "regeneration_command": command
            }
        })
    }

    fn blocked_start_here(command: &str) -> Value {
        serde_json::json!({
            "selected": {
                "state": "blocked_artifact",
                "message": "first-run packet is blocked by unavailable evidence",
                "next_command": command
            }
        })
    }

    fn assert_redirect_pair(out: &str) -> Result<(), String> {
        let bash = out
            .find(&format!("- next command: `{REDIRECT_COMMAND}`\n"))
            .ok_or_else(|| format!("bash next command drifted:\n{out}"))?;
        let twin = out
            .find("- next command (PowerShell): `")
            .ok_or_else(|| format!("powershell twin missing:\n{out}"))?;
        if !out.contains("WriteAllText") {
            return Err(format!("twin must be the guarded UTF-8 write:\n{out}"));
        }
        if bash > twin {
            return Err(format!("bash form must come before the twin:\n{out}"));
        }
        Ok(())
    }

    /// #4950: the missing-artifact Start Here next command pairs its bash
    /// form with the shared PowerShell translation, like first-pr's pairing
    /// of the same `regeneration_command` field.
    #[test]
    fn start_here_missing_next_command_pairs_redirect_with_powershell_twin() -> Result<(), String> {
        let mut out = String::new();
        render_start_here_missing(
            &mut out,
            Some(&missing_artifact_start_here(REDIRECT_COMMAND)),
        );
        assert_redirect_pair(&out)
    }

    /// #4950: the blocked Start Here next command pairs the same way for the
    /// same `next_command` field first-pr pairs.
    #[test]
    fn start_here_blocked_next_command_pairs_redirect_with_powershell_twin() -> Result<(), String> {
        let mut out = String::new();
        render_start_here_blocked(&mut out, Some(&blocked_start_here(REDIRECT_COMMAND)));
        assert_redirect_pair(&out)
    }

    /// #4950: for the same packet, pr-summary's Start Here carries exactly
    /// the PowerShell pairing first-pr renders for the same JSON fields —
    /// counted, so the surfaces cannot drift apart silently in either
    /// direction.
    #[test]
    fn start_here_next_command_powershell_count_matches_first_pr_pairing() -> Result<(), String> {
        for packet in [
            missing_artifact_start_here(REDIRECT_COMMAND),
            blocked_start_here(REDIRECT_COMMAND),
        ] {
            let mut summary = String::new();
            match packet.pointer("/selected/state").and_then(Value::as_str) {
                Some("missing_artifact") => {
                    render_start_here_missing(&mut summary, Some(&packet));
                }
                _ => render_start_here_blocked(&mut summary, Some(&packet)),
            }
            let first_pr = crate::output::first_pr::first_pr_start_here_markdown(&packet);
            let count = |text: &str| text.matches("(PowerShell)").count();
            if count(&summary) != count(&first_pr) {
                return Err(format!(
                    "PowerShell pairing count must match first-pr for the same packet:\n\
                     pr-summary:\n{summary}\nfirst-pr:\n{first_pr}"
                ));
            }
            if count(&summary) != 1 {
                return Err(format!(
                    "a redirect command pairs exactly one PowerShell twin:\n\
                     pr-summary:\n{summary}\nfirst-pr:\n{first_pr}"
                ));
            }
        }
        Ok(())
    }

    /// #4950 negative control: a command PowerShell runs unchanged gains no
    /// twin — the bash bytes stay the only command form — and the
    /// runs-unchanged note is the only added line.
    #[test]
    fn start_here_plain_next_command_gains_no_powershell_twin() -> Result<(), String> {
        let bash = "ripr check --root . --mode instant --format repo-exposure-json";
        let mut out = String::new();
        render_start_here_missing(&mut out, Some(&missing_artifact_start_here(bash)));
        if !out.contains(&format!("- next command: `{bash}`\n")) {
            return Err(format!("bash next command drifted:\n{out}"));
        }
        if out.contains("- next command (PowerShell)") {
            return Err(format!(
                "an unchanged command must not gain a PowerShell twin:\n{out}"
            ));
        }
        if !out.contains(
            "- next command runs unchanged in Bash and PowerShell; cmd.exe is not supported.\n",
        ) {
            return Err(format!("unchanged note missing:\n{out}"));
        }
        Ok(())
    }

    /// #4950 negative control: a compound command under-emits to the shared
    /// availability disclosure instead of an invalid or invented translation.
    #[test]
    fn start_here_compound_next_command_discloses_unavailable_form() -> Result<(), String> {
        let bash =
            "ripr check --root . --json > check.json && ripr reports gap-ledger --out ledger.json";
        let mut out = String::new();
        render_start_here_blocked(&mut out, Some(&blocked_start_here(bash)));
        if !out.contains(&format!("- next command: `{bash}`\n")) {
            return Err(format!("bash next command drifted:\n{out}"));
        }
        let disclosure = format!(
            "- {}: `{bash}`\n",
            crate::output::markdown::POWERSHELL_UNAVAILABLE_DISCLOSURE
        );
        if !out.contains(&disclosure) {
            return Err(format!("compound disclosure missing:\n{out}"));
        }
        if out.contains("- next command (PowerShell)") {
            return Err(format!(
                "a compound command must not gain a PowerShell twin:\n{out}"
            ));
        }
        Ok(())
    }

    /// #4950 negative control: a missing or empty command keeps its existing
    /// `not_available` line and gains no shell outcome at all.
    #[test]
    fn start_here_absent_next_command_keeps_not_available_without_powershell() -> Result<(), String>
    {
        for render in [
            render_start_here_missing as fn(&mut String, Option<&Value>),
            render_start_here_blocked,
        ] {
            let mut out = String::new();
            render(
                &mut out,
                Some(&serde_json::json!({
                    "selected": {"state": "missing_artifact"}
                })),
            );
            if !out.contains("- next command: `not_available`\n") {
                return Err(format!("absent field must keep its state line:\n{out}"));
            }
            if out.contains("PowerShell") {
                return Err(format!(
                    "absent command must gain no PowerShell lines:\n{out}"
                ));
            }
        }
        Ok(())
    }
}
