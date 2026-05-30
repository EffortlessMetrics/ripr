use crate::domain::{LanguageId, LanguageStatus, StaticLimitKind};
use crate::output::first_useful_action::DEFAULT_FIRST_USEFUL_ACTION_OUT;
use crate::output::gap_decision_ledger::DEFAULT_GAP_DECISION_LEDGER_OUT;
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::{Component, Path};

const DEFAULT_ACTIONABLE_GAPS_OUT: &str = "target/ripr/reports/actionable-gaps.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GapArtifactValidationContext<'a> {
    pub(super) root: &'a Path,
    pub(super) enabled_languages: &'a [LanguageId],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ValidatedGapArtifact {
    pub(super) kind: GapArtifactKind,
    pub(super) root: Option<String>,
    pub(super) identities: Vec<GapArtifactIdentity>,
    pub(super) language: Option<LanguageId>,
    pub(super) language_status: Option<LanguageStatus>,
    pub(super) gap_state: Option<String>,
    pub(super) related_paths: Vec<String>,
    pub(super) verify_commands: Vec<String>,
    pub(super) receipt_commands: Vec<String>,
    pub(super) static_limit_kinds: Vec<String>,
    pub(super) has_text_static_limit: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct GapArtifactValidationReport {
    pub(super) artifacts: Vec<ValidatedGapArtifact>,
    pub(super) rejections: Vec<GapArtifactRejection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GapArtifactKind {
    ActionableGaps,
    EvidenceRecord,
    FirstUsefulAction,
    GapDecisionLedger,
    AgentReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GapArtifactIdentity {
    pub(super) canonical_gap_id: Option<String>,
    pub(super) seam_id: Option<String>,
    pub(super) finding_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum GapArtifactRejection {
    DisabledLanguage(String),
    MalformedArtifact(&'static str),
    MalformedCommandPayload(String),
    MissingIdentity,
    OutOfWorkspacePath(String),
    StaleArtifact,
    UnavailableLanguage(String),
    UnsupportedSchema(String),
    UnsupportedStaticLimitKind(String),
    UnsupportedKind(String),
    WrongRoot(String),
}

impl ValidatedGapArtifact {
    pub(super) fn is_safe_projection_input(&self) -> bool {
        let kind_is_supported = matches!(
            self.kind,
            GapArtifactKind::ActionableGaps
                | GapArtifactKind::EvidenceRecord
                | GapArtifactKind::FirstUsefulAction
                | GapArtifactKind::GapDecisionLedger
                | GapArtifactKind::AgentReceipt
        );
        let root_is_present_or_deferred = self
            .root
            .as_ref()
            .is_none_or(|root| !root.trim().is_empty());
        let identity_is_present = self.identities.iter().any(|identity| {
            identity.canonical_gap_id.is_some()
                || identity.seam_id.is_some()
                || identity.finding_id.is_some()
        }) || (self.kind == GapArtifactKind::ActionableGaps
            && self.is_no_action_gap());
        let language_is_supported = self.language.is_none_or(|language| language.is_available());
        let status_is_supported = self.language_status.is_none_or(|status| {
            matches!(status, LanguageStatus::Stable | LanguageStatus::Preview)
        });
        let gap_state_is_present_or_deferred = self
            .gap_state
            .as_ref()
            .is_none_or(|state| !state.trim().is_empty());
        let path_payloads_are_present = self
            .related_paths
            .iter()
            .all(|path| !path.trim().is_empty());
        let command_payloads_are_present = self
            .verify_commands
            .iter()
            .chain(self.receipt_commands.iter())
            .all(|command| !command.trim().is_empty());
        let static_limits_are_structured_or_text = self
            .static_limit_kinds
            .iter()
            .all(|kind| known_static_limit_kind(kind))
            || self.has_text_static_limit;

        kind_is_supported
            && root_is_present_or_deferred
            && identity_is_present
            && language_is_supported
            && status_is_supported
            && gap_state_is_present_or_deferred
            && path_payloads_are_present
            && command_payloads_are_present
            && static_limits_are_structured_or_text
    }

    pub(super) fn is_actionable_gap(&self) -> bool {
        self.gap_state.as_deref() == Some("actionable")
    }

    pub(super) fn is_no_action_gap(&self) -> bool {
        matches!(
            self.gap_state.as_deref(),
            Some(
                "already_improved"
                    | "baseline_only"
                    | "no_actionable_seam"
                    | "suppressed"
                    | "acknowledged"
                    | "waived"
            )
        )
    }

    pub(super) fn is_preview(&self) -> bool {
        self.language_status == Some(LanguageStatus::Preview)
    }

    pub(super) fn has_static_limit(&self) -> bool {
        !self.static_limit_kinds.is_empty() || self.has_text_static_limit
    }
}

impl GapArtifactRejection {
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            Self::DisabledLanguage(_) => "disabled_language",
            Self::MalformedArtifact(_) => "malformed_artifact",
            Self::MalformedCommandPayload(_) => "malformed_command_payload",
            Self::MissingIdentity => "missing_identity",
            Self::OutOfWorkspacePath(_) => "out_of_workspace_path",
            Self::StaleArtifact => "stale_artifact",
            Self::UnavailableLanguage(_) => "unavailable_language",
            Self::UnsupportedSchema(_) => "unsupported_schema",
            Self::UnsupportedStaticLimitKind(_) => "unsupported_static_limit_kind",
            Self::UnsupportedKind(_) => "unsupported_kind",
            Self::WrongRoot(_) => "wrong_root",
        }
    }
}

pub(super) fn validate_workspace_gap_artifact_report(
    root: &Path,
    enabled_languages: &[LanguageId],
) -> GapArtifactValidationReport {
    let context = GapArtifactValidationContext {
        root,
        enabled_languages,
    };
    let mut report = GapArtifactValidationReport::default();
    for relative in [
        DEFAULT_ACTIONABLE_GAPS_OUT,
        DEFAULT_FIRST_USEFUL_ACTION_OUT,
        DEFAULT_GAP_DECISION_LEDGER_OUT,
    ] {
        match validate_workspace_gap_artifact(root, relative, &context) {
            Some(Ok(artifact)) => report.artifacts.push(artifact),
            Some(Err(rejection)) => report.rejections.push(rejection),
            None => {}
        }
    }
    report
}

fn validate_workspace_gap_artifact(
    root: &Path,
    relative: &str,
    context: &GapArtifactValidationContext<'_>,
) -> Option<Result<ValidatedGapArtifact, GapArtifactRejection>> {
    let text = match std::fs::read_to_string(root.join(relative)) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => return None,
        Err(_) => {
            return Some(Err(GapArtifactRejection::MalformedArtifact(
                "gap artifact file must be readable",
            )));
        }
    };
    let value = match serde_json::from_str::<Value>(&text) {
        Ok(value) => value,
        Err(_) => {
            return Some(Err(GapArtifactRejection::MalformedArtifact(
                "gap artifact JSON must parse",
            )));
        }
    };
    Some(validate_gap_artifact(&value, context))
}

pub(super) fn validate_gap_artifact(
    artifact: &Value,
    context: &GapArtifactValidationContext<'_>,
) -> Result<ValidatedGapArtifact, GapArtifactRejection> {
    let object = artifact
        .as_object()
        .ok_or(GapArtifactRejection::MalformedArtifact(
            "gap artifact root must be a JSON object",
        ))?;
    validate_schema(object.get("schema_version").and_then(Value::as_str))?;
    let kind = artifact_kind(artifact)?;
    let report_root = object.get("root").and_then(Value::as_str);
    validate_report_root(context.root, report_root)?;
    validate_freshness(artifact)?;

    let mut validation = match kind {
        GapArtifactKind::ActionableGaps => {
            validate_actionable_gaps(artifact, report_root, context)?
        }
        GapArtifactKind::FirstUsefulAction => validate_first_useful_action(artifact, report_root)?,
        GapArtifactKind::GapDecisionLedger => validate_gap_decision_ledger(artifact, report_root)?,
        GapArtifactKind::EvidenceRecord => validate_evidence_record(artifact, report_root)?,
        GapArtifactKind::AgentReceipt => validate_agent_receipt(artifact, report_root)?,
    };

    validate_language(context, validation.language, validation.language_status)?;
    validate_paths(context.root, &validation.related_paths)?;
    validate_commands(context.root, &validation.verify_commands)?;
    validate_commands(context.root, &validation.receipt_commands)?;
    validate_static_limits(artifact)?;
    normalize_static_limits(artifact, &mut validation);
    if validation.identities.is_empty()
        && !(validation.kind == GapArtifactKind::ActionableGaps && validation.is_no_action_gap())
    {
        return Err(GapArtifactRejection::MissingIdentity);
    }
    Ok(validation)
}

fn validate_first_useful_action(
    artifact: &Value,
    report_root: Option<&str>,
) -> Result<ValidatedGapArtifact, GapArtifactRejection> {
    let selected = artifact.get("selected");
    let target = artifact.get("target");
    let commands = artifact.get("commands");
    let identity = identity_from_sources(&[selected, Some(artifact)])
        .ok_or(GapArtifactRejection::MissingIdentity)?;
    let related_paths = string_values(&[
        path_value(target, &["file"]),
        path_value(target, &["related_test"]),
        path_value(selected, &["path"]),
    ]);
    let verify_commands = string_values(&[path_value(commands, &["verify"])]);
    let receipt_commands = string_values(&[path_value(commands, &["receipt"])]);
    Ok(ValidatedGapArtifact {
        kind: GapArtifactKind::FirstUsefulAction,
        root: report_root.map(ToOwned::to_owned),
        identities: vec![identity],
        language: language_from_value(artifact)?,
        language_status: language_status_from_value(artifact)?,
        gap_state: artifact
            .get("status")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        related_paths,
        verify_commands,
        receipt_commands,
        static_limit_kinds: Vec::new(),
        has_text_static_limit: false,
    })
}

fn validate_gap_decision_ledger(
    artifact: &Value,
    report_root: Option<&str>,
) -> Result<ValidatedGapArtifact, GapArtifactRejection> {
    let records = artifact
        .get("records")
        .or_else(|| artifact.get("gap_records"))
        .and_then(Value::as_array)
        .ok_or(GapArtifactRejection::MalformedArtifact(
            "gap decision ledger must contain records",
        ))?;
    let mut identities = Vec::new();
    let mut related_paths = Vec::new();
    let mut verify_commands = Vec::new();
    let mut receipt_commands = Vec::new();
    let mut language = None;
    let mut language_status = None;
    let gap_state;
    let mut has_actionable_record = false;
    let mut no_action_records = 0usize;
    for record in records {
        identities.push(
            identity_from_sources(&[Some(record)]).ok_or(GapArtifactRejection::MissingIdentity)?,
        );
        let record_language = language_from_value(record)?;
        let record_language_status = language_status_from_value(record)?;
        if record_language_status == Some(LanguageStatus::Preview) {
            language = record_language.or(language);
            language_status = Some(LanguageStatus::Preview);
        } else {
            language = language.or(record_language);
            language_status = language_status.or(record_language_status);
        }
        if record_is_actionable_for_editor(record) {
            has_actionable_record = true;
        }
        if record_is_no_action(record) {
            no_action_records += 1;
        }
        let route = record.get("repair_route");
        let anchor = record.get("anchor");
        let receipt = record.get("receipt");
        related_paths.extend(string_values(&[
            path_value(route, &["target_file"]),
            path_value(route, &["related_test"]),
            path_value(anchor, &["file"]),
            path_value(receipt, &["path"]),
        ]));
        if let Some(commands) = record
            .get("verification_commands")
            .and_then(Value::as_array)
        {
            verify_commands.extend(
                commands
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned),
            );
        }
        if let Some(command) = record.get("receipt_command").and_then(Value::as_str) {
            receipt_commands.push(command.to_string());
        }
    }
    if has_actionable_record {
        gap_state = Some("actionable".to_string());
    } else if !records.is_empty() && no_action_records == records.len() {
        gap_state = Some("no_actionable_seam".to_string());
    } else {
        gap_state = records.iter().find_map(|record| {
            record
                .get("gap_state")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        });
    }
    Ok(ValidatedGapArtifact {
        kind: GapArtifactKind::GapDecisionLedger,
        root: report_root.map(ToOwned::to_owned),
        identities,
        language,
        language_status,
        gap_state,
        related_paths,
        verify_commands,
        receipt_commands,
        static_limit_kinds: Vec::new(),
        has_text_static_limit: false,
    })
}

fn validate_actionable_gaps(
    artifact: &Value,
    report_root: Option<&str>,
    context: &GapArtifactValidationContext<'_>,
) -> Result<ValidatedGapArtifact, GapArtifactRejection> {
    if report_has_run_limitations(artifact) {
        return Err(GapArtifactRejection::MalformedArtifact(
            "actionable-gaps report must not carry run_limitations for editor projection",
        ));
    }
    if !actionable_gaps_runtime_status_is_editor_consumable(artifact) {
        return Err(GapArtifactRejection::MalformedArtifact(
            "actionable-gaps report runtime_status must be full and downstream-consumable for editor projection",
        ));
    }
    let packets = artifact.get("packets").and_then(Value::as_array).ok_or(
        GapArtifactRejection::MalformedArtifact("actionable-gaps report must contain packets"),
    )?;
    let mut identities = Vec::new();
    let mut related_paths = Vec::new();
    let mut verify_commands = Vec::new();
    let mut receipt_commands = Vec::new();
    let mut language = None;
    let mut language_status = None;
    let mut has_actionable_packet = false;
    let mut no_action_packets = 0usize;

    for packet in packets {
        let packet_language = language_from_actionable_packet(packet)?;
        let packet_language_status = language_status_from_actionable_packet(packet)?;
        validate_actionable_packet_languages(context, packet)?;
        if packet_language_status == Some(LanguageStatus::Preview) {
            language = packet_language.or(language);
            language_status = Some(LanguageStatus::Preview);
        } else {
            language = language.or(packet_language);
            language_status = language_status.or(packet_language_status);
        }

        if packet_is_actionable(packet) {
            has_actionable_packet = true;
            require_actionable_packet_string(
                packet,
                &["canonical_gap_id"],
                "actionable packet must carry canonical_gap_id",
            )?;
            identities.push(
                identity_from_sources(&[Some(packet)])
                    .ok_or(GapArtifactRejection::MissingIdentity)?,
            );
            validate_actionable_packet_projection_fields(packet)?;
            require_actionable_packet_repair_route(packet)?;
            require_actionable_packet_string(
                packet,
                &["repair_kind"],
                "actionable packet must carry repair_kind",
            )?;
            require_actionable_packet_string(
                packet,
                &["target_test_type"],
                "actionable packet must carry target_test_type",
            )?;
            require_actionable_packet_string(
                packet,
                &["assertion_shape"],
                "actionable packet must carry assertion_shape",
            )?;
            require_actionable_packet_string(
                packet,
                &["target_test_shape"],
                "actionable packet must carry target_test_shape",
            )?;
            require_actionable_packet_string(
                packet,
                &["verify_command"],
                "actionable packet must carry verify_command",
            )?;
            require_actionable_packet_string(
                packet,
                &["confidence_basis"],
                "actionable packet must carry confidence_basis",
            )?;
            require_actionable_packet_string(
                packet,
                &["receipt_command_or_path"],
                "actionable packet must carry receipt_command_or_path",
            )?;
            require_actionable_packet_string(
                packet,
                &["receipt_command"],
                "actionable packet must carry receipt_command",
            )?;
            require_actionable_packet_typed_related_target(packet)?;
            require_actionable_packet_array(
                packet,
                &["must_not_change"],
                "actionable packet must carry must_not_change",
            )?;
            require_actionable_packet_array(
                packet,
                &["allowed_edit_surface"],
                "actionable packet must carry allowed_edit_surface",
            )?;
            require_actionable_packet_raw_evidence_refs(
                packet,
                &["raw_evidence_refs"],
                "actionable packet must carry raw_evidence_refs",
            )?;
        } else {
            if packet_is_no_action(packet) {
                no_action_packets += 1;
            }
            if let Some(identity) = identity_from_sources(&[Some(packet)]) {
                identities.push(identity);
            }
        }

        related_paths.extend(actionable_packet_related_paths(packet));
        if let Some(command) = path_value(Some(packet), &["verify_command"]).and_then(Value::as_str)
        {
            verify_commands.push(command.to_string());
        }
        if let Some(receipt) = path_value(Some(packet), &["receipt_command_or_path"])
            .and_then(Value::as_str)
            .and_then(non_empty)
        {
            if looks_like_command_payload(&receipt) {
                receipt_commands.push(receipt);
            } else {
                related_paths.push(receipt);
            }
        }
    }

    let gap_state = if has_actionable_packet {
        Some("actionable".to_string())
    } else if packets.is_empty() {
        if !actionable_gaps_empty_queue_is_complete(artifact) {
            return Err(GapArtifactRejection::MalformedArtifact(
                "empty actionable-gaps report must carry completed zero-count summary",
            ));
        }
        Some("no_actionable_seam".to_string())
    } else if no_action_packets == packets.len() {
        Some("no_actionable_seam".to_string())
    } else {
        packets.iter().find_map(|packet| {
            packet
                .get("gap_state")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
    };

    Ok(ValidatedGapArtifact {
        kind: GapArtifactKind::ActionableGaps,
        root: report_root.map(ToOwned::to_owned),
        identities,
        language,
        language_status,
        gap_state,
        related_paths,
        verify_commands,
        receipt_commands,
        static_limit_kinds: Vec::new(),
        has_text_static_limit: false,
    })
}

fn report_has_run_limitations(artifact: &Value) -> bool {
    artifact
        .get("run_limitations")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
}

fn actionable_gaps_runtime_status_is_editor_consumable(artifact: &Value) -> bool {
    if path_value(Some(artifact), &["run_status"])
        .and_then(Value::as_str)
        .and_then(non_empty)
        .is_some_and(|state| state != "full")
    {
        return false;
    }
    let runtime_status = artifact.get("runtime_status");
    if path_value(runtime_status, &["state"])
        .and_then(Value::as_str)
        .and_then(non_empty)
        .is_some_and(|state| state != "full")
    {
        return false;
    }
    !matches!(
        path_value(runtime_status, &["downstream_consumable"]).and_then(Value::as_bool),
        Some(false)
    )
}

fn actionable_gaps_empty_queue_is_complete(artifact: &Value) -> bool {
    let Some(summary) = artifact.get("summary") else {
        return false;
    };
    numeric_value(summary, &["actionable_gaps"]) == Some(0)
        && numeric_value(summary, &["packets_emitted"]) == Some(0)
}

fn numeric_value(value: &Value, path: &[&str]) -> Option<u64> {
    path_value(Some(value), path).and_then(Value::as_u64)
}

fn validate_actionable_packet_projection_fields(
    packet: &Value,
) -> Result<(), GapArtifactRejection> {
    require_actionable_packet_source(
        packet,
        &["repair_route_source"],
        "canonical_item.repair_route",
        "actionable packet must carry canonical repair_route_source",
    )?;
    require_actionable_packet_source(
        packet,
        &["verify_command_source"],
        "canonical_item.verify_command",
        "actionable packet must carry canonical verify_command_source",
    )?;
    if packet
        .get("public_projection_eligible")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err(GapArtifactRejection::MalformedArtifact(
            "actionable packet must be public_projection_eligible",
        ));
    }
    if packet
        .get("projection_exclusion_reasons")
        .and_then(Value::as_array)
        .is_none_or(|reasons| !reasons.is_empty())
    {
        return Err(GapArtifactRejection::MalformedArtifact(
            "actionable packet must not carry projection_exclusion_reasons",
        ));
    }
    Ok(())
}

fn require_actionable_packet_repair_route(packet: &Value) -> Result<(), GapArtifactRejection> {
    let message = "actionable packet must carry structured repair_route";
    if !matches!(packet.get("repair_route"), Some(Value::Object(route)) if !route.is_empty()) {
        return Err(GapArtifactRejection::MalformedArtifact(message));
    }
    require_actionable_packet_string(packet, &["repair_route", "repair_kind"], message)?;
    require_actionable_packet_string(packet, &["repair_route", "target_test_type"], message)?;
    if require_actionable_packet_string(packet, &["repair_route", "assertion_shape"], message)
        .is_err()
        && require_actionable_packet_string(
            packet,
            &["repair_route", "suggested_assertion"],
            message,
        )
        .is_err()
    {
        return Err(GapArtifactRejection::MalformedArtifact(message));
    }
    Ok(())
}

fn require_actionable_packet_typed_related_target(
    packet: &Value,
) -> Result<(), GapArtifactRejection> {
    let Some(value) = path_value(Some(packet), &["related_test_or_observer"]) else {
        return Err(GapArtifactRejection::MalformedArtifact(
            "actionable packet must carry typed related_test_or_observer",
        ));
    };
    if actionable_packet_related_target_file(value).is_some() {
        Ok(())
    } else {
        Err(GapArtifactRejection::MalformedArtifact(
            "actionable packet must carry typed related_test_or_observer",
        ))
    }
}

fn require_actionable_packet_source(
    packet: &Value,
    path: &[&str],
    expected: &str,
    message: &'static str,
) -> Result<(), GapArtifactRejection> {
    if path_value(Some(packet), path).and_then(Value::as_str) == Some(expected) {
        Ok(())
    } else {
        Err(GapArtifactRejection::MalformedArtifact(message))
    }
}

fn validate_actionable_packet_languages(
    context: &GapArtifactValidationContext<'_>,
    packet: &Value,
) -> Result<(), GapArtifactRejection> {
    validate_language(
        context,
        language_from_value(packet)?,
        language_status_from_value(packet)?,
    )?;
    if let Some(findings) = packet.get("raw_findings").and_then(Value::as_array) {
        for finding in findings {
            validate_language(
                context,
                language_from_value(finding)?,
                language_status_from_value(finding)?,
            )?;
        }
    }
    Ok(())
}

fn record_is_actionable_for_editor(record: &Value) -> bool {
    record.get("gap_state").and_then(Value::as_str) == Some("actionable")
        && record.get("repairability").and_then(Value::as_str) == Some("repairable")
        && path_value(
            Some(record),
            &["projection_eligibility", "lsp_diagnostic", "eligible"],
        )
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn record_is_no_action(record: &Value) -> bool {
    record.get("repairability").and_then(Value::as_str) == Some("no_action")
        || matches!(
            record.get("gap_state").and_then(Value::as_str),
            Some(
                "already_improved"
                    | "already_observed"
                    | "baseline_only"
                    | "internal"
                    | "internal_only"
                    | "no_actionable_seam"
                    | "not_policy_targeted"
                    | "report_only"
                    | "resolved"
                    | "suppressed"
                    | "acknowledged"
                    | "waived"
            )
        )
}

fn packet_is_actionable(packet: &Value) -> bool {
    packet.get("gap_state").and_then(Value::as_str) == Some("actionable")
}

fn packet_is_no_action(packet: &Value) -> bool {
    matches!(
        packet.get("gap_state").and_then(Value::as_str),
        Some(
            "already_improved"
                | "already_observed"
                | "baseline_only"
                | "internal"
                | "internal_only"
                | "no_actionable_seam"
                | "not_policy_targeted"
                | "report_only"
                | "resolved"
                | "static_limit_only"
                | "suppressed"
                | "acknowledged"
                | "waived"
        )
    ) || matches!(
        packet.get("actionability").and_then(Value::as_str),
        Some("no_action" | "report_only" | "static_limit_only")
    )
}

fn require_actionable_packet_string(
    packet: &Value,
    path: &[&str],
    message: &'static str,
) -> Result<(), GapArtifactRejection> {
    let value = path_value(Some(packet), path)
        .and_then(Value::as_str)
        .and_then(non_empty)
        .filter(|value| !actionable_packet_guidance_is_missing(value))
        .ok_or(GapArtifactRejection::MalformedArtifact(message))?;
    if value.contains('\n') || value.contains('\r') {
        return Err(GapArtifactRejection::MalformedArtifact(message));
    }
    Ok(())
}

fn require_actionable_packet_array(
    packet: &Value,
    path: &[&str],
    message: &'static str,
) -> Result<(), GapArtifactRejection> {
    path_value(Some(packet), path)
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .map(|_| ())
        .ok_or(GapArtifactRejection::MalformedArtifact(message))
}

fn require_actionable_packet_raw_evidence_refs(
    packet: &Value,
    path: &[&str],
    message: &'static str,
) -> Result<(), GapArtifactRejection> {
    let values = path_value(Some(packet), path)
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or(GapArtifactRejection::MalformedArtifact(message))?;
    if values
        .iter()
        .all(actionable_packet_raw_evidence_ref_is_structured)
    {
        Ok(())
    } else {
        Err(GapArtifactRejection::MalformedArtifact(message))
    }
}

fn actionable_packet_raw_evidence_ref_is_structured(value: &Value) -> bool {
    let has_anchor = path_value(Some(value), &["file"])
        .or_else(|| path_value(Some(value), &["path"]))
        .or_else(|| path_value(Some(value), &["source_file"]))
        .and_then(Value::as_str)
        .and_then(non_empty)
        .is_some();
    let has_identity = path_value(Some(value), &["kind"])
        .or_else(|| path_value(Some(value), &["source_id"]))
        .or_else(|| path_value(Some(value), &["evidence_record_ref"]))
        .or_else(|| path_value(Some(value), &["canonical_gap_id"]))
        .and_then(Value::as_str)
        .and_then(non_empty)
        .is_some();
    has_anchor && has_identity
}

fn actionable_packet_guidance_is_missing(value: &str) -> bool {
    matches!(
        value.trim(),
        "" | "unknown"
            | "none"
            | "no_action"
            | "repair_route_unknown"
            | "repair_kind_unknown"
            | "receipt_command_unknown"
            | "receipt_path_unknown"
            | "confidence_basis_unknown"
            | "verify_command_unknown"
            | "target_test_type_unknown"
            | "target_test_shape_unknown"
            | "assertion_shape_unknown"
            | "recommended_repair_unknown"
    )
}

fn actionable_packet_related_paths(packet: &Value) -> Vec<String> {
    let observer = packet.get("related_test_or_observer");
    let primary_anchor = packet.get("primary_anchor");
    string_values(&[
        path_value(Some(packet), &["source_file"]),
        path_value(Some(packet), &["target_test"]),
        path_value(Some(packet), &["target_file"]),
        path_value(primary_anchor, &["file"]),
        path_value(observer, &["file"]),
        path_value(observer, &["path"]),
        path_value(observer, &["related_test"]),
        path_value(observer, &["test"]),
        path_value(observer, &["target_file"]),
    ])
}

fn actionable_packet_related_target_file(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => actionable_packet_related_target_file_from_text(value),
        Value::Object(object) => object
            .get("file")
            .and_then(Value::as_str)
            .and_then(actionable_packet_workspace_relative_file_token),
        Value::Array(values) => values
            .iter()
            .find_map(actionable_packet_related_target_file),
        Value::Null | Value::Bool(_) | Value::Number(_) => None,
    }
}

fn actionable_packet_related_target_file_from_text(value: &str) -> Option<String> {
    if let Some((file, _)) = value.split_once("::") {
        return actionable_packet_workspace_relative_file_token(file);
    }
    actionable_packet_workspace_relative_file_token(value)
}

fn actionable_packet_workspace_relative_file_token(value: &str) -> Option<String> {
    let normalized = value.trim().replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.contains(':')
        || normalized.chars().any(char::is_whitespace)
        || !normalized.contains('.')
    {
        return None;
    }
    if normalized
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return None;
    }
    Some(normalized)
}

fn validate_evidence_record(
    artifact: &Value,
    report_root: Option<&str>,
) -> Result<ValidatedGapArtifact, GapArtifactRejection> {
    let identity = identity_from_sources(&[
        Some(artifact),
        artifact.get("canonical_item"),
        artifact.get("identity"),
    ])
    .ok_or(GapArtifactRejection::MissingIdentity)?;
    let location = artifact.get("location");
    let canonical_item = artifact.get("canonical_item");
    let recommendation = artifact.get("recommendation");
    let related_paths = string_values(&[
        path_value(location, &["file"]),
        path_value(canonical_item, &["related_test", "file"]),
        path_value(recommendation, &["recommended_test", "file"]),
        path_value(recommendation, &["nearest_test_to_imitate", "file"]),
    ]);
    let verify_commands = string_values(&[
        path_value(canonical_item, &["verify_command"]),
        path_value(recommendation, &["verify_command"]),
    ]);
    Ok(ValidatedGapArtifact {
        kind: GapArtifactKind::EvidenceRecord,
        root: report_root.map(ToOwned::to_owned),
        identities: vec![identity],
        language: language_from_value(artifact)?,
        language_status: language_status_from_value(artifact)?,
        gap_state: path_value(canonical_item, &["gap_state"])
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        related_paths,
        verify_commands,
        receipt_commands: Vec::new(),
        static_limit_kinds: Vec::new(),
        has_text_static_limit: false,
    })
}

fn validate_agent_receipt(
    artifact: &Value,
    report_root: Option<&str>,
) -> Result<ValidatedGapArtifact, GapArtifactRejection> {
    let identity = identity_from_sources(&[Some(artifact), artifact.get("selected")])
        .ok_or(GapArtifactRejection::MissingIdentity)?;
    let inputs = artifact.get("inputs");
    let related_paths = string_values(&[
        path_value(inputs, &["receipt"]),
        path_value(inputs, &["verify"]),
        path_value(artifact.get("target"), &["file"]),
    ]);
    Ok(ValidatedGapArtifact {
        kind: GapArtifactKind::AgentReceipt,
        root: report_root.map(ToOwned::to_owned),
        identities: vec![identity],
        language: language_from_value(artifact)?,
        language_status: language_status_from_value(artifact)?,
        gap_state: artifact
            .get("movement")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        related_paths,
        verify_commands: Vec::new(),
        receipt_commands: Vec::new(),
        static_limit_kinds: Vec::new(),
        has_text_static_limit: false,
    })
}

fn validate_schema(schema_version: Option<&str>) -> Result<(), GapArtifactRejection> {
    match schema_version {
        Some("0.1") => Ok(()),
        Some(version) => Err(GapArtifactRejection::UnsupportedSchema(version.to_string())),
        None => Err(GapArtifactRejection::UnsupportedSchema(
            "missing".to_string(),
        )),
    }
}

fn artifact_kind(artifact: &Value) -> Result<GapArtifactKind, GapArtifactRejection> {
    let kind = artifact.get("kind").and_then(Value::as_str);
    match kind {
        Some("first_useful_action") => Ok(GapArtifactKind::FirstUsefulAction),
        Some("gap_decision_ledger") => Ok(GapArtifactKind::GapDecisionLedger),
        Some("agent_receipt") => Ok(GapArtifactKind::AgentReceipt),
        Some(other) => Err(GapArtifactRejection::UnsupportedKind(other.to_string())),
        None if artifact.get("report").and_then(Value::as_str) == Some("actionable-gaps") => {
            Ok(GapArtifactKind::ActionableGaps)
        }
        None if artifact.get("evidence_path").is_some() && artifact.get("seam_id").is_some() => {
            Ok(GapArtifactKind::EvidenceRecord)
        }
        None => Err(GapArtifactRejection::UnsupportedKind("missing".to_string())),
    }
}

fn validate_report_root(
    root: &Path,
    report_root: Option<&str>,
) -> Result<(), GapArtifactRejection> {
    let Some(report_root) = report_root else {
        return Ok(());
    };
    if report_root == "." || normalize_path(root) == normalize_path(&root.join(report_root)) {
        return Ok(());
    }
    let candidate = Path::new(report_root);
    if candidate.is_absolute() && normalize_path(candidate) == normalize_path(root) {
        return Ok(());
    }
    Err(GapArtifactRejection::WrongRoot(report_root.to_string()))
}

fn validate_freshness(artifact: &Value) -> Result<(), GapArtifactRejection> {
    for path in [
        &["status"][..],
        &["freshness"][..],
        &["editor_context", "freshness"][..],
    ] {
        if path_value(Some(artifact), path).and_then(Value::as_str) == Some("stale") {
            return Err(GapArtifactRejection::StaleArtifact);
        }
    }
    Ok(())
}

fn validate_language(
    context: &GapArtifactValidationContext<'_>,
    language: Option<LanguageId>,
    language_status: Option<LanguageStatus>,
) -> Result<(), GapArtifactRejection> {
    let Some(language) = language else {
        return Ok(());
    };
    if !language.is_available() {
        return Err(GapArtifactRejection::UnavailableLanguage(
            language.as_str().to_string(),
        ));
    }
    if !language_enabled(context.enabled_languages, language) {
        return Err(GapArtifactRejection::DisabledLanguage(
            language.as_str().to_string(),
        ));
    }
    if language_status == Some(LanguageStatus::Preview) && language == LanguageId::Rust {
        return Err(GapArtifactRejection::MalformedArtifact(
            "rust gap artifacts must not be labeled preview",
        ));
    }
    Ok(())
}

fn language_enabled(enabled_languages: &[LanguageId], language: LanguageId) -> bool {
    enabled_languages.contains(&language)
        || (language == LanguageId::JavaScript
            && enabled_languages.contains(&LanguageId::TypeScript))
}

fn validate_paths(root: &Path, paths: &[String]) -> Result<(), GapArtifactRejection> {
    for path in paths {
        if !workspace_path_is_safe(root, path) {
            return Err(GapArtifactRejection::OutOfWorkspacePath(path.clone()));
        }
    }
    Ok(())
}

fn validate_commands(root: &Path, commands: &[String]) -> Result<(), GapArtifactRejection> {
    for command in commands {
        if !command_payload_is_safe(root, command) {
            return Err(GapArtifactRejection::MalformedCommandPayload(
                command.clone(),
            ));
        }
    }
    Ok(())
}

fn validate_static_limits(artifact: &Value) -> Result<(), GapArtifactRejection> {
    for kind in static_limit_kind_values(artifact) {
        if !known_static_limit_kind(&kind) {
            return Err(GapArtifactRejection::UnsupportedStaticLimitKind(kind));
        }
    }
    Ok(())
}

fn normalize_static_limits(artifact: &Value, validation: &mut ValidatedGapArtifact) {
    let mut seen = BTreeSet::new();
    for kind in static_limit_kind_values(artifact) {
        if seen.insert(kind.clone()) {
            validation.static_limit_kinds.push(kind);
        }
    }
    validation.has_text_static_limit = contains_text_static_limit(artifact);
}

fn identity_from_sources(sources: &[Option<&Value>]) -> Option<GapArtifactIdentity> {
    let canonical_gap_id = string_from_sources(sources, &["canonical_gap_id"])
        .or_else(|| string_from_sources(sources, &["identity", "canonical_gap_id"]))
        .or_else(|| string_from_sources(sources, &["evidence_record", "canonical_gap_id"]));
    let seam_id = string_from_sources(sources, &["seam_id"]);
    let finding_id = string_from_sources(sources, &["finding_id"]);
    if canonical_gap_id.is_none() && seam_id.is_none() && finding_id.is_none() {
        return None;
    }
    Some(GapArtifactIdentity {
        canonical_gap_id,
        seam_id,
        finding_id,
    })
}

fn language_from_value(value: &Value) -> Result<Option<LanguageId>, GapArtifactRejection> {
    let language = value.get("language").and_then(Value::as_str);
    match language {
        None => Ok(None),
        Some("rust") => Ok(Some(LanguageId::Rust)),
        Some("typescript") => Ok(Some(LanguageId::TypeScript)),
        Some("javascript") => Ok(Some(LanguageId::JavaScript)),
        Some("python") => Ok(Some(LanguageId::Python)),
        Some(other) => Err(GapArtifactRejection::MalformedArtifact(match other {
            "" => "language must not be empty",
            _ => "unsupported language",
        })),
    }
}

fn language_from_actionable_packet(
    packet: &Value,
) -> Result<Option<LanguageId>, GapArtifactRejection> {
    if let Some(language) = language_from_value(packet)? {
        return Ok(Some(language));
    }
    packet
        .get("raw_findings")
        .and_then(Value::as_array)
        .and_then(|findings| {
            findings
                .iter()
                .find_map(|finding| language_from_value(finding).transpose())
        })
        .transpose()
}

fn language_status_from_value(
    value: &Value,
) -> Result<Option<LanguageStatus>, GapArtifactRejection> {
    match value.get("language_status").and_then(Value::as_str) {
        None => Ok(None),
        Some("stable") => Ok(Some(LanguageStatus::Stable)),
        Some("preview") => Ok(Some(LanguageStatus::Preview)),
        Some(_) => Err(GapArtifactRejection::MalformedArtifact(
            "unsupported language_status",
        )),
    }
}

fn language_status_from_actionable_packet(
    packet: &Value,
) -> Result<Option<LanguageStatus>, GapArtifactRejection> {
    if let Some(status) = language_status_from_value(packet)? {
        return Ok(Some(status));
    }
    packet
        .get("raw_findings")
        .and_then(Value::as_array)
        .and_then(|findings| {
            findings
                .iter()
                .find_map(|finding| language_status_from_value(finding).transpose())
        })
        .transpose()
}

fn string_values(values: &[Option<&Value>]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| value.and_then(Value::as_str))
        .filter_map(non_empty)
        .collect()
}

fn string_from_sources(sources: &[Option<&Value>], path: &[&str]) -> Option<String> {
    sources
        .iter()
        .find_map(|source| path_value(*source, path).and_then(Value::as_str))
        .and_then(non_empty)
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn path_value<'a>(value: Option<&'a Value>, path: &[&str]) -> Option<&'a Value> {
    let mut current = value?;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

pub(super) fn workspace_path_is_safe(root: &Path, raw: &str) -> bool {
    let path_text = path_part(raw).trim();
    if path_text.is_empty() || path_text.contains('\n') || path_text.contains('\r') {
        return false;
    }
    let path = Path::new(path_text);
    if path.is_absolute() {
        return is_inside_workspace(root, path);
    }
    !path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    })
}

fn path_part(raw: &str) -> &str {
    raw.split_once("::").map_or(raw, |(path, _)| path)
}

pub(super) fn command_payload_is_safe(root: &Path, command: &str) -> bool {
    let trimmed = command.trim();
    if trimmed.is_empty() || trimmed.contains('\n') || trimmed.contains('\r') {
        return false;
    }
    if trimmed
        .chars()
        .any(|character| matches!(character, ';' | '&' | '|' | '<' | '>' | '`'))
    {
        return false;
    }
    if trimmed.contains("../") || trimmed.contains("..\\") {
        return false;
    }
    let tokens = command_tokens(trimmed);
    if !command_program_is_allowed(&tokens) {
        return false;
    }
    for index in 0..tokens.len() {
        if tokens[index] == "--root" {
            let Some(root_arg) = tokens.get(index + 1) else {
                return false;
            };
            if root_arg != "."
                && normalize_path(Path::new(root_arg)) != normalize_path(root)
                && !workspace_path_is_safe(root, root_arg)
            {
                return false;
            }
        }
    }
    true
}

fn command_program_is_allowed(tokens: &[String]) -> bool {
    match tokens.first().map(String::as_str) {
        Some("cargo" | "ripr" | "pytest") => true,
        Some("python") => {
            tokens.get(1).map(String::as_str) == Some("-m")
                && tokens.get(2).map(String::as_str) == Some("unittest")
        }
        _ => false,
    }
}

fn looks_like_command_payload(value: &str) -> bool {
    let trimmed = value.trim_start();
    trimmed.starts_with("cargo ")
        || trimmed.starts_with("ripr ")
        || trimmed.starts_with("pytest ")
        || trimmed.starts_with("python -m unittest ")
}

fn command_tokens(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for character in command.chars() {
        match (quote, character) {
            (Some(active), value) if value == active => quote = None,
            (None, '"' | '\'') => quote = Some(character),
            (None, value) if value.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn is_inside_workspace(root: &Path, path: &Path) -> bool {
    let root = normalize_path(root);
    let path = normalize_path(path);
    path == root || path.starts_with(&(root + "/"))
}

fn normalize_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

fn static_limit_kind_values(value: &Value) -> Vec<String> {
    let mut values = Vec::new();
    collect_static_limit_kind_values(value, &mut values);
    values
}

fn collect_static_limit_kind_values(value: &Value, values: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if let Some(kind) = object.get("static_limit_kind").and_then(Value::as_str) {
                values.push(kind.to_string());
            }
            for child in object.values() {
                collect_static_limit_kind_values(child, values);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_static_limit_kind_values(item, values);
            }
        }
        _ => {}
    }
}

fn contains_text_static_limit(value: &Value) -> bool {
    match value {
        Value::Object(object) => {
            object.iter().any(|(key, child)| {
                key.starts_with("static_limit")
                    && key != "static_limit_kind"
                    && child.as_str().is_some_and(|text| !text.trim().is_empty())
            }) || object.values().any(contains_text_static_limit)
        }
        Value::Array(items) => items.iter().any(contains_text_static_limit),
        _ => false,
    }
}

fn known_static_limit_kind(kind: &str) -> bool {
    [
        StaticLimitKind::DynamicDispatch,
        StaticLimitKind::Metaprogramming,
        StaticLimitKind::MissingImportGraph,
        StaticLimitKind::DecoratorIndirection,
        StaticLimitKind::MockedModule,
        StaticLimitKind::UnsupportedSyntax,
    ]
    .iter()
    .any(|known| known.as_str() == kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root() -> PathBuf {
        PathBuf::from("/workspace")
    }

    fn context(enabled_languages: &[LanguageId]) -> GapArtifactValidationContext<'_> {
        GapArtifactValidationContext {
            root: Path::new("/workspace"),
            enabled_languages,
        }
    }

    fn first_action() -> Value {
        json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "first_useful_action",
            "root": ".",
            "status": "actionable",
            "selected": {
                "seam_id": "seam-1",
                "path": "src/pricing.rs",
                "line": 88
            },
            "target": {
                "file": "tests/pricing.rs",
                "related_test": "tests/pricing.rs::below_threshold"
            },
            "commands": {
                "verify": "ripr agent verify --root . --json",
                "receipt": "ripr agent receipt --root . --json"
            }
        })
    }

    fn actionable_gaps_report() -> Value {
        let raw_finding = json!({
            "file": "src/pricing.rs",
            "line": 42,
            "kind": "weakly_exposed",
            "language": "rust",
            "language_status": "stable"
        });
        let repair_route = json!({
            "repair_kind": "add_boundary_assertion",
            "target_test_type": "boundary_discriminator",
            "assertion_shape": "assert_eq!(price(threshold, threshold), expected)"
        });
        let packet = json!({
            "canonical_gap_id": "gap:rust:pricing-boundary",
            "evidence_class": "predicate_boundary",
            "gap_state": "actionable",
            "actionability": "extend_related_test",
            "source_file": "src/pricing.rs",
            "primary_anchor": {
                "file": "src/pricing.rs",
                "line": 42
            },
            "repair_kind": "add_boundary_assertion",
            "target_test_type": "boundary_discriminator",
            "target_test": "tests/pricing.rs::below_threshold_has_no_discount",
            "assertion_shape": "assert_eq!(price(threshold, threshold), expected)",
            "repair_route": repair_route,
            "target_test_shape": "boundary_discriminator: assert_eq!(price(threshold, threshold), expected)",
            "recommended_repair": "Add the equality-boundary assertion.",
            "why": "Related tests reach the seam but miss equality at the threshold.",
            "related_test_or_observer": {
                "file": "tests/pricing.rs",
                "name": "below_threshold_has_no_discount",
                "line": 10
            },
            "verify_command": "ripr agent verify --root . --json",
            "repair_route_source": "canonical_item.repair_route",
            "verify_command_source": "canonical_item.verify_command",
            "receipt_command": "ripr agent receipt --root . --json",
            "receipt_command_or_path": "ripr agent receipt --root . --json",
            "receipt_source": "canonical_item.receipt_command",
            "public_projection_eligible": true,
            "projection_exclusion_reasons": [],
            "raw_evidence_refs": [raw_finding.clone()],
            "raw_findings": [raw_finding],
            "confidence_basis": "static_only",
            "must_not_change": [
                "Do not infer actionability from raw static class."
            ],
            "allowed_edit_surface": [
                "tests/pricing.rs"
            ]
        });
        json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "report": "actionable-gaps",
            "scope": "repo",
            "status": "advisory",
            "source_report": "target/ripr/reports/lane1-evidence-audit.json",
            "summary": {
                "actionable_gaps": 1,
                "packets_emitted": 1,
                "public_projection_eligible_packets": 1,
                "public_projection_excluded_packets": 0
            },
            "run_limitations": [],
            "packets": [packet],
            "must_not_infer": [
                "do not claim mutation execution or runtime proof from this packet"
            ]
        })
    }

    fn temp_root(label: &str) -> Result<PathBuf, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system time before epoch: {err}"))?;
        Ok(std::env::temp_dir().join(format!(
            "ripr-lsp-gap-artifacts-{label}-{}-{}",
            std::process::id(),
            now.as_nanos()
        )))
    }

    fn preview_gap_ledger() -> Value {
        json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "gap_decision_ledger",
            "root": ".",
            "status": "advisory",
            "records": [
                {
                    "gap_id": "gap:py:pricing",
                    "canonical_gap_id": "gap:py:pricing",
                    "kind": "MissingBoundaryAssertion",
                    "language": "python",
                    "language_status": "preview",
                    "gap_state": "actionable",
                    "repairability": "repairable",
                    "static_limit_kind": "missing_import_graph",
                    "repair_route": {
                        "route_kind": "AddBoundaryAssertion",
                        "target_file": "tests/test_pricing.py",
                        "related_test": "tests/test_pricing.py::test_discount_boundary"
                    },
                    "verification_commands": [
                        "ripr agent verify --root . --json"
                    ],
                    "receipt_command": "ripr agent receipt --root . --json",
                    "projection_eligibility": {
                        "lsp_diagnostic": {
                            "eligible": true,
                            "reason": "local_file_scope"
                        }
                    }
                }
            ]
        })
    }

    fn no_action_gap_record() -> Value {
        json!({
            "gap_id": "gap:rust:observed",
            "canonical_gap_id": "gap:rust:observed",
            "kind": "NoActionAlreadyObserved",
            "language": "rust",
            "language_status": "stable",
            "scope": "repo_scoped",
            "evidence_class": "already_observed",
            "gap_state": "already_observed",
            "policy_state": "resolved",
            "repairability": "no_action",
            "projection_eligibility": {
                "lsp_diagnostic": {
                    "eligible": false,
                    "reason": "no_user_repair_needed"
                }
            }
        })
    }

    #[test]
    fn gap_artifact_state_helpers_classify_status_preview_and_static_limits() -> Result<(), String>
    {
        let artifact = preview_gap_ledger();
        let mut validated =
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust, LanguageId::Python]))
                .map_err(|err| format!("{err:?}"))?;

        assert!(validated.is_actionable_gap());
        assert!(!validated.is_no_action_gap());
        assert!(validated.is_preview());
        assert!(validated.has_static_limit());

        for status in [
            "already_improved",
            "baseline_only",
            "no_actionable_seam",
            "suppressed",
            "acknowledged",
            "waived",
        ] {
            validated.gap_state = Some(status.to_string());
            assert!(validated.is_no_action_gap(), "{status}");
            assert!(!validated.is_actionable_gap(), "{status}");
        }

        validated.gap_state = Some("unchanged_after_attempt".to_string());
        assert!(!validated.is_no_action_gap());
        assert!(!validated.is_actionable_gap());

        validated.language_status = Some(LanguageStatus::Stable);
        assert!(!validated.is_preview());
        validated.static_limit_kinds.clear();
        validated.has_text_static_limit = false;
        assert!(!validated.has_static_limit());
        validated.has_text_static_limit = true;
        assert!(validated.has_static_limit());
        Ok(())
    }

    #[test]
    fn rejection_kind_strings_cover_fail_closed_reasons() {
        let cases = [
            (
                GapArtifactRejection::DisabledLanguage("python".to_string()),
                "disabled_language",
            ),
            (
                GapArtifactRejection::MalformedArtifact("bad artifact"),
                "malformed_artifact",
            ),
            (
                GapArtifactRejection::MalformedCommandPayload("bad command".to_string()),
                "malformed_command_payload",
            ),
            (GapArtifactRejection::MissingIdentity, "missing_identity"),
            (
                GapArtifactRejection::OutOfWorkspacePath("../outside.py".to_string()),
                "out_of_workspace_path",
            ),
            (GapArtifactRejection::StaleArtifact, "stale_artifact"),
            (
                GapArtifactRejection::UnavailableLanguage("python".to_string()),
                "unavailable_language",
            ),
            (
                GapArtifactRejection::UnsupportedSchema("9.9".to_string()),
                "unsupported_schema",
            ),
            (
                GapArtifactRejection::UnsupportedStaticLimitKind("runtime_magic".to_string()),
                "unsupported_static_limit_kind",
            ),
            (
                GapArtifactRejection::UnsupportedKind("unknown".to_string()),
                "unsupported_kind",
            ),
            (
                GapArtifactRejection::WrongRoot("/other/workspace".to_string()),
                "wrong_root",
            ),
        ];

        for (rejection, expected) in cases {
            assert_eq!(rejection.as_str(), expected);
        }
    }

    #[test]
    fn first_useful_action_validates_read_only_projection_inputs() -> Result<(), String> {
        let artifact = first_action();
        let validated = validate_gap_artifact(&artifact, &context(&[LanguageId::Rust]))
            .map_err(|err| format!("{err:?}"))?;

        assert_eq!(validated.kind, GapArtifactKind::FirstUsefulAction);
        assert_eq!(validated.root.as_deref(), Some("."));
        assert_eq!(validated.identities[0].seam_id.as_deref(), Some("seam-1"));
        assert_eq!(validated.gap_state.as_deref(), Some("actionable"));
        assert_eq!(validated.related_paths.len(), 3);
        assert_eq!(validated.verify_commands.len(), 1);
        assert_eq!(validated.receipt_commands.len(), 1);
        Ok(())
    }

    #[test]
    fn actionable_gaps_report_validates_read_only_queue_inputs() -> Result<(), String> {
        let artifact = actionable_gaps_report();
        let validated = validate_gap_artifact(&artifact, &context(&[LanguageId::Rust]))
            .map_err(|err| format!("{err:?}"))?;

        assert_eq!(validated.kind, GapArtifactKind::ActionableGaps);
        assert_eq!(
            validated.identities[0].canonical_gap_id.as_deref(),
            Some("gap:rust:pricing-boundary")
        );
        assert_eq!(validated.language, Some(LanguageId::Rust));
        assert_eq!(validated.language_status, Some(LanguageStatus::Stable));
        assert_eq!(validated.gap_state.as_deref(), Some("actionable"));
        assert!(validated.is_actionable_gap());
        assert!(validated.is_safe_projection_input());
        assert_eq!(validated.verify_commands.len(), 1);
        assert_eq!(validated.receipt_commands.len(), 1);
        assert!(
            validated
                .related_paths
                .iter()
                .any(|path| path == "tests/pricing.rs::below_threshold_has_no_discount")
        );
        Ok(())
    }

    #[test]
    fn actionable_gaps_report_allows_empty_no_action_queue() -> Result<(), String> {
        let artifact = json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "report": "actionable-gaps",
            "scope": "repo",
            "status": "advisory",
            "summary": {
                "actionable_gaps": 0,
                "packets_emitted": 0,
                "public_projection_eligible_packets": 0,
                "public_projection_excluded_packets": 0
            },
            "run_limitations": [],
            "packets": []
        });

        let validated = validate_gap_artifact(&artifact, &context(&[LanguageId::Rust]))
            .map_err(|err| format!("{err:?}"))?;

        assert_eq!(validated.kind, GapArtifactKind::ActionableGaps);
        assert!(validated.identities.is_empty());
        assert_eq!(validated.gap_state.as_deref(), Some("no_actionable_seam"));
        assert!(validated.is_no_action_gap());
        assert!(validated.is_safe_projection_input());
        Ok(())
    }

    #[test]
    fn actionable_gaps_report_rejects_empty_queue_without_completed_summary() {
        let artifact = json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "report": "actionable-gaps",
            "scope": "repo",
            "status": "advisory",
            "run_limitations": [],
            "packets": []
        });

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "empty actionable-gaps report must carry completed zero-count summary"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_limited_run_artifacts() {
        let mut artifact = actionable_gaps_report();
        artifact["run_limitations"] = json!([
            {
                "category": "lane1_repo_exposure_timeout",
                "phase": "repo_exposure_generation",
                "repair_route": "inspect repo-exposure latency trace"
            }
        ]);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable-gaps report must not carry run_limitations for editor projection"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_limited_runtime_status_without_run_limitations() {
        let mut artifact = actionable_gaps_report();
        artifact["run_status"] = json!("limited_stale_input");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable-gaps report runtime_status must be full and downstream-consumable for editor projection"
            ))
        );

        artifact["run_status"] = json!("full");
        artifact["runtime_status"] = json!({
            "state": "limited_large_cache_skip",
            "phase": "repo_seam_facts_cache",
            "downstream_consumable": false,
            "repair_route": "cargo xtask cache report && cargo xtask cache gc --dry-run"
        });

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable-gaps report runtime_status must be full and downstream-consumable for editor projection"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_actionable_packet_without_identity() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["canonical_gap_id"] = json!(null);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry canonical_gap_id"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_actionable_packet_without_canonical_gap_id_even_with_seam_id()
    {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["canonical_gap_id"] = json!(null);
        artifact["packets"][0]["seam_id"] = json!("fallback-seam-id");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry canonical_gap_id"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_projection_excluded_actionable_packets() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["repair_route_source"] = json!("missing");
        artifact["packets"][0]["public_projection_eligible"] = json!(false);
        artifact["packets"][0]["projection_exclusion_reasons"] = json!(["missing_repair_route"]);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry canonical repair_route_source"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_public_projection_exclusion_reasons() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["public_projection_eligible"] = json!(true);
        artifact["packets"][0]["projection_exclusion_reasons"] = json!(["missing_receipt_path"]);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must not carry projection_exclusion_reasons"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_missing_repair_guidance() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["repair_kind"] = json!("repair_route_unknown");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry repair_kind"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_missing_structured_repair_route() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["repair_route"] = json!(null);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry structured repair_route"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_missing_receipt_command_or_path() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["receipt_command_or_path"] = json!(null);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry receipt_command_or_path"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_missing_verify_command() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["verify_command"] = json!("verify_command_unknown");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry verify_command"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_missing_target_test_shape() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["target_test_shape"] = json!(null);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry target_test_shape"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_missing_receipt_command() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["receipt_command"] = json!(null);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry receipt_command"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_missing_related_context() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["related_test_or_observer"] = json!(null);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry typed related_test_or_observer"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_related_context_without_typed_target() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["related_test_or_observer"] = json!("input that reaches the branch");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry typed related_test_or_observer"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_missing_must_not_change() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["must_not_change"] = json!([]);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry must_not_change"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_missing_allowed_edit_surface() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["allowed_edit_surface"] = json!([]);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry allowed_edit_surface"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_missing_raw_evidence_refs() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["raw_evidence_refs"] = json!([]);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry raw_evidence_refs"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_malformed_raw_evidence_refs() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["raw_evidence_refs"] = json!([{}]);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry raw_evidence_refs"
            ))
        );

        artifact["packets"][0]["raw_evidence_refs"] = json!(["placeholder"]);
        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry raw_evidence_refs"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_compound_verify_command() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["verify_command"] =
            json!("ripr agent verify --root . --json; cargo test");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedCommandPayload(
                "ripr agent verify --root . --json; cargo test".to_string()
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_missing_confidence_basis() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["confidence_basis"] = json!(null);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry confidence_basis"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_placeholder_guidance_fields() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["confidence_basis"] = json!("unknown");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry confidence_basis"
            ))
        );

        artifact["packets"][0]["confidence_basis"] = json!("confidence_basis_unknown");
        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry confidence_basis"
            ))
        );

        artifact["packets"][0]["confidence_basis"] = json!("static_only");
        artifact["packets"][0]["receipt_command"] = json!("receipt_command_unknown");
        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedArtifact(
                "actionable packet must carry receipt_command"
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_disabled_language_in_later_packet() -> Result<(), String> {
        let mut artifact = actionable_gaps_report();
        let python_packet = json!({
            "canonical_gap_id": "gap:python:pricing-boundary",
            "evidence_class": "predicate_boundary",
            "gap_state": "actionable",
            "actionability": "extend_related_test",
            "language": "python",
            "language_status": "preview",
            "source_file": "src/pricing.py",
            "repair_kind": "add_boundary_assertion",
            "target_test_type": "boundary_discriminator",
            "target_test": "tests/test_pricing.py::test_boundary",
            "assertion_shape": "assert price(threshold, threshold) == expected",
            "repair_route": {
                "repair_kind": "add_boundary_assertion",
                "target_test_type": "boundary_discriminator",
                "assertion_shape": "assert price(threshold, threshold) == expected"
            },
            "verify_command": "ripr agent verify --root . --json",
            "repair_route_source": "canonical_item.repair_route",
            "verify_command_source": "canonical_item.verify_command",
            "receipt_command_or_path": "ripr agent receipt --root . --json",
            "receipt_source": "canonical_item.receipt_command",
            "public_projection_eligible": true,
            "projection_exclusion_reasons": [],
            "confidence_basis": "static_only"
        });
        artifact["packets"]
            .as_array_mut()
            .ok_or_else(|| "packets fixture must be an array".to_string())?
            .push(python_packet);

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::DisabledLanguage("python".to_string()))
        );
        Ok(())
    }

    #[test]
    fn actionable_gaps_report_rejects_unsafe_related_path() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["source_file"] = json!("../outside.rs");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::OutOfWorkspacePath(
                "../outside.rs".to_string()
            ))
        );
    }

    #[test]
    fn actionable_gaps_report_rejects_unsafe_verify_command() {
        let mut artifact = actionable_gaps_report();
        artifact["packets"][0]["verify_command"] =
            json!("ripr agent verify --root ../outside --json");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedCommandPayload(
                "ripr agent verify --root ../outside --json".to_string()
            ))
        );
    }

    #[test]
    fn preview_gap_ledger_validates_only_when_language_is_enabled() -> Result<(), String> {
        let artifact = preview_gap_ledger();
        let disabled = validate_gap_artifact(&artifact, &context(&[LanguageId::Rust]));
        assert_eq!(
            disabled,
            Err(GapArtifactRejection::DisabledLanguage("python".to_string()))
        );

        let enabled =
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust, LanguageId::Python]))
                .map_err(|err| format!("{err:?}"))?;
        assert_eq!(enabled.kind, GapArtifactKind::GapDecisionLedger);
        assert_eq!(enabled.language, Some(LanguageId::Python));
        assert_eq!(enabled.language_status, Some(LanguageStatus::Preview));
        assert_eq!(
            enabled.static_limit_kinds,
            vec!["missing_import_graph".to_string()]
        );
        assert_eq!(
            enabled.identities[0].canonical_gap_id.as_deref(),
            Some("gap:py:pricing")
        );
        Ok(())
    }

    #[test]
    fn javascript_preview_gap_ledger_is_enabled_by_typescript_adapter() -> Result<(), String> {
        let mut artifact = preview_gap_ledger();
        artifact["records"][0]["gap_id"] = json!("gap:js:pricing");
        artifact["records"][0]["canonical_gap_id"] = json!("gap:js:pricing");
        artifact["records"][0]["language"] = json!("javascript");

        let disabled = validate_gap_artifact(&artifact, &context(&[LanguageId::Rust]));
        assert_eq!(
            disabled,
            Err(GapArtifactRejection::DisabledLanguage(
                "javascript".to_string()
            ))
        );

        let enabled = validate_gap_artifact(
            &artifact,
            &context(&[LanguageId::Rust, LanguageId::TypeScript]),
        )
        .map_err(|err| format!("{err:?}"))?;
        assert_eq!(enabled.language, Some(LanguageId::JavaScript));
        assert_eq!(enabled.language_status, Some(LanguageStatus::Preview));
        assert_eq!(
            enabled.identities[0].canonical_gap_id.as_deref(),
            Some("gap:js:pricing")
        );
        Ok(())
    }

    #[test]
    fn gap_ledger_summary_uses_all_records_for_actionability_and_preview_status()
    -> Result<(), String> {
        let mut artifact = preview_gap_ledger();
        artifact["records"]
            .as_array_mut()
            .ok_or_else(|| "expected records array".to_string())?
            .insert(0, no_action_gap_record());

        let validated =
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust, LanguageId::Python]))
                .map_err(|err| format!("{err:?}"))?;

        assert_eq!(validated.identities.len(), 2);
        assert_eq!(validated.gap_state.as_deref(), Some("actionable"));
        assert!(validated.is_actionable_gap());
        assert!(validated.is_preview());
        Ok(())
    }

    #[test]
    fn gap_ledger_summary_reports_no_action_only_when_all_records_are_no_action()
    -> Result<(), String> {
        let artifact = json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "gap_decision_ledger",
            "root": ".",
            "status": "advisory",
            "records": [
                no_action_gap_record(),
                {
                    "gap_id": "gap:rust:suppressed",
                    "canonical_gap_id": "gap:rust:suppressed",
                    "kind": "MissingBoundaryAssertion",
                    "language": "rust",
                    "language_status": "stable",
                    "scope": "repo_scoped",
                    "evidence_class": "predicate_boundary",
                    "gap_state": "suppressed",
                    "policy_state": "suppressed",
                    "repairability": "repairable",
                    "projection_eligibility": {
                        "lsp_diagnostic": {
                            "eligible": false,
                            "reason": "suppressed"
                        }
                    }
                }
            ]
        });

        let validated = validate_gap_artifact(&artifact, &context(&[LanguageId::Rust]))
            .map_err(|err| format!("{err:?}"))?;

        assert_eq!(validated.gap_state.as_deref(), Some("no_actionable_seam"));
        assert!(validated.is_no_action_gap());
        assert!(!validated.is_actionable_gap());
        Ok(())
    }

    #[test]
    fn gap_ledger_validation_checks_languages_for_every_record() -> Result<(), String> {
        let mut artifact = preview_gap_ledger();
        artifact["records"]
            .as_array_mut()
            .ok_or_else(|| "expected records array".to_string())?
            .insert(0, no_action_gap_record());

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::DisabledLanguage("python".to_string()))
        );
        Ok(())
    }

    #[test]
    fn validation_rejects_wrong_root() {
        let mut artifact = first_action();
        artifact["root"] = json!("/other/workspace");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::WrongRoot(
                "/other/workspace".to_string()
            ))
        );
    }

    #[test]
    fn validation_rejects_unsupported_schema() {
        let mut artifact = first_action();
        artifact["schema_version"] = json!("9.9");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::UnsupportedSchema("9.9".to_string()))
        );
    }

    #[test]
    fn validation_rejects_missing_identity() {
        let mut artifact = first_action();
        artifact["selected"] = json!({"path": "src/pricing.rs"});

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MissingIdentity)
        );
    }

    #[test]
    fn validation_rejects_stale_artifact() {
        let mut artifact = first_action();
        artifact["status"] = json!("stale");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::StaleArtifact)
        );
    }

    #[test]
    fn validation_rejects_out_of_workspace_related_paths() {
        let mut artifact = first_action();
        artifact["target"]["file"] = json!("../outside.rs");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::OutOfWorkspacePath(
                "../outside.rs".to_string()
            ))
        );
    }

    #[test]
    fn validation_rejects_malformed_command_payloads() {
        let mut artifact = first_action();
        artifact["commands"]["verify"] = json!("ripr agent verify --root ../outside --json");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedCommandPayload(
                "ripr agent verify --root ../outside --json".to_string()
            ))
        );
    }

    #[test]
    fn validation_rejects_unregistered_static_limit_kind() {
        let mut artifact = preview_gap_ledger();
        artifact["records"][0]["static_limit_kind"] = json!("runtime_magic");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust, LanguageId::Python])),
            Err(GapArtifactRejection::UnsupportedStaticLimitKind(
                "runtime_magic".to_string()
            ))
        );
    }

    #[test]
    fn evidence_record_validates_text_static_limits_without_parsing_action_semantics()
    -> Result<(), String> {
        let artifact = json!({
            "schema_version": "0.1",
            "seam_id": "seam-2",
            "canonical_gap_id": "gap:rust:seam-2",
            "language": "rust",
            "language_status": "stable",
            "location": {
                "file": "src/lib.rs",
                "line": 12
            },
            "canonical_item": {
                "gap_state": "static_limitation",
                "verify_command": "ripr agent verify --root . --json"
            },
            "evidence_path": {},
            "static_limit_detail": "helper indirection hides the observed value"
        });

        let validated = validate_gap_artifact(&artifact, &context(&[LanguageId::Rust]))
            .map_err(|err| format!("{err:?}"))?;
        assert_eq!(validated.kind, GapArtifactKind::EvidenceRecord);
        assert!(validated.has_text_static_limit);
        assert!(validated.static_limit_kinds.is_empty());
        Ok(())
    }

    #[test]
    fn command_root_accepts_quoted_workspace_path() {
        let workspace = root();
        let command = "ripr agent verify --root \"/workspace\" --json";

        assert!(command_payload_is_safe(&workspace, command));
    }

    #[test]
    fn command_payload_accepts_python_test_verify_commands() {
        let workspace = root();

        assert!(command_payload_is_safe(
            &workspace,
            "pytest tests/test_pricing.py::test_discount_boundary"
        ));
        assert!(command_payload_is_safe(
            &workspace,
            "python -m unittest tests.test_pricing.TestDiscount.test_boundary"
        ));
        assert!(!command_payload_is_safe(
            &workspace,
            "python script.py --do-anything"
        ));
        assert!(!command_payload_is_safe(&workspace, "tox -e py"));
        assert!(!command_payload_is_safe(
            &workspace,
            "pytest ../outside/test_pricing.py"
        ));
        assert!(looks_like_command_payload(
            " pytest tests/test_pricing.py::test_discount_boundary"
        ));
        assert!(looks_like_command_payload(
            "python -m unittest tests.test_pricing.TestDiscount.test_boundary"
        ));
    }

    #[test]
    fn workspace_gap_artifact_loader_validates_known_report_paths() -> Result<(), String> {
        let root = temp_root("loader")?;
        let first_action_path = root.join(DEFAULT_FIRST_USEFUL_ACTION_OUT);
        let parent = first_action_path
            .parent()
            .ok_or_else(|| format!("missing parent for {}", first_action_path.display()))?;
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
        fs::write(&first_action_path, first_action().to_string())
            .map_err(|err| format!("write {}: {err}", first_action_path.display()))?;

        let actionable_path = root.join(DEFAULT_ACTIONABLE_GAPS_OUT);
        let parent = actionable_path
            .parent()
            .ok_or_else(|| format!("missing parent for {}", actionable_path.display()))?;
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
        fs::write(&actionable_path, actionable_gaps_report().to_string())
            .map_err(|err| format!("write {}: {err}", actionable_path.display()))?;

        let report = validate_workspace_gap_artifact_report(&root, &[LanguageId::Rust]);
        assert_eq!(report.artifacts.len(), 2);
        assert!(report.rejections.is_empty());
        assert!(
            report
                .artifacts
                .iter()
                .any(|artifact| artifact.kind == GapArtifactKind::ActionableGaps)
        );
        assert!(
            report
                .artifacts
                .iter()
                .any(|artifact| artifact.kind == GapArtifactKind::FirstUsefulAction)
        );

        fs::remove_dir_all(&root).map_err(|err| format!("remove {}: {err}", root.display()))?;
        Ok(())
    }

    #[test]
    fn workspace_gap_artifact_loader_reports_rejections_for_present_invalid_artifacts()
    -> Result<(), String> {
        let root = temp_root("loader-rejection")?;
        let report_path = root.join(DEFAULT_FIRST_USEFUL_ACTION_OUT);
        let parent = report_path
            .parent()
            .ok_or_else(|| format!("missing parent for {}", report_path.display()))?;
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
        fs::write(&report_path, "{")
            .map_err(|err| format!("write {}: {err}", report_path.display()))?;

        let report = validate_workspace_gap_artifact_report(&root, &[LanguageId::Rust]);
        assert!(report.artifacts.is_empty());
        assert_eq!(
            report.rejections,
            vec![GapArtifactRejection::MalformedArtifact(
                "gap artifact JSON must parse"
            )]
        );
        assert_eq!(report.rejections[0].as_str(), "malformed_artifact");

        fs::remove_dir_all(&root).map_err(|err| format!("remove {}: {err}", root.display()))?;
        Ok(())
    }
}
