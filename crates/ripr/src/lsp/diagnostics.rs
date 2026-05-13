use super::config::LspAnalysisConfig;
use super::state::{AnalysisSnapshot, RefreshMetadata};
use super::uri::file_uri_for_path;
use crate::analysis::ClassifiedSeam;
use crate::analysis::inventory_classified_seams_at_with_config;
use crate::analysis::seams::SeamGripClass;
use crate::app::check_workspace_with_config;
use crate::config::{ConfigSeverity, SeverityConfig};
use crate::domain::{Finding, LanguageId, RelatedTest};
use std::collections::{BTreeMap, BTreeSet};
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

    let diagnostics_by_uri = grouped.clone();
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
        diagnostics_by_uri,
    };
    Ok(WorkspaceDiagnostics { snapshot, batches })
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
    Diagnostic {
        range: diagnostic_range_for_finding(finding),
        severity: lsp_severity(config.for_exposure(&finding.class)),
        code: Some(NumberOrString::String(finding.class.as_str().to_string())),
        code_description: None,
        source: Some("ripr".to_string()),
        message: lsp_message(finding),
        related_information: related_information_for_finding(root, finding),
        tags: None,
        data: Some(serde_json::json!({
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
        })),
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
    finding
        .recommended_next_step
        .clone()
        .unwrap_or_else(|| format!("{} static RIPR exposure", finding.class.as_str()))
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

    #[test]
    fn absolute_related_test_path_joins_repo_root_for_relative_paths() {
        let test = RelatedTest {
            name: "tests::pricing::handles_discount".to_string(),
            file: PathBuf::from("tests/pricing.rs"),
            line: 33,
            oracle: None,
            oracle_kind: crate::domain::OracleKind::ExactValue,
            oracle_strength: crate::domain::OracleStrength::Weak,
            relation_reason: None,
            relation_confidence: None,
            language: None,
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
            relation_reason: None,
            relation_confidence: None,
            language: None,
        };

        let path = absolute_related_test_path(Path::new("/repo"), &test);
        assert_eq!(path, Path::new("/tmp/workspace/tests/pricing.rs"));
    }
}
