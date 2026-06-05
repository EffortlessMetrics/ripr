use crate::domain::{
    ExposureClass, Finding, LanguageId, LanguageStatus, OracleStrength, RelatedTest,
};
use serde_json::{Value, json};

const AUTHORITY_BOUNDARY: &str = "preview_advisory_only";
const SURFACE_SCOPE: &str = "check_json_human_sarif";
const VERIFY_STATUS: &str = "fact_only_not_delegated";
const RECEIPT_STATUS: &str = "available_not_delegated";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PerlPreviewCard {
    pub(crate) card_version: String,
    pub(crate) source: String,
    pub(crate) language: String,
    pub(crate) language_status: String,
    pub(crate) authority_boundary: String,
    pub(crate) surface_scope: String,
    pub(crate) public_projection_ready: bool,
    pub(crate) public_repair_packet: bool,
    pub(crate) repair_packet_ready: bool,
    pub(crate) agent_packet_ready: bool,
    pub(crate) gate_candidate: bool,
    pub(crate) badge_candidate: bool,
    pub(crate) ripr_zero_candidate: bool,
    pub(crate) packet_id: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) gap_state: String,
    pub(crate) changed_owner: String,
    pub(crate) evidence_class: String,
    pub(crate) repair_route: String,
    pub(crate) current_test_evidence: String,
    pub(crate) missing_discriminator: String,
    pub(crate) target_test_shape: String,
    pub(crate) suggested_test_location: String,
    pub(crate) suggested_assertion: String,
    pub(crate) verify_command: String,
    pub(crate) confidence: String,
    pub(crate) raw_evidence_refs: Vec<PerlRawEvidenceRef>,
    pub(crate) stop_if: Vec<String>,
    pub(crate) must_not_change: Vec<String>,
    pub(crate) limits: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PerlRawEvidenceRef {
    pub(crate) raw: String,
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) kind: String,
    pub(crate) source_id: String,
    pub(crate) owner: String,
    pub(crate) leg: String,
    pub(crate) sample: Option<String>,
}

pub(crate) fn perl_preview_card(finding: &Finding) -> Option<PerlPreviewCard> {
    if finding.language != Some(LanguageId::Perl)
        || finding.language_status != Some(LanguageStatus::Preview)
        || finding.class != ExposureClass::WeaklyExposed
        || has_dynamic_or_partial_boundary(finding)
    {
        return None;
    }

    let gap = finding.canonical_gap.as_ref()?;
    let repair_route = evidence_value(finding, "perl_repair_kind: ")?;
    let target_test_shape = evidence_value(finding, "perl_target_test_shape: ")?;
    let suggested_test_location = evidence_value(finding, "perl_suggested_test_location: ")?;
    let suggested_assertion = evidence_value(finding, "perl_suggested_assertion: ")?;
    let verify_command = evidence_value(finding, "perl_verify_command: ")?;
    let _receipt_command = evidence_value(finding, "perl_receipt_command: ")?;
    let stop_if = evidence_values(finding, "perl_stop_if: ");
    let must_not_change = evidence_values(finding, "perl_must_not_change: ");
    let raw_evidence_refs = raw_evidence_refs(finding)?;

    if stop_if.is_empty() || must_not_change.is_empty() {
        return None;
    }

    let missing_discriminator = finding
        .activation
        .missing_discriminators
        .first()
        .map(|missing| missing.value.clone())
        .or_else(|| evidence_value(finding, "perl_missing_discriminator: ").map(str::to_string))?;
    let packet_id = evidence_value(finding, "perl_packet_id: ")
        .map(str::to_string)
        .unwrap_or_else(|| format!("perl-preview:{}", gap.id));
    let confidence = evidence_value(finding, "perl_confidence: ")
        .unwrap_or("medium")
        .to_string();
    let current_test_evidence = evidence_value(finding, "perl_current_test_evidence: ")
        .map(str::to_string)
        .or_else(|| strongest_related_test(finding).map(current_test_evidence))?;

    Some(PerlPreviewCard {
        card_version: "perl_preview_card.v1".to_string(),
        source: "check_perl_preview".to_string(),
        language: "perl".to_string(),
        language_status: "preview".to_string(),
        authority_boundary: AUTHORITY_BOUNDARY.to_string(),
        surface_scope: SURFACE_SCOPE.to_string(),
        public_projection_ready: true,
        public_repair_packet: false,
        repair_packet_ready: false,
        agent_packet_ready: false,
        gate_candidate: false,
        badge_candidate: false,
        ripr_zero_candidate: false,
        packet_id,
        canonical_gap_id: gap.id.clone(),
        gap_state: "actionable".to_string(),
        changed_owner: gap.owner.clone(),
        evidence_class: finding.class.as_str().to_string(),
        repair_route: repair_route.to_string(),
        current_test_evidence,
        missing_discriminator,
        target_test_shape: target_test_shape.to_string(),
        suggested_test_location: suggested_test_location.to_string(),
        suggested_assertion: suggested_assertion.to_string(),
        verify_command: verify_command.to_string(),
        confidence,
        raw_evidence_refs,
        stop_if,
        must_not_change,
        limits: limits(),
    })
}

pub(crate) fn perl_preview_card_json_value(card: &PerlPreviewCard) -> Value {
    json!({
        "card_version": card.card_version.as_str(),
        "source": card.source.as_str(),
        "language": card.language.as_str(),
        "language_status": card.language_status.as_str(),
        "authority_boundary": card.authority_boundary.as_str(),
        "surface_scope": card.surface_scope.as_str(),
        "public_projection_ready": card.public_projection_ready,
        "public_repair_packet": card.public_repair_packet,
        "repair_packet_ready": card.repair_packet_ready,
        "agent_packet_ready": card.agent_packet_ready,
        "gate_candidate": card.gate_candidate,
        "badge_candidate": card.badge_candidate,
        "ripr_zero_candidate": card.ripr_zero_candidate,
        "packet_id": card.packet_id.as_str(),
        "canonical_gap_id": card.canonical_gap_id.as_str(),
        "gap_state": card.gap_state.as_str(),
        "changed_owner": card.changed_owner.as_str(),
        "evidence_class": card.evidence_class.as_str(),
        "repair_route": card.repair_route.as_str(),
        "current_test_evidence": card.current_test_evidence.as_str(),
        "missing_discriminator": card.missing_discriminator.as_str(),
        "target_test_shape": card.target_test_shape.as_str(),
        "suggested_test_location": card.suggested_test_location.as_str(),
        "suggested_assertion": card.suggested_assertion.as_str(),
        "verify": {
            "command": card.verify_command.as_str(),
            "status": VERIFY_STATUS,
        },
        "receipt": {
            "command": Value::Null,
            "status": RECEIPT_STATUS,
            "guidance": "receipt evidence is required internally, but public Perl preview surfaces do not delegate receipt commands yet",
        },
        "confidence": card.confidence.as_str(),
        "raw_evidence_refs": card.raw_evidence_refs.iter().map(raw_ref_json).collect::<Vec<_>>(),
        "stop_if": &card.stop_if,
        "must_not_change": &card.must_not_change,
        "limits": &card.limits,
    })
}

fn strongest_related_test(finding: &Finding) -> Option<&RelatedTest> {
    finding
        .related_tests
        .iter()
        .max_by_key(|test| test.oracle_strength.rank())
        .filter(|test| test.oracle_strength != OracleStrength::None)
}

fn current_test_evidence(test: &RelatedTest) -> String {
    let oracle = test
        .oracle
        .as_deref()
        .map(|value| format!(": {value}"))
        .unwrap_or_default();
    format!(
        "{}:{} {} currently has oracle_strength={}, oracle_kind={}{}",
        test.file.display(),
        test.line,
        test.name,
        test.oracle_strength.as_str(),
        test.oracle_kind.as_str(),
        oracle
    )
}

fn evidence_value<'a>(finding: &'a Finding, prefix: &str) -> Option<&'a str> {
    finding
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix(prefix))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn evidence_values(finding: &Finding, prefix: &str) -> Vec<String> {
    finding
        .evidence
        .iter()
        .filter_map(|entry| entry.strip_prefix(prefix))
        .flat_map(split_csv)
        .collect()
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn raw_evidence_refs(finding: &Finding) -> Option<Vec<PerlRawEvidenceRef>> {
    let mut refs = Vec::new();
    for entry in &finding.evidence {
        if let Some(value) = entry.strip_prefix("raw_evidence_ref: ") {
            refs.push(parse_raw_evidence_ref(value)?);
        }
    }
    if refs.is_empty() { None } else { Some(refs) }
}

fn parse_raw_evidence_ref(value: &str) -> Option<PerlRawEvidenceRef> {
    let mut file = None;
    let mut line = None;
    let mut kind = None;
    let mut source_id = None;
    let mut owner = None;
    let mut leg = None;
    let mut sample = None;

    for part in value.split(';') {
        let Some((key, raw_value)) = part.split_once('=') else {
            continue;
        };
        let raw_value = raw_value.trim();
        if raw_value.is_empty() {
            continue;
        }
        match key.trim() {
            "file" => {
                if !is_safe_repo_relative_path(raw_value) {
                    return None;
                }
                file = Some(raw_value.to_string());
            }
            "line" => line = raw_value.parse::<usize>().ok().filter(|value| *value > 0),
            "kind" => kind = Some(raw_value.to_string()),
            "source_id" => source_id = Some(raw_value.to_string()),
            "owner" => owner = Some(raw_value.to_string()),
            "leg" => leg = Some(raw_value.to_string()),
            "sample" => sample = Some(raw_value.to_string()),
            _ => {}
        }
    }

    Some(PerlRawEvidenceRef {
        raw: value.trim().to_string(),
        file: file?,
        line: line?,
        kind: kind?,
        source_id: source_id?,
        owner: owner?,
        leg: leg?,
        sample,
    })
}

fn is_safe_repo_relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('\\')
        && !value.contains("://")
        && !value.starts_with('/')
        && value.as_bytes().get(1).is_none_or(|byte| *byte != b':')
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn raw_ref_json(reference: &PerlRawEvidenceRef) -> Value {
    json!({
        "raw": reference.raw.as_str(),
        "file": reference.file.as_str(),
        "line": reference.line,
        "kind": reference.kind.as_str(),
        "source_id": reference.source_id.as_str(),
        "owner": reference.owner.as_str(),
        "leg": reference.leg.as_str(),
        "sample": reference.sample.as_deref(),
    })
}

fn has_dynamic_or_partial_boundary(finding: &Finding) -> bool {
    finding.evidence.iter().any(|entry| {
        matches!(
            entry.as_str(),
            "perl_dynamic_boundary: true"
                | "perl_boundary_status: dynamic"
                | "perl_packet_status: partial"
                | "perl_fact_packet_status: partial"
        )
    })
}

fn limits() -> Vec<String> {
    vec![
        "Perl preview fact-packet evidence only.".to_string(),
        "No live perl-lsp session, Perl runtime execution, generated tests, source edits, provider calls, badge authority, gate authority, or RIPR Zero authority.".to_string(),
        "This JSON card is advisory; public agent-packet, SARIF, PR, CI, LSP, and swarm routing surfaces remain separate work.".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{perl_preview_card, perl_preview_card_json_value};
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, FindingCanonicalGap,
        LanguageId, LanguageStatus, MissingDiscriminatorFact, OracleKind, OracleStrength, Probe,
        ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation,
        StageEvidence, StageState,
    };
    use std::path::PathBuf;

    #[test]
    fn perl_preview_card_projects_public_preview_card() -> Result<(), String> {
        let finding = sample_perl_finding();
        let card = perl_preview_card(&finding).ok_or_else(|| "expected Perl card".to_string())?;

        assert_eq!(card.card_version, "perl_preview_card.v1");
        assert_eq!(card.source, "check_perl_preview");
        assert_eq!(card.language, "perl");
        assert_eq!(card.language_status, "preview");
        assert_eq!(card.authority_boundary, "preview_advisory_only");
        assert_eq!(card.surface_scope, "check_json_human_sarif");
        assert!(card.public_projection_ready);
        assert!(!card.public_repair_packet);
        assert!(!card.repair_packet_ready);
        assert!(!card.agent_packet_ready);
        assert!(!card.gate_candidate);
        assert!(!card.badge_candidate);
        assert!(!card.ripr_zero_candidate);
        assert_eq!(
            card.canonical_gap_id,
            "gap:perl:lib/My/App.pm:My::App::discount:return_value:exact_return_assertion:return_value"
        );
        assert_eq!(card.changed_owner, "perl:lib/My/App.pm::My::App::discount");
        assert_eq!(card.repair_route, "add_exact_return_assertion");
        assert_eq!(card.target_test_shape, "Test::More exact_return_assertion");
        assert_eq!(card.suggested_test_location, "t/app.t::discount_smoke");
        assert_eq!(card.verify_command, "prove t/app.t");

        let value = perl_preview_card_json_value(&card);
        assert_eq!(value["surface_scope"], "check_json_human_sarif");
        assert_eq!(value["public_repair_packet"], false);
        assert_eq!(value["public_projection_ready"], true);
        assert_eq!(value["repair_packet_ready"], false);
        assert_eq!(value["agent_packet_ready"], false);
        assert_eq!(value["gate_candidate"], false);
        assert_eq!(value["badge_candidate"], false);
        assert_eq!(value["ripr_zero_candidate"], false);
        assert_eq!(value["verify"]["command"], "prove t/app.t");
        assert_eq!(value["verify"]["status"], "fact_only_not_delegated");
        assert!(value["receipt"]["command"].is_null());
        assert_eq!(value["receipt"]["status"], "available_not_delegated");
        assert_eq!(value["raw_evidence_refs"][0]["file"], "lib/My/App.pm");
        assert_eq!(value["raw_evidence_refs"][0]["line"], 8);
        assert!(value.get("allowed_edit_surface").is_none());
        assert!(value.get("forbidden_files").is_none());
        assert!(value.get("allowed_edit_boundaries").is_none());
        assert!(value.get("forbidden_edit_boundaries").is_none());
        assert!(value["receipt"].get("argv").is_none());
        Ok(())
    }

    #[test]
    fn perl_preview_card_fails_closed_without_strict_projection_inputs() {
        let mut missing_receipt = sample_perl_finding();
        missing_receipt
            .evidence
            .retain(|line| !line.starts_with("perl_receipt_command: "));
        assert!(perl_preview_card(&missing_receipt).is_none());

        let mut non_perl = sample_perl_finding();
        non_perl.language = Some(LanguageId::Python);
        assert!(perl_preview_card(&non_perl).is_none());

        let mut missing_refs = sample_perl_finding();
        missing_refs
            .evidence
            .retain(|line| !line.starts_with("raw_evidence_ref: "));
        assert!(perl_preview_card(&missing_refs).is_none());

        let mut dynamic_boundary = sample_perl_finding();
        dynamic_boundary
            .evidence
            .push("perl_dynamic_boundary: true".to_string());
        assert!(perl_preview_card(&dynamic_boundary).is_none());

        let mut partial_packet = sample_perl_finding();
        partial_packet
            .evidence
            .push("perl_packet_status: partial".to_string());
        assert!(perl_preview_card(&partial_packet).is_none());

        let mut weak_oracle = sample_perl_finding();
        weak_oracle.related_tests[0].oracle_strength = OracleStrength::None;
        weak_oracle
            .evidence
            .retain(|line| !line.starts_with("perl_current_test_evidence: "));
        assert!(perl_preview_card(&weak_oracle).is_none());
    }

    #[test]
    fn perl_preview_card_rejects_unsafe_or_incomplete_raw_refs() {
        for unsafe_file in [
            format!("{}:/repo/lib/My/App.pm", "C"),
            "lib\\My\\App.pm".to_string(),
            "/repo/lib/My/App.pm".to_string(),
            "../lib/My/App.pm".to_string(),
            "lib//My/App.pm".to_string(),
        ] {
            let mut finding = sample_perl_finding();
            finding.evidence = finding
                .evidence
                .into_iter()
                .map(|line| {
                    if line.starts_with("raw_evidence_ref: leg=perl_change;") {
                        format!(
                            "raw_evidence_ref: leg=perl_change;file={unsafe_file};line=8;kind=perl_change;source_id=change:lib/My/App.pm:8:return;owner=perl:lib/My/App.pm::My::App::discount;sample=return $discounted"
                        )
                    } else {
                        line
                    }
                })
                .collect();
            assert!(
                perl_preview_card(&finding).is_none(),
                "expected unsafe file path {unsafe_file} to fail closed"
            );
        }

        let mut missing_provenance = sample_perl_finding();
        missing_provenance.evidence = missing_provenance
            .evidence
            .into_iter()
            .map(|line| {
                if line.starts_with("raw_evidence_ref: leg=perl_change;") {
                    "raw_evidence_ref: leg=perl_change;file=lib/My/App.pm;line=8;kind=perl_change"
                        .to_string()
                } else {
                    line
                }
            })
            .collect();
        assert!(perl_preview_card(&missing_provenance).is_none());
    }

    pub(crate) fn sample_perl_finding() -> Finding {
        Finding {
            id: "probe:lib_My_App_pm:8:perl_return".to_string(),
            canonical_gap: Some(FindingCanonicalGap {
                id: "gap:perl:lib/My/App.pm:My::App::discount:return_value:exact_return_assertion:return_value"
                    .to_string(),
                language: "perl".to_string(),
                file: "lib/My/App.pm".to_string(),
                owner: "perl:lib/My/App.pm::My::App::discount".to_string(),
                behavior_kind: "return_value".to_string(),
                probe_kind: "exact_return_assertion".to_string(),
                normalized_discriminator: "return_value".to_string(),
            }),
            probe: Probe {
                id: ProbeId("probe:lib_My_App_pm:8:perl_return".to_string()),
                location: SourceLocation::new("lib/My/App.pm", 8, 5),
                owner: None,
                family: ProbeFamily::ReturnValue,
                delta: DeltaKind::Value,
                before: Some("return $price".to_string()),
                after: Some("return $discounted".to_string()),
                expression: "return $discounted".to_string(),
                expected_sinks: vec!["return_value".to_string()],
                required_oracles: vec!["exact_return_assertion".to_string()],
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: stage("Perl fact packet links the related test to the changed owner"),
                infect: stage("Changed return value reaches the owner result"),
                propagate: stage("Return value can propagate to Test::More assertion"),
                reveal: RevealEvidence {
                    observe: stage("Related test reaches the changed owner"),
                    discriminate: stage("Exact return discriminator is missing"),
                },
            },
            confidence: 0.8,
            evidence: vec![
                "perl_packet_id: perl-preview:gap-return".to_string(),
                "perl_repair_kind: add_exact_return_assertion".to_string(),
                "perl_target_test_shape: Test::More exact_return_assertion".to_string(),
                "perl_suggested_test_location: t/app.t::discount_smoke".to_string(),
                "perl_suggested_assertion: assert the exact returned `return_value` value".to_string(),
                "perl_verify_command: prove t/app.t".to_string(),
                "perl_receipt_command: ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id perl-gap --json".to_string(),
                "perl_confidence: medium".to_string(),
                "perl_allowed_edit_boundary: t/app.t".to_string(),
                "perl_forbidden_edit_boundary: lib/My/App.pm, badges/ripr-plus.json".to_string(),
                "perl_stop_if: perl-lsp packet status changes".to_string(),
                "perl_stop_if: related test no longer reaches owner".to_string(),
                "perl_must_not_change: do not edit Perl production code".to_string(),
                "perl_must_not_change: do not add suppressions or intent ledger entries".to_string(),
                "raw_evidence_ref: leg=perl_change;file=lib/My/App.pm;line=8;kind=perl_change;source_id=change:lib/My/App.pm:8:return;owner=perl:lib/My/App.pm::My::App::discount;sample=return $discounted".to_string(),
                "raw_evidence_ref: leg=perl_oracle;file=t/app.t;line=7;kind=perl_oracle;source_id=oracle:t/app.t:7:is;owner=perl:lib/My/App.pm::My::App::discount;sample=is(discount(...), 90)".to_string(),
            ],
            missing: vec!["return_value".to_string()],
            flow_sinks: vec![],
            activation: ActivationEvidence {
                observed_values: vec![],
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "return_value".to_string(),
                    reason: "Related Perl test reaches the owner but lacks an exact return discriminator"
                        .to_string(),
                    flow_sink: None,
                }],
            },
            stop_reasons: vec![],
            related_tests: vec![RelatedTest {
                name: "discount_smoke".to_string(),
                file: PathBuf::from("t/app.t"),
                line: 7,
                oracle: Some("ok(discount(...))".to_string()),
                oracle_kind: OracleKind::SmokeOnly,
                oracle_strength: OracleStrength::Weak,
            }],
            recommended_next_step: Some("Add a focused Perl assertion.".to_string()),
            language: Some(LanguageId::Perl),
            language_status: Some(LanguageStatus::Preview),
            owner_kind: None,
            static_limit_kind: None,
        }
    }

    fn stage(summary: &str) -> StageEvidence {
        StageEvidence::new(StageState::Yes, Confidence::Medium, summary)
    }
}
