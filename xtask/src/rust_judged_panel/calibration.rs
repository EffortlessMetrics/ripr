//! Exact targeted runtime calibration and static×runtime scorecard (#4795).
//!
//! Joins independently accepted #3806 structural judgments to bounded
//! calibration receipts by exact identity. Runtime results never rewrite
//! judgment bytes. PR CI validates retained fixtures offline.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::release_judgments::{self, ReleaseJudgments};
use super::{RELEASE_SELECTION_PATH, RustJudgedPanelItem, RustJudgedPanelManifest};

mod classify;
mod eligibility;
#[cfg(test)]
mod execute;
mod receipts;
mod scorecard;

#[cfg(test)]
mod tests;

use classify::{AttemptFacts, classify_attempt, not_run};
use eligibility::{Eligibility, eligibility_for};
use receipts::{
    claimed_caught_or_survived_without_subjects, digest_file, identity_matches, load_receipts_at,
    selector_matches,
};
use scorecard::{RenderedScorecard, build_scorecard, output_paths, verify_stored, write_scorecard};

const RERUN: &str = "cargo xtask rust-judged-panel calibrate";
const CALIBRATE_USAGE: &str = "calibrate [--records <dir>] [--out <dir>] [--check]";
pub(super) const RETAINED_SCORECARD_JSON: &str =
    "metrics/rust-judged-behavior-panel/calibration-scorecard.json";
pub(super) const RETAINED_SCORECARD_MD: &str =
    "metrics/rust-judged-behavior-panel/calibration-scorecard.md";
const DEFAULT_RECORDS: &str = "metrics/rust-judged-behavior-panel/calibration-receipts";
const DEFAULT_OUT: &str = "target/ripr/rust-judged-panel/calibration";
const RELEASE_JUDGMENTS_PATH: &str = release_judgments::RELEASE_JUDGMENTS_PATH;
const ROLLING_OBSERVATION_PATH: &str =
    "metrics/rust-judged-behavior-panel/rolling-observation.json";

pub(super) const SCOPE_AUTHORIZED: &str = "authorized";
pub(super) const RESULT_CAUGHT: &str = "caught";
pub(super) const RESULT_SURVIVED: &str = "survived";
pub(super) const RESULT_INCONCLUSIVE: &str = "inconclusive";
pub(super) const RESULT_EQUIVALENT: &str = "equivalent_or_unusable";
pub(super) const RESULT_NOT_RUN: &str = "not_run";
pub(super) const RESULT_INSTRUMENT: &str = "instrument_failure";
pub(super) const RESULT_STALE: &str = "stale_or_wrong_subject";
pub(super) const RUNTIME_RESULTS: [&str; 7] = [
    RESULT_CAUGHT,
    RESULT_SURVIVED,
    RESULT_INCONCLUSIVE,
    RESULT_EQUIVALENT,
    RESULT_NOT_RUN,
    RESULT_INSTRUMENT,
    RESULT_STALE,
];
pub(super) const STATIC_DIRECTIONS: [&str; 3] = ["should_gap", "should_stay_quiet", "should_limit"];
pub(super) const INSTRUMENT_TIMEOUT: &str = "timeout";
pub(super) const INSTRUMENT_COMPILE: &str = "compile_failure";
pub(super) const INSTRUMENT_PROCESS: &str = "process_failure";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct BehaviorIdentity {
    pub(super) file: String,
    pub(super) line: u64,
    pub(super) owner: String,
    pub(super) changed_behavior: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Selector {
    pub(super) package: String,
    pub(super) target: String,
    pub(super) filter: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct SubjectCounts {
    pub(super) intended: u64,
    pub(super) discovered: u64,
    pub(super) selected: u64,
    pub(super) executed: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct JudgedCase {
    pub(super) case_id: String,
    pub(super) expected_direction: String,
    pub(super) terminal: String,
    pub(super) behavior_family: String,
    pub(super) relation_basis: String,
    pub(super) oracle_class: String,
    pub(super) witness_completeness: String,
    pub(super) repository: String,
    pub(super) base: String,
    pub(super) head: String,
    pub(super) tree_identity: String,
    pub(super) scope_authorization: String,
    pub(super) no_focused_mutant: bool,
    pub(super) behavior: BehaviorIdentity,
    pub(super) required_selector: Option<Selector>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CalibrationReceipt {
    pub(super) case_id: String,
    pub(super) selection_sha256: String,
    pub(super) judgments_sha256: String,
    pub(super) repository: String,
    pub(super) base: String,
    pub(super) head: String,
    pub(super) tree_identity: String,
    pub(super) behavior: BehaviorIdentity,
    pub(super) expected_direction: String,
    pub(super) terminal: String,
    pub(super) mutant_operator: String,
    pub(super) mutant_range: String,
    pub(super) mutant_identity: String,
    pub(super) package: String,
    pub(super) target: String,
    pub(super) filter: String,
    pub(super) runner_tool: String,
    pub(super) runner_version: String,
    pub(super) runner_path: String,
    pub(super) runner_sha256: String,
    pub(super) argv: Vec<String>,
    pub(super) cwd: String,
    pub(super) timeout_ms: u64,
    pub(super) environment_policy: String,
    pub(super) subjects: SubjectCounts,
    pub(super) exit_code: Option<i32>,
    pub(super) timed_out: bool,
    pub(super) compile_failed: bool,
    pub(super) process_failed: bool,
    pub(super) stdout_sha256: String,
    pub(super) stderr_sha256: String,
    pub(super) currentness: String,
    pub(super) claimed_result: String,
    pub(super) limitations: Vec<String>,
    pub(super) semantic_receipt_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct JoinedRow {
    pub(super) case: JudgedCase,
    pub(super) eligibility: &'static str,
    pub(super) runtime_result: &'static str,
    pub(super) instrument_kind: Option<&'static str>,
    pub(super) non_calibration_reason: Option<&'static str>,
    pub(super) receipt: Option<CalibrationReceipt>,
    pub(super) limitations: Vec<String>,
}

pub(super) fn run(args: &[String]) -> Result<(), String> {
    let mut records = DEFAULT_RECORDS.to_string();
    let mut out_dir = DEFAULT_OUT.to_string();
    let mut check = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--records" => records = take_value(args, &mut index, "--records <dir>")?,
            "--out" => out_dir = take_value(args, &mut index, "--out <dir>")?,
            "--check" => check = true,
            other => {
                return Err(format!(
                    "unknown rust-judged-panel calibrate argument `{other}`; expected `{CALIBRATE_USAGE}`\nrerun: {RERUN}"
                ));
            }
        }
        index += 1;
    }
    let report = build_at(Path::new("."), Path::new(&records))?;
    let (json_path, markdown_path) = if check {
        (
            Path::new(RETAINED_SCORECARD_JSON).to_path_buf(),
            Path::new(RETAINED_SCORECARD_MD).to_path_buf(),
        )
    } else {
        output_paths(&out_dir)
    };
    if check {
        verify_stored(&json_path, &report.json)?;
        verify_stored(&markdown_path, &report.markdown)?;
        println!(
            "Rust judged-panel calibration scorecard matches: {}",
            json_path.display()
        );
    } else {
        write_scorecard(&json_path, &markdown_path, &report)?;
        println!("wrote: {}", json_path.display());
        println!("wrote: {}", markdown_path.display());
    }
    println!(
        "judged={} eligible={} completed={}\nrerun: {RERUN}",
        report.value["counts"]["judged"],
        report.value["counts"]["calibration_eligible"],
        report.value["counts"]["completed"]
    );
    Ok(())
}

pub(super) fn validate_retained_at(root: &Path) -> Result<(), String> {
    let report = build_at(root, Path::new(DEFAULT_RECORDS))?;
    verify_stored(&root.join(RETAINED_SCORECARD_JSON), &report.json)?;
    verify_stored(&root.join(RETAINED_SCORECARD_MD), &report.markdown)?;
    Ok(())
}

fn build_at(root: &Path, records_dir: &Path) -> Result<RenderedScorecard, String> {
    let (selection, judgments) = release_judgments::check_release_judgments_at(root)?;
    let selection_sha256 = digest_file(&root.join(RELEASE_SELECTION_PATH))?;
    let judgments_sha256 = digest_file(&root.join(RELEASE_JUDGMENTS_PATH))?;
    if selection_sha256 != judgments.selection_sha256 {
        return Err(format!(
            "calibration selection digest `{selection_sha256}` does not match judgments binding `{}`\nrerun: {RERUN}",
            judgments.selection_sha256
        ));
    }
    let rolling_sha256 = digest_file(&root.join(ROLLING_OBSERVATION_PATH))?;
    let cases = judged_from_panel(&selection, &judgments)?;
    let receipts = load_receipts_at(root, records_dir)?;
    let before = fs::read(root.join(RELEASE_JUDGMENTS_PATH))
        .map_err(|error| format!("reread judgments before join: {error}\nrerun: {RERUN}"))?;
    let rows = join_rows(&cases, &receipts, &selection_sha256, &judgments_sha256)?;
    let after = fs::read(root.join(RELEASE_JUDGMENTS_PATH))
        .map_err(|error| format!("reread judgments after join: {error}\nrerun: {RERUN}"))?;
    if before != after {
        return Err(format!(
            "calibration mutated structural judgment bytes; runtime results cannot rewrite #3806\nrerun: {RERUN}"
        ));
    }
    build_scorecard(&selection_sha256, &judgments_sha256, &rolling_sha256, &rows)
}

pub(super) fn join_rows(
    cases: &[JudgedCase],
    receipts: &BTreeMap<String, CalibrationReceipt>,
    selection_sha256: &str,
    judgments_sha256: &str,
) -> Result<Vec<JoinedRow>, String> {
    let known: BTreeMap<&str, &JudgedCase> = cases
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect();
    for case_id in receipts.keys() {
        if !known.contains_key(case_id.as_str()) {
            return Err(format!(
                "calibration receipt names unknown case `{case_id}`\nrerun: {RERUN}"
            ));
        }
    }
    let mut rows = Vec::new();
    for case in cases {
        let eligibility = eligibility_for(case);
        if eligibility != Eligibility::Eligible {
            let classified = not_run();
            let limitation = if receipts.contains_key(&case.case_id) {
                format!(
                    "receipt ignored: eligibility `{}` cannot be calibrated",
                    eligibility.as_str()
                )
            } else {
                format!(
                    "no exact calibration receipt; eligibility `{}`",
                    eligibility.as_str()
                )
            };
            rows.push(JoinedRow {
                case: case.clone(),
                eligibility: eligibility.as_str(),
                runtime_result: classified.runtime_result,
                instrument_kind: classified.instrument_kind,
                non_calibration_reason: Some(eligibility.as_str()),
                receipt: None,
                limitations: vec![limitation],
            });
            continue;
        }
        let Some(receipt) = receipts.get(&case.case_id) else {
            let classified = not_run();
            rows.push(JoinedRow {
                case: case.clone(),
                eligibility: eligibility.as_str(),
                runtime_result: classified.runtime_result,
                instrument_kind: classified.instrument_kind,
                non_calibration_reason: Some(eligibility.as_str()),
                receipt: None,
                limitations: vec![format!(
                    "no exact calibration receipt; eligibility `{}`",
                    eligibility.as_str()
                )],
            });
            continue;
        };
        let identity_ok = identity_matches(case, receipt, selection_sha256, judgments_sha256);
        let selector_ok = selector_matches(case, receipt);
        let mut facts = AttemptFacts {
            identity_ok,
            selector_ok,
            equivalent: receipt.claimed_result == RESULT_EQUIVALENT,
            timed_out: receipt.timed_out,
            compile_failed: receipt.compile_failed,
            process_failed: receipt.process_failed,
            exit_code: receipt.exit_code,
            intended: receipt.subjects.intended,
            discovered: receipt.subjects.discovered,
            selected: receipt.subjects.selected,
            executed: receipt.subjects.executed,
        };
        if claimed_caught_or_survived_without_subjects(receipt) {
            facts.intended = 0;
            facts.selected = 0;
            facts.executed = 0;
        }
        let classified = classify_attempt(&facts);
        let mut limitations = receipt.limitations.clone();
        if let Some(reason) = classified.non_calibration_reason {
            limitations.push(reason.to_string());
        }
        rows.push(JoinedRow {
            case: case.clone(),
            eligibility: eligibility.as_str(),
            runtime_result: classified.runtime_result,
            instrument_kind: classified.instrument_kind,
            non_calibration_reason: classified.non_calibration_reason,
            receipt: Some(receipt.clone()),
            limitations,
        });
    }
    Ok(rows)
}

fn judged_from_panel(
    selection: &RustJudgedPanelManifest,
    judgments: &ReleaseJudgments,
) -> Result<Vec<JudgedCase>, String> {
    let by_id: BTreeMap<&str, &RustJudgedPanelItem> = selection
        .items
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect();
    let mut cases = Vec::new();
    for judgment in &judgments.judgments {
        let item = by_id.get(judgment.case_id.as_str()).ok_or_else(|| {
            format!(
                "judgment `{}` is not in the bound selection\nrerun: {RERUN}",
                judgment.case_id
            )
        })?;
        cases.push(judged_from_item(item, &judgment.terminal));
    }
    Ok(cases)
}

fn judged_from_item(item: &RustJudgedPanelItem, terminal: &str) -> JudgedCase {
    let aligned = item.test_evidence.aligned_observer.value().is_some();
    let missing = item
        .test_evidence
        .missing
        .value()
        .is_some_and(|value| !value.trim().is_empty());
    let witness = if aligned {
        "aligned"
    } else if missing {
        "missing_observer"
    } else {
        "unspecified"
    };
    JudgedCase {
        case_id: item.id.clone(),
        expected_direction: item.expected_direction.clone(),
        terminal: terminal.to_string(),
        behavior_family: item.behavior_family.clone(),
        relation_basis: item.selection_dimensions.relation_basis.clone(),
        oracle_class: item.selection_dimensions.oracle_family.clone(),
        witness_completeness: witness.to_string(),
        repository: item.repository.clone(),
        base: item.base.value().cloned().unwrap_or_default(),
        head: item.head.value().cloned().unwrap_or_default(),
        tree_identity: item.tree_identity.value().cloned().unwrap_or_default(),
        scope_authorization: item
            .scope_authorization
            .value()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        no_focused_mutant: true,
        behavior: BehaviorIdentity {
            file: item.anchor.file.clone(),
            line: item.anchor.line,
            owner: item.anchor.owner.clone(),
            changed_behavior: item.anchor.changed_behavior.clone(),
        },
        required_selector: None,
    }
}

pub(super) fn static_direction_from_terminal(terminal: &str) -> Option<&'static str> {
    match terminal {
        "confirmed_should_gap" => Some("should_gap"),
        "confirmed_should_stay_quiet" => Some("should_stay_quiet"),
        "confirmed_should_limit" => Some("should_limit"),
        _ => None,
    }
}

pub(super) fn digest_pref(bytes: &[u8]) -> String {
    format!(
        "sha256:{}",
        crate::python_judged_panel_replay::sha256_hex(bytes)
    )
}

fn take_value(args: &[String], index: &mut usize, what: &str) -> Result<String, String> {
    let value = args
        .get(*index + 1)
        .ok_or_else(|| format!("{what} requires a value\nrerun: {RERUN}"))?;
    if value.starts_with("--") {
        return Err(format!(
            "{what} requires a value, found flag `{value}`\nrerun: {RERUN}"
        ));
    }
    *index += 1;
    Ok(value.trim().to_string())
}
