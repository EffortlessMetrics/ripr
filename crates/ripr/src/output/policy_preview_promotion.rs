use crate::output::markdown::{markdown_text, render_string_section};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "preview_evidence_promotion_packet";
const LIMITS_NOTE: &str = "Read-only advisory preview promotion packet. Preview evidence remains visible and non-gating until a later explicit promotion policy is reviewed.";
const DEFAULT_REASON: &str = "preview promotion evidence not supplied";

const SUPPORTED_LANGUAGES: [&str; 2] = ["typescript", "python"];

const NON_GOALS: [&str; 17] = [
    "No actual promotion.",
    "No gate eligibility change.",
    "No RIPR Zero inclusion.",
    "No calibrated confidence.",
    "No CI blocking.",
    "No config mutation.",
    "No baseline mutation or adoption.",
    "No suppression creation, deletion, or auto-expiry.",
    "No workflow, branch-protection, or generated CI mutation.",
    "No history append.",
    "No gate decision.",
    "No analyzer behavior changes.",
    "No evidence identity rewrites.",
    "No LSP or editor behavior changes.",
    "No generated tests.",
    "No provider calls.",
    "No mutation execution.",
];

const REQUIRED_EVIDENCE: [EvidenceRequirement; 9] = [
    EvidenceRequirement {
        kind: "fixture_corpus_coverage",
        required: true,
        description: "Representative fixtures cover the candidate class and known static limits.",
    },
    EvidenceRequirement {
        kind: "static_limit_exclusions",
        required: true,
        description: "Known static parser and language-adapter limits are excluded or labeled.",
    },
    EvidenceRequirement {
        kind: "false_positive_review",
        required: true,
        description: "Maintainer-reviewed false-positive sample is documented for this language and class.",
    },
    EvidenceRequirement {
        kind: "recommendation_calibration",
        required: true,
        description: "Same-class recommendation calibration supports policy eligibility.",
    },
    EvidenceRequirement {
        kind: "mutation_calibration",
        required: false,
        description: "Optional runtime calibration exists for this language and class without being inferred from Rust.",
    },
    EvidenceRequirement {
        kind: "baseline_behavior",
        required: true,
        description: "Baseline handling keeps preview debt visible and does not auto-adopt new preview findings.",
    },
    EvidenceRequirement {
        kind: "waiver_suppression_behavior",
        required: true,
        description: "Waivers and suppressions preserve owner, reason, scope, and preview status.",
    },
    EvidenceRequirement {
        kind: "rollback_path",
        required: true,
        description: "Manual rollback to advisory preview status is documented.",
    },
    EvidenceRequirement {
        kind: "generated_ci_posture",
        required: true,
        description: "Generated CI remains advisory and non-blocking unless a later explicit gate mode is configured.",
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EvidenceRequirement {
    kind: &'static str,
    required: bool,
    description: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreviewPromotionInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) language: String,
    pub(crate) candidate_class: String,
    pub(crate) evidence_path: Option<String>,
    pub(crate) evidence_json: Option<Result<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreviewPromotionReport {
    root: String,
    generated_at: String,
    language: String,
    language_status: String,
    candidate_class: String,
    target_status: String,
    allowed_now: bool,
    reason: String,
    required_evidence: Vec<EvidenceRequirement>,
    supplied_evidence: Vec<String>,
    missing_evidence: Vec<String>,
    required_repairs: Vec<String>,
    required_receipts: Vec<String>,
    rollback_path: Vec<String>,
    generated_ci_posture: GeneratedCiPosture,
    input_artifacts: Vec<InputArtifact>,
    warnings: Vec<Notice>,
    unknowns: Vec<Notice>,
    non_goals: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GeneratedCiPosture {
    may_upload_artifact: bool,
    may_summarize_artifact: bool,
    may_fail_check: bool,
    may_post_comment: bool,
    may_mutate_config: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InputArtifact {
    kind: String,
    path: Option<String>,
    status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Notice {
    kind: String,
    message: String,
    source_artifact: Option<String>,
}

#[derive(Clone, Debug)]
struct ParsedArtifact {
    kind: &'static str,
    path: Option<String>,
    status: &'static str,
    value: Option<Value>,
}

pub(crate) fn default_preview_promotion_out(language: &str, candidate_class: &str) -> String {
    format!(
        "target/ripr/reports/preview-promotion-{}-{}.json",
        file_label(language),
        file_label(candidate_class)
    )
}

pub(crate) fn default_preview_promotion_md_out(language: &str, candidate_class: &str) -> String {
    format!(
        "target/ripr/reports/preview-promotion-{}-{}.md",
        file_label(language),
        file_label(candidate_class)
    )
}

pub(crate) fn is_supported_language(language: &str) -> bool {
    SUPPORTED_LANGUAGES.contains(&language)
}

pub(crate) fn build_preview_promotion_report(
    input: PreviewPromotionInput,
) -> PreviewPromotionReport {
    let language = input.language;
    let candidate_class = input.candidate_class;
    let evidence = parse_optional_json(
        "preview_promotion_evidence",
        input.evidence_path,
        input.evidence_json,
    );

    let mut warnings = Vec::new();
    collect_artifact_notices(&evidence, &mut warnings);
    collect_supplied_boundary_warnings(
        evidence.value.as_ref(),
        &language,
        &candidate_class,
        evidence.path.clone(),
        &mut warnings,
    );

    let supplied_evidence = evidence
        .value
        .as_ref()
        .map(supplied_evidence)
        .unwrap_or_default();
    let missing_evidence = REQUIRED_EVIDENCE
        .iter()
        .filter(|requirement| requirement.required)
        .filter(|requirement| {
            !supplied_evidence
                .iter()
                .any(|supplied| supplied == requirement.kind)
        })
        .map(|requirement| requirement.kind.to_string())
        .collect::<Vec<_>>();

    let reason = if evidence.status == "omitted" {
        DEFAULT_REASON.to_string()
    } else if evidence.status != "read" {
        "preview promotion evidence is missing or malformed".to_string()
    } else if !missing_evidence.is_empty() {
        "preview promotion evidence is incomplete".to_string()
    } else {
        "preview promotion evidence is supplied, but this command remains advisory until a later explicit promotion policy recognizes those receipts.".to_string()
    };

    let input_artifacts = if evidence.path.is_some() {
        vec![input_artifact(&evidence)]
    } else {
        Vec::new()
    };
    let required_repairs = required_repairs(&missing_evidence, evidence.status);

    PreviewPromotionReport {
        root: input.root,
        generated_at: input.generated_at,
        language: language.clone(),
        language_status: "preview".to_string(),
        candidate_class: candidate_class.clone(),
        target_status: "policy_eligible".to_string(),
        allowed_now: false,
        reason,
        required_evidence: REQUIRED_EVIDENCE.to_vec(),
        supplied_evidence,
        missing_evidence,
        required_repairs,
        required_receipts: required_receipts(&language, &candidate_class),
        rollback_path: rollback_path(&language, &candidate_class),
        generated_ci_posture: GeneratedCiPosture {
            may_upload_artifact: true,
            may_summarize_artifact: true,
            may_fail_check: false,
            may_post_comment: false,
            may_mutate_config: false,
        },
        input_artifacts,
        warnings,
        unknowns: Vec::new(),
        non_goals: NON_GOALS.iter().map(|value| (*value).to_string()).collect(),
    }
}

pub(crate) fn render_preview_promotion_json(
    report: &PreviewPromotionReport,
) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": REPORT_KIND,
        "root": report.root,
        "generated_at": report.generated_at,
        "language": report.language,
        "language_status": report.language_status,
        "candidate_class": report.candidate_class,
        "target_status": report.target_status,
        "allowed_now": report.allowed_now,
        "reason": report.reason,
        "required_evidence": report.required_evidence.iter().map(requirement_json).collect::<Vec<_>>(),
        "supplied_evidence": report.supplied_evidence,
        "missing_evidence": report.missing_evidence,
        "required_repairs": report.required_repairs,
        "required_receipts": report.required_receipts,
        "rollback_path": report.rollback_path,
        "generated_ci_posture": generated_ci_posture_json(&report.generated_ci_posture),
        "input_artifacts": report.input_artifacts.iter().map(input_artifact_json).collect::<Vec<_>>(),
        "warnings": report.warnings.iter().map(notice_json).collect::<Vec<_>>(),
        "unknowns": report.unknowns.iter().map(notice_json).collect::<Vec<_>>(),
        "non_goals": report.non_goals,
        "limits_note": LIMITS_NOTE,
    }))
    .map_err(|err| format!("failed to render preview promotion JSON: {err}"))
}

pub(crate) fn render_preview_promotion_markdown(report: &PreviewPromotionReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Preview Evidence Promotion Packet\n\n");
    out.push_str(&format!("Language: {}\n", markdown_text(&report.language)));
    out.push_str(&format!(
        "Class: {}\n",
        markdown_text(&report.candidate_class)
    ));
    out.push_str(&format!(
        "Current status: {}\n",
        markdown_text(&report.language_status)
    ));
    out.push_str(&format!(
        "Target status: {}\n",
        markdown_text(&report.target_status)
    ));
    out.push_str("Allowed now: no\n");
    out.push_str(&format!("Why: {}\n", markdown_text(&report.reason)));

    render_string_section(&mut out, "Missing Evidence", &report.missing_evidence);
    render_string_section(&mut out, "Supplied Evidence", &report.supplied_evidence);
    render_string_section(&mut out, "Required Repairs", &report.required_repairs);
    render_string_section(&mut out, "Required Receipts", &report.required_receipts);
    render_string_section(&mut out, "Rollback", &report.rollback_path);

    out.push_str("\n## Generated CI Posture\n\n");
    out.push_str(&format!(
        "- may upload artifact: {}\n",
        yes_no(report.generated_ci_posture.may_upload_artifact)
    ));
    out.push_str(&format!(
        "- may summarize artifact: {}\n",
        yes_no(report.generated_ci_posture.may_summarize_artifact)
    ));
    out.push_str(&format!(
        "- may fail check: {}\n",
        yes_no(report.generated_ci_posture.may_fail_check)
    ));
    out.push_str(&format!(
        "- may post comment: {}\n",
        yes_no(report.generated_ci_posture.may_post_comment)
    ));
    out.push_str(&format!(
        "- may mutate config: {}\n",
        yes_no(report.generated_ci_posture.may_mutate_config)
    ));

    if !report.input_artifacts.is_empty() {
        out.push_str("\n## Input Artifacts\n\n");
        for artifact in &report.input_artifacts {
            out.push_str(&format!("- {}: {}", artifact.kind, artifact.status));
            if let Some(path) = artifact.path.as_deref() {
                out.push_str(&format!(" ({})", markdown_text(path)));
            }
            out.push('\n');
        }
    }

    if !report.warnings.is_empty() {
        out.push_str("\n## Warnings\n\n");
        for warning in &report.warnings {
            out.push_str(&format!(
                "- {}: {}\n",
                warning.kind,
                markdown_text(&warning.message)
            ));
        }
    }

    if !report.unknowns.is_empty() {
        out.push_str("\n## Unknowns\n\n");
        for unknown in &report.unknowns {
            out.push_str(&format!(
                "- {}: {}\n",
                unknown.kind,
                markdown_text(&unknown.message)
            ));
        }
    }

    render_string_section(&mut out, "Non-Goals", &report.non_goals);
    out.push_str("\nLimits:\n");
    out.push_str(LIMITS_NOTE);
    out.push('\n');
    out
}

pub(crate) fn preview_promotion_allowed_now(report: &PreviewPromotionReport) -> bool {
    report.allowed_now
}

pub(crate) fn display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn parse_optional_json(
    kind: &'static str,
    path: Option<String>,
    text: Option<Result<String, String>>,
) -> ParsedArtifact {
    if path.is_none() {
        return ParsedArtifact {
            kind,
            path,
            status: "omitted",
            value: None,
        };
    }
    let Some(text) = text else {
        return ParsedArtifact {
            kind,
            path,
            status: "missing",
            value: None,
        };
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            return ParsedArtifact {
                kind,
                path,
                status: if looks_like_missing_file(&error) {
                    "missing"
                } else {
                    "malformed"
                },
                value: None,
            };
        }
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => ParsedArtifact {
            kind,
            path,
            status: "read",
            value: Some(value),
        },
        Err(_) => ParsedArtifact {
            kind,
            path,
            status: "malformed",
            value: None,
        },
    }
}

fn looks_like_missing_file(error: &str) -> bool {
    error.contains("os error 2")
        || error.contains("No such file")
        || error.contains("cannot find the file")
}

fn collect_artifact_notices(artifact: &ParsedArtifact, warnings: &mut Vec<Notice>) {
    match artifact.status {
        "read" | "omitted" => {}
        "missing" => warnings.push(Notice {
            kind: format!("{}_missing", artifact.kind),
            message: format!(
                "{} was supplied but could not be read; preview promotion remains advisory.",
                artifact.kind.replace('_', " ")
            ),
            source_artifact: artifact.path.clone(),
        }),
        "malformed" => warnings.push(Notice {
            kind: format!("{}_malformed", artifact.kind),
            message: format!(
                "{} input is not valid JSON for this packet.",
                artifact.kind.replace('_', " ")
            ),
            source_artifact: artifact.path.clone(),
        }),
        _ => {}
    }
}

fn collect_supplied_boundary_warnings(
    value: Option<&Value>,
    language: &str,
    candidate_class: &str,
    path: Option<String>,
    warnings: &mut Vec<Notice>,
) {
    let Some(value) = value else {
        return;
    };
    if let Some(supplied_language) = string_path(value, &["language"])
        && supplied_language != language
    {
        warnings.push(Notice {
            kind: "preview_promotion_language_mismatch".to_string(),
            message: format!(
                "Supplied evidence language {supplied_language} does not match requested language {language}; requested language remains authoritative."
            ),
            source_artifact: path.clone(),
        });
    }
    if let Some(status) = string_path(value, &["language_status"])
        && status != "preview"
    {
        warnings.push(Notice {
            kind: "preview_promotion_language_status_ignored".to_string(),
            message: format!(
                "Supplied language_status {status} cannot promote preview evidence; packet keeps language_status preview."
            ),
            source_artifact: path.clone(),
        });
    }
    let supplied_class =
        string_path(value, &["candidate_class"]).or_else(|| string_path(value, &["class"]));
    if let Some(supplied_class) = supplied_class
        && supplied_class != candidate_class
    {
        warnings.push(Notice {
            kind: "preview_promotion_class_mismatch".to_string(),
            message: format!(
                "Supplied evidence class {supplied_class} does not match requested class {candidate_class}; requested class remains authoritative."
            ),
            source_artifact: path,
        });
    }
}

fn supplied_evidence(value: &Value) -> Vec<String> {
    let mut supplied = BTreeSet::new();
    if let Some(values) = value.get("supplied_evidence").and_then(Value::as_array) {
        for entry in values {
            let raw = entry
                .as_str()
                .map(ToOwned::to_owned)
                .or_else(|| string_path(entry, &["kind"]));
            if let Some(raw) = raw
                && let Some(kind) = normalize_evidence_kind(&raw)
            {
                supplied.insert(kind);
            }
        }
    }
    for requirement in REQUIRED_EVIDENCE {
        if field_supplied(value, requirement.kind) {
            supplied.insert(requirement.kind.to_string());
        }
    }
    supplied.into_iter().collect()
}

fn field_supplied(value: &Value, kind: &str) -> bool {
    let Some(entry) = value.get(kind) else {
        return false;
    };
    match entry {
        Value::Bool(value) => *value,
        Value::Null => false,
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        Value::String(value) => !value.trim().is_empty(),
        Value::Number(_) => true,
    }
}

fn normalize_evidence_kind(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    let normalized = match normalized.as_str() {
        "waiver_and_suppression_behavior" => "waiver_suppression_behavior".to_string(),
        other => other.to_string(),
    };
    REQUIRED_EVIDENCE
        .iter()
        .any(|requirement| requirement.kind == normalized)
        .then_some(normalized)
}

fn required_repairs(missing_evidence: &[String], evidence_status: &str) -> Vec<String> {
    if evidence_status == "malformed" {
        return vec![
            "Repair malformed preview promotion evidence before policy eligibility review."
                .to_string(),
        ];
    }
    if evidence_status == "missing" {
        return vec![
            "Supply readable preview promotion evidence before policy eligibility review."
                .to_string(),
        ];
    }
    if missing_evidence.is_empty() {
        return vec![
            "Review supplied receipts manually before any later explicit preview promotion policy."
                .to_string(),
        ];
    }
    if evidence_status == "omitted" {
        return vec![
            "Supply explicit preview promotion evidence before policy eligibility review."
                .to_string(),
        ];
    }
    vec![format!(
        "Supply missing preview promotion evidence: {}.",
        missing_evidence.join(", ")
    )]
}

fn required_receipts(language: &str, candidate_class: &str) -> Vec<String> {
    let class_label = candidate_class.replace('_', "-");
    let language_label = display_language(language);
    vec![
        format!("preview-promotion-{language}-{class_label}.json"),
        "preview-boundary report showing advisory language status".to_string(),
        format!("fixture corpus coverage receipt for {language_label} {candidate_class}"),
        format!("static-limit exclusions receipt for {language_label} {candidate_class}"),
        format!("false-positive review receipt for {language_label} {candidate_class}"),
        format!("recommendation-calibration receipt for {language_label} {candidate_class}"),
        format!("baseline behavior receipt for {language_label} {candidate_class}"),
        format!("waiver/suppression behavior receipt for {language_label} {candidate_class}"),
        format!("rollback path receipt for {language_label} {candidate_class}"),
        format!("generated CI posture receipt for {language_label} {candidate_class}"),
    ]
}

fn rollback_path(language: &str, candidate_class: &str) -> Vec<String> {
    let language_label = display_language(language);
    vec![
        format!("Keep {language_label} {candidate_class} evidence advisory."),
        "Remove any manual preview promotion config if one was reviewed later.".to_string(),
        "Regenerate policy operations and preview promotion packets after rollback.".to_string(),
    ]
}

fn requirement_json(requirement: &EvidenceRequirement) -> Value {
    json!({
        "kind": requirement.kind,
        "required": requirement.required,
        "description": requirement.description,
    })
}

fn generated_ci_posture_json(posture: &GeneratedCiPosture) -> Value {
    json!({
        "may_upload_artifact": posture.may_upload_artifact,
        "may_summarize_artifact": posture.may_summarize_artifact,
        "may_fail_check": posture.may_fail_check,
        "may_post_comment": posture.may_post_comment,
        "may_mutate_config": posture.may_mutate_config,
    })
}

fn input_artifact(artifact: &ParsedArtifact) -> InputArtifact {
    InputArtifact {
        kind: artifact.kind.to_string(),
        path: artifact.path.clone(),
        status: artifact.status.to_string(),
    }
}

fn input_artifact_json(artifact: &InputArtifact) -> Value {
    json!({
        "kind": artifact.kind,
        "path": artifact.path,
        "status": artifact.status,
    })
}

fn notice_json(notice: &Notice) -> Value {
    json!({
        "kind": notice.kind,
        "message": notice.message,
        "source_artifact": notice.source_artifact,
    })
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn file_label(value: &str) -> String {
    value.to_ascii_lowercase().replace('_', "-")
}

fn display_language(language: &str) -> String {
    match language {
        "typescript" => "TypeScript".to_string(),
        "python" => "Python".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(language: &str, candidate_class: &str) -> PreviewPromotionInput {
        PreviewPromotionInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1".to_string(),
            language: language.to_string(),
            candidate_class: candidate_class.to_string(),
            evidence_path: None,
            evidence_json: None,
        }
    }

    #[test]
    fn preview_promotion_default_typescript_is_blocked() {
        let report = build_preview_promotion_report(input("typescript", "boundary_gap"));

        assert!(!report.allowed_now);
        assert_eq!(report.reason, DEFAULT_REASON);
        assert_eq!(report.language_status, "preview");
        assert_eq!(report.target_status, "policy_eligible");
        assert_eq!(report.missing_evidence.len(), 8);
        assert!(report.input_artifacts.is_empty());
        assert!(report.unknowns.is_empty());
    }

    #[test]
    fn preview_promotion_default_python_is_blocked() {
        let report = build_preview_promotion_report(input("python", "boundary_gap"));

        assert!(!report.allowed_now);
        assert_eq!(report.language, "python");
        assert!(
            report
                .required_receipts
                .iter()
                .any(|receipt| receipt.contains("Python boundary_gap"))
        );
    }

    #[test]
    fn preview_promotion_partial_evidence_remains_blocked() {
        let mut input = input("python", "boundary_gap");
        input.evidence_path = Some("preview-evidence.json".to_string());
        input.evidence_json = Some(Ok(r#"{
              "language": "python",
              "language_status": "preview",
              "candidate_class": "boundary_gap",
              "supplied_evidence": ["fixture_corpus_coverage"],
              "static_limit_exclusions": true
            }"#
        .to_string()));
        let report = build_preview_promotion_report(input);

        assert!(!report.allowed_now);
        assert_eq!(report.reason, "preview promotion evidence is incomplete");
        assert!(
            report
                .supplied_evidence
                .iter()
                .any(|evidence| evidence == "static_limit_exclusions")
        );
        assert!(
            report
                .missing_evidence
                .iter()
                .any(|evidence| evidence == "recommendation_calibration")
        );
        assert!(
            report
                .required_repairs
                .iter()
                .any(|repair| repair.contains("false_positive_review"))
        );
    }

    #[test]
    fn preview_promotion_complete_evidence_stays_advisory_until_later_policy() {
        let mut input = input("typescript", "boundary_gap");
        input.evidence_path = Some("preview-evidence.json".to_string());
        input.evidence_json = Some(Ok(r#"{
              "language": "typescript",
              "language_status": "preview",
              "candidate_class": "boundary_gap",
              "supplied_evidence": [
                "fixture_corpus_coverage",
                "static_limit_exclusions",
                "false_positive_review",
                "recommendation_calibration",
                "baseline_behavior",
                "waiver_suppression_behavior",
                "rollback_path",
                "generated_ci_posture"
              ]
            }"#
        .to_string()));
        let report = build_preview_promotion_report(input);

        assert!(!report.allowed_now);
        assert!(report.missing_evidence.is_empty());
        assert!(
            report
                .reason
                .contains("later explicit promotion policy recognizes")
        );
        assert!(
            !report
                .supplied_evidence
                .iter()
                .any(|evidence| evidence == "mutation_calibration")
        );
    }

    #[test]
    fn preview_promotion_optional_mutation_calibration_does_not_block() {
        let mut input = input("typescript", "boundary_gap");
        input.evidence_path = Some("preview-evidence.json".to_string());
        input.evidence_json = Some(Ok(r#"{
              "supplied_evidence": [
                "fixture_corpus_coverage",
                "static_limit_exclusions",
                "false_positive_review",
                "recommendation_calibration",
                "baseline_behavior",
                "waiver_suppression_behavior",
                "rollback_path",
                "generated_ci_posture"
              ]
            }"#
        .to_string()));
        let report = build_preview_promotion_report(input);

        assert!(!report.allowed_now);
        assert!(report.missing_evidence.is_empty());
        assert!(
            report
                .required_evidence
                .iter()
                .any(|requirement| requirement.kind == "mutation_calibration"
                    && !requirement.required)
        );
    }

    #[test]
    fn preview_promotion_malformed_evidence_warns() {
        let mut input = input("typescript", "boundary_gap");
        input.evidence_path = Some("preview-evidence.json".to_string());
        input.evidence_json = Some(Ok("{".to_string()));
        let report = build_preview_promotion_report(input);

        assert!(!report.allowed_now);
        assert_eq!(
            report.reason,
            "preview promotion evidence is missing or malformed"
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "preview_promotion_evidence_malformed")
        );
        assert_eq!(report.input_artifacts[0].status, "malformed");
    }

    #[test]
    fn preview_promotion_ignores_supplied_status_promotion() {
        let mut input = input("typescript", "boundary_gap");
        input.evidence_path = Some("preview-evidence.json".to_string());
        input.evidence_json = Some(Ok(r#"{
              "language_status": "policy_eligible",
              "candidate_class": "other_class",
              "language": "python"
            }"#
        .to_string()));
        let report = build_preview_promotion_report(input);

        assert_eq!(report.language_status, "preview");
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "preview_promotion_language_status_ignored")
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "preview_promotion_language_mismatch")
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "preview_promotion_class_mismatch")
        );
    }

    #[test]
    fn preview_promotion_json_and_markdown_are_structured() -> Result<(), String> {
        let report = build_preview_promotion_report(input("typescript", "boundary_gap"));
        let json = render_preview_promotion_json(&report)?;
        let parsed =
            serde_json::from_str::<Value>(&json).map_err(|err| format!("json parse: {err}"))?;
        assert_eq!(
            string_path(&parsed, &["kind"]),
            Some("preview_evidence_promotion_packet".to_string())
        );
        assert_eq!(
            string_path(&parsed, &["language_status"]),
            Some("preview".to_string())
        );
        assert_eq!(
            path_value(&parsed, &["allowed_now"]).and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            path_value(&parsed, &["generated_ci_posture", "may_fail_check"])
                .and_then(Value::as_bool),
            Some(false)
        );
        let markdown = render_preview_promotion_markdown(&report);
        assert!(markdown.contains("# RIPR Preview Evidence Promotion Packet"));
        assert!(markdown.contains("Language: typescript"));
        assert!(markdown.contains("Allowed now: no"));
        assert!(markdown.contains("may fail check: no"));
        Ok(())
    }

    #[test]
    fn preview_promotion_default_paths_use_dash_class_label() {
        assert_eq!(
            default_preview_promotion_out("typescript", "boundary_gap"),
            "target/ripr/reports/preview-promotion-typescript-boundary-gap.json"
        );
        assert_eq!(
            default_preview_promotion_md_out("typescript", "boundary_gap"),
            "target/ripr/reports/preview-promotion-typescript-boundary-gap.md"
        );
    }
}
