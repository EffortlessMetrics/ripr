use crate::domain::{Finding, LanguageId, LanguageStatus};
use crate::output::agent_seam_packets::validate_agent_gap_record_packet;
use crate::output::gap_decision_ledger::GapRecord;
use crate::output::typescript_packet_projection::typescript_gap_record_for;
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
    pub(crate) missing_graph_legs: Vec<String>,
    pub(crate) unlock_condition: Option<String>,
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
    pub(crate) leg: Option<String>,
    pub(crate) sample: Option<String>,
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
    let repair_route = evidence_value(finding, "repair_route: ")?;
    let evidence_needed_to_promote = evidence_value(finding, "evidence_needed_to_promote: ")?;
    let missing_graph_legs = evidence_value(finding, "missing_graph_legs: ")
        .map(split_csv)
        .unwrap_or_default();
    let unlock_condition = evidence_value(finding, "unlock_condition: ").map(ToString::to_string);
    let raw_evidence_refs = finding
        .evidence
        .iter()
        .filter_map(|entry| entry.strip_prefix("raw_evidence_ref: "))
        .map(parse_raw_evidence_ref)
        .collect::<Vec<_>>();

    // §1 / §2 (RIPR-SPEC-0087 §PR7): compute repair_packet_ready by projecting
    // a GapRecord from the finding and calling the SHARED Rust validator.
    // `None` packet ⇒ `false` automatically (fail-closed default).
    // No parallel TypeScript validator is introduced: the only flip authority
    // is `validate_agent_gap_record_packet`.
    let packet = typescript_gap_record_for(finding);
    let repair_packet_ready = packet
        .as_ref()
        .is_some_and(|record| validate_agent_gap_record_packet(record).is_ok());

    // If the validator rejected the packet, surface why so the missing field
    // is visible to the operator (§1.3).
    let why_not_actionable_computed = if let Some(record) = packet.as_ref() {
        match validate_agent_gap_record_packet(record) {
            Ok(()) => {
                // actionable — this field now reads as the "why actionable"
                // line (the renderer relabels it). It must NOT contradict the
                // complete packet, so it names the resolved contract fields.
                "complete repair packet — package root, runner, owner, oracle target, verify, receipt, and edit cage all resolved; delegatable (advisory)".to_string()
            }
            Err(reason) => {
                // Surface the validator's reason alongside the evidence reason
                let base = evidence_value(finding, "why_not_actionable: ")
                    .unwrap_or("incomplete repair packet")
                    .to_string();
                format!("{base}; validator: {reason}")
            }
        }
    } else {
        evidence_value(finding, "why_not_actionable: ")
            .unwrap_or("TypeScript preview lacks a complete repair packet contract")
            .to_string()
    };

    // §1.3 / RIPR-SPEC-0088 §PR8: flip gap_state + actionability_category when
    // repair_packet_ready. For the actionable case the incomplete-packet
    // evidence strings (repair_route "only after ... available",
    // evidence_needed_to_promote, missing fields) are written for blocked
    // cases and would directly contradict the complete packet, so we replace
    // them with actionable-reading values: the actual repair action and an
    // empty "needed" list.
    let (
        resolved_gap_state,
        resolved_category,
        resolved_missing_fields,
        resolved_repair_route,
        resolved_evidence_needed,
    ) = if repair_packet_ready {
        (
            "actionable".to_string(),
            "complete_repair_packet".to_string(),
            Vec::new(),
            actionable_repair_route(packet.as_ref()),
            String::new(),
        )
    } else {
        (
            gap_state.to_string(),
            actionability_category.to_string(),
            evidence_value(finding, "missing_actionability_fields: ")
                .map(split_csv)
                .unwrap_or_default(),
            repair_route.to_string(),
            evidence_needed_to_promote.to_string(),
        )
    };

    Some(PreviewActionability {
        authority_boundary: AUTHORITY_BOUNDARY.to_string(),
        repair_packet_ready,
        gap_state: resolved_gap_state,
        actionability_category: resolved_category,
        why_not_actionable: why_not_actionable_computed,
        repair_route: resolved_repair_route,
        missing_actionability_fields: resolved_missing_fields,
        missing_graph_legs,
        unlock_condition,
        evidence_needed_to_promote: resolved_evidence_needed,
        raw_evidence_refs,
    })
}

/// Build the actionable repair-route line for a complete TypeScript packet.
///
/// The incomplete-packet `repair_route` ("project ... only after verify,
/// receipt, evidence refs, and edit boundaries are available") is false for an
/// actionable finding — those fields ARE available. This names the actual
/// repair action instead, derived from the projected GapRecord's repair route
/// (suggested assertion shape / missing discriminator), falling back to a
/// concise generic action.
fn actionable_repair_route(packet: Option<&GapRecord>) -> String {
    if let Some(route) = packet.and_then(|record| record.repair_route.as_ref()) {
        if let Some(shape) = route.assertion_shape.as_deref().filter(|s| !s.is_empty()) {
            return format!(
                "add or strengthen the focused assertion `{shape}` in the related test"
            );
        }
        if let Some(disc) = route
            .missing_discriminator
            .as_deref()
            .filter(|d| !d.is_empty())
        {
            return format!(
                "add a focused assertion for the missing discriminator `{disc}` in the related test"
            );
        }
    }
    "add or strengthen a focused assertion in the related test to cover the changed behavior"
        .to_string()
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
        "missing_graph_legs": actionability.missing_graph_legs,
        "unlock_condition": actionability.unlock_condition.as_deref(),
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
        || line.starts_with("missing_graph_legs: ")
        || line.starts_with("unlock_condition: ")
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
        leg: None,
        sample: None,
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
            "leg" => parsed.leg = Some(raw_value.to_string()),
            "sample" => parsed.sample = Some(raw_value.to_string()),
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
        "leg": raw_ref.leg,
        "sample": raw_ref.sample,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        actionable_repair_route, is_preview_actionability_evidence_line, preview_actionability_for,
        preview_actionability_json_value,
    };
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, LanguageId,
        LanguageStatus, Probe, ProbeFamily, ProbeId, RevealEvidence, RiprEvidence, SourceLocation,
        StageEvidence, StageState,
    };
    use crate::output::gap_decision_ledger::{GapRecord, GapRepairRoute};

    #[test]
    fn actionable_repair_route_names_assertion_shape_not_blocked_text() {
        let record = GapRecord {
            repair_route: Some(GapRepairRoute {
                assertion_shape: Some("expect(result).toBe(50)".to_string()),
                ..GapRepairRoute::default()
            }),
            ..GapRecord::default()
        };
        let route = actionable_repair_route(Some(&record));
        // The actionable route must name the real repair action and must NOT
        // contain the blocked-case "only after ... available" phrasing
        // (RIPR-SPEC-0088 §PR8).
        assert!(
            route.contains("expect(result).toBe(50)"),
            "expected assertion shape in route, got {route:?}"
        );
        assert!(
            !route.contains("only after"),
            "actionable route must not contain blocked-case text, got {route:?}"
        );
    }

    #[test]
    fn actionable_repair_route_falls_back_to_discriminator_then_generic() {
        let record = GapRecord {
            repair_route: Some(GapRepairRoute {
                missing_discriminator: Some("amount == threshold".to_string()),
                ..GapRepairRoute::default()
            }),
            ..GapRecord::default()
        };
        assert!(
            actionable_repair_route(Some(&record)).contains("amount == threshold"),
            "expected discriminator fallback"
        );
        // No repair route at all → generic, still actionable-reading.
        let generic = actionable_repair_route(None);
        assert!(generic.contains("focused assertion"), "got {generic:?}");
        assert!(!generic.contains("only after"), "got {generic:?}");
    }

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
            actionability.missing_graph_legs,
            vec!["verify_command", "receipt_command"]
        );
        assert_eq!(
            actionability.unlock_condition.as_deref(),
            Some("project complete repair packet fields before public projection")
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
        assert_eq!(
            actionability.raw_evidence_refs[0].leg.as_deref(),
            Some("rust_seam")
        );
        assert_eq!(
            actionability.raw_evidence_refs[0].sample.as_deref(),
            Some("if amount >= threshold")
        );

        let value = preview_actionability_json_value(&actionability);
        assert_eq!(value["repair_packet_ready"], false);
        assert_eq!(value["raw_evidence_refs"][0]["owner"], "applyDiscount");
        assert_eq!(value["raw_evidence_refs"][0]["leg"], "rust_seam");
        assert_eq!(
            value["unlock_condition"],
            "project complete repair packet fields before public projection"
        );
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
        assert!(is_preview_actionability_evidence_line(
            "unlock_condition: add graph legs"
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
                "missing_graph_legs: verify_command, receipt_command".to_string(),
                "unlock_condition: project complete repair packet fields before public projection"
                    .to_string(),
                "evidence_needed_to_promote: canonical gap identity and verify command".to_string(),
                "raw_evidence_ref: leg=rust_seam;file=src/lib.ts;line=2;kind=typescript_preview_probe;source_id=probe:src_lib.ts:2:typescript_preview;owner=applyDiscount;sample=if amount >= threshold".to_string(),
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
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
        }
    }

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Low, "stage")
    }
}
