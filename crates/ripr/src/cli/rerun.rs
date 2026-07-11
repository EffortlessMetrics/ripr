use crate::analysis::{
    canonical_gap::canonical_gap_identity,
    inventory_changed_test_classified_seams_at_with_config_node,
    inventory_classified_seams_at_with_config,
    inventory_diff_scoped_classified_seams_at_with_config,
};
use crate::cli::commands_context::{ensure_command_root, load_root_input_and_config};
use crate::cli::help;
use crate::cli::parse::expect_value;
use crate::output::gap_decision_ledger::{GapRecord, parse_gap_record_source_json};
use crate::output::outcome::{TargetedRerunStaticSeam, targeted_rerun_movement_from_json};
use serde::Serialize;
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
    invalidation_status: &'static str,
}

#[derive(Serialize)]
struct TargetedRerunSeam {
    canonical_gap_id: Option<String>,
    seam_id: String,
    file: String,
    line: usize,
    owner: String,
    static_class: String,
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
    selected_seam_count: usize,
    matched_seam_count: usize,
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
    Ok(report(
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
    ))
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
    let mut seams = BTreeMap::<String, TargetedRerunSeam>::new();
    for scope in scopes {
        let changed_files = vec![scope.file.clone()];
        let changed_owner_names = scope.owner.iter().cloned().collect::<Vec<_>>();
        let inventory = inventory_diff_scoped_classified_seams_at_with_config(
            root,
            config,
            &changed_files,
            &changed_owner_names,
        )?;
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
            for record_index in record_indexes_for_scope(&records, &scope) {
                scope_limitations.push(TargetedRerunScopeLimitation {
                    kind: "gap_scope_unresolved",
                    record_index,
                    message: format!(
                        "anchored scope {} no longer contains canonical gap `{canonical_gap_id}`",
                        display_scope(&scope)
                    ),
                });
            }
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
    Ok(report(
        "current_state_only",
        selector,
        cache_from(&cache, ["selected_gap_scopes_recomputed"]),
        seams.into_values().collect(),
        Some(route_from_gap_records(&records)),
        None,
        scope_limitations,
    ))
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
    let full_by_id = full
        .iter()
        .map(seam_from)
        .map(|seam| (seam.seam_id.clone(), seam))
        .collect::<BTreeMap<_, _>>();
    let mismatches = report
        .seams
        .iter()
        .filter(|targeted| {
            full_by_id.get(&targeted.seam_id).is_none_or(|full| {
                full.canonical_gap_id != targeted.canonical_gap_id
                    || full.static_class != targeted.static_class
                    || full.file != targeted.file
                    || full.owner != targeted.owner
            })
        })
        .count();
    let matched = report.seams.len().saturating_sub(mismatches);
    report.parity = Some(TargetedRerunParity {
        state: if limit_info.is_none() && mismatches == 0 {
            "matched"
        } else {
            "limited"
        },
        selected_seam_count: report.seams.len(),
        matched_seam_count: matched,
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
    } else if mismatches > 0 {
        report.state = "limited";
        report.limitation = Some(TargetedRerunLimitation {
            kind: "full_pipeline_parity_mismatch",
            message: format!(
                "{mismatches} selected seam(s) differed from the full pipeline; targeted evidence is not a successful parity result"
            ),
        });
    }
    Ok(())
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
            invalidation_status: "not_available",
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
        invalidation_status: "not_available",
    }
}

fn seams_from(entries: &[crate::analysis::ClassifiedSeam]) -> Vec<TargetedRerunSeam> {
    entries.iter().map(seam_from).collect()
}

fn seam_from(entry: &crate::analysis::ClassifiedSeam) -> TargetedRerunSeam {
    TargetedRerunSeam {
        canonical_gap_id: canonical_gap_identity(entry).map(|identity| identity.id),
        seam_id: entry.seam.id().as_str().to_string(),
        file: display_path(entry.seam.file()),
        line: entry.seam.display_line(),
        owner: entry.seam.owner().to_string(),
        static_class: entry.class.as_str().to_string(),
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
        RerunSelector, TargetedRerunCache, TargetedRerunMovement, TargetedRerunReport,
        TargetedRerunSeam, TargetedRerunSelector, cache_from, entry_matches_selected_gap,
        parse_options, render_human, resolve_gap_records, route_from_gap_records, same_root,
        scopes_from_gap_records,
    };
    use crate::analysis::seam_cache::FileFactCacheStats;
    use crate::output::gap_decision_ledger::{GapAnchor, GapRecord};
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
                invalidation_status: "not_available",
            },
            seams: vec![TargetedRerunSeam {
                canonical_gap_id: Some("gap:example".to_string()),
                seam_id: "seam:example".to_string(),
                file: "src/lib.rs".to_string(),
                line: 8,
                owner: "pricing::discounted_total".to_string(),
                static_class: "weakly_gripped".to_string(),
            }],
            movement: None,
            parity: None,
            route: None,
            limitation: None,
            scope_limitations: Vec::new(),
            authority_boundary: "static evidence only; no before snapshot was supplied, so gap movement is not inferred",
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
                invalidation_status: "not_available",
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
