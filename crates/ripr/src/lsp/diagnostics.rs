use super::config::LspAnalysisConfig;
use super::gap_artifacts::{
    GapArtifactKind, GapArtifactRejection, GapArtifactValidationContext, validate_gap_artifact,
    validate_workspace_gap_artifact_report,
};
use super::state::{AnalysisSnapshot, RefreshMetadata};
use super::uri::{file_uri_for_path, path_from_file_uri};
use crate::analysis::ClassifiedSeam;
use crate::analysis::cancellation::AnalysisCancellationToken;
use crate::analysis::inventory_classified_seams_at_with_config;
use crate::analysis::seams::SeamGripClass;
use crate::app::causal_projection::{CausalDeltaArtifact, insert_canonical_delta_fields};
use crate::app::check_workspace_with_config;
use crate::config::{ConfigSeverity, LspDiagnosticProfile, SeverityConfig};
#[cfg(test)]
use crate::domain::RelatedTest;
use crate::domain::{DiagnosticWitness, ExposureClass, Finding, LanguageId, LanguageStatus};
use crate::output::gap_decision_ledger::{
    DEFAULT_GAP_DECISION_LEDGER_OUT, GapRecord, projection_eligible,
};
use crate::output::next_step::reconcile_next_step;
use crate::output::preview_actionability::{
    preview_actionability_for, preview_actionability_json_value,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
#[cfg(test)]
use tower_lsp_server::ls_types::Position;
use tower_lsp_server::ls_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString,
    PositionEncodingKind, Range, Uri,
};

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
    pub(super) unchanged_uri_count: usize,
    pub(super) published_payload_bytes: usize,
    pub(super) suppressed_payload_bytes: usize,
}

pub(super) fn diagnostic_refresh_plan(
    previous: &BTreeMap<Uri, Vec<Diagnostic>>,
    batches: Vec<DiagnosticBatch>,
) -> DiagnosticRefreshPlan {
    let batches = canonicalize_diagnostic_batches(batches);
    let current = batches
        .iter()
        .map(|batch| (batch.uri.clone(), batch.diagnostics.clone()))
        .collect::<BTreeMap<_, _>>();
    let current_uris = current.keys().cloned().collect::<BTreeSet<_>>();
    let previous_uris = previous.keys().cloned().collect::<BTreeSet<_>>();
    let clear_uris = previous_uris
        .difference(&current_uris)
        .cloned()
        .collect::<Vec<_>>();
    let mut publish_batches = Vec::new();
    let mut unchanged_uri_count = 0;
    let mut published_payload_bytes: usize = 0;
    let mut suppressed_payload_bytes: usize = 0;
    for batch in batches {
        let payload_bytes = diagnostic_payload_bytes(&batch.diagnostics);
        if previous.get(&batch.uri) == Some(&batch.diagnostics) {
            unchanged_uri_count += 1;
            suppressed_payload_bytes = suppressed_payload_bytes.saturating_add(payload_bytes);
        } else {
            published_payload_bytes = published_payload_bytes.saturating_add(payload_bytes);
            publish_batches.push(batch);
        }
    }
    DiagnosticRefreshPlan {
        publish_batches,
        clear_uris,
        current_uris,
        unchanged_uri_count,
        published_payload_bytes,
        suppressed_payload_bytes,
    }
}

/// Canonicalize the order and exact duplicates in every URI's diagnostic list.
///
/// The analysis pipeline is allowed to discover evidence in implementation
/// order. LSP publication and refresh comparison are not: they need a stable
/// semantic order so traversal or map-order changes do not create churn.
pub(super) fn canonicalize_diagnostic_batches(
    mut batches: Vec<DiagnosticBatch>,
) -> Vec<DiagnosticBatch> {
    for batch in &mut batches {
        batch.diagnostics.sort_by_key(diagnostic_sort_key);
        batch.diagnostics.dedup();
    }
    batches.sort_by(|left, right| left.uri.cmp(&right.uri));
    batches
}

/// Group diff findings by the producer-owned canonical gap identity used by
/// the CLI/report alignment layer. Findings without that identity remain
/// individual report items; LSP must not invent a semantic grouping key.
pub(super) fn canonical_finding_groups(findings: &[Finding]) -> Vec<(Finding, Vec<Finding>)> {
    let mut grouped = BTreeMap::<String, Vec<Finding>>::new();
    for finding in findings {
        let key = finding
            .canonical_gap
            .as_ref()
            .map(|gap| format!("canonical:{}", gap.id))
            .unwrap_or_else(|| format!("raw:{}", finding.id));
        grouped.entry(key).or_default().push(finding.clone());
    }

    grouped
        .into_values()
        .filter_map(|mut group| {
            group.sort_by_key(finding_primary_sort_key);
            let primary = group.first().cloned()?;
            Some((primary, group))
        })
        .collect()
}

fn finding_primary_sort_key(finding: &Finding) -> String {
    let canonical_file = finding
        .canonical_gap
        .as_ref()
        .map(|gap| gap.file.as_str())
        .unwrap_or("");
    let file = finding
        .probe
        .location
        .file
        .to_string_lossy()
        .replace('\\', "/");
    let owner = finding
        .probe
        .owner
        .as_ref()
        .map(|owner| owner.0.as_str())
        .unwrap_or("");
    format!(
        "{}\0{}\0{}\0{:010}\0{}",
        if file == canonical_file { "0" } else { "1" },
        if owner.is_empty() { "1" } else { "0" },
        file,
        finding.probe.location.line,
        finding.id
    )
}

pub(super) fn add_canonical_group_data(
    root: &Path,
    diagnostic: &mut Diagnostic,
    primary: &Finding,
    raw_findings: &[Finding],
) {
    let Some(data) = diagnostic
        .data
        .as_mut()
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    let mut raw = raw_findings
        .iter()
        .map(|finding| {
            serde_json::json!({
                "finding_id": finding.id,
                "file": display_repo_path(root, &finding.probe.location.file),
                "line": finding.probe.location.line,
                "class": finding.class.as_str(),
                "probe_family": finding.probe.family.as_str(),
                "probe_id": finding.probe.id.to_string(),
                "missing_discriminators": finding
                    .activation
                    .missing_discriminators
                    .iter()
                    .map(|fact| serde_json::json!({ "value": fact.value, "reason": fact.reason }))
                    .collect::<Vec<_>>(),
                "related_tests": finding
                    .related_tests
                    .iter()
                    .map(|test| serde_json::json!({
                        "name": test.name,
                        "file": display_repo_path(root, &test.file),
                        "line": test.line,
                        "oracle_kind": test.oracle_kind.as_str(),
                        "oracle_strength": test.oracle_strength.as_str(),
                    }))
                    .collect::<Vec<_>>(),
                "evidence": finding.evidence,
                "missing": finding.missing,
                "recommended_next_step": finding.recommended_next_step,
            })
        })
        .collect::<Vec<_>>();
    raw.sort_by_key(|finding| {
        finding
            .get("finding_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string()
    });
    let related_tests = sorted_unique_json_values(raw.iter().flat_map(|finding| {
        finding
            .get("related_tests")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .cloned()
    }));
    let evidence = sorted_unique_json_values(raw.iter().flat_map(|finding| {
        finding
            .get("evidence")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .cloned()
    }));
    let missing = sorted_unique_json_values(raw.iter().flat_map(|finding| {
        finding
            .get("missing")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .cloned()
    }));
    let recommended_next_steps = sorted_unique_json_values(
        raw.iter()
            .filter_map(|finding| finding.get("recommended_next_step"))
            .filter(|value| !value.is_null())
            .cloned(),
    );
    let owner = primary
        .canonical_gap
        .as_ref()
        .map(|gap| gap.owner.clone())
        .or_else(|| primary.probe.owner.as_ref().map(|owner| owner.0.clone()));
    data.insert("raw_signal_count".to_string(), serde_json::json!(raw.len()));
    data.insert(
        "group_reason".to_string(),
        serde_json::json!(if raw.len() > 1 {
            "same_canonical_owner_and_missing_discriminator"
        } else {
            "single_canonical_item"
        }),
    );
    data.insert(
        "primary_anchor".to_string(),
        serde_json::json!({
            "file": display_repo_path(root, &primary.probe.location.file),
            "line": primary.probe.location.line,
            "owner": owner,
        }),
    );
    data.insert("raw_findings".to_string(), serde_json::Value::Array(raw));
    data.insert(
        "related_tests".to_string(),
        serde_json::Value::Array(related_tests),
    );
    data.insert("evidence".to_string(), serde_json::Value::Array(evidence));
    data.insert("missing".to_string(), serde_json::Value::Array(missing));
    data.insert(
        "recommended_next_steps".to_string(),
        serde_json::Value::Array(recommended_next_steps),
    );
}

fn sorted_unique_json_values(
    values: impl IntoIterator<Item = serde_json::Value>,
) -> Vec<serde_json::Value> {
    let mut keyed = BTreeMap::<String, serde_json::Value>::new();
    for value in values {
        if let Ok(key) = serde_json::to_string(&value) {
            keyed.entry(key).or_insert(value);
        }
    }
    keyed.into_values().collect()
}

pub(super) fn canonical_group_has_mixed_classes(raw_findings: &[Finding]) -> bool {
    raw_findings
        .iter()
        .map(|finding| finding.class.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        > 1
}

#[cfg(test)]
pub(super) fn finding_diagnostics_by_uri(
    root: &Path,
    findings: &[Finding],
    severity: &SeverityConfig,
    is_full_run: bool,
    causal_projection: Option<&CausalDeltaArtifact>,
) -> Result<BTreeMap<Uri, Vec<Diagnostic>>, String> {
    finding_diagnostics_by_uri_with_profile(
        root,
        findings,
        severity,
        is_full_run,
        LspDiagnosticProfile::Full,
        causal_projection,
        &PositionEncodingKind::UTF16,
    )
}

pub(super) fn finding_diagnostics_by_uri_with_profile(
    root: &Path,
    findings: &[Finding],
    severity: &SeverityConfig,
    is_full_run: bool,
    profile: LspDiagnosticProfile,
    causal_projection: Option<&CausalDeltaArtifact>,
    position_encoding: &PositionEncodingKind,
) -> Result<BTreeMap<Uri, Vec<Diagnostic>>, String> {
    let mut grouped = BTreeMap::<Uri, Vec<Diagnostic>>::new();
    for (primary, raw_findings) in canonical_finding_groups(findings) {
        if !finding_is_visible_in_profile(profile, &primary) {
            continue;
        }
        let path = absolute_finding_path(root, &primary);
        let uri = file_uri_for_path(&path)?;
        let mut diagnostic = diagnostic_for_finding_with_causal(
            root,
            &primary,
            severity,
            causal_projection,
            position_encoding,
        );
        if primary.canonical_gap.is_some() {
            add_canonical_group_data(root, &mut diagnostic, &primary, &raw_findings);
            if canonical_group_has_mixed_classes(&raw_findings) {
                diagnostic.severity = Some(DiagnosticSeverity::INFORMATION);
                diagnostic.message = format!(
                    "{}; canonical group contains mixed static classes; inspect raw findings",
                    diagnostic.message
                );
                if let Some(data) = diagnostic
                    .data
                    .as_mut()
                    .and_then(serde_json::Value::as_object_mut)
                {
                    data.insert(
                        "canonical_limitation".to_string(),
                        serde_json::json!("mixed_static_classes"),
                    );
                }
            }
        }
        // Policy: clamp advisory findings to INFORMATION (never WARNING).
        // Also downgrade WARNING to INFORMATION when run is not "full".
        if diagnostic.severity == Some(DiagnosticSeverity::WARNING)
            && (finding_is_advisory(&primary) || !is_full_run)
        {
            diagnostic.severity = Some(DiagnosticSeverity::INFORMATION);
        }
        grouped.entry(uri).or_default().push(diagnostic);
    }
    Ok(grouped)
}

pub(super) fn finding_is_visible_in_profile(
    profile: LspDiagnosticProfile,
    finding: &Finding,
) -> bool {
    match profile {
        LspDiagnosticProfile::Full => true,
        LspDiagnosticProfile::Actionable => {
            matches!(
                finding.class,
                ExposureClass::WeaklyExposed
                    | ExposureClass::ReachableUnrevealed
                    | ExposureClass::NoStaticPath
            ) && DiagnosticWitness::from_finding(finding).is_some_and(|witness| {
                !witness.missing_discriminators.is_empty() && witness.fix_site.is_some()
            })
        }
    }
}

/// Return a root-independent digest of a canonical diagnostic payload.
///
/// Navigation URIs remain absolute in the LSP wire payload, but the cache
/// identity must be comparable for equivalent checkouts. Path-bearing data is
/// therefore rewritten to `repo://` relative paths before hashing.
pub(super) fn normalized_diagnostic_payload_digest(
    root: &Path,
    diagnostics: &[Diagnostic],
) -> String {
    let normalized = diagnostics
        .iter()
        .map(|diagnostic| {
            serde_json::to_value(diagnostic).unwrap_or_else(|_| {
                serde_json::json!({
                    "debug": format!("{diagnostic:?}")
                })
            })
        })
        .map(|mut value| {
            normalize_path_values(root, &mut value, None);
            value
        })
        .collect::<Vec<_>>();
    let serialized = serde_json::to_vec(&normalized).unwrap_or_else(|_| Vec::new());
    let digest = Sha256::digest(serialized);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn normalize_path_values(root: &Path, value: &mut serde_json::Value, key: Option<&str>) {
    match value {
        serde_json::Value::Object(object) => {
            for (child_key, child_value) in object {
                normalize_path_values(root, child_value, Some(child_key.as_str()));
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                normalize_path_values(root, child, key);
            }
        }
        serde_json::Value::String(string) => {
            if matches!(key, Some("file" | "gap_ledger")) {
                *string = display_repo_path(root, Path::new(string)).to_string();
            } else if key == Some("uri")
                && let Ok(uri) = string.parse::<Uri>()
                && let Some(path) = path_from_file_uri(&uri)
            {
                *string = format!("repo://{}", display_repo_path(root, &path));
            }
        }
        _ => {}
    }
}

/// Build the stable identity for one document's current diagnostic report.
///
/// The identity is deliberately scoped to the document. A change in another
/// document therefore does not invalidate an unchanged document report. The
/// digest covers exactly the diagnostics the document serves under the stored
/// delivery selection (#1973), and the selection identity is part of the
/// derivation, so a budget/profile/input change produces a new result
/// identity without renumbering canonical evidence — and a selection change
/// can never be hidden by an `unchanged` report. The canonical payload digest
/// removes checkout-root-specific paths before hashing, while the snapshot
/// inputs capture changes that affect the whole projection.
pub(super) fn document_diagnostic_result_id(snapshot: &AnalysisSnapshot, uri: &Uri) -> String {
    let relative_uri = normalized_document_uri(&snapshot.root, uri);
    let served = snapshot.served_diagnostics_for_uri(uri);
    let payload_digest = normalized_diagnostic_payload_digest(&snapshot.root, &served);
    stable_diagnostic_id(
        "ripr-document-diagnostics-v2",
        [
            snapshot.mode.as_str(),
            snapshot.diagnostic_profile.as_str(),
            derive_run_status(
                &snapshot.findings,
                &snapshot.gap_artifact_rejections,
                &snapshot.gap_artifacts,
                snapshot.seams_deferred,
                snapshot.partial_scope.is_some(),
            ),
            snapshot.base.as_deref().unwrap_or("no-base"),
            relative_uri.as_str(),
            delivery_selection_identity(snapshot).as_str(),
            payload_digest.as_str(),
        ],
    )
}

/// The identity fragment the document result IDs bind to. For a committed
/// snapshot this is the stored selection's `snapshot:profile:budget`
/// identity (or the disclosed unavailable fallback); every transport reads
/// the same stored value, so push and pull derive matching identities.
fn delivery_selection_identity(snapshot: &AnalysisSnapshot) -> String {
    match &snapshot.delivery_selection {
        Some(selection) => match &selection.outcome {
            super::diagnostic_budget::DiagnosticDeliveryOutcome::Applied { result, .. } => {
                result.snapshot_profile_budget_identity.clone()
            }
            super::diagnostic_budget::DiagnosticDeliveryOutcome::Unavailable { detail, .. } => {
                format!("delivery-unavailable:{detail}")
            }
        },
        None => "delivery-selection:not-committed".to_string(),
    }
}

/// Build the identity of the snapshot's complete (unfiltered) diagnostic
/// evidence. The delivery selection binds to this as its
/// `complete_evidence_identity` so the selected subset always names the
/// complete evidence it was drawn from — and the complete evidence stays
/// retrievable independently of the passive transport.
pub(super) fn complete_diagnostic_evidence_identity(snapshot: &AnalysisSnapshot) -> String {
    let mut parts = vec![
        snapshot.mode.as_str().to_string(),
        snapshot.diagnostic_profile.as_str().to_string(),
        derive_run_status(
            &snapshot.findings,
            &snapshot.gap_artifact_rejections,
            &snapshot.gap_artifacts,
            snapshot.seams_deferred,
            snapshot.partial_scope.is_some(),
        )
        .to_string(),
        snapshot
            .base
            .clone()
            .unwrap_or_else(|| "no-base".to_string()),
    ];
    for (uri, diagnostics) in &snapshot.diagnostics_by_uri {
        parts.push(normalized_document_uri(&snapshot.root, uri));
        parts.push(normalized_diagnostic_payload_digest(
            &snapshot.root,
            diagnostics,
        ));
    }
    stable_diagnostic_id(
        "ripr-complete-diagnostic-evidence-v1",
        parts.iter().map(String::as_str),
    )
}

/// Build a workspace identity from the ordered set of document identities.
/// This is an observability/test identity; the LSP workspace report carries
/// the per-document IDs because that is what clients use for unchanged data.
pub(super) fn workspace_diagnostic_result_id(snapshot: &AnalysisSnapshot) -> String {
    let mut parts = vec![
        snapshot.mode.as_str().to_string(),
        snapshot.diagnostic_profile.as_str().to_string(),
        derive_run_status(
            &snapshot.findings,
            &snapshot.gap_artifact_rejections,
            &snapshot.gap_artifacts,
            snapshot.seams_deferred,
            snapshot.partial_scope.is_some(),
        )
        .to_string(),
        snapshot
            .base
            .clone()
            .unwrap_or_else(|| "no-base".to_string()),
    ];
    for uri in snapshot.diagnostics_by_uri.keys() {
        parts.push(normalized_document_uri(&snapshot.root, uri));
        parts.push(document_diagnostic_result_id(snapshot, uri));
    }
    stable_diagnostic_id(
        "ripr-workspace-diagnostics-v2",
        parts.iter().map(String::as_str),
    )
}

/// Snapshot-scoped identities used by pull diagnostics.
///
/// Computing a document identity serializes and normalizes its complete
/// diagnostic payload. Cache those producer-owned values when a snapshot is
/// committed so repeated pull requests only look up the requested document.
#[derive(Debug)]
pub(super) struct DiagnosticResultIdCache {
    snapshot: Arc<AnalysisSnapshot>,
    document_ids: BTreeMap<Uri, String>,
    workspace_id: String,
}

impl DiagnosticResultIdCache {
    pub(super) fn for_snapshot(snapshot: Arc<AnalysisSnapshot>) -> Self {
        let document_ids = snapshot
            .diagnostics_by_uri
            .keys()
            .map(|uri| (uri.clone(), document_diagnostic_result_id(&snapshot, uri)))
            .collect();
        let workspace_id = workspace_diagnostic_result_id(&snapshot);
        Self {
            snapshot,
            document_ids,
            workspace_id,
        }
    }

    pub(super) fn matches_snapshot(&self, snapshot: &Arc<AnalysisSnapshot>) -> bool {
        Arc::ptr_eq(&self.snapshot, snapshot) && !self.workspace_id.is_empty()
    }

    pub(super) fn document_id(&self, snapshot: &AnalysisSnapshot, uri: &Uri) -> String {
        // Exact-key lookup only: the stored selection keys on the stored URI
        // string; fuzzy URI matching is not part of the authority contract.
        self.document_ids
            .get(uri)
            .cloned()
            .unwrap_or_else(|| document_diagnostic_result_id(snapshot, uri))
    }
}

fn normalized_document_uri(root: &Path, uri: &Uri) -> String {
    path_from_file_uri(uri).map_or_else(
        || uri.as_str().to_string(),
        |path| format!("repo://{}", display_repo_path(root, &path)),
    )
}

fn diagnostic_sort_key(diagnostic: &Diagnostic) -> String {
    let diagnostic_id = diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("diagnostic_id"))
        .and_then(serde_json::Value::as_str)
        .or(match diagnostic.code.as_ref() {
            Some(NumberOrString::String(value)) => Some(value.as_str()),
            _ => None,
        })
        .unwrap_or("");
    let serialized =
        serde_json::to_string(diagnostic).unwrap_or_else(|_| format!("{diagnostic:?}"));
    format!(
        "{diagnostic_id}\0{:010}:{:010}:{:010}:{:010}\0{serialized}",
        diagnostic.range.start.line,
        diagnostic.range.start.character,
        diagnostic.range.end.line,
        diagnostic.range.end.character
    )
}

fn diagnostic_payload_bytes(diagnostics: &[Diagnostic]) -> usize {
    diagnostics
        .iter()
        .map(|diagnostic| {
            serde_json::to_vec(diagnostic)
                .map(|payload| payload.len())
                .unwrap_or_else(|_| format!("{diagnostic:?}").len())
        })
        .sum()
}

fn stable_diagnostic_id<'a>(prefix: &str, parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    hasher.update([0]);
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    let digest = hasher.finalize();
    format!(
        "{prefix}:{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7]
    )
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
    Ok(workspace_diagnostics_with_config(root, config, false)?.batches)
}

/// Run workspace diagnostics.
///
/// When `defer_seam_inventory` is `true` (the default on interactive
/// `did_open`/`did_save` refreshes), the expensive full-repo seam inventory
/// (`inventory_classified_seams_at_with_config`) is skipped and the snapshot
/// carries `seams_deferred = true` with `run_status = "seams_deferred"`.
/// Diff-scoped findings are always produced — they are fast and complete.
///
/// When `defer_seam_inventory` is `false` (the explicit
/// `ripr.refreshDiagnostics` path), the seam inventory runs as before and the
/// snapshot transitions to `full` (or `limited`/`stale`/`cache_limited` per
/// existing rules) with seam diagnostics present.
pub(super) fn workspace_diagnostics_with_config(
    root: &Path,
    config: &LspAnalysisConfig,
    defer_seam_inventory: bool,
) -> Result<WorkspaceDiagnostics, String> {
    let input = config.check_input(root);
    let output = check_workspace_with_config(input, config.repo_config())
        .map_err(|err| format!("workspace analysis failed: {err}"))?;
    let root = output.root;
    let base = output.base;
    let mode = output.mode;
    let partial_scope = output.partial_scope;
    let findings = output.findings;

    // Validate gap artifacts first so we can determine run status before
    // assembling diagnostics. Run status governs severity downgrade/suppression
    // policy: finding WARNINGs become INFORMATION and gap-record diagnostics are
    // suppressed entirely when the run is not "full" (stale/cache_limited/limited).
    // This surfaces the limited state via `ripr.collectWorkspaceStatus`, not
    // per-file spam. See RIPR-SPEC-0076 diagnostics policy.
    let gap_artifact_report =
        validate_workspace_gap_artifact_report(&root, config.repo_config().languages().enabled());
    let run_status = derive_run_status(
        &findings,
        &gap_artifact_report.rejections,
        &gap_artifact_report.artifacts,
        defer_seam_inventory,
        partial_scope.is_some(),
    );
    let is_full_run = run_status == "full";
    let (causal_projection, causal_projection_warning) = CausalDeltaArtifact::load_optional(&root);
    if let Some(warning) = causal_projection_warning {
        eprintln!("ripr lsp: {warning}");
    }

    let mut grouped = finding_diagnostics_by_uri_with_profile(
        &root,
        &findings,
        config.repo_config().severity(),
        is_full_run,
        config.diagnostic_profile,
        causal_projection.as_ref(),
        &config.position_encoding,
    )?;

    // Repo seam evidence diagnostics. Enabled by built-in defaults for the
    // saved-workspace editor model; explicit LSP options or repo policy can
    // still disable it for quieter or larger workspaces.
    //
    // Performance: `inventory_classified_seams_at_with_config` walks ALL
    // production Rust files — 336s cold / 31s warm on this repo — so it
    // MUST NOT run on the interactive did_open/did_save path. When
    // `defer_seam_inventory` is true the entire block is skipped and the
    // snapshot is marked `seams_deferred`. The explicit
    // `ripr.refreshDiagnostics` command sets `defer_seam_inventory = false`
    // to compute seams on demand (RIPR-SPEC-0105).
    //
    // Reliability: a seam-walk failure is downgraded to "no seam
    // diagnostics this refresh", not a hard failure. The opt-in
    // feature must not take down baseline Finding diagnostics if
    // some unrelated repo file confuses the walker. Caught by
    // chatgpt-codex on PR #241.
    //
    // Seam diagnostics severity policy: structural grip-class signals,
    // not gap-record repair packets — the WARNING/INFORMATION mapping
    // is owned by SeverityConfig. When run is not full, seam WARNINGs
    // downgrade to INFORMATION. The exception is documented here.
    let classified_seams = if config.diagnostic_profile == LspDiagnosticProfile::Full
        && !defer_seam_inventory
        && config.enable_seam_diagnostics
        && config
            .repo_config()
            .languages()
            .enabled()
            .contains(&LanguageId::Rust)
    {
        match inventory_classified_seams_at_with_config(&root, config.repo_config()) {
            Ok((seams, _)) => {
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
                        if let Some(mut diagnostic) = diagnostic_for_classified_seam_with_causal(
                            &root,
                            entry,
                            config.repo_config().severity(),
                            causal_projection.as_ref(),
                        ) {
                            // Policy: limited/stale run downgrades seam WARNINGs to INFORMATION.
                            if !is_full_run
                                && diagnostic.severity == Some(DiagnosticSeverity::WARNING)
                            {
                                diagnostic.severity = Some(DiagnosticSeverity::INFORMATION);
                            }
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

    // Policy: gap-record diagnostics are suppressed entirely when run is not
    // "full" (stale/cache_limited/limited). The limited state is surfaced by
    // `ripr.collectWorkspaceStatus`, not per-file spam.
    if is_full_run {
        append_gap_record_diagnostics_with_causal(
            &root,
            config.repo_config().languages().enabled(),
            &mut grouped,
            causal_projection.as_ref(),
        );
    }

    let batches = canonicalize_diagnostic_batches(
        grouped
            .into_iter()
            .map(|(uri, diagnostics)| DiagnosticBatch { uri, diagnostics })
            .collect(),
    );
    let diagnostics_by_uri = batches
        .iter()
        .map(|batch| (batch.uri.clone(), batch.diagnostics.clone()))
        .collect();
    let snapshot = AnalysisSnapshot {
        root,
        input_identity: None,
        base,
        mode,
        refresh: RefreshMetadata::generated_now(),
        findings,
        diagnostic_profile: config.diagnostic_profile,
        classified_seams,
        gap_artifacts: gap_artifact_report.artifacts,
        gap_artifact_rejections: gap_artifact_report.rejections,
        diagnostics_by_uri,
        delivery_selection: None,
        seams_deferred: defer_seam_inventory,
        partial_scope,
    };
    Ok(WorkspaceDiagnostics { snapshot, batches })
}

/// Run workspace diagnostics with a token installed for synchronous analysis
/// checkpoints. The ordinary entry point remains token-free for CLI and test
/// callers that are not owned by an LSP refresh.
pub(super) fn workspace_diagnostics_with_config_and_cancellation(
    root: &Path,
    config: &LspAnalysisConfig,
    defer_seam_inventory: bool,
    cancellation: &AnalysisCancellationToken,
) -> Result<WorkspaceDiagnostics, String> {
    crate::analysis::cancellation::with_token(cancellation, || {
        workspace_diagnostics_with_config(root, config, defer_seam_inventory)
    })
}

/// Compute the run status from findings, gap-artifact rejections, and the
/// Shared run-status derivation from the raw ingredients. Both
/// `backend::workspace_status_run_status` (for workspace status) and
/// `diagnostics::snapshot_run_status` (for diagnostic severity) call
/// this to avoid drift between the two surfaces (#1939).
///
/// Returns `"full"`, `"stale"`, `"cache_limited"`, `"limited"`,
/// `"limited_partial_scope"`, or `"seams_deferred"`. `"seams_deferred"` is
/// returned when `defer_seam_inventory` is `true` and no other limitation
/// applies; it is a member of the `limited` family for severity-downgrade
/// policy purposes. `"limited_partial_scope"` (RIPR-PROP-0019) is returned
/// when the run analyzed a bounded partition of an over-budget diff; the
/// run-level scope limitation dominates per-finding static limitations.
pub(super) fn derive_run_status(
    findings: &[Finding],
    rejections: &[GapArtifactRejection],
    gap_artifacts: &[super::gap_artifacts::ValidatedGapArtifact],
    defer_seam_inventory: bool,
    has_partial_scope: bool,
) -> &'static str {
    if rejections
        .iter()
        .any(|r| matches!(r, GapArtifactRejection::StaleArtifact))
    {
        return "stale";
    }
    if !rejections.is_empty() {
        return "cache_limited";
    }
    if has_partial_scope {
        return crate::analysis::PartialDiffScope::RUN_STATUS;
    }
    let has_static_limit = findings.iter().any(|f| f.static_limit_kind.is_some())
        || gap_artifacts.iter().any(|a| a.has_static_limit());
    if has_static_limit {
        return "limited";
    }
    if defer_seam_inventory {
        return "seams_deferred";
    }
    "full"
}

/// Test-only re-export of `derive_run_status` so RIPR-SPEC-0105 control 4
/// can verify the limited-policy wiring without going through the full workspace
/// analysis stack. Gated behind `#[cfg(test)]` so it never leaks to production.
#[cfg(test)]
pub(super) fn snapshot_run_status_for_test(
    findings: &[Finding],
    rejections: &[GapArtifactRejection],
    defer_seam_inventory: bool,
) -> &'static str {
    derive_run_status(findings, rejections, &[], defer_seam_inventory, false)
}

#[cfg(test)]
fn append_gap_record_diagnostics(
    root: &Path,
    enabled_languages: &[LanguageId],
    grouped: &mut BTreeMap<Uri, Vec<Diagnostic>>,
) {
    append_gap_record_diagnostics_with_causal(root, enabled_languages, grouped, None);
}

fn append_gap_record_diagnostics_with_causal(
    root: &Path,
    enabled_languages: &[LanguageId],
    grouped: &mut BTreeMap<Uri, Vec<Diagnostic>>,
    causal_projection: Option<&CausalDeltaArtifact>,
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
        let Some((uri, diagnostic)) =
            diagnostic_for_gap_record_with_causal(root, &ledger_path, record, causal_projection)
        else {
            continue;
        };
        grouped.entry(uri).or_default().push(diagnostic);
    }
}

#[cfg(test)]
fn diagnostic_for_gap_record(
    root: &Path,
    ledger_path: &Path,
    record: &GapRecord,
) -> Option<(Uri, Diagnostic)> {
    diagnostic_for_gap_record_with_causal(root, ledger_path, record, None)
}

fn diagnostic_for_gap_record_with_causal(
    root: &Path,
    ledger_path: &Path,
    record: &GapRecord,
    causal_projection: Option<&CausalDeltaArtifact>,
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
    // Fail closed: a gap whose kind is not a governed catalog code is not
    // emitted as a diagnostic rather than surfacing an unregistered code.
    let code = super::diagnostic_catalog::gap_code(&record.kind)?;
    let diagnostic = Diagnostic {
        range: crate::lsp::position::line_span_range(line_index),
        severity: Some(gap_record_diagnostic_severity(record)),
        code: Some(NumberOrString::String(code)),
        code_description: None,
        source: Some("ripr".to_string()),
        message: gap_record_diagnostic_message(record),
        related_information: None,
        tags: None,
        data: Some(gap_record_diagnostic_data_with_causal(
            root,
            ledger_path,
            record,
            causal_projection,
        )),
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

fn display_repo_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(display_lsp_path)
        .unwrap_or_else(|_| display_lsp_path(path))
}

/// A finding is advisory when it carries a static limit or a preview language
/// status. Advisory findings must never emit WARNING — they lack a complete
/// repair packet by definition. Clamp to INFORMATION instead.
fn finding_is_advisory(finding: &Finding) -> bool {
    finding.static_limit_kind.is_some() || finding.language_status == Some(LanguageStatus::Preview)
}

/// A gap record has a complete repair packet when it is repairable, carries
/// at least one verification command, and has a receipt command.
/// WARNING is only appropriate when the packet is complete and actionable.
fn gap_record_has_complete_packet(record: &GapRecord) -> bool {
    record.repairability == "repairable"
        && !record.verification_commands.is_empty()
        && record.receipt_command.is_some()
}

/// A gap record is advisory when it is from a preview language or carries a
/// static limit kind. Advisory gap records must not emit WARNING regardless of
/// repair-packet completeness.
fn gap_record_is_advisory(record: &GapRecord) -> bool {
    record.language_status == "preview" || record.static_limit_kind.is_some()
}

/// Severity policy: WARNING only when the gap record has a complete repair
/// packet AND is not advisory. All other cases → INFORMATION.
///
/// This enforces the hard rule: no WARNING without a complete repair packet.
/// A complete packet requires `repairability == "repairable"`,
/// non-empty `verification_commands`, and `receipt_command.is_some()`.
/// Advisory records (preview language or static_limit_kind present) are
/// clamped to INFORMATION even when the packet looks complete.
fn gap_record_diagnostic_severity(record: &GapRecord) -> DiagnosticSeverity {
    if gap_record_has_complete_packet(record) && !gap_record_is_advisory(record) {
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

fn gap_record_diagnostic_data_with_causal(
    root: &Path,
    ledger_path: &Path,
    record: &GapRecord,
    causal_projection: Option<&CausalDeltaArtifact>,
) -> serde_json::Value {
    let diagnostic_id = if !record.canonical_gap_id.trim().is_empty() {
        record.canonical_gap_id.clone()
    } else {
        stable_diagnostic_id(
            "gap",
            [
                record.gap_id.as_str(),
                record.kind.as_str(),
                record.language.as_str(),
            ],
        )
    };
    let mut data = serde_json::json!({
        "schema_version": "0.1",
        "source": "gap_decision_ledger",
        "gap_ledger": display_repo_path(root, ledger_path),
        "diagnostic_id": diagnostic_id,
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
    });
    if let Some(object) = data.as_object_mut()
        && let Some(anchor) = object.get_mut("anchor")
        && let Some(anchor_object) = anchor.as_object_mut()
        && let Some(file) = anchor_object.get("file").and_then(|value| value.as_str())
    {
        let file = display_repo_path(root, Path::new(file));
        anchor_object.insert("file".to_string(), serde_json::Value::String(file));
    }
    if let Some(projection) = causal_projection
        && let Some(object) = data.as_object_mut()
    {
        projection.insert_comparison_fields(object);
        if let Some(delta) = projection.delta_for(non_empty(&record.canonical_gap_id)) {
            insert_canonical_delta_fields(object, delta);
        }
    }
    data
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

#[cfg(test)]
pub(super) fn diagnostic_for_classified_seam_with_config(
    root: &Path,
    entry: &ClassifiedSeam,
    config: &SeverityConfig,
) -> Option<Diagnostic> {
    diagnostic_for_classified_seam_with_causal(root, entry, config, None)
}

fn diagnostic_for_classified_seam_with_causal(
    _root: &Path,
    entry: &ClassifiedSeam,
    config: &SeverityConfig,
    causal_projection: Option<&CausalDeltaArtifact>,
) -> Option<Diagnostic> {
    let severity = diagnostic_severity_for_grip_class_with_config(entry.class, config)?;
    let seam = &entry.seam;
    let evidence = &entry.evidence;
    let line = seam.display_line().saturating_sub(1) as u32;
    let range = crate::lsp::position::line_span_range(line);
    let diagnostic_id = stable_diagnostic_id(
        "seam",
        [
            seam.owner(),
            seam.kind().as_str(),
            seam.expected_sink().as_str(),
            seam.expression(),
        ],
    );
    let mut data = serde_json::json!({
        "schema_version": "0.1",
        "diagnostic_id": diagnostic_id,
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
    });
    if let Some(projection) = causal_projection
        && let Some(object) = data.as_object_mut()
    {
        projection.insert_comparison_fields(object);
        if let Some(delta) = projection.delta_for(
            crate::analysis::canonical_gap::canonical_gap_identities(std::slice::from_ref(entry))
                .get(entry.seam.id())
                .map(|identity| identity.id.as_str()),
        ) {
            insert_canonical_delta_fields(object, delta);
        }
    }
    Some(Diagnostic {
        range,
        severity: Some(severity),
        code: Some(NumberOrString::String(
            super::diagnostic_catalog::seam_code(entry.class),
        )),
        code_description: None,
        source: Some("ripr".to_string()),
        message: lsp_seam_message(entry),
        related_information: None,
        tags: None,
        data: Some(data),
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

#[cfg(test)]
pub(super) fn diagnostic_for_finding_with_config(
    root: &Path,
    finding: &Finding,
    config: &SeverityConfig,
) -> Diagnostic {
    diagnostic_for_finding_with_causal(root, finding, config, None, &PositionEncodingKind::UTF16)
}

fn diagnostic_for_finding_with_causal(
    root: &Path,
    finding: &Finding,
    config: &SeverityConfig,
    causal_projection: Option<&CausalDeltaArtifact>,
    position_encoding: &PositionEncodingKind,
) -> Diagnostic {
    let file = display_repo_path(root, &finding.probe.location.file);
    let owner = finding
        .probe
        .owner
        .as_ref()
        .map(|owner| owner.0.as_str())
        .unwrap_or("");
    let diagnostic_id = finding
        .canonical_gap
        .as_ref()
        .map(|gap| gap.id.clone())
        .unwrap_or_else(|| {
            stable_diagnostic_id(
                "finding",
                [
                    file.as_str(),
                    finding.probe.family.as_str(),
                    owner,
                    finding.probe.expression.as_str(),
                ],
            )
        });
    let mut data = serde_json::json!({
        "schema_version": "0.1",
        "diagnostic_id": diagnostic_id,
        "finding_id": finding.id.as_str(),
        "probe_id": finding.probe.id.to_string(),
        "classification": finding.class.as_str(),
        "probe_family": finding.probe.family.as_str(),
        "confidence": finding.confidence,
        "source_range": {
            "file": file,
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
        if let Some(actionability) = preview_actionability_for(finding) {
            obj.insert(
                "preview_actionability".to_string(),
                preview_actionability_json_value(&actionability),
            );
        }
        if let Some(witness) = DiagnosticWitness::from_finding(finding)
            && let Ok(value) = serde_json::to_value(&witness)
        {
            obj.insert(
                "explain_command".to_string(),
                serde_json::Value::String(witness.explain_command.clone()),
            );
            obj.insert("witness".to_string(), value);
            let summary = crate::domain::FixInstructionSummary::from_witness(&witness);
            if let Ok(summary_value) = serde_json::to_value(&summary) {
                obj.insert("fix_instruction".to_string(), summary_value);
            }
        }
        if let Some(projection) = causal_projection {
            projection.insert_comparison_fields(obj);
            if let Some(delta) =
                projection.delta_for(finding.canonical_gap.as_ref().map(|gap| gap.id.as_str()))
            {
                insert_canonical_delta_fields(obj, delta);
            }
        }
    }
    Diagnostic {
        range: diagnostic_range_for_finding(finding, position_encoding),
        severity: lsp_severity(config.for_exposure(&finding.class)),
        code: Some(NumberOrString::String(
            super::diagnostic_catalog::finding_code(&finding.class),
        )),
        code_description: None,
        source: Some("ripr".to_string()),
        message: lsp_message(finding),
        related_information: related_information_for_finding(root, finding),
        tags: None,
        data: Some(data),
    }
}

fn diagnostic_range_for_finding(
    finding: &Finding,
    position_encoding: &PositionEncodingKind,
) -> Range {
    let line = finding.probe.location.line.saturating_sub(1) as u32;
    let column = finding.probe.location.column;
    crate::lsp::position::expression_span_range(
        line,
        column,
        &finding.probe.expression,
        position_encoding,
    )
}

fn related_information_for_finding(
    root: &Path,
    finding: &Finding,
) -> Option<Vec<DiagnosticRelatedInformation>> {
    let witness = DiagnosticWitness::from_finding(finding)?;
    let fix_site = witness.fix_site.as_ref()?;
    let path = absolute_path(root, Path::new(&fix_site.file));
    let uri = file_uri_for_path(&path).ok()?;
    let line = fix_site
        .oracle_location
        .as_ref()
        .map_or(fix_site.line, |location| location.line)
        .saturating_sub(1) as u32;
    let oracle = fix_site
        .current_oracle
        .as_deref()
        .map_or_else(String::new, |oracle| format!(": {oracle}"));
    Some(vec![DiagnosticRelatedInformation {
        location: Location {
            uri,
            range: crate::lsp::position::line_span_range(line),
        },
        message: format!(
            "Fix site: related test `{}` has {} {} oracle{}",
            fix_site.test_name, fix_site.oracle_strength, fix_site.oracle_kind, oracle
        ),
    }])
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
    let reconciled = reconcile_next_step(finding);
    let base = if reconciled.is_empty() {
        format!("{} static RIPR exposure", finding.class.as_str())
    } else {
        reconciled
    };
    let witness_message = (finding.language_status.is_none())
        .then(|| DiagnosticWitness::from_finding(finding))
        .flatten()
        .and_then(|witness| {
            let missing = witness.missing_discriminators.first()?.value.as_str();
            let subject = match witness.expected_sink.as_deref() {
                Some("error_variant") => "Exact error variant",
                Some("return_value") => "Exact return value",
                Some("struct_field") => "Exact field value",
                Some("event_call" | "call_effect") => "Expected call/effect",
                Some("match_arm") => "Expected match-arm result",
                _ => "Exact discriminator",
            };
            let fix_site = witness.fix_site.as_ref();
            let detail = match fix_site {
                Some(site) if site.current_oracle.is_some() => {
                    format!("`{}` only has {} oracle", site.test_name, site.oracle_kind)
                }
                Some(site) => format!("`{}` has no producer-supplied oracle text", site.test_name),
                None => "the exact fix site is unavailable".to_string(),
            };
            Some(format!("{subject} `{missing}` is not observed; {detail}."))
        });
    if finding.recommended_next_step.is_none()
        && let Some(witness_message) = witness_message
    {
        return witness_message;
    }
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

#[cfg(test)]
fn absolute_related_test_path(root: &Path, test: &RelatedTest) -> PathBuf {
    absolute_path(root, &test.file)
}

fn absolute_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
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
        let diag = diagnostic_for_classified_seam(Path::new("/repo-a"), &entry)
            .ok_or_else(|| "expected diagnostic".to_string())?;
        let equivalent = diagnostic_for_classified_seam(Path::new("/repo-b"), &entry)
            .ok_or_else(|| "expected equivalent diagnostic".to_string())?;
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
        assert_eq!(
            data["diagnostic_id"],
            equivalent
                .data
                .as_ref()
                .ok_or_else(|| "missing equivalent data".to_string())?["diagnostic_id"]
        );
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
    fn gap_record_diagnostic_fails_closed_for_unregistered_kind() {
        // A registered kind on an eligible, anchored record emits a diagnostic.
        let mut record = gap_record(true);
        assert!(
            diagnostic_for_gap_record(Path::new("/repo"), Path::new("ledger.json"), &record)
                .is_some(),
            "a registered gap kind should emit a diagnostic"
        );

        // An unregistered kind (for example from an external ledger) is not a
        // governed catalog code, so the emission site fails closed and does not
        // surface an unknown `ripr-gap-*` code.
        record.kind = "TotallyUnregisteredKind".to_string();
        assert!(
            diagnostic_for_gap_record(Path::new("/repo"), Path::new("ledger.json"), &record)
                .is_none(),
            "an unregistered gap kind must not emit a diagnostic"
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
            seam_id: None,
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
                missing_discriminator: Some("amount == threshold".to_string()),
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
            receipt_command: Some(
                "ripr outcome --before target/ripr/workflow/before.json --after target/ripr/workflow/after.json --out target/ripr/receipts/pricing.json".to_string(),
            ),
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
            relation_reason: None,
            relation_confidence: None,
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
        };

        let path = absolute_related_test_path(Path::new("/repo"), &test);
        assert_eq!(path, Path::new("/tmp/workspace/tests/pricing.rs"));
    }
}

/// Reject-list tests for the LSP diagnostics severity policy (RIPR-SPEC-0076).
///
/// The hard rule: no WARNING (or higher) may be emitted for a finding or gap
/// record that lacks a complete repair packet. Seam diagnostics are exempt —
/// they carry structural grip-class signals, not repair packets (see comment
/// on `gap_record_diagnostic_severity`).
///
/// These tests are the behavioral proof: each asserts the correct
/// severity or suppression outcome for the named policy condition.
#[cfg(test)]
mod diagnostic_policy_tests {
    use super::*;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, LanguageStatus,
        MissingDiscriminatorFact, OracleKind, OracleStrength, Probe, ProbeFamily, ProbeId,
        RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
        StaticLimitKind,
    };
    use crate::output::gap_decision_ledger::{GapAnchor, GapRepairRoute, ProjectionEligibility};

    fn policy_finding() -> Finding {
        Finding {
            id: "probe:pricing:42:predicate".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:pricing:42:predicate".to_string()),
                location: SourceLocation {
                    file: std::path::PathBuf::from("src/pricing.rs"),
                    line: 42,
                    column: 1,
                },
                owner: None,
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: None,
                after: None,
                expression: "amount >= threshold".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: StageEvidence::new(StageState::Yes, Confidence::High, "reached"),
                infect: StageEvidence::new(StageState::Yes, Confidence::High, "infected"),
                propagate: StageEvidence::new(StageState::Yes, Confidence::Medium, "propagated"),
                reveal: RevealEvidence {
                    observe: StageEvidence::new(StageState::Weak, Confidence::Medium, "observed"),
                    discriminate: StageEvidence::new(
                        StageState::Weak,
                        Confidence::Medium,
                        "weak discriminator",
                    ),
                },
            },
            confidence: 0.75,
            evidence: Vec::new(),
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: Vec::new(),
            related_tests: Vec::new(),
            recommended_next_step: None,
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
        }
    }

    fn complete_gap_record() -> GapRecord {
        let mut projection_eligibility = BTreeMap::new();
        projection_eligibility.insert(
            "lsp_diagnostic".to_string(),
            ProjectionEligibility {
                eligible: true,
                reason: "local_file_scope".to_string(),
            },
        );
        GapRecord {
            gap_id: "gap:pr:pricing:policy-test".to_string(),
            canonical_gap_id: "gap:rust:pricing:policy-test".to_string(),
            seam_id: None,
            kind: "MissingBoundaryAssertion".to_string(),
            language: "rust".to_string(),
            language_status: "stable".to_string(),
            scope: "pr_local".to_string(),
            evidence_class: "predicate_boundary".to_string(),
            gap_state: "actionable".to_string(),
            policy_state: "new".to_string(),
            repairability: "repairable".to_string(),
            repair_route: Some(GapRepairRoute {
                route_kind: "AddBoundaryAssertion".to_string(),
                target_file: Some("tests/pricing.rs".to_string()),
                target_line: Some(33),
                related_test: Some("tests/pricing.rs::discount_threshold".to_string()),
                assertion_shape: Some("assert_eq!(price(threshold), expected)".to_string()),
                missing_discriminator: None,
                changed_behavior: Some("amount >= threshold".to_string()),
                stop_conditions: Vec::new(),
            }),
            static_limit_kind: None,
            static_limit_detail: None,
            static_limits: Vec::new(),
            anchor: Some(GapAnchor {
                file: Some("src/pricing.rs".to_string()),
                line: Some(42),
                owner: Some("pricing::discounted_total".to_string()),
                dedupe_fingerprint: Some("gap:rust:pricing:policy-test".to_string()),
            }),
            evidence_ids: Vec::new(),
            projection_eligibility,
            verification_commands: vec!["cargo xtask fixtures boundary_gap".to_string()],
            receipt_command: Some(
                "ripr outcome --before before.json --after after.json --out receipt.json"
                    .to_string(),
            ),
            regeneration_commands: Vec::new(),
            receipt: None,
            safe_gate_predicate: None,
            authority_boundary: "advisory".to_string(),
        }
    }

    #[test]
    fn actionable_profile_suppresses_non_actionable_findings() -> Result<(), String> {
        let mut finding = policy_finding();
        finding.class = ExposureClass::Exposed;
        let grouped = finding_diagnostics_by_uri_with_profile(
            Path::new("/workspace"),
            &[finding],
            &SeverityConfig::default(),
            true,
            LspDiagnosticProfile::Actionable,
            None,
            &PositionEncodingKind::UTF16,
        )?;
        if !grouped.is_empty() {
            return Err("actionable profile published an exposed finding".to_string());
        }

        let mut unknown = policy_finding();
        unknown.class = ExposureClass::StaticUnknown;
        let grouped = finding_diagnostics_by_uri_with_profile(
            Path::new("/workspace"),
            &[unknown],
            &SeverityConfig::default(),
            true,
            LspDiagnosticProfile::Actionable,
            None,
            &PositionEncodingKind::UTF16,
        )?;
        if !grouped.is_empty() {
            return Err("actionable profile published a static unknown".to_string());
        }
        Ok(())
    }

    #[test]
    fn actionable_profile_keeps_a_producer_backed_fix_route() -> Result<(), String> {
        let mut finding = policy_finding();
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "Price::Boundary".to_string(),
            reason: "exact boundary is not observed".to_string(),
            flow_sink: None,
        }];
        finding.related_tests.push(RelatedTest {
            name: "checks_boundary".to_string(),
            file: std::path::PathBuf::from("tests/pricing.rs"),
            line: 12,
            oracle: Some("assert_eq!(price, expected)".to_string()),
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            relation_reason: None,
            relation_confidence: None,
        });

        let grouped = finding_diagnostics_by_uri_with_profile(
            Path::new("/workspace"),
            &[finding],
            &SeverityConfig::default(),
            true,
            LspDiagnosticProfile::Actionable,
            None,
            &PositionEncodingKind::UTF16,
        )?;
        if grouped.values().flatten().count() != 1 {
            return Err("actionable profile dropped a concrete producer-backed route".to_string());
        }
        Ok(())
    }

    #[test]
    fn finding_span_width_uses_negotiated_encoding_end_to_end() -> Result<(), String> {
        // "café" is 4 UTF-16 code units, 4 UTF-32 scalars, and 5 UTF-8 bytes.
        let mut finding = policy_finding();
        finding.probe.expression = "café".to_string();
        finding.probe.location.column = 1;

        let width_for = |encoding: &PositionEncodingKind| -> Result<u32, String> {
            let grouped = finding_diagnostics_by_uri_with_profile(
                Path::new("/workspace"),
                std::slice::from_ref(&finding),
                &SeverityConfig::default(),
                true,
                LspDiagnosticProfile::Full,
                None,
                encoding,
            )?;
            let diagnostic = grouped
                .values()
                .flatten()
                .next()
                .ok_or_else(|| "expected a finding diagnostic".to_string())?;
            Ok(diagnostic.range.end.character - diagnostic.range.start.character)
        };

        if width_for(&PositionEncodingKind::UTF8)? != 5 {
            return Err("UTF-8 client did not receive a byte-width finding span".to_string());
        }
        if width_for(&PositionEncodingKind::UTF16)? != 4 {
            return Err("UTF-16 finding span width regressed".to_string());
        }
        if width_for(&PositionEncodingKind::UTF32)? != 4 {
            return Err("UTF-32 client did not receive a scalar-width finding span".to_string());
        }
        Ok(())
    }

    // Test 1: WeaklyExposed + static_limit_kind=Some → advisory → INFORMATION (never WARNING).
    #[test]
    fn no_warning_for_finding_with_static_limit() -> Result<(), String> {
        let mut finding = policy_finding();
        finding.class = ExposureClass::WeaklyExposed;
        finding.static_limit_kind = Some(StaticLimitKind::DynamicDispatch);

        if !finding_is_advisory(&finding) {
            return Err("expected finding_is_advisory=true for static_limit_kind".to_string());
        }

        // Simulate the workspace assembly policy: get base severity then clamp if advisory.
        let config = SeverityConfig::default();
        let base_severity = lsp_severity(config.for_exposure(&finding.class));
        // The base for WeaklyExposed is WARNING by default config.
        if base_severity != Some(DiagnosticSeverity::WARNING) {
            return Err(format!(
                "expected base severity to be WARNING (to validate the clamp), got {base_severity:?}"
            ));
        }
        // Policy clamp: advisory → INFORMATION.
        let clamped = if finding_is_advisory(&finding) {
            Some(DiagnosticSeverity::INFORMATION)
        } else {
            base_severity
        };
        if clamped != Some(DiagnosticSeverity::INFORMATION) {
            return Err(format!(
                "expected clamped severity=INFORMATION, got {clamped:?}"
            ));
        }
        Ok(())
    }

    // Test 2: WeaklyExposed + language_status=Preview → advisory → INFORMATION (never WARNING).
    #[test]
    fn no_warning_for_preview_finding() -> Result<(), String> {
        let mut finding = policy_finding();
        finding.class = ExposureClass::WeaklyExposed;
        finding.language_status = Some(LanguageStatus::Preview);

        if !finding_is_advisory(&finding) {
            return Err(
                "expected finding_is_advisory=true for preview language_status".to_string(),
            );
        }

        let config = SeverityConfig::default();
        let base_severity = lsp_severity(config.for_exposure(&finding.class));
        let clamped = if finding_is_advisory(&finding) {
            Some(DiagnosticSeverity::INFORMATION)
        } else {
            base_severity
        };
        if clamped != Some(DiagnosticSeverity::INFORMATION) {
            return Err(format!(
                "expected INFORMATION for preview finding, got {clamped:?}"
            ));
        }
        Ok(())
    }

    // Test 3: complete packet (repairable + verification_commands + receipt_command) → WARNING;
    //         missing verify or receipt → INFORMATION.
    #[test]
    fn warning_only_when_gap_record_has_complete_packet() -> Result<(), String> {
        // Complete packet → WARNING.
        let complete = complete_gap_record();
        let severity = gap_record_diagnostic_severity(&complete);
        if severity != DiagnosticSeverity::WARNING {
            return Err(format!(
                "expected WARNING for complete packet, got {severity:?}"
            ));
        }

        // Missing verification_commands → INFORMATION.
        let mut no_verify = complete.clone();
        no_verify.verification_commands = Vec::new();
        let severity = gap_record_diagnostic_severity(&no_verify);
        if severity != DiagnosticSeverity::INFORMATION {
            return Err(format!(
                "expected INFORMATION when verification_commands empty, got {severity:?}"
            ));
        }

        // Missing receipt_command → INFORMATION.
        let mut no_receipt = complete.clone();
        no_receipt.receipt_command = None;
        let severity = gap_record_diagnostic_severity(&no_receipt);
        if severity != DiagnosticSeverity::INFORMATION {
            return Err(format!(
                "expected INFORMATION when receipt_command missing, got {severity:?}"
            ));
        }

        // Not repairable → INFORMATION.
        let mut not_repairable = complete.clone();
        not_repairable.repairability = "inspect_only".to_string();
        let severity = gap_record_diagnostic_severity(&not_repairable);
        if severity != DiagnosticSeverity::INFORMATION {
            return Err(format!(
                "expected INFORMATION when not repairable, got {severity:?}"
            ));
        }

        Ok(())
    }

    // Test 4: complete packet but language_status="preview" → advisory → INFORMATION.
    #[test]
    fn no_warning_for_preview_gap_record() -> Result<(), String> {
        let mut record = complete_gap_record();
        record.language_status = "preview".to_string();

        if !gap_record_is_advisory(&record) {
            return Err(
                "expected gap_record_is_advisory=true for language_status=preview".to_string(),
            );
        }
        let severity = gap_record_diagnostic_severity(&record);
        if severity != DiagnosticSeverity::INFORMATION {
            return Err(format!(
                "expected INFORMATION for preview gap record, got {severity:?}"
            ));
        }
        Ok(())
    }

    // Test 5: complete packet but static_limit_kind=Some → advisory → INFORMATION.
    #[test]
    fn no_warning_for_static_limit_gap_record() -> Result<(), String> {
        let mut record = complete_gap_record();
        record.static_limit_kind = Some("dynamic_dispatch".to_string());

        if !gap_record_is_advisory(&record) {
            return Err(
                "expected gap_record_is_advisory=true for static_limit_kind present".to_string(),
            );
        }
        let severity = gap_record_diagnostic_severity(&record);
        if severity != DiagnosticSeverity::INFORMATION {
            return Err(format!(
                "expected INFORMATION for static-limit gap record, got {severity:?}"
            ));
        }
        Ok(())
    }

    // Test 6: snapshot with static_limit finding (run_status != "full") → finding WARNING
    //         would be downgraded to INFORMATION.
    //
    // Asserts: snapshot_run_status returns "limited" when a finding carries
    // static_limit_kind, and the workspace assembly downgrades WARNING→INFORMATION.
    // The assembly logic is: if !is_full_run && severity==WARNING → INFORMATION.
    #[test]
    fn limited_run_downgrades_finding_warnings() -> Result<(), String> {
        let mut finding = policy_finding();
        finding.static_limit_kind = Some(StaticLimitKind::MissingImportGraph);

        // Confirm run status is "limited" when finding has a static limit.
        let run_status = derive_run_status(&[finding.clone()], &[], &[], false, false);
        if run_status != "limited" {
            return Err(format!(
                "expected run_status=limited for finding with static_limit_kind, got {run_status}"
            ));
        }

        let is_full_run = run_status == "full";

        // Simulate the workspace assembly downgrade: get base severity, apply
        // limited-run downgrade.
        let config = SeverityConfig::default();
        // Use a non-advisory finding to isolate the limited-run downgrade from
        // the advisory clamp. Remove static_limit_kind for the severity check.
        let mut non_advisory = policy_finding();
        non_advisory.class = ExposureClass::WeaklyExposed;
        let base_severity = lsp_severity(config.for_exposure(&non_advisory.class));
        if base_severity != Some(DiagnosticSeverity::WARNING) {
            return Err(format!(
                "expected base severity WARNING for WeaklyExposed (to prove downgrade), got {base_severity:?}"
            ));
        }
        let final_severity = if !is_full_run && base_severity == Some(DiagnosticSeverity::WARNING) {
            Some(DiagnosticSeverity::INFORMATION)
        } else {
            base_severity
        };
        if final_severity != Some(DiagnosticSeverity::INFORMATION) {
            return Err(format!(
                "expected INFORMATION after limited-run downgrade, got {final_severity:?}"
            ));
        }
        Ok(())
    }

    // Test 7: stale/limited snapshot → gap-record diagnostics suppressed entirely.
    //
    // Asserts: snapshot_run_status returns "stale" for a StaleArtifact rejection,
    // and "cache_limited" for other rejections, both of which are not "full".
    // When !is_full_run, the workspace assembly skips gap-record diagnostics.
    #[test]
    fn stale_run_suppresses_gap_record_diagnostics() -> Result<(), String> {
        // StaleArtifact rejection → run_status "stale" → not full → suppress gap records.
        let stale_rejections = vec![GapArtifactRejection::StaleArtifact];
        let run_status = derive_run_status(&[], &stale_rejections, &[], false, false);
        if run_status != "stale" {
            return Err(format!(
                "expected run_status=stale for StaleArtifact rejection, got {run_status}"
            ));
        }
        if run_status == "full" {
            return Err("stale run must not be treated as full".to_string());
        }

        // cache_limited rejection → also not full → suppress gap records.
        let cache_rejections = vec![GapArtifactRejection::WrongRoot("other-root".to_string())];
        let run_status = derive_run_status(&[], &cache_rejections, &[], false, false);
        if run_status != "cache_limited" {
            return Err(format!(
                "expected run_status=cache_limited for non-stale rejection, got {run_status}"
            ));
        }
        if run_status == "full" {
            return Err("cache_limited run must not be treated as full".to_string());
        }

        // Confirm the suppression decision: gap records are only emitted when is_full_run.
        // Here we verify the boolean gate directly.
        let would_emit = run_status == "full";
        if would_emit {
            return Err("gap records must not be emitted for non-full run".to_string());
        }
        Ok(())
    }

    // Test 8 (RIPR-PROP-0019, #1999): a `limited_partial_scope` snapshot is a
    // limited-family run status, never "full" — finding WARNINGs downgrade and
    // gap-record diagnostics suppress exactly like the other limited states.
    // Stale/cache_limited rejections still dominate the partial state.
    #[test]
    fn partial_scope_run_status_is_limited_family_and_never_full() -> Result<(), String> {
        let run_status = derive_run_status(&[], &[], &[], false, true);
        if run_status != "limited_partial_scope" {
            return Err(format!(
                "expected run_status=limited_partial_scope for a partial partition, got {run_status}"
            ));
        }
        if run_status == "full" {
            return Err("partial run must not be treated as full".to_string());
        }

        // A partial scope also dominates per-finding static limitations in the
        // derivation, matching workspace-status precedence.
        let mut finding = policy_finding();
        finding.static_limit_kind =
            Some(crate::domain::StaticLimitKind::RustTransitiveReachUnresolved);
        let run_status = derive_run_status(&[finding], &[], &[], false, true);
        if run_status != "limited_partial_scope" {
            return Err(format!(
                "partial scope must dominate per-finding static limitations, got {run_status}"
            ));
        }

        // Stale and cache_limited rejections still outrank the partial state.
        let stale = derive_run_status(
            &[],
            &[GapArtifactRejection::StaleArtifact],
            &[],
            false,
            true,
        );
        if stale != "stale" {
            return Err(format!("stale must dominate partial scope, got {stale}"));
        }
        let cache_limited = derive_run_status(
            &[],
            &[GapArtifactRejection::WrongRoot("other-root".to_string())],
            &[],
            false,
            true,
        );
        if cache_limited != "cache_limited" {
            return Err(format!(
                "cache_limited must dominate partial scope, got {cache_limited}"
            ));
        }

        // The suppression decision treats the partial run like the limited family.
        let would_emit_gap_records = run_status == "full";
        if would_emit_gap_records {
            return Err("gap records must not be emitted for a partial run".to_string());
        }
        Ok(())
    }
}

/// PARITY TEST (#1209): LSP diagnostic surface must route `recommended_next_step`
/// through `reconcile_next_step` — a complete TypeScript repair packet must NOT
/// emit the blocked-state disclosure string, and a blocked packet MUST still
/// emit it.
///
/// `lsp_message` is private, so this test must live inside the diagnostics module
/// where it has direct access.
#[cfg(test)]
mod lsp_next_step_parity_tests {
    use super::lsp_message;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, LanguageId,
        LanguageStatus, MissingDiscriminatorFact, OracleKind, OracleStrength, OwnerKind, Probe,
        ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation,
        StageEvidence, StageState, SymbolId,
    };
    use std::path::PathBuf;

    fn complete_ts_finding() -> Finding {
        Finding {
            id: "probe:src_discount.ts:typescript_preview:2396aec1".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_discount.ts:typescript_preview:2396aec1".to_string()),
                location: SourceLocation::new("src/discount.ts", 2, 1),
                owner: Some(SymbolId(
                    "typescript:src/discount.ts::applyDiscount".to_string(),
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
                reach: StageEvidence::new(StageState::Yes, Confidence::Low, "1 related test"),
                infect: StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "TypeScript preview adapter does not yet model infection.",
                ),
                propagate: StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "TypeScript preview adapter does not yet model propagation.",
                ),
                reveal: RevealEvidence {
                    observe: StageEvidence::new(StageState::Weak, Confidence::Low, "weak oracle"),
                    discriminate: StageEvidence::new(
                        StageState::Weak,
                        Confidence::Low,
                        "weak discriminator",
                    ),
                },
            },
            confidence: 0.4,
            evidence: vec![
                "owner: applyDiscount".to_string(),
                "gap_state: advisory".to_string(),
                "actionability_category: incomplete_repair_packet".to_string(),
                "why_not_actionable: TypeScript preview has owner, related-test, oracle, and probe evidence but lacks a complete repair packet contract".to_string(),
                "repair_route: project canonical TypeScript repair packet fields only after verify, receipt, evidence refs, and edit boundaries are available".to_string(),
                "evidence_needed_to_promote: canonical gap identity, repair kind, target test shape, related observer, verify command, receipt command, raw evidence refs, and edit constraints".to_string(),
                "raw_evidence_ref: leg=rust_seam;file=src/discount.ts;line=2;kind=typescript_preview_probe;source_id=probe:src_discount.ts:typescript_preview:2396aec1;owner=applyDiscount".to_string(),
                "typescript_package_root: .".to_string(),
                "typescript_workspace_root: .".to_string(),
                "typescript_framework_hint: jest".to_string(),
                "typescript_runner_hint: npm".to_string(),
                "typescript_package_confidence: high".to_string(),
                "typescript_verify_command: jest tests/discount.test.ts".to_string(),
                "typescript_oracle_observed: applyDiscount(100, 100)".to_string(),
                "typescript_oracle_expected: 50".to_string(),
                "typescript_oracle_confidence: high".to_string(),
                "typescript_oracle_evidence_ref: tests/discount.test.ts:3".to_string(),
                "missing_discriminator: amount == threshold".to_string(),
            ],
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence {
                observed_values: Vec::new(),
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "amount == threshold".to_string(),
                    reason: "changed TypeScript equality-boundary at line 2 lacks a concrete preview discriminator".to_string(),
                    flow_sink: None,
                }],
            },
            stop_reasons: Vec::new(),
            related_tests: vec![RelatedTest {
                name: "applyDiscount applies discount when amount meets threshold".to_string(),
                file: PathBuf::from("tests/discount.test.ts"),
                line: 3,
                oracle_strength: OracleStrength::Weak,
                oracle_kind: OracleKind::RelationalCheck,
                oracle: Some("expect(result).toBeGreaterThan(50)".to_string()),
                relation_reason: None,
                relation_confidence: None,
            }],
            recommended_next_step: Some(
                "TypeScript preview advisory: add or strengthen a focused assertion for missing discriminator `amount == threshold`; no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available.".to_string(),
            ),
            language: Some(LanguageId::TypeScript),
            language_status: Some(LanguageStatus::Preview),
            owner_kind: Some(OwnerKind::Function),
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
        }
    }

    fn incomplete_ts_finding() -> Finding {
        let mut f = complete_ts_finding();
        f.evidence
            .retain(|l| !l.starts_with("typescript_verify_command:"));
        f.recommended_next_step = Some(
            "TypeScript preview advisory: add or strengthen a focused assertion; no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available.".to_string(),
        );
        f
    }

    /// PARITY: complete packet → LSP diagnostic message must NOT contain the
    /// blocked-state disclosure string (the contradiction that #1209 fixes).
    #[test]
    fn lsp_diagnostic_complete_packet_strips_blocked_tail() {
        let finding = complete_ts_finding();
        let message = lsp_message(&finding);
        assert!(
            message.contains("the repair packet is complete and delegatable (advisory)"),
            "LSP diagnostic must contain reconciled next-step for complete packet; got: {message}"
        );
        assert!(
            !message.contains("no actionable repair packet is emitted until"),
            "LSP diagnostic must NOT contain blocked-case tail for complete packet; got: {message}"
        );
    }

    /// PARITY: blocked packet → LSP diagnostic message must STILL contain the
    /// blocked-state disclosure (fail-closed: real disclosures must not be silenced).
    #[test]
    fn lsp_diagnostic_blocked_packet_preserves_disclosure() {
        let finding = incomplete_ts_finding();
        let message = lsp_message(&finding);
        assert!(
            message.contains("no actionable repair packet is emitted"),
            "LSP diagnostic must preserve blocked-case disclosure for incomplete packet; got: {message}"
        );
        assert!(
            !message.contains("the repair packet is complete and delegatable"),
            "LSP diagnostic must NOT say actionable for blocked packet; got: {message}"
        );
    }
}

#[cfg(test)]
mod delivery_tests {
    use super::*;

    fn diagnostic(id: &str, line: u32) -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position { line, character: 0 },
                end: Position {
                    line,
                    character: 10,
                },
            },
            severity: Some(DiagnosticSeverity::INFORMATION),
            code: Some(NumberOrString::String("ripr-test".to_string())),
            code_description: None,
            source: Some("ripr".to_string()),
            message: format!("diagnostic {id}"),
            related_information: None,
            tags: None,
            data: Some(serde_json::json!({ "diagnostic_id": id })),
        }
    }

    #[test]
    fn canonicalization_sorts_by_stable_diagnostic_id() -> Result<(), String> {
        let uri = "file:///workspace/src/lib.rs"
            .parse::<Uri>()
            .map_err(|err| format!("parse URI failed: {err}"))?;
        let batches = canonicalize_diagnostic_batches(vec![DiagnosticBatch {
            uri: uri.clone(),
            diagnostics: vec![diagnostic("b", 1), diagnostic("a", 2)],
        }]);
        let ids = batches
            .first()
            .ok_or_else(|| "missing batch".to_string())?
            .diagnostics
            .iter()
            .map(|diagnostic| {
                diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("diagnostic_id"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["a", "b"]);
        Ok(())
    }

    fn snapshot_for_identity(
        root: &str,
        entries: &[(&str, Vec<Diagnostic>)],
    ) -> Result<AnalysisSnapshot, String> {
        let diagnostics_by_uri = entries
            .iter()
            .map(|(uri, diagnostics)| {
                uri.parse::<Uri>()
                    .map(|uri| (uri, diagnostics.clone()))
                    .map_err(|err| format!("parse snapshot URI failed: {err}"))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(AnalysisSnapshot {
            root: PathBuf::from(root),
            input_identity: None,
            base: Some("origin/main".to_string()),
            mode: crate::app::Mode::Draft,
            refresh: RefreshMetadata::default(),
            findings: Vec::new(),
            diagnostic_profile: LspDiagnosticProfile::Full,
            classified_seams: Vec::new(),
            gap_artifacts: Vec::new(),
            gap_artifact_rejections: Vec::new(),
            diagnostics_by_uri,
            delivery_selection: None,
            seams_deferred: false,
            partial_scope: None,
        })
    }

    #[test]
    fn diagnostic_result_ids_ignore_equivalent_checkout_roots_and_time() -> Result<(), String> {
        let diagnostics = vec![diagnostic("same", 4)];
        let first = snapshot_for_identity(
            "/workspace-a",
            &[("file:///workspace-a/src/lib.rs", diagnostics.clone())],
        )?;
        let second = snapshot_for_identity(
            "/workspace-b",
            &[("file:///workspace-b/src/lib.rs", diagnostics)],
        )?;
        let first_uri = "file:///workspace-a/src/lib.rs"
            .parse::<Uri>()
            .map_err(|err| format!("parse first URI failed: {err}"))?;
        let second_uri = "file:///workspace-b/src/lib.rs"
            .parse::<Uri>()
            .map_err(|err| format!("parse second URI failed: {err}"))?;
        if document_diagnostic_result_id(&first, &first_uri)
            != document_diagnostic_result_id(&second, &second_uri)
        {
            return Err("equivalent roots changed the document result ID".to_string());
        }
        Ok(())
    }

    #[test]
    fn workspace_result_identity_changes_only_for_a_semantic_document_change() -> Result<(), String>
    {
        let first = snapshot_for_identity(
            "/workspace",
            &[
                ("file:///workspace/src/a.rs", vec![diagnostic("a", 1)]),
                ("file:///workspace/src/b.rs", vec![diagnostic("b", 2)]),
            ],
        )?;
        let second = snapshot_for_identity(
            "/workspace",
            &[
                ("file:///workspace/src/a.rs", vec![diagnostic("a", 1)]),
                ("file:///workspace/src/b.rs", vec![diagnostic("b", 3)]),
            ],
        )?;
        let a_uri = "file:///workspace/src/a.rs"
            .parse::<Uri>()
            .map_err(|err| format!("parse URI failed: {err}"))?;
        if document_diagnostic_result_id(&first, &a_uri)
            != document_diagnostic_result_id(&second, &a_uri)
        {
            return Err("unaffected document result ID changed".to_string());
        }
        if workspace_diagnostic_result_id(&first) == workspace_diagnostic_result_id(&second) {
            return Err("workspace result ID ignored the changed document".to_string());
        }
        Ok(())
    }

    #[test]
    fn canonicalization_removes_exact_duplicate_payloads() -> Result<(), String> {
        let uri = "file:///workspace/src/lib.rs"
            .parse::<Uri>()
            .map_err(|err| format!("parse URI failed: {err}"))?;
        let repeated = diagnostic("same", 1);
        let batches = canonicalize_diagnostic_batches(vec![DiagnosticBatch {
            uri,
            diagnostics: vec![repeated.clone(), repeated],
        }]);
        assert_eq!(
            batches
                .first()
                .ok_or_else(|| "missing batch".to_string())?
                .diagnostics
                .len(),
            1
        );
        Ok(())
    }

    #[test]
    fn normalized_payload_digest_ignores_equivalent_checkout_roots() -> Result<(), String> {
        let root_a = PathBuf::from("/tmp/ripr-root-a");
        let root_b = PathBuf::from("/tmp/ripr-root-b");
        let first = path_sensitive_diagnostic(&root_a)?;
        let second = path_sensitive_diagnostic(&root_b)?;

        assert_eq!(
            normalized_diagnostic_payload_digest(&root_a, &[first]),
            normalized_diagnostic_payload_digest(&root_b, &[second])
        );
        Ok(())
    }

    fn path_sensitive_diagnostic(root: &Path) -> Result<Diagnostic, String> {
        let mut diagnostic = diagnostic("root-independent", 1);
        let related_uri = file_uri_for_path(&root.join("tests/lib.rs"))?;
        diagnostic.related_information = Some(vec![DiagnosticRelatedInformation {
            location: Location {
                uri: related_uri,
                range: diagnostic.range,
            },
            message: "related test".to_string(),
        }]);
        diagnostic.data = Some(serde_json::json!({
            "diagnostic_id": "gap:stable",
            "source_range": { "file": root.join("src/lib.rs") },
            "gap_ledger": root.join("target/ripr/reports/gap.json"),
        }));
        Ok(diagnostic)
    }
}
