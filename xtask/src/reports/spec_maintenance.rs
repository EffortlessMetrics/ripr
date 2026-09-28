//! Advisory, deterministic inventory of specification documents.
//!
//! This report is deliberately separate from `check-spec-format`: it explains
//! why a maintainer may want to look at a document, but never declares a
//! document invalid or changes lifecycle/support state.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::spec_receipts::{
    ReceiptInput, ReceiptObservation, ReceiptsObservation, RejectedReceipt, scan_receipt_directory,
};

const JSON_FILE: &str = "spec-maintenance.json";
const MARKDOWN_FILE: &str = "spec-maintenance.md";
const DIGEST_FILE: &str = "spec-maintenance-digest.md";
const SCHEMA_VERSION: &str = "2";
const USAGE: &str =
    "usage: cargo xtask specs maintenance --as-of YYYY-MM-DD [--json] [--receipts <dir>]";
const DIGEST_USAGE: &str =
    "usage: cargo xtask specs digest --as-of YYYY-MM-DD [--json] [--receipts <dir>]";

/// Bounded top-candidate queue rendered by the advisory digest (#3467); the
/// full report retains the complete denominator and every candidate.
const DIGEST_TOP_N: usize = 8;

/// The two maintenance states a successful observation can report (#3467).
/// Candidate presence never changes the exit status: both are useful
/// successful observations. Only an `Err` return is an instrument failure.
pub(crate) const MAINTENANCE_STATUS_CLEAN: &str = "clean";
pub(crate) const MAINTENANCE_STATUS_ATTENTION_REQUIRED: &str = "attention_required";

/// New reason codes introduced with content-bound review receipts (#3466):
/// a valid receipt whose bound content no longer matches the current spec
/// bytes reopens the finding; a time-bound waiver past its date reopens it.
pub(crate) const REASON_CONTENT_CHANGED_SINCE_REVIEW: &str = "content_changed_since_review";
pub(crate) const REASON_REVIEW_WAIVER_EXPIRED: &str = "review_waiver_expired";

const RECEIPT_STATUS_CLOSED: &str = "closed";
const RECEIPT_STATUS_STALE: &str = "stale";
const RECEIPT_STATUS_WAIVER_EXPIRED: &str = "waiver_expired";

/// The synthetic omission recorded when `docs/specs/README.md` is absent: it
/// documents an unscannable input rather than an omitted document, so it is
/// excluded from the discoverable denominator, from the digest's omitted
/// total, and from the `clean` maintenance state.
const REASON_SPEC_INDEX_MISSING: &str = "spec-index-missing";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct SpecMaintenanceReportV1 {
    pub(crate) schema_version: String,
    pub(crate) producer: String,
    pub(crate) as_of: String,
    pub(crate) history: HistoryObservation,
    pub(crate) denominator: Denominator,
    pub(crate) status_counts: BTreeMap<String, usize>,
    pub(crate) specs: Vec<SpecMaintenanceRow>,
    pub(crate) closed_specs: Vec<SpecMaintenanceRow>,
    pub(crate) closure_counts: BTreeMap<String, usize>,
    pub(crate) reason_counts: BTreeMap<String, usize>,
    pub(crate) receipts: ReceiptsObservation,
    pub(crate) limitations: Vec<String>,
    pub(crate) claim_boundary: ClaimBoundary,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct HistoryObservation {
    pub(crate) available: bool,
    pub(crate) source: String,
    pub(crate) limitation: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Denominator {
    pub(crate) discoverable: usize,
    pub(crate) included: usize,
    pub(crate) closed: usize,
    pub(crate) omitted: Vec<OmittedInput>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct OmittedInput {
    pub(crate) path: String,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct SpecMaintenanceRow {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) content_digest: String,
    pub(crate) status: String,
    pub(crate) reason_codes: Vec<String>,
    pub(crate) evidence_refs: Vec<String>,
    pub(crate) next_route: String,
    pub(crate) limitations: Vec<String>,
    pub(crate) age_observation: Option<AgeObservation>,
    pub(crate) receipt: Option<ReceiptObservation>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct AgeObservation {
    pub(crate) last_changed: String,
    pub(crate) bucket: String,
    pub(crate) days: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct ClaimBoundary {
    pub(crate) establishes: String,
    pub(crate) does_not_establish: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Options {
    as_of: String,
    json: bool,
    receipts: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HistoryInput {
    repository_available: bool,
    source: String,
    limitation: Option<String>,
    last_changed: BTreeMap<String, String>,
}

impl Default for HistoryInput {
    fn default() -> Self {
        Self {
            repository_available: false,
            source: "none".to_string(),
            limitation: Some(
                "Git/history input was unavailable; repository-only findings remain available"
                    .to_string(),
            ),
            last_changed: BTreeMap::new(),
        }
    }
}

pub(crate) fn spec_maintenance(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    let (report, json) = run_inventory(Path::new("."), &options)?;
    crate::write_report(JSON_FILE, &format!("{json}\n"))?;
    crate::write_report(MARKDOWN_FILE, &render_markdown(&report))?;
    if options.json {
        println!("{json}");
    } else {
        println!("Wrote target/ripr/reports/{JSON_FILE}");
        println!("Wrote target/ripr/reports/{MARKDOWN_FILE}");
    }
    Ok(())
}

/// Advisory digest entry point (#3467): runs the same inventory pipeline as
/// `specs maintenance`, keeps the full JSON/Markdown report as the retained
/// artifact, and renders one short bounded digest from the same DTO. The
/// digest never introduces a second parser or a second scan.
pub(crate) fn spec_digest(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{DIGEST_USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    spec_digest_report(Path::new("."), &crate::reports_dir(), &options)
}

/// The digest write pipeline, parameterized over the inventory root and the
/// report directory so tests can run it hermetically. The three report files
/// are removed before the scan: a failed rerun must not leave the previous
/// run's digest, JSON, or Markdown on disk looking current (#3586 review).
fn spec_digest_report(root: &Path, reports_dir: &Path, options: &Options) -> Result<(), String> {
    for file in [JSON_FILE, MARKDOWN_FILE, DIGEST_FILE] {
        // A stale report that cannot be removed (locked file, non-writable
        // directory) must fail the instrument before the scan: letting the
        // run proceed would leave the old digest on disk looking current.
        crate::remove_report_in(reports_dir, file)?;
    }
    let (report, json) = run_inventory(root, options)?;
    let status = maintenance_status(&report);
    crate::write_report_in(reports_dir, JSON_FILE, &format!("{json}\n"))?;
    crate::write_report_in(reports_dir, MARKDOWN_FILE, &render_markdown(&report))?;
    crate::write_report_in(reports_dir, DIGEST_FILE, &render_digest(&report))?;
    if options.json {
        println!("{json}");
    } else {
        println!("Wrote target/ripr/reports/{JSON_FILE}");
        println!("Wrote target/ripr/reports/{MARKDOWN_FILE}");
        println!("Wrote target/ripr/reports/{DIGEST_FILE}");
        // The machine-readable closing state for the advisory workflow:
        // found and none are both successful observations, distinguished by
        // value; an instrument failure never reaches this line because the
        // pipeline above returned `Err` first.
        println!("maintenance_status={status}");
    }
    Ok(())
}

/// Shared inventory pipeline for `specs maintenance` and `specs digest`:
/// one history capture, one receipt scan, one `build_report`, one JSON
/// serialization. Instrument failures (unreadable or non-UTF-8 required
/// specs, Git failures, serialization errors) surface here as `Err`.
fn run_inventory(
    root: &Path,
    options: &Options,
) -> Result<(SpecMaintenanceReportV1, String), String> {
    let history = capture_history(root)?;
    // `--receipts <dir>` overrides the receipts directory; the default is the
    // committed `.allow/spec-system/reviews` directory when it exists, and
    // absence of any receipts directory is the zero-receipt baseline.
    let receipts = match &options.receipts {
        Some(dir) => {
            let path = PathBuf::from(dir);
            if !path.exists() {
                return Err(format!(
                    "--receipts directory does not exist: `{dir}`\n{USAGE}"
                ));
            }
            ReceiptInput {
                directory: Some(path),
            }
        }
        None => {
            let default_dir = Path::new(crate::reports::spec_receipts::RECEIPTS_DEFAULT_DIR);
            ReceiptInput {
                directory: default_dir.exists().then(|| default_dir.to_path_buf()),
            }
        }
    };
    let report = build_report(root, &options.as_of, &history, &receipts)?;
    let json = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("serialize spec maintenance report: {error}"))?;
    Ok((report, json))
}

/// The advisory maintenance state of a successful observation (#3467):
/// `clean` when the open queue is empty, `attention_required` otherwise.
/// Candidate counts never make this a failure; only the pipeline's `Err`
/// path is an instrument failure. A structurally blind scan is never
/// `clean`: when the spec index itself is missing (the synthetic
/// `spec-index-missing` omission) and nothing was scanned (zero included
/// specs), the actionable item is restoring the index, so the honest state
/// is `attention_required`. A genuine zero-candidate scan (index present,
/// no findings) stays `clean`.
pub(crate) fn maintenance_status(report: &SpecMaintenanceReportV1) -> &'static str {
    let index_missing = report
        .denominator
        .omitted
        .iter()
        .any(|input| input.reason == REASON_SPEC_INDEX_MISSING);
    if report.specs.is_empty() && !index_missing {
        MAINTENANCE_STATUS_CLEAN
    } else {
        MAINTENANCE_STATUS_ATTENTION_REQUIRED
    }
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut as_of = None;
    let mut json = false;
    let mut receipts = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            // Both subcommands share one option parser; dispatch strips the
            // subcommand token, but a repeated token stays harmless.
            "maintenance" | "digest" => {}
            "--json" => json = true,
            "--as-of" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "missing value for --as-of\n".to_string() + USAGE)?;
                validate_date(value)?;
                as_of = Some(value.clone());
            }
            "--receipts" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "missing value for --receipts\n".to_string() + USAGE)?;
                receipts = Some(value.clone());
            }
            other => {
                return Err(format!(
                    "unknown specs maintenance argument `{other}`\n{USAGE}"
                ));
            }
        }
        index += 1;
    }
    let as_of = as_of.ok_or_else(|| format!("missing required --as-of\n{USAGE}"))?;
    Ok(Options {
        as_of,
        json,
        receipts,
    })
}

pub(crate) fn validate_date(value: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Err(format!("expected YYYY-MM-DD date, got `{value}`"));
    }
    let year = value[0..4]
        .parse::<i32>()
        .map_err(|error| format!("invalid year in date `{value}`: {error}"))?;
    let month = value[5..7]
        .parse::<u32>()
        .map_err(|error| format!("invalid month in date `{value}`: {error}"))?;
    let day = value[8..10]
        .parse::<u32>()
        .map_err(|error| format!("invalid day in date `{value}`: {error}"))?;
    if year < 1
        || !(1..=12).contains(&month)
        || day == 0
        || i64::from(day) > days_in_month(i64::from(year), i64::from(month))
    {
        return Err(format!("not a valid calendar date: `{value}`"));
    }
    Ok(())
}

pub(crate) fn build_report(
    root: &Path,
    as_of: &str,
    history: &HistoryInput,
    receipts: &ReceiptInput,
) -> Result<SpecMaintenanceReportV1, String> {
    validate_date(as_of)?;
    let spec_root = root.join("docs/specs");
    let files = collect_markdown_files(&spec_root)?;
    let mut included = Vec::new();
    let mut omitted = Vec::new();
    for path in files {
        let relative = relative_path(root, &path);
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if let Some(id) = parse_spec_id(name) {
            let contents = fs::read(&path)
                .map_err(|error| format!("read required spec {relative}: {error}"))?;
            let text = String::from_utf8(contents.clone())
                .map_err(|error| format!("required spec {relative} is not UTF-8: {error}"))?;
            included.push(build_row(
                root, &id, &relative, &contents, &text, as_of, history,
            ));
        } else {
            omitted.push(OmittedInput {
                path: relative,
                reason: "unsupported-or-non-discoverable-markdown-input".to_string(),
            });
        }
    }
    // The spec index is a discoverable denominator input even when absent:
    // an empty or index-less specs directory still owes the reader an account
    // of what was not scannable, per the denominator-honesty contract.
    let index_path = spec_root.join("README.md");
    if !index_path.exists() {
        omitted.push(OmittedInput {
            path: relative_path(root, &index_path),
            reason: REASON_SPEC_INDEX_MISSING.to_string(),
        });
    }
    included.sort_by(|left, right| left.id.cmp(&right.id).then(left.path.cmp(&right.path)));
    omitted.sort_by(|left, right| left.path.cmp(&right.path));

    // Receipt matching (#3466): a valid, content-bound receipt closes a
    // finding until the spec bytes change or the waiver date passes; a
    // rejected receipt closes nothing and is recorded with a named reason.
    let scan = match &receipts.directory {
        Some(directory) => Some(scan_receipt_directory(directory)?),
        None => None,
    };
    let mut rejected = scan
        .as_ref()
        .map(|scan| scan.rejected.clone())
        .unwrap_or_default();
    let mut open_specs = Vec::new();
    let mut closed_specs = Vec::new();
    for row in included {
        let Some(matched) = scan.as_ref().and_then(|scan| scan.matched.get(&row.id)) else {
            open_specs.push(row);
            continue;
        };
        // A receipt bound to another path is not evidence about this file.
        if matched.receipt.spec_path != row.path {
            rejected.push(RejectedReceipt {
                path: matched.file_path.clone(),
                reason: crate::reports::spec_receipts::REJECTED_PATH_MISMATCH.to_string(),
            });
            open_specs.push(row);
            continue;
        }
        let mut row = row;
        // A reviewed spec is by definition not "never reviewed": a matched
        // observation of any status suppresses that heuristic reason.
        row.reason_codes.retain(|reason| reason != "never_reviewed");
        let digest_matches = matched.receipt.content_digest == row.content_digest;
        // Waived-until dates are zero-padded ISO dates, so ordering is a
        // stable string comparison: `waived_until == as_of` stays closed.
        let waiver_expired = matched
            .receipt
            .waived_until
            .as_deref()
            .is_some_and(|until| until < as_of);
        let status = if digest_matches && !waiver_expired {
            RECEIPT_STATUS_CLOSED
        } else if waiver_expired {
            row.reason_codes
                .push(REASON_REVIEW_WAIVER_EXPIRED.to_string());
            RECEIPT_STATUS_WAIVER_EXPIRED
        } else {
            row.reason_codes
                .push(REASON_CONTENT_CHANGED_SINCE_REVIEW.to_string());
            RECEIPT_STATUS_STALE
        };
        row.reason_codes.sort();
        row.reason_codes.dedup();
        if row.reason_codes.is_empty() {
            row.reason_codes
                .push("no_objective_maintenance_finding".to_string());
        }
        let receipt = &matched.receipt;
        row.receipt = Some(ReceiptObservation {
            status: status.to_string(),
            receipt_id: receipt.receipt_id.clone(),
            disposition: receipt.disposition.clone(),
            observed_at: receipt.observed_at.clone(),
            waived_until: receipt.waived_until.clone(),
            reviewed_by: receipt.reviewed_by.clone(),
        });
        if status == RECEIPT_STATUS_CLOSED {
            closed_specs.push(row);
        } else {
            open_specs.push(row);
        }
    }

    let mut reason_counts = BTreeMap::new();
    let mut status_counts = BTreeMap::new();
    // Queue counts cover open findings only, so a closed spec can neither
    // inflate nor deflate the maintenance queue.
    for row in &open_specs {
        *status_counts.entry(row.status.clone()).or_insert(0) += 1;
        for reason in &row.reason_codes {
            *reason_counts.entry(reason.clone()).or_insert(0) += 1;
        }
    }
    let mut closure_counts = BTreeMap::new();
    for row in &closed_specs {
        if let Some(receipt) = &row.receipt {
            *closure_counts
                .entry(receipt.disposition.clone())
                .or_insert(0) += 1;
        }
    }
    let history = HistoryObservation {
        available: history.repository_available,
        source: history.source.clone(),
        limitation: history.limitation.clone(),
    };
    let receipts_observation = match &scan {
        Some(scan) => ReceiptsObservation {
            source: scan.source.clone(),
            parsed: scan.parsed,
            applied: closed_specs.len(),
            rejected: {
                let mut rejected = rejected;
                rejected.sort_by(|left, right| left.path.cmp(&right.path));
                rejected
            },
        },
        None => ReceiptsObservation::none(),
    };
    let mut limitations = vec![
        "age is an observation and ordering hint, not a validity or lifecycle decision".to_string(),
    ];
    if let Some(limitation) = &history.limitation {
        limitations.push(limitation.clone());
    }
    Ok(SpecMaintenanceReportV1 {
        schema_version: SCHEMA_VERSION.to_string(),
        producer: "xtask specs maintenance".to_string(),
        as_of: as_of.to_string(),
        history,
        // A present file counts toward the discoverable denominator; the
        // synthetic missing-index record documents an absent input and must
        // not inflate the count of documents the scan saw. Closed
        // observations remain discoverable and are reported separately.
        denominator: Denominator {
            discoverable: open_specs.len()
                + closed_specs.len()
                + omitted
                    .iter()
                    .filter(|input| input.reason != REASON_SPEC_INDEX_MISSING)
                    .count(),
            included: open_specs.len(),
            closed: closed_specs.len(),
            omitted,
        },
        status_counts,
        specs: open_specs,
        closed_specs,
        closure_counts,
        reason_counts,
        receipts: receipts_observation,
        limitations,
        claim_boundary: ClaimBoundary {
            establishes: "An explainable advisory queue of specification documents deserving maintainer attention".to_string(),
            does_not_establish: vec![
                "specification correctness, implementation, evidence, or support".to_string(),
                "acceptance, deprecation, supersession, migration, merge eligibility, or branch protection".to_string(),
            ],
        },
    })
}

fn collect_markdown_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    let entries =
        fs::read_dir(root).map_err(|error| format!("read {}: {error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read spec directory entry: {error}"))?;
        let path = entry.path();
        // A directory symlink can point at an ancestor: recursing through it
        // makes the scan unbounded, so symbolic links are never followed.
        let is_symlink = entry
            .file_type()
            .map(|kind| kind.is_symlink())
            .unwrap_or(false);
        if is_symlink {
            continue;
        }
        if path.is_dir() {
            files.extend(collect_markdown_files(&path)?);
        } else if path.extension().and_then(|value| value.to_str()) == Some("md") {
            files.push(path);
        }
    }
    Ok(files)
}

fn capture_history(root: &Path) -> Result<HistoryInput, String> {
    let inside =
        match crate::run::run_output_optional("git", &["rev-parse", "--is-inside-work-tree"]) {
            Ok(output) => output,
            Err(_) => return Ok(HistoryInput::default()),
        };
    let repository_available = inside.trim() == "true";
    if !repository_available {
        return Ok(HistoryInput {
            repository_available: false,
            source: "none".to_string(),
            limitation: Some(
                "Git/history input was unavailable; repository-only findings remain available"
                    .to_string(),
            ),
            last_changed: BTreeMap::new(),
        });
    }
    let mut last_changed = BTreeMap::new();
    for path in collect_markdown_files(&root.join("docs/specs"))? {
        let relative = relative_path(root, &path);
        let changed = crate::run::run_output_optional(
            "git",
            &["log", "-1", "--format=%cI", "--", relative.as_str()],
        )?;
        if let Some(date) = changed
            .lines()
            .next()
            .filter(|date| !date.trim().is_empty())
        {
            last_changed.insert(relative, date.trim().chars().take(10).collect());
        }
    }
    Ok(HistoryInput {
        repository_available: true,
        source: "captured-git-observation".to_string(),
        limitation: None,
        last_changed,
    })
}

fn age_bucket(days: i64) -> String {
    match days {
        ..=0 => "same-day-or-future".to_string(),
        1..=89 => "under-90-days".to_string(),
        90..=364 => "90-to-364-days".to_string(),
        _ => "365-days-or-more".to_string(),
    }
}

fn days_between(start: &str, end: &str) -> Option<i64> {
    let start = parse_date_parts(start)?;
    let end = parse_date_parts(end)?;
    Some(civil_days(end) - civil_days(start))
}

fn parse_date_parts(value: &str) -> Option<(i64, i64, i64)> {
    if value.len() != 10
        || value.as_bytes().get(4) != Some(&b'-')
        || value.as_bytes().get(7) != Some(&b'-')
    {
        return None;
    }
    let year = value[0..4].parse().ok()?;
    let month = value[5..7].parse().ok()?;
    let day = value[8..10].parse().ok()?;
    if month == 0 || month > 12 || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    Some((year, month, day))
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn civil_days((year, month, day): (i64, i64, i64)) -> i64 {
    let adjusted_year = year - i64::from(month <= 2);
    let era = (if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    }) / 400;
    let year_of_era = adjusted_year - era * 400;
    let month_index = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146097 + day_of_era
}

fn parse_spec_id(name: &str) -> Option<String> {
    // The repository's canonical naming is `RIPR-SPEC-NNNN-slug.md`; reuse the
    // shared identifier parser so this report and the spec gates agree on what
    // a spec file is.
    crate::spec_id_from_file_name(name)
}

fn build_row(
    root: &Path,
    id: &str,
    path: &str,
    bytes: &[u8],
    text: &str,
    as_of: &str,
    history: &HistoryInput,
) -> SpecMaintenanceRow {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = format!("sha256:{:x}", hasher.finalize());
    let status = text
        .lines()
        .find_map(|line| line.strip_prefix("Status:"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string();
    let mut reasons = Vec::new();
    let mut evidence = vec![path.to_string()];
    let mut limitations = Vec::new();
    // Reasons are derived from document structure — headings — rather than
    // bare token presence anywhere in the text (token coincidence clears a
    // spec for an unrelated mention and flags one for a link target).
    let headings: Vec<&str> = text.lines().filter(|line| line.starts_with('#')).collect();
    let has_review_heading = headings
        .iter()
        .any(|heading| heading.to_ascii_lowercase().contains("review"));
    if !has_review_heading {
        reasons.push("never_reviewed".to_string());
    }
    // The heading alone is not a mapping: an accepted spec whose
    // `## Test Mapping` section is empty or missing still owes the reader a
    // current-or-planned test mapping, so the section content decides.
    let has_test_mapping_content = text
        .split("## Test Mapping")
        .nth(1)
        .and_then(|rest| {
            rest.split(
                "
## ",
            )
            .next()
        })
        .is_some_and(|section| {
            section
                .lines()
                .any(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        });
    if status.eq_ignore_ascii_case("accepted") && !has_test_mapping_content {
        reasons.push("accepted_without_current_or_planned_test_mapping".to_string());
    }
    for link in markdown_links(text) {
        let target = root
            .join(path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&link);
        if !target.exists() {
            reasons.push("linked_artifact_missing".to_string());
            evidence.push(format!("missing:{link}"));
        }
    }
    let last_changed = history.last_changed.get(path).cloned();
    if !history.repository_available || last_changed.is_none() {
        reasons.push("history_unavailable".to_string());
        limitations.push(if history.repository_available {
            "no Git history was available for this spec".to_string()
        } else {
            "Git/history input was unavailable; repository-only findings remain available"
                .to_string()
        });
    }
    let age_observation = last_changed.and_then(|last_changed| {
        let days = days_between(last_changed.as_str(), as_of)?;
        let bucket = age_bucket(days);
        if days >= 365 {
            reasons.push("periodic_attention_due".to_string());
        }
        Some(AgeObservation {
            last_changed,
            bucket,
            days: Some(days),
        })
    });
    if reasons.is_empty() {
        reasons.push("no_objective_maintenance_finding".to_string());
    }
    reasons.sort();
    reasons.dedup();
    evidence.sort();
    evidence.dedup();
    SpecMaintenanceRow {
        id: id.to_string(),
        path: path.to_string(),
        content_digest: digest,
        status,
        reason_codes: reasons,
        evidence_refs: evidence,
        next_route: "maintainer-review/spec-maintenance".to_string(),
        limitations,
        age_observation,
        receipt: None,
    }
}

fn markdown_links(text: &str) -> Vec<String> {
    text.split("](")
        .skip(1)
        .filter_map(|part| part.split(')').next())
        .filter(|link| !link.starts_with('#') && !link.contains("://"))
        // A link target may carry a title (`path "Title"`) and a fragment
        // (`path#section`); existence is judged on the bare path only.
        .map(|link| {
            let bare = link.split_whitespace().next().unwrap_or_default();
            bare.split('#').next().unwrap_or_default().to_string()
        })
        .filter(|link| !link.is_empty())
        .collect()
}

fn relative_path(root: &Path, path: &Path) -> String {
    crate::normalize_path(path.strip_prefix(root).unwrap_or(path))
}

/// Bounded advisory digest (#3467), rendered as a pure function of the report
/// DTO: identical inputs produce byte-identical output, and the queue is
/// capped so the summary stays small while the full report keeps the whole
/// denominator.
fn render_digest(report: &SpecMaintenanceReportV1) -> String {
    let index_missing = report
        .denominator
        .omitted
        .iter()
        .any(|input| input.reason == REASON_SPEC_INDEX_MISSING);
    // The synthetic `spec-index-missing` record documents an absent input,
    // not an omitted document: the digest's omitted total excludes it (with
    // a one-line disclosure) so the rendered arithmetic matches
    // `discoverable == open + closed + omitted` (#3586 review).
    let omitted = report.denominator.omitted.len() - usize::from(index_missing);
    let mut out = String::from("# Specification maintenance digest\n\nStatus: advisory\n\n");
    out.push_str(&format!(
        "maintenance_status: {}\n\n",
        maintenance_status(report)
    ));
    out.push_str(&format!(
        "- As of: `{}`\n- Schema: `{}`\n- Discoverable: `{}` — open `{}`, closed `{}`, omitted `{}`\n",
        report.as_of,
        report.schema_version,
        report.denominator.discoverable,
        report.denominator.included,
        report.denominator.closed,
        omitted,
    ));
    if index_missing {
        out.push_str(
            "- Note: the `docs/specs/README.md` index is absent; it is recorded as\n  `spec-index-missing` in the full report and is not counted in the omitted total.\n",
        );
    }
    out.push_str(&format!(
        "- History: `{}`\n",
        if report.history.available {
            "available"
        } else {
            "unavailable"
        }
    ));
    out.push_str(&format!(
        "- Receipts: parsed `{}`, applied `{}`, rejected `{}`\n\n",
        report.receipts.parsed,
        report.receipts.applied,
        report.receipts.rejected.len(),
    ));
    out.push_str("## Status counts (open findings)\n\n| Status | Count |\n| --- | --- |\n");
    if report.status_counts.is_empty() {
        out.push_str("| — | 0 |\n");
    } else {
        for (status, count) in &report.status_counts {
            out.push_str(&format!("| `{status}` | {count} |\n"));
        }
    }
    out.push_str("\n## Reason counts (open findings)\n\n| Reason | Count |\n| --- | --- |\n");
    if report.reason_counts.is_empty() {
        out.push_str("| — | 0 |\n");
    } else {
        for (reason, count) in &report.reason_counts {
            out.push_str(&format!("| `{reason}` | {count} |\n"));
        }
    }
    out.push_str(&format!(
        "\n## Top candidates (stalest first, bounded to {DIGEST_TOP_N})\n\n| ID | Path | Status | Age bucket | Reasons |\n| --- | --- | --- | --- | --- |\n"
    ));
    for row in top_candidates(report) {
        let age = row
            .age_observation
            .as_ref()
            .map(|observation| observation.bucket.as_str())
            .unwrap_or("-");
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            row.id,
            row.path,
            row.status,
            age,
            row.reason_codes.join(", ")
        ));
    }
    if report.specs.is_empty() {
        out.push_str("| — | — | — | — | no open candidates |\n");
    }
    out.push_str("\n## Limitations\n\n");
    for limitation in &report.limitations {
        out.push_str(&format!("- {limitation}\n"));
    }
    out.push_str(
        "\nFull inventory: `target/ripr/reports/spec-maintenance.md`; the full JSON and\nMarkdown reports are retained as the workflow artifact with the complete\ndenominator and every candidate.\n\n",
    );
    out.push_str(
        "Advisory only: candidate counts never gate merges; an instrument failure is a\nfailed advisory observation, not a failed required product gate.\n",
    );
    out
}

/// The bounded top-of-queue: stalest first by observed age, rows without an
/// age observation last, ID as the deterministic tie-break.
fn top_candidates(report: &SpecMaintenanceReportV1) -> Vec<&SpecMaintenanceRow> {
    let mut rows: Vec<&SpecMaintenanceRow> = report.specs.iter().collect();
    rows.sort_by(|left_row, right_row| {
        let left_days = left_row.age_observation.as_ref().and_then(|age| age.days);
        let right_days = right_row.age_observation.as_ref().and_then(|age| age.days);
        match (left_days, right_days) {
            (Some(left), Some(right)) => right
                .cmp(&left)
                .then_with(|| left_row.id.cmp(&right_row.id)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left_row.id.cmp(&right_row.id),
        }
    });
    rows.truncate(DIGEST_TOP_N);
    rows
}

fn render_markdown(report: &SpecMaintenanceReportV1) -> String {
    let mut out = String::from("# Specification maintenance inventory\n\nStatus: advisory\n\n");
    out.push_str(&format!(
        "- Schema: `{}`\n- As of: `{}`\n- Discoverable: `{}`\n- Included: `{}`\n- Closed: `{}`\n",
        report.schema_version,
        report.as_of,
        report.denominator.discoverable,
        report.denominator.included,
        report.denominator.closed
    ));
    out.push_str(&format!(
        "- History: `{}`\n\n",
        if report.history.available {
            "available"
        } else {
            "unavailable"
        }
    ));
    out.push_str("## Candidates\n\n| ID | Path | Status | Reasons | Next route |\n| --- | --- | --- | --- | --- |\n");
    for row in &report.specs {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            row.id,
            row.path,
            row.status,
            row.reason_codes.join(", "),
            row.next_route
        ));
    }
    if report.specs.is_empty() {
        out.push_str("| — | — | — | none | — |\n");
    }
    // Closed observations stay visible with their disposition rather than
    // silently vanishing from the report.
    out.push_str("\n## Closed observations\n\n| ID | Path | Disposition | Waived until | Observed | Reviewer |\n| --- | --- | --- | --- | --- | --- |\n");
    for row in &report.closed_specs {
        let receipt = row.receipt.as_ref();
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            row.id,
            row.path,
            receipt
                .map(|value| value.disposition.as_str())
                .unwrap_or("-"),
            receipt
                .and_then(|value| value.waived_until.as_deref())
                .unwrap_or("-"),
            receipt
                .map(|value| value.observed_at.as_str())
                .unwrap_or("-"),
            receipt
                .map(|value| value.reviewed_by.as_str())
                .unwrap_or("-")
        ));
    }
    if report.closed_specs.is_empty() {
        out.push_str("| — | — | none | — | — | — |\n");
    }
    out.push_str("\n## Omitted inputs\n\n");
    if report.denominator.omitted.is_empty() {
        out.push_str("None.\n\n");
    } else {
        for input in &report.denominator.omitted {
            out.push_str(&format!("- `{}` — {}\n", input.path, input.reason));
        }
        out.push('\n');
    }
    out.push_str("## Receipt observations\n\n");
    out.push_str(&format!(
        "- Source: `{}`\n- Parsed: `{}`\n- Applied: `{}`\n\n| Receipt | Rejection reason |\n| --- | --- |\n",
        report.receipts.source, report.receipts.parsed, report.receipts.applied
    ));
    if report.receipts.rejected.is_empty() {
        out.push_str("| — | none |\n");
    } else {
        for rejected in &report.receipts.rejected {
            out.push_str(&format!(
                "| `{}` | `{}` |\n",
                rejected.path, rejected.reason
            ));
        }
    }
    out.push_str("\n## Limitations and claim boundary\n\n");
    for limitation in &report.limitations {
        out.push_str(&format!("- {}\n", limitation));
    }
    out.push_str(&format!(
        "\nEstablishes: {}.\n\nDoes not establish:\n",
        report.claim_boundary.establishes
    ));
    for claim in &report.claim_boundary.does_not_establish {
        out.push_str(&format!("- {}\n", claim));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root() -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-spec-maintenance-{stamp}"));
        fs::create_dir_all(root.join("docs/specs")).map_err(|error| error.to_string())?;
        Ok(root)
    }

    #[test]
    fn report_is_stable_and_history_limitation_is_visible() -> Result<(), String> {
        let root = fixture_root()?;
        let path = root.join("docs/specs/RIPR-SPEC-9999-example.md");
        fs::write(&path, "# Example\n\nStatus: proposed\n").map_err(|error| error.to_string())?;
        fs::write(root.join("docs/specs/README.md"), "index").map_err(|error| error.to_string())?;
        let left = build_report(
            &root,
            "2026-08-27",
            &HistoryInput::default(),
            &ReceiptInput::default(),
        )?;
        let right = build_report(
            &root,
            "2026-08-27",
            &HistoryInput::default(),
            &ReceiptInput::default(),
        )?;
        if left != right {
            return Err("fixed inputs did not produce stable report".to_string());
        }
        if !left.reason_counts.contains_key("history_unavailable") {
            return Err("history limitation was not reported".to_string());
        }
        if left.denominator.omitted.len() != 1 {
            return Err("unsupported input was not reported".to_string());
        }
        Ok(())
    }

    #[test]
    fn age_is_an_observation_and_periodic_reason_has_a_basis() -> Result<(), String> {
        let root = fixture_root()?;
        fs::write(
            root.join("docs/specs/RIPR-SPEC-9999-example.md"),
            "# Example\n\nStatus: proposed\n",
        )
        .map_err(|error| error.to_string())?;
        let mut history = HistoryInput {
            repository_available: true,
            source: "fixture".to_string(),
            ..HistoryInput::default()
        };
        history.last_changed.insert(
            "docs/specs/RIPR-SPEC-9999-example.md".to_string(),
            "2025-01-01".to_string(),
        );
        let report = build_report(&root, "2026-08-27", &history, &ReceiptInput::default())?;
        let row = report
            .specs
            .first()
            .ok_or_else(|| "missing fixture row".to_string())?;
        if row.age_observation.as_ref().map(|age| age.bucket.as_str()) != Some("365-days-or-more") {
            return Err("age bucket was not observed deterministically".to_string());
        }
        if !row
            .reason_codes
            .contains(&"periodic_attention_due".to_string())
        {
            return Err("periodic reason lacked its age basis".to_string());
        }
        if row
            .reason_codes
            .contains(&"history_unavailable".to_string())
        {
            return Err("available history was reported as unavailable".to_string());
        }
        Ok(())
    }

    #[test]
    fn accepted_without_test_mapping_content_stays_flagged() -> Result<(), String> {
        let build = |mapping_section: &str| -> Result<bool, String> {
            let root = fixture_root()?;
            fs::write(
                root.join("docs/specs/RIPR-SPEC-9999-example.md"),
                format!(
                    "# Example

Status: accepted

## Test Mapping
{mapping_section}
"
                ),
            )
            .map_err(|error| error.to_string())?;
            let report = build_report(
                &root,
                "2026-08-27",
                &HistoryInput::default(),
                &ReceiptInput::default(),
            )?;
            let row = report
                .specs
                .first()
                .ok_or_else(|| "missing fixture row".to_string())?;
            Ok(row
                .reason_codes
                .contains(&"accepted_without_current_or_planned_test_mapping".to_string()))
        };

        if !build("")? {
            return Err("empty Test Mapping section was accepted as a mapping".to_string());
        }
        if build("None yet; mapping is planned for the next slice.")? {
            return Err("planned mapping text was flagged as absent".to_string());
        }
        Ok(())
    }

    #[test]
    fn periodic_attention_threshold_is_pinned_at_365_days() -> Result<(), String> {
        let build = |last_changed: &str| -> Result<(bool,), String> {
            let root = fixture_root()?;
            let relative = "docs/specs/RIPR-SPEC-9999-example.md";
            fs::write(
                root.join(relative),
                "# Example

## Review
2026-01-02 reviewer note

Status: proposed
",
            )
            .map_err(|error| error.to_string())?;
            let mut history = HistoryInput {
                repository_available: true,
                source: "fixture".to_string(),
                ..HistoryInput::default()
            };
            history
                .last_changed
                .insert(relative.to_string(), last_changed.to_string());
            let report = build_report(&root, "2026-08-27", &history, &ReceiptInput::default())?;
            let row = report
                .specs
                .first()
                .ok_or_else(|| "missing fixture row".to_string())?;
            Ok((row
                .reason_codes
                .contains(&"periodic_attention_due".to_string()),))
        };

        let at_threshold = build("2025-08-27")?.0;
        let below_threshold = build("2025-08-28")?.0;
        if !at_threshold {
            return Err("365 days did not trigger periodic_attention_due".to_string());
        }
        if below_threshold {
            return Err("364 days unexpectedly triggered periodic_attention_due".to_string());
        }
        Ok(())
    }

    #[test]
    fn zero_and_many_denominators_succeed_and_render_from_one_dto() -> Result<(), String> {
        let empty = fixture_root()?;
        let empty_report = build_report(
            &empty,
            "2026-08-27",
            &HistoryInput::default(),
            &ReceiptInput::default(),
        )?;
        if empty_report.denominator.included != 0 || empty_report.denominator.omitted.len() != 1 {
            return Err("zero-candidate denominator was not reported".to_string());
        }
        let root = fixture_root()?;
        for id in ["0001", "0002"] {
            fs::write(
                root.join(format!("docs/specs/RIPR-SPEC-{id}-example.md")),
                "# Example\n\nStatus: planned\n",
            )
            .map_err(|error| error.to_string())?;
        }
        let report = build_report(
            &root,
            "2026-08-27",
            &HistoryInput::default(),
            &ReceiptInput::default(),
        )?;
        let json = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
        let decoded: SpecMaintenanceReportV1 =
            serde_json::from_str(&json).map_err(|error| error.to_string())?;
        if decoded != report || render_markdown(&decoded).is_empty() || report.specs.len() != 2 {
            return Err("JSON/Markdown did not use the same DTO or many case failed".to_string());
        }
        Ok(())
    }

    #[test]
    fn malformed_as_of_is_rejected() -> Result<(), String> {
        if validate_date("2026-02-31").is_ok() {
            return Err("invalid calendar date unexpectedly accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn malformed_required_source_fails_visibly() -> Result<(), String> {
        let root = fixture_root()?;
        fs::write(
            root.join("docs/specs/RIPR-SPEC-9999-example.md"),
            [0xff, 0xfe],
        )
        .map_err(|error| error.to_string())?;
        if build_report(
            &root,
            "2026-08-27",
            &HistoryInput::default(),
            &ReceiptInput::default(),
        )
        .is_ok()
        {
            return Err("invalid UTF-8 unexpectedly succeeded".to_string());
        }
        Ok(())
    }

    // --- Receipt integration (#3466) ---------------------------------------

    use crate::reports::spec_receipts::{
        RECEIPT_CLAIM_BOUNDARY, RECEIPT_NON_CLAIMS, RECEIPT_SCHEMA_VERSION,
        REJECTED_DUPLICATE_SPEC, REJECTED_MALFORMED, REJECTED_NAME_NOT_A_SPEC_ID,
        REJECTED_PATH_MISMATCH, REJECTED_SPEC_MISMATCH, REJECTED_UNKNOWN_SCHEMA,
        REJECTED_UNSUPPORTED_FILE, SpecReviewReceiptV1, compute_receipt_id,
    };

    fn reviews_dir(root: &Path) -> PathBuf {
        root.join(".allow/spec-system/reviews")
    }

    fn fixture_receipt(
        spec_id: &str,
        spec_path: &str,
        digest: &str,
        disposition: &str,
        waived_until: Option<&str>,
    ) -> SpecReviewReceiptV1 {
        let mut receipt = SpecReviewReceiptV1 {
            schema_version: RECEIPT_SCHEMA_VERSION.to_string(),
            producer: "hand-authored".to_string(),
            spec_id: spec_id.to_string(),
            spec_path: spec_path.to_string(),
            content_digest: digest.to_string(),
            status_observed: "proposed".to_string(),
            observed_at: "2026-08-01".to_string(),
            reviewed_by: "maintainer".to_string(),
            disposition: disposition.to_string(),
            waived_until: waived_until.map(str::to_string),
            disposition_detail: String::new(),
            reasons_inspected: Vec::new(),
            evidence_refs: Vec::new(),
            limitations: Vec::new(),
            receipt_id: String::new(),
            predecessor_receipt_id: None,
            claim_boundary: RECEIPT_CLAIM_BOUNDARY.to_string(),
            non_claims: RECEIPT_NON_CLAIMS
                .iter()
                .map(|claim| claim.to_string())
                .collect(),
        };
        receipt.receipt_id = compute_receipt_id(&receipt);
        receipt
    }

    fn write_receipt(
        root: &Path,
        file_name: &str,
        receipt: &SpecReviewReceiptV1,
    ) -> Result<(), String> {
        let dir = reviews_dir(root);
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let mut body =
            toml::to_string(receipt).map_err(|error| format!("serialize receipt: {error}"))?;
        if !body.ends_with('\n') {
            body.push('\n');
        }
        fs::write(dir.join(file_name), body).map_err(|error| error.to_string())
    }

    fn digest_of(root: &Path, relative: &str) -> Result<String, String> {
        let bytes = fs::read(root.join(relative)).map_err(|error| error.to_string())?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        Ok(format!("sha256:{:x}", hasher.finalize()))
    }

    fn build_with_receipts(root: &Path, as_of: &str) -> Result<SpecMaintenanceReportV1, String> {
        build_report(
            root,
            as_of,
            &HistoryInput::default(),
            &ReceiptInput {
                directory: Some(reviews_dir(root)),
            },
        )
    }

    #[test]
    fn matching_receipt_closes_finding_and_pins_denominator_arithmetic() -> Result<(), String> {
        let root = fixture_root()?;
        let closed_relative = "docs/specs/RIPR-SPEC-9001-closed.md";
        fs::write(root.join(closed_relative), "# A\n\nStatus: proposed\n")
            .map_err(|error| error.to_string())?;
        fs::write(
            root.join("docs/specs/RIPR-SPEC-9002-open.md"),
            "# B\n\nStatus: planned\n",
        )
        .map_err(|error| error.to_string())?;
        fs::write(root.join("docs/specs/README.md"), "index").map_err(|error| error.to_string())?;
        fs::write(root.join("docs/specs/notes.md"), "not a spec")
            .map_err(|error| error.to_string())?;
        let digest = digest_of(&root, closed_relative)?;
        write_receipt(
            &root,
            "RIPR-SPEC-9001.toml",
            &fixture_receipt(
                "RIPR-SPEC-9001",
                closed_relative,
                &digest,
                "current_no_source_change",
                None,
            ),
        )?;
        let report = build_with_receipts(&root, "2026-08-27")?;
        // Discoverable = 1 open + 1 closed + 2 omitted non-index inputs
        // (`README.md` and `notes.md` are present, non-discoverable files).
        if report.denominator.included != 1
            || report.denominator.closed != 1
            || report.denominator.discoverable != 4
        {
            return Err(format!(
                "denominator arithmetic drifted: {:?}",
                report.denominator
            ));
        }
        let omitted_non_index = report
            .denominator
            .omitted
            .iter()
            .filter(|input| input.reason != REASON_SPEC_INDEX_MISSING)
            .count();
        if report.denominator.discoverable
            != report.denominator.included + report.denominator.closed + omitted_non_index
        {
            return Err(
                "discoverable != included + closed + omitted(non-index-missing)".to_string(),
            );
        }
        if report.specs.len() != 1 || report.specs[0].id != "RIPR-SPEC-9002" {
            return Err("the open finding left the queue".to_string());
        }
        let closed = report
            .closed_specs
            .first()
            .ok_or_else(|| "closed observation missing".to_string())?;
        if closed.id != "RIPR-SPEC-9001"
            || closed.receipt.as_ref().map(|value| value.status.as_str()) != Some("closed")
        {
            return Err("matching receipt did not close the finding".to_string());
        }
        if report
            .closure_counts
            .get("current_no_source_change")
            .copied()
            != Some(1)
        {
            return Err("closure_counts did not cover the closed disposition".to_string());
        }
        // Queue counts stay open-only: the closed spec must not inflate or
        // deflate them.
        if report.status_counts.contains_key("proposed") {
            return Err("closed spec leaked into status_counts".to_string());
        }
        let open_reason_total: usize = report.specs.iter().map(|row| row.reason_codes.len()).sum();
        if report.reason_counts.values().sum::<usize>() != open_reason_total {
            return Err("reason_counts does not count only open findings".to_string());
        }
        if report.receipts.applied != 1 || report.receipts.parsed != 1 {
            return Err("receipts observation did not report parsed/applied".to_string());
        }
        Ok(())
    }

    #[test]
    fn spec_byte_change_reopens_finding_as_stale() -> Result<(), String> {
        let root = fixture_root()?;
        let relative = "docs/specs/RIPR-SPEC-9003-stale.md";
        fs::write(root.join(relative), "# A\n\nStatus: proposed\n")
            .map_err(|error| error.to_string())?;
        let digest = digest_of(&root, relative)?;
        write_receipt(
            &root,
            "RIPR-SPEC-9003.toml",
            &fixture_receipt(
                "RIPR-SPEC-9003",
                relative,
                &digest,
                "current_no_source_change",
                None,
            ),
        )?;
        // One content byte changes; the receipt is stale even though the
        // document status is unchanged.
        fs::write(root.join(relative), "# A!\n\nStatus: proposed\n")
            .map_err(|error| error.to_string())?;
        let report = build_with_receipts(&root, "2026-08-27")?;
        if !report.closed_specs.is_empty() {
            return Err("stale receipt still closed the finding".to_string());
        }
        let row = report
            .specs
            .first()
            .ok_or_else(|| "reopened finding missing".to_string())?;
        if !row
            .reason_codes
            .contains(&REASON_CONTENT_CHANGED_SINCE_REVIEW.to_string())
        {
            return Err("stale receipt did not add content_changed_since_review".to_string());
        }
        if row.receipt.as_ref().map(|value| value.status.as_str()) != Some("stale") {
            return Err("stale status was not observed on the row".to_string());
        }
        if row.reason_codes.contains(&"never_reviewed".to_string()) {
            return Err("a reviewed spec was reported as never_reviewed".to_string());
        }
        if report.denominator.closed != 0 || report.denominator.included != 1 {
            return Err("stale receipt changed the denominator buckets".to_string());
        }
        Ok(())
    }

    #[test]
    fn waiver_expiry_boundary_pins_both_sides() -> Result<(), String> {
        let build =
            |as_of: &str, waived_until: &str| -> Result<(bool, String, Vec<String>), String> {
                let root = fixture_root()?;
                let relative = "docs/specs/RIPR-SPEC-9004-waiver.md";
                fs::write(root.join(relative), "# A\n\nStatus: proposed\n")
                    .map_err(|error| error.to_string())?;
                let digest = digest_of(&root, relative)?;
                write_receipt(
                    &root,
                    "RIPR-SPEC-9004.toml",
                    &fixture_receipt(
                        "RIPR-SPEC-9004",
                        relative,
                        &digest,
                        "current_no_source_change",
                        Some(waived_until),
                    ),
                )?;
                let report = build_with_receipts(&root, as_of)?;
                if let Some(row) = report.closed_specs.first() {
                    return Ok((
                        true,
                        row.receipt
                            .as_ref()
                            .map(|value| value.status.clone())
                            .unwrap_or_default(),
                        Vec::new(),
                    ));
                }
                let row = report
                    .specs
                    .first()
                    .ok_or_else(|| "finding missing".to_string())?;
                Ok((
                    false,
                    row.receipt
                        .as_ref()
                        .map(|value| value.status.clone())
                        .unwrap_or_default(),
                    row.reason_codes.clone(),
                ))
            };
        let (closed_on_date, closed_status, _) = build("2026-08-27", "2026-08-27")?;
        if !closed_on_date || closed_status != "closed" {
            return Err("waived_until == as_of did not stay closed".to_string());
        }
        let (expired_closed, expired_status, expired_reasons) = build("2026-08-28", "2026-08-27")?;
        if expired_closed
            || expired_status != "waiver_expired"
            || !expired_reasons
                .iter()
                .any(|reason| reason == REASON_REVIEW_WAIVER_EXPIRED)
        {
            return Err(
                "waived_until < as_of did not reopen with review_waiver_expired".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn rejected_receipts_close_nothing_and_carry_named_reasons() -> Result<(), String> {
        let root = fixture_root()?;
        let relative = "docs/specs/RIPR-SPEC-9005-rejected.md";
        fs::write(root.join(relative), "# A\n\nStatus: proposed\n")
            .map_err(|error| error.to_string())?;
        let digest = digest_of(&root, relative)?;
        let dir = reviews_dir(&root);
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        // spec_id field disagrees with the filename.
        write_receipt(
            &root,
            "RIPR-SPEC-9006.toml",
            &fixture_receipt("RIPR-SPEC-9005", relative, &digest, "not_applicable", None),
        )?;
        // Right ID, wrong bound path.
        write_receipt(
            &root,
            "RIPR-SPEC-9005.toml",
            &fixture_receipt(
                "RIPR-SPEC-9005",
                "docs/specs/RIPR-SPEC-9005-elsewhere.md",
                &digest,
                "not_applicable",
                None,
            ),
        )?;
        // Unparsable filename and a non-TOML entry.
        fs::write(dir.join("notes.toml"), "x = 1\n").map_err(|error| error.to_string())?;
        fs::write(dir.join("RIPR-SPEC-9007.md"), "nope\n").map_err(|error| error.to_string())?;
        // Malformed TOML and an unknown schema version.
        fs::write(dir.join("RIPR-SPEC-9008.toml"), "not = toml")
            .map_err(|error| error.to_string())?;
        let mut unknown_schema =
            fixture_receipt("RIPR-SPEC-9009", relative, &digest, "not_applicable", None);
        unknown_schema.schema_version = "2".to_string();
        unknown_schema.receipt_id = compute_receipt_id(&unknown_schema);
        write_receipt(&root, "RIPR-SPEC-9009.toml", &unknown_schema)?;
        // Advisory contract: malformed receipts never fail the command.
        let report = build_with_receipts(&root, "2026-08-27")?;
        if report.specs.len() != 1 || !report.closed_specs.is_empty() {
            return Err("a rejected receipt closed a finding".to_string());
        }
        if !report.specs[0]
            .reason_codes
            .contains(&"never_reviewed".to_string())
        {
            return Err("rejected receipts must not suppress never_reviewed".to_string());
        }
        let reasons: Vec<&str> = report
            .receipts
            .rejected
            .iter()
            .map(|rejected| rejected.reason.as_str())
            .collect();
        for expected in [
            REJECTED_SPEC_MISMATCH,
            REJECTED_PATH_MISMATCH,
            REJECTED_NAME_NOT_A_SPEC_ID,
            REJECTED_UNSUPPORTED_FILE,
            REJECTED_MALFORMED,
            REJECTED_UNKNOWN_SCHEMA,
        ] {
            if !reasons.contains(&expected) {
                return Err(format!("missing rejected reason `{expected}`: {reasons:?}"));
            }
        }
        if report.receipts.applied != 0 {
            return Err("rejected receipts were counted as applied".to_string());
        }
        Ok(())
    }

    #[test]
    fn duplicate_receipt_rejects_the_later_entry_and_keeps_the_first() -> Result<(), String> {
        let root = fixture_root()?;
        let relative = "docs/specs/RIPR-SPEC-9010-dup.md";
        fs::write(root.join(relative), "# A\n\nStatus: proposed\n")
            .map_err(|error| error.to_string())?;
        let digest = digest_of(&root, relative)?;
        write_receipt(
            &root,
            "RIPR-SPEC-9010.toml",
            &fixture_receipt("RIPR-SPEC-9010", relative, &digest, "not_applicable", None),
        )?;
        write_receipt(
            &root,
            "RIPR-SPEC-9010-second.toml",
            &fixture_receipt(
                "RIPR-SPEC-9010",
                relative,
                &digest,
                "followup_issue_required",
                None,
            ),
        )?;
        let report = build_with_receipts(&root, "2026-08-27")?;
        // The first receipt in sorted path order wins (`-` sorts before `.`,
        // so `RIPR-SPEC-9010-second.toml` is applied).
        if report.closed_specs.len() != 1
            || report.closed_specs[0]
                .receipt
                .as_ref()
                .map(|value| value.disposition.as_str())
                != Some("followup_issue_required")
        {
            return Err("duplicate handling did not keep the first sorted receipt".to_string());
        }
        if !report
            .receipts
            .rejected
            .iter()
            .any(|rejected| rejected.reason == REJECTED_DUPLICATE_SPEC)
        {
            return Err("duplicate receipt was not rejected by name".to_string());
        }
        if report.receipts.parsed != 2 || report.receipts.applied != 1 {
            return Err("parsed/applied did not disclose the duplicate".to_string());
        }
        Ok(())
    }

    #[test]
    fn zero_receipts_keeps_the_pinned_baseline_and_reports_schema_two() -> Result<(), String> {
        let root = fixture_root()?;
        fs::write(
            root.join("docs/specs/RIPR-SPEC-9011-example.md"),
            "# Example\n\nStatus: planned\n",
        )
        .map_err(|error| error.to_string())?;
        let report = build_report(
            &root,
            "2026-08-27",
            &HistoryInput::default(),
            &ReceiptInput::default(),
        )?;
        if report.schema_version != "2" {
            return Err("schema_version was not honestly bumped to 2".to_string());
        }
        if report.receipts.source != "none"
            || report.receipts.parsed != 0
            || report.receipts.applied != 0
            || !report.receipts.rejected.is_empty()
        {
            return Err("zero-receipt baseline was not reported as none".to_string());
        }
        if !report.closed_specs.is_empty()
            || !report.closure_counts.is_empty()
            || report.denominator.closed != 0
        {
            return Err("zero receipts produced closed observations".to_string());
        }
        // The markdown renders the new sections from the same DTO.
        let markdown = render_markdown(&report);
        for section in [
            "## Closed observations",
            "## Receipt observations",
            "- Closed: `0`",
        ] {
            if !markdown.contains(section) {
                return Err(format!("markdown is missing `{section}`"));
            }
        }
        Ok(())
    }

    #[test]
    fn receipts_render_from_one_dto_and_stay_byte_stable() -> Result<(), String> {
        let root = fixture_root()?;
        let closed_relative = "docs/specs/RIPR-SPEC-9012-closed.md";
        fs::write(root.join(closed_relative), "# A\n\nStatus: proposed\n")
            .map_err(|error| error.to_string())?;
        let digest = digest_of(&root, closed_relative)?;
        write_receipt(
            &root,
            "RIPR-SPEC-9012.toml",
            &fixture_receipt(
                "RIPR-SPEC-9012",
                closed_relative,
                &digest,
                "retain_proposed_named_evidence_gap",
                Some("2027-02-28"),
            ),
        )?;
        let left = build_with_receipts(&root, "2026-08-27")?;
        let right = build_with_receipts(&root, "2026-08-27")?;
        if left != right {
            return Err("fixed inputs did not produce a stable report".to_string());
        }
        let json = serde_json::to_string_pretty(&left).map_err(|error| error.to_string())?;
        let decoded: SpecMaintenanceReportV1 =
            serde_json::from_str(&json).map_err(|error| error.to_string())?;
        if decoded != left {
            return Err("JSON round trip did not preserve the report".to_string());
        }
        let markdown = render_markdown(&left);
        for expected in [
            "| `RIPR-SPEC-9012` |",
            "| `retain_proposed_named_evidence_gap` |",
            "| `2027-02-28` |",
            "| `maintainer` |",
            "- Closed: `1`",
        ] {
            if !markdown.contains(expected) {
                return Err(format!("closed section is missing `{expected}`"));
            }
        }
        // Receipt rejections stay sorted by path.
        let rejected_paths: Vec<&str> = left
            .receipts
            .rejected
            .iter()
            .map(|rejected| rejected.path.as_str())
            .collect();
        let mut sorted_paths = rejected_paths.clone();
        sorted_paths.sort();
        if rejected_paths != sorted_paths {
            return Err("rejected receipts are not sorted by path".to_string());
        }
        Ok(())
    }

    // --- Advisory digest (#3467) --------------------------------------------

    fn write_aged_spec(root: &Path, id: &str) -> Result<(), String> {
        let relative = format!("docs/specs/RIPR-SPEC-{id}-digest.md");
        fs::write(
            root.join(&relative),
            "# Example\n\n## Review\nnote\n\nStatus: proposed\n",
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn digest_row_count(digest: &str) -> usize {
        let table = match digest.split("## Top candidates").nth(1) {
            Some(table) => table,
            None => return 0,
        };
        table
            .lines()
            .filter(|line| line.starts_with("| `RIPR-SPEC-"))
            .count()
    }

    #[test]
    fn digest_is_deterministic_and_bounded_from_one_dto() -> Result<(), String> {
        let root = fixture_root()?;
        let mut history = HistoryInput {
            repository_available: true,
            source: "fixture".to_string(),
            ..HistoryInput::default()
        };
        let ages = [
            ("0001", "2020-01-01"),
            ("0002", "2021-06-01"),
            ("0003", "2022-03-01"),
            ("0004", "2023-09-01"),
            ("0005", "2024-02-01"),
            ("0006", "2025-01-01"),
            ("0007", "2025-05-01"),
            ("0008", "2025-07-01"),
            ("0009", "2026-01-01"),
            ("0010", "2026-08-01"),
        ];
        for (id, changed) in ages {
            write_aged_spec(&root, id)?;
            history.last_changed.insert(
                format!("docs/specs/RIPR-SPEC-{id}-digest.md"),
                changed.to_string(),
            );
        }
        let report = build_report(&root, "2026-08-27", &history, &ReceiptInput::default())?;
        let left = render_digest(&report);
        let right = render_digest(&report);
        if left != right {
            return Err("the same DTO rendered two different digests".to_string());
        }
        // The queue is bounded even when the denominator is not.
        if report.specs.len() != ages.len() {
            return Err("fixture denominator did not match the written specs".to_string());
        }
        if digest_row_count(&left) != DIGEST_TOP_N {
            return Err(format!(
                "digest queue was not bounded to {DIGEST_TOP_N}: {}",
                digest_row_count(&left)
            ));
        }
        // Stalest first: the 2020 spec leads, the 2026 spec is absent.
        let queue = left.split("## Top candidates").nth(1).unwrap_or_default();
        if !queue.contains("RIPR-SPEC-0001") || queue.contains("RIPR-SPEC-0009") {
            return Err("digest queue is not ordered stalest first".to_string());
        }
        if !left.contains("maintenance_status: attention_required") {
            return Err("digest did not report the attention_required state".to_string());
        }
        Ok(())
    }

    #[test]
    fn digest_pins_found_and_none_as_two_successful_states() -> Result<(), String> {
        if MAINTENANCE_STATUS_CLEAN != "clean"
            || MAINTENANCE_STATUS_ATTENTION_REQUIRED != "attention_required"
        {
            return Err("maintenance status vocabulary drifted".to_string());
        }
        // State 1: candidates found is a useful successful observation.
        let root = fixture_root()?;
        fs::write(
            root.join("docs/specs/RIPR-SPEC-9501-found.md"),
            "# Example\n\nStatus: planned\n",
        )
        .map_err(|error| error.to_string())?;
        let found = build_report(
            &root,
            "2026-08-27",
            &HistoryInput::default(),
            &ReceiptInput::default(),
        )?;
        if maintenance_status(&found) != MAINTENANCE_STATUS_ATTENTION_REQUIRED {
            return Err("candidates found did not report attention_required".to_string());
        }
        let found_digest = render_digest(&found);
        if !found_digest.contains("maintenance_status: attention_required")
            || !found_digest.contains("| `never_reviewed` | 1 |")
        {
            return Err("found-state digest is missing the status or reason counts".to_string());
        }
        // State 2: no candidates is also a useful successful observation —
        // but only when the scan actually saw the denominator: the index is
        // present and the queue is empty.
        let empty = fixture_root()?;
        fs::write(empty.join("docs/specs/README.md"), "index")
            .map_err(|error| error.to_string())?;
        let empty_report = build_report(
            &empty,
            "2026-08-27",
            &HistoryInput::default(),
            &ReceiptInput::default(),
        )?;
        if maintenance_status(&empty_report) != MAINTENANCE_STATUS_CLEAN {
            return Err("an empty queue with a present index did not report clean".to_string());
        }
        let clean_digest = render_digest(&empty_report);
        if !clean_digest.contains("maintenance_status: clean")
            || !clean_digest.contains("no open candidates")
        {
            return Err("clean-state digest is missing the empty-queue rendering".to_string());
        }
        // The present index is not an omission, so no suppression note.
        if clean_digest.contains("not counted in the omitted total") {
            return Err(
                "clean-state digest disclosed a suppressed omission it does not have".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn digest_blind_scan_without_index_is_attention_required_not_clean() -> Result<(), String> {
        // docs/specs exists but is empty and the index is absent: nothing was
        // scanned, so the honest state is attention_required — the actionable
        // item is restoring the index — never clean.
        let root = fixture_root()?;
        let report = build_report(
            &root,
            "2026-08-27",
            &HistoryInput::default(),
            &ReceiptInput::default(),
        )?;
        if !report.specs.is_empty() || report.denominator.included != 0 {
            return Err("blind-scan fixture unexpectedly scanned documents".to_string());
        }
        if !report
            .denominator
            .omitted
            .iter()
            .any(|input| input.reason == REASON_SPEC_INDEX_MISSING)
        {
            return Err("blind-scan fixture lacks the synthetic index omission".to_string());
        }
        if maintenance_status(&report) != MAINTENANCE_STATUS_ATTENTION_REQUIRED {
            return Err("a structurally blind scan was reported clean".to_string());
        }
        if !render_digest(&report).contains("maintenance_status: attention_required") {
            return Err(
                "digest did not render the blind-scan state as attention_required".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn digest_failed_rerun_removes_stale_report_artifacts() -> Result<(), String> {
        // A previous successful run left its three artifacts on disk; the
        // rerun's inventory is untrustworthy (non-UTF-8 required spec), so
        // the failed instrument must leave no stale files behind.
        let root = fixture_root()?;
        fs::write(
            root.join("docs/specs/RIPR-SPEC-9503-broken.md"),
            [0xff, 0xfe],
        )
        .map_err(|error| error.to_string())?;
        let reports = root.join("target/ripr/reports");
        fs::create_dir_all(&reports).map_err(|error| error.to_string())?;
        for file in [JSON_FILE, MARKDOWN_FILE, DIGEST_FILE] {
            fs::write(reports.join(file), "stale").map_err(|error| error.to_string())?;
        }
        let options = Options {
            as_of: "2026-08-27".to_string(),
            json: false,
            receipts: None,
        };
        let outcome = spec_digest_report(&root, &reports, &options);
        if outcome.is_ok() {
            return Err("a broken inventory did not fail the digest rerun".to_string());
        }
        for file in [JSON_FILE, MARKDOWN_FILE, DIGEST_FILE] {
            if reports.join(file).exists() {
                return Err(format!(
                    "stale `{file}` remained after a failed digest rerun"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn digest_omitted_count_excludes_the_synthetic_index_record() -> Result<(), String> {
        // Without the index: the synthetic omission is disclosed in a note,
        // not counted, so the rendered arithmetic
        // `Discoverable == open + closed + omitted` still holds.
        let blind = fixture_root()?;
        let blind_report = build_report(
            &blind,
            "2026-08-27",
            &HistoryInput::default(),
            &ReceiptInput::default(),
        )?;
        if blind_report.denominator.discoverable != 0 {
            return Err("blind-scan discoverable was not zero".to_string());
        }
        let blind_digest = render_digest(&blind_report);
        if !blind_digest.contains("omitted `0`")
            || !blind_digest.contains("not counted in the omitted total")
            || !blind_digest.contains("spec-index-missing")
        {
            return Err(
                "digest did not suppress and disclose the synthetic index omission".to_string(),
            );
        }
        // With the index present and one non-spec Markdown file: the real
        // omissions are counted (`README.md` and `notes.md` are present,
        // non-discoverable inputs) and no suppression note appears.
        let indexed = fixture_root()?;
        fs::write(indexed.join("docs/specs/README.md"), "index")
            .map_err(|error| error.to_string())?;
        fs::write(indexed.join("docs/specs/notes.md"), "not a spec")
            .map_err(|error| error.to_string())?;
        let indexed_report = build_report(
            &indexed,
            "2026-08-27",
            &HistoryInput::default(),
            &ReceiptInput::default(),
        )?;
        if indexed_report.denominator.discoverable != 2 {
            return Err("indexed-scan discoverable did not count the omitted files".to_string());
        }
        let indexed_digest = render_digest(&indexed_report);
        if !indexed_digest.contains("Discoverable: `2`")
            || !indexed_digest.contains("omitted `2`")
            || indexed_digest.contains("not counted in the omitted total")
        {
            return Err(
                "digest miscounted a real omission or invented a suppression note".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn digest_instrument_failure_is_a_command_error_not_a_status() -> Result<(), String> {
        // State 3: an untrustworthy report (unreadable/non-UTF-8 required
        // spec) makes the pipeline return `Err`, so no maintenance_status is
        // printed and the advisory workflow observes a failed instrument.
        let root = fixture_root()?;
        fs::write(
            root.join("docs/specs/RIPR-SPEC-9502-broken.md"),
            [0xff, 0xfe],
        )
        .map_err(|error| error.to_string())?;
        let outcome = build_report(
            &root,
            "2026-08-27",
            &HistoryInput::default(),
            &ReceiptInput::default(),
        );
        if outcome.is_ok() {
            return Err("an untrustworthy inventory did not fail the instrument".to_string());
        }
        // The status helper only classifies successful observations: the
        // `Err` from the shared pipeline is the instrument-failure state,
        // and the digest file is never written from a failed run because
        // `spec_digest` propagates the error before any write_report call.
        Ok(())
    }

    #[test]
    fn source_of_truth_workflow_publishes_the_advisory_digest() -> Result<(), String> {
        let workflow_path = crate::repo_root()?.join(".github/workflows/source-of-truth.yml");
        let text = fs::read_to_string(&workflow_path)
            .map_err(|error| format!("read {}: {error}", workflow_path.display()))?;

        // Trigger 1: bounded schedule, no more frequent than weekly. The
        // predicate rejects comma lists and ranges: `37 6 * * 1-5` fires
        // five times a week and `37 6 1-31 * *` fires daily.
        let cron_line = text
            .lines()
            .map(str::trim)
            .find_map(|line| line.strip_prefix("- cron:"))
            .ok_or_else(|| "source-of-truth.yml is missing its schedule cron".to_string())?
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_string();
        if !is_weekly_or_slower_cron(&cron_line) {
            return Err(format!(
                "cron `{cron_line}` is not a bounded weekly-or-slower schedule"
            ));
        }
        // Trigger 2: explicit operator invocation.
        if !text.contains("  workflow_dispatch:") {
            return Err("source-of-truth.yml is missing workflow_dispatch".to_string());
        }
        // Trigger 3: PR execution only when spec-governance paths change.
        for event in ["pull_request", "push"] {
            let paths = workflow_event_paths(&text, event)?;
            for required in [
                ".allow/spec-system/**",
                ".github/workflows/source-of-truth.yml",
                ".ripr/traceability.toml",
                "docs/specs/**",
                "docs/status/SUPPORT_TIERS.md",
                "docs/templates/**",
            ] {
                if !paths.iter().any(|path| path == required) {
                    return Err(format!(
                        "{event} trigger is missing spec-governance path `{required}`: {paths:?}"
                    ));
                }
            }
        }

        // The digest job stays advisory: a failed instrument is a failed
        // advisory observation, never a failed required gate.
        let job = workflow_job_block(&text, "  spec-maintenance-digest:").ok_or_else(|| {
            "source-of-truth.yml is missing the spec-maintenance-digest job".to_string()
        })?;
        if !job.contains("continue-on-error: true") {
            return Err(
                "the digest job is not advisory (missing continue-on-error: true)".to_string(),
            );
        }
        if !job.contains("cancel-in-progress: false") {
            return Err(
                "the digest job must queue a nearly complete scheduled inventory, not cancel it"
                    .to_string(),
            );
        }
        let digest_step = job
            .lines()
            .find(|line| line.trim().starts_with("run: cargo xtask specs digest"))
            .ok_or_else(|| "the digest job does not invoke cargo xtask specs digest".to_string())?;
        if digest_step.contains("|| true") {
            return Err(
                "the digest instrument must not be shielded: a broken instrument must stay visible"
                    .to_string(),
            );
        }
        if !job.contains("if: always()") {
            return Err(
                "the digest job must publish its summary (or instrument-failure annotation) with if: always()".to_string(),
            );
        }
        if !job.contains("actions/upload-artifact@v7") || !job.contains("ripr-spec-maintenance") {
            return Err(
                "the digest job must retain the full report as the ripr-spec-maintenance artifact"
                    .to_string(),
            );
        }
        // Backticks inside a double-quoted echo run as shell command
        // substitution: `echo "Head: `sha`"` executes the SHA as a command
        // and silently publishes an empty value. Every backtick-bearing
        // echo must be single-quoted or escape its backticks.
        for line in text.lines() {
            if double_quoted_echo_has_unescaped_backtick(line) {
                return Err(format!(
                    "workflow echo would run command substitution inside double quotes: {line}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn weekly_cadence_predicate_accepts_only_single_shot_weekly_schedules() -> Result<(), String> {
        for passing in ["37 6 * * 1", "0 9 * * 1"] {
            if !is_weekly_or_slower_cron(passing) {
                return Err(format!("`{passing}` should pass the weekly predicate"));
            }
        }
        for failing in [
            "37 6 * * 1-5",  // five runs per week
            "37 6 * * 1,3",  // twice per week
            "37 6 1-31 * *", // daily
            "37 6 */2 * 1",  // stepped minutes
            "* * * * *",     // every minute
            "37 6 * * *",    // daily via wildcard day-of-week
            "37 6 * * 1-7",  // daily via a full day-of-week range
        ] {
            if is_weekly_or_slower_cron(failing) {
                return Err(format!("`{failing}` should fail the weekly predicate"));
            }
        }
        Ok(())
    }

    /// A bounded weekly-or-slower cron: a single minute (0-59) and hour
    /// (0-23), `*` for day-of-month and month, and a day-of-week that is
    /// exactly one integer 0-6. Comma lists and ranges in the day-of-week
    /// field fire more often than weekly; a non-`*` day-of-month or a
    /// non-`*` month is more frequent than weekly; step values (`*/2`) are
    /// not a single fire time and are rejected.
    fn is_weekly_or_slower_cron(cron: &str) -> bool {
        let fields: Vec<&str> = cron.split_whitespace().collect();
        if fields.len() != 5 {
            return false;
        }
        let single_bounded_integer = |field: &str, max: u32| -> bool {
            field
                .parse::<u32>()
                .map(|value| value <= max)
                .unwrap_or(false)
        };
        single_bounded_integer(fields[0], 59)
            && single_bounded_integer(fields[1], 23)
            && fields[2] == "*"
            && fields[3] == "*"
            && single_bounded_integer(fields[4], 6)
    }

    /// True when an `echo "..."` line carries a backtick the shell would
    /// treat as command substitution (an unescaped backtick inside the
    /// double quotes). Escaped backticks (`\``) render literally and are
    /// safe.
    fn double_quoted_echo_has_unescaped_backtick(line: &str) -> bool {
        let Some(rest) = line.trim_start().strip_prefix("echo \"") else {
            return false;
        };
        let Some(end) = rest.rfind('"') else {
            return false;
        };
        let payload = &rest[..end];
        payload.match_indices('`').any(|(index, _)| {
            let backslashes = payload[..index]
                .chars()
                .rev()
                .take_while(|&character| character == '\\')
                .count();
            // An even run of backslashes does not escape the backtick.
            backslashes % 2 == 0
        })
    }

    /// Collects the `paths:` entries of one `on:` event block. Deliberate
    /// line-scanning: check-workflows reads workflows the same way and the
    /// repo does not take a YAML dependency.
    fn workflow_event_paths(text: &str, event: &str) -> Result<Vec<String>, String> {
        let mut paths = Vec::new();
        let mut in_event = false;
        let mut in_paths = false;
        for line in text.lines() {
            let indent = line.len() - line.trim_start().len();
            let trimmed = line.trim();
            if indent == 0 {
                if in_event {
                    break;
                }
                if trimmed == "on:" {
                    continue;
                }
            }
            if indent == 2 {
                if trimmed == format!("{event}:") {
                    in_event = true;
                    continue;
                }
                if in_event {
                    break;
                }
            }
            if !in_event {
                continue;
            }
            if indent == 4 {
                in_paths = trimmed == "paths:";
                continue;
            }
            if in_paths
                && indent > 4
                && let Some(entry) = trimmed.strip_prefix("- ")
            {
                paths.push(entry.trim_matches('\'').trim_matches('"').to_string());
            }
        }
        if !in_event && paths.is_empty() {
            return Err(format!(
                "source-of-truth.yml has no `{event}` trigger block"
            ));
        }
        Ok(paths)
    }

    /// Returns the body of one top-level job block (two-space indented key).
    fn workflow_job_block(text: &str, job_key: &str) -> Option<String> {
        let mut body = Vec::new();
        let mut in_job = false;
        for line in text.lines() {
            let indent = line.len() - line.trim_start().len();
            if indent == 0 && !line.trim().is_empty() {
                in_job = false;
            }
            if indent == 2 && line.trim_end() == job_key.trim_end() {
                in_job = true;
                continue;
            }
            if in_job {
                if indent == 2 && !line.trim().is_empty() {
                    break;
                }
                body.push(line);
            }
        }
        if body.is_empty() {
            None
        } else {
            Some(body.join("\n"))
        }
    }
}
