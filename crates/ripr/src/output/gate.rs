mod exception_policy;
mod input;
mod model;
mod presentation;

use super::gap_decision_ledger::{self, GapRecord};
#[cfg(test)]
use input::baseline_index_from_value;
use model::*;
pub(crate) use model::{GateEvaluateInput, GateMode};
pub(crate) use presentation::{
    gate_decision_should_fail, gate_decision_status, markdown_path_for, render_gate_decision_json,
    render_gate_decision_markdown,
};
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const DEFAULT_GATE_OUT: &str = "target/ripr/reports/gate-decision.json";
const SCHEMA_VERSION: &str = "0.1";
const DEFAULT_THRESHOLD: &str = "high_confidence_new_gap";
const DEFAULT_ACKNOWLEDGEMENT_LABEL: &str = "ripr-waive";
const LIMITS_NOTE: &str = "Optional policy over static RIPR evidence; advisory by default; runtime mutation calibration is used only when supplied.";

pub(crate) fn build_gate_decision_report(
    input: &GateEvaluateInput,
) -> Result<GateDecisionReport, String> {
    let mut warnings = Vec::new();
    let mut config_errors = Vec::new();
    let labels = read_labels(input, &mut warnings)?;
    if input.pr_guidance.is_none() && input.gap_ledger.is_none() {
        config_errors
            .push("gate evaluate requires --pr-guidance <path> or --gap-ledger <path>".to_string());
    }
    let pr_guidance = match input.pr_guidance.as_ref() {
        Some(path) => {
            let pr_guidance_path = resolve_root_path(&input.root, path);
            match read_json_value_with_display(&pr_guidance_path, path) {
                Ok(value) => {
                    if let Some(defect) = pr_guidance_document_defect(&value) {
                        config_errors.push(format!(
                            "pr-guidance {} is not a recognized review-comments guidance document: {defect}",
                            display_path(path)
                        ));
                        Value::Null
                    } else if let Some(producer_error) = pr_guidance_producer_error(&value, path) {
                        // The document is structurally valid but the producer
                        // disclosed it did not complete (status=error/incomplete/
                        // timeout, or a tool_error/timeout warning kind).  Treat
                        // as config_error so the gate fails-closed — exactly as
                        // malformed JSON does — rather than deriving `pass` from
                        // zero candidates.
                        config_errors.push(producer_error);
                        Value::Null
                    } else {
                        value
                    }
                }
                Err(error) => {
                    config_errors.push(format!(
                        "required PR guidance input {} is invalid: {error}",
                        display_path(path)
                    ));
                    Value::Null
                }
            }
        }
        None => Value::Null,
    };
    let gap_ledger = read_gap_ledger(input, &mut config_errors);
    warn_for_optional_json(
        &input.root,
        input.repo_exposure.as_ref(),
        "repo_exposure",
        &mut warnings,
    );
    warn_for_optional_json(
        &input.root,
        input.sarif_policy.as_ref(),
        "sarif_policy",
        &mut warnings,
    );
    warn_for_optional_json(
        &input.root,
        input.agent_verify.as_ref(),
        "agent_verify",
        &mut warnings,
    );
    warn_for_optional_json(
        &input.root,
        input.agent_receipt.as_ref(),
        "agent_receipt",
        &mut warnings,
    );
    warn_for_optional_json(
        &input.root,
        input.mutation_calibration.as_ref(),
        "mutation_calibration",
        &mut warnings,
    );

    let recommendation_calibration = read_recommendation_calibration(input, &mut warnings);
    let mutation_calibration = read_mutation_calibration(input, &mut warnings);
    let baseline = read_baseline(input, &mut warnings, &mut config_errors);
    // #1442: an explicitly requested exception ledger fails closed — a
    // missing or malformed ledger is a config_error, never a silently
    // ignored input. Warn-severity violations (past-due review under
    // `due_review = "warn"`) surface through the shared warnings list.
    let exception_policy = match input.exception_policy.as_ref() {
        Some(path) => {
            let resolved = resolve_root_path(&input.root, path);
            let display = display_path(path);
            match exception_policy::load_exception_ledger(&resolved, &display) {
                Ok(ledger) => {
                    let today = crate::output::suppressions::current_iso_date();
                    let report =
                        exception_policy::evaluate_exception_ledger(&ledger, &today, &display);
                    warnings.extend(report.warning_details());
                    Some(report)
                }
                Err(error) => {
                    config_errors.push(error);
                    None
                }
            }
        }
        None => None,
    };
    let candidates = if config_errors.is_empty() {
        if let Some(records) = gap_ledger.as_ref() {
            candidates_from_gap_ledger(records)
        } else {
            candidates_from_pr_guidance(&pr_guidance)
        }
    } else {
        Vec::new()
    };
    let policy = GatePolicy {
        mode: input.mode,
        threshold: DEFAULT_THRESHOLD.to_string(),
        acknowledgement_labels: acknowledgement_labels(input),
        default_workflow_posture: "advisory".to_string(),
    };
    let mut decisions = candidates
        .iter()
        .map(|candidate| {
            gate_decision(
                candidate,
                &policy,
                &labels,
                &recommendation_calibration,
                &mutation_calibration,
                &baseline,
            )
        })
        .collect::<Vec<_>>();
    decisions.sort_by(|left, right| left.id.cmp(&right.id));
    let summary = summarize_decisions(&decisions);
    let new_unsuppressed = compute_new_unsuppressed(&decisions, &config_errors, input.mode);
    let exception_blocking = exception_policy
        .as_ref()
        .map(|report| report.blocking_count())
        .unwrap_or(0);
    let status = top_level_status(
        &summary,
        &warnings,
        &config_errors,
        input.mode,
        exception_blocking,
    )
    .to_string();
    Ok(GateDecisionReport {
        status,
        mode: input.mode,
        root: display_path(&input.root),
        inputs: GateDecisionInputs {
            repo_exposure: input.repo_exposure.as_ref().map(|path| display_path(path)),
            pr_guidance: input.pr_guidance.as_ref().map(|path| display_path(path)),
            gap_ledger: input.gap_ledger.as_ref().map(|path| display_path(path)),
            sarif_policy: input.sarif_policy.as_ref().map(|path| display_path(path)),
            labels_json: input.labels_json.as_ref().map(|path| display_path(path)),
            labels,
            agent_verify: input.agent_verify.as_ref().map(|path| display_path(path)),
            agent_receipt: input.agent_receipt.as_ref().map(|path| display_path(path)),
            recommendation_calibration: input
                .recommendation_calibration
                .as_ref()
                .map(|path| display_path(path)),
            mutation_calibration: input
                .mutation_calibration
                .as_ref()
                .map(|path| display_path(path)),
            baseline: input.baseline.as_ref().map(|path| display_path(path)),
            exception_policy: input
                .exception_policy
                .as_ref()
                .map(|path| display_path(path)),
        },
        policy,
        summary,
        new_unsuppressed,
        decisions,
        warnings,
        config_errors,
        exception_policy,
    })
}

fn read_labels(
    input: &GateEvaluateInput,
    warnings: &mut Vec<String>,
) -> Result<Vec<String>, String> {
    Ok(input::read_labels_impl(input, warnings))
}

fn warn_for_optional_json(
    root: &Path,
    path: Option<&PathBuf>,
    name: &str,
    warnings: &mut Vec<String>,
) {
    input::warn_for_optional_json_impl(root, path, name, warnings);
}

fn read_gap_ledger(
    input: &GateEvaluateInput,
    config_errors: &mut Vec<String>,
) -> Option<Vec<GapRecord>> {
    input::read_gap_ledger_impl(input, config_errors)
}

fn read_recommendation_calibration(
    input: &GateEvaluateInput,
    warnings: &mut Vec<String>,
) -> CalibrationIndex {
    input::read_recommendation_calibration_impl(input, warnings)
}

fn read_mutation_calibration(
    input: &GateEvaluateInput,
    warnings: &mut Vec<String>,
) -> CalibrationIndex {
    input::read_mutation_calibration_impl(input, warnings)
}

fn read_baseline(
    input: &GateEvaluateInput,
    warnings: &mut Vec<String>,
    config_errors: &mut Vec<String>,
) -> BaselineIndex {
    input::read_baseline_impl(input, warnings, config_errors)
}

fn candidates_from_pr_guidance(value: &Value) -> Vec<GateCandidate> {
    let nearby_test_changed = value
        .pointer("/summary/unchanged_tests")
        .and_then(Value::as_bool)
        .map(|unchanged| !unchanged)
        .unwrap_or(false);
    let mut candidates = Vec::new();
    for source in ["comments", "summary_only"] {
        for item in value
            .get(source)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            candidates.push(candidate_from_guidance_item(
                source,
                item,
                nearby_test_changed,
                false,
            ));
        }
    }
    for item in value
        .get("suppressed")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        candidates.push(candidate_from_guidance_item(
            "suppressed",
            item,
            nearby_test_changed,
            true,
        ));
    }
    candidates
}

fn candidates_from_gap_ledger(records: &[GapRecord]) -> Vec<GateCandidate> {
    records.iter().map(candidate_from_gap_record).collect()
}

fn candidate_from_guidance_item(
    source: &str,
    item: &Value,
    nearby_test_changed: bool,
    suppressed: bool,
) -> GateCandidate {
    let source_id = item
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| item.get("dedupe_key").and_then(Value::as_str))
        .or_else(|| item.get("seam_id").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string();
    let placement = GatePlacement {
        path: string_field(item.pointer("/placement/path"))
            .or_else(|| string_field(item.pointer("/seam/file"))),
        line: item
            .pointer("/placement/line")
            .and_then(Value::as_u64)
            .or_else(|| item.pointer("/seam/line").and_then(Value::as_u64)),
    };
    let recommended_file = item
        .pointer("/suggested_test/recommended_file")
        .and_then(Value::as_str);
    let near_test = item
        .pointer("/suggested_test/near_test")
        .and_then(Value::as_str);
    let recommended_test = match (recommended_file, near_test) {
        (Some(file), Some(test)) => Some(format!("{file}::{test}")),
        (Some(file), None) => Some(file.to_string()),
        (None, Some(test)) => Some(test.to_string()),
        (None, None) => None,
    };
    let suppression_reason = item
        .get("reason")
        .and_then(Value::as_str)
        .or_else(|| item.get("suppression_reason").and_then(Value::as_str))
        .map(ToOwned::to_owned);
    let summary_reason = string_field(item.get("summary_reason"));
    GateCandidate {
        source: source.to_string(),
        source_id,
        gap_id: None,
        gap_kind: None,
        canonical_gap_id: canonical_gap_id_from_value(item),
        seam_id: string_field(item.get("seam_id")),
        static_class: string_field(item.get("grip_class"))
            .or_else(|| string_field(item.get("class"))),
        severity: string_field(item.get("severity")),
        placement,
        missing_discriminator: string_field(item.get("missing_discriminator")),
        assertion_shape: string_field(item.pointer("/suggested_test/assertion_shape")),
        candidate_values: item
            .pointer("/suggested_test/candidate_values")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        recommended_test,
        repair_route: None,
        verification_commands: Vec::new(),
        nearby_test_changed,
        suppressed,
        configured_off: suppression_reason.as_deref() == Some("severity_off"),
        suppression_reason,
        summary_reason,
        gap_ledger_gate_candidate: false,
        gap_ledger_gate_reason: None,
        gap_ledger_safe_gate_predicate: false,
    }
}

fn candidate_from_gap_record(record: &GapRecord) -> GateCandidate {
    let gap_id = non_empty_string(&record.gap_id);
    let canonical_gap_id = non_empty_string(&record.canonical_gap_id);
    let source_id = gap_id
        .clone()
        .or_else(|| canonical_gap_id.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let projection = record.projection_eligibility.get("gate_candidate");
    let gate_candidate = projection.is_some_and(|projection| projection.eligible);
    let gate_reason = projection
        .and_then(|projection| non_empty_string(&projection.reason))
        .or_else(|| Some("not_gate_candidate".to_string()));
    let repair_route = record.repair_route.clone();
    let changed_behavior = repair_route
        .as_ref()
        .and_then(|route| route.changed_behavior.clone());
    let assertion_shape = repair_route
        .as_ref()
        .and_then(|route| route.assertion_shape.clone());
    let recommended_test = repair_route
        .as_ref()
        .and_then(|route| route.related_test.clone())
        .or_else(|| {
            repair_route
                .as_ref()
                .and_then(|route| route.target_file.clone())
        });
    let placement = GatePlacement {
        path: record
            .anchor
            .as_ref()
            .and_then(|anchor| anchor.file.clone()),
        line: record.anchor.as_ref().and_then(|anchor| anchor.line),
    };
    GateCandidate {
        source: "gap_decision_ledger".to_string(),
        source_id,
        gap_id,
        gap_kind: non_empty_string(&record.kind),
        canonical_gap_id,
        seam_id: None,
        static_class: non_empty_string(&record.kind),
        severity: Some("warning".to_string()),
        placement,
        missing_discriminator: changed_behavior.clone(),
        assertion_shape,
        candidate_values: Vec::new(),
        recommended_test,
        repair_route,
        verification_commands: record.verification_commands.clone(),
        nearby_test_changed: false,
        suppressed: record.policy_state == "suppressed"
            || record
                .safe_gate_predicate
                .as_ref()
                .is_some_and(|predicate| predicate.suppressed),
        configured_off: record.policy_state == "not_policy_targeted",
        suppression_reason: (record.policy_state == "suppressed").then(|| "suppressed".to_string()),
        summary_reason: None,
        gap_ledger_gate_candidate: gate_candidate,
        gap_ledger_gate_reason: gate_reason,
        gap_ledger_safe_gate_predicate: gap_decision_ledger::safe_gate_predicate_satisfied(record),
    }
}

fn gate_decision(
    candidate: &GateCandidate,
    policy: &GatePolicy,
    labels: &[String],
    recommendation_calibration: &CalibrationIndex,
    mutation_calibration: &CalibrationIndex,
    baseline: &BaselineIndex,
) -> GateDecision {
    let recommendation_calibration =
        calibration_for_candidate(candidate, recommendation_calibration);
    let mutation_calibration = calibration_for_candidate(candidate, mutation_calibration);
    let eligible = candidate_is_policy_eligible(candidate);
    let identity_candidates = baseline_identity_candidates(candidate);
    let baseline_identity = identity_candidates.first().cloned();
    let is_baseline_new = !identity_candidates
        .iter()
        .any(|identity| baseline.identities.contains(identity));
    let acknowledgement_label = acknowledgement_label(policy, labels);
    let would_block = candidate_would_block(
        candidate,
        policy.mode,
        eligible,
        is_baseline_new,
        &recommendation_calibration,
        &mutation_calibration,
    );
    let decision = if candidate.suppressed || candidate.configured_off {
        "suppressed"
    } else if !eligible
        && (candidate.static_class.is_none() || candidate.source == "gap_decision_ledger")
    {
        "not_applicable"
    } else if would_block && acknowledgement_label.is_some() {
        "acknowledged"
    } else if would_block {
        "blocking"
    } else {
        "advisory"
    }
    .to_string();
    let gate_reason = gate_reason(
        candidate,
        GateReasonContext {
            mode: policy.mode,
            decision: &decision,
            eligible,
            is_baseline_new,
            recommendation_calibration: &recommendation_calibration,
            mutation_calibration: &mutation_calibration,
            acknowledgement_label: acknowledgement_label.as_deref(),
        },
    );
    GateDecision {
        id: format!("ripr-gate-{}", stable_identity(candidate)),
        source: if candidate.source == "summary_only" {
            "pr_guidance_summary".to_string()
        } else if candidate.source == "gap_decision_ledger" {
            "gap_decision_ledger".to_string()
        } else {
            "pr_guidance".to_string()
        },
        decision,
        gate_reason,
        gap_id: candidate.gap_id.clone(),
        gap_kind: candidate.gap_kind.clone(),
        canonical_gap_id: candidate.canonical_gap_id.clone(),
        seam_id: candidate.seam_id.clone(),
        source_id: candidate.source_id.clone(),
        static_class: candidate.static_class.clone(),
        severity: candidate.severity.clone(),
        placement: candidate.placement.clone(),
        policy: GateDecisionPolicy {
            mode: policy.mode,
            threshold: policy.threshold.clone(),
            acknowledgement_label,
            baseline_identity,
        },
        evidence: GateEvidence {
            missing_discriminator: candidate.missing_discriminator.clone(),
            assertion_shape: candidate.assertion_shape.clone(),
            candidate_values: candidate.candidate_values.clone(),
            recommended_test: candidate.recommended_test.clone(),
            repair_route: candidate.repair_route.clone(),
            verification_commands: candidate.verification_commands.clone(),
            nearby_test_changed: candidate.nearby_test_changed,
            suppressed: candidate.suppressed,
            configured_off: candidate.configured_off,
            recommendation_calibration,
            mutation_calibration,
        },
        is_baseline_new,
    }
}

fn calibration_for_candidate(
    candidate: &GateCandidate,
    calibration: &CalibrationIndex,
) -> CalibrationEvidence {
    candidate
        .seam_id
        .as_ref()
        .and_then(|seam_id| calibration.by_seam_id.get(seam_id))
        .or_else(|| calibration.by_source_id.get(&candidate.source_id))
        .cloned()
        .unwrap_or_else(|| CalibrationEvidence {
            available: false,
            outcome: None,
            confidence_effect: "not_used".to_string(),
        })
}

fn candidate_is_policy_eligible(candidate: &GateCandidate) -> bool {
    if candidate.source == "gap_decision_ledger" {
        return !candidate.suppressed
            && !candidate.configured_off
            && candidate.gap_ledger_gate_candidate
            && candidate.gap_ledger_safe_gate_predicate
            && candidate.repair_route.is_some()
            && !candidate.verification_commands.is_empty()
            && candidate.placement.path.is_some()
            && candidate.placement.line.is_some();
    }
    // A `summary_only` item demoted solely because the inline display cap was
    // reached (`summary_reason=inline_comment_cap_reached`) retains a valid
    // changed-line placement and concrete guidance — it is block-eligible just
    // as it would be if it had landed in the `comments` array.  Items demoted
    // for `no_safe_changed_line_placement` or
    // `navigation_only_cross_language_target` lack a safe placement anchor and
    // remain advisory-only.
    let demoted_by_cap = candidate.source == "summary_only"
        && candidate.summary_reason.as_deref() == Some("inline_comment_cap_reached");
    let placement_excluded = candidate.source == "summary_only" && !demoted_by_cap;
    !candidate.suppressed
        && !candidate.configured_off
        && candidate_class_is_policy_eligible(candidate.static_class.as_deref())
        && has_concrete_guidance(candidate)
        && !candidate.nearby_test_changed
        && candidate.placement.path.is_some()
        && candidate.placement.line.is_some()
        && !placement_excluded
}

fn candidate_class_is_policy_eligible(class: Option<&str>) -> bool {
    matches!(
        class,
        Some("weakly_gripped" | "ungripped" | "reachable_unrevealed" | "weakly_exposed")
    )
}

fn has_concrete_guidance(candidate: &GateCandidate) -> bool {
    candidate.missing_discriminator.is_some()
        || candidate.assertion_shape.is_some()
        || !candidate.candidate_values.is_empty()
        || candidate.recommended_test.is_some()
}

fn baseline_identity(candidate: &GateCandidate) -> Option<String> {
    baseline_identity_candidates(candidate).into_iter().next()
}

/// Candidate identities are ordered from the canonical domain identity through
/// legacy selectors. New output prefers the first value, while baseline
/// comparison accepts any historical selector so adding canonical identity to
/// a review card does not reclassify existing reviewed debt as new.
fn baseline_identity_candidates(candidate: &GateCandidate) -> Vec<String> {
    let mut identities = Vec::new();
    let fallback = match (
        candidate.placement.path.as_deref(),
        candidate.placement.line,
    ) {
        (Some(path), Some(line)) => Some(format!(
            "{path}:{line}:{}",
            candidate.static_class.as_deref().unwrap_or("unknown")
        )),
        _ => None,
    };
    for identity in [
        candidate.canonical_gap_id.clone(),
        candidate.gap_id.clone(),
        candidate.seam_id.clone(),
        (!candidate.source_id.is_empty()).then(|| candidate.source_id.clone()),
        fallback,
    ]
    .into_iter()
    .flatten()
    {
        if !identities.contains(&identity) {
            identities.push(identity);
        }
    }
    identities
}

fn stable_identity(candidate: &GateCandidate) -> String {
    baseline_identity(candidate)
        .unwrap_or_else(|| candidate.source_id.clone())
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn acknowledgement_label(policy: &GatePolicy, labels: &[String]) -> Option<String> {
    policy
        .acknowledgement_labels
        .iter()
        .find(|label| labels.iter().any(|present| present == *label))
        .cloned()
}

fn candidate_would_block(
    candidate: &GateCandidate,
    mode: GateMode,
    eligible: bool,
    is_baseline_new: bool,
    recommendation_calibration: &CalibrationEvidence,
    mutation_calibration: &CalibrationEvidence,
) -> bool {
    if !eligible {
        return false;
    }
    match mode {
        GateMode::VisibleOnly => false,
        GateMode::Acknowledgeable => true,
        GateMode::BaselineCheck => is_baseline_new,
        GateMode::CalibratedGate => {
            is_baseline_new
                && (recommendation_calibration.confidence_effect == "supports_static_gap"
                    || mutation_calibration.confidence_effect == "supports_static_gap")
                && candidate.severity.as_deref() == Some("warning")
        }
    }
}

fn gate_reason(candidate: &GateCandidate, context: GateReasonContext<'_>) -> String {
    if candidate.suppressed || candidate.configured_off {
        return format!(
            "configured-hidden or suppressed candidate preserved as `{}`",
            candidate
                .suppression_reason
                .as_deref()
                .unwrap_or("suppressed")
        );
    }
    if !context.eligible {
        if candidate.source == "gap_decision_ledger" {
            if !candidate.gap_ledger_gate_candidate {
                return format!(
                    "gap decision ledger record is not gate-candidate eligible: {}",
                    candidate
                        .gap_ledger_gate_reason
                        .as_deref()
                        .unwrap_or("not_gate_candidate")
                );
            }
            if !candidate.gap_ledger_safe_gate_predicate {
                return "gap decision ledger record does not satisfy the safe gate predicate"
                    .to_string();
            }
            if candidate.repair_route.is_none() || candidate.verification_commands.is_empty() {
                return "gap decision ledger record is missing repair route or verification command"
                    .to_string();
            }
            if candidate.placement.path.is_none() || candidate.placement.line.is_none() {
                return "gap decision ledger record is missing a stable file and line anchor"
                    .to_string();
            }
        }
        if candidate.source == "summary_only" {
            return "summary-only recommendation remains visible and advisory".to_string();
        }
        if candidate.nearby_test_changed {
            return "nearby focused test changed in this PR, so the candidate stays advisory"
                .to_string();
        }
        if !has_concrete_guidance(candidate) {
            return "candidate is missing concrete focused-test guidance".to_string();
        }
        return "candidate is outside the initial policy-eligible class or placement scope"
            .to_string();
    }
    match context.decision {
        "acknowledged" => format!(
            "policy-eligible gap acknowledged by {}",
            context
                .acknowledgement_label
                .unwrap_or(DEFAULT_ACKNOWLEDGEMENT_LABEL)
        ),
        "blocking"
            if candidate.source == "gap_decision_ledger"
                && context.mode == GateMode::BaselineCheck
                && context.is_baseline_new =>
        {
            format!(
                "new repairable {} gap blocks under baseline-check from gap decision ledger",
                candidate.gap_kind.as_deref().unwrap_or("Rust")
            )
        }
        "blocking" if context.mode == GateMode::BaselineCheck && context.is_baseline_new => {
            "new policy-eligible gap blocks under baseline-check".to_string()
        }
        "blocking" if candidate.source == "gap_decision_ledger" => format!(
            "new repairable {} gap blocks from gap decision ledger",
            candidate.gap_kind.as_deref().unwrap_or("Rust")
        ),
        "blocking" if context.mode == GateMode::CalibratedGate => {
            if context.mutation_calibration.confidence_effect == "supports_static_gap" {
                "new policy-eligible gap has supporting imported mutation calibration".to_string()
            } else {
                "new policy-eligible gap has supporting recommendation calibration".to_string()
            }
        }
        "blocking" => "policy-eligible gap blocks under acknowledgeable mode".to_string(),
        _ if context.mode == GateMode::VisibleOnly => {
            "visible-only mode records evidence without blocking".to_string()
        }
        _ if !context.is_baseline_new => {
            "candidate identity is already present in the explicit baseline".to_string()
        }
        _ if context.recommendation_calibration.available
            && context.recommendation_calibration.confidence_effect == "keeps_advisory" =>
        {
            "recommendation calibration keeps this candidate advisory".to_string()
        }
        _ if context.mutation_calibration.available
            && context.mutation_calibration.confidence_effect == "keeps_advisory" =>
        {
            "imported mutation calibration keeps this candidate advisory".to_string()
        }
        _ => "candidate remains advisory under current policy inputs".to_string(),
    }
}

fn summarize_decisions(decisions: &[GateDecision]) -> GateSummary {
    let mut summary = GateSummary {
        evaluated: decisions.len(),
        ..GateSummary::default()
    };
    for decision in decisions {
        match decision.decision.as_str() {
            "blocking" => summary.blocking += 1,
            "acknowledged" => summary.acknowledged += 1,
            "advisory" => summary.advisory += 1,
            "suppressed" => summary.suppressed += 1,
            "not_applicable" => summary.not_applicable += 1,
            _ => {}
        }
        if decision.decision == "advisory"
            && decision
                .evidence
                .recommendation_calibration
                .confidence_effect
                == "not_used"
            && candidate_class_is_policy_eligible(decision.static_class.as_deref())
        {
            summary.unknown_confidence += 1;
        }
    }
    summary
}

/// Compute the `new_unsuppressed` receipt field: a canonical count of
/// policy-eligible, unhandled decisions that a downstream thresholder can use
/// as a single stable number (`max_new_unsuppressed=0`).
///
/// Fail-closed: when `config_errors` is non-empty, analysis did not run and we
/// return `basis=None, count=0, reason=<first error>` rather than a zero that
/// looks like a clean pass.
fn compute_new_unsuppressed(
    decisions: &[GateDecision],
    config_errors: &[String],
    mode: GateMode,
) -> NewUnsuppressed {
    // Fail-closed: analysis did not run.
    if let Some(first_error) = config_errors.first() {
        return NewUnsuppressed {
            basis: None,
            count: 0,
            reason: Some(format!("analysis did not run: {first_error}")),
        };
    }
    // Determine basis: baseline-aware modes that actually have a baseline use
    // "baseline"; everything else is "diff" (all surviving candidates are new).
    let use_baseline_basis = matches!(mode, GateMode::BaselineCheck | GateMode::CalibratedGate);
    let basis = if use_baseline_basis {
        "baseline"
    } else {
        "diff"
    };
    // Count: decisions d where:
    //   - candidate_class_is_policy_eligible(d.static_class) is true, AND
    //   - d.decision ∈ {"blocking", "advisory"} (suppressed/acknowledged/not_applicable excluded), AND
    //   - if basis=="baseline": d.is_baseline_new is true.
    let count = decisions
        .iter()
        .filter(|d| {
            candidate_class_is_policy_eligible(d.static_class.as_deref())
                && matches!(d.decision.as_str(), "blocking" | "advisory")
                && (!use_baseline_basis || d.is_baseline_new)
        })
        .count();
    NewUnsuppressed {
        basis: Some(basis.to_string()),
        count: count as u64,
        reason: None,
    }
}

fn top_level_status(
    summary: &GateSummary,
    warnings: &[String],
    config_errors: &[String],
    mode: GateMode,
    exception_blocking: usize,
) -> &'static str {
    if !config_errors.is_empty() {
        "config_error"
    } else if summary.blocking > 0 || exception_blocking > 0 {
        "blocked"
    } else if summary.acknowledged > 0 {
        "acknowledged"
    } else if mode == GateMode::VisibleOnly
        || summary.advisory > 0
        || summary.suppressed > 0
        || summary.unknown_confidence > 0
        || !warnings.is_empty()
    {
        "advisory"
    } else {
        "pass"
    }
}

fn acknowledgement_labels(input: &GateEvaluateInput) -> Vec<String> {
    if input.acknowledgement_labels.is_empty() {
        vec![DEFAULT_ACKNOWLEDGEMENT_LABEL.to_string()]
    } else {
        input.acknowledgement_labels.clone()
    }
}

fn read_json_value_with_display(path: &Path, display: &Path) -> Result<Value, String> {
    let display = display_path(display);
    let text = fs::read_to_string(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            format!("read {display} failed: not found")
        } else {
            format!("read {display} failed: {err}")
        }
    })?;
    serde_json::from_str(&text).map_err(|err| format!("parse {display} failed: {err}"))
}

fn string_field(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn non_empty_string(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_string())
}

fn canonical_gap_id_from_value(value: &Value) -> Option<String> {
    string_field(value.get("canonical_gap_id"))
        .or_else(|| string_field(value.pointer("/identity/canonical_gap_id")))
        .or_else(|| string_field(value.pointer("/evidence_record/canonical_gap_id")))
}

/// Returns `Some(defect_description)` if `value` is not a recognized
/// `ripr review-comments` guidance document, or `None` if it is valid.
///
/// A valid guidance document must have:
/// - `schema_version`: a non-empty string (the ripr schema marker)
/// - `comments`: a JSON array (the primary findings list consumed by the gate)
///
/// A valid document with an empty `comments` array is accepted — that is a
/// legitimate "zero findings" result and must still produce `status=advisory`.
fn pr_guidance_document_defect(value: &Value) -> Option<String> {
    let has_schema_version = value
        .get("schema_version")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty());
    let has_comments_array = value.get("comments").is_some_and(|v| v.is_array());
    match (has_schema_version, has_comments_array) {
        (false, false) => {
            Some("missing required fields `schema_version` and `comments`".to_string())
        }
        (false, true) => Some("missing required field `schema_version`".to_string()),
        (true, false) => {
            Some("missing required field `comments` (expected a JSON array)".to_string())
        }
        (true, true) => None,
    }
}

/// Returns `Some(error_message)` when a structurally-valid guidance document
/// discloses that the producer did not complete successfully.
///
/// The producer writes `status: "error"` (or `"incomplete"` / `"timeout"`) and
/// populates `warnings` with a `tool_error` or `timeout` entry when it crashes
/// or times out.  The document is schema-valid (has `schema_version` + `comments`
/// array) but its `comments` array is empty *because the producer failed*, not
/// because there genuinely are no gaps.  If the gate consumed it without
/// inspecting `status` it would derive `pass` from zero candidates — a
/// fake-clean.
///
/// Matching any one of these signals is sufficient to fail-close:
/// - top-level `status` ∈ { "error", "incomplete", "timeout" }
/// - any entry in top-level `warnings` array with `kind` ∈ { "tool_error", "timeout" }
///
/// A `status` of `"advisory"` or `"complete"` with no error-kind warnings is
/// treated as a healthy producer run and returns `None`.
fn pr_guidance_producer_error(value: &Value, path: &Path) -> Option<String> {
    let status = value.get("status").and_then(Value::as_str).unwrap_or("");
    let error_status = matches!(status, "error" | "incomplete" | "timeout");

    let warning_error = value
        .get("warnings")
        .and_then(Value::as_array)
        .is_some_and(|warnings| {
            warnings.iter().any(|w| {
                matches!(
                    w.get("kind").and_then(Value::as_str),
                    Some("tool_error" | "timeout")
                )
            })
        });

    if error_status || warning_error {
        let reason = if error_status {
            format!("producer status={status:?}")
        } else {
            "producer warnings contain tool_error or timeout".to_string()
        };
        Some(format!(
            "pr-guidance {} discloses producer did not complete ({reason}); \
             gate fails-closed to prevent a fake-clean pass on zero candidates",
            display_path(path)
        ))
    } else {
        None
    }
}

fn resolve_root_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn display_path(path: &Path) -> String {
    let value = path.display().to_string().replace('\\', "/");
    if value.is_empty() {
        ".".to_string()
    } else {
        value.strip_prefix("./").unwrap_or(&value).to_string()
    }
}

#[cfg(test)]
mod tests;
