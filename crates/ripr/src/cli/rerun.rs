#[cfg(feature = "lang-typescript")]
use crate::analysis::targeted_typescript_findings_for_scope;
use crate::analysis::{
    canonical_gap::canonical_gap_identity,
    inventory_changed_test_classified_seams_at_with_config_node,
    inventory_classified_seams_at_with_config,
    inventory_diff_scoped_classified_seams_at_with_config, seam_cache::stable_input_hash,
    workspace_cache_key_at_with_config,
};
use crate::app::repair_route_readiness;
use crate::cli::commands_context::{ensure_command_root, load_root_input_and_config};
use crate::cli::help;
use crate::cli::parse::expect_value;
use crate::output::gap_decision_ledger::{GapRecord, parse_gap_record_source_json};
use crate::output::outcome::{TargetedRerunStaticSeam, targeted_rerun_movement_from_json};
#[cfg(feature = "lang-typescript")]
use crate::output::typescript_packet_projection::typescript_canonical_gap_id;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
struct RerunOptions {
    root: PathBuf,
    selector: RerunSelector,
    json: bool,
    out: Option<PathBuf>,
    before: Option<PathBuf>,
    check_parity: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum RerunSelector {
    ChangedTest {
        file: PathBuf,
        node: Option<String>,
    },
    Gap {
        canonical_gap_id: String,
        gap_ledger: PathBuf,
    },
}

#[derive(Serialize)]
struct TargetedRerunReport {
    schema_version: &'static str,
    state: &'static str,
    selector: TargetedRerunSelector,
    cache: TargetedRerunCache,
    seams: Vec<TargetedRerunSeam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    movement: Option<TargetedRerunMovement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parity: Option<TargetedRerunParity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    route: Option<TargetedRerunRoute>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limitation: Option<TargetedRerunLimitation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    scope_limitations: Vec<TargetedRerunScopeLimitation>,
    authority_boundary: &'static str,
    #[serde(skip)]
    parity_scope: Option<ResolvedRerunScope>,
}

#[derive(Serialize)]
struct TargetedRerunSelector {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    changed_test: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical_gap_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gap_ledger: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    matched_record_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recomputed_scope_count: Option<usize>,
    selected_test_count: usize,
    direct_call_names: Vec<String>,
}

#[derive(Debug, Serialize)]
struct TargetedRerunCache {
    schema_version: &'static str,
    reuse_state: &'static str,
    file_fact_status: String,
    hits: usize,
    misses: usize,
    corrupt_ignored: usize,
    stores: usize,
    store_errors: usize,
    recomputation_reasons: Vec<String>,
    invalidation_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_fingerprint: Option<TargetedRerunInputFingerprint>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
struct TargetedRerunInputFingerprint {
    schema_version: String,
    analyzer_version: String,
    workspace_root_hash: String,
    files_content_hash: String,
    cfg_features_hash: String,
    config_hash: String,
    test_intent_hash: String,
    suppressions_hash: String,
    workspace_manifests_hash: String,
    lockfile_hash: String,
    toolchain_hash: String,
    seam_limit_key: String,
    selector_ledger_hash: String,
    #[serde(default)]
    graph_provenance: TargetedRerunGraphProvenance,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
struct TargetedRerunGraphProvenance {
    package_graph_status: String,
    package_graph_hash: Option<String>,
    package_graph_detail: Option<String>,
    feature_graph_status: String,
    feature_graph_hash: Option<String>,
    feature_graph_detail: Option<String>,
    external_dependency_graph_status: String,
    external_dependency_graph_detail: String,
}

impl From<&crate::analysis::seam_cache::RepoSeamCacheKey> for TargetedRerunInputFingerprint {
    fn from(key: &crate::analysis::seam_cache::RepoSeamCacheKey) -> Self {
        Self {
            schema_version: key.schema_version.clone(),
            analyzer_version: key.analyzer_version.clone(),
            workspace_root_hash: key.workspace_root_hash.clone(),
            files_content_hash: key.files_content_hash.clone(),
            cfg_features_hash: key.cfg_features_hash.clone(),
            config_hash: key.config_hash.clone(),
            test_intent_hash: key.test_intent_hash.clone(),
            suppressions_hash: key.suppressions_hash.clone(),
            workspace_manifests_hash: key.workspace_manifests_hash.clone(),
            lockfile_hash: key.lockfile_hash.clone(),
            toolchain_hash: key.toolchain_hash.clone(),
            seam_limit_key: key.seam_limit_key.clone(),
            selector_ledger_hash: "not_applicable".to_string(),
            graph_provenance: TargetedRerunGraphProvenance::default(),
        }
    }
}

impl From<&crate::analysis::seam_cache::WorkspaceGraphProvenance> for TargetedRerunGraphProvenance {
    fn from(provenance: &crate::analysis::seam_cache::WorkspaceGraphProvenance) -> Self {
        Self {
            package_graph_status: provenance.package_graph_status.clone(),
            package_graph_hash: provenance.package_graph_hash.clone(),
            package_graph_detail: provenance.package_graph_detail.clone(),
            feature_graph_status: provenance.feature_graph_status.clone(),
            feature_graph_hash: provenance.feature_graph_hash.clone(),
            feature_graph_detail: provenance.feature_graph_detail.clone(),
            external_dependency_graph_status: provenance.external_dependency_graph_status.clone(),
            external_dependency_graph_detail: provenance.external_dependency_graph_detail.clone(),
        }
    }
}

fn input_fingerprint_for(
    root: &Path,
    key: &crate::analysis::seam_cache::RepoSeamCacheKey,
) -> TargetedRerunInputFingerprint {
    let mut fingerprint: TargetedRerunInputFingerprint = key.into();
    fingerprint.graph_provenance =
        (&crate::analysis::seam_cache::workspace_graph_provenance(root)).into();
    fingerprint
}

#[derive(Clone, Serialize)]
struct TargetedRerunSeam {
    canonical_gap_id: Option<String>,
    seam_id: String,
    file: String,
    line: usize,
    owner: String,
    static_class: String,
    repair_route_readiness: crate::analysis::repair_route::RepairRouteReadiness,
    related_tests: Vec<TargetedRerunRelatedTest>,
    missing_discriminators: Vec<TargetedRerunMissingDiscriminator>,
}

#[derive(Clone, Serialize, PartialEq, Eq)]
struct TargetedRerunRelatedTest {
    test_name: String,
    file: String,
    line: usize,
    oracle_kind: String,
    oracle_strength: String,
    evidence_summary: String,
    relation_reason: String,
    relation_confidence: String,
}

#[derive(Clone, Serialize, PartialEq, Eq)]
struct TargetedRerunMissingDiscriminator {
    value: String,
    reason: String,
}

#[derive(Serialize)]
struct TargetedRerunMovement {
    state: &'static str,
    before: String,
    before_seam_count: usize,
    matched_seam_count: usize,
}

#[derive(Serialize)]
struct TargetedRerunParity {
    state: &'static str,
    basis: &'static str,
    targeted_seam_count: usize,
    full_selected_seam_count: usize,
    selected_seam_count: usize,
    matched_seam_count: usize,
    missing_from_targeted: Vec<String>,
    unexpected_in_targeted: Vec<String>,
    differing: Vec<TargetedRerunParityMismatch>,
    mismatches: Vec<TargetedRerunParityMismatch>,
    input_mismatches: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
struct TargetedRerunParityMismatch {
    seam_id: String,
    fields: Vec<&'static str>,
}

#[derive(Serialize)]
struct TargetedRerunRoute {
    verify_commands: Vec<String>,
    receipt_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    receipt_command_conflict: Option<TargetedRerunLimitation>,
}

#[derive(Serialize)]
struct TargetedRerunLimitation {
    kind: &'static str,
    message: String,
}

#[derive(Serialize)]
struct TargetedRerunScopeLimitation {
    kind: &'static str,
    record_index: usize,
    message: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct GapRerunScope {
    file: PathBuf,
    owner: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct ResolvedRerunScope {
    canonical_gap_id: Option<String>,
    explicit_seam_ids: BTreeSet<String>,
    files: BTreeSet<PathBuf>,
    owners: BTreeSet<String>,
    changed_test: Option<PathBuf>,
    direct_call_names: BTreeSet<String>,
}

pub(super) fn run(args: &[String]) -> Result<(), String> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        help::print_rerun_help();
        return Ok(());
    }
    let options = parse_options(args)?;
    ensure_command_root(&options.root, "rerun")?;
    let (input, config) = load_root_input_and_config(&options.root)?;
    let mut report = match options.selector {
        RerunSelector::ChangedTest { file, node } => {
            rerun_changed_test(&input.root, &config, &file, node.as_deref())?
        }
        RerunSelector::Gap {
            canonical_gap_id,
            gap_ledger,
        } => rerun_gap(&input.root, &config, &canonical_gap_id, &gap_ledger)?,
    };
    if options.check_parity {
        apply_full_pipeline_parity(&mut report, &input.root, &config)?;
    }
    if let Some(before) = options.before.as_deref() {
        apply_before(&mut report, before);
    }
    let rendered = if options.json {
        serde_json::to_string_pretty(&report)
            .map_err(|err| format!("serialize targeted rerun report failed: {err}"))?
    } else {
        render_human(&report)
    };
    if let Some(out) = options.out {
        let out = resolve_output_path(&input.root, &out);
        write_text_file(&out, &rendered)?;
        println!("Wrote {}", out.display());
    } else {
        println!("{rendered}");
    }
    Ok(())
}

fn parse_options(args: &[String]) -> Result<RerunOptions, String> {
    let mut root = PathBuf::from(".");
    let mut changed_test = None;
    let mut canonical_gap_id = None;
    let mut gap_ledger = None;
    let mut json = false;
    let mut out = None;
    let mut before = None;
    let mut check_parity = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                root = PathBuf::from(expect_value(args, index, "--root")?);
            }
            "--changed-test" => {
                index += 1;
                if changed_test.is_some() || canonical_gap_id.is_some() {
                    return Err("rerun accepts exactly one selector".to_string());
                }
                changed_test = Some(expect_value(args, index, "--changed-test")?.to_string());
            }
            "--gap" => {
                index += 1;
                if changed_test.is_some() || canonical_gap_id.is_some() {
                    return Err("rerun accepts exactly one selector".to_string());
                }
                canonical_gap_id = Some(expect_value(args, index, "--gap")?.to_string());
            }
            "--gap-ledger" => {
                index += 1;
                gap_ledger = Some(PathBuf::from(expect_value(args, index, "--gap-ledger")?));
            }
            "--json" => json = true,
            "--out" => {
                index += 1;
                out = Some(PathBuf::from(expect_value(args, index, "--out")?));
            }
            "--before" => {
                index += 1;
                before = Some(PathBuf::from(expect_value(args, index, "--before")?));
            }
            "--check-parity" => check_parity = true,
            other => return Err(format!("unknown rerun argument {other:?}")),
        }
        index += 1;
    }
    let selector = match (changed_test, canonical_gap_id, gap_ledger) {
        (Some(changed_test), None, None) => {
            let (file, node) = parse_changed_test_selector(&changed_test)?;
            RerunSelector::ChangedTest { file, node }
        }
        (Some(_), None, Some(_)) => {
            return Err("rerun --gap-ledger requires --gap".to_string());
        }
        (None, Some(canonical_gap_id), Some(gap_ledger)) => RerunSelector::Gap {
            canonical_gap_id,
            gap_ledger,
        },
        (None, Some(_), None) => {
            return Err("rerun --gap requires --gap-ledger <path>".to_string());
        }
        (None, None, Some(_)) => {
            return Err("rerun --gap-ledger requires --gap".to_string());
        }
        (None, None, None) => {
            return Err("rerun requires --changed-test <path> or --gap <canonical-gap-id> --gap-ledger <path>".to_string());
        }
        (Some(_), Some(_), _) => return Err("rerun accepts exactly one selector".to_string()),
    };
    Ok(RerunOptions {
        root,
        selector,
        json,
        out,
        before,
        check_parity,
    })
}

fn rerun_changed_test(
    root: &Path,
    config: &crate::config::RiprConfig,
    changed_test: &Path,
    test_node: Option<&str>,
) -> Result<TargetedRerunReport, String> {
    let changed_test = normalize_changed_test(root, changed_test)?;
    let inventory = inventory_changed_test_classified_seams_at_with_config_node(
        root,
        config,
        &changed_test,
        test_node,
    )?;
    let direct_call_names = inventory.direct_call_names.clone();
    let mut report = report(
        "current_state_only",
        TargetedRerunSelector {
            kind: "changed_test",
            changed_test: Some(match test_node {
                Some(test_node) => format!("{}::{test_node}", display_path(&changed_test)),
                None => display_path(&changed_test),
            }),
            canonical_gap_id: None,
            gap_ledger: None,
            matched_record_count: None,
            recomputed_scope_count: None,
            selected_test_count: inventory.selected_test_count,
            direct_call_names: inventory.direct_call_names,
        },
        cache_from(
            &inventory.file_fact_cache,
            ["selected_test_scope_recomputed"],
        ),
        seams_from(&inventory.classified),
        None,
        None,
        Vec::new(),
    );
    report.cache.input_fingerprint =
        Some(input_fingerprint_for(root, &inventory.workspace_cache_key));
    report.parity_scope = Some(ResolvedRerunScope {
        files: inventory
            .classified
            .iter()
            .map(|entry| entry.seam.file().to_path_buf())
            .collect(),
        owners: inventory
            .classified
            .iter()
            .map(|entry| entry.seam.owner().to_string())
            .collect(),
        changed_test: Some(changed_test),
        direct_call_names: direct_call_names.iter().cloned().collect(),
        ..ResolvedRerunScope::default()
    });
    Ok(report)
}

fn rerun_gap(
    root: &Path,
    config: &crate::config::RiprConfig,
    canonical_gap_id: &str,
    gap_ledger: &Path,
) -> Result<TargetedRerunReport, String> {
    // The ledger is an explicit input artifact. Like other CLI inputs, a
    // relative path is resolved from the invocation directory, not from
    // `--root`; only `--out` is rooted under the selected workspace.
    let gap_ledger = gap_ledger.to_path_buf();
    let mut selector = TargetedRerunSelector {
        kind: "canonical_gap",
        changed_test: None,
        canonical_gap_id: Some(canonical_gap_id.to_string()),
        gap_ledger: Some(display_path(&gap_ledger)),
        matched_record_count: Some(0),
        recomputed_scope_count: Some(0),
        selected_test_count: 0,
        direct_call_names: Vec::new(),
    };
    let contents = match std::fs::read_to_string(&gap_ledger) {
        Ok(contents) => contents,
        Err(err) => {
            return Ok(limited_report(
                selector,
                "canonical_gap_unresolved",
                format!("read gap ledger {} failed: {err}", gap_ledger.display()),
            ));
        }
    };
    let source = match parse_gap_record_source_json(&contents) {
        Ok(source) => source,
        Err(err) => {
            return Ok(limited_report(
                selector,
                "canonical_gap_unresolved",
                format!("parse gap ledger {} failed: {err}", gap_ledger.display()),
            ));
        }
    };
    if let Some(ledger_root) = source
        .root
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        && !same_root(root, Path::new(ledger_root))
    {
        return Ok(limited_report(
            selector,
            "stale_gap_ledger",
            format!(
                "gap ledger root `{ledger_root}` does not match rerun root `{}`",
                root.display()
            ),
        ));
    }
    let records = match resolve_gap_records(source.records, canonical_gap_id) {
        Ok(records) => records,
        Err(limitation) => {
            return Ok(limited_report(
                selector,
                limitation.kind,
                limitation.message,
            ));
        }
    };
    selector.matched_record_count = Some(records.len());
    let (scopes, mut scope_limitations) = scopes_from_gap_records(&records);
    selector.recomputed_scope_count = Some(scopes.len());
    if scopes.is_empty() {
        return Ok(limited_report_with_scope_limitations(
            selector,
            "canonical_gap_unresolved",
            format!("no anchored scope is available for canonical gap `{canonical_gap_id}`"),
            scope_limitations,
        ));
    }

    let mut cache = crate::analysis::seam_cache::FileFactCacheStats::default();
    let mut input_fingerprint = None;
    let mut seams = BTreeMap::<String, TargetedRerunSeam>::new();
    for scope in scopes {
        #[cfg(feature = "lang-typescript")]
        if is_typescript_scope(&scope) {
            let current = targeted_typescript_findings_for_scope(
                root,
                config,
                &scope.file,
                anchor_line_for_scope(&records, &scope),
            )?;
            let scope_seams = current
                .iter()
                .filter(|finding| {
                    typescript_canonical_gap_id(&finding.id) == canonical_gap_id
                        || record_evidence_ids_for_scope(&records, &scope).contains(&finding.id)
                })
                .collect::<Vec<_>>();
            if scope_seams.is_empty() {
                push_scope_unresolved(&mut scope_limitations, &records, &scope, canonical_gap_id);
                continue;
            }
            for finding in scope_seams {
                let seam = typescript_seam_from_finding(finding, canonical_gap_id);
                seams.entry(seam.seam_id.clone()).or_insert(seam);
            }
            continue;
        }

        let changed_files = vec![scope.file.clone()];
        let changed_owner_names = scope.owner.iter().cloned().collect::<Vec<_>>();
        let inventory = inventory_diff_scoped_classified_seams_at_with_config(
            root,
            config,
            &changed_files,
            &changed_owner_names,
        )?;
        input_fingerprint
            .get_or_insert_with(|| input_fingerprint_for(root, &inventory.workspace_cache_key));
        add_cache_stats(&mut cache, &inventory.file_fact_cache);
        let scoped_record_seam_ids = seam_ids_for_scope(&records, &scope);
        let scope_seams = inventory
            .classified
            .iter()
            .filter(|entry| {
                entry_matches_selected_gap(
                    canonical_gap_identity(entry).map(|identity| identity.id),
                    entry.seam.id().as_str(),
                    canonical_gap_id,
                    &scoped_record_seam_ids,
                )
            })
            .collect::<Vec<_>>();
        if scope_seams.is_empty() {
            push_scope_unresolved(&mut scope_limitations, &records, &scope, canonical_gap_id);
            continue;
        }
        for entry in scope_seams {
            let seam = seam_from_with_selected_gap(entry, canonical_gap_id);
            seams.entry(seam.seam_id.clone()).or_insert(seam);
        }
    }
    if seams.is_empty() {
        return Ok(limited_report_with_scope_limitations(
            selector,
            "canonical_gap_unresolved",
            format!(
                "no current seam matched canonical gap `{canonical_gap_id}` from the supplied ledger"
            ),
            scope_limitations,
        ));
    }
    let mut report = report(
        "current_state_only",
        selector,
        cache_from(&cache, ["selected_gap_scopes_recomputed"]),
        seams.into_values().collect(),
        Some(route_from_gap_records(&records)),
        None,
        scope_limitations,
    );
    report.cache.input_fingerprint = input_fingerprint;
    if let Some(fingerprint) = report.cache.input_fingerprint.as_mut() {
        fingerprint.selector_ledger_hash = stable_input_hash(contents.as_bytes());
    }
    report.parity_scope = Some(ResolvedRerunScope {
        canonical_gap_id: Some(canonical_gap_id.to_string()),
        explicit_seam_ids: records
            .iter()
            .filter_map(|(_, record)| record.seam_id.clone())
            .collect(),
        files: scopes_from_gap_records(&records)
            .0
            .into_iter()
            .map(|scope| scope.file)
            .collect(),
        owners: records
            .iter()
            .filter_map(|(_, record)| record.anchor.as_ref()?.owner.clone())
            .collect(),
        ..ResolvedRerunScope::default()
    });
    Ok(report)
}

fn push_scope_unresolved(
    scope_limitations: &mut Vec<TargetedRerunScopeLimitation>,
    records: &[(usize, GapRecord)],
    scope: &GapRerunScope,
    canonical_gap_id: &str,
) {
    for record_index in record_indexes_for_scope(records, scope) {
        scope_limitations.push(TargetedRerunScopeLimitation {
            kind: "gap_scope_unresolved",
            record_index,
            message: format!(
                "anchored scope {} no longer contains canonical gap `{canonical_gap_id}`",
                display_scope(scope)
            ),
        });
    }
}

#[cfg(feature = "lang-typescript")]
fn is_typescript_scope(scope: &GapRerunScope) -> bool {
    matches!(
        scope.file.extension().and_then(|ext| ext.to_str()),
        Some("ts" | "tsx" | "js" | "jsx")
    )
}

#[cfg(feature = "lang-typescript")]
fn anchor_line_for_scope(records: &[(usize, GapRecord)], scope: &GapRerunScope) -> Option<u64> {
    records.iter().find_map(|(_, record)| {
        let anchor = record.anchor.as_ref()?;
        let file = anchor.file.as_deref()?.trim();
        if Path::new(file) == scope.file {
            anchor.line
        } else {
            None
        }
    })
}

#[cfg(feature = "lang-typescript")]
fn record_evidence_ids_for_scope(
    records: &[(usize, GapRecord)],
    scope: &GapRerunScope,
) -> BTreeSet<String> {
    records
        .iter()
        .filter(|(_, record)| {
            record
                .anchor
                .as_ref()
                .and_then(|anchor| anchor.file.as_deref())
                .is_some_and(|file| Path::new(file.trim()) == scope.file)
        })
        .flat_map(|(_, record)| record.evidence_ids.iter().cloned())
        .collect()
}

#[cfg(feature = "lang-typescript")]
fn typescript_seam_from_finding(
    finding: &crate::domain::Finding,
    canonical_gap_id: &str,
) -> TargetedRerunSeam {
    TargetedRerunSeam {
        canonical_gap_id: Some(canonical_gap_id.to_string()),
        seam_id: finding.id.clone(),
        file: display_path(&finding.probe.location.file),
        line: finding.probe.location.line,
        owner: finding
            .probe
            .owner
            .as_ref()
            .map(|owner| owner.0.clone())
            .unwrap_or_else(|| "typescript:<unknown>".to_string()),
        static_class: finding.class.as_str().to_string(),
        // Preview-language reruns are advisory before/after comparisons, never
        // repair-ready: fail closed into StaticLimitation with the adapter's
        // own evidence split rather than fabricating readiness.
        repair_route_readiness: crate::analysis::repair_route::RepairRouteReadiness {
            state: crate::analysis::repair_route::RepairRouteState::StaticLimitation,
            seam_id: finding.id.clone(),
            canonical_gap_id: Some(canonical_gap_id.to_string()),
            required_evidence: finding
                .evidence
                .iter()
                .chain(finding.missing.iter())
                .cloned()
                .collect(),
            present_evidence: finding.evidence.clone(),
            missing_evidence: finding.missing.clone(),
            target_selection: crate::analysis::repair_route::RepairTargetSelection::Missing,
            test_target: None,
            proposed_oracle: None,
            current_oracle: None,
            authority_boundary: crate::analysis::repair_route::REPAIR_ROUTE_AUTHORITY_BOUNDARY,
        },
        related_tests: finding
            .related_tests
            .iter()
            .map(|test| TargetedRerunRelatedTest {
                test_name: test.name.clone(),
                file: display_path(&test.file),
                line: test.line,
                oracle_kind: test.oracle_kind.as_str().to_string(),
                oracle_strength: test.oracle_strength.as_str().to_string(),
                evidence_summary: test.oracle.clone().unwrap_or_default(),
                relation_reason: test
                    .relation_reason
                    .map(|reason| reason.as_str().to_string())
                    .unwrap_or_default(),
                relation_confidence: test
                    .relation_confidence
                    .map(|confidence| confidence.as_str().to_string())
                    .unwrap_or_default(),
            })
            .collect(),
        missing_discriminators: finding
            .missing
            .iter()
            .map(|value| TargetedRerunMissingDiscriminator {
                value: value.clone(),
                reason: value.clone(),
            })
            .collect(),
    }
}

fn entry_matches_selected_gap(
    current_canonical_gap_id: Option<String>,
    seam_id: &str,
    selected_canonical_gap_id: &str,
    selected_seam_ids: &BTreeSet<String>,
) -> bool {
    current_canonical_gap_id.as_deref() == Some(selected_canonical_gap_id)
        || selected_seam_ids.contains(seam_id)
}

fn seam_ids_for_scope(records: &[(usize, GapRecord)], scope: &GapRerunScope) -> BTreeSet<String> {
    records
        .iter()
        .filter_map(|(_, record)| {
            let anchor = record.anchor.as_ref()?;
            let file = anchor.file.as_deref()?.trim();
            if file.is_empty() {
                return None;
            }
            let owner = anchor
                .owner
                .as_deref()
                .map(str::trim)
                .filter(|owner| !owner.is_empty())
                .map(str::to_string);
            (GapRerunScope {
                file: PathBuf::from(file),
                owner,
            } == *scope)
                .then(|| record.seam_id.clone())
                .flatten()
        })
        .collect()
}

fn resolve_gap_records(
    records: Vec<GapRecord>,
    canonical_gap_id: &str,
) -> Result<Vec<(usize, GapRecord)>, TargetedRerunLimitation> {
    let matches = records
        .into_iter()
        .enumerate()
        .filter(|(_, record)| record.canonical_gap_id == canonical_gap_id)
        .collect::<Vec<_>>();
    if matches.is_empty() {
        Err(TargetedRerunLimitation {
            kind: "canonical_gap_unresolved",
            message: format!("no gap ledger record has canonical_gap_id `{canonical_gap_id}`"),
        })
    } else {
        Ok(matches)
    }
}

fn scopes_from_gap_records(
    records: &[(usize, GapRecord)],
) -> (BTreeSet<GapRerunScope>, Vec<TargetedRerunScopeLimitation>) {
    let mut scopes = BTreeSet::new();
    let mut limitations = Vec::new();
    for (record_index, record) in records {
        let Some(anchor) = record.anchor.as_ref() else {
            limitations.push(TargetedRerunScopeLimitation {
                kind: "gap_anchor_unresolved",
                record_index: *record_index,
                message: "gap ledger record has no source anchor".to_string(),
            });
            continue;
        };
        let Some(file) = anchor
            .file
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            limitations.push(TargetedRerunScopeLimitation {
                kind: "gap_anchor_unresolved",
                record_index: *record_index,
                message: "gap ledger record has no anchored file".to_string(),
            });
            continue;
        };
        scopes.insert(GapRerunScope {
            file: PathBuf::from(file),
            owner: anchor
                .owner
                .as_deref()
                .map(str::trim)
                .filter(|owner| !owner.is_empty())
                .map(str::to_string),
        });
    }
    (scopes, limitations)
}

fn record_indexes_for_scope(records: &[(usize, GapRecord)], scope: &GapRerunScope) -> Vec<usize> {
    records
        .iter()
        .filter_map(|(record_index, record)| {
            let anchor = record.anchor.as_ref()?;
            let file = anchor.file.as_deref()?.trim();
            if file.is_empty() {
                return None;
            }
            let owner = anchor
                .owner
                .as_deref()
                .map(str::trim)
                .filter(|owner| !owner.is_empty())
                .map(str::to_string);
            (GapRerunScope {
                file: PathBuf::from(file),
                owner,
            } == *scope)
                .then_some(*record_index)
        })
        .collect()
}

fn route_from_gap_records(records: &[(usize, GapRecord)]) -> TargetedRerunRoute {
    let verify_commands = stable_unique(
        records
            .iter()
            .flat_map(|(_, record)| record.verification_commands.iter().cloned()),
    );
    let receipt_commands = stable_unique(records.iter().filter_map(|(_, record)| {
        record
            .receipt_command
            .as_deref()
            .map(str::trim)
            .filter(|command| !command.is_empty())
            .map(str::to_string)
    }));
    let receipt_command = (receipt_commands.len() == 1).then(|| receipt_commands[0].clone());
    let receipt_command_conflict = (receipt_commands.len() > 1).then(|| TargetedRerunLimitation {
        kind: "receipt_command_conflict",
        message: format!(
            "{} distinct receipt commands were supplied by matching gap ledger records",
            receipt_commands.len()
        ),
    });
    TargetedRerunRoute {
        verify_commands,
        receipt_command,
        receipt_command_conflict,
    }
}

fn stable_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn add_cache_stats(
    target: &mut crate::analysis::seam_cache::FileFactCacheStats,
    source: &crate::analysis::seam_cache::FileFactCacheStats,
) {
    target.hits += source.hits;
    target.misses += source.misses;
    target.corrupt_ignored += source.corrupt_ignored;
    target.stores += source.stores;
    target.store_errors += source.store_errors;
}

fn display_scope(scope: &GapRerunScope) -> String {
    match scope.owner.as_deref() {
        Some(owner) => format!("{}::{owner}", display_path(&scope.file)),
        None => display_path(&scope.file),
    }
}

fn same_root(root: &Path, ledger_root: &Path) -> bool {
    match (
        std::fs::canonicalize(root),
        std::fs::canonicalize(ledger_root),
    ) {
        (Ok(root), Ok(ledger_root)) => root == ledger_root,
        _ => false,
    }
}

fn report(
    state: &'static str,
    selector: TargetedRerunSelector,
    cache: TargetedRerunCache,
    seams: Vec<TargetedRerunSeam>,
    route: Option<TargetedRerunRoute>,
    limitation: Option<TargetedRerunLimitation>,
    scope_limitations: Vec<TargetedRerunScopeLimitation>,
) -> TargetedRerunReport {
    TargetedRerunReport {
        schema_version: "ripr-targeted-rerun-v1",
        state,
        selector,
        cache,
        seams,
        movement: None,
        parity: None,
        route,
        limitation,
        scope_limitations,
        authority_boundary: "static evidence only; no before snapshot was supplied, so gap movement is not inferred",
        parity_scope: None,
    }
}

fn apply_full_pipeline_parity(
    report: &mut TargetedRerunReport,
    root: &Path,
    config: &crate::config::RiprConfig,
) -> Result<(), String> {
    if report.state == "limited" {
        return Ok(());
    }
    let (full, limit_info) = inventory_classified_seams_at_with_config(root, config)?;
    let mut input_mismatches = match workspace_cache_key_at_with_config(root, config) {
        Ok(full_key) => {
            let full_fingerprint = input_fingerprint_for(root, &full_key);
            report.cache.input_fingerprint.as_ref().map_or_else(
                || vec!["input_fingerprint"],
                |targeted| input_fingerprint_changes(targeted, &full_fingerprint),
            )
        }
        Err(_) => vec!["input_fingerprint_unavailable"],
    };
    if let Some(fingerprint) = report.cache.input_fingerprint.as_ref() {
        for field in graph_provenance_unavailable_fields(fingerprint) {
            if !input_mismatches.contains(&field) {
                input_mismatches.push(field);
            }
        }
    }
    let full_by_id = full
        .iter()
        .map(seam_from)
        .map(|seam| (seam.seam_id.clone(), seam))
        .collect::<BTreeMap<_, _>>();
    let scope = report.parity_scope.clone().unwrap_or_default();
    let expected_full = full_by_id
        .values()
        .filter(|seam| seam_matches_resolved_scope(seam, &scope))
        .map(|seam| (seam.seam_id.clone(), seam))
        .collect::<BTreeMap<_, _>>();
    let comparison = compare_selector_scoped_seams(&report.seams, &expected_full);
    let mismatch_count = comparison.mismatches.len();
    let has_mismatches = mismatch_count > 0;
    report.parity = Some(TargetedRerunParity {
        state: if limit_info.is_none() && !has_mismatches && input_mismatches.is_empty() {
            "matched"
        } else {
            "limited"
        },
        basis: "selected_classified_seams",
        targeted_seam_count: report.seams.len(),
        full_selected_seam_count: expected_full.len(),
        selected_seam_count: report.seams.len(),
        matched_seam_count: comparison.matched_seam_count,
        missing_from_targeted: comparison.missing_from_targeted,
        unexpected_in_targeted: comparison.unexpected_in_targeted,
        differing: comparison.differing,
        mismatches: comparison.mismatches,
        input_mismatches,
    });
    if let Some(limit_info) = limit_info {
        report.state = "limited";
        report.limitation = Some(TargetedRerunLimitation {
            kind: "full_pipeline_parity_incomplete",
            message: format!(
                "full pipeline analyzed {} of {} seams under the configured limit; parity is inconclusive",
                limit_info.analyzed, limit_info.total
            ),
        });
    } else if has_mismatches {
        report.state = "limited";
        report.limitation = Some(TargetedRerunLimitation {
            kind: "full_pipeline_parity_mismatch",
            message: format!(
                "{} selected seam(s) differed from the full pipeline; targeted evidence is not a successful parity result",
                mismatch_count
            ),
        });
    } else if report
        .parity
        .as_ref()
        .is_some_and(|parity| !parity.input_mismatches.is_empty())
    {
        report.state = "limited";
        report.limitation = Some(TargetedRerunLimitation {
            kind: "full_pipeline_parity_input_mismatch",
            message:
                "targeted and full pipeline workspace input fingerprints differed; parity is inconclusive"
                    .to_string(),
        });
    }
    Ok(())
}

fn seam_matches_resolved_scope(seam: &TargetedRerunSeam, scope: &ResolvedRerunScope) -> bool {
    if scope.explicit_seam_ids.contains(&seam.seam_id) {
        return true;
    }
    if let Some(canonical_gap_id) = scope.canonical_gap_id.as_deref() {
        return seam.canonical_gap_id.as_deref() == Some(canonical_gap_id);
    }
    if scope.changed_test.is_none() {
        return false;
    }
    let owner_name = seam.owner.rsplit("::").next().unwrap_or(&seam.owner);
    scope.direct_call_names.contains(owner_name)
        || scope.owners.contains(&seam.owner)
        || (scope.direct_call_names.is_empty()
            && scope.owners.is_empty()
            && scope.files.contains(Path::new(&seam.file)))
}

#[derive(Debug, Default)]
struct SelectorScopedParityComparison {
    matched_seam_count: usize,
    missing_from_targeted: Vec<String>,
    unexpected_in_targeted: Vec<String>,
    differing: Vec<TargetedRerunParityMismatch>,
    mismatches: Vec<TargetedRerunParityMismatch>,
}

fn compare_selector_scoped_seams(
    targeted: &[TargetedRerunSeam],
    expected_full: &BTreeMap<String, &TargetedRerunSeam>,
) -> SelectorScopedParityComparison {
    let targeted_by_id = targeted
        .iter()
        .map(|seam| (seam.seam_id.clone(), seam))
        .collect::<BTreeMap<_, _>>();
    let missing_from_targeted = expected_full
        .keys()
        .filter(|seam_id| !targeted_by_id.contains_key(*seam_id))
        .cloned()
        .collect::<Vec<_>>();
    let unexpected_in_targeted = targeted_by_id
        .keys()
        .filter(|seam_id| !expected_full.contains_key(*seam_id))
        .cloned()
        .collect::<Vec<_>>();
    let differing = targeted_by_id
        .iter()
        .filter_map(|(seam_id, targeted)| {
            let full = expected_full.get(seam_id)?;
            let fields = parity_mismatch_fields(targeted, full);
            (!fields.is_empty()).then(|| TargetedRerunParityMismatch {
                seam_id: seam_id.clone(),
                fields,
            })
        })
        .collect::<Vec<_>>();
    let mut mismatches = missing_from_targeted
        .iter()
        .map(|seam_id| TargetedRerunParityMismatch {
            seam_id: seam_id.clone(),
            fields: vec!["missing_from_targeted"],
        })
        .collect::<Vec<_>>();
    mismatches.extend(
        unexpected_in_targeted
            .iter()
            .map(|seam_id| TargetedRerunParityMismatch {
                seam_id: seam_id.clone(),
                fields: vec!["unexpected_in_targeted"],
            }),
    );
    mismatches.extend(differing.clone());
    let matched_seam_count = expected_full
        .keys()
        .filter(|seam_id| {
            targeted_by_id
                .get(*seam_id)
                .zip(expected_full.get(*seam_id))
                .is_some_and(|(targeted, full)| parity_mismatch_fields(targeted, full).is_empty())
        })
        .count();
    SelectorScopedParityComparison {
        matched_seam_count,
        missing_from_targeted,
        unexpected_in_targeted,
        differing,
        mismatches,
    }
}

fn graph_provenance_unavailable_fields(
    fingerprint: &TargetedRerunInputFingerprint,
) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if fingerprint.graph_provenance.package_graph_status != "complete" {
        fields.push("package_graph_provenance_unavailable");
    }
    if fingerprint.graph_provenance.feature_graph_status != "complete" {
        fields.push("feature_graph_provenance_unavailable");
    }
    fields
}

fn parity_mismatch_fields(
    targeted: &TargetedRerunSeam,
    full: &TargetedRerunSeam,
) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if targeted.canonical_gap_id != full.canonical_gap_id {
        fields.push("canonical_gap_id");
    }
    if targeted.static_class != full.static_class {
        fields.push("static_class");
    }
    if targeted.file != full.file {
        fields.push("file");
    }
    if targeted.line != full.line {
        fields.push("line");
    }
    if targeted.owner != full.owner {
        fields.push("owner");
    }
    if targeted.repair_route_readiness != full.repair_route_readiness {
        fields.push("repair_route_readiness");
    }
    if targeted.related_tests != full.related_tests {
        fields.push("related_tests");
    }
    if targeted.missing_discriminators != full.missing_discriminators {
        fields.push("missing_discriminators");
    }
    fields
}

fn apply_before(report: &mut TargetedRerunReport, before: &Path) {
    if report.state == "limited" {
        return;
    }
    let before_text = match std::fs::read_to_string(before) {
        Ok(text) => text,
        Err(err) => {
            set_before_limitation(
                report,
                "before_artifact_unavailable",
                format!(
                    "read explicit before artifact {} failed: {err}",
                    before.display()
                ),
            );
            return;
        }
    };
    disclose_before_input_changes(report, &before_text);
    let current = report
        .seams
        .iter()
        .map(|seam| TargetedRerunStaticSeam {
            seam_id: seam.seam_id.clone(),
            seam_kind: "unknown".to_string(),
            file: seam.file.clone(),
            line: seam.line,
            static_class: seam.static_class.clone(),
        })
        .collect::<Vec<_>>();
    match targeted_rerun_movement_from_json(&before_text, &current) {
        Ok(movement) => {
            report.state = movement.state;
            report.movement = Some(TargetedRerunMovement {
                state: movement.state,
                before: display_path(before),
                before_seam_count: movement.before_seam_count,
                matched_seam_count: movement.matched_seam_count,
            });
            if let Some(message) = movement.limitation {
                report.limitation = Some(TargetedRerunLimitation {
                    kind: "movement_indeterminate",
                    message,
                });
            }
            report.authority_boundary = "static before/after evidence only; movement does not establish runtime mutation behavior, correctness, coverage adequacy, or complete test quality";
        }
        Err(err) => set_before_limitation(report, "before_artifact_incompatible", err),
    }
}

fn disclose_before_input_changes(report: &mut TargetedRerunReport, before_text: &str) {
    let Ok(value) = serde_json::from_str::<Value>(before_text) else {
        return;
    };
    let Some(before_value) = value
        .get("cache")
        .and_then(|cache| cache.get("input_fingerprint"))
    else {
        return;
    };
    let Ok(before) = serde_json::from_value::<TargetedRerunInputFingerprint>(before_value.clone())
    else {
        return;
    };
    let Some(current) = report.cache.input_fingerprint.as_ref() else {
        return;
    };
    let changed = input_fingerprint_changes(&before, current);
    if changed.is_empty() {
        return;
    }
    report.cache.invalidation_status = "workspace_input_changed".to_string();
    report.cache.recomputation_reasons.extend(
        changed
            .into_iter()
            .map(|field| format!("input_changed:{field}")),
    );
}

fn input_fingerprint_changes(
    before: &TargetedRerunInputFingerprint,
    current: &TargetedRerunInputFingerprint,
) -> Vec<&'static str> {
    let fields = [
        (
            "schema_version",
            &before.schema_version,
            &current.schema_version,
        ),
        (
            "analyzer_version",
            &before.analyzer_version,
            &current.analyzer_version,
        ),
        (
            "workspace_root_hash",
            &before.workspace_root_hash,
            &current.workspace_root_hash,
        ),
        (
            "files_content_hash",
            &before.files_content_hash,
            &current.files_content_hash,
        ),
        (
            "cfg_features_hash",
            &before.cfg_features_hash,
            &current.cfg_features_hash,
        ),
        ("config_hash", &before.config_hash, &current.config_hash),
        (
            "test_intent_hash",
            &before.test_intent_hash,
            &current.test_intent_hash,
        ),
        (
            "suppressions_hash",
            &before.suppressions_hash,
            &current.suppressions_hash,
        ),
        (
            "workspace_manifests_hash",
            &before.workspace_manifests_hash,
            &current.workspace_manifests_hash,
        ),
        (
            "lockfile_hash",
            &before.lockfile_hash,
            &current.lockfile_hash,
        ),
        (
            "toolchain_hash",
            &before.toolchain_hash,
            &current.toolchain_hash,
        ),
        (
            "seam_limit_key",
            &before.seam_limit_key,
            &current.seam_limit_key,
        ),
        (
            "selector_ledger_hash",
            &before.selector_ledger_hash,
            &current.selector_ledger_hash,
        ),
    ];
    let mut changed = fields
        .into_iter()
        .filter_map(|(name, before, current)| (before != current).then_some(name))
        .collect::<Vec<_>>();
    let package_graph_changed = before.graph_provenance.package_graph_status
        != current.graph_provenance.package_graph_status
        || before.graph_provenance.package_graph_hash
            != current.graph_provenance.package_graph_hash
        || before.graph_provenance.package_graph_detail
            != current.graph_provenance.package_graph_detail;
    if package_graph_changed && !changed.contains(&"package_graph_provenance") {
        changed.push("package_graph_provenance");
    }
    let feature_graph_changed = before.graph_provenance.feature_graph_status
        != current.graph_provenance.feature_graph_status
        || before.graph_provenance.feature_graph_hash
            != current.graph_provenance.feature_graph_hash
        || before.graph_provenance.feature_graph_detail
            != current.graph_provenance.feature_graph_detail;
    if feature_graph_changed && !changed.contains(&"feature_graph_provenance") {
        changed.push("feature_graph_provenance");
    }
    if before.graph_provenance.external_dependency_graph_status
        != current.graph_provenance.external_dependency_graph_status
        || before.graph_provenance.external_dependency_graph_detail
            != current.graph_provenance.external_dependency_graph_detail
    {
        changed.push("external_dependency_graph_provenance");
    }
    changed
}

fn set_before_limitation(report: &mut TargetedRerunReport, kind: &'static str, message: String) {
    report.state = "limited";
    report.limitation = Some(TargetedRerunLimitation { kind, message });
    report.authority_boundary = "static evidence only; explicit before-state movement was unavailable, so no improvement, closure, unchanged, or regression claim is made";
}

fn limited_report(
    selector: TargetedRerunSelector,
    kind: &'static str,
    message: String,
) -> TargetedRerunReport {
    report(
        "limited",
        selector,
        TargetedRerunCache {
            schema_version: crate::analysis::seam_cache::FILE_FACT_CACHE_SCHEMA_VERSION,
            reuse_state: "not_run",
            file_fact_status: "not_run".to_string(),
            hits: 0,
            misses: 0,
            corrupt_ignored: 0,
            stores: 0,
            store_errors: 0,
            recomputation_reasons: Vec::new(),
            invalidation_status: "not_available".to_string(),
            input_fingerprint: None,
        },
        Vec::new(),
        None,
        Some(TargetedRerunLimitation { kind, message }),
        Vec::new(),
    )
}

fn limited_report_with_scope_limitations(
    selector: TargetedRerunSelector,
    kind: &'static str,
    message: String,
    scope_limitations: Vec<TargetedRerunScopeLimitation>,
) -> TargetedRerunReport {
    let mut report = limited_report(selector, kind, message);
    report.scope_limitations = scope_limitations;
    report
}

fn cache_from(
    cache: &crate::analysis::seam_cache::FileFactCacheStats,
    selected_scope_reasons: impl IntoIterator<Item = &'static str>,
) -> TargetedRerunCache {
    let mut recomputation_reasons = selected_scope_reasons
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if cache.corrupt_ignored > 0 {
        recomputation_reasons.push("corrupt_file_fact_ignored".to_string());
    }
    if cache.store_errors > 0 {
        recomputation_reasons.push("file_fact_store_error".to_string());
    }
    recomputation_reasons.extend(
        cache
            .invalidated_files
            .iter()
            .map(|path| format!("file_content_changed:{}", display_path(path))),
    );
    TargetedRerunCache {
        schema_version: crate::analysis::seam_cache::FILE_FACT_CACHE_SCHEMA_VERSION,
        reuse_state: if cache.misses == 0 && cache.corrupt_ignored == 0 {
            "reused_file_facts"
        } else if cache.hits == 0 {
            "recomputed_file_facts"
        } else {
            "mixed_file_fact_reuse"
        },
        file_fact_status: cache.status_label(),
        hits: cache.hits,
        misses: cache.misses,
        corrupt_ignored: cache.corrupt_ignored,
        stores: cache.stores,
        store_errors: cache.store_errors,
        recomputation_reasons,
        invalidation_status: if cache.invalidated_files.is_empty() {
            "not_available".to_string()
        } else {
            "file_content_changed".to_string()
        },
        input_fingerprint: None,
    }
}

fn seams_from(entries: &[crate::analysis::ClassifiedSeam]) -> Vec<TargetedRerunSeam> {
    entries.iter().map(seam_from).collect()
}

fn seam_from(entry: &crate::analysis::ClassifiedSeam) -> TargetedRerunSeam {
    let repair_route_readiness = repair_route_readiness(entry);
    TargetedRerunSeam {
        canonical_gap_id: canonical_gap_identity(entry).map(|identity| identity.id),
        seam_id: entry.seam.id().as_str().to_string(),
        file: display_path(entry.seam.file()),
        line: entry.seam.display_line(),
        owner: entry.seam.owner().to_string(),
        static_class: entry.class.as_str().to_string(),
        repair_route_readiness,
        related_tests: entry
            .evidence
            .related_tests
            .iter()
            .map(|test| TargetedRerunRelatedTest {
                test_name: test.test_name.clone(),
                file: display_path(&test.file),
                line: test.line,
                oracle_kind: test.oracle_kind.as_str().to_string(),
                oracle_strength: test.oracle_strength.as_str().to_string(),
                evidence_summary: test.evidence_summary.clone(),
                relation_reason: test.relation_reason.as_str().to_string(),
                relation_confidence: test.relation_confidence.as_str().to_string(),
            })
            .collect(),
        missing_discriminators: entry
            .evidence
            .missing_discriminators
            .iter()
            .map(|fact| TargetedRerunMissingDiscriminator {
                value: fact.value.clone(),
                reason: fact.reason.clone(),
            })
            .collect(),
    }
}

fn seam_from_with_selected_gap(
    entry: &crate::analysis::ClassifiedSeam,
    canonical_gap_id: &str,
) -> TargetedRerunSeam {
    let mut seam = seam_from(entry);
    seam.canonical_gap_id = Some(canonical_gap_id.to_string());
    seam
}

fn normalize_changed_test(root: &Path, changed_test: &Path) -> Result<PathBuf, String> {
    if changed_test.is_absolute() {
        return changed_test
            .strip_prefix(root)
            .map(PathBuf::from)
            .map_err(|err| {
                format!(
                    "rerun changed test {} is outside root {}: {err}",
                    changed_test.display(),
                    root.display()
                )
            });
    }
    Ok(changed_test.to_path_buf())
}

fn parse_changed_test_selector(selector: &str) -> Result<(PathBuf, Option<String>), String> {
    let (file, node) = selector
        .split_once("::")
        .map_or((selector, None), |(file, node)| (file, Some(node)));
    if file.trim().is_empty() {
        return Err("rerun --changed-test requires a file path".to_string());
    }
    let node = node
        .map(str::trim)
        .filter(|node| !node.is_empty())
        .map(str::to_string);
    if selector.contains("::") && node.is_none() {
        return Err("rerun --changed-test test node must not be empty".to_string());
    }
    Ok((PathBuf::from(file), node))
}

fn resolve_output_path(root: &Path, out: &Path) -> PathBuf {
    if out.is_absolute() {
        out.to_path_buf()
    } else {
        root.join(out)
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn write_text_file(path: &Path, rendered: &str) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    std::fs::write(path, rendered).map_err(|err| format!("write {} failed: {err}", path.display()))
}

fn render_human(report: &TargetedRerunReport) -> String {
    let mut lines = vec![
        format!("State: {}", report.state),
        format!("Selector: {}", report.selector.kind),
        format!("Selected tests: {}", report.selector.selected_test_count),
        format!(
            "Direct call names: {}",
            report.selector.direct_call_names.join(", ")
        ),
        format!(
            "File-fact cache: {} (hits {}, misses {})",
            report.cache.file_fact_status, report.cache.hits, report.cache.misses
        ),
        "Recomputed seams:".to_string(),
    ];
    if let Some(changed_test) = report.selector.changed_test.as_deref() {
        lines.push(format!("Changed test: {changed_test}"));
    }
    if let Some(canonical_gap_id) = report.selector.canonical_gap_id.as_deref() {
        lines.push(format!("Canonical gap: {canonical_gap_id}"));
    }
    if let Some(gap_ledger) = report.selector.gap_ledger.as_deref() {
        lines.push(format!("Gap ledger: {gap_ledger}"));
    }
    if let Some(matched_record_count) = report.selector.matched_record_count {
        lines.push(format!("Matched ledger records: {matched_record_count}"));
    }
    if let Some(recomputed_scope_count) = report.selector.recomputed_scope_count {
        lines.push(format!("Recomputed scopes: {recomputed_scope_count}"));
    }
    lines.extend(report.seams.iter().map(|seam| {
        format!(
            "  - [{}] {}:{} {} ({})",
            seam.static_class, seam.file, seam.line, seam.owner, seam.seam_id
        )
    }));
    if let Some(movement) = report.movement.as_ref() {
        lines.push(format!("Movement: {}", movement.state));
        lines.push(format!("Before: {}", movement.before));
    }
    if let Some(route) = report.route.as_ref() {
        if !route.verify_commands.is_empty() {
            lines.push(format!("Verify: {}", route.verify_commands.join(" && ")));
        }
        if let Some(receipt_command) = route.receipt_command.as_deref() {
            lines.push(format!("Receipt: {receipt_command}"));
        }
        if let Some(conflict) = route.receipt_command_conflict.as_ref() {
            lines.push(format!(
                "Route limitation ({}): {}",
                conflict.kind, conflict.message
            ));
        }
    }
    if let Some(limitation) = report.limitation.as_ref() {
        lines.push(format!(
            "Limitation ({}): {}",
            limitation.kind, limitation.message
        ));
    }
    for limitation in &report.scope_limitations {
        lines.push(format!(
            "Scope limitation #{} ({}): {}",
            limitation.record_index, limitation.kind, limitation.message
        ));
    }
    lines.push(format!("Boundary: {}", report.authority_boundary));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{
        RerunSelector, ResolvedRerunScope, TargetedRerunCache, TargetedRerunGraphProvenance,
        TargetedRerunInputFingerprint, TargetedRerunMissingDiscriminator, TargetedRerunMovement,
        TargetedRerunRelatedTest, TargetedRerunReport, TargetedRerunSeam, TargetedRerunSelector,
        cache_from, compare_selector_scoped_seams, entry_matches_selected_gap,
        graph_provenance_unavailable_fields, input_fingerprint_changes, parity_mismatch_fields,
        parse_options, render_human, rerun_gap, resolve_gap_records, route_from_gap_records,
        same_root, scopes_from_gap_records, seam_from, seam_matches_resolved_scope,
    };
    use crate::analysis::ClassifiedSeam;
    use crate::analysis::classify_seam;
    use crate::analysis::repair_route::{
        NewTestKind, NewTestProposalProvenance, NewTestTargetProposal,
        REPAIR_ROUTE_AUTHORITY_BOUNDARY, RepairRouteReadiness, RepairRouteState,
        RepairTargetSelection,
    };
    use crate::analysis::seam_cache::FileFactCacheStats;
    use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
    #[cfg(feature = "lang-typescript")]
    use crate::analysis::targeted_typescript_findings_for_scope;
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, RelationConfidence, RelationReason, TestGripEvidence, TestTargetEvidence,
    };
    use crate::config::RiprConfig;
    use crate::domain::{
        Confidence, MissingDiscriminatorFact, OracleKind, OracleStrength, StageEvidence, StageState,
    };
    use crate::output::gap_decision_ledger::{GapAnchor, GapRecord};
    #[cfg(feature = "lang-typescript")]
    use crate::output::typescript_packet_projection::typescript_canonical_gap_id;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn gap_record(
        canonical_gap_id: &str,
        file: Option<&str>,
        owner: Option<&str>,
        verification_commands: &[&str],
        receipt_command: Option<&str>,
    ) -> GapRecord {
        GapRecord {
            canonical_gap_id: canonical_gap_id.to_string(),
            anchor: file.map(|file| GapAnchor {
                file: Some(file.to_string()),
                owner: owner.map(str::to_string),
                ..GapAnchor::default()
            }),
            verification_commands: verification_commands
                .iter()
                .map(|command| (*command).to_string())
                .collect(),
            receipt_command: receipt_command.map(str::to_string),
            ..GapRecord::default()
        }
    }

    #[cfg(feature = "lang-typescript")]
    #[test]
    fn canonical_gap_rerun_recomputes_typescript_scope() -> Result<(), String> {
        let root = unique_temp_root("ts-rerun")?;
        write_file(
            &root.join("src/discount.ts"),
            "export function applyDiscount(amount: number, discount: number) {\n  return amount - discount;\n}\n",
        )?;
        write_file(
            &root.join("tests/discount.test.ts"),
            "import { applyDiscount } from '../src/discount';\n\ntest('applies discount', () => {\n  expect(applyDiscount(10000, 100)).toBe(9900);\n});\n",
        )?;

        let config = RiprConfig::default();
        let findings = targeted_typescript_findings_for_scope(
            &root,
            &config,
            Path::new("src/discount.ts"),
            Some(2),
        )?;
        let finding = findings
            .first()
            .ok_or_else(|| "expected TypeScript finding from rerun scope".to_string())?;
        let canonical_gap_id = typescript_canonical_gap_id(&finding.id);
        let ledger_path = root.join("target/ripr/gaps.json");
        write_file(
            &ledger_path,
            &format!(
                r#"{{"schema_version":"ripr-gap-decision-ledger-v1","root":"{}","records":[{{"canonical_gap_id":"{}","language":"typescript","anchor":{{"file":"src/discount.ts","line":2,"owner":"applyDiscount"}},"evidence_ids":["{}"],"verification_commands":["npm test -- tests/discount.test.ts"],"receipt_command":"ripr outcome --before before.json --after after.json --format json"}}]}}"#,
                root.display(),
                canonical_gap_id,
                finding.id
            ),
        )?;

        let report = rerun_gap(&root, &config, &canonical_gap_id, &ledger_path)?;
        let _ = std::fs::remove_dir_all(&root);
        if report.state != "current_state_only"
            || report.seams.len() != 1
            || report.seams[0].canonical_gap_id.as_deref() != Some(canonical_gap_id.as_str())
            || report.seams[0].file != "src/discount.ts"
        {
            return Err(format!(
                "TypeScript rerun did not select the anchored current seam: state={} seams={}",
                report.state,
                report.seams.len()
            ));
        }
        Ok(())
    }

    #[cfg(feature = "lang-typescript")]
    #[test]
    fn typescript_rerun_covers_every_accepted_extension() -> Result<(), String> {
        for extension in ["ts", "tsx", "js", "jsx"] {
            let root = unique_temp_root(&format!("ts-rerun-ext-{extension}"))?;
            let source = format!("src/discount.{extension}");
            // Type annotations only in TypeScript-family fixtures; .js/.jsx
            // fixtures must be plain ECMAScript or the control is unfaithful.
            let body = if extension.starts_with("ts") {
                "export function applyDiscount(amount: number, discount: number) {\n  return amount - discount;\n}\n"
            } else {
                "export function applyDiscount(amount, discount) {\n  return amount - discount;\n}\n"
            };
            write_file(&root.join(&source), body)?;
            let config = RiprConfig::default();
            let findings = crate::analysis::targeted_typescript_findings_for_scope(
                &root,
                &config,
                Path::new(&source),
                Some(2),
            )?;
            let finding = findings
                .first()
                .ok_or_else(|| format!("expected TypeScript finding for .{extension} scope"))?;
            let canonical_gap_id = typescript_canonical_gap_id(&finding.id);
            let ledger_path = root.join("target/ripr/gaps.json");
            write_file(
                &ledger_path,
                &format!(
                    r#"{{"schema_version":"ripr-gap-decision-ledger-v1","root":"{}","records":[{{"canonical_gap_id":"{}","language":"typescript","anchor":{{"file":"{}","line":2,"owner":"applyDiscount"}},"evidence_ids":["{}"],"verification_commands":["npm test"],"receipt_command":"ripr outcome --before b.json --after a.json --format json"}}]}}"#,
                    root.display(),
                    canonical_gap_id,
                    source,
                    finding.id
                ),
            )?;
            let report = rerun_gap(&root, &config, &canonical_gap_id, &ledger_path)?;
            let _ = std::fs::remove_dir_all(&root);
            if report.seams.len() != 1
                || report.seams[0].canonical_gap_id.as_deref() != Some(canonical_gap_id.as_str())
            {
                return Err(format!(
                    "TypeScript rerun did not select the anchored seam for .{extension}: state={} seams={}",
                    report.state,
                    report.seams.len()
                ));
            }
        }
        Ok(())
    }

    #[cfg(feature = "lang-typescript")]
    #[test]
    fn typescript_rerun_scope_rejects_root_escaping_anchors() -> Result<(), String> {
        let root = unique_temp_root("ts-rerun-escape")?;
        let config = RiprConfig::default();
        for escaping in [
            Path::new("../outside.ts"),
            Path::new("src/../../outside.ts"),
            Path::new("/etc/hostname"),
        ] {
            match crate::analysis::targeted_typescript_findings_for_scope(
                &root,
                &config,
                escaping,
                Some(1),
            ) {
                Err(message) if message.contains("escapes the workspace root") => {}
                other => {
                    let _ = std::fs::remove_dir_all(&root);
                    return Err(format!(
                        "escaping anchor {} was not rejected: {other:?}",
                        escaping.display()
                    ));
                }
            }
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[cfg(feature = "lang-typescript")]
    #[test]
    fn typescript_rerun_without_current_seam_fails_closed() -> Result<(), String> {
        let root = unique_temp_root("ts-rerun-no-match")?;
        write_file(
            &root.join("src/other.ts"),
            "export function other(amount: number) {\n  return amount + 1;\n}\n",
        )?;
        let config = RiprConfig::default();
        let ledger_path = root.join("target/ripr/gaps.json");
        write_file(
            &ledger_path,
            &format!(
                r#"{{"schema_version":"ripr-gap-decision-ledger-v1","root":"{}","records":[{{"canonical_gap_id":"gap:typescript:missing","language":"typescript","anchor":{{"file":"src/other.ts","line":1,"owner":"other"}},"evidence_ids":["missing"],"verification_commands":["npm test"],"receipt_command":"ripr outcome --before b.json --after a.json --format json"}}]}}"#,
                root.display()
            ),
        )?;
        let report = rerun_gap(&root, &config, "gap:typescript:missing", &ledger_path)?;
        let _ = std::fs::remove_dir_all(&root);
        if !report
            .scope_limitations
            .iter()
            .any(|limitation| limitation.kind == "gap_scope_unresolved")
        {
            return Err(format!(
                "unmatched TypeScript rerun fabricated a current seam: state={} seams={}",
                report.state,
                report.seams.len()
            ));
        }
        Ok(())
    }

    #[cfg(feature = "lang-typescript")]
    fn unique_temp_root(name: &str) -> Result<PathBuf, String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|err| err.to_string())?
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).map_err(|err| format!("create temp root failed: {err}"))?;
        Ok(root)
    }

    #[cfg(feature = "lang-typescript")]
    fn write_file(path: &Path, contents: &str) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
        }
        std::fs::write(path, contents)
            .map_err(|err| format!("write {} failed: {err}", path.display()))
    }

    #[test]
    fn rerun_requires_one_selector() {
        assert_eq!(
            parse_options(&[]),
            Err(
                "rerun requires --changed-test <path> or --gap <canonical-gap-id> --gap-ledger <path>"
                    .to_string()
            )
        );
    }

    #[test]
    fn rerun_gap_requires_explicit_ledger() {
        assert_eq!(
            parse_options(&args(&["--gap", "gap:example"])),
            Err("rerun --gap requires --gap-ledger <path>".to_string())
        );
    }

    #[test]
    fn rerun_cache_disclosure_names_reuse_and_fallbacks() -> Result<(), String> {
        let reused = cache_from(
            &FileFactCacheStats {
                hits: 2,
                ..FileFactCacheStats::default()
            },
            ["selected_test_scope_recomputed"],
        );
        if reused.reuse_state != "reused_file_facts"
            || reused.schema_version != crate::analysis::seam_cache::FILE_FACT_CACHE_SCHEMA_VERSION
            || reused.recomputation_reasons != vec!["selected_test_scope_recomputed"]
        {
            return Err(format!("unexpected reused cache disclosure: {reused:?}"));
        }
        let fallback = cache_from(
            &FileFactCacheStats {
                corrupt_ignored: 1,
                store_errors: 1,
                ..FileFactCacheStats::default()
            },
            ["selected_gap_scopes_recomputed"],
        );
        if fallback.reuse_state != "recomputed_file_facts"
            || fallback.recomputation_reasons
                != vec![
                    "selected_gap_scopes_recomputed",
                    "corrupt_file_fact_ignored",
                    "file_fact_store_error",
                ]
        {
            return Err(format!(
                "unexpected fallback cache disclosure: {fallback:?}"
            ));
        }
        let mut invalidated_stats = FileFactCacheStats::default();
        invalidated_stats
            .invalidated_files
            .insert(PathBuf::from("tests/pricing.rs"));
        let invalidated = cache_from(&invalidated_stats, ["selected_test_scope_recomputed"]);
        if invalidated.invalidation_status != "file_content_changed"
            || invalidated.recomputation_reasons
                != vec![
                    "selected_test_scope_recomputed",
                    "file_content_changed:tests/pricing.rs",
                ]
        {
            return Err(format!(
                "unexpected content invalidation disclosure: {invalidated:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn canonical_gap_selection_retains_explicit_seam_after_gap_identity_closes()
    -> Result<(), String> {
        let selected = BTreeSet::from(["seam:price".to_string()]);
        if !entry_matches_selected_gap(None, "seam:price", "gap:price", &selected)
            || entry_matches_selected_gap(None, "seam:other", "gap:price", &selected)
        {
            return Err(
                "explicit ledger seam identity did not constrain closed-gap selection".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn canonical_gap_records_preserve_multiple_anchored_scopes() -> Result<(), String> {
        let canonical_gap_id = "gap:shared";
        let records = resolve_gap_records(
            vec![
                gap_record(
                    canonical_gap_id,
                    Some("src/lib.rs"),
                    Some("crate::price"),
                    &[],
                    None,
                ),
                gap_record(
                    canonical_gap_id,
                    Some("src/lib.rs"),
                    Some("crate::price"),
                    &[],
                    None,
                ),
                gap_record(
                    canonical_gap_id,
                    Some("src/lib.rs"),
                    Some("crate::validate"),
                    &[],
                    None,
                ),
            ],
            canonical_gap_id,
        )
        .map_err(|limitation| limitation.message)?;
        let (scopes, limitations) = scopes_from_gap_records(&records);
        if records.len() != 3 || scopes.len() != 2 || !limitations.is_empty() {
            return Err(format!(
                "expected three grouped records and two scopes, got records={} scopes={} limitations={}",
                records.len(),
                scopes.len(),
                limitations.len()
            ));
        }
        Ok(())
    }

    #[test]
    fn canonical_gap_duplicate_projection_deduplicates_scope_and_commands() -> Result<(), String> {
        let canonical_gap_id = "gap:shared";
        let record = gap_record(
            canonical_gap_id,
            Some("src/lib.rs"),
            Some("crate::price"),
            &["cargo test price", "cargo test price"],
            Some("ripr receipt write --gap gap:shared"),
        );
        let records = resolve_gap_records(vec![record.clone(), record], canonical_gap_id)
            .map_err(|limitation| limitation.message)?;
        let (scopes, limitations) = scopes_from_gap_records(&records);
        let route = route_from_gap_records(&records);
        if scopes.len() != 1
            || !limitations.is_empty()
            || route.verify_commands != vec!["cargo test price".to_string()]
            || route.receipt_command.as_deref() != Some("ripr receipt write --gap gap:shared")
            || route.receipt_command_conflict.is_some()
        {
            return Err("duplicate gap-record projection was not stably deduplicated".to_string());
        }
        Ok(())
    }

    #[test]
    fn canonical_gap_conflicting_receipt_commands_remain_explicit() -> Result<(), String> {
        let records = resolve_gap_records(
            vec![
                gap_record(
                    "gap:shared",
                    Some("src/lib.rs"),
                    Some("crate::price"),
                    &["cargo test price"],
                    Some("ripr receipt write --gap gap:shared --route one"),
                ),
                gap_record(
                    "gap:shared",
                    Some("src/lib.rs"),
                    Some("crate::validate"),
                    &["cargo test validate", "cargo test price"],
                    Some("ripr receipt write --gap gap:shared --route two"),
                ),
            ],
            "gap:shared",
        )
        .map_err(|limitation| limitation.message)?;
        let route = route_from_gap_records(&records);
        if route.verify_commands
            != vec![
                "cargo test price".to_string(),
                "cargo test validate".to_string(),
            ]
            || route.receipt_command.is_some()
            || route
                .receipt_command_conflict
                .as_ref()
                .map(|limitation| limitation.kind)
                != Some("receipt_command_conflict")
        {
            return Err(format!(
                "conflicting receipt command route was not explicit: {:?}",
                route.receipt_command
            ));
        }
        Ok(())
    }

    #[test]
    fn failed_root_canonicalizations_do_not_compare_equal() -> Result<(), String> {
        let missing = Path::new("target/ripr/missing-rerun-root");
        if same_root(missing, missing) {
            return Err("two failed root canonicalizations compared equal".to_string());
        }
        Ok(())
    }

    #[test]
    fn rerun_parses_gap_selector_with_explicit_ledger() -> Result<(), String> {
        let options = parse_options(&args(&[
            "--gap",
            "gap:example",
            "--gap-ledger",
            "target/ripr/gaps.json",
        ]))?;
        if options.selector
            != (RerunSelector::Gap {
                canonical_gap_id: "gap:example".to_string(),
                gap_ledger: PathBuf::from("target/ripr/gaps.json"),
            })
        {
            return Err(format!("unexpected rerun options: {options:?}"));
        }
        Ok(())
    }

    #[test]
    fn rerun_rejects_multiple_changed_test_selectors() {
        assert_eq!(
            parse_options(&args(&[
                "--changed-test",
                "tests/first.rs",
                "--changed-test",
                "tests/second.rs",
            ])),
            Err("rerun accepts exactly one selector".to_string())
        );
    }

    #[test]
    fn rerun_parses_changed_test_json_output() -> Result<(), String> {
        let options = parse_options(&args(&[
            "--root",
            "workspace",
            "--changed-test",
            "tests/pricing.rs",
            "--json",
            "--out",
            "target/rerun.json",
        ]))?;
        if options.root.as_path() != Path::new("workspace")
            || options.selector
                != (RerunSelector::ChangedTest {
                    file: PathBuf::from("tests/pricing.rs"),
                    node: None,
                })
            || !options.json
            || options.out != Some(PathBuf::from("target/rerun.json"))
            || options.before.is_some()
        {
            return Err(format!("unexpected rerun options: {options:?}"));
        }
        Ok(())
    }

    #[test]
    fn rerun_parses_changed_test_node_selector() -> Result<(), String> {
        let options = parse_options(&args(&[
            "--changed-test",
            "tests/pricing.rs::discounted_total_case",
        ]))?;
        if options.selector
            != (RerunSelector::ChangedTest {
                file: PathBuf::from("tests/pricing.rs"),
                node: Some("discounted_total_case".to_string()),
            })
        {
            return Err(format!("unexpected node selector: {:?}", options.selector));
        }
        Ok(())
    }

    #[test]
    fn rerun_rejects_empty_changed_test_node() -> Result<(), String> {
        parse_options(&args(&["--changed-test", "tests/pricing.rs::"]))
            .is_err()
            .then_some(())
            .ok_or_else(|| "empty changed-test node should be rejected".to_string())
    }

    #[test]
    fn rerun_parses_explicit_before_artifact() -> Result<(), String> {
        let options = parse_options(&args(&[
            "--changed-test",
            "tests/pricing.rs",
            "--before",
            "target/ripr/before.json",
        ]))?;
        if options.before != Some(PathBuf::from("target/ripr/before.json")) {
            return Err(format!("expected explicit before path, got {options:?}"));
        }
        Ok(())
    }

    #[test]
    fn rerun_parses_explicit_full_pipeline_parity_check() -> Result<(), String> {
        let options = parse_options(&args(&[
            "--changed-test",
            "tests/pricing.rs",
            "--check-parity",
        ]))?;
        if !options.check_parity {
            return Err("expected explicit parity check option".to_string());
        }
        Ok(())
    }

    #[test]
    fn parity_names_related_test_and_missing_discriminator_drift() {
        let baseline = TargetedRerunSeam {
            canonical_gap_id: Some("gap:example".to_string()),
            seam_id: "seam:example".to_string(),
            file: "src/lib.rs".to_string(),
            line: 8,
            owner: "pricing::discounted_total".to_string(),
            static_class: "weakly_gripped".to_string(),
            repair_route_readiness: baseline_readiness(),
            related_tests: Vec::new(),
            missing_discriminators: Vec::new(),
        };
        let targeted = TargetedRerunSeam {
            related_tests: vec![TargetedRerunRelatedTest {
                test_name: "discounted_total_case".to_string(),
                file: "tests/pricing.rs".to_string(),
                line: 12,
                oracle_kind: "exact_value".to_string(),
                oracle_strength: "strong".to_string(),
                evidence_summary: "asserts exact total".to_string(),
                relation_reason: "direct_owner_call".to_string(),
                relation_confidence: "high".to_string(),
            }],
            missing_discriminators: vec![TargetedRerunMissingDiscriminator {
                value: "amount == threshold".to_string(),
                reason: "boundary value is not observed".to_string(),
            }],
            ..baseline_fields()
        };

        assert_eq!(
            parity_mismatch_fields(&targeted, &baseline),
            vec!["related_tests", "missing_discriminators"]
        );
    }

    #[test]
    fn parity_detects_missing_expected_targeted_seam() {
        let first = baseline_fields();
        let second = TargetedRerunSeam {
            seam_id: "seam:second".to_string(),
            ..baseline_fields()
        };
        let expected = [(&first.seam_id, &first), (&second.seam_id, &second)]
            .into_iter()
            .map(|(id, seam)| (id.clone(), seam))
            .collect();
        let comparison = compare_selector_scoped_seams(std::slice::from_ref(&first), &expected);
        assert_eq!(comparison.missing_from_targeted, vec!["seam:second"]);
        assert!(comparison.unexpected_in_targeted.is_empty());
        assert_eq!(comparison.matched_seam_count, 1);
    }

    #[test]
    fn parity_detects_unexpected_targeted_seam() {
        let expected_seam = baseline_fields();
        let unexpected = TargetedRerunSeam {
            seam_id: "seam:unexpected".to_string(),
            ..baseline_fields()
        };
        let expected = [(&expected_seam.seam_id, &expected_seam)]
            .into_iter()
            .map(|(id, seam)| (id.clone(), seam))
            .collect();
        let comparison =
            compare_selector_scoped_seams(&[expected_seam.clone(), unexpected], &expected);
        assert_eq!(comparison.unexpected_in_targeted, vec!["seam:unexpected"]);
        assert!(comparison.missing_from_targeted.is_empty());
    }

    #[test]
    fn parity_reports_differing_static_class_owner_and_file() {
        let targeted = TargetedRerunSeam {
            file: "src/other.rs".to_string(),
            owner: "pricing::other_total".to_string(),
            static_class: "exposed".to_string(),
            ..baseline_fields()
        };
        let mut full = baseline_fields();
        full.seam_id = targeted.seam_id.clone();
        full.file = "src/lib.rs".to_string();
        full.owner = "pricing::discounted_total".to_string();
        full.static_class = "weakly_gripped".to_string();
        let expected = [(&full.seam_id, &full)]
            .into_iter()
            .map(|(id, seam)| (id.clone(), seam))
            .collect();
        let comparison = compare_selector_scoped_seams(&[targeted], &expected);
        assert_eq!(
            comparison.differing[0].fields,
            vec!["static_class", "file", "owner"]
        );
    }

    #[test]
    fn parity_reports_repair_route_readiness_drift() {
        let targeted = baseline_fields();
        let mut full = baseline_fields();
        full.repair_route_readiness.state = RepairRouteState::StaticLimitation;
        let expected = [(&full.seam_id, &full)]
            .into_iter()
            .map(|(id, seam)| (id.clone(), seam))
            .collect();
        let comparison = compare_selector_scoped_seams(&[targeted], &expected);
        assert_eq!(
            comparison.differing[0].fields,
            vec!["repair_route_readiness"]
        );
    }

    #[test]
    fn targeted_rerun_serializes_static_limitation_route_state() -> Result<(), String> {
        let mut seam = baseline_fields();
        seam.static_class = "activation_unknown".to_string();
        seam.repair_route_readiness.state = RepairRouteState::StaticLimitation;
        seam.repair_route_readiness.required_evidence = vec![
            "producer-owned missing discriminator fact".to_string(),
            "safe test target".to_string(),
            "incomplete evidence stage: activation".to_string(),
        ];
        seam.repair_route_readiness.present_evidence = vec![
            "producer-owned missing discriminator fact".to_string(),
            "safe test target".to_string(),
        ];
        seam.repair_route_readiness.missing_evidence =
            vec!["incomplete evidence stage: activation".to_string()];

        let value = serde_json::to_value(seam)
            .map_err(|error| format!("serialize targeted rerun seam: {error}"))?;
        if value["repair_route_readiness"]["state"] != "static_limitation" {
            return Err(format!(
                "targeted rerun must serialize static_limitation: {value}"
            ));
        }
        Ok(())
    }

    #[test]
    fn resolved_scope_matches_canonical_gap_or_typed_seam_identity() {
        let seam = baseline_fields();
        let canonical = ResolvedRerunScope {
            canonical_gap_id: Some("gap:example".to_string()),
            ..ResolvedRerunScope::default()
        };
        assert!(seam_matches_resolved_scope(&seam, &canonical));
        let mut closed = seam.clone();
        closed.canonical_gap_id = None;
        let typed = ResolvedRerunScope {
            explicit_seam_ids: [closed.seam_id.clone()].into_iter().collect(),
            ..ResolvedRerunScope::default()
        };
        assert!(seam_matches_resolved_scope(&closed, &typed));

        let changed = ResolvedRerunScope {
            files: [PathBuf::from("src/lib.rs")].into_iter().collect(),
            owners: ["pricing::discounted_total".to_string()]
                .into_iter()
                .collect(),
            changed_test: Some(PathBuf::from("tests/pricing.rs")),
            direct_call_names: ["discounted_total".to_string()].into_iter().collect(),
            ..ResolvedRerunScope::default()
        };
        assert!(seam_matches_resolved_scope(&seam, &changed));
        let unrelated = TargetedRerunSeam {
            owner: "pricing::unrelated_total".to_string(),
            ..seam
        };
        assert!(!seam_matches_resolved_scope(&unrelated, &changed));
    }

    #[test]
    fn input_fingerprint_changes_name_owned_workspace_inputs() {
        let before = sample_input_fingerprint();
        let mut current = before.clone();
        current.workspace_manifests_hash = "changed-manifest".to_string();
        current.lockfile_hash = "changed-lockfile".to_string();
        current.config_hash = "changed-config".to_string();

        assert_eq!(
            input_fingerprint_changes(&before, &current),
            vec!["config_hash", "workspace_manifests_hash", "lockfile_hash"]
        );
    }

    #[test]
    fn input_fingerprint_changes_name_graph_provenance_inputs() {
        let before = sample_input_fingerprint();
        let mut current = before.clone();
        current.graph_provenance.package_graph_hash = Some("changed-package-graph".to_string());
        current.graph_provenance.feature_graph_status = "limited".to_string();
        assert_eq!(
            input_fingerprint_changes(&before, &current),
            vec!["package_graph_provenance", "feature_graph_provenance"]
        );
        current.graph_provenance.external_dependency_graph_detail =
            "metadata became available".to_string();
        assert_eq!(
            input_fingerprint_changes(&before, &current),
            vec![
                "package_graph_provenance",
                "feature_graph_provenance",
                "external_dependency_graph_provenance"
            ]
        );
    }

    #[test]
    fn missing_graph_provenance_is_explicitly_unavailable() -> Result<(), String> {
        let mut value = serde_json::to_value(sample_input_fingerprint())
            .map_err(|err| format!("serialize fingerprint: {err}"))?;
        value
            .as_object_mut()
            .ok_or_else(|| "fingerprint should serialize as an object".to_string())?
            .remove("graph_provenance");
        let missing: TargetedRerunInputFingerprint = serde_json::from_value(value)
            .map_err(|err| format!("deserialize legacy fingerprint: {err}"))?;
        assert_eq!(
            graph_provenance_unavailable_fields(&missing),
            vec![
                "package_graph_provenance_unavailable",
                "feature_graph_provenance_unavailable"
            ]
        );
        Ok(())
    }

    fn sample_input_fingerprint() -> TargetedRerunInputFingerprint {
        TargetedRerunInputFingerprint {
            schema_version: "0.3".to_string(),
            analyzer_version: "0.10.0".to_string(),
            workspace_root_hash: "root".to_string(),
            files_content_hash: "files".to_string(),
            cfg_features_hash: "features".to_string(),
            config_hash: "config".to_string(),
            test_intent_hash: "intent".to_string(),
            suppressions_hash: "suppressions".to_string(),
            workspace_manifests_hash: "manifests".to_string(),
            lockfile_hash: "lockfile".to_string(),
            toolchain_hash: "toolchain".to_string(),
            seam_limit_key: "unlimited".to_string(),
            selector_ledger_hash: "not_applicable".to_string(),
            graph_provenance: TargetedRerunGraphProvenance {
                package_graph_status: "complete".to_string(),
                package_graph_hash: Some("package-graph".to_string()),
                package_graph_detail: None,
                feature_graph_status: "complete".to_string(),
                feature_graph_hash: Some("feature-graph".to_string()),
                feature_graph_detail: None,
                external_dependency_graph_status: "unavailable".to_string(),
                external_dependency_graph_detail: "external dependency metadata is not resolved"
                    .to_string(),
            },
        }
    }

    fn baseline_fields() -> TargetedRerunSeam {
        TargetedRerunSeam {
            canonical_gap_id: Some("gap:example".to_string()),
            seam_id: "seam:example".to_string(),
            file: "src/lib.rs".to_string(),
            line: 8,
            owner: "pricing::discounted_total".to_string(),
            static_class: "weakly_gripped".to_string(),
            repair_route_readiness: baseline_readiness(),
            related_tests: Vec::new(),
            missing_discriminators: Vec::new(),
        }
    }

    fn baseline_readiness() -> RepairRouteReadiness {
        RepairRouteReadiness {
            state: RepairRouteState::Ready,
            seam_id: "seam:example".to_string(),
            canonical_gap_id: Some("gap:example".to_string()),
            required_evidence: vec!["producer-owned missing discriminator fact".to_string()],
            present_evidence: vec!["producer-owned missing discriminator fact".to_string()],
            missing_evidence: Vec::new(),
            target_selection: RepairTargetSelection::Proposed(NewTestTargetProposal {
                kind: NewTestKind::Integration,
                file: "tests/example.rs".into(),
                owner: "pricing::discounted_total".to_string(),
                provenance: NewTestProposalProvenance::ProducerOwned,
            }),
            test_target: None,
            proposed_oracle: Some(OracleKind::ExactValue),
            current_oracle: None,
            authority_boundary: REPAIR_ROUTE_AUTHORITY_BOUNDARY,
        }
    }

    #[test]
    fn targeted_rerun_seam_preserves_producer_route_readiness() -> Result<(), String> {
        let seam = seam_from(&error_variant_entry());

        let value = serde_json::to_value(&seam)
            .map_err(|error| format!("serialize targeted rerun seam: {error}"))?;
        if value["canonical_gap_id"] != value["repair_route_readiness"]["canonical_gap_id"]
            || value["seam_id"] != value["repair_route_readiness"]["seam_id"]
            || value["repair_route_readiness"]["state"] != "ready"
            || value["repair_route_readiness"]["proposed_oracle"] != "ExactErrorVariant"
            || value["repair_route_readiness"]["target_selection"]["existing"]["symbol_id"]
                .as_str()
                .is_none()
            || value["repair_route_readiness"]["test_target"]["file"] != "tests/pricing.rs"
            || value["repair_route_readiness"]["authority_boundary"]
                != REPAIR_ROUTE_AUTHORITY_BOUNDARY
            || value["missing_discriminators"][0]["value"] != "PricingError::Other"
        {
            return Err(format!(
                "targeted rerun lost producer-owned route readiness: {value}"
            ));
        }
        Ok(())
    }

    fn error_variant_entry() -> ClassifiedSeam {
        let seam = RepoSeam::new(
            "src/pricing.rs",
            "pricing::classify_boundary",
            SeamKind::ErrorVariant,
            9,
            88,
            "Err(PricingError::Other)",
            RequiredDiscriminator::ErrorVariant {
                variant: "PricingError::Other".to_string(),
            },
            ExpectedSink::ErrorChannel,
        );
        let evidence = TestGripEvidence {
            seam_id: seam.id().clone(),
            related_tests: vec![RelatedTestGrip {
                test_name: "rejects_boundary".to_string(),
                file: PathBuf::from("tests/pricing.rs"),
                line: 4,
                test_target: Some(TestTargetEvidence::fixture(
                    "rejects_boundary",
                    Path::new("tests/pricing.rs"),
                    4,
                )),
                oracle_kind: OracleKind::BroadError,
                oracle_strength: OracleStrength::Weak,
                evidence_summary: "broad error assertion".to_string(),
                relation_reason: RelationReason::DirectOwnerCall,
                relation_confidence: RelationConfidence::High,
            }],
            reach: stage("owner is reached"),
            activate: stage("exact error variant flows"),
            propagate: stage("error channel flow"),
            observe: stage("error is observed"),
            discriminate: StageEvidence::new(
                StageState::Weak,
                Confidence::Medium,
                "broad error assertion misses the exact variant",
            ),
            observed_values: Vec::new(),
            missing_discriminators: vec![MissingDiscriminatorFact {
                value: "PricingError::Other".to_string(),
                reason: "the changed error variant is not asserted exactly".to_string(),
                flow_sink: None,
            }],
        };
        let class = classify_seam(&seam, &evidence);
        ClassifiedSeam {
            seam,
            evidence,
            class,
        }
    }

    fn stage(summary: &str) -> StageEvidence {
        StageEvidence::new(StageState::Yes, Confidence::Medium, summary)
    }

    #[test]
    fn human_rerun_report_names_current_state_only_boundary() -> Result<(), String> {
        let report = TargetedRerunReport {
            schema_version: "ripr-targeted-rerun-v1",
            state: "current_state_only",
            selector: TargetedRerunSelector {
                kind: "changed_test",
                changed_test: Some("tests/pricing.rs".to_string()),
                canonical_gap_id: None,
                gap_ledger: None,
                matched_record_count: None,
                recomputed_scope_count: None,
                selected_test_count: 1,
                direct_call_names: vec!["discounted_total".to_string()],
            },
            cache: TargetedRerunCache {
                schema_version: "0.2",
                reuse_state: "reused_file_facts",
                file_fact_status: "warm".to_string(),
                hits: 2,
                misses: 0,
                corrupt_ignored: 0,
                stores: 0,
                store_errors: 0,
                recomputation_reasons: Vec::new(),
                invalidation_status: "not_available".to_string(),
                input_fingerprint: None,
            },
            seams: vec![TargetedRerunSeam {
                canonical_gap_id: Some("gap:example".to_string()),
                seam_id: "seam:example".to_string(),
                file: "src/lib.rs".to_string(),
                line: 8,
                owner: "pricing::discounted_total".to_string(),
                static_class: "weakly_gripped".to_string(),
                repair_route_readiness: baseline_readiness(),
                related_tests: Vec::new(),
                missing_discriminators: Vec::new(),
            }],
            movement: None,
            parity: None,
            route: None,
            limitation: None,
            scope_limitations: Vec::new(),
            authority_boundary: "static evidence only; no before snapshot was supplied, so gap movement is not inferred",
            parity_scope: None,
        };
        let rendered = render_human(&report);
        for expected in [
            "State: current_state_only",
            "File-fact cache: warm",
            "static evidence only",
        ] {
            if !rendered.contains(expected) {
                return Err(format!("missing {expected:?} from {rendered:?}"));
            }
        }
        Ok(())
    }

    #[test]
    fn human_rerun_report_names_explicit_static_movement() -> Result<(), String> {
        let report = TargetedRerunReport {
            schema_version: "ripr-targeted-rerun-v1",
            state: "improved",
            selector: TargetedRerunSelector {
                kind: "changed_test",
                changed_test: Some("tests/pricing.rs".to_string()),
                canonical_gap_id: None,
                gap_ledger: None,
                matched_record_count: None,
                recomputed_scope_count: None,
                selected_test_count: 1,
                direct_call_names: Vec::new(),
            },
            cache: TargetedRerunCache {
                schema_version: "0.2",
                reuse_state: "reused_file_facts",
                file_fact_status: "warm".to_string(),
                hits: 1,
                misses: 0,
                corrupt_ignored: 0,
                stores: 0,
                store_errors: 0,
                recomputation_reasons: Vec::new(),
                invalidation_status: "not_available".to_string(),
                input_fingerprint: None,
            },
            seams: Vec::new(),
            movement: Some(TargetedRerunMovement {
                state: "improved",
                before: "target/ripr/before.json".to_string(),
                before_seam_count: 1,
                matched_seam_count: 1,
            }),
            parity: None,
            route: None,
            limitation: None,
            scope_limitations: Vec::new(),
            authority_boundary: "static before/after evidence only",
            parity_scope: None,
        };
        let rendered = render_human(&report);
        for expected in [
            "State: improved",
            "Movement: improved",
            "Before: target/ripr/before.json",
        ] {
            if !rendered.contains(expected) {
                return Err(format!("missing {expected:?} from {rendered:?}"));
            }
        }
        Ok(())
    }
}
