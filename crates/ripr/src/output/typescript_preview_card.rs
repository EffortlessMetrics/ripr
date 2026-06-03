use crate::domain::{
    Finding, LanguageId, LanguageStatus, OracleKind, ProbeFamily, RelatedTest, StaticLimitKind,
};
use crate::output::preview_actionability::preview_actionability_for;
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptPreviewCard {
    pub(crate) card_version: String,
    pub(crate) source: String,
    pub(crate) language: String,
    pub(crate) language_status: String,
    pub(crate) authority_boundary: String,
    pub(crate) owner: String,
    pub(crate) owner_kind: Option<String>,
    pub(crate) probe_family: String,
    pub(crate) changed_behavior: String,
    pub(crate) related_test: Option<TypeScriptPreviewCardRelatedTest>,
    pub(crate) oracle_kind: String,
    pub(crate) oracle_strength: String,
    pub(crate) bun_cross_language_grip: Option<TypeScriptBunCrossLanguageGrip>,
    pub(crate) missing_discriminator: Option<String>,
    pub(crate) suggested_assertion_shape: String,
    pub(crate) static_limits: Vec<String>,
    pub(crate) verify_command: Option<String>,
    pub(crate) why_not_actionable: String,
    pub(crate) repair_route: String,
    pub(crate) repair_packet_ready: bool,
    pub(crate) limits: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptPreviewCardRelatedTest {
    pub(crate) name: String,
    pub(crate) file: String,
    pub(crate) line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptBunCrossLanguageGrip {
    pub(crate) state: String,
    pub(crate) rust_file: String,
    pub(crate) rust_owner: String,
    pub(crate) rust_boundary: String,
    pub(crate) ts_test_file: String,
    pub(crate) ts_verdict: String,
    pub(crate) bridge_confidence: String,
    pub(crate) missing_discriminators: Vec<String>,
    pub(crate) action: String,
    pub(crate) suggested_test_file: String,
    pub(crate) authority_boundary: String,
    pub(crate) repair_packet_ready: bool,
}

pub(crate) fn typescript_preview_card(finding: &Finding) -> Option<TypeScriptPreviewCard> {
    if !matches!(
        finding.language,
        Some(LanguageId::TypeScript | LanguageId::JavaScript)
    ) || finding.language_status != Some(LanguageStatus::Preview)
    {
        return None;
    }

    let actionability = preview_actionability_for(finding)?;
    let language = finding.language?.as_str().to_string();
    let owner = evidence_value(finding, "owner: ")
        .or_else(|| {
            finding
                .probe
                .owner
                .as_ref()
                .and_then(|owner| owner.0.rsplit("::").next())
        })
        .unwrap_or("unknown")
        .to_string();
    let strongest = strongest_related_test(finding);
    let oracle_kind = strongest
        .map(|test| test.oracle_kind.as_str())
        .unwrap_or("unknown")
        .to_string();
    let oracle_strength = strongest
        .map(|test| test.oracle_strength.as_str())
        .unwrap_or("none")
        .to_string();
    let missing_discriminator = finding
        .activation
        .missing_discriminators
        .first()
        .map(|missing| missing.value.clone());
    let static_limits = finding
        .static_limit_kind
        .map(static_limit_label)
        .into_iter()
        .collect::<Vec<_>>();
    let bun_cross_language_grip = bun_cross_language_grip(finding);

    Some(TypeScriptPreviewCard {
        card_version: "typescript_preview_card.v1".to_string(),
        source: "check_typescript_preview".to_string(),
        language,
        language_status: "preview".to_string(),
        authority_boundary: actionability.authority_boundary.clone(),
        owner,
        owner_kind: finding.owner_kind.map(|kind| kind.as_str().to_string()),
        probe_family: finding.probe.family.as_str().to_string(),
        changed_behavior: changed_behavior(finding),
        related_test: strongest.map(related_test_card),
        oracle_kind,
        oracle_strength,
        bun_cross_language_grip,
        missing_discriminator: missing_discriminator.clone(),
        suggested_assertion_shape: suggested_assertion_shape(
            finding,
            strongest.map(|test| &test.oracle_kind),
            missing_discriminator.as_deref(),
            &static_limits,
        ),
        static_limits,
        verify_command: evidence_value(finding, "suggested_verify_command: ")
            .map(ToString::to_string),
        why_not_actionable: actionability.why_not_actionable,
        repair_route: actionability.repair_route,
        repair_packet_ready: actionability.repair_packet_ready,
        limits: limits(),
    })
}

pub(crate) fn typescript_preview_card_json_value(card: &TypeScriptPreviewCard) -> Value {
    json!({
        "card_version": card.card_version.as_str(),
        "source": card.source.as_str(),
        "language": card.language.as_str(),
        "language_status": card.language_status.as_str(),
        "authority_boundary": card.authority_boundary.as_str(),
        "owner": card.owner.as_str(),
        "owner_kind": card.owner_kind.as_deref(),
        "probe_family": card.probe_family.as_str(),
        "changed_behavior": card.changed_behavior.as_str(),
        "related_test": card.related_test.as_ref().map(|test| json!({
            "name": test.name.as_str(),
            "file": test.file.as_str(),
            "line": test.line,
        })),
        "oracle_kind": card.oracle_kind.as_str(),
        "oracle_strength": card.oracle_strength.as_str(),
        "bun_cross_language_grip": card.bun_cross_language_grip.as_ref().map(|grip| json!({
            "state": grip.state.as_str(),
            "rust_seam": {
                "file": grip.rust_file.as_str(),
                "owner": grip.rust_owner.as_str(),
                "boundary": grip.rust_boundary.as_str(),
            },
            "typescript_evidence": {
                "test_file": grip.ts_test_file.as_str(),
                "verdict": grip.ts_verdict.as_str(),
                "bridge_confidence": grip.bridge_confidence.as_str(),
                "missing_discriminators": &grip.missing_discriminators,
            },
            "action": grip.action.as_str(),
            "suggested_test_file": grip.suggested_test_file.as_str(),
            "authority_boundary": grip.authority_boundary.as_str(),
            "repair_packet_ready": grip.repair_packet_ready,
        })),
        "missing_discriminator": card.missing_discriminator.as_deref(),
        "suggested_assertion_shape": card.suggested_assertion_shape.as_str(),
        "static_limits": &card.static_limits,
        "verify": {
            "command": card.verify_command.as_deref(),
        },
        "why_not_actionable": card.why_not_actionable.as_str(),
        "repair_route": card.repair_route.as_str(),
        "repair_packet_ready": card.repair_packet_ready,
        "limits": &card.limits,
    })
}

fn changed_behavior(finding: &Finding) -> String {
    let expression = finding
        .probe
        .after
        .as_deref()
        .unwrap_or(finding.probe.expression.as_str())
        .trim();
    format!(
        "{} changed at {}:{}: `{expression}`",
        finding.probe.family.as_str(),
        finding.probe.location.file.display(),
        finding.probe.location.line
    )
}

fn related_test_card(test: &RelatedTest) -> TypeScriptPreviewCardRelatedTest {
    TypeScriptPreviewCardRelatedTest {
        name: test.name.clone(),
        file: test.file.display().to_string(),
        line: test.line,
    }
}

fn suggested_assertion_shape(
    finding: &Finding,
    oracle_kind: Option<&OracleKind>,
    missing_discriminator: Option<&str>,
    static_limits: &[String],
) -> String {
    if !static_limits.is_empty() {
        return "Resolve the named static limitation before selecting an assertion shape."
            .to_string();
    }

    match oracle_kind {
        Some(OracleKind::Snapshot) => {
            "Add an exact-value toBe/toEqual/toStrictEqual assertion alongside the snapshot."
                .to_string()
        }
        Some(OracleKind::SmokeOnly) => {
            "Replace or augment the truthiness check with an exact-value toBe/toEqual/toStrictEqual assertion."
                .to_string()
        }
        Some(OracleKind::MockExpectation) => {
            if strongest_related_test(finding)
                .and_then(|test| test.oracle.as_deref())
                .is_some_and(|oracle| oracle.contains("toHaveBeenCalledWith") || oracle.contains("toHaveBeenCalledTimes"))
            {
                return "Keep the bounded mock interaction evidence advisory until strict packet fields are available."
                    .to_string();
            }
            "Name the mock callee and payload before suggesting a repair shape.".to_string()
        }
        Some(OracleKind::BroadError) => {
            "Use literal toThrow/rejects.toThrow or rejects.toMatchObject payload evidence; broad error checks stay weak."
                .to_string()
        }
        Some(OracleKind::ExactErrorVariant) => {
            "Exact error payload evidence is present; no repair packet is emitted without strict fields."
                .to_string()
        }
        Some(OracleKind::ExactValue) => {
            "Exact-value evidence is present; verify it targets the changed discriminator."
                .to_string()
        }
        _ => match finding.probe.family {
            ProbeFamily::Predicate => {
                if let Some(discriminator) = missing_discriminator {
                    return format!("Add an exact boundary assertion for `{discriminator}`.");
                }
                "Add an exact boundary assertion for the changed predicate.".to_string()
            }
            ProbeFamily::ReturnValue => {
                "Add an exact return-value assertion for the changed output.".to_string()
            }
            ProbeFamily::ErrorPath => {
                "Add an exact error payload or variant assertion for the changed error path."
                    .to_string()
            }
            ProbeFamily::FieldConstruction => {
                "Add an exact field/object assertion for the constructed value.".to_string()
            }
            ProbeFamily::SideEffect | ProbeFamily::CallDeletion => {
                "Add an exact call/output/log assertion for the changed side effect.".to_string()
            }
            _ => "Add an exact assertion for the changed behavior.".to_string(),
        },
    }
}

fn strongest_related_test(finding: &Finding) -> Option<&RelatedTest> {
    finding
        .related_tests
        .iter()
        .max_by_key(|test| test.oracle_strength.rank())
}

fn evidence_value<'a>(finding: &'a Finding, prefix: &str) -> Option<&'a str> {
    finding
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix(prefix))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn bun_cross_language_grip(finding: &Finding) -> Option<TypeScriptBunCrossLanguageGrip> {
    let hint = evidence_value(finding, "typescript_bun_ub_bridge_hint: ")?;
    let verdict = evidence_value(finding, "typescript_bun_ub_bridge_verdict: ")?;
    let grip = evidence_value(finding, "typescript_bun_ub_cross_language_grip: ");
    let ts_verdict = verdict.split_whitespace().next()?.to_string();
    let missing = keyed_value(verdict, "missing_discriminators")
        .map(|value| {
            if value == "none" {
                Vec::new()
            } else {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(ToString::to_string)
                    .collect()
            }
        })
        .unwrap_or_default();
    let state = grip
        .and_then(|line| keyed_value(line, "state"))
        .unwrap_or_else(|| cross_language_state_for_verdict(&ts_verdict).to_string());

    Some(TypeScriptBunCrossLanguageGrip {
        state,
        rust_file: keyed_value(hint, "rust_file")?,
        rust_owner: keyed_value(hint, "rust_owner")?,
        rust_boundary: keyed_value(hint, "rust_boundary")?,
        ts_test_file: keyed_value(hint, "ts_test_file")?,
        ts_verdict,
        bridge_confidence: keyed_value(hint, "confidence")?,
        missing_discriminators: missing,
        action: keyed_value(verdict, "action")?,
        suggested_test_file: keyed_value(verdict, "suggested_test_file")?,
        authority_boundary: grip
            .and_then(|line| keyed_value(line, "authority"))
            .unwrap_or_else(|| "preview_advisory_only".to_string()),
        repair_packet_ready: grip
            .and_then(|line| keyed_value(line, "repair_packet_ready"))
            .is_some_and(|value| value == "true"),
    })
}

fn keyed_value(input: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=");
    let start = input.find(&needle)? + needle.len();
    let rest = &input[start..];
    if let Some(quoted) = rest.strip_prefix('"') {
        return quoted.split_once('"').map(|(value, _)| value.to_string());
    }
    rest.split_whitespace().next().map(ToString::to_string)
}

fn cross_language_state_for_verdict(verdict: &str) -> &'static str {
    match verdict {
        "ts_discriminated" => "rust_ungripped_ts_discriminated",
        "ts_missing_resizable" | "ts_missing_shared" | "ts_missing_shared_and_resizable" => {
            "rust_ungripped_ts_missing_discriminator"
        }
        "ts_mention_not_observer" => "ts_mention_not_observer",
        "bridge_unknown" => "bridge_unknown",
        _ => "bridge_unknown",
    }
}

fn static_limit_label(kind: StaticLimitKind) -> String {
    kind.as_str().to_string()
}

fn limits() -> Vec<String> {
    vec![
        "Syntax-first TypeScript/JavaScript preview evidence only.".to_string(),
        "No tsc, tsserver, package graph, Jest/Vitest runtime, mutation execution, generated tests, provider calls, source edits, gate authority, or badge authority.".to_string(),
        "This card is advisory and is not a repair packet.".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{typescript_preview_card, typescript_preview_card_json_value};
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, LanguageId,
        LanguageStatus, MissingDiscriminatorFact, OracleKind, OracleStrength, OwnerKind, Probe,
        ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation,
        StageEvidence, StageState, StaticLimitKind,
    };
    use std::path::PathBuf;

    #[test]
    fn typescript_preview_card_projects_advisory_fields() -> Result<(), String> {
        let mut finding = sample_finding(OracleKind::SmokeOnly, OracleStrength::Smoke);
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "amount == threshold".to_string(),
            reason: "missing exact boundary".to_string(),
            flow_sink: None,
        }];

        let card = typescript_preview_card(&finding)
            .ok_or_else(|| "expected TypeScript preview card".to_string())?;
        assert_eq!(card.card_version, "typescript_preview_card.v1");
        assert_eq!(card.language, "typescript");
        assert_eq!(card.language_status, "preview");
        assert_eq!(card.authority_boundary, "preview_advisory_only");
        assert_eq!(card.owner, "discountedTotal");
        assert_eq!(card.owner_kind.as_deref(), Some("function"));
        assert!(!card.repair_packet_ready);
        assert!(card.suggested_assertion_shape.contains("truthiness check"));

        let json = typescript_preview_card_json_value(&card);
        assert_eq!(json["repair_packet_ready"], false);
        assert_eq!(json["related_test"]["name"], "discount smoke");
        assert_eq!(json["missing_discriminator"], "amount == threshold");
        Ok(())
    }

    #[test]
    fn typescript_preview_card_projects_bun_cross_language_grip() -> Result<(), String> {
        let mut finding = sample_finding(OracleKind::ExactValue, OracleStrength::Strong);
        finding.evidence.extend([
            "typescript_bun_ub_bridge_hint: confidence=configured_hint rust_file=src/jsc/Blob.rs rust_owner=Blob::from_js_without_defer_gc rust_boundary=\"array_buffer.shared || array_buffer.resizable\" ts_test_file=test/js/web/fetch/blob.test.ts".to_string(),
            "typescript_bun_ub_bridge_verdict: ts_missing_resizable missing_discriminators=resizable_array_buffer action=add_resizable_array_buffer_blob_case suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_missing_discriminator rust_grip=ungripped ts_verdict=ts_missing_resizable action=add_resizable_array_buffer_blob_case authority=preview_advisory_only suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_bridge_boundary: preview_advisory_only no_source_edits no_generated_tests no_runtime_bun_execution no_mutation_execution no_default_gates no_badge_baseline_zero_or_support_tier_authority".to_string(),
        ]);

        let card = typescript_preview_card(&finding)
            .ok_or_else(|| "expected TypeScript preview card".to_string())?;
        let grip = card
            .bun_cross_language_grip
            .as_ref()
            .ok_or_else(|| "expected Bun cross-language grip".to_string())?;
        assert_eq!(grip.state, "rust_ungripped_ts_missing_discriminator");
        assert_eq!(grip.rust_file, "src/jsc/Blob.rs");
        assert_eq!(grip.rust_owner, "Blob::from_js_without_defer_gc");
        assert_eq!(
            grip.rust_boundary,
            "array_buffer.shared || array_buffer.resizable"
        );
        assert_eq!(grip.missing_discriminators, vec!["resizable_array_buffer"]);
        assert!(!grip.repair_packet_ready);

        let json = typescript_preview_card_json_value(&card);
        let projected = &json["bun_cross_language_grip"];
        assert_eq!(
            projected["state"],
            "rust_ungripped_ts_missing_discriminator"
        );
        assert_eq!(projected["rust_seam"]["file"], "src/jsc/Blob.rs");
        assert_eq!(
            projected["typescript_evidence"]["missing_discriminators"][0],
            "resizable_array_buffer"
        );
        assert_eq!(
            projected["suggested_test_file"],
            "test/js/web/fetch/blob.test.ts"
        );
        assert_eq!(projected["authority_boundary"], "preview_advisory_only");
        assert_eq!(projected["repair_packet_ready"], false);
        Ok(())
    }

    #[test]
    fn typescript_preview_card_keeps_mock_payload_advisory() -> Result<(), String> {
        let mut finding = sample_finding(OracleKind::MockExpectation, OracleStrength::Medium);
        finding.related_tests[0].oracle =
            Some("expect(sink.record).toHaveBeenCalledWith(\"ready\")".to_string());

        let card = typescript_preview_card(&finding)
            .ok_or_else(|| "expected TypeScript preview card".to_string())?;

        assert!(!card.repair_packet_ready);
        assert!(
            card.suggested_assertion_shape
                .contains("bounded mock interaction evidence advisory")
        );
        assert!(
            card.limits
                .iter()
                .any(|limit| limit.contains("gate authority"))
        );
        Ok(())
    }

    #[test]
    fn typescript_preview_card_names_static_limit() -> Result<(), String> {
        let mut finding = sample_finding(OracleKind::ExactValue, OracleStrength::Strong);
        finding.static_limit_kind = Some(StaticLimitKind::MockedModule);

        let card = typescript_preview_card(&finding)
            .ok_or_else(|| "expected TypeScript preview card".to_string())?;

        assert_eq!(card.static_limits, vec!["mocked_module"]);
        assert!(card.suggested_assertion_shape.contains("static limitation"));
        assert_eq!(card.verify_command, None);
        Ok(())
    }

    fn sample_finding(oracle_kind: OracleKind, oracle_strength: OracleStrength) -> Finding {
        Finding {
            id: "probe:src_pricing.ts:2:typescript_preview".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_pricing.ts:2:typescript_preview".to_string()),
                location: SourceLocation::new("src/pricing.ts", 2, 1),
                owner: Some(crate::domain::SymbolId(
                    "typescript:src/pricing.ts::discountedTotal".to_string(),
                )),
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
                "owner: discountedTotal".to_string(),
                "gap_state: advisory".to_string(),
                "actionability_category: incomplete_repair_packet".to_string(),
                "why_not_actionable: TypeScript preview lacks a complete repair packet contract"
                    .to_string(),
                "repair_route: project canonical TypeScript repair packet fields later"
                    .to_string(),
                "missing_actionability_fields: canonical_gap_id, verify_command".to_string(),
                "evidence_needed_to_promote: canonical gap identity and verify command"
                    .to_string(),
                "raw_evidence_ref: file=src/pricing.ts;line=2;kind=typescript_preview_probe;source_id=probe:src_pricing.ts:2:typescript_preview;owner=discountedTotal".to_string(),
            ],
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: Vec::new(),
            related_tests: vec![RelatedTest {
                name: "discount smoke".to_string(),
                file: PathBuf::from("tests/pricing.test.ts"),
                line: 7,
                oracle: Some("expect(discountedTotal(10)).toBeTruthy()".to_string()),
                oracle_kind,
                oracle_strength,
            }],
            recommended_next_step: None,
            language: Some(LanguageId::TypeScript),
            language_status: Some(LanguageStatus::Preview),
            owner_kind: Some(OwnerKind::Function),
            static_limit_kind: None,
        }
    }

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Low, "stage")
    }
}
