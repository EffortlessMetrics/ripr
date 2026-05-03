use super::config::LspAnalysisConfig;
use super::uri::file_uri_for_path;
use crate::app::check_workspace;
use crate::domain::{ExposureClass, Finding, RelatedTest};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use tower_lsp_server::ls_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString,
    Position, Range, Uri,
};

pub struct DiagnosticBatch {
    pub uri: Uri,
    pub diagnostics: Vec<Diagnostic>,
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
    let input = config.check_input(root);
    let output =
        check_workspace(input).map_err(|err| format!("workspace analysis failed: {err}"))?;
    let mut grouped = BTreeMap::<Uri, Vec<Diagnostic>>::new();
    for finding in &output.findings {
        let path = absolute_finding_path(&output.root, finding);
        let uri = file_uri_for_path(&path)?;
        grouped
            .entry(uri)
            .or_default()
            .push(diagnostic_for_finding(&output.root, finding));
    }
    Ok(grouped
        .into_iter()
        .map(|(uri, diagnostics)| DiagnosticBatch { uri, diagnostics })
        .collect())
}

pub(super) fn diagnostic_for_finding(root: &Path, finding: &Finding) -> Diagnostic {
    let line = finding.probe.location.line.saturating_sub(1) as u32;
    Diagnostic {
        range: Range {
            start: Position { line, character: 0 },
            end: Position {
                line,
                character: 120,
            },
        },
        severity: Some(diagnostic_severity_for_class(&finding.class)),
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

pub(super) fn diagnostic_severity_for_class(class: &ExposureClass) -> DiagnosticSeverity {
    match class {
        ExposureClass::Exposed
        | ExposureClass::PropagationUnknown
        | ExposureClass::StaticUnknown => DiagnosticSeverity::INFORMATION,
        ExposureClass::WeaklyExposed
        | ExposureClass::ReachableUnrevealed
        | ExposureClass::NoStaticPath
        | ExposureClass::InfectionUnknown => DiagnosticSeverity::WARNING,
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
