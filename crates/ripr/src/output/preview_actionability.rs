use crate::domain::{Finding, LanguageId, LanguageStatus};
use serde_json::{Value, json};

const AUTHORITY_BOUNDARY: &str = "preview_advisory_only";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreviewActionability {
    pub(crate) authority_boundary: String,
    pub(crate) repair_packet_ready: bool,
    pub(crate) gap_state: String,
    pub(crate) actionability_category: String,
    pub(crate) why_not_actionable: String,
    pub(crate) repair_route: String,
    pub(crate) missing_actionability_fields: Vec<String>,
    pub(crate) evidence_needed_to_promote: String,
    pub(crate) raw_evidence_refs: Vec<PreviewRawEvidenceRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreviewRawEvidenceRef {
    pub(crate) raw: String,
    pub(crate) file: Option<String>,
    pub(crate) line: Option<usize>,
    pub(crate) kind: Option<String>,
    pub(crate) source_id: Option<String>,
    pub(crate) owner: Option<String>,
}

pub(crate) fn preview_actionability_for(finding: &Finding) -> Option<PreviewActionability> {
    if !matches!(
        finding.language,
        Some(LanguageId::TypeScript | LanguageId::JavaScript)
    ) || finding.language_status != Some(LanguageStatus::Preview)
    {
        return None;
    }

    let gap_state = evidence_value(finding, "gap_state: ")?;
    let actionability_category = evidence_value(finding, "actionability_category: ")?;
    let why_not_actionable = evidence_value(finding, "why_not_actionable: ")?;
    let repair_route = evidence_value(finding, "repair_route: ")?;
    let evidence_needed_to_promote = evidence_value(finding, "evidence_needed_to_promote: ")?;
    let missing_actionability_fields = evidence_value(finding, "missing_actionability_fields: ")
        .map(split_csv)
        .unwrap_or_default();
    let raw_evidence_refs = finding
        .evidence
        .iter()
        .filter_map(|entry| entry.strip_prefix("raw_evidence_ref: "))
        .map(parse_raw_evidence_ref)
        .collect::<Vec<_>>();

    Some(PreviewActionability {
        authority_boundary: AUTHORITY_BOUNDARY.to_string(),
        repair_packet_ready: false,
        gap_state: gap_state.to_string(),
        actionability_category: actionability_category.to_string(),
        why_not_actionable: why_not_actionable.to_string(),
        repair_route: repair_route.to_string(),
        missing_actionability_fields,
        evidence_needed_to_promote: evidence_needed_to_promote.to_string(),
        raw_evidence_refs,
    })
}

pub(crate) fn preview_actionability_json_value(actionability: &PreviewActionability) -> Value {
    json!({
        "authority_boundary": actionability.authority_boundary,
        "repair_packet_ready": actionability.repair_packet_ready,
        "gap_state": actionability.gap_state,
        "actionability_category": actionability.actionability_category,
        "why_not_actionable": actionability.why_not_actionable,
        "repair_route": actionability.repair_route,
        "missing_actionability_fields": actionability.missing_actionability_fields,
        "evidence_needed_to_promote": actionability.evidence_needed_to_promote,
        "raw_evidence_refs": actionability.raw_evidence_refs.iter().map(raw_ref_json).collect::<Vec<_>>(),
    })
}

pub(crate) fn is_preview_actionability_evidence_line(line: &str) -> bool {
    line.starts_with("gap_state: ")
        || line.starts_with("actionability_category: ")
        || line.starts_with("why_not_actionable: ")
        || line.starts_with("repair_route: ")
        || line.starts_with("missing_actionability_fields: ")
        || line.starts_with("evidence_needed_to_promote: ")
        || line.starts_with("raw_evidence_ref: ")
}

pub(crate) fn is_preview_actionability_missing_summary(line: &str) -> bool {
    line.starts_with("TypeScript preview actionability `")
}

fn evidence_value<'a>(finding: &'a Finding, prefix: &str) -> Option<&'a str> {
    finding
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix(prefix))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn parse_raw_evidence_ref(value: &str) -> PreviewRawEvidenceRef {
    let mut parsed = PreviewRawEvidenceRef {
        raw: value.trim().to_string(),
        file: None,
        line: None,
        kind: None,
        source_id: None,
        owner: None,
    };

    for part in value.split(';') {
        let Some((key, raw_value)) = part.split_once('=') else {
            continue;
        };
        let raw_value = raw_value.trim();
        if raw_value.is_empty() {
            continue;
        }
        match key.trim() {
            "file" => parsed.file = Some(raw_value.to_string()),
            "line" => parsed.line = raw_value.parse::<usize>().ok(),
            "kind" => parsed.kind = Some(raw_value.to_string()),
            "source_id" => parsed.source_id = Some(raw_value.to_string()),
            "owner" => parsed.owner = Some(raw_value.to_string()),
            _ => {}
        }
    }

    parsed
}

fn raw_ref_json(raw_ref: &PreviewRawEvidenceRef) -> Value {
    json!({
        "raw": raw_ref.raw,
        "file": raw_ref.file,
        "line": raw_ref.line,
        "kind": raw_ref.kind,
        "source_id": raw_ref.source_id,
        "owner": raw_ref.owner,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        is_preview_actionability_evidence_line, preview_actionability_for,
        preview_actionability_json_value,
    };
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, LanguageId,
        LanguageStatus, Probe, ProbeFamily, ProbeId, RevealEvidence, RiprEvidence, SourceLocation,
        StageEvidence, StageState,
    };

    #[test]
    fn parses_typescript_preview_actionability_strings() -> Result<(), String> {
        let finding = sample_typescript_finding();
        let actionability = preview_actionability_for(&finding)
            .ok_or_else(|| "expected structured TypeScript actionability".to_string())?;

        assert_eq!(actionability.authority_boundary, "preview_advisory_only");
        assert!(!actionability.repair_packet_ready);
        assert_eq!(actionability.gap_state, "advisory");
        assert_eq!(
            actionability.actionability_category,
            "incomplete_repair_packet"
        );
        assert_eq!(
            actionability.missing_actionability_fields,
            vec!["canonical_gap_id", "verify_command"]
        );
        assert_eq!(
            actionability.raw_evidence_refs[0].file.as_deref(),
            Some("src/lib.ts")
        );
        assert_eq!(actionability.raw_evidence_refs[0].line, Some(2));
        assert_eq!(
            actionability.raw_evidence_refs[0].source_id.as_deref(),
            Some("probe:src_lib.ts:2:typescript_preview")
        );

        let value = preview_actionability_json_value(&actionability);
        assert_eq!(value["repair_packet_ready"], false);
        assert_eq!(value["raw_evidence_refs"][0]["owner"], "applyDiscount");
        Ok(())
    }

    #[test]
    fn ignores_non_preview_and_non_actionability_lines() {
        let mut finding = sample_typescript_finding();
        finding.language = Some(LanguageId::Rust);

        assert!(preview_actionability_for(&finding).is_none());
        assert!(is_preview_actionability_evidence_line(
            "gap_state: advisory"
        ));
        assert!(!is_preview_actionability_evidence_line(
            "owner: applyDiscount"
        ));
    }

    fn sample_typescript_finding() -> Finding {
        Finding {
            id: "probe:src_lib.ts:2:typescript_preview".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_lib.ts:2:typescript_preview".to_string()),
                location: SourceLocation::new("src/lib.ts", 2, 1),
                owner: None,
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: None,
                after: Some("if (amount >= threshold) {".to_string()),
                expression: "if (amount >= threshold) {".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: stage(StageState::Yes),
                infect: stage(StageState::Unknown),
                propagate: stage(StageState::Unknown),
                reveal: RevealEvidence {
                    observe: stage(StageState::Weak),
                    discriminate: stage(StageState::Weak),
                },
            },
            confidence: 0.4,
            evidence: vec![
                "owner: applyDiscount".to_string(),
                "gap_state: advisory".to_string(),
                "actionability_category: incomplete_repair_packet".to_string(),
                "why_not_actionable: TypeScript preview lacks a complete repair packet contract"
                    .to_string(),
                "repair_route: project canonical TypeScript repair packet fields later".to_string(),
                "missing_actionability_fields: canonical_gap_id, verify_command".to_string(),
                "evidence_needed_to_promote: canonical gap identity and verify command".to_string(),
                "raw_evidence_ref: file=src/lib.ts;line=2;kind=typescript_preview_probe;source_id=probe:src_lib.ts:2:typescript_preview;owner=applyDiscount".to_string(),
            ],
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: Vec::new(),
            related_tests: Vec::new(),
            recommended_next_step: None,
            language: Some(LanguageId::TypeScript),
            language_status: Some(LanguageStatus::Preview),
            owner_kind: None,
            static_limit_kind: None,
        }
    }

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Low, "stage")
    }
}
