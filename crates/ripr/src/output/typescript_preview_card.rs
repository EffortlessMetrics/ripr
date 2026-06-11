use crate::domain::{
    Finding, LanguageId, LanguageStatus, OracleKind, ProbeFamily, RelatedTest, StaticLimitKind,
};
use crate::output::preview_actionability::{
    PreviewActionability, PreviewRawEvidenceRef, preview_actionability_for,
};
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
    pub(crate) limitation_category: String,
    pub(crate) repair_route: String,
    pub(crate) missing_graph_legs: Vec<String>,
    pub(crate) unlock_condition: Option<String>,
    pub(crate) raw_evidence_refs: Vec<PreviewRawEvidenceRef>,
    pub(crate) action: String,
    pub(crate) suggested_test_file: String,
    pub(crate) placement: Option<TypeScriptBunTestPlacement>,
    pub(crate) authority_boundary: String,
    pub(crate) repair_packet_ready: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptBunTestPlacement {
    pub(crate) rank: usize,
    pub(crate) suggested_test_file: String,
    pub(crate) reason: String,
    pub(crate) basis: Vec<String>,
    pub(crate) authority_boundary: String,
    pub(crate) repair_packet_ready: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptBunCrossLanguageAdvisoryPacket {
    pub(crate) packet_version: String,
    pub(crate) cross_language_state: String,
    pub(crate) rust_file: String,
    pub(crate) rust_owner: String,
    pub(crate) rust_boundary: String,
    pub(crate) ts_test_file: Option<String>,
    pub(crate) missing_discriminators: Vec<String>,
    pub(crate) suggested_shape: String,
    pub(crate) bridge_confidence: String,
    pub(crate) missing_graph_legs: Vec<String>,
    pub(crate) proof_mode: TypeScriptBunStableByteProofMode,
    pub(crate) next_action: String,
    pub(crate) authority_boundary: String,
    pub(crate) repair_packet_ready: bool,
    pub(crate) public_repair_packet: bool,
    pub(crate) must_not_change: Vec<String>,
    pub(crate) stop_condition: String,
    pub(crate) raw_evidence_refs: Vec<PreviewRawEvidenceRef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptBunStableByteProofMode {
    pub(crate) mode: String,
    pub(crate) reason: String,
    pub(crate) authority_boundary: String,
    pub(crate) runtime_execution: bool,
    pub(crate) mutation_execution: bool,
    pub(crate) miri_execution: bool,
    pub(crate) proof_claim: bool,
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
    let bun_cross_language_grip = bun_cross_language_grip(finding, &actionability);

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
        "bun_cross_language_grip": card.bun_cross_language_grip.as_ref().map(|grip| {
            let advisory_packet = bun_cross_language_advisory_packet(grip);
            let proof_mode = stable_byte_proof_mode(grip);
            json!({
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
                "limitation_category": grip.limitation_category.as_str(),
                "repair_route": grip.repair_route.as_str(),
                "missing_graph_legs": &grip.missing_graph_legs,
                "unlock_condition": grip.unlock_condition.as_deref(),
                "raw_evidence_refs": grip.raw_evidence_refs.iter().map(raw_ref_json).collect::<Vec<_>>(),
                "action": grip.action.as_str(),
                "suggested_test_file": grip.suggested_test_file.as_str(),
                "placement": grip.placement.as_ref().map(|placement| json!({
                    "rank": placement.rank,
                    "suggested_test_file": placement.suggested_test_file.as_str(),
                    "reason": placement.reason.as_str(),
                    "basis": &placement.basis,
                    "authority_boundary": placement.authority_boundary.as_str(),
                    "repair_packet_ready": placement.repair_packet_ready,
                })),
                "proof_mode": proof_mode_json(&proof_mode),
                "advisory_packet": advisory_packet_json(&advisory_packet),
                "authority_boundary": grip.authority_boundary.as_str(),
                "repair_packet_ready": grip.repair_packet_ready,
            })
        }),
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

fn bun_cross_language_grip(
    finding: &Finding,
    actionability: &PreviewActionability,
) -> Option<TypeScriptBunCrossLanguageGrip> {
    let hint = evidence_value(finding, "typescript_bun_ub_bridge_hint: ")?;
    let verdict = evidence_value(finding, "typescript_bun_ub_bridge_verdict: ")?;
    let grip = evidence_value(finding, "typescript_bun_ub_cross_language_grip: ");
    let placement = evidence_value(finding, "typescript_bun_ub_test_placement: ");
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
        limitation_category: actionability.actionability_category.clone(),
        repair_route: actionability.repair_route.clone(),
        missing_graph_legs: actionability.missing_graph_legs.clone(),
        unlock_condition: actionability.unlock_condition.clone(),
        raw_evidence_refs: actionability.raw_evidence_refs.clone(),
        action: keyed_value(verdict, "action")?,
        suggested_test_file: keyed_value(verdict, "suggested_test_file")?,
        placement: placement.and_then(bun_test_placement),
        authority_boundary: grip
            .and_then(|line| keyed_value(line, "authority"))
            .unwrap_or_else(|| "preview_advisory_only".to_string()),
        repair_packet_ready: grip
            .and_then(|line| keyed_value(line, "repair_packet_ready"))
            .is_some_and(|value| value == "true"),
    })
}

pub(crate) fn bun_cross_language_advisory_packet(
    grip: &TypeScriptBunCrossLanguageGrip,
) -> TypeScriptBunCrossLanguageAdvisoryPacket {
    TypeScriptBunCrossLanguageAdvisoryPacket {
        packet_version: "bun_cross_language_advisory_packet.v1".to_string(),
        cross_language_state: grip.state.clone(),
        rust_file: grip.rust_file.clone(),
        rust_owner: grip.rust_owner.clone(),
        rust_boundary: grip.rust_boundary.clone(),
        ts_test_file: advisory_packet_ts_test_file(grip),
        missing_discriminators: grip.missing_discriminators.clone(),
        suggested_shape: advisory_packet_suggested_shape(grip),
        bridge_confidence: grip.bridge_confidence.clone(),
        missing_graph_legs: grip.missing_graph_legs.clone(),
        proof_mode: stable_byte_proof_mode(grip),
        next_action: advisory_packet_next_action(grip).to_string(),
        authority_boundary: grip.authority_boundary.clone(),
        repair_packet_ready: false,
        public_repair_packet: false,
        must_not_change: advisory_packet_must_not_change(),
        stop_condition: advisory_packet_stop_condition(grip),
        raw_evidence_refs: grip.raw_evidence_refs.clone(),
    }
}

fn advisory_packet_json(packet: &TypeScriptBunCrossLanguageAdvisoryPacket) -> Value {
    json!({
        "packet_version": packet.packet_version.as_str(),
        "cross_language_state": packet.cross_language_state.as_str(),
        "rust_file": packet.rust_file.as_str(),
        "rust_owner": packet.rust_owner.as_str(),
        "rust_boundary": packet.rust_boundary.as_str(),
        "ts_test_file": packet.ts_test_file.as_deref(),
        "missing_discriminators": &packet.missing_discriminators,
        "suggested_shape": packet.suggested_shape.as_str(),
        "bridge_confidence": packet.bridge_confidence.as_str(),
        "missing_graph_legs": &packet.missing_graph_legs,
        "proof_mode": proof_mode_json(&packet.proof_mode),
        "next_action": packet.next_action.as_str(),
        "authority_boundary": packet.authority_boundary.as_str(),
        "repair_packet_ready": packet.repair_packet_ready,
        "public_repair_packet": packet.public_repair_packet,
        "must_not_change": &packet.must_not_change,
        "stop_condition": packet.stop_condition.as_str(),
        "raw_evidence_refs": packet.raw_evidence_refs.iter().map(raw_ref_json).collect::<Vec<_>>(),
    })
}

pub(crate) fn stable_byte_proof_mode(
    grip: &TypeScriptBunCrossLanguageGrip,
) -> TypeScriptBunStableByteProofMode {
    let (mode, reason) = if is_bridge_unknown(grip) {
        (
            "bridge_unknown",
            "The binding or FFI edge is missing, so TypeScript evidence cannot be credited to the Rust seam.",
        )
    } else if is_helper_gated(grip) {
        (
            "helper_gated",
            "The route is visible, but proof is blocked on a helper or upstream primitive before a witness can be selected.",
        )
    } else if is_mutation_plus_miri(grip) {
        (
            "mutation_plus_miri",
            "The calibrated route is non-observable through a direct TypeScript assertion and needs mutation plus Miri or model proof.",
        )
    } else if grip.state == "rust_ungripped_ts_discriminated" {
        (
            "observable_red_green",
            "Configured TypeScript stable-byte evidence already observes the bridged Rust seam; future proof should be a system-Bun red/patched-green witness if behavior changes.",
        )
    } else if grip.state == "rust_ungripped_ts_missing_discriminator"
        && !grip.missing_discriminators.is_empty()
        && grip.placement.is_some()
    {
        (
            "observable_red_green",
            "The missing TypeScript discriminator belongs in an existing bridged stable-byte observer route; future proof should be a system-Bun red/patched-green witness after the discriminator is added.",
        )
    } else if grip.state == "ts_mention_not_observer" {
        (
            "static_limitation",
            "Token evidence is not an observer, so proof mode remains a static limitation until callsite and oracle legs are credited.",
        )
    } else {
        (
            "static_limitation",
            "The cross-language route is visible but lacks a credited observer, discriminator, placement, or safe action leg.",
        )
    };

    TypeScriptBunStableByteProofMode {
        mode: mode.to_string(),
        reason: reason.to_string(),
        authority_boundary: "preview_advisory_only".to_string(),
        runtime_execution: false,
        mutation_execution: false,
        miri_execution: false,
        proof_claim: false,
    }
}

fn proof_mode_json(proof_mode: &TypeScriptBunStableByteProofMode) -> Value {
    json!({
        "mode": proof_mode.mode.as_str(),
        "reason": proof_mode.reason.as_str(),
        "authority_boundary": proof_mode.authority_boundary.as_str(),
        "runtime_execution": proof_mode.runtime_execution,
        "mutation_execution": proof_mode.mutation_execution,
        "miri_execution": proof_mode.miri_execution,
        "proof_claim": proof_mode.proof_claim,
    })
}

fn advisory_packet_ts_test_file(grip: &TypeScriptBunCrossLanguageGrip) -> Option<String> {
    if is_bridge_unknown(grip) {
        return None;
    }
    if let Some(placement) = &grip.placement {
        return Some(placement.suggested_test_file.clone());
    }
    if grip.state == "rust_ungripped_ts_discriminated" {
        return Some(grip.ts_test_file.clone());
    }
    None
}

fn advisory_packet_suggested_shape(grip: &TypeScriptBunCrossLanguageGrip) -> String {
    if is_bridge_unknown(grip) {
        return "Inspect or add binding/FFI edge evidence before editing TypeScript tests."
            .to_string();
    }
    if grip.state == "rust_ungripped_ts_discriminated" {
        return "No missing bridge discriminator; continue manual review without adding duplicate tests."
            .to_string();
    }
    if grip.state == "ts_mention_not_observer" {
        return "Replace token mention with Blob/view input and stable-byte observer evidence only after bridge and observer evidence are credited."
            .to_string();
    }
    if grip.state == "rust_ungripped_ts_missing_external_oracle"
        || grip
            .missing_graph_legs
            .iter()
            .any(|leg| leg == "external_oracle:stable_byte_copy")
    {
        return "Connect the external callsite and stable-byte byte/text/value oracle before suggesting test placement."
            .to_string();
    }
    if grip.missing_discriminators.is_empty() {
        return "Inspect the named static limitation before editing tests.".to_string();
    }

    let missing_shared = grip
        .missing_discriminators
        .iter()
        .any(|missing| missing == "shared_array_buffer");
    let missing_resizable = grip
        .missing_discriminators
        .iter()
        .any(|missing| missing == "resizable_array_buffer");
    match (missing_shared, missing_resizable) {
        (true, true) => {
            "Add SharedArrayBuffer and resizable ArrayBuffer Blob/view cases with stable-byte byte/text/value assertions."
                .to_string()
        }
        (true, false) => {
            "Add a new SharedArrayBuffer(...) Blob/view case with a stable-byte byte/text/value assertion."
                .to_string()
        }
        (false, true) => {
            "Add new ArrayBuffer(..., { maxByteLength: ... }) through Blob/view with a stable-byte byte/text/value assertion."
                .to_string()
        }
        (false, false) => format!(
            "Add TypeScript discriminator coverage for {} with a stable-byte byte/text/value assertion.",
            grip.missing_discriminators.join(", ")
        ),
    }
}

fn advisory_packet_next_action(grip: &TypeScriptBunCrossLanguageGrip) -> &'static str {
    if is_bridge_unknown(grip) {
        return "inspect_or_add_bridge_evidence";
    }
    if grip.state == "rust_ungripped_ts_discriminated" {
        return "continue_manual_review_no_missing_bridge_discriminator";
    }
    if grip.state == "ts_mention_not_observer" {
        return "replace_mention_with_observer_before_credit";
    }
    if grip.state == "rust_ungripped_ts_missing_external_oracle" {
        return "connect_external_oracle_before_test_placement";
    }
    if !grip.missing_discriminators.is_empty() && grip.placement.is_some() {
        return "add_typescript_discriminator_in_suggested_file";
    }
    if !grip.missing_discriminators.is_empty() {
        return "inspect_typescript_placement_evidence";
    }
    "inspect_named_static_limitation"
}

fn advisory_packet_must_not_change() -> Vec<String> {
    vec![
        "Rust production behavior".to_string(),
        "public API".to_string(),
        "test framework shape".to_string(),
        "generated tests".to_string(),
        "runtime Bun/TypeScript execution".to_string(),
        "public repair-packet authority".to_string(),
    ]
}

fn advisory_packet_stop_condition(grip: &TypeScriptBunCrossLanguageGrip) -> String {
    if is_bridge_unknown(grip) {
        return "Stop before editing tests; missing binding_or_ffi_edge.".to_string();
    }
    if grip.state == "rust_ungripped_ts_discriminated" {
        return "Stop if the configured Rust seam or TypeScript witness route does not match the reviewed Bun change."
            .to_string();
    }
    if grip.state == "ts_mention_not_observer" {
        return "Stop before adding tests from token-only evidence; require observer and bridge evidence first."
            .to_string();
    }
    if grip.state == "rust_ungripped_ts_missing_external_oracle" {
        return "Stop before adding tests until the external stable-byte oracle graph leg is credited."
            .to_string();
    }
    if !grip.missing_discriminators.is_empty() && grip.placement.is_some() {
        return "Stop if placement evidence disappears or the stable-byte assertion requires production-code, public API, or test-framework changes."
            .to_string();
    }
    if !grip.missing_discriminators.is_empty() {
        return "Stop before adding tests until TypeScript placement evidence is credited."
            .to_string();
    }
    "Stop before emitting public repair packets from preview evidence.".to_string()
}

fn is_bridge_unknown(grip: &TypeScriptBunCrossLanguageGrip) -> bool {
    grip.state == "bridge_unknown"
        || grip.bridge_confidence == "unknown"
        || grip
            .missing_graph_legs
            .iter()
            .any(|leg| leg == "binding_or_ffi_edge")
}

fn is_helper_gated(grip: &TypeScriptBunCrossLanguageGrip) -> bool {
    grip.state == "helper_gated"
        || grip.state == "rust_ungripped_ts_helper_gated"
        || grip.action.contains("helper_gated")
        || grip
            .missing_graph_legs
            .iter()
            .any(|leg| leg == "helper_gated" || leg.starts_with("helper:"))
}

fn is_mutation_plus_miri(grip: &TypeScriptBunCrossLanguageGrip) -> bool {
    grip.state == "mutation_plus_miri"
        || grip.state == "rust_ungripped_ts_mutation_plus_miri"
        || grip.action.contains("mutation_plus_miri")
        || grip
            .missing_graph_legs
            .iter()
            .any(|leg| leg == "mutation_model" || leg == "miri_model" || leg.contains("miri"))
}

fn raw_ref_json(raw_ref: &PreviewRawEvidenceRef) -> Value {
    json!({
        "raw": raw_ref.raw.as_str(),
        "file": raw_ref.file.as_deref(),
        "line": raw_ref.line,
        "kind": raw_ref.kind.as_deref(),
        "source_id": raw_ref.source_id.as_deref(),
        "owner": raw_ref.owner.as_deref(),
        "leg": raw_ref.leg.as_deref(),
        "sample": raw_ref.sample.as_deref(),
    })
}

fn bun_test_placement(input: &str) -> Option<TypeScriptBunTestPlacement> {
    Some(TypeScriptBunTestPlacement {
        rank: keyed_value(input, "rank")?.parse().ok()?,
        suggested_test_file: keyed_value(input, "suggested_test_file")?,
        reason: keyed_value(input, "reason")?,
        basis: keyed_value(input, "basis")
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        authority_boundary: keyed_value(input, "authority")
            .unwrap_or_else(|| "preview_advisory_only".to_string()),
        repair_packet_ready: keyed_value(input, "repair_packet_ready")
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
        "ts_missing_external_oracle" => "rust_ungripped_ts_missing_external_oracle",
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
    use super::{
        TypeScriptBunCrossLanguageGrip, TypeScriptBunTestPlacement,
        bun_cross_language_advisory_packet, stable_byte_proof_mode, typescript_preview_card,
        typescript_preview_card_json_value,
    };
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
        finding.evidence = vec![
            "owner: Blob::from_js_without_defer_gc".to_string(),
            "gap_state: static_limitation".to_string(),
            "actionability_category: cross_language_oracle_visibility_unresolved".to_string(),
            "why_not_actionable: configured Bun Blob TypeScript preview evidence is missing external discriminator(s): resizable_array_buffer; placement can name the existing TypeScript Blob test file, but RIPR cannot emit a public repair packet without verification, receipt, and edit-surface evidence".to_string(),
            "repair_route: analysis/cross-language-oracle-visibility".to_string(),
            "missing_actionability_fields: verify_command, receipt_command, must_not_change, allowed_edit_surface".to_string(),
            "missing_graph_legs: boundary_discriminator:resizable_array_buffer".to_string(),
            "unlock_condition: add or inspect the missing external TypeScript discriminator(s) in test/js/web/fetch/blob.test.ts and keep repair-packet projection blocked until verify, receipt, and edit-surface evidence exists".to_string(),
            "evidence_needed_to_promote: the missing TypeScript discriminator in the configured Blob test file plus verify command, receipt command, and edit constraints before repair-packet projection".to_string(),
            "raw_evidence_ref: leg=rust_seam;file=src/jsc/Blob.rs;line=42;kind=rust_boundary;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=array_buffer.shared || array_buffer.resizable".to_string(),
            "raw_evidence_ref: leg=binding_edge;file=src/jsc/Blob.rs;line=42;kind=configured_bridge;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=configured Bun Blob bridge to test/js/web/fetch/blob.test.ts".to_string(),
            "raw_evidence_ref: leg=boundary_discriminator;file=test/js/web/fetch/blob.test.ts;line=12;kind=shared_array_buffer;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=const shared = new SharedArrayBuffer(4)".to_string(),
            "raw_evidence_ref: leg=external_callsite;file=test/js/web/fetch/blob.test.ts;line=13;kind=view_backed_blob_input;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=const blob = new Blob([new Uint8Array(shared)])".to_string(),
            "raw_evidence_ref: leg=external_oracle;file=test/js/web/fetch/blob.test.ts;line=15;kind=stable_byte_copy_oracle;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=expect([...copied]).toEqual([0, 0, 0, 0])".to_string(),
            "typescript_bun_ub_bridge_hint: confidence=configured_hint rust_file=src/jsc/Blob.rs rust_owner=Blob::from_js_without_defer_gc rust_boundary=\"array_buffer.shared || array_buffer.resizable\" ts_test_file=test/js/web/fetch/blob.test.ts".to_string(),
            "typescript_bun_ub_bridge_verdict: ts_missing_resizable missing_discriminators=resizable_array_buffer action=route_cross_language_oracle_visibility_limitation suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_missing_discriminator rust_grip=ungripped ts_verdict=ts_missing_resizable action=route_cross_language_oracle_visibility_limitation authority=preview_advisory_only suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_test_placement: rank=1 suggested_test_file=test/js/web/fetch/blob.test.ts reason=\"existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable ArrayBuffer\" basis=configured_bridge_suggested_test_file,same_js_surface,same_boundary_vocabulary authority=preview_advisory_only repair_packet_ready=false".to_string(),
            "typescript_bun_ub_bridge_boundary: preview_advisory_only no_source_edits no_generated_tests no_runtime_bun_execution no_mutation_execution no_default_gates no_badge_baseline_zero_or_support_tier_authority".to_string(),
        ];

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
        assert_eq!(
            grip.limitation_category,
            "cross_language_oracle_visibility_unresolved"
        );
        assert_eq!(
            grip.repair_route,
            "analysis/cross-language-oracle-visibility"
        );
        assert_eq!(
            grip.missing_graph_legs,
            vec!["boundary_discriminator:resizable_array_buffer"]
        );
        assert_eq!(
            grip.unlock_condition.as_deref(),
            Some(
                "add or inspect the missing external TypeScript discriminator(s) in test/js/web/fetch/blob.test.ts and keep repair-packet projection blocked until verify, receipt, and edit-surface evidence exists"
            )
        );
        assert_eq!(grip.raw_evidence_refs[0].leg.as_deref(), Some("rust_seam"));
        assert!(
            grip.raw_evidence_refs
                .iter()
                .any(|raw_ref| raw_ref.leg.as_deref() == Some("binding_edge"))
        );
        assert_eq!(grip.suggested_test_file, "test/js/web/fetch/blob.test.ts");
        let placement = grip
            .placement
            .as_ref()
            .ok_or_else(|| "expected advisory TypeScript placement".to_string())?;
        assert_eq!(placement.rank, 1);
        assert_eq!(
            placement.suggested_test_file,
            "test/js/web/fetch/blob.test.ts"
        );
        assert_eq!(
            placement.reason,
            "existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable ArrayBuffer"
        );
        assert_eq!(
            placement.basis,
            vec![
                "configured_bridge_suggested_test_file",
                "same_js_surface",
                "same_boundary_vocabulary"
            ]
        );
        assert!(!placement.repair_packet_ready);
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
        assert_eq!(
            projected["placement"]["suggested_test_file"],
            "test/js/web/fetch/blob.test.ts"
        );
        assert_eq!(
            projected["placement"]["reason"],
            "existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable ArrayBuffer"
        );
        assert_eq!(projected["placement"]["repair_packet_ready"], false);
        assert_eq!(
            projected["limitation_category"],
            "cross_language_oracle_visibility_unresolved"
        );
        assert_eq!(
            projected["missing_graph_legs"][0],
            "boundary_discriminator:resizable_array_buffer"
        );
        assert_eq!(projected["raw_evidence_refs"][0]["leg"], "rust_seam");
        assert!(
            projected["raw_evidence_refs"]
                .as_array()
                .ok_or_else(|| "expected raw refs array".to_string())?
                .iter()
                .any(|raw_ref| raw_ref["leg"] == "binding_edge")
        );
        assert_eq!(projected["authority_boundary"], "preview_advisory_only");
        assert_eq!(projected["repair_packet_ready"], false);
        assert_eq!(projected["proof_mode"]["mode"], "observable_red_green");
        assert_eq!(
            projected["proof_mode"]["authority_boundary"],
            "preview_advisory_only"
        );
        assert_eq!(projected["proof_mode"]["runtime_execution"], false);
        assert_eq!(projected["proof_mode"]["mutation_execution"], false);
        assert_eq!(projected["proof_mode"]["miri_execution"], false);
        assert_eq!(projected["proof_mode"]["proof_claim"], false);
        Ok(())
    }

    #[test]
    fn bun_cross_language_agent_packet_projects_missing_discriminator() -> Result<(), String> {
        let card = typescript_preview_card(&sample_bun_missing_resizable_finding())
            .ok_or_else(|| "expected TypeScript preview card".to_string())?;
        let json = typescript_preview_card_json_value(&card);
        let packet = &json["bun_cross_language_grip"]["advisory_packet"];

        assert_eq!(
            packet["packet_version"],
            "bun_cross_language_advisory_packet.v1"
        );
        assert_eq!(
            packet["cross_language_state"],
            "rust_ungripped_ts_missing_discriminator"
        );
        assert_eq!(packet["rust_file"], "src/jsc/Blob.rs");
        assert_eq!(packet["rust_owner"], "Blob::from_js_without_defer_gc");
        assert_eq!(
            packet["rust_boundary"],
            "array_buffer.shared || array_buffer.resizable"
        );
        assert_eq!(packet["ts_test_file"], "test/js/web/fetch/blob.test.ts");
        assert_eq!(
            packet["missing_discriminators"][0],
            "resizable_array_buffer"
        );
        assert!(
            packet["suggested_shape"]
                .as_str()
                .ok_or_else(|| "expected suggested shape".to_string())?
                .contains("maxByteLength")
        );
        assert_eq!(packet["bridge_confidence"], "configured_hint");
        assert_eq!(
            packet["missing_graph_legs"][0],
            "boundary_discriminator:resizable_array_buffer"
        );
        assert_eq!(
            packet["next_action"],
            "add_typescript_discriminator_in_suggested_file"
        );
        assert_eq!(packet["proof_mode"]["mode"], "observable_red_green");
        assert_eq!(packet["proof_mode"]["proof_claim"], false);
        assert_eq!(packet["authority_boundary"], "preview_advisory_only");
        assert_eq!(packet["repair_packet_ready"], false);
        assert_eq!(packet["public_repair_packet"], false);
        assert!(
            packet["must_not_change"]
                .as_array()
                .ok_or_else(|| "expected must_not_change array".to_string())?
                .iter()
                .any(|value| value == "Rust production behavior")
        );
        assert!(
            packet["must_not_change"]
                .as_array()
                .ok_or_else(|| "expected must_not_change array".to_string())?
                .iter()
                .any(|value| value == "public repair-packet authority")
        );
        assert!(
            packet["stop_condition"]
                .as_str()
                .ok_or_else(|| "expected stop condition".to_string())?
                .contains("placement evidence")
        );
        assert_eq!(packet["raw_evidence_refs"][0]["leg"], "rust_seam");
        assert!(
            packet["raw_evidence_refs"]
                .as_array()
                .ok_or_else(|| "expected raw refs array".to_string())?
                .iter()
                .any(|raw_ref| raw_ref["leg"] == "binding_edge")
        );
        Ok(())
    }

    #[test]
    fn typescript_preview_card_projects_missing_external_oracle_grip() -> Result<(), String> {
        let mut finding = sample_finding(OracleKind::Unknown, OracleStrength::Unknown);
        finding.evidence = vec![
            "owner: Blob::from_js_without_defer_gc".to_string(),
            "gap_state: static_limitation".to_string(),
            "actionability_category: cross_language_oracle_visibility_unresolved".to_string(),
            "why_not_actionable: configured Bun Blob TypeScript preview facts include a partial external observer path, but the Blob callsite or stable-byte oracle edge is incomplete".to_string(),
            "repair_route: analysis/cross-language-oracle-visibility".to_string(),
            "missing_actionability_fields: external_oracle_path, verify_command, receipt_command, allowed_edit_surface, raw_evidence_refs".to_string(),
            "missing_graph_legs: external_oracle:stable_byte_copy".to_string(),
            "unlock_condition: Connect the partial Blob observer evidence to a stable byte oracle before crediting the Rust seam or suggesting placement.".to_string(),
            "evidence_needed_to_promote: Blob input, stable-byte observer, binding or FFI route, verify command, receipt command, raw evidence refs, and edit constraints".to_string(),
            "raw_evidence_ref: leg=rust_seam;file=src/jsc/Blob.rs;line=42;kind=rust_boundary;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=array_buffer.shared || array_buffer.resizable".to_string(),
            "raw_evidence_ref: leg=binding_edge;file=src/jsc/Blob.rs;line=42;kind=configured_bridge;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=configured Bun Blob bridge to test/js/web/fetch/blob.test.ts".to_string(),
            "raw_evidence_ref: leg=boundary_discriminator;file=test/js/web/fetch/blob.test.ts;line=12;kind=shared_array_buffer;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=const shared = new SharedArrayBuffer(4)".to_string(),
            "raw_evidence_ref: leg=external_callsite;file=test/js/web/fetch/blob.test.ts;line=14;kind=view_backed_blob_input;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=const blob = new Blob([new Uint8Array(shared)])".to_string(),
            "typescript_bun_ub_bridge_hint: confidence=configured_hint rust_file=src/jsc/Blob.rs rust_owner=Blob::from_js_without_defer_gc rust_boundary=\"array_buffer.shared || array_buffer.resizable\" ts_test_file=test/js/web/fetch/blob.test.ts".to_string(),
            "typescript_bun_ub_bridge_verdict: ts_missing_external_oracle missing_discriminators=none action=route_cross_language_oracle_visibility_limitation suggested_test_file=not_applicable repair_packet_ready=false".to_string(),
            "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_missing_external_oracle rust_grip=ungripped ts_verdict=ts_missing_external_oracle action=route_cross_language_oracle_visibility_limitation authority=preview_advisory_only suggested_test_file=not_applicable repair_packet_ready=false".to_string(),
        ];

        let card = typescript_preview_card(&finding)
            .ok_or_else(|| "expected TypeScript preview card".to_string())?;
        let grip = card
            .bun_cross_language_grip
            .as_ref()
            .ok_or_else(|| "expected Bun cross-language grip".to_string())?;

        assert_eq!(grip.state, "rust_ungripped_ts_missing_external_oracle");
        assert_eq!(grip.ts_verdict, "ts_missing_external_oracle");
        assert_eq!(
            grip.missing_graph_legs,
            vec!["external_oracle:stable_byte_copy"]
        );
        assert_eq!(grip.suggested_test_file, "not_applicable");
        assert!(grip.placement.is_none());
        assert!(!grip.repair_packet_ready);
        assert!(
            !grip
                .raw_evidence_refs
                .iter()
                .any(|raw_ref| raw_ref.leg.as_deref() == Some("external_oracle"))
        );

        let json = typescript_preview_card_json_value(&card);
        let projected = &json["bun_cross_language_grip"];
        assert_eq!(
            projected["state"],
            "rust_ungripped_ts_missing_external_oracle"
        );
        assert_eq!(
            projected["missing_graph_legs"][0],
            "external_oracle:stable_byte_copy"
        );
        assert_eq!(projected["repair_packet_ready"], false);
        Ok(())
    }

    #[test]
    fn typescript_preview_card_projects_bridge_unknown_without_binding_ref() -> Result<(), String> {
        let mut finding = sample_finding(OracleKind::ExactValue, OracleStrength::Strong);
        finding.evidence = vec![
            "owner: Blob::from_js_without_defer_gc".to_string(),
            "gap_state: static_limitation".to_string(),
            "actionability_category: cross_language_oracle_visibility_unresolved".to_string(),
            "why_not_actionable: TypeScript discriminators are present, but the Rust bridge is unknown and must not be reported as no_static_path".to_string(),
            "repair_route: analysis/cross-language-oracle-visibility".to_string(),
            "missing_actionability_fields: bridge_hint, raw_evidence_refs".to_string(),
            "missing_graph_legs: binding_or_ffi_edge".to_string(),
            "unlock_condition: name the binding or FFI edge from the Rust seam to the external test before crediting external discriminators".to_string(),
            "evidence_needed_to_promote: configured bridge hint or generated bridge fact plus raw evidence refs".to_string(),
            "raw_evidence_ref: leg=rust_seam;file=src/jsc/Blob.rs;line=42;kind=rust_boundary;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=array_buffer.shared || array_buffer.resizable".to_string(),
            "raw_evidence_ref: leg=boundary_discriminator;file=test/js/web/fetch/blob.test.ts;line=12;kind=shared_array_buffer;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=const shared = new SharedArrayBuffer(4)".to_string(),
            "raw_evidence_ref: leg=boundary_discriminator;file=test/js/web/fetch/blob.test.ts;line=13;kind=resizable_array_buffer;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=const growable = new ArrayBuffer(4, { maxByteLength: 8 })".to_string(),
            "raw_evidence_ref: leg=external_callsite;file=test/js/web/fetch/blob.test.ts;line=14;kind=view_backed_blob_input;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=const blob = new Blob([new Uint8Array(shared), new Uint8Array(growable)])".to_string(),
            "raw_evidence_ref: leg=external_oracle;file=test/js/web/fetch/blob.test.ts;line=16;kind=stable_byte_copy_oracle;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=expect([...copied]).toEqual([0, 0, 0, 0])".to_string(),
            "typescript_bun_ub_bridge_hint: confidence=unknown rust_file=src/jsc/Blob.rs rust_owner=Blob::from_js_without_defer_gc rust_boundary=\"array_buffer.shared || array_buffer.resizable\" ts_test_file=test/js/web/fetch/blob.test.ts".to_string(),
            "typescript_bun_ub_bridge_verdict: bridge_unknown missing_discriminators=none action=report_bridge_unknown_not_no_static_path suggested_test_file=not_applicable repair_packet_ready=false".to_string(),
            "typescript_bun_ub_cross_language_grip: state=bridge_unknown rust_grip=ungripped ts_verdict=bridge_unknown action=report_bridge_unknown_not_no_static_path authority=preview_advisory_only suggested_test_file=not_applicable repair_packet_ready=false".to_string(),
        ];

        let card = typescript_preview_card(&finding)
            .ok_or_else(|| "expected TypeScript preview card".to_string())?;
        let grip = card
            .bun_cross_language_grip
            .as_ref()
            .ok_or_else(|| "expected Bun cross-language grip".to_string())?;

        assert_eq!(grip.state, "bridge_unknown");
        assert_eq!(grip.bridge_confidence, "unknown");
        assert_eq!(grip.missing_graph_legs, vec!["binding_or_ffi_edge"]);
        assert_eq!(grip.suggested_test_file, "not_applicable");
        assert!(grip.placement.is_none());
        assert!(
            !grip
                .raw_evidence_refs
                .iter()
                .any(|raw_ref| raw_ref.leg.as_deref() == Some("binding_edge"))
        );

        let json = typescript_preview_card_json_value(&card);
        let projected = &json["bun_cross_language_grip"];
        assert_eq!(projected["state"], "bridge_unknown");
        assert_eq!(projected["missing_graph_legs"][0], "binding_or_ffi_edge");
        assert!(
            projected["raw_evidence_refs"]
                .as_array()
                .ok_or_else(|| "expected raw refs array".to_string())?
                .iter()
                .all(|raw_ref| raw_ref["leg"] != "binding_edge")
        );
        assert_eq!(projected["repair_packet_ready"], false);
        Ok(())
    }

    #[test]
    fn bun_cross_language_agent_packet_projects_bridge_unknown_stop_condition() -> Result<(), String>
    {
        let card = typescript_preview_card(&sample_bun_bridge_unknown_finding())
            .ok_or_else(|| "expected TypeScript preview card".to_string())?;
        let json = typescript_preview_card_json_value(&card);
        let packet = &json["bun_cross_language_grip"]["advisory_packet"];

        assert_eq!(packet["cross_language_state"], "bridge_unknown");
        assert_eq!(packet["ts_test_file"], serde_json::Value::Null);
        assert_eq!(packet["bridge_confidence"], "unknown");
        assert_eq!(packet["missing_graph_legs"][0], "binding_or_ffi_edge");
        assert_eq!(packet["proof_mode"]["mode"], "bridge_unknown");
        assert!(
            packet["proof_mode"]["reason"]
                .as_str()
                .ok_or_else(|| "expected proof mode reason".to_string())?
                .contains("binding or FFI edge")
        );
        assert_eq!(packet["proof_mode"]["runtime_execution"], false);
        assert_eq!(packet["next_action"], "inspect_or_add_bridge_evidence");
        assert!(
            packet["suggested_shape"]
                .as_str()
                .ok_or_else(|| "expected suggested shape".to_string())?
                .contains("binding/FFI edge evidence")
        );
        assert!(
            packet["stop_condition"]
                .as_str()
                .ok_or_else(|| "expected stop condition".to_string())?
                .contains("missing binding_or_ffi_edge")
        );
        assert!(
            !packet["suggested_shape"]
                .as_str()
                .ok_or_else(|| "expected suggested shape".to_string())?
                .contains("Add new ArrayBuffer")
        );
        assert_eq!(packet["repair_packet_ready"], false);
        assert_eq!(packet["public_repair_packet"], false);
        assert!(
            packet["raw_evidence_refs"]
                .as_array()
                .ok_or_else(|| "expected raw refs array".to_string())?
                .iter()
                .all(|raw_ref| raw_ref["leg"] != "binding_edge")
        );
        Ok(())
    }

    #[test]
    fn bun_cross_language_agent_packet_covers_non_edit_states() {
        let discriminated = bun_cross_language_advisory_packet(&packet_grip(
            "rust_ungripped_ts_discriminated",
            Vec::new(),
            Vec::new(),
            None,
        ));
        assert_eq!(
            discriminated.ts_test_file.as_deref(),
            Some("test/js/web/fetch/blob.test.ts")
        );
        assert_eq!(
            discriminated.next_action,
            "continue_manual_review_no_missing_bridge_discriminator"
        );
        assert!(
            discriminated
                .suggested_shape
                .contains("No missing bridge discriminator")
        );
        assert!(discriminated.stop_condition.contains("reviewed Bun change"));

        let mention = bun_cross_language_advisory_packet(&packet_grip(
            "ts_mention_not_observer",
            Vec::new(),
            Vec::new(),
            None,
        ));
        assert_eq!(mention.ts_test_file, None);
        assert_eq!(
            mention.next_action,
            "replace_mention_with_observer_before_credit"
        );
        assert!(mention.suggested_shape.contains("token mention"));
        assert!(mention.stop_condition.contains("token-only evidence"));

        let missing_external = bun_cross_language_advisory_packet(&packet_grip(
            "rust_ungripped_ts_missing_external_oracle",
            Vec::new(),
            vec!["external_oracle:stable_byte_copy".to_string()],
            None,
        ));
        assert_eq!(
            missing_external.next_action,
            "connect_external_oracle_before_test_placement"
        );
        assert!(
            missing_external
                .suggested_shape
                .contains("external callsite")
        );
        assert!(
            missing_external
                .stop_condition
                .contains("external stable-byte oracle")
        );
    }

    #[test]
    fn bun_cross_language_agent_packet_covers_missing_discriminator_shapes() {
        let shared = bun_cross_language_advisory_packet(&packet_grip(
            "rust_ungripped_ts_missing_discriminator",
            vec!["shared_array_buffer".to_string()],
            vec!["boundary_discriminator:shared_array_buffer".to_string()],
            Some(packet_placement()),
        ));
        assert!(
            shared
                .suggested_shape
                .contains("new SharedArrayBuffer(...)")
        );
        assert_eq!(
            shared.next_action,
            "add_typescript_discriminator_in_suggested_file"
        );

        let both = bun_cross_language_advisory_packet(&packet_grip(
            "rust_ungripped_ts_missing_discriminator",
            vec![
                "shared_array_buffer".to_string(),
                "resizable_array_buffer".to_string(),
            ],
            vec![
                "boundary_discriminator:shared_array_buffer".to_string(),
                "boundary_discriminator:resizable_array_buffer".to_string(),
            ],
            Some(packet_placement()),
        ));
        assert!(both.suggested_shape.contains("SharedArrayBuffer"));
        assert!(both.suggested_shape.contains("resizable ArrayBuffer"));

        let generic_without_placement = bun_cross_language_advisory_packet(&packet_grip(
            "rust_ungripped_ts_missing_discriminator",
            vec!["custom_discriminator".to_string()],
            vec!["boundary_discriminator:custom_discriminator".to_string()],
            None,
        ));
        assert_eq!(generic_without_placement.ts_test_file, None);
        assert_eq!(
            generic_without_placement.next_action,
            "inspect_typescript_placement_evidence"
        );
        assert!(
            generic_without_placement
                .suggested_shape
                .contains("custom_discriminator")
        );
        assert!(
            generic_without_placement
                .stop_condition
                .contains("placement evidence is credited")
        );

        let named_limitation = bun_cross_language_advisory_packet(&packet_grip(
            "named_static_limitation",
            Vec::new(),
            Vec::new(),
            None,
        ));
        assert_eq!(
            named_limitation.next_action,
            "inspect_named_static_limitation"
        );
        assert!(
            named_limitation
                .suggested_shape
                .contains("named static limitation")
        );
        assert!(
            named_limitation
                .stop_condition
                .contains("public repair packets")
        );
    }

    #[test]
    fn stable_byte_proof_mode_is_advisory() {
        let discriminated = stable_byte_proof_mode(&packet_grip(
            "rust_ungripped_ts_discriminated",
            Vec::new(),
            Vec::new(),
            None,
        ));
        assert_eq!(discriminated.mode, "observable_red_green");
        assert!(discriminated.reason.contains("stable-byte evidence"));
        assert_eq!(discriminated.authority_boundary, "preview_advisory_only");
        assert!(!discriminated.runtime_execution);
        assert!(!discriminated.mutation_execution);
        assert!(!discriminated.miri_execution);
        assert!(!discriminated.proof_claim);

        let missing_with_placement = stable_byte_proof_mode(&packet_grip(
            "rust_ungripped_ts_missing_discriminator",
            vec!["resizable_array_buffer".to_string()],
            vec!["boundary_discriminator:resizable_array_buffer".to_string()],
            Some(packet_placement()),
        ));
        assert_eq!(missing_with_placement.mode, "observable_red_green");
        assert!(
            missing_with_placement
                .reason
                .contains("missing TypeScript discriminator")
        );

        let bridge_unknown = stable_byte_proof_mode(&packet_grip(
            "bridge_unknown",
            Vec::new(),
            vec!["binding_or_ffi_edge".to_string()],
            None,
        ));
        assert_eq!(bridge_unknown.mode, "bridge_unknown");
        assert!(bridge_unknown.reason.contains("binding or FFI edge"));

        let mention = stable_byte_proof_mode(&packet_grip(
            "ts_mention_not_observer",
            Vec::new(),
            vec![
                "external_callsite:view_backed_blob_input".to_string(),
                "external_oracle:stable_byte_copy".to_string(),
            ],
            None,
        ));
        assert_eq!(mention.mode, "static_limitation");
        assert!(mention.reason.contains("Token evidence"));

        let missing_external = stable_byte_proof_mode(&packet_grip(
            "rust_ungripped_ts_missing_external_oracle",
            Vec::new(),
            vec!["external_oracle:stable_byte_copy".to_string()],
            None,
        ));
        assert_eq!(missing_external.mode, "static_limitation");
        assert!(
            missing_external
                .reason
                .contains("lacks a credited observer")
        );

        let helper_gated = stable_byte_proof_mode(&packet_grip(
            "rust_ungripped_ts_helper_gated",
            Vec::new(),
            vec!["helper:bun_write_fixture_helper".to_string()],
            None,
        ));
        assert_eq!(helper_gated.mode, "helper_gated");
        assert!(helper_gated.reason.contains("blocked on a helper"));

        let mutation_plus_miri = stable_byte_proof_mode(&packet_grip(
            "rust_ungripped_ts_mutation_plus_miri",
            Vec::new(),
            vec!["miri_model".to_string()],
            None,
        ));
        assert_eq!(mutation_plus_miri.mode, "mutation_plus_miri");
        assert!(
            mutation_plus_miri
                .reason
                .contains("mutation plus Miri or model proof")
        );

        let named_limitation = stable_byte_proof_mode(&packet_grip(
            "named_static_limitation",
            Vec::new(),
            Vec::new(),
            None,
        ));
        assert_eq!(named_limitation.mode, "static_limitation");
        assert!(named_limitation.reason.contains("safe action leg"));
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

    fn sample_bun_missing_resizable_finding() -> Finding {
        let mut finding = sample_finding(OracleKind::ExactValue, OracleStrength::Strong);
        finding.evidence = vec![
            "owner: Blob::from_js_without_defer_gc".to_string(),
            "gap_state: static_limitation".to_string(),
            "actionability_category: cross_language_oracle_visibility_unresolved".to_string(),
            "why_not_actionable: configured Bun Blob TypeScript preview evidence is missing external discriminator(s): resizable_array_buffer; placement can name the existing TypeScript Blob test file, but RIPR cannot emit a public repair packet without verification, receipt, and edit-surface evidence".to_string(),
            "repair_route: analysis/cross-language-oracle-visibility".to_string(),
            "missing_actionability_fields: verify_command, receipt_command, must_not_change, allowed_edit_surface".to_string(),
            "missing_graph_legs: boundary_discriminator:resizable_array_buffer".to_string(),
            "unlock_condition: add or inspect the missing external TypeScript discriminator(s) in test/js/web/fetch/blob.test.ts and keep repair-packet projection blocked until verify, receipt, and edit-surface evidence exists".to_string(),
            "evidence_needed_to_promote: the missing TypeScript discriminator in the configured Blob test file plus verify command, receipt command, and edit constraints before repair-packet projection".to_string(),
            "raw_evidence_ref: leg=rust_seam;file=src/jsc/Blob.rs;line=42;kind=rust_boundary;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=array_buffer.shared || array_buffer.resizable".to_string(),
            "raw_evidence_ref: leg=binding_edge;file=src/jsc/Blob.rs;line=42;kind=configured_bridge;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=configured Bun Blob bridge to test/js/web/fetch/blob.test.ts".to_string(),
            "raw_evidence_ref: leg=boundary_discriminator;file=test/js/web/fetch/blob.test.ts;line=12;kind=shared_array_buffer;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=const shared = new SharedArrayBuffer(4)".to_string(),
            "raw_evidence_ref: leg=external_callsite;file=test/js/web/fetch/blob.test.ts;line=13;kind=view_backed_blob_input;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=const blob = new Blob([new Uint8Array(shared)])".to_string(),
            "raw_evidence_ref: leg=external_oracle;file=test/js/web/fetch/blob.test.ts;line=15;kind=stable_byte_copy_oracle;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=expect([...copied]).toEqual([0, 0, 0, 0])".to_string(),
            "typescript_bun_ub_bridge_hint: confidence=configured_hint rust_file=src/jsc/Blob.rs rust_owner=Blob::from_js_without_defer_gc rust_boundary=\"array_buffer.shared || array_buffer.resizable\" ts_test_file=test/js/web/fetch/blob.test.ts".to_string(),
            "typescript_bun_ub_bridge_verdict: ts_missing_resizable missing_discriminators=resizable_array_buffer action=route_cross_language_oracle_visibility_limitation suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_missing_discriminator rust_grip=ungripped ts_verdict=ts_missing_resizable action=route_cross_language_oracle_visibility_limitation authority=preview_advisory_only suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_test_placement: rank=1 suggested_test_file=test/js/web/fetch/blob.test.ts reason=\"existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable ArrayBuffer\" basis=configured_bridge_suggested_test_file,same_js_surface,same_boundary_vocabulary authority=preview_advisory_only repair_packet_ready=false".to_string(),
            "typescript_bun_ub_bridge_boundary: preview_advisory_only no_source_edits no_generated_tests no_runtime_bun_execution no_mutation_execution no_default_gates no_badge_baseline_zero_or_support_tier_authority".to_string(),
        ];
        finding
    }

    fn sample_bun_bridge_unknown_finding() -> Finding {
        let mut finding = sample_finding(OracleKind::ExactValue, OracleStrength::Strong);
        finding.evidence = vec![
            "owner: Blob::from_js_without_defer_gc".to_string(),
            "gap_state: static_limitation".to_string(),
            "actionability_category: cross_language_oracle_visibility_unresolved".to_string(),
            "why_not_actionable: TypeScript discriminators are present, but the Rust bridge is unknown and must not be reported as no_static_path".to_string(),
            "repair_route: analysis/cross-language-oracle-visibility".to_string(),
            "missing_actionability_fields: bridge_hint, raw_evidence_refs".to_string(),
            "missing_graph_legs: binding_or_ffi_edge".to_string(),
            "unlock_condition: name the binding or FFI edge from the Rust seam to the external test before crediting external discriminators".to_string(),
            "evidence_needed_to_promote: configured bridge hint or generated bridge fact plus raw evidence refs".to_string(),
            "raw_evidence_ref: leg=rust_seam;file=src/jsc/Blob.rs;line=42;kind=rust_boundary;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=array_buffer.shared || array_buffer.resizable".to_string(),
            "raw_evidence_ref: leg=boundary_discriminator;file=test/js/web/fetch/blob.test.ts;line=12;kind=shared_array_buffer;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=const shared = new SharedArrayBuffer(4)".to_string(),
            "raw_evidence_ref: leg=boundary_discriminator;file=test/js/web/fetch/blob.test.ts;line=13;kind=resizable_array_buffer;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=const growable = new ArrayBuffer(4, { maxByteLength: 8 })".to_string(),
            "raw_evidence_ref: leg=external_callsite;file=test/js/web/fetch/blob.test.ts;line=14;kind=view_backed_blob_input;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=const blob = new Blob([new Uint8Array(shared), new Uint8Array(growable)])".to_string(),
            "raw_evidence_ref: leg=external_oracle;file=test/js/web/fetch/blob.test.ts;line=16;kind=stable_byte_copy_oracle;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=expect([...copied]).toEqual([0, 0, 0, 0])".to_string(),
            "typescript_bun_ub_bridge_hint: confidence=unknown rust_file=src/jsc/Blob.rs rust_owner=Blob::from_js_without_defer_gc rust_boundary=\"array_buffer.shared || array_buffer.resizable\" ts_test_file=test/js/web/fetch/blob.test.ts".to_string(),
            "typescript_bun_ub_bridge_verdict: bridge_unknown missing_discriminators=none action=report_bridge_unknown_not_no_static_path suggested_test_file=not_applicable repair_packet_ready=false".to_string(),
            "typescript_bun_ub_cross_language_grip: state=bridge_unknown rust_grip=ungripped ts_verdict=bridge_unknown action=report_bridge_unknown_not_no_static_path authority=preview_advisory_only suggested_test_file=not_applicable repair_packet_ready=false".to_string(),
        ];
        finding
    }

    fn packet_grip(
        state: &str,
        missing_discriminators: Vec<String>,
        missing_graph_legs: Vec<String>,
        placement: Option<TypeScriptBunTestPlacement>,
    ) -> TypeScriptBunCrossLanguageGrip {
        TypeScriptBunCrossLanguageGrip {
            state: state.to_string(),
            rust_file: "src/jsc/Blob.rs".to_string(),
            rust_owner: "Blob::from_js_without_defer_gc".to_string(),
            rust_boundary: "array_buffer.shared || array_buffer.resizable".to_string(),
            ts_test_file: "test/js/web/fetch/blob.test.ts".to_string(),
            ts_verdict: state.to_string(),
            bridge_confidence: "configured_hint".to_string(),
            missing_discriminators,
            limitation_category: "cross_language_oracle_visibility_unresolved".to_string(),
            repair_route: "analysis/cross-language-oracle-visibility".to_string(),
            missing_graph_legs,
            unlock_condition: None,
            raw_evidence_refs: Vec::new(),
            action: "route_cross_language_oracle_visibility_limitation".to_string(),
            suggested_test_file: "not_applicable".to_string(),
            placement,
            authority_boundary: "preview_advisory_only".to_string(),
            repair_packet_ready: false,
        }
    }

    fn packet_placement() -> TypeScriptBunTestPlacement {
        TypeScriptBunTestPlacement {
            rank: 1,
            suggested_test_file: "test/js/web/fetch/blob.test.ts".to_string(),
            reason: "existing Blob + ArrayBuffer integration tests live there".to_string(),
            basis: vec!["configured_bridge_suggested_test_file".to_string()],
            authority_boundary: "preview_advisory_only".to_string(),
            repair_packet_ready: false,
        }
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
