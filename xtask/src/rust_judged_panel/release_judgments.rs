//! Independent adjudication packet for the frozen 0.11 release challenge
//! (#3806).
//!
//! The packet binds one terminal structural judgment to every row of the
//! frozen selection (`release-selection.json`, #3805) by the selection's
//! exact byte digest. It never edits the selection: a judgment that
//! disagrees with a row's expected direction is recorded as such, not
//! folded back into the manifest.
//!
//! The reference-run outcome labels compare the judgment with one named
//! analyzer run used during investigation. They are not the #1609
//! candidate replay (#2769), which consumes this packet later.

use std::fs;
use std::path::Path;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{
    RELEASE_SELECTION_PATH, RustJudgedPanelManifest, check_release_selection_at,
    parse_json_without_duplicate_keys,
};

pub(crate) const RELEASE_JUDGMENTS_PATH: &str =
    "metrics/rust-judged-behavior-panel/release-judgments.json";
const JUDGMENTS_RERUN_COMMAND: &str = "cargo xtask check-release-challenge-judgments";
const JUDGMENTS_KIND: &str = "rust_release_challenge_judgments";
const JUDGMENTS_AUTHORITY: &str = "EffortlessMetrics/ripr-swarm#3806";

/// Terminal vocabulary from #3806. `confirmed_should_*` names the direction
/// the evidence supports, which may differ from the row's expected one.
const TERMINALS: [&str; 6] = [
    "confirmed_should_gap",
    "confirmed_should_stay_quiet",
    "confirmed_should_limit",
    "inconclusive_missing_evidence",
    "inconclusive_disagreement",
    "invalid_case_identity",
];
const CONFIDENCE: [&str; 3] = ["high", "medium", "low"];
/// Review verdicts and the confirmed direction each one supports. Reviews
/// that support different directions, or a direction other than the
/// confirmed terminal, are a disagreement and must be recorded as one.
const REVIEW_JUDGMENTS: [(&str, &str); 5] = [
    ("discriminated", "should_stay_quiet"),
    ("no_production_behavior", "should_stay_quiet"),
    ("weakly_discriminated", "should_gap"),
    ("not_discriminated", "should_gap"),
    ("limited", "should_limit"),
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleaseJudgments {
    schema_version: String,
    kind: String,
    authority: String,
    selection_path: String,
    pub(super) selection_sha256: String,
    reference_run: ReferenceRun,
    roles: Vec<ReviewRole>,
    limits: Vec<String>,
    pub(super) judgments: Vec<RowJudgment>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceRun {
    analyzer: String,
    source: String,
    argv: Vec<String>,
    scope: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewRole {
    id: String,
    method: String,
    blind_to: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RowJudgment {
    pub(super) case_id: String,
    expected_direction: String,
    pub(super) terminal: String,
    structural: Structural,
    reviews: Vec<Review>,
    disagreement: Option<Disagreement>,
    reference_outcome: ReferenceOutcome,
    non_claims: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Structural {
    owner: String,
    activation: String,
    propagation: String,
    observer: String,
    oracle: String,
    target: String,
    limitation: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    role: String,
    judgment: String,
    confidence: String,
    evidence: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Disagreement {
    summary: String,
    resolution: Option<String>,
}

/// Outcome labels against the reference run. `None` is "not established",
/// never false.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceOutcome {
    observation: String,
    false_actionable: Option<bool>,
    false_exposed: Option<bool>,
    under_credit: Option<bool>,
    limitation_correct: Option<bool>,
}

mod summarize;
mod validate;

#[cfg(test)]
mod tests;

use summarize::summarize;
use validate::validate_judgments;

pub(crate) fn check_release_judgments() -> Result<(), String> {
    let (_, judgments) = check_release_judgments_at(Path::new("."))?;
    let summary = summarize(&judgments);
    crate::write_report("release-judgments.md", &summary)?;
    print!("{summary}");
    Ok(())
}

pub(crate) fn check_release_judgments_at(
    root: &Path,
) -> Result<(RustJudgedPanelManifest, ReleaseJudgments), String> {
    let selection = check_release_selection_at(root)?;
    let selection_bytes = fs::read(root.join(RELEASE_SELECTION_PATH))
        .map_err(|error| format!("read `{RELEASE_SELECTION_PATH}`: {error}"))?;
    let body = fs::read_to_string(root.join(RELEASE_JUDGMENTS_PATH))
        .map_err(|error| format!("read `{RELEASE_JUDGMENTS_PATH}`: {error}"))?;
    let judgments = parse_bound_judgments(&selection_bytes, &body)?;
    let mut violations = validate_judgments(&selection_bytes, &selection, &judgments);
    violations.sort();
    violations.dedup();
    if violations.is_empty() {
        Ok((selection, judgments))
    } else {
        Err(format!(
            "Release-challenge judgments `{RELEASE_JUDGMENTS_PATH}` have {} violation(s):\n- {}\nrerun: {JUDGMENTS_RERUN_COMMAND}",
            violations.len(),
            violations.join("\n- ")
        ))
    }
}

fn selection_digest(selection_bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(selection_bytes))
}

fn stale_selection(bound: &str, actual: &str) -> String {
    format!(
        "selection_sha256: packet binds `{bound}` but `{RELEASE_SELECTION_PATH}` is `{actual}`; re-adjudicate against the current selection"
    )
}

/// Reads the selection binding before the typed rows, so a stale packet
/// reports the stale selection even when its rows no longer parse.
fn parse_bound_judgments(selection_bytes: &[u8], body: &str) -> Result<ReleaseJudgments, String> {
    let value = parse_json_without_duplicate_keys(body)
        .map_err(|error| format!("parse `{RELEASE_JUDGMENTS_PATH}`: {error}"))?;
    let actual = selection_digest(selection_bytes);
    let bound = value
        .get("selection_sha256")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("<missing>");
    if bound != actual {
        return Err(format!(
            "Release-challenge judgments `{RELEASE_JUDGMENTS_PATH}`: {}\nrerun: {JUDGMENTS_RERUN_COMMAND}",
            stale_selection(bound, &actual)
        ));
    }
    serde_json::from_value(value)
        .map_err(|error| format!("parse `{RELEASE_JUDGMENTS_PATH}`: {error}"))
}
