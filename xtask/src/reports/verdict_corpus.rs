//! Labeled Rust verdict corpus (RIPR-SPEC-0219).
//!
//! Each case pairs a one-line edit in a pinned real crate with a runtime truth
//! label: the edited behavior was mutated and the crate's own test suite was
//! run against every mutant. The harness re-runs `ripr check` on the retained
//! upstream excerpt, projects the anchored findings to one verdict, and scores
//! it against that label. It reports false-verdict and contradiction rates; it
//! does not run mutation testing, and its rates describe this corpus only.

use super::fixtures::ripr_fixture_binary;
use crate::normalize_path;
use crate::run::run_output_owned_with_envs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const CORPUS_DIR: &str = "fixtures/rust-verdict-corpus";
const CORPUS_SCHEMA: &str = "ripr_verdict_corpus.v1";
const REPORT_SCHEMA: &str = "ripr_verdict_corpus_report.v1";
const DEFAULT_OUT: &str = "target/ripr/reports/verdict-corpus";
const CACHE_ENV: &str = "RIPR_CACHE_DIR";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Corpus {
    pub(crate) schema_version: String,
    pub(crate) kind: String,
    pub(crate) spec: String,
    pub(crate) corpus_version: String,
    pub(crate) description: String,
    pub(crate) label_method: String,
    pub(crate) verdict_projection: String,
    pub(crate) non_claims: Vec<String>,
    pub(crate) subjects: Vec<Subject>,
    pub(crate) cases: Vec<Case>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Subject {
    pub(crate) subject_id: String,
    pub(crate) upstream: String,
    pub(crate) commit: String,
    pub(crate) version_label: String,
    pub(crate) license: String,
    pub(crate) shared_corpus: Option<SharedCorpusRef>,
    pub(crate) retained_files: Vec<RetainedFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SharedCorpusRef {
    pub(crate) manifest: String,
    pub(crate) corpus_version: String,
    pub(crate) repo_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetainedFile {
    pub(crate) path: String,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Case {
    pub(crate) case_id: String,
    pub(crate) subject_id: String,
    pub(crate) diff: String,
    pub(crate) anchor: Anchor,
    pub(crate) edit_kind: EditKind,
    pub(crate) behavior_family: String,
    pub(crate) test_shape: String,
    pub(crate) hard_case: Option<String>,
    pub(crate) truth: Truth,
    pub(crate) expected: Expected,
    pub(crate) reasoning: String,
    pub(crate) labeling_observation: LabelingObservation,
}

/// What ripr said when the case was labeled, on the full pinned checkout and
/// on the retained excerpt. Matching excerpt and full-checkout findings is
/// what lets the excerpt stand in for the repository.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelingObservation {
    pub(crate) ripr_commit: String,
    pub(crate) full_checkout_verdict: Verdict,
    pub(crate) full_checkout_classifications: Vec<String>,
    pub(crate) excerpt_parity: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Anchor {
    pub(crate) file: String,
    pub(crate) line: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EditKind {
    /// Same behavior, rewritten; mutants of the rewritten expression carry truth.
    BehaviorPreservingRewrite,
    /// The edit itself changes behavior and is its own mutant.
    BehaviorChange,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Truth {
    pub(crate) state: TruthState,
    pub(crate) method: String,
    pub(crate) test_command: String,
    pub(crate) toolchain: String,
    pub(crate) mutants: Vec<Mutant>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TruthState {
    Discriminated,
    PartiallyDiscriminated,
    NotDiscriminated,
}

impl TruthState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Discriminated => "discriminated",
            Self::PartiallyDiscriminated => "partially_discriminated",
            Self::NotDiscriminated => "not_discriminated",
        }
    }

    /// The only verdicts a label may name for this truth. Labels restate the
    /// table so a reader sees it per case; the validator rejects any drift.
    fn ideal(self) -> Verdict {
        match self {
            Self::Discriminated => Verdict::Credited,
            Self::PartiallyDiscriminated | Self::NotDiscriminated => Verdict::Gap,
        }
    }

    fn acceptable(self) -> &'static [Verdict] {
        match self {
            Self::Discriminated => &[Verdict::Credited, Verdict::Limited, Verdict::Silent],
            Self::PartiallyDiscriminated | Self::NotDiscriminated => {
                &[Verdict::Gap, Verdict::Limited]
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Mutant {
    pub(crate) replacement: String,
    pub(crate) outcome: MutantOutcome,
    pub(crate) failing_test: Option<String>,
    pub(crate) equivalence_review: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MutantOutcome {
    TestsFailed,
    TestsPassed,
}

impl MutantOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::TestsFailed => "tests_failed",
            Self::TestsPassed => "tests_passed",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Expected {
    pub(crate) ideal_verdict: Verdict,
    pub(crate) acceptable_verdicts: Vec<Verdict>,
}

/// One verdict per case, projected from the anchored candidate-current
/// findings the same way the human "Start here" triage reads a finding
/// (`crates/ripr/src/output/human/triage.rs`): `exposed` credits a
/// discriminator, a named static limitation or an unknown/no-path class
/// limits, and any other class routes a gap.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Verdict {
    Credited,
    Gap,
    Limited,
    Silent,
}

impl Verdict {
    fn as_str(self) -> &'static str {
        match self {
            Self::Credited => "credited",
            Self::Gap => "gap",
            Self::Limited => "limited",
            Self::Silent => "silent",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Outcome {
    Ideal,
    Abstained,
    FalseActionable,
    FalseExposed,
    FalseSilent,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ideal => "ideal",
            Self::Abstained => "abstained",
            Self::FalseActionable => "false_actionable",
            Self::FalseExposed => "false_exposed",
            Self::FalseSilent => "false_silent",
        }
    }
}

pub(crate) fn score(truth: TruthState, observed: Verdict) -> Outcome {
    if observed == truth.ideal() {
        return Outcome::Ideal;
    }
    if truth.acceptable().contains(&observed) {
        return Outcome::Abstained;
    }
    match (truth, observed) {
        (TruthState::Discriminated, _) => Outcome::FalseActionable,
        (_, Verdict::Credited) => Outcome::FalseExposed,
        _ => Outcome::FalseSilent,
    }
}

pub(crate) fn finding_verdict(finding: &Value) -> Verdict {
    let class = finding
        .get("classification")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if class == "exposed" {
        return Verdict::Credited;
    }
    let named_limit = finding
        .get("static_limit_kind")
        .is_some_and(|kind| !kind.is_null());
    if named_limit
        || matches!(
            class,
            "no_static_path" | "infection_unknown" | "propagation_unknown" | "static_unknown"
        )
    {
        return Verdict::Limited;
    }
    Verdict::Gap
}

fn probe_file(finding: &Value) -> String {
    let file = finding
        .pointer("/probe/file")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .replace('\\', "/");
    file.strip_prefix("./").unwrap_or(&file).to_string()
}

fn is_candidate_current(finding: &Value) -> bool {
    finding.get("source_currentness").and_then(Value::as_str) == Some("candidate_current")
}

/// Findings that speak for the anchored line on the candidate side. Base-side
/// evidence for a removed line is never a candidate verdict.
pub(crate) fn anchored_findings<'a>(check: &'a Value, anchor: &Anchor) -> Vec<&'a Value> {
    check
        .get("findings")
        .and_then(Value::as_array)
        .map(|findings| {
            findings
                .iter()
                .filter(|finding| is_candidate_current(finding))
                .filter(|finding| {
                    finding.pointer("/probe/line").and_then(Value::as_u64)
                        == Some(anchor.line as u64)
                        && probe_file(finding) == anchor.file
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Line-level precedence is this corpus's own policy, not the triage
/// ranking (which orders findings by where to start). A gap anywhere on the
/// line routes repair work, so it outranks credit; credit outranks a
/// limitation because it claims a discriminator exists.
pub(crate) fn case_verdict(findings: &[&Value]) -> Verdict {
    let verdicts: BTreeSet<Verdict> = findings.iter().map(|f| finding_verdict(f)).collect();
    if verdicts.is_empty() {
        Verdict::Silent
    } else if verdicts.contains(&Verdict::Gap) {
        Verdict::Gap
    } else if verdicts.contains(&Verdict::Credited) {
        Verdict::Credited
    } else {
        Verdict::Limited
    }
}

/// Internal contradictions a reader can see inside one finding, independent
/// of any truth label.
pub(crate) fn finding_contradictions(finding: &Value) -> Vec<&'static str> {
    let mut codes = Vec::new();
    let class = finding
        .get("classification")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let total = finding
        .get("related_tests_total")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let listed = finding
        .get("related_tests")
        .and_then(Value::as_array)
        .map_or(0, Vec::len) as u64;
    let stage = |name: &str| {
        finding
            .pointer(&format!("/ripr/{name}/state"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    if stage("reach") == "yes" && total == 0 {
        codes.push("reach_yes_without_related_tests");
    }
    if class == "no_static_path" && total > 0 {
        codes.push("no_static_path_with_related_tests");
    }
    if class == "exposed" && stage("discriminate") != "yes" {
        codes.push("exposed_without_discriminator");
    }
    if listed > total {
        codes.push("related_tests_listed_exceed_total");
    }
    codes
}

const SUMMARY_CLASSES: [&str; 7] = [
    "exposed",
    "weakly_exposed",
    "reachable_unrevealed",
    "no_static_path",
    "infection_unknown",
    "propagation_unknown",
    "static_unknown",
];

/// The summary block and the findings list are two renderings of one result.
pub(crate) fn summary_contradictions(check: &Value) -> Vec<String> {
    let findings = check
        .get("findings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut codes = Vec::new();
    let summary_total = check.pointer("/summary/findings").and_then(Value::as_u64);
    if summary_total.is_some_and(|total| total != findings.len() as u64) {
        codes.push("summary_findings_count_mismatch".to_string());
    }
    // Under a suppression policy ripr's per-class buckets count unsuppressed
    // findings only, while `findings` lists every finding; buckets plus
    // `suppressed_by_policy` must add back up to the list instead.
    if let Some(suppressed) = check
        .pointer("/summary/suppressed_by_policy")
        .and_then(Value::as_u64)
    {
        let buckets: u64 = SUMMARY_CLASSES
            .iter()
            .filter_map(|class| {
                check
                    .pointer(&format!("/summary/{class}"))
                    .and_then(Value::as_u64)
            })
            .sum();
        if buckets + suppressed != findings.len() as u64 {
            codes.push("summary_suppression_count_mismatch".to_string());
        }
        return codes;
    }
    for class in SUMMARY_CLASSES {
        let counted = findings
            .iter()
            .filter(|f| f.get("classification").and_then(Value::as_str) == Some(class))
            .count() as u64;
        if let Some(stated) = check
            .pointer(&format!("/summary/{class}"))
            .and_then(Value::as_u64)
            && stated != counted
        {
            codes.push(format!("summary_{class}_count_mismatch"));
        }
    }
    codes
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Ratio {
    pub(crate) numerator: usize,
    pub(crate) denominator: usize,
    pub(crate) rate: String,
}

pub(crate) fn ratio(numerator: usize, denominator: usize) -> Ratio {
    // Fixed four-decimal text keeps the golden report byte-stable.
    let rate = if denominator == 0 {
        "n/a".to_string()
    } else {
        let scaled = (numerator as u128 * 1_000_000 / denominator as u128 + 50) / 100;
        format!("{}.{:04}", scaled / 10_000, scaled % 10_000)
    };
    Ratio {
        numerator,
        denominator,
        rate,
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CaseRow {
    pub(crate) case_id: String,
    pub(crate) subject_id: String,
    pub(crate) anchor: String,
    pub(crate) behavior_family: String,
    pub(crate) test_shape: String,
    pub(crate) hard_case: bool,
    pub(crate) truth: TruthState,
    pub(crate) ideal_verdict: Verdict,
    pub(crate) observed_verdict: Verdict,
    /// Differs from the verdict recorded at labeling: re-check excerpt parity
    /// against the full pinned checkout before accepting the new verdict.
    pub(crate) changed_since_labeling: bool,
    pub(crate) observed_classifications: Vec<String>,
    pub(crate) outcome: Outcome,
    pub(crate) contradictions: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Report {
    pub(crate) schema_version: String,
    pub(crate) spec: String,
    pub(crate) corpus_version: String,
    pub(crate) cases_total: usize,
    pub(crate) by_truth: BTreeMap<String, usize>,
    pub(crate) by_outcome: BTreeMap<String, usize>,
    pub(crate) by_observed_verdict: BTreeMap<String, usize>,
    pub(crate) false_verdict_rate: Ratio,
    pub(crate) false_actionable_rate: Ratio,
    pub(crate) false_exposed_rate: Ratio,
    pub(crate) false_silent_rate: Ratio,
    pub(crate) ideal_rate: Ratio,
    pub(crate) abstention_rate: Ratio,
    pub(crate) contradiction_rate: Ratio,
    pub(crate) contradictions_by_code: BTreeMap<String, usize>,
    pub(crate) rows: Vec<CaseRow>,
    pub(crate) non_claims: Vec<String>,
}

/// Per-case scoring. Contradictions are counted per candidate-current
/// finding across the whole check, not only the anchor line: a
/// self-contradicting finding anywhere in the run is an internal ripr
/// inconsistency. Summary-count codes are per check.
pub(crate) fn case_row(
    case: &Case,
    check: &Value,
) -> (CaseRow, usize, usize, BTreeMap<String, usize>) {
    let anchored = anchored_findings(check, &case.anchor);
    let observed = case_verdict(&anchored);
    let mut classes: Vec<String> = anchored
        .iter()
        .filter_map(|f| f.get("classification").and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    classes.sort();
    classes.dedup();
    let mut contradictions = BTreeSet::new();
    let mut scored = 0;
    let mut contradicted = 0;
    let mut code_counts = BTreeMap::new();
    for finding in check
        .get("findings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|f| is_candidate_current(f))
    {
        scored += 1;
        let codes = finding_contradictions(finding);
        if !codes.is_empty() {
            contradicted += 1;
        }
        for code in codes {
            *code_counts.entry(code.to_string()).or_insert(0) += 1;
            contradictions.insert(code.to_string());
        }
    }
    for code in summary_contradictions(check) {
        *code_counts.entry(code.clone()).or_insert(0) += 1;
        contradictions.insert(code);
    }
    let row = CaseRow {
        case_id: case.case_id.clone(),
        subject_id: case.subject_id.clone(),
        anchor: format!("{}:{}", case.anchor.file, case.anchor.line),
        behavior_family: case.behavior_family.clone(),
        test_shape: case.test_shape.clone(),
        hard_case: case.hard_case.is_some(),
        truth: case.truth.state,
        ideal_verdict: case.truth.state.ideal(),
        observed_verdict: observed,
        changed_since_labeling: observed != case.labeling_observation.full_checkout_verdict
            || classes != case.labeling_observation.full_checkout_classifications,
        observed_classifications: classes,
        outcome: score(case.truth.state, observed),
        contradictions: contradictions.into_iter().collect(),
    };
    (row, scored, contradicted, code_counts)
}

pub(crate) fn build_report(corpus: &Corpus, checks: &[(String, Value)]) -> Result<Report, String> {
    let by_id: BTreeMap<&str, &Value> = checks.iter().map(|(id, v)| (id.as_str(), v)).collect();
    let mut rows = Vec::new();
    let mut findings_scored = 0;
    let mut findings_contradicted = 0;
    let mut by_code = BTreeMap::new();
    for case in &corpus.cases {
        let check = by_id
            .get(case.case_id.as_str())
            .ok_or_else(|| format!("no ripr result for case `{}`", case.case_id))?;
        let (row, scored, contradicted, code_counts) = case_row(case, check);
        findings_scored += scored;
        findings_contradicted += contradicted;
        for (code, n) in code_counts {
            *by_code.entry(code).or_insert(0) += n;
        }
        rows.push(row);
    }
    let count = |outcome: Outcome| rows.iter().filter(|r| r.outcome == outcome).count();
    let truth_count =
        |pred: &dyn Fn(TruthState) -> bool| rows.iter().filter(|r| pred(r.truth)).count();
    let discriminated = truth_count(&|t| t == TruthState::Discriminated);
    let not_fully = rows.len() - discriminated;
    let false_actionable = count(Outcome::FalseActionable);
    let false_exposed = count(Outcome::FalseExposed);
    let false_silent = count(Outcome::FalseSilent);
    let mut by_truth = BTreeMap::new();
    let mut by_outcome = BTreeMap::new();
    let mut by_observed = BTreeMap::new();
    for row in &rows {
        *by_truth.entry(row.truth.as_str().to_string()).or_insert(0) += 1;
        *by_outcome
            .entry(row.outcome.as_str().to_string())
            .or_insert(0) += 1;
        *by_observed
            .entry(row.observed_verdict.as_str().to_string())
            .or_insert(0) += 1;
    }
    Ok(Report {
        schema_version: REPORT_SCHEMA.to_string(),
        spec: corpus.spec.clone(),
        corpus_version: corpus.corpus_version.clone(),
        cases_total: rows.len(),
        by_truth,
        by_outcome,
        by_observed_verdict: by_observed,
        false_verdict_rate: ratio(false_actionable + false_exposed + false_silent, rows.len()),
        false_actionable_rate: ratio(false_actionable, discriminated),
        false_exposed_rate: ratio(false_exposed, not_fully),
        false_silent_rate: ratio(false_silent, not_fully),
        ideal_rate: ratio(count(Outcome::Ideal), rows.len()),
        abstention_rate: ratio(count(Outcome::Abstained), rows.len()),
        contradiction_rate: ratio(findings_contradicted, findings_scored),
        contradictions_by_code: by_code,
        rows,
        non_claims: corpus.non_claims.clone(),
    })
}

pub(crate) fn render_report_json(report: &Report) -> Result<String, String> {
    serde_json::to_string_pretty(report)
        .map(|text| format!("{text}\n"))
        .map_err(|err| format!("render verdict corpus report: {err}"))
}

pub(crate) fn render_report_markdown(report: &Report) -> String {
    let mut out = String::new();
    out.push_str("# Rust verdict corpus report\n\n");
    out.push_str(&format!(
        "Spec: {}. Corpus version: {}. Cases: {}.\n\n",
        report.spec, report.corpus_version, report.cases_total
    ));
    out.push_str("| Rate | Count | Rate |\n| --- | --- | --- |\n");
    for (label, ratio) in [
        ("False verdicts (all cases)", &report.false_verdict_rate),
        (
            "False actionable (of discriminated)",
            &report.false_actionable_rate,
        ),
        (
            "False exposed (of not fully discriminated)",
            &report.false_exposed_rate,
        ),
        (
            "False silent (of not fully discriminated)",
            &report.false_silent_rate,
        ),
        ("Ideal verdict", &report.ideal_rate),
        (
            "Abstained (limited or silent where acceptable)",
            &report.abstention_rate,
        ),
        ("Findings with a contradiction", &report.contradiction_rate),
    ] {
        out.push_str(&format!(
            "| {label} | {}/{} | {} |\n",
            ratio.numerator, ratio.denominator, ratio.rate
        ));
    }
    out.push_str(
        "\n| Case | Truth | Ideal | Observed | Classes | Outcome | Changed since labeling | Contradictions |\n",
    );
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for row in &report.rows {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} |\n",
            row.case_id,
            row.truth.as_str(),
            row.ideal_verdict.as_str(),
            row.observed_verdict.as_str(),
            if row.observed_classifications.is_empty() {
                "none".to_string()
            } else {
                row.observed_classifications.join(", ")
            },
            row.outcome.as_str(),
            if row.changed_since_labeling {
                "yes"
            } else {
                "no"
            },
            if row.contradictions.is_empty() {
                "none".to_string()
            } else {
                row.contradictions.join(", ")
            },
        ));
    }
    out.push_str("\nNon-claims:\n\n");
    for claim in &report.non_claims {
        out.push_str(&format!("- {claim}\n"));
    }
    out
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|err| format!("read {}: {err}", normalize_path(path)))
}

pub(crate) fn load_corpus(dir: &Path) -> Result<Corpus, String> {
    let path = dir.join("corpus.json");
    serde_json::from_str(&read(&path)?)
        .map_err(|err| format!("parse {}: {err}", normalize_path(&path)))
}

/// An identifier that becomes one directory name under the run root.
fn safe_id(id: &str) -> bool {
    safe_relative(id) && !id.contains('/')
}

fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// Structural and label-law validation. Runs without building or invoking
/// ripr, so it can sit on the fast path.
pub(crate) fn validate(corpus: &Corpus, dir: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    if corpus.schema_version != CORPUS_SCHEMA {
        violations.push(format!(
            "schema_version `{}` is not `{CORPUS_SCHEMA}`",
            corpus.schema_version
        ));
    }
    if corpus.kind != "ripr_verdict_corpus" {
        violations.push(format!(
            "kind `{}` is not `ripr_verdict_corpus`",
            corpus.kind
        ));
    }
    for (field, value) in [
        ("spec", &corpus.spec),
        ("corpus_version", &corpus.corpus_version),
        ("description", &corpus.description),
        ("label_method", &corpus.label_method),
        ("verdict_projection", &corpus.verdict_projection),
    ] {
        if value.trim().is_empty() {
            violations.push(format!("`{field}` is empty"));
        }
    }
    if corpus.non_claims.is_empty() {
        violations.push("`non_claims` is empty".to_string());
    }
    let mut subjects = BTreeMap::new();
    for subject in &corpus.subjects {
        if subjects
            .insert(subject.subject_id.as_str(), subject)
            .is_some()
        {
            violations.push(format!("duplicate subject `{}`", subject.subject_id));
        }
        violations.extend(subject_violations(subject, dir));
    }
    let mut ids = BTreeSet::new();
    for case in &corpus.cases {
        if !ids.insert(case.case_id.as_str()) {
            violations.push(format!("duplicate case `{}`", case.case_id));
        }
        if !safe_id(&case.case_id) {
            violations.push(format!(
                "case id `{}` is not a single safe path segment; use letters, digits and dashes",
                case.case_id
            ));
        }
        match subjects.get(case.subject_id.as_str()) {
            Some(subject) => violations.extend(case_violations(case, subject, dir)),
            None => violations.push(format!(
                "case `{}` names unknown subject `{}`",
                case.case_id, case.subject_id
            )),
        }
    }
    let truths: BTreeSet<TruthState> = corpus.cases.iter().map(|c| c.truth.state).collect();
    for required in [TruthState::Discriminated, TruthState::NotDiscriminated] {
        if !truths.contains(&required) {
            violations.push(format!(
                "corpus has no `{}` case; it cannot measure both error directions",
                required.as_str()
            ));
        }
    }
    violations
}

fn subject_violations(subject: &Subject, dir: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    let id = &subject.subject_id;
    if !safe_id(id) {
        violations.push(format!(
            "subject id `{id}` is not a single safe path segment; use letters, digits and dashes"
        ));
    }
    if subject.commit.len() != 40 || !subject.commit.chars().all(|c| c.is_ascii_hexdigit()) {
        violations.push(format!("subject `{id}` commit is not a 40-hex sha"));
    }
    if !subject.upstream.starts_with("https://") {
        violations.push(format!("subject `{id}` upstream is not an https URL"));
    }
    if subject.license.trim().is_empty() || subject.version_label.trim().is_empty() {
        violations.push(format!("subject `{id}` has no license or version label"));
    }
    if let Some(shared) = &subject.shared_corpus
        && (shared.manifest.trim().is_empty()
            || shared.repo_id.trim().is_empty()
            || shared.corpus_version.trim().is_empty())
    {
        violations.push(format!(
            "subject `{id}` shared corpus reference is incomplete"
        ));
    }
    let root = dir.join("subjects").join(id);
    let mut listed = BTreeSet::new();
    for file in &subject.retained_files {
        if !safe_relative(&file.path) {
            violations.push(format!(
                "subject `{id}` retained path `{}` is unsafe",
                file.path
            ));
            continue;
        }
        listed.insert(file.path.clone());
        match fs::read(root.join(stored_path(&file.path))) {
            Ok(bytes) if sha256_hex(&bytes) == file.sha256 => {}
            Ok(_) => violations.push(format!(
                "subject `{id}` file `{}` does not match its pinned sha256; restore the upstream bytes from {} at {}, or re-pin the sha256 if the pin moved deliberately",
                file.path, subject.upstream, subject.commit
            )),
            Err(err) => violations.push(format!(
                "subject `{id}` file `{}` is unreadable: {err}",
                file.path
            )),
        }
    }
    if !subject
        .retained_files
        .iter()
        .any(|f| f.path.contains("LICENSE"))
    {
        violations.push(format!("subject `{id}` retains no LICENSE file"));
    }
    match files_under(&root) {
        Ok(stored) => {
            let mut found = BTreeSet::new();
            for path in stored {
                match logical_path(&path) {
                    Some(logical) => {
                        found.insert(logical);
                    }
                    None => violations.push(format!(
                        "subject `{id}` stores `{path}` as Rust source; rename it to `{path}.txt` so vendored code stays fixture data"
                    )),
                }
            }
            for path in found.difference(&listed) {
                violations.push(format!(
                    "subject `{id}` carries unlisted file `{path}`; list it with its sha256 or remove it"
                ));
            }
        }
        Err(err) => violations.push(err),
    }
    violations
}

/// Retained Rust sources are stored as `<name>.rs.txt` so the vendored
/// upstream code is fixture data, not repository Rust: it stays out of the
/// workspace's Rust gates and PR diff-scope budget. The run-owned copy gets
/// the upstream `.rs` name back.
pub(crate) fn stored_path(logical: &str) -> String {
    if logical.ends_with(".rs") {
        format!("{logical}.txt")
    } else {
        logical.to_string()
    }
}

/// The upstream name of a stored file, or `None` for a bare `.rs` file that
/// should have been stored as `.rs.txt`.
pub(crate) fn logical_path(stored: &str) -> Option<String> {
    if let Some(rust) = stored.strip_suffix(".rs.txt") {
        Some(format!("{rust}.rs"))
    } else if stored.ends_with(".rs") {
        None
    } else {
        Some(stored.to_string())
    }
}

fn files_under(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut found = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries =
            fs::read_dir(&dir).map_err(|err| format!("read {}: {err}", normalize_path(&dir)))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("read {}: {err}", normalize_path(&dir)))?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(rel) = path.strip_prefix(root) {
                found.insert(normalize_path(rel));
            }
        }
    }
    Ok(found)
}

pub(crate) fn case_violations(case: &Case, subject: &Subject, dir: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    let id = &case.case_id;
    for (field, value) in [
        ("behavior_family", &case.behavior_family),
        ("test_shape", &case.test_shape),
        ("truth.test_command", &case.truth.test_command),
        ("truth.toolchain", &case.truth.toolchain),
        (
            "labeling_observation.ripr_commit",
            &case.labeling_observation.ripr_commit,
        ),
    ] {
        if value.trim().is_empty() {
            violations.push(format!("case `{id}` has an empty `{field}`"));
        }
    }
    if case.truth.method != "runtime_mutant_kill" {
        violations.push(format!(
            "case `{id}` truth method `{}` is not runtime_mutant_kill",
            case.truth.method
        ));
    }
    if case
        .hard_case
        .as_deref()
        .is_some_and(|note| note.trim().is_empty())
    {
        violations.push(format!(
            "case `{id}` hard_case is present but empty; say why the case is hard or remove the field"
        ));
    }
    if case.labeling_observation.excerpt_parity != "matched" {
        violations.push(format!(
            "case `{id}` excerpt findings were not shown to match the full checkout"
        ));
    }
    if case.reasoning.trim().len() < 40 {
        violations.push(format!(
            "case `{id}` reasoning is missing or too thin to audit"
        ));
    }
    let ideal = case.truth.state.ideal();
    if case.expected.ideal_verdict != ideal {
        violations.push(format!(
            "case `{id}` ideal_verdict `{}` contradicts truth `{}` (must be `{}`)",
            case.expected.ideal_verdict.as_str(),
            case.truth.state.as_str(),
            ideal.as_str()
        ));
    }
    let acceptable: BTreeSet<Verdict> = case.expected.acceptable_verdicts.iter().copied().collect();
    let law: BTreeSet<Verdict> = case.truth.state.acceptable().iter().copied().collect();
    if acceptable != law {
        violations.push(format!(
            "case `{id}` acceptable_verdicts drift from the truth table for `{}`",
            case.truth.state.as_str()
        ));
    }
    let mutants = &case.truth.mutants;
    if mutants.is_empty() {
        violations.push(format!(
            "case `{id}` has no mutant outcome behind its truth"
        ));
    }
    if case.edit_kind == EditKind::BehaviorChange && mutants.len() != 1 {
        violations.push(format!(
            "case `{id}` is a behavior_change with {} mutants; it must list exactly one, the edit itself",
            mutants.len()
        ));
    }
    let failing = mutants
        .iter()
        .filter(|m| m.outcome == MutantOutcome::TestsFailed)
        .count();
    let derived = match failing {
        0 => TruthState::NotDiscriminated,
        k if k == mutants.len() => TruthState::Discriminated,
        _ => TruthState::PartiallyDiscriminated,
    };
    if !mutants.is_empty() && derived != case.truth.state {
        violations.push(format!(
            "case `{id}` truth `{}` does not follow from {failing}/{} mutants that failed the tests (`{}`)",
            case.truth.state.as_str(),
            mutants.len(),
            derived.as_str()
        ));
    }
    for mutant in mutants {
        if mutant.equivalence_review.trim().is_empty() {
            violations.push(format!(
                "case `{id}` mutant `{}` has no equivalence review",
                mutant.replacement
            ));
        }
        if (mutant.outcome == MutantOutcome::TestsFailed) != mutant.failing_test.is_some() {
            violations.push(format!(
                "case `{id}` mutant `{}` is `{}` but {} a failing test; name the failing test exactly when the outcome is tests_failed",
                mutant.replacement,
                mutant.outcome.as_str(),
                if mutant.failing_test.is_some() { "names" } else { "does not name" }
            ));
        }
    }
    let mut unsafe_path = false;
    for (field, path) in [("diff", &case.diff), ("anchor.file", &case.anchor.file)] {
        if !safe_relative(path) {
            violations.push(format!(
                "case `{id}` {field} `{path}` is unsafe; use a relative path without `..`"
            ));
            unsafe_path = true;
        }
    }
    if unsafe_path {
        return violations;
    }
    if !subject
        .retained_files
        .iter()
        .any(|f| f.path == case.anchor.file)
    {
        violations.push(format!(
            "case `{id}` anchor file `{}` is not retained by subject `{}`",
            case.anchor.file, subject.subject_id
        ));
    }
    match read(&dir.join(&case.diff)).and_then(|text| parse_patch(&text)) {
        Ok(patches) => {
            // Every patched path must be a retained file, so applying the
            // diff can only touch the run-owned copy of the subject.
            for patch in &patches {
                if !safe_relative(&patch.path)
                    || !subject.retained_files.iter().any(|f| f.path == patch.path)
                {
                    violations.push(format!(
                        "case `{id}` diff patches `{}`, which is unsafe or not retained by subject `{}`; patch only retained files",
                        patch.path, subject.subject_id
                    ));
                }
            }
            let touches = patches.iter().any(|patch| {
                patch.path == case.anchor.file && patch.added_lines().contains(&case.anchor.line)
            });
            if !touches {
                violations.push(format!(
                    "case `{id}` diff does not add line {} of `{}`",
                    case.anchor.line, case.anchor.file
                ));
            }
        }
        Err(err) => violations.push(format!("case `{id}` diff: {err}")),
    }
    violations
}

#[derive(Debug)]
pub(crate) struct FilePatch {
    pub(crate) path: String,
    pub(crate) hunks: Vec<Hunk>,
}

#[derive(Debug)]
pub(crate) struct Hunk {
    pub(crate) old_start: usize,
    pub(crate) new_start: usize,
    pub(crate) lines: Vec<(char, String)>,
}

impl FilePatch {
    pub(crate) fn added_lines(&self) -> BTreeSet<usize> {
        let mut added = BTreeSet::new();
        for hunk in &self.hunks {
            let mut line = hunk.new_start;
            for (kind, _) in &hunk.lines {
                match kind {
                    '+' => {
                        added.insert(line);
                        line += 1;
                    }
                    ' ' => line += 1,
                    _ => {}
                }
            }
        }
        added
    }
}

/// `@@ -a[,b] +c[,d] @@` as (old_start, old_count, new_start, new_count).
fn hunk_header(header: &str) -> Option<(usize, usize, usize, usize)> {
    let body = header.strip_prefix("@@ -")?;
    let (old, rest) = body.split_once(" +")?;
    let (new, _) = rest.split_once(" @@")?;
    let range = |range: &str| -> Option<(usize, usize)> {
        match range.split_once(',') {
            Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
            None => Some((range.parse().ok()?, 1)),
        }
    };
    let (old_start, old_count) = range(old)?;
    let (new_start, new_count) = range(new)?;
    Some((old_start, old_count, new_start, new_count))
}

/// Minimal unified-diff reader for the corpus's edit-in-place patches: one or
/// more existing files, no renames, creations or deletions. Each hunk body is
/// read to exactly the line counts its header declares, and each hunk's new
/// start must follow from its old start and the earlier hunks' size changes,
/// so an anchor read from `added_lines` names the line the edit really adds.
pub(crate) fn parse_patch(text: &str) -> Result<Vec<FilePatch>, String> {
    let mut patches: Vec<FilePatch> = Vec::new();
    let mut offset: i64 = 0;
    let mut lines = text.lines().enumerate().peekable();
    while let Some((idx, line)) = lines.next() {
        let at = idx + 1;
        if let Some(old) = line.strip_prefix("--- ") {
            let new = lines
                .next()
                .and_then(|(_, l)| l.strip_prefix("+++ "))
                .ok_or_else(|| format!("diff line {at}: `---` header is not followed by `+++`"))?;
            let old = old
                .strip_prefix("a/")
                .ok_or_else(|| format!("diff line {at}: old path `{old}` lacks the `a/` prefix"))?;
            let new = new.strip_prefix("b/").ok_or_else(|| {
                format!(
                    "diff line {}: new path `{new}` lacks the `b/` prefix",
                    at + 1
                )
            })?;
            if old != new {
                return Err(format!(
                    "diff line {at}: rename `{old}` -> `{new}` is not supported; edit one file in place"
                ));
            }
            patches.push(FilePatch {
                path: new.to_string(),
                hunks: Vec::new(),
            });
            offset = 0;
        } else if line.starts_with("@@") {
            let (old_start, old_count, new_start, new_count) = hunk_header(line)
                .ok_or_else(|| format!("diff line {at}: bad hunk header `{line}`"))?;
            // Corpus edits rewrite lines in place; a pure insertion or
            // deletion hunk has shifted start semantics the anchor check
            // cannot pin, so it is refused outright.
            if old_count == 0 || new_count == 0 {
                return Err(format!(
                    "diff line {at}: hunk `{line}` only inserts or only deletes; corpus edits must rewrite at least one line in place"
                ));
            }
            if new_start as i64 != old_start as i64 + offset {
                return Err(format!(
                    "diff line {at}: hunk new start {new_start} does not follow from old start {old_start} and earlier hunks (expected {}); regenerate the diff",
                    old_start as i64 + offset
                ));
            }
            let patch = patches.last_mut().ok_or_else(|| {
                format!("diff line {at}: hunk before any `---`/`+++` file header")
            })?;
            let mut hunk = Hunk {
                old_start,
                new_start,
                lines: Vec::new(),
            };
            let (mut old_seen, mut new_seen) = (0usize, 0usize);
            while old_seen < old_count || new_seen < new_count {
                let Some((body_idx, next)) = lines.next() else {
                    return Err(format!(
                        "diff line {at}: hunk ends early ({old_seen}/{old_count} old and {new_seen}/{new_count} new lines); regenerate the diff"
                    ));
                };
                let kind = next.chars().next().unwrap_or(' ');
                match kind {
                    ' ' => {
                        old_seen += 1;
                        new_seen += 1;
                    }
                    '-' => old_seen += 1,
                    '+' => new_seen += 1,
                    '\\' => continue,
                    _ => {
                        return Err(format!(
                            "diff line {}: `{next}` is not a hunk body line; the hunk header declares more lines than follow",
                            body_idx + 1
                        ));
                    }
                }
                hunk.lines
                    .push((kind, next.get(1..).unwrap_or_default().to_string()));
            }
            if old_seen != old_count || new_seen != new_count {
                return Err(format!(
                    "diff line {at}: hunk body has {old_seen} old and {new_seen} new lines but the header declares {old_count} and {new_count}"
                ));
            }
            if lines.peek().is_some_and(|(_, l)| l.starts_with('\\')) {
                lines.next();
            }
            offset += new_count as i64 - old_count as i64;
            patch.hunks.push(hunk);
        }
    }
    if patches.is_empty() {
        return Err("diff has no `---`/`+++` file patches".to_string());
    }
    Ok(patches)
}

/// Apply one file patch, refusing on any context or removed-line mismatch so
/// a drifted excerpt fails loudly instead of measuring a different edit.
pub(crate) fn apply_patch(original: &str, patch: &FilePatch) -> Result<String, String> {
    let trailing_newline = original.ends_with('\n');
    let source: Vec<&str> = original.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut cursor = 0usize;
    for hunk in &patch.hunks {
        let start = hunk.old_start.saturating_sub(1);
        if start < cursor || start > source.len() {
            return Err(format!(
                "{}: hunk at line {} is out of order",
                patch.path, hunk.old_start
            ));
        }
        out.extend(source[cursor..start].iter().map(|s| (*s).to_string()));
        cursor = start;
        for (kind, text) in &hunk.lines {
            match kind {
                ' ' | '-' => {
                    if source.get(cursor) != Some(&text.as_str()) {
                        return Err(format!(
                            "{}: line {} does not match the patch context",
                            patch.path,
                            cursor + 1
                        ));
                    }
                    if *kind == ' ' {
                        out.push(text.clone());
                    }
                    cursor += 1;
                }
                _ => out.push(text.clone()),
            }
        }
    }
    out.extend(source[cursor..].iter().map(|s| (*s).to_string()));
    let mut text = out.join("\n");
    if trailing_newline {
        text.push('\n');
    }
    Ok(text)
}

/// Copy a stored subject, restoring upstream `.rs` names.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    for rel in files_under(from)? {
        let logical = logical_path(&rel).ok_or_else(|| {
            format!(
                "{} is stored as bare Rust source; run validate",
                normalize_path(&from.join(&rel))
            )
        })?;
        let target = to.join(&logical);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("create {}: {err}", normalize_path(parent)))?;
        }
        fs::copy(from.join(&rel), &target)
            .map_err(|err| format!("copy {}: {err}", normalize_path(&target)))?;
    }
    Ok(())
}

/// Copy the retained subject into a fresh run-owned workspace and apply the
/// case edit there; the tracked excerpt is never mutated.
fn materialize(dir: &Path, case: &Case, work_root: &Path) -> Result<(PathBuf, PathBuf), String> {
    let work = work_root.join(&case.case_id);
    match fs::remove_dir_all(&work) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("clear {}: {err}", normalize_path(&work))),
    }
    copy_tree(&dir.join("subjects").join(&case.subject_id), &work)?;
    let diff_path = dir.join(&case.diff);
    let context = |err: String| {
        format!(
            "case `{}` ({}): {err}; the retained excerpt and the case diff disagree, so restore the excerpt or regenerate the diff",
            case.case_id,
            normalize_path(&diff_path)
        )
    };
    for patch in parse_patch(&read(&diff_path)?).map_err(context)? {
        // validate() already refuses these; refuse again here so no caller
        // can write outside the run-owned copy.
        if !safe_relative(&patch.path) {
            return Err(context(format!("patch path `{}` is unsafe", patch.path)));
        }
        let target = work.join(&patch.path);
        let patched = apply_patch(&read(&target)?, &patch).map_err(context)?;
        fs::write(&target, patched)
            .map_err(|err| format!("write {}: {err}", normalize_path(&target)))?;
    }
    let absolute = |p: &Path| {
        std::path::absolute(p).map_err(|err| format!("resolve {}: {err}", normalize_path(p)))
    };
    Ok((absolute(&work)?, absolute(&diff_path)?))
}

fn run_case(dir: &Path, case: &Case, work_root: &Path) -> Result<Value, String> {
    let (work, diff) = materialize(dir, case, work_root)?;
    let cache = std::path::absolute(work_root.join(".cache").join(&case.case_id))
        .map_err(|err| format!("resolve cache dir: {err}"))?;
    let binary = ripr_fixture_binary()?;
    let args = vec![
        "check".to_string(),
        "--root".to_string(),
        work.to_string_lossy().into_owned(),
        "--diff".to_string(),
        diff.to_string_lossy().into_owned(),
        "--json".to_string(),
    ];
    let cache_value = cache.to_string_lossy().into_owned();
    let stdout = run_output_owned_with_envs(&binary, &args, &[(CACHE_ENV, &cache_value)])?;
    let mut check: Value = serde_json::from_str(&stdout).map_err(|err| {
        format!(
            "case `{}`: ripr check did not emit JSON: {err}",
            case.case_id
        )
    })?;
    relativize_probe_files(&mut check, &work);
    // ripr may report the canonical root (macOS `/var` is `/private/var`);
    // strip that spelling too so findings still land on their anchors.
    if let Ok(canonical) = fs::canonicalize(&work) {
        relativize_probe_files(&mut check, &canonical);
    }
    Ok(check)
}

/// ripr names probe files by joining them onto the absolute `--root`; the
/// corpus anchors are root-relative, so strip the run-owned workspace prefix.
pub(crate) fn relativize_probe_files(check: &mut Value, root: &Path) {
    let prefix = format!("{}/", normalize_path(root).trim_end_matches('/'));
    if let Some(findings) = check.get_mut("findings").and_then(Value::as_array_mut) {
        for finding in findings {
            if let Some(file) = finding.pointer_mut("/probe/file")
                && let Some(text) = file.as_str()
            {
                let text = text.replace('\\', "/");
                if let Some(rel) = text.strip_prefix(&prefix) {
                    *file = Value::String(rel.to_string());
                }
            }
        }
    }
}

fn run_corpus(dir: &Path, corpus: &Corpus, work_root: &Path) -> Result<Report, String> {
    let mut checks = Vec::new();
    for case in &corpus.cases {
        checks.push((case.case_id.clone(), run_case(dir, case, work_root)?));
    }
    build_report(corpus, &checks)
}

fn validated_corpus(dir: &Path) -> Result<Corpus, String> {
    let corpus = load_corpus(dir)?;
    let violations = validate(&corpus, dir);
    if violations.is_empty() {
        Ok(corpus)
    } else {
        Err(format!(
            "verdict corpus is invalid:\n- {}",
            violations.join("\n- ")
        ))
    }
}

fn first_differing_line(expected: &str, actual: &str) -> String {
    for (index, (e, a)) in expected.lines().zip(actual.lines()).enumerate() {
        if e != a {
            return format!("line {}: expected `{e}`, got `{a}`", index + 1);
        }
    }
    "length differs".to_string()
}

pub(crate) fn verdict_corpus(args: &[String]) -> Result<(), String> {
    let dir = Path::new(CORPUS_DIR);
    let mut iter = args.iter();
    let sub = iter.next().map(String::as_str).unwrap_or("check");
    let mut out = PathBuf::from(DEFAULT_OUT);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" => {
                out = PathBuf::from(iter.next().ok_or("--out needs a directory")?);
            }
            other => return Err(format!("verdict-corpus: unknown argument `{other}`")),
        }
    }
    match sub {
        "validate" => {
            if args.len() > 1 {
                return Err(
                    "verdict-corpus validate takes no options; `--out` applies to report and check"
                        .to_string(),
                );
            }
            let corpus = validated_corpus(dir)?;
            println!(
                "verdict-corpus: {} cases across {} subjects are valid",
                corpus.cases.len(),
                corpus.subjects.len()
            );
            Ok(())
        }
        "report" | "check" => {
            let corpus = validated_corpus(dir)?;
            // Read the golden before writing anything, so an `--out` that
            // aliases the expected directory cannot make the check pass.
            let expected_path = dir.join("expected").join("report.json");
            let expected_md_path = dir.join("expected").join("report.md");
            let expected = if sub == "check" {
                Some((read(&expected_path)?, read(&expected_md_path)?))
            } else {
                None
            };
            // Canonical paths collapse `..` and symlinks, so no spelling of
            // the expected directory slips past the guard.
            let canonical = |p: &Path| {
                fs::canonicalize(p).map_err(|err| format!("resolve {}: {err}", normalize_path(p)))
            };
            if sub == "check" {
                fs::create_dir_all(&out)
                    .map_err(|err| format!("create {}: {err}", normalize_path(&out)))?;
            }
            if sub == "check" && canonical(&out)? == canonical(&dir.join("expected"))? {
                return Err(format!(
                    "verdict-corpus: --out {} is the expected-report directory; use `report --out` there to re-bless deliberately when a verdict change is intended",
                    normalize_path(&out)
                ));
            }
            let report = run_corpus(dir, &corpus, Path::new("target/ripr/verdict-corpus"))?;
            let json = render_report_json(&report)?;
            let markdown = render_report_markdown(&report);
            fs::create_dir_all(&out)
                .map_err(|err| format!("create {}: {err}", normalize_path(&out)))?;
            fs::write(out.join("report.json"), &json)
                .map_err(|err| format!("write report.json: {err}"))?;
            fs::write(out.join("report.md"), &markdown)
                .map_err(|err| format!("write report.md: {err}"))?;
            println!(
                "verdict-corpus: false verdicts {} ({}/{}), contradictions {} ({}/{}); wrote {}",
                report.false_verdict_rate.rate,
                report.false_verdict_rate.numerator,
                report.false_verdict_rate.denominator,
                report.contradiction_rate.rate,
                report.contradiction_rate.numerator,
                report.contradiction_rate.denominator,
                normalize_path(&out)
            );
            if let Some((_, expected_md)) = &expected
                && *expected_md != markdown
            {
                return Err(format!(
                    "verdict-corpus: {} drifted from the rendered report ({}); re-bless both expected files with `report --out {}`",
                    normalize_path(&expected_md_path),
                    first_differing_line(expected_md, &markdown),
                    normalize_path(&dir.join("expected"))
                ));
            }
            if let Some((expected, _)) = expected
                && expected != json
            {
                return Err(format!(
                    "verdict-corpus: report drifted from {} ({}). A verdict changed; read {} and, if the change is intended, copy it over the expected report with the reason in the PR.",
                    normalize_path(&expected_path),
                    first_differing_line(&expected, &json),
                    normalize_path(&out.join("report.md"))
                ));
            }
            Ok(())
        }
        other => Err(format!(
            "verdict-corpus: unknown subcommand `{other}` (expected validate, check, or report)"
        )),
    }
}

#[cfg(test)]
#[path = "verdict_corpus_tests.rs"]
mod tests;
