use super::config::LspAnalysisConfig;
use super::gap_artifacts::{
    GapArtifactKind, GapArtifactValidationContext, validate_gap_artifact,
    validate_workspace_gap_artifact_report,
};
use super::state::{AnalysisSnapshot, RefreshMetadata};
use super::uri::file_uri_for_path;
use crate::analysis::ClassifiedSeam;
use crate::analysis::inventory_classified_seams_at_with_config;
use crate::analysis::seams::SeamGripClass;
use crate::app::check_workspace_with_config;
use crate::config::{ConfigSeverity, SeverityConfig};
use crate::domain::{Finding, LanguageId, RelatedTest};
use crate::output::gap_decision_ledger::{
    DEFAULT_GAP_DECISION_LEDGER_OUT, GapRecord, projection_eligible,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use tower_lsp_server::ls_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString,
    Position, Range, Uri,
};

const MAX_DIAGNOSTIC_RANGE_WIDTH: u32 = 120;

pub struct DiagnosticBatch {
    pub uri: Uri,
    pub diagnostics: Vec<Diagnostic>,
}

pub(super) struct WorkspaceDiagnostics {
    pub(super) snapshot: AnalysisSnapshot,
    pub(super) batches: Vec<DiagnosticBatch>,
}

pub(super) struct DiagnosticRefreshPlan {
    pub(super) publish_batches: Vec<DiagnosticBatch>,
    pub(super) clear_uris: Vec<Uri>,
    pub(super) current_uris: BTreeSet<Uri>,
}

pub(super) fn diagnostic_refresh_plan(
    previous_uris: &BTreeSet<Uri>,
    batches: Vec<DiagnosticBatch>,
) -> DiagnosticRefreshPlan {
    let current_uris = batches
        .iter()
        .map(|batch| batch.uri.clone())
        .collect::<BTreeSet<_>>();
    let clear_uris = previous_uris
        .difference(&current_uris)
        .cloned()
        .collect::<Vec<_>>();
    DiagnosticRefreshPlan {
        publish_batches: batches,
        clear_uris,
        current_uris,
    }
}

pub(super) fn take_all_uris(uris: &mut BTreeSet<Uri>) -> Vec<Uri> {
    let cleared = uris.iter().cloned().collect::<Vec<_>>();
    uris.clear();
    cleared
}

pub fn workspace_diagnostic_batches(root: &Path) -> Result<Vec<DiagnosticBatch>, String> {
    workspace_diagnostic_batches_with_config(root, &LspAnalysisConfig::default())
}

pub(super) fn workspace_diagnostic_batches_with_config(
    root: &Path,
    config: &LspAnalysisConfig,
) -> Result<Vec<DiagnosticBatch>, String> {
    Ok(workspace_diagnostics_with_config(root, config)?.batches)
}

pub(super) fn workspace_diagnostics_with_config(
    root: &Path,
    config: &LspAnalysisConfig,
) -> Result<WorkspaceDiagnostics, String> {
    let input = config.check_input(root);
    let output = check_workspace_with_config(input, config.repo_config())
        .map_err(|err| format!("workspace analysis failed: {err}"))?;
    let root = output.root;
    let base = output.base;
    let mode = output.mode;
    let findings = output.findings;
    let mut grouped = BTreeMap::<Uri, Vec<Diagnostic>>::new();
    for finding in &findings {
        let path = absolute_finding_path(&root, finding);
        let uri = file_uri_for_path(&path)?;
        grouped
            .entry(uri)
            .or_default()
            .push(diagnostic_for_finding_with_config(
                &root,
                finding,
                config.repo_config().severity(),
            ));
    }

    // Repo seam evidence diagnostics. Enabled by built-in defaults for the
    // saved-workspace editor model; explicit LSP options or repo policy can
    // still disable it for quieter or larger workspaces.
    //
    // Reliability: a seam-walk failure is downgraded to "no seam
    // diagnostics this refresh", not a hard failure. The opt-in
    // feature must not take down baseline Finding diagnostics if
    // some unrelated repo file confuses the walker. Caught by
    // chatgpt-codex on PR #241.
    let classified_seams = if config.enable_seam_diagnostics
        && config
            .repo_config()
            .languages()
            .enabled()
            .contains(&LanguageId::Rust)
    {
        match inventory_classified_seams_at_with_config(&root, config.repo_config()) {
            Ok(seams) => {
                seams
                    .into_iter()
                    .filter(|entry| {
                        // Drop entries that won't produce a published
                        // diagnostic so `is_consistent` keeps counting
                        // the snapshot accurately. URI-resolution
                        // failures are silent here on purpose: they
                        // are operational noise, not analysis errors.
                        if diagnostic_severity_for_grip_class_with_config(
                            entry.class,
                            config.repo_config().severity(),
                        )
                        .is_none()
                        {
                            return false;
                        }
                        let path = absolute_seam_path(&root, &entry.seam);
                        let Ok(uri) = file_uri_for_path(&path) else {
                            return false;
                        };
                        if let Some(diagnostic) = diagnostic_for_classified_seam_with_config(
                            &root,
                            entry,
                            config.repo_config().severity(),
                        ) {
                            grouped.entry(uri).or_default().push(diagnostic);
                            true
                        } else {
                            false
                        }
                    })
                    .collect()
            }
            Err(err) => {
                eprintln!("ripr lsp: seam diagnostics skipped this refresh: {err}");
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    append_gap_record_diagnostics(
        &root,
        config.repo_config().languages().enabled(),
        &mut grouped,
    );

    let diagnostics_by_uri = grouped.clone();
    let gap_artifact_report =
        validate_workspace_gap_artifact_report(&root, config.repo_config().languages().enabled());
    let batches = grouped
        .into_iter()
        .map(|(uri, diagnostics)| DiagnosticBatch { uri, diagnostics })
        .collect();
    let snapshot = AnalysisSnapshot {
        root,
        base,
        mode,
        refresh: RefreshMetadata::generated_now(),
        findings,
        classified_seams,
        gap_artifacts: gap_artifact_report.artifacts,
        gap_artifact_rejections: gap_artifact_report.rejections,
        diagnostics_by_uri,
    };
    Ok(WorkspaceDiagnostics { snapshot, batches })
}

fn append_gap_record_diagnostics(
    root: &Path,
    enabled_languages: &[LanguageId],
    grouped: &mut BTreeMap<Uri, Vec<Diagnostic>>,
) {
    let ledger_path = root.join(DEFAULT_GAP_DECISION_LEDGER_OUT);
    let contents = match fs::read_to_string(&ledger_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => {
            eprintln!(
                "ripr lsp: gap diagnostics skipped: read {} failed: {err}",
                ledger_path.display()
            );
            return;
        }
    };
    let artifact = match serde_json::from_str::<serde_json::Value>(&contents) {
        Ok(artifact) => artifact,
        Err(err) => {
            eprintln!(
                "ripr lsp: gap diagnostics skipped: parse {} failed: {err}",
                ledger_path.display()
            );
            return;
        }
    };
    let context = GapArtifactValidationContext {
        root,
        enabled_languages,
    };
    match validate_gap_artifact(&artifact, &context) {
        Ok(validated) if validated.kind == GapArtifactKind::GapDecisionLedger => {}
        Ok(_) => {
            eprintln!(
                "ripr lsp: gap diagnostics skipped: {} is not a gap decision ledger",
                ledger_path.display()
            );
            return;
        }
        Err(rejection) => {
            eprintln!(
                "ripr lsp: gap diagnostics skipped: {} rejected as {}",
                ledger_path.display(),
                rejection.as_str()
            );
            return;
        }
    }
    let records = match crate::output::gap_decision_ledger::parse_gap_records_json(&contents) {
        Ok(records) => records,
        Err(err) => {
            eprintln!(
                "ripr lsp: gap diagnostics skipped: parse {} failed: {err}",
                ledger_path.display()
            );
            return;
        }
    };
    for record in &records {
        let Some((uri, diagnostic)) = diagnostic_for_gap_record(root, &ledger_path, record) else {
            continue;
        };
        grouped.entry(uri).or_default().push(diagnostic);
    }
}

fn diagnostic_for_gap_record(
    root: &Path,
    ledger_path: &Path,
    record: &GapRecord,
) -> Option<(Uri, Diagnostic)> {
    if !projection_eligible(record, "lsp_diagnostic") {
        return None;
    }
    let anchor = record.anchor.as_ref()?;
    let file = anchor.file.as_ref()?.trim();
    if file.is_empty() {
        return None;
    }
    let line = anchor.line?;
    if line == 0 {
        return None;
    }
    let path = absolute_gap_anchor_path(root, Path::new(file));
    let uri = file_uri_for_path(&path).ok()?;
    let line_index = line.saturating_sub(1) as u32;
    let diagnostic = Diagnostic {
        range: Range {
            start: Position {
                line: line_index,
                character: 0,
            },
            end: Position {
                line: line_index,
                character: MAX_DIAGNOSTIC_RANGE_WIDTH,
            },
        },
        severity: Some(gap_record_diagnostic_severity(record)),
        code: Some(NumberOrString::String(format!(
            "ripr-gap-{}",
            record.kind.replace('_', "-")
        ))),
        code_description: None,
        source: Some("ripr".to_string()),
        message: gap_record_diagnostic_message(record),
        related_information: None,
        tags: None,
        data: Some(gap_record_diagnostic_data(ledger_path, record)),
    };
    Some((uri, diagnostic))
}

fn absolute_gap_anchor_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn display_lsp_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn gap_record_diagnostic_severity(record: &GapRecord) -> DiagnosticSeverity {
    if record.repairability == "repairable" {
        DiagnosticSeverity::WARNING
    } else {
        DiagnosticSeverity::INFORMATION
    }
}

fn gap_record_diagnostic_message(record: &GapRecord) -> String {
    let kind = non_empty(&record.kind).unwrap_or("Unknown");
    let route = record
        .repair_route
        .as_ref()
        .and_then(|route| non_empty(&route.route_kind))
        .unwrap_or("InspectGap");
    let mut message = format!("ripr gap: {kind}; repair route: {route}");
    if let Some(route) = &record.repair_route {
        if let Some(changed) = route.changed_behavior.as_deref().and_then(non_empty) {
            message.push_str(&format!("; changed behavior: {changed}"));
        }
        if let Some(assertion) = route.assertion_shape.as_deref().and_then(non_empty) {
            message.push_str(&format!("; suggested check: {assertion}"));
        }
    }
    if record.language_status == "preview" {
        message.push_str("; preview advisory evidence");
    }
    message
}

fn gap_record_diagnostic_data(ledger_path: &Path, record: &GapRecord) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "0.1",
        "source": "gap_decision_ledger",
        "gap_ledger": display_lsp_path(ledger_path),
        "gap_id": record.gap_id,
        "canonical_gap_id": record.canonical_gap_id,
        "gap_kind": record.kind,
        "language": record.language,
        "language_status": record.language_status,
        "scope": record.scope,
        "evidence_class": record.evidence_class,
        "gap_state": record.gap_state,
        "policy_state": record.policy_state,
        "repairability": record.repairability,
        "static_limit_kind": record.static_limit_kind,
        "static_limit_detail": record.static_limit_detail,
        "static_limits": record.static_limits,
        "repair_route": record.repair_route,
        "anchor": record.anchor,
        "evidence_ids": record.evidence_ids,
        "verification_commands": record.verification_commands,
        "regeneration_commands": record.regeneration_commands,
        "receipt_command": record.receipt_command,
        "receipt": record.receipt,
        "authority_boundary": record.authority_boundary,
    })
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Per-class severity for seam diagnostics. WARNING for the headline-
/// eligible classes (the agent should act); INFORMATION for `Opaque`
/// (visible but advisory). `StronglyGripped`, `Intentional`, and
/// `Suppressed` produce no diagnostic — `diagnostic_for_classified_seam`
/// returns `None` for those.
pub(super) fn diagnostic_severity_for_grip_class(
    class: SeamGripClass,
) -> Option<DiagnosticSeverity> {
    diagnostic_severity_for_grip_class_with_config(class, &SeverityConfig::default())
}

pub(super) fn diagnostic_severity_for_grip_class_with_config(
    class: SeamGripClass,
    config: &SeverityConfig,
) -> Option<DiagnosticSeverity> {
    lsp_severity(config.for_seam(class))
}

/// Build the LSP `Diagnostic` for a single classified seam, or `None`
/// if the class is not surfacable (strongly gripped / intentional /
/// suppressed). Diagnostic codes are prefixed with `ripr-seam-` so
/// editor consumers can filter by code without parsing severity.
///
/// `_root` is reserved for future range resolution: today seams do
/// not carry a column, so we anchor the range to the full seam line
/// (start char 0 to `MAX_DIAGNOSTIC_RANGE_WIDTH`). That way the
/// squiggle always covers the seam origin even for deeply indented
/// expressions — caught by chatgpt-codex on PR #241. When seams gain
/// a stored column, this function can read the source via `_root` to
/// produce a tighter range.
#[cfg(test)]
pub(super) fn diagnostic_for_classified_seam(
    _root: &Path,
    entry: &ClassifiedSeam,
) -> Option<Diagnostic> {
    diagnostic_for_classified_seam_with_config(_root, entry, &SeverityConfig::default())
}

pub(super) fn diagnostic_for_classified_seam_with_config(
    _root: &Path,
    entry: &ClassifiedSeam,
    config: &SeverityConfig,
) -> Option<Diagnostic> {
    let severity = diagnostic_severity_for_grip_class_with_config(entry.class, config)?;
    let seam = &entry.seam;
    let evidence = &entry.evidence;
    let line = seam.display_line().saturating_sub(1) as u32;
    let range = Range {
        start: Position { line, character: 0 },
        end: Position {
            line,
            character: MAX_DIAGNOSTIC_RANGE_WIDTH,
        },
    };
    Some(Diagnostic {
        range,
        severity: Some(severity),
        code: Some(NumberOrString::String(format!(
            "ripr-seam-{}",
            entry.class.as_str().replace('_', "-")
        ))),
        code_description: None,
        source: Some("ripr".to_string()),
        message: lsp_seam_message(entry),
        related_information: None,
        tags: None,
        data: Some(serde_json::json!({
            "schema_version": "0.1",
            "seam_id": seam.id().as_str(),
            "seam_kind": seam.kind().as_str(),
            "grip_class": entry.class.as_str(),
            "headline_eligible": entry.class.is_headline_eligible(),
            "owner": seam.owner(),
            "expected_sink": seam.expected_sink().as_str(),
            "evidence": {
                "reach": evidence.reach.state.as_str(),
                "activate": evidence.activate.state.as_str(),
                "propagate": evidence.propagate.state.as_str(),
                "observe": evidence.observe.state.as_str(),
                "discriminate": evidence.discriminate.state.as_str(),
            },
        })),
    })
}

fn lsp_seam_message(entry: &ClassifiedSeam) -> String {
    let seam = &entry.seam;
    let head = match entry.class {
        SeamGripClass::Opaque => "Opaque static evidence",
        SeamGripClass::Ungripped => "No detected test grip",
        SeamGripClass::WeaklyGripped => "Weakly gripped behavioral seam",
        SeamGripClass::ReachableUnrevealed => "Test reaches seam but does not reveal it",
        SeamGripClass::ActivationUnknown => "Activation evidence is unclear",
        SeamGripClass::PropagationUnknown => "Propagation to sink is unclear",
        SeamGripClass::ObservationUnknown => "Sink observation is unclear",
        SeamGripClass::DiscriminationUnknown => "Oracle specificity is unclear",
        // Filtered earlier; included for exhaustiveness.
        SeamGripClass::StronglyGripped => "Strongly gripped",
        SeamGripClass::Intentional => "Intentional low-grip",
        SeamGripClass::Suppressed => "Suppressed",
    };
    format!(
        "{} ({}): {}",
        head,
        seam.kind().as_str(),
        seam.expression()
            .lines()
            .next()
            .unwrap_or(seam.expression())
    )
}

fn absolute_seam_path(root: &Path, seam: &crate::analysis::seams::RepoSeam) -> PathBuf {
    let path = seam.file();
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

#[cfg(test)]
pub(super) fn diagnostic_for_finding(root: &Path, finding: &Finding) -> Diagnostic {
    diagnostic_for_finding_with_config(root, finding, &SeverityConfig::default())
}

pub(super) fn diagnostic_for_finding_with_config(
    root: &Path,
    finding: &Finding,
    config: &SeverityConfig,
) -> Diagnostic {
    let mut data = serde_json::json!({
        "schema_version": "0.1",
        "finding_id": finding.id.as_str(),
        "probe_id": finding.probe.id.to_string(),
        "classification": finding.class.as_str(),
        "probe_family": finding.probe.family.as_str(),
        "confidence": finding.confidence,
        "source_range": {
            "file": finding.probe.location.file.display().to_string(),
            "line": finding.probe.location.line,
            "column": finding.probe.location.column,
        },
    });
    if let Some(obj) = data.as_object_mut() {
        if let Some(language) = &finding.language {
            obj.insert(
                "language".to_string(),
                serde_json::Value::String(language.as_str().to_string()),
            );
        }
        if let Some(gap) = &finding.canonical_gap {
            obj.insert(
                "canonical_gap_id".to_string(),
                serde_json::Value::String(gap.id.clone()),
            );
        }
        if let Some(status) = &finding.language_status {
            obj.insert(
                "language_status".to_string(),
                serde_json::Value::String(status.as_str().to_string()),
            );
        }
        if let Some(owner_kind) = &finding.owner_kind {
            obj.insert(
                "owner_kind".to_string(),
                serde_json::Value::String(owner_kind.as_str().to_string()),
            );
        }
        if let Some(static_limit_kind) = &finding.static_limit_kind {
            obj.insert(
                "static_limit_kind".to_string(),
                serde_json::Value::String(static_limit_kind.as_str().to_string()),
            );
        }
    }
    Diagnostic {
        range: diagnostic_range_for_finding(finding),
        severity: lsp_severity(config.for_exposure(&finding.class)),
        code: Some(NumberOrString::String(finding.class.as_str().to_string())),
        code_description: None,
        source: Some("ripr".to_string()),
        message: lsp_message(finding),
        related_information: related_information_for_finding(root, finding),
        tags: None,
        data: Some(data),
    }
}

fn diagnostic_range_for_finding(finding: &Finding) -> Range {
    let line = finding.probe.location.line.saturating_sub(1) as u32;
    let start_character = finding.probe.location.column.saturating_sub(1) as u32;
    let width = expression_lsp_width(&finding.probe.expression).min(MAX_DIAGNOSTIC_RANGE_WIDTH);
    Range {
        start: Position {
            line,
            character: start_character,
        },
        end: Position {
            line,
            character: start_character.saturating_add(width),
        },
    }
}

fn expression_lsp_width(expression: &str) -> u32 {
    expression
        .chars()
        .map(|character| character.len_utf16() as u32)
        .sum::<u32>()
        .max(1)
}

fn related_information_for_finding(
    root: &Path,
    finding: &Finding,
) -> Option<Vec<DiagnosticRelatedInformation>> {
    let related = finding
        .related_tests
        .iter()
        .filter_map(|test| related_information_for_test(root, test))
        .collect::<Vec<_>>();
    if related.is_empty() {
        None
    } else {
        Some(related)
    }
}

fn related_information_for_test(
    root: &Path,
    test: &RelatedTest,
) -> Option<DiagnosticRelatedInformation> {
    let path = absolute_related_test_path(root, test);
    let uri = file_uri_for_path(&path).ok()?;
    let line = test.line.saturating_sub(1) as u32;
    Some(DiagnosticRelatedInformation {
        location: Location {
            uri,
            range: Range {
                start: Position { line, character: 0 },
                end: Position {
                    line,
                    character: 120,
                },
            },
        },
        message: related_test_message(test),
    })
}

fn related_test_message(test: &RelatedTest) -> String {
    let strength = test.oracle_strength.as_str();
    match &test.oracle {
        Some(oracle) => format!(
            "Related test `{}` has {strength} oracle: {oracle}",
            test.name
        ),
        None => format!("Related test `{}` has {strength} oracle", test.name),
    }
}

#[cfg(test)]
pub(super) fn diagnostic_severity_for_class(
    class: &crate::domain::ExposureClass,
) -> DiagnosticSeverity {
    lsp_severity(SeverityConfig::default().for_exposure(class))
        .unwrap_or(DiagnosticSeverity::INFORMATION)
}

fn lsp_severity(severity: ConfigSeverity) -> Option<DiagnosticSeverity> {
    match severity {
        ConfigSeverity::Off => None,
        ConfigSeverity::Info | ConfigSeverity::Note => Some(DiagnosticSeverity::INFORMATION),
        ConfigSeverity::Warning => Some(DiagnosticSeverity::WARNING),
    }
}

fn lsp_message(finding: &Finding) -> String {
    let base = finding
        .recommended_next_step
        .clone()
        .unwrap_or_else(|| format!("{} static RIPR exposure", finding.class.as_str()));
    if finding
        .language_status
        .as_ref()
        .is_some_and(|status| status.as_str() == "preview")
    {
        let language = finding
            .language
            .as_ref()
            .map(|language| language.as_str())
            .unwrap_or("preview-language");
        let mut message = format!("{language} preview evidence (syntax-first, advisory): {base}");
        if let Some(static_limit_kind) = &finding.static_limit_kind {
            message.push_str(&format!(" Static limit: {}.", static_limit_kind.as_str()));
        }
        return message;
    }
    base
}

fn absolute_finding_path(root: &Path, finding: &Finding) -> PathBuf {
    if finding.probe.location.file.is_absolute() {
        finding.probe.location.file.clone()
    } else {
        root.join(&finding.probe.location.file)
    }
}

fn absolute_related_test_path(root: &Path, test: &RelatedTest) -> PathBuf {
    if test.file.is_absolute() {
        test.file.clone()
    } else {
        root.join(&test.file)
    }
}

#[cfg(test)]
mod seam_diagnostic_tests {
    use super::*;
    use crate::analysis::seams::{
        ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind,
    };
    use crate::analysis::test_grip_evidence::TestGripEvidence;
    use crate::domain::{Confidence, StageEvidence, StageState};
    use crate::output::gap_decision_ledger::{GapAnchor, GapRepairRoute, ProjectionEligibility};

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, "test stage")
    }

    fn classified(class: SeamGripClass) -> ClassifiedSeam {
        let seam = RepoSeam::new(
            "src/pricing.rs",
            "pricing::discounted_total",
            SeamKind::PredicateBoundary,
            42,
            88,
            "amount >= discount_threshold",
            RequiredDiscriminator::BoundaryValue {
                description: "amount >= discount_threshold".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        let evidence = TestGripEvidence {
            seam_id: seam.id().clone(),
            related_tests: Vec::new(),
            reach: stage(StageState::Yes),
            activate: stage(StageState::Yes),
            propagate: stage(StageState::Yes),
            observe: stage(StageState::Yes),
            discriminate: stage(StageState::Weak),
            observed_values: Vec::new(),
            missing_discriminators: Vec::new(),
        };
        ClassifiedSeam {
            seam,
            evidence,
            class,
        }
    }

    #[test]
    fn weakly_gripped_seam_emits_warning_with_stable_code() -> Result<(), String> {
        let entry = classified(SeamGripClass::WeaklyGripped);
        let diag = diagnostic_for_classified_seam(Path::new("/repo"), &entry)
            .ok_or_else(|| "expected diagnostic for weakly_gripped".to_string())?;
        if diag.severity != Some(DiagnosticSeverity::WARNING) {
            return Err(format!("expected WARNING, got {:?}", diag.severity));
        }
        match &diag.code {
            Some(NumberOrString::String(code)) if code == "ripr-seam-weakly-gripped" => Ok(()),
            other => Err(format!("expected ripr-seam-weakly-gripped, got {other:?}")),
        }
    }

    #[test]
    fn ungripped_and_reachable_unrevealed_emit_warning() -> Result<(), String> {
        for class in [SeamGripClass::Ungripped, SeamGripClass::ReachableUnrevealed] {
            let entry = classified(class);
            let diag = diagnostic_for_classified_seam(Path::new("/repo"), &entry)
                .ok_or_else(|| format!("expected diagnostic for {}", class.as_str()))?;
            if diag.severity != Some(DiagnosticSeverity::WARNING) {
                return Err(format!(
                    "expected WARNING for {}, got {:?}",
                    class.as_str(),
                    diag.severity
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn unknown_classes_emit_information() -> Result<(), String> {
        for class in [
            SeamGripClass::ActivationUnknown,
            SeamGripClass::PropagationUnknown,
            SeamGripClass::ObservationUnknown,
            SeamGripClass::DiscriminationUnknown,
        ] {
            let entry = classified(class);
            let diag = diagnostic_for_classified_seam(Path::new("/repo"), &entry)
                .ok_or_else(|| format!("expected diagnostic for {}", class.as_str()))?;
            if diag.severity != Some(DiagnosticSeverity::INFORMATION) {
                return Err(format!(
                    "expected INFORMATION for {}, got {:?}",
                    class.as_str(),
                    diag.severity
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn opaque_emits_information_severity() -> Result<(), String> {
        let entry = classified(SeamGripClass::Opaque);
        let diag = diagnostic_for_classified_seam(Path::new("/repo"), &entry)
            .ok_or_else(|| "expected diagnostic for opaque".to_string())?;
        if diag.severity != Some(DiagnosticSeverity::INFORMATION) {
            return Err(format!("expected INFORMATION, got {:?}", diag.severity));
        }
        Ok(())
    }

    #[test]
    fn configured_seam_severity_can_disable_a_class() -> Result<(), String> {
        let config =
            crate::config::tests_only_parse("[severity.seams]\nweakly_gripped = \"off\"\n")?;
        let entry = classified(SeamGripClass::WeaklyGripped);
        let diagnostic = diagnostic_for_classified_seam_with_config(
            Path::new("/repo"),
            &entry,
            config.severity(),
        );
        if diagnostic.is_some() {
            return Err("configured off severity should suppress seam diagnostic".to_string());
        }
        Ok(())
    }

    #[test]
    fn strongly_gripped_emits_no_diagnostic() {
        let entry = classified(SeamGripClass::StronglyGripped);
        assert!(diagnostic_for_classified_seam(Path::new("/repo"), &entry).is_none());
    }

    #[test]
    fn intentional_and_suppressed_emit_no_diagnostic() {
        for class in [SeamGripClass::Intentional, SeamGripClass::Suppressed] {
            let entry = classified(class);
            assert!(
                diagnostic_for_classified_seam(Path::new("/repo"), &entry).is_none(),
                "{} should produce no diagnostic",
                class.as_str()
            );
        }
    }

    #[test]
    fn diagnostic_data_field_carries_seam_id_and_grip_class() -> Result<(), String> {
        let entry = classified(SeamGripClass::WeaklyGripped);
        let diag = diagnostic_for_classified_seam(Path::new("/repo"), &entry)
            .ok_or_else(|| "expected diagnostic".to_string())?;
        let data = diag
            .data
            .as_ref()
            .ok_or_else(|| "missing data".to_string())?;
        let seam_id = data
            .get("seam_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing seam_id".to_string())?;
        if seam_id != entry.seam.id().as_str() {
            return Err(format!("seam_id mismatch: {seam_id}"));
        }
        let grip_class = data
            .get("grip_class")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing grip_class".to_string())?;
        if grip_class != "weakly_gripped" {
            return Err(format!("grip_class mismatch: {grip_class}"));
        }
        Ok(())
    }

    #[test]
    fn gap_record_diagnostic_carries_shared_repair_payload() -> Result<(), String> {
        let record = gap_record(true);
        let (_, diagnostic) = diagnostic_for_gap_record(
            Path::new("/repo"),
            Path::new("/repo/target/ripr/reports/gap-decision-ledger.json"),
            &record,
        )
        .ok_or_else(|| "expected gap diagnostic".to_string())?;

        if diagnostic.severity != Some(DiagnosticSeverity::WARNING) {
            return Err(format!(
                "expected warning severity, got {:?}",
                diagnostic.severity
            ));
        }
        match &diagnostic.code {
            Some(NumberOrString::String(code)) if code == "ripr-gap-MissingBoundaryAssertion" => {}
            other => return Err(format!("unexpected diagnostic code: {other:?}")),
        }
        if !diagnostic
            .message
            .contains("repair route: AddBoundaryAssertion")
            || !diagnostic.message.contains("amount >= threshold")
            || diagnostic.message.contains("confidence")
        {
            return Err(format!(
                "unexpected gap diagnostic message: {}",
                diagnostic.message
            ));
        }
        let data = diagnostic
            .data
            .as_ref()
            .ok_or_else(|| "missing diagnostic data".to_string())?;
        assert_eq!(data["source"], "gap_decision_ledger");
        assert_eq!(data["gap_id"], "gap:pr:pricing:threshold-boundary");
        assert_eq!(data["gap_kind"], "MissingBoundaryAssertion");
        assert_eq!(data["repair_route"]["route_kind"], "AddBoundaryAssertion");
        assert_eq!(
            data["verification_commands"][0],
            "cargo xtask fixtures boundary_gap"
        );
        Ok(())
    }

    #[test]
    fn gap_record_diagnostic_requires_projection_eligibility_and_anchor() {
        let mut record = gap_record(false);
        assert!(
            diagnostic_for_gap_record(Path::new("/repo"), Path::new("ledger.json"), &record)
                .is_none()
        );

        record.projection_eligibility.insert(
            "lsp_diagnostic".to_string(),
            ProjectionEligibility {
                eligible: true,
                reason: "local_file_scope".to_string(),
            },
        );
        record.anchor = None;
        assert!(
            diagnostic_for_gap_record(Path::new("/repo"), Path::new("ledger.json"), &record)
                .is_none()
        );
    }

    #[test]
    fn gap_record_diagnostic_names_preview_inspection_route() -> Result<(), String> {
        let mut record = gap_record(true);
        record.repairability = "inspect_only".to_string();
        record.language_status = "preview".to_string();
        record.repair_route = None;

        let (_, diagnostic) =
            diagnostic_for_gap_record(Path::new("/repo"), Path::new("ledger.json"), &record)
                .ok_or_else(|| "expected gap diagnostic".to_string())?;

        if diagnostic.severity != Some(DiagnosticSeverity::INFORMATION) {
            return Err(format!(
                "expected information severity, got {:?}",
                diagnostic.severity
            ));
        }
        if !diagnostic.message.contains("repair route: InspectGap")
            || !diagnostic.message.contains("preview advisory evidence")
        {
            return Err(format!(
                "unexpected preview gap diagnostic message: {}",
                diagnostic.message
            ));
        }
        Ok(())
    }

    #[test]
    fn append_gap_record_diagnostics_reads_default_ledger() -> Result<(), String> {
        let root = temp_gap_root()?;
        let ledger_path = root.join(DEFAULT_GAP_DECISION_LEDGER_OUT);
        let contents = gap_ledger_json(vec![gap_record(true)]).to_string();
        fs::write(&ledger_path, contents)
            .map_err(|err| format!("write {} failed: {err}", ledger_path.display()))?;

        let mut grouped = std::collections::BTreeMap::new();
        append_gap_record_diagnostics(&root, &[LanguageId::Rust], &mut grouped);

        let diagnostic_count: usize = grouped.values().map(Vec::len).sum();
        if diagnostic_count != 1 {
            return Err(format!(
                "expected one gap diagnostic, got {diagnostic_count}"
            ));
        }
        let uri = grouped
            .keys()
            .next()
            .ok_or_else(|| "missing diagnostic URI".to_string())?
            .as_str()
            .to_string();
        if !uri.ends_with("/src/pricing.rs") {
            return Err(format!("unexpected diagnostic URI: {uri}"));
        }

        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    #[test]
    fn append_gap_record_diagnostics_fails_closed_for_invalid_artifacts() -> Result<(), String> {
        let root = temp_gap_root()?;
        let ledger_path = root.join(DEFAULT_GAP_DECISION_LEDGER_OUT);

        let mut stale = gap_ledger_json(vec![gap_record(true)]);
        stale["status"] = serde_json::json!("stale");
        fs::write(&ledger_path, stale.to_string())
            .map_err(|err| format!("write stale ledger failed: {err}"))?;
        let mut grouped = std::collections::BTreeMap::new();
        append_gap_record_diagnostics(&root, &[LanguageId::Rust], &mut grouped);
        assert!(
            grouped.is_empty(),
            "stale gap artifact must not publish diagnostics"
        );

        fs::write(&ledger_path, "{")
            .map_err(|err| format!("write malformed ledger failed: {err}"))?;
        append_gap_record_diagnostics(&root, &[LanguageId::Rust], &mut grouped);
        assert!(
            grouped.is_empty(),
            "malformed gap artifact must not publish diagnostics"
        );

        let first_action = serde_json::json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "first_useful_action",
            "root": ".",
            "status": "actionable",
            "selected": {
                "seam_id": "seam:pricing",
                "path": "src/pricing.rs"
            },
            "target": {
                "file": "tests/pricing.rs",
                "related_test": "tests/pricing.rs::handles_threshold"
            },
            "commands": {
                "verify": "ripr agent verify --root . --json",
                "receipt": "ripr agent receipt --root . --json"
            }
        });
        fs::write(&ledger_path, first_action.to_string())
            .map_err(|err| format!("write wrong-kind ledger failed: {err}"))?;
        append_gap_record_diagnostics(&root, &[LanguageId::Rust], &mut grouped);
        assert!(
            grouped.is_empty(),
            "non-ledger gap artifact must not publish diagnostics"
        );

        let mut wrong_root = gap_ledger_json(vec![gap_record(true)]);
        wrong_root["root"] = serde_json::json!("/other/workspace");
        fs::write(&ledger_path, wrong_root.to_string())
            .map_err(|err| format!("write wrong-root ledger failed: {err}"))?;
        append_gap_record_diagnostics(&root, &[LanguageId::Rust], &mut grouped);
        assert!(
            grouped.is_empty(),
            "wrong-root gap artifact must not publish diagnostics"
        );

        let mut disabled_record = gap_record(true);
        disabled_record.language = "python".to_string();
        disabled_record.language_status = "preview".to_string();
        let disabled = gap_ledger_json(vec![disabled_record]);
        fs::write(&ledger_path, disabled.to_string())
            .map_err(|err| format!("write disabled-language ledger failed: {err}"))?;
        append_gap_record_diagnostics(&root, &[LanguageId::Rust], &mut grouped);
        assert!(
            grouped.is_empty(),
            "disabled preview-language gap artifact must not publish diagnostics"
        );

        fs::write(&ledger_path, "{not json")
            .map_err(|err| format!("write malformed ledger failed: {err}"))?;
        append_gap_record_diagnostics(&root, &[LanguageId::Rust], &mut grouped);
        assert!(
            grouped.is_empty(),
            "malformed gap artifact must not publish diagnostics"
        );

        let first_useful_action = serde_json::json!({
            "schema_version": "0.1",
            "kind": "first_useful_action",
            "root": ".",
            "canonical_gap_id": "gap:rust:first-useful-action",
            "language": "rust",
            "language_status": "stable",
        });
        fs::write(&ledger_path, first_useful_action.to_string())
            .map_err(|err| format!("write non-ledger artifact failed: {err}"))?;
        append_gap_record_diagnostics(&root, &[LanguageId::Rust], &mut grouped);
        assert!(
            grouped.is_empty(),
            "non-ledger gap artifact must not publish ledger diagnostics"
        );

        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {} failed: {err}", root.display()))?;
        Ok(())
    }

    #[test]
    fn diagnostic_message_names_seam_kind_and_expression() -> Result<(), String> {
        let entry = classified(SeamGripClass::WeaklyGripped);
        let diag = diagnostic_for_classified_seam(Path::new("/repo"), &entry)
            .ok_or_else(|| "expected diagnostic".to_string())?;
        if !diag.message.contains("predicate_boundary") {
            return Err(format!("message missing kind: {}", diag.message));
        }
        if !diag.message.contains("amount >= discount_threshold") {
            return Err(format!("message missing expression: {}", diag.message));
        }
        Ok(())
    }

    fn gap_record(lsp_eligible: bool) -> GapRecord {
        let mut projection_eligibility = BTreeMap::new();
        projection_eligibility.insert(
            "lsp_diagnostic".to_string(),
            ProjectionEligibility {
                eligible: lsp_eligible,
                reason: "local_file_scope".to_string(),
            },
        );
        GapRecord {
            gap_id: "gap:pr:pricing:threshold-boundary".to_string(),
            canonical_gap_id: "gap:rust:pricing:threshold-boundary".to_string(),
            kind: "MissingBoundaryAssertion".to_string(),
            language: "rust".to_string(),
            language_status: "stable".to_string(),
            scope: "pr_local".to_string(),
            evidence_class: "presentation_text".to_string(),
            gap_state: "actionable".to_string(),
            policy_state: "new".to_string(),
            repairability: "repairable".to_string(),
            repair_route: Some(GapRepairRoute {
                route_kind: "AddBoundaryAssertion".to_string(),
                target_file: Some("tests/pricing.rs".to_string()),
                target_line: Some(33),
                related_test: Some("tests/pricing.rs::discount_threshold".to_string()),
                assertion_shape: Some("assert_eq!(price(threshold), expected)".to_string()),
                changed_behavior: Some("amount >= threshold".to_string()),
                stop_conditions: vec!["Stop if the target owner moved.".to_string()],
            }),
            static_limit_kind: None,
            static_limit_detail: None,
            static_limits: Vec::new(),
            anchor: Some(GapAnchor {
                file: Some("src/pricing.rs".to_string()),
                line: Some(42),
                owner: Some("pricing::discounted_total".to_string()),
                dedupe_fingerprint: Some("gap:rust:pricing:threshold-boundary".to_string()),
            }),
            evidence_ids: vec!["evidence:pricing".to_string()],
            projection_eligibility,
            verification_commands: vec!["cargo xtask fixtures boundary_gap".to_string()],
            receipt_command: None,
            regeneration_commands: Vec::new(),
            receipt: None,
            safe_gate_predicate: None,
            authority_boundary: "advisory".to_string(),
        }
    }

    fn gap_ledger_json(records: Vec<GapRecord>) -> serde_json::Value {
        serde_json::json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "gap_decision_ledger",
            "status": "advisory",
            "root": ".",
            "records": records,
        })
    }

    fn temp_gap_root() -> Result<PathBuf, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-lsp-gap-diagnostics-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("target/ripr/reports"))
            .map_err(|err| format!("create temp root {} failed: {err}", root.display()))?;
        Ok(root)
    }

    #[test]
    fn absolute_related_test_path_joins_repo_root_for_relative_paths() {
        let test = RelatedTest {
            name: "tests::pricing::handles_discount".to_string(),
            file: PathBuf::from("tests/pricing.rs"),
            line: 33,
            oracle: None,
            oracle_kind: crate::domain::OracleKind::ExactValue,
            oracle_strength: crate::domain::OracleStrength::Weak,
        };

        let path = absolute_related_test_path(Path::new("/repo"), &test);
        assert_eq!(path, Path::new("/repo/tests/pricing.rs"));
    }

    #[test]
    fn absolute_related_test_path_keeps_absolute_paths() {
        let test = RelatedTest {
            name: "tests::pricing::handles_discount".to_string(),
            file: PathBuf::from("/tmp/workspace/tests/pricing.rs"),
            line: 33,
            oracle: None,
            oracle_kind: crate::domain::OracleKind::ExactValue,
            oracle_strength: crate::domain::OracleStrength::Weak,
        };

        let path = absolute_related_test_path(Path::new("/repo"), &test);
        assert_eq!(path, Path::new("/tmp/workspace/tests/pricing.rs"));
    }
}
