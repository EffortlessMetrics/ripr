use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: &str = "0.1";
pub(crate) const DEFAULT_GATE_OUT: &str = "target/ripr/reports/gate-decision.json";
const DEFAULT_THRESHOLD: &str = "high_confidence_new_gap";
const DEFAULT_ACKNOWLEDGEMENT_LABEL: &str = "ripr-waive";
const LIMITS_NOTE: &str = "Optional policy over static RIPR evidence; advisory by default; runtime mutation calibration is used only when supplied.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GateMode {
    VisibleOnly,
    Acknowledgeable,
    BaselineCheck,
    CalibratedGate,
}

impl GateMode {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "visible-only" => Ok(Self::VisibleOnly),
            "acknowledgeable" => Ok(Self::Acknowledgeable),
            "baseline-check" => Ok(Self::BaselineCheck),
            "calibrated-gate" => Ok(Self::CalibratedGate),
            other => Err(format!("unknown gate mode `{other}`")),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::VisibleOnly => "visible-only",
            Self::Acknowledgeable => "acknowledgeable",
            Self::BaselineCheck => "baseline-check",
            Self::CalibratedGate => "calibrated-gate",
        }
    }

    fn requires_baseline(self) -> bool {
        matches!(self, Self::BaselineCheck | Self::CalibratedGate)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GateEvaluateInput {
    pub(crate) root: PathBuf,
    pub(crate) repo_exposure: Option<PathBuf>,
    pub(crate) pr_guidance: PathBuf,
    pub(crate) sarif_policy: Option<PathBuf>,
    pub(crate) labels_json: Option<PathBuf>,
    pub(crate) labels: Vec<String>,
    pub(crate) agent_verify: Option<PathBuf>,
    pub(crate) agent_receipt: Option<PathBuf>,
    pub(crate) recommendation_calibration: Option<PathBuf>,
    pub(crate) mutation_calibration: Option<PathBuf>,
    pub(crate) baseline: Option<PathBuf>,
    pub(crate) mode: GateMode,
    pub(crate) acknowledgement_labels: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GateDecisionReport {
    status: String,
    mode: GateMode,
    root: String,
    inputs: GateDecisionInputs,
    policy: GatePolicy,
    summary: GateSummary,
    decisions: Vec<GateDecision>,
    warnings: Vec<String>,
    config_errors: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GateDecisionInputs {
    repo_exposure: Option<String>,
    pr_guidance: String,
    sarif_policy: Option<String>,
    labels_json: Option<String>,
    labels: Vec<String>,
    agent_verify: Option<String>,
    agent_receipt: Option<String>,
    recommendation_calibration: Option<String>,
    mutation_calibration: Option<String>,
    baseline: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GatePolicy {
    mode: GateMode,
    threshold: String,
    acknowledgement_labels: Vec<String>,
    default_workflow_posture: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GateSummary {
    evaluated: usize,
    blocking: usize,
    acknowledged: usize,
    advisory: usize,
    suppressed: usize,
    not_applicable: usize,
    unknown_confidence: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GateDecision {
    id: String,
    source: String,
    decision: String,
    gate_reason: String,
    seam_id: Option<String>,
    source_id: String,
    static_class: Option<String>,
    severity: Option<String>,
    placement: GatePlacement,
    policy: GateDecisionPolicy,
    evidence: GateEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GatePlacement {
    path: Option<String>,
    line: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GateDecisionPolicy {
    mode: GateMode,
    threshold: String,
    acknowledgement_label: Option<String>,
    baseline_identity: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GateEvidence {
    missing_discriminator: Option<String>,
    assertion_shape: Option<String>,
    candidate_values: Vec<String>,
    recommended_test: Option<String>,
    nearby_test_changed: bool,
    suppressed: bool,
    configured_off: bool,
    recommendation_calibration: CalibrationEvidence,
    mutation_calibration: CalibrationEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CalibrationEvidence {
    available: bool,
    outcome: Option<String>,
    confidence_effect: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GateCandidate {
    source: String,
    source_id: String,
    seam_id: Option<String>,
    static_class: Option<String>,
    severity: Option<String>,
    placement: GatePlacement,
    missing_discriminator: Option<String>,
    assertion_shape: Option<String>,
    candidate_values: Vec<String>,
    recommended_test: Option<String>,
    nearby_test_changed: bool,
    suppressed: bool,
    configured_off: bool,
    suppression_reason: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct GateReasonContext<'a> {
    mode: GateMode,
    decision: &'a str,
    eligible: bool,
    is_baseline_new: bool,
    recommendation_calibration: &'a CalibrationEvidence,
    mutation_calibration: &'a CalibrationEvidence,
    acknowledgement_label: Option<&'a str>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CalibrationIndex {
    by_source_id: BTreeMap<String, CalibrationEvidence>,
    by_seam_id: BTreeMap<String, CalibrationEvidence>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct BaselineIndex {
    identities: BTreeSet<String>,
}

pub(crate) fn build_gate_decision_report(
    input: &GateEvaluateInput,
) -> Result<GateDecisionReport, String> {
    let mut warnings = Vec::new();
    let mut config_errors = Vec::new();
    let labels = read_labels(input, &mut warnings)?;
    let pr_guidance_path = resolve_root_path(&input.root, &input.pr_guidance);
    let pr_guidance = match read_json_value(&pr_guidance_path) {
        Ok(value) => value,
        Err(error) => {
            config_errors.push(format!(
                "required PR guidance input {} is invalid: {error}",
                display_path(&input.pr_guidance)
            ));
            Value::Null
        }
    };
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
    let candidates = if config_errors.is_empty() {
        candidates_from_pr_guidance(&pr_guidance)
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
    let status = top_level_status(&summary, &warnings, &config_errors, input.mode).to_string();
    Ok(GateDecisionReport {
        status,
        mode: input.mode,
        root: display_path(&input.root),
        inputs: GateDecisionInputs {
            repo_exposure: input.repo_exposure.as_ref().map(|path| display_path(path)),
            pr_guidance: display_path(&input.pr_guidance),
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
        },
        policy,
        summary,
        decisions,
        warnings,
        config_errors,
    })
}

pub(crate) fn render_gate_decision_json(report: &GateDecisionReport) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "status": report.status,
        "mode": report.mode.as_str(),
        "root": report.root,
        "inputs": inputs_json(&report.inputs),
        "policy": policy_json(&report.policy),
        "summary": summary_json(&report.summary),
        "decisions": report.decisions.iter().map(decision_json).collect::<Vec<_>>(),
        "warnings": report.warnings,
        "config_errors": report.config_errors,
        "limits_note": LIMITS_NOTE,
    }))
    .map_err(|err| format!("failed to render gate decision JSON: {err}"))
}

pub(crate) fn render_gate_decision_markdown(report: &GateDecisionReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Gate Decision\n\n");
    out.push_str(&format!("Decision: {}\n", report.status));
    out.push_str(&format!("Mode: {}\n", report.mode.as_str()));
    out.push_str(&format!("Evaluated: {}\n", report.summary.evaluated));
    out.push_str(&format!("Blocking: {}\n", report.summary.blocking));
    out.push_str(&format!("Acknowledged: {}\n", report.summary.acknowledged));
    out.push_str(&format!("Advisory: {}\n\n", report.summary.advisory));

    push_decision_section(&mut out, "Blocking", &report.decisions, "blocking");
    push_decision_section(&mut out, "Acknowledged", &report.decisions, "acknowledged");
    push_decision_section(&mut out, "Advisory", &report.decisions, "advisory");
    push_decision_section(&mut out, "Suppressed", &report.decisions, "suppressed");
    push_decision_section(
        &mut out,
        "Not Applicable",
        &report.decisions,
        "not_applicable",
    );

    if !report.config_errors.is_empty() {
        out.push_str("## Config Errors\n\n");
        for error in &report.config_errors {
            out.push_str(&format!("- {}\n", md_escape(error)));
        }
        out.push('\n');
    }
    if !report.warnings.is_empty() {
        out.push_str("## Warnings\n\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {}\n", md_escape(warning)));
        }
        out.push('\n');
    }
    out.push_str("## Limits\n\n");
    out.push_str(LIMITS_NOTE);
    out.push('\n');
    out
}

pub(crate) fn gate_decision_should_fail(report: &GateDecisionReport) -> bool {
    matches!(report.status.as_str(), "blocked" | "config_error")
}

pub(crate) fn gate_decision_status(report: &GateDecisionReport) -> &str {
    &report.status
}

pub(crate) fn markdown_path_for(out: &Path) -> PathBuf {
    let mut path = out.to_path_buf();
    path.set_extension("md");
    path
}

fn read_labels(
    input: &GateEvaluateInput,
    warnings: &mut Vec<String>,
) -> Result<Vec<String>, String> {
    let mut labels = input
        .labels
        .iter()
        .filter(|label| !label.trim().is_empty())
        .cloned()
        .collect::<BTreeSet<_>>();
    if let Some(path) = &input.labels_json {
        let resolved = resolve_root_path(&input.root, path);
        match read_json_value(&resolved) {
            Ok(value) => {
                for label in labels_from_value(&value) {
                    labels.insert(label);
                }
            }
            Err(error) => warnings.push(format!(
                "optional labels_json {} is unavailable: {error}",
                display_path(path)
            )),
        }
    }
    Ok(labels.into_iter().collect())
}

fn labels_from_value(value: &Value) -> Vec<String> {
    if let Some(values) = value.as_array() {
        return values
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect();
    }
    value
        .get("labels")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn warn_for_optional_json(
    root: &Path,
    path: Option<&PathBuf>,
    name: &str,
    warnings: &mut Vec<String>,
) {
    let Some(path) = path else {
        return;
    };
    if let Err(error) = read_json_value(&resolve_root_path(root, path)) {
        warnings.push(format!(
            "optional {name} {} is unavailable: {error}",
            display_path(path)
        ));
    }
}

fn read_recommendation_calibration(
    input: &GateEvaluateInput,
    warnings: &mut Vec<String>,
) -> CalibrationIndex {
    let mut index = CalibrationIndex::default();
    let Some(path) = &input.recommendation_calibration else {
        return index;
    };
    let resolved = resolve_root_path(&input.root, path);
    let value = match read_json_value(&resolved) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(format!(
                "optional recommendation_calibration {} is unavailable: {error}",
                display_path(path)
            ));
            return index;
        }
    };
    for item in value
        .get("recommendations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let evidence = CalibrationEvidence {
            available: true,
            outcome: string_field(item.pointer("/calibration/outcome")),
            confidence_effect: recommendation_confidence_effect(
                item.pointer("/calibration/outcome").and_then(Value::as_str),
            )
            .to_string(),
        };
        if let Some(id) = item.get("id").and_then(Value::as_str) {
            index.by_source_id.insert(id.to_string(), evidence.clone());
        }
        if let Some(seam_id) = item.get("seam_id").and_then(Value::as_str) {
            index.by_seam_id.insert(seam_id.to_string(), evidence);
        }
    }
    index
}

fn read_mutation_calibration(
    input: &GateEvaluateInput,
    warnings: &mut Vec<String>,
) -> CalibrationIndex {
    let mut index = CalibrationIndex::default();
    let Some(path) = &input.mutation_calibration else {
        return index;
    };
    let resolved = resolve_root_path(&input.root, path);
    let value = match read_json_value(&resolved) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(format!(
                "optional mutation_calibration {} is unavailable: {error}",
                display_path(path)
            ));
            return index;
        }
    };
    for item in value
        .get("matches")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let seam_id = item
            .pointer("/static/seam_id")
            .and_then(Value::as_str)
            .or_else(|| item.pointer("/runtime/seam_id").and_then(Value::as_str));
        let Some(seam_id) = seam_id else {
            continue;
        };
        let outcome = item
            .pointer("/runtime/runtime_outcome")
            .and_then(Value::as_str)
            .or_else(|| item.pointer("/runtime/outcome").and_then(Value::as_str));
        index.by_seam_id.insert(
            seam_id.to_string(),
            CalibrationEvidence {
                available: true,
                outcome: outcome.map(ToOwned::to_owned),
                confidence_effect: mutation_confidence_effect(outcome).to_string(),
            },
        );
    }
    for item in value
        .get("static_only_findings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(seam_id) = item.pointer("/static/seam_id").and_then(Value::as_str) {
            index.by_seam_id.insert(
                seam_id.to_string(),
                CalibrationEvidence {
                    available: true,
                    outcome: Some("static_gap_without_runtime_signal".to_string()),
                    confidence_effect: "keeps_advisory".to_string(),
                },
            );
        }
    }
    if !value
        .get("ambiguous_file_line_matches")
        .and_then(Value::as_array)
        .map(|items| items.is_empty())
        .unwrap_or(true)
    {
        warnings.push(format!(
            "mutation_calibration {} contains ambiguous file/line matches; those records do not raise gate confidence",
            display_path(path)
        ));
    }
    index
}

fn recommendation_confidence_effect(outcome: Option<&str>) -> &'static str {
    match outcome {
        Some("useful" | "summary_only_correct" | "suppressed_correctly") => "supports_static_gap",
        Some("noisy" | "wrong_line" | "wrong_target" | "already_covered") => "keeps_advisory",
        Some(_) => "unknown",
        None => "not_used",
    }
}

fn mutation_confidence_effect(outcome: Option<&str>) -> &'static str {
    let Some(outcome) = outcome else {
        return "not_used";
    };
    if is_runtime_gap_outcome(outcome) {
        "supports_static_gap"
    } else if matches!(
        outcome,
        "caught" | "timeout" | "static_gap_without_runtime_signal"
    ) {
        "keeps_advisory"
    } else {
        "unknown"
    }
}

fn is_runtime_gap_outcome(outcome: &str) -> bool {
    outcome == "missed"
        || outcome == "not_caught"
        || outcome == "uncaught"
        || outcome == format!("{}{}", "sur", "vived")
}

fn read_baseline(
    input: &GateEvaluateInput,
    warnings: &mut Vec<String>,
    config_errors: &mut Vec<String>,
) -> BaselineIndex {
    if input.mode.requires_baseline() && input.baseline.is_none() {
        config_errors.push(format!(
            "{} mode requires an explicit --baseline artifact",
            input.mode.as_str()
        ));
        return BaselineIndex::default();
    }
    let Some(path) = &input.baseline else {
        return BaselineIndex::default();
    };
    let resolved = resolve_root_path(&input.root, path);
    match read_json_value(&resolved) {
        Ok(value) => baseline_index_from_value(&value),
        Err(error) if input.mode.requires_baseline() => {
            config_errors.push(format!(
                "required baseline {} is invalid: {error}",
                display_path(path)
            ));
            BaselineIndex::default()
        }
        Err(error) => {
            warnings.push(format!(
                "optional baseline {} is unavailable: {error}",
                display_path(path)
            ));
            BaselineIndex::default()
        }
    }
}

fn baseline_index_from_value(value: &Value) -> BaselineIndex {
    let mut index = BaselineIndex::default();
    for item in value
        .get("decisions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        collect_identity(&mut index.identities, item.get("seam_id"));
        collect_identity(&mut index.identities, item.get("source_id"));
    }
    for collection in ["comments", "summary_only", "suppressed"] {
        for item in value
            .get(collection)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            collect_identity(&mut index.identities, item.get("seam_id"));
            collect_identity(&mut index.identities, item.get("id"));
            collect_identity(&mut index.identities, item.get("dedupe_key"));
        }
    }
    index
}

fn collect_identity(identities: &mut BTreeSet<String>, value: Option<&Value>) {
    if let Some(text) = value
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        identities.insert(text.to_string());
    }
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
    GateCandidate {
        source: source.to_string(),
        source_id,
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
        nearby_test_changed,
        suppressed,
        configured_off: suppression_reason.as_deref() == Some("severity_off"),
        suppression_reason,
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
    let baseline_identity = baseline_identity(candidate);
    let is_baseline_new = baseline_identity
        .as_ref()
        .map(|identity| !baseline.identities.contains(identity))
        .unwrap_or(true);
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
    } else if !eligible && candidate.static_class.is_none() {
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
        } else {
            "pr_guidance".to_string()
        },
        decision,
        gate_reason,
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
            nearby_test_changed: candidate.nearby_test_changed,
            suppressed: candidate.suppressed,
            configured_off: candidate.configured_off,
            recommendation_calibration,
            mutation_calibration,
        },
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
    !candidate.suppressed
        && !candidate.configured_off
        && candidate_class_is_policy_eligible(candidate.static_class.as_deref())
        && has_concrete_guidance(candidate)
        && !candidate.nearby_test_changed
        && candidate.placement.path.is_some()
        && candidate.placement.line.is_some()
        && candidate.source != "summary_only"
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
    candidate
        .seam_id
        .clone()
        .or_else(|| (!candidate.source_id.is_empty()).then(|| candidate.source_id.clone()))
        .or_else(|| {
            Some(format!(
                "{}:{}:{}",
                candidate.placement.path.as_deref()?,
                candidate.placement.line?,
                candidate.static_class.as_deref().unwrap_or("unknown")
            ))
        })
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
        "blocking" if context.mode == GateMode::BaselineCheck && context.is_baseline_new => {
            "new policy-eligible gap blocks under baseline-check".to_string()
        }
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

fn top_level_status(
    summary: &GateSummary,
    warnings: &[String],
    config_errors: &[String],
    mode: GateMode,
) -> &'static str {
    if !config_errors.is_empty() {
        "config_error"
    } else if summary.blocking > 0 {
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

fn inputs_json(inputs: &GateDecisionInputs) -> Value {
    json!({
        "repo_exposure": inputs.repo_exposure,
        "pr_guidance": inputs.pr_guidance,
        "sarif_policy": inputs.sarif_policy,
        "labels_json": inputs.labels_json,
        "labels": inputs.labels,
        "agent_verify": inputs.agent_verify,
        "agent_receipt": inputs.agent_receipt,
        "recommendation_calibration": inputs.recommendation_calibration,
        "mutation_calibration": inputs.mutation_calibration,
        "baseline": inputs.baseline,
    })
}

fn policy_json(policy: &GatePolicy) -> Value {
    json!({
        "mode": policy.mode.as_str(),
        "threshold": policy.threshold,
        "acknowledgement_labels": policy.acknowledgement_labels,
        "default_workflow_posture": policy.default_workflow_posture,
    })
}

fn summary_json(summary: &GateSummary) -> Value {
    json!({
        "evaluated": summary.evaluated,
        "blocking": summary.blocking,
        "acknowledged": summary.acknowledged,
        "advisory": summary.advisory,
        "suppressed": summary.suppressed,
        "not_applicable": summary.not_applicable,
        "unknown_confidence": summary.unknown_confidence,
    })
}

fn decision_json(decision: &GateDecision) -> Value {
    json!({
        "id": decision.id,
        "source": decision.source,
        "decision": decision.decision,
        "gate_reason": decision.gate_reason,
        "seam_id": decision.seam_id,
        "source_id": decision.source_id,
        "static_class": decision.static_class,
        "severity": decision.severity,
        "placement": {
            "path": decision.placement.path,
            "line": decision.placement.line,
        },
        "policy": {
            "mode": decision.policy.mode.as_str(),
            "threshold": decision.policy.threshold,
            "acknowledgement_label": decision.policy.acknowledgement_label,
            "baseline_identity": decision.policy.baseline_identity,
        },
        "evidence": {
            "missing_discriminator": decision.evidence.missing_discriminator,
            "assertion_shape": decision.evidence.assertion_shape,
            "candidate_values": decision.evidence.candidate_values,
            "recommended_test": decision.evidence.recommended_test,
            "nearby_test_changed": decision.evidence.nearby_test_changed,
            "suppressed": decision.evidence.suppressed,
            "configured_off": decision.evidence.configured_off,
            "recommendation_calibration": calibration_json(&decision.evidence.recommendation_calibration),
            "mutation_calibration": calibration_json(&decision.evidence.mutation_calibration),
        }
    })
}

fn calibration_json(evidence: &CalibrationEvidence) -> Value {
    json!({
        "available": evidence.available,
        "outcome": evidence.outcome,
        "confidence_effect": evidence.confidence_effect,
    })
}

fn push_decision_section(
    out: &mut String,
    title: &str,
    decisions: &[GateDecision],
    decision_value: &str,
) {
    let section = decisions
        .iter()
        .filter(|decision| decision.decision == decision_value)
        .collect::<Vec<_>>();
    if section.is_empty() {
        return;
    }
    out.push_str(&format!("## {title}\n\n"));
    for decision in section {
        let path = decision.placement.path.as_deref().unwrap_or("<no path>");
        let line = decision
            .placement
            .line
            .map(|line| line.to_string())
            .unwrap_or_else(|| "?".to_string());
        out.push_str(&format!(
            "- {}:{} {} — {}\n",
            md_escape(path),
            line,
            md_escape(decision.static_class.as_deref().unwrap_or("unknown")),
            md_escape(&decision.gate_reason)
        ));
    }
    out.push('\n');
}

fn acknowledgement_labels(input: &GateEvaluateInput) -> Vec<String> {
    if input.acknowledgement_labels.is_empty() {
        vec![DEFAULT_ACKNOWLEDGEMENT_LABEL.to_string()]
    } else {
        input.acknowledgement_labels.clone()
    }
}

fn read_json_value(path: &Path) -> Result<Value, String> {
    let text =
        fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("parse {} failed: {err}", path.display()))
}

fn string_field(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
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

fn md_escape(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn gate_visible_only_records_pr_guidance_without_blocking() -> Result<(), String> {
        let input = fixture_input(GateMode::VisibleOnly);
        let report = build_gate_decision_report(&input)?;
        assert_eq!(report.status, "advisory");
        assert_eq!(report.summary.evaluated, 1);
        assert_eq!(report.summary.advisory, 1);
        assert!(!gate_decision_should_fail(&report));
        let json_text = render_gate_decision_json(&report)?;
        let value: Value = serde_json::from_str(&json_text)
            .map_err(|err| format!("gate decision JSON should parse: {err}"))?;
        assert_eq!(value["schema_version"], SCHEMA_VERSION);
        assert_eq!(value["status"], "advisory");
        assert_eq!(value["decisions"][0]["decision"], "advisory");
        assert_eq!(
            value["decisions"][0]["evidence"]["recommended_test"],
            "tests/pricing.rs::above_threshold_gets_discount"
        );
        let markdown = render_gate_decision_markdown(&report);
        assert!(markdown.contains("# RIPR Gate Decision"));
        assert!(markdown.contains("Decision: advisory"));
        assert!(markdown.contains("visible-only mode records evidence without blocking"));
        Ok(())
    }

    #[test]
    fn gate_acknowledgeable_blocks_policy_candidate_without_label() -> Result<(), String> {
        let input = fixture_input(GateMode::Acknowledgeable);
        let report = build_gate_decision_report(&input)?;
        assert_eq!(report.status, "blocked");
        assert_eq!(report.summary.blocking, 1);
        assert!(gate_decision_should_fail(&report));
        assert_eq!(report.decisions[0].decision, "blocking");
        Ok(())
    }

    #[test]
    fn gate_acknowledgeable_keeps_waived_candidate_visible() -> Result<(), String> {
        let mut input = fixture_input(GateMode::Acknowledgeable);
        input.labels.push("ripr-waive".to_string());
        let report = build_gate_decision_report(&input)?;
        assert_eq!(report.status, "acknowledged");
        assert_eq!(report.summary.acknowledged, 1);
        assert!(!gate_decision_should_fail(&report));
        assert_eq!(
            report.decisions[0].policy.acknowledgement_label,
            Some("ripr-waive".to_string())
        );
        Ok(())
    }

    #[test]
    fn gate_calibrated_mode_requires_explicit_baseline() -> Result<(), String> {
        let input = fixture_input(GateMode::CalibratedGate);
        let report = build_gate_decision_report(&input)?;
        assert_eq!(report.status, "config_error");
        assert_eq!(report.summary.evaluated, 0);
        assert!(gate_decision_should_fail(&report));
        assert!(
            report
                .config_errors
                .iter()
                .any(|error| error.contains("requires an explicit --baseline"))
        );
        Ok(())
    }

    #[test]
    fn gate_calibrated_mode_blocks_new_supported_candidate() -> Result<(), String> {
        let dir = temp_dir("gate-calibrated")?;
        let baseline = dir.join("baseline.json");
        fs::write(&baseline, r#"{"schema_version":"0.1","decisions":[]}"#)
            .map_err(|err| format!("write baseline failed: {err}"))?;
        let mut input = fixture_input(GateMode::CalibratedGate);
        input.baseline = Some(baseline);
        input.recommendation_calibration = Some(PathBuf::from(
            "fixtures/boundary_gap/expected/recommendation-calibration/recommendation-calibration.json",
        ));
        let report = build_gate_decision_report(&input)?;
        assert_eq!(report.status, "blocked");
        assert_eq!(report.summary.blocking, 1);
        assert_eq!(
            report.decisions[0]
                .evidence
                .recommendation_calibration
                .confidence_effect,
            "supports_static_gap"
        );
        let _ = fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn gate_calibrated_mode_uses_imported_mutation_support() -> Result<(), String> {
        let dir = temp_dir("gate-mutation-calibrated")?;
        let baseline = dir.join("baseline.json");
        let mutation = dir.join("mutation-calibration.json");
        fs::write(&baseline, r#"{"schema_version":"0.1","decisions":[]}"#)
            .map_err(|err| format!("write baseline failed: {err}"))?;
        fs::write(
            &mutation,
            r#"{
              "schema_version": "0.1",
              "matches": [
                {
                  "join_method": "seam_id",
                  "runtime": {
                    "seam_id": "8f7fa8644fd12280",
                    "runtime_outcome": "missed"
                  },
                  "static": {
                    "seam_id": "8f7fa8644fd12280"
                  }
                }
              ],
              "ambiguous_file_line_matches": []
            }"#,
        )
        .map_err(|err| format!("write mutation calibration failed: {err}"))?;
        let mut input = fixture_input(GateMode::CalibratedGate);
        input.baseline = Some(baseline);
        input.mutation_calibration = Some(mutation);
        let report = build_gate_decision_report(&input)?;
        assert_eq!(report.status, "blocked");
        assert_eq!(report.summary.blocking, 1);
        assert_eq!(
            report.decisions[0]
                .evidence
                .mutation_calibration
                .confidence_effect,
            "supports_static_gap"
        );
        assert!(
            report.decisions[0]
                .gate_reason
                .contains("imported mutation calibration")
        );
        let _ = fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn gate_labels_json_acknowledges_candidate() -> Result<(), String> {
        let dir = temp_dir("gate-labels-json")?;
        let labels = dir.join("labels.json");
        fs::write(&labels, r#"{"labels":["ripr-waive"]}"#)
            .map_err(|err| format!("write labels failed: {err}"))?;
        let mut input = fixture_input(GateMode::Acknowledgeable);
        input.labels_json = Some(labels);
        let report = build_gate_decision_report(&input)?;
        assert_eq!(report.status, "acknowledged");
        assert_eq!(report.inputs.labels, vec!["ripr-waive".to_string()]);
        let _ = fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn gate_baseline_check_keeps_existing_candidate_advisory() -> Result<(), String> {
        let dir = temp_dir("gate-baseline-existing")?;
        let baseline = dir.join("baseline.json");
        fs::write(
            &baseline,
            r#"{
              "schema_version": "0.1",
              "decisions": [
                {"seam_id": "8f7fa8644fd12280", "source_id": "ripr-review-8f7fa8644fd12280"}
              ]
            }"#,
        )
        .map_err(|err| format!("write baseline failed: {err}"))?;
        let mut input = fixture_input(GateMode::BaselineCheck);
        input.baseline = Some(baseline);
        let report = build_gate_decision_report(&input)?;
        assert_eq!(report.status, "advisory");
        assert_eq!(report.summary.blocking, 0);
        assert_eq!(report.summary.advisory, 1);
        assert!(
            report.decisions[0]
                .gate_reason
                .contains("explicit baseline")
        );
        let _ = fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn gate_mode_parse_covers_all_values_and_unknowns() {
        assert_eq!(GateMode::parse("visible-only"), Ok(GateMode::VisibleOnly));
        assert_eq!(
            GateMode::parse("acknowledgeable"),
            Ok(GateMode::Acknowledgeable)
        );
        assert_eq!(
            GateMode::parse("baseline-check"),
            Ok(GateMode::BaselineCheck)
        );
        assert_eq!(
            GateMode::parse("calibrated-gate"),
            Ok(GateMode::CalibratedGate)
        );
        assert_eq!(
            GateMode::parse("hard"),
            Err("unknown gate mode `hard`".to_string())
        );
    }

    #[test]
    fn gate_optional_inputs_emit_warnings_and_markdown_sections() -> Result<(), String> {
        let dir = temp_dir("gate-optional-warnings")?;
        let invalid = write_temp_json(&dir, "invalid.json", "{")?;
        let mut input = fixture_input(GateMode::VisibleOnly);
        input.root = dir.clone();
        input.pr_guidance = write_temp_json(&dir, "comments.json", PR_GUIDANCE_JSON)?;
        input.repo_exposure = Some(PathBuf::from("missing-repo.json"));
        input.sarif_policy = Some(
            invalid
                .strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        );
        input.labels_json = Some(input.sarif_policy.clone().unwrap_or_default());
        input.agent_verify = Some(PathBuf::from("missing-verify.json"));
        input.agent_receipt = Some(input.sarif_policy.clone().unwrap_or_default());
        input.recommendation_calibration = Some(PathBuf::from("missing-recommendation.json"));
        input.mutation_calibration = Some(input.sarif_policy.clone().unwrap_or_default());
        input.baseline = Some(input.sarif_policy.clone().unwrap_or_default());

        let report = build_gate_decision_report(&input)?;
        let mut warning_report = report.clone();
        warning_report
            .warnings
            .push("manual | warning\nwith newline".to_string());
        let markdown = render_gate_decision_markdown(&warning_report);

        assert_eq!(report.status, "advisory");
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("optional repo_exposure"))
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("optional labels_json"))
        );
        assert!(markdown.contains("## Warnings"));
        assert!(markdown.contains("manual \\| warning with newline"));
        let _ = fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn gate_config_errors_render_markdown_and_fail_status() -> Result<(), String> {
        let input = GateEvaluateInput {
            root: repo_root(),
            repo_exposure: None,
            pr_guidance: PathBuf::from("missing-comments.json"),
            sarif_policy: None,
            labels_json: None,
            labels: Vec::new(),
            agent_verify: None,
            agent_receipt: None,
            recommendation_calibration: None,
            mutation_calibration: None,
            baseline: None,
            mode: GateMode::BaselineCheck,
            acknowledgement_labels: Vec::new(),
        };

        let report = build_gate_decision_report(&input)?;
        let markdown = render_gate_decision_markdown(&report);

        assert_eq!(report.status, "config_error");
        assert!(gate_decision_should_fail(&report));
        assert!(markdown.contains("## Config Errors"));
        assert!(markdown.contains("requires an explicit --baseline"));
        Ok(())
    }

    #[test]
    fn gate_summary_only_and_suppressed_candidates_remain_visible() -> Result<(), String> {
        let dir = temp_dir("gate-summary-suppressed")?;
        let guidance = write_temp_json(&dir, "comments.json", SUMMARY_AND_SUPPRESSED_JSON)?;
        let mut input = fixture_input(GateMode::Acknowledgeable);
        input.root = dir.clone();
        input.pr_guidance = guidance
            .strip_prefix(&dir)
            .map_err(|err| err.to_string())?
            .to_path_buf();

        let report = build_gate_decision_report(&input)?;

        assert_eq!(report.status, "advisory");
        assert_eq!(report.summary.suppressed, 1);
        assert_eq!(report.summary.advisory, 1);
        assert!(
            report
                .decisions
                .iter()
                .any(|decision| decision.gate_reason.contains("summary-only"))
        );
        assert!(
            report
                .decisions
                .iter()
                .any(|decision| decision.gate_reason.contains("configured-hidden"))
        );
        let _ = fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn gate_changed_test_and_missing_guidance_candidates_stay_advisory() -> Result<(), String> {
        let dir = temp_dir("gate-ineligible")?;
        let guidance = write_temp_json(&dir, "comments.json", INELIGIBLE_GUIDANCE_JSON)?;
        let mut input = fixture_input(GateMode::Acknowledgeable);
        input.root = dir.clone();
        input.pr_guidance = guidance
            .strip_prefix(&dir)
            .map_err(|err| err.to_string())?
            .to_path_buf();

        let report = build_gate_decision_report(&input)?;

        assert_eq!(report.status, "advisory");
        assert_eq!(report.summary.blocking, 0);
        assert!(
            report
                .decisions
                .iter()
                .any(|decision| decision.gate_reason.contains("nearby focused test changed"))
        );
        let missing_guidance = write_temp_json(&dir, "missing.json", MISSING_GUIDANCE_JSON)?;
        input.pr_guidance = missing_guidance
            .strip_prefix(&dir)
            .map_err(|err| err.to_string())?
            .to_path_buf();
        let report = build_gate_decision_report(&input)?;
        assert!(
            report
                .decisions
                .iter()
                .any(|decision| decision.gate_reason.contains("missing concrete"))
        );
        let _ = fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn gate_baseline_check_blocks_new_candidate() -> Result<(), String> {
        let dir = temp_dir("gate-baseline-new")?;
        let baseline = write_temp_json(&dir, "baseline.json", r#"{"decisions":[]}"#)?;
        let mut input = fixture_input(GateMode::BaselineCheck);
        input.baseline = Some(baseline);

        let report = build_gate_decision_report(&input)?;

        assert_eq!(report.status, "blocked");
        assert_eq!(report.summary.blocking, 1);
        assert!(report.decisions[0].gate_reason.contains("baseline-check"));
        let _ = fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn gate_labels_array_supports_custom_acknowledgement_label() -> Result<(), String> {
        let dir = temp_dir("gate-label-array")?;
        let labels = write_temp_json(&dir, "labels.json", r#"["accepted-risk"]"#)?;
        let mut input = fixture_input(GateMode::Acknowledgeable);
        input.labels_json = Some(labels);
        input.acknowledgement_labels = vec!["accepted-risk".to_string()];

        let report = build_gate_decision_report(&input)?;

        assert_eq!(report.status, "acknowledged");
        assert_eq!(
            report.decisions[0].policy.acknowledgement_label.as_deref(),
            Some("accepted-risk")
        );
        let _ = fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn gate_calibration_can_keep_candidates_advisory() -> Result<(), String> {
        let dir = temp_dir("gate-calibration-advisory")?;
        let baseline = write_temp_json(&dir, "baseline.json", r#"{"decisions":[]}"#)?;
        let recommendation = write_temp_json(
            &dir,
            "recommendation.json",
            r#"{"recommendations":[{"id":"ripr-review-8f7fa8644fd12280","calibration":{"outcome":"wrong_target"}}]}"#,
        )?;
        let mutation = write_temp_json(
            &dir,
            "mutation.json",
            r#"{
              "matches": [
                {
                  "static": {"seam_id": "other-seam"},
                  "runtime": {"runtime_outcome": "caught"}
                }
              ],
              "static_only_findings": [
                {"static": {"seam_id": "8f7fa8644fd12280"}}
              ],
              "ambiguous_file_line_matches": [{"file":"src/lib.rs","line":7}]
            }"#,
        )?;
        let mut input = fixture_input(GateMode::CalibratedGate);
        input.baseline = Some(baseline);
        input.recommendation_calibration = Some(recommendation);
        input.mutation_calibration = Some(mutation);

        let report = build_gate_decision_report(&input)?;

        assert_eq!(report.status, "advisory");
        assert_eq!(
            report.decisions[0]
                .evidence
                .recommendation_calibration
                .confidence_effect,
            "keeps_advisory"
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("ambiguous file/line"))
        );
        let _ = fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn display_path_normalizes_empty_and_dot_prefixed_paths() {
        assert_eq!(display_path(Path::new("")), ".");
        assert_eq!(
            display_path(Path::new("./target/out.json")),
            "target/out.json"
        );
    }

    fn fixture_input(mode: GateMode) -> GateEvaluateInput {
        GateEvaluateInput {
            root: repo_root(),
            repo_exposure: None,
            pr_guidance: PathBuf::from(
                "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            ),
            sarif_policy: None,
            labels_json: None,
            labels: Vec::new(),
            agent_verify: None,
            agent_receipt: None,
            recommendation_calibration: None,
            mutation_calibration: None,
            baseline: None,
            mode,
            acknowledgement_labels: Vec::new(),
        }
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn temp_dir(name: &str) -> Result<PathBuf, String> {
        let mut path = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system time before unix epoch: {err}"))?
            .as_nanos();
        path.push(format!("ripr-{name}-{stamp}"));
        fs::create_dir_all(&path).map_err(|err| format!("create temp dir failed: {err}"))?;
        Ok(path)
    }

    fn write_temp_json(dir: &Path, name: &str, contents: &str) -> Result<PathBuf, String> {
        let path = dir.join(name);
        fs::write(&path, contents).map_err(|err| format!("write {name} failed: {err}"))?;
        Ok(path)
    }

    const PR_GUIDANCE_JSON: &str = r#"{
      "schema_version": "0.1",
      "summary": {"unchanged_tests": true},
      "comments": [
        {
          "id": "ripr-review-8f7fa8644fd12280",
          "seam_id": "8f7fa8644fd12280",
          "grip_class": "weakly_gripped",
          "severity": "warning",
          "missing_discriminator": "amount == discount_threshold",
          "placement": {"path": "src/pricing.rs", "line": 88},
          "suggested_test": {
            "candidate_values": ["amount == discount_threshold"],
            "near_test": "above_threshold_gets_discount"
          }
        }
      ],
      "summary_only": [],
      "suppressed": []
    }"#;

    const SUMMARY_AND_SUPPRESSED_JSON: &str = r#"{
      "schema_version": "0.1",
      "summary": {"unchanged_tests": true},
      "comments": [],
      "summary_only": [
        {
          "id": "summary-1",
          "seam_id": "summary-seam",
          "grip_class": "weakly_gripped",
          "severity": "warning",
          "missing_discriminator": "amount == discount_threshold",
          "placement": {"path": "src/pricing.rs", "line": 88}
        }
      ],
      "suppressed": [
        {
          "id": "suppressed-1",
          "seam_id": "suppressed-seam",
          "grip_class": "weakly_gripped",
          "severity": "off",
          "reason": "severity_off",
          "missing_discriminator": "amount == discount_threshold",
          "placement": {"path": "src/pricing.rs", "line": 89}
        }
      ]
    }"#;

    const INELIGIBLE_GUIDANCE_JSON: &str = r#"{
      "schema_version": "0.1",
      "summary": {"unchanged_tests": false},
      "comments": [
        {
          "id": "changed-test",
          "seam_id": "changed-test-seam",
          "grip_class": "weakly_gripped",
          "severity": "warning",
          "missing_discriminator": "amount == discount_threshold",
          "placement": {"path": "src/pricing.rs", "line": 88}
        },
        {
          "id": "missing-guidance",
          "seam_id": "missing-guidance-seam",
          "grip_class": "weakly_gripped",
          "severity": "warning",
          "placement": {"path": "src/pricing.rs", "line": 89}
        }
      ],
      "summary_only": [],
      "suppressed": []
    }"#;

    const MISSING_GUIDANCE_JSON: &str = r#"{
      "schema_version": "0.1",
      "summary": {"unchanged_tests": true},
      "comments": [
        {
          "id": "missing-guidance",
          "seam_id": "missing-guidance-seam",
          "grip_class": "weakly_gripped",
          "severity": "warning",
          "placement": {"path": "src/pricing.rs", "line": 89}
        }
      ],
      "summary_only": [],
      "suppressed": []
    }"#;
}
