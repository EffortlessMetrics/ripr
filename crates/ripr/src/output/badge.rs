//! Private badge summary model and renderer.
//!
//! This module is the rendering substrate for the `ripr` and (future)
//! `ripr+` badges. Its types are intentionally crate-private — the public
//! contract is the JSON wire shape, not the Rust types. See
//! [`docs/BADGE_POLICY.md`](../../../../../docs/BADGE_POLICY.md) for the
//! locked semantics, color thresholds, and JSON shape.
//!
//! Both `ripr` (exposure-gap count) and `ripr+` (exposure + actionable
//! test-efficiency, minus declared intent) badge formats are supported.
//! Suppressions, CI artifacts, and the published Shields endpoint live
//! in their own scoped PRs.

use crate::app::CheckOutput;
use crate::domain::ExposureClass;
use crate::output::json::escape as json_escape;
use crate::output::suppressions::{
    SuppressionEntry, apply_exposure_suppressions, apply_test_efficiency_suppressions,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeKind {
    /// Counts unsuppressed static exposure gaps only.
    Ripr,
    /// Counts unsuppressed exposure gaps plus unsuppressed actionable
    /// test-efficiency findings (excluding declared intent).
    RiprPlus,
}

impl BadgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BadgeKind::Ripr => "ripr",
            BadgeKind::RiprPlus => "ripr_plus",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BadgeKind::Ripr => "ripr",
            BadgeKind::RiprPlus => "ripr+",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeStatus {
    Pass,
    Warn,
    Fail,
}

impl BadgeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            BadgeStatus::Pass => "pass",
            BadgeStatus::Warn => "warn",
            BadgeStatus::Fail => "fail",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadgeCounts {
    pub unsuppressed_exposure_gaps: usize,
    pub unsuppressed_test_efficiency_findings: usize,
    pub intentional_test_efficiency_findings: usize,
    pub suppressed_exposure_gaps: usize,
    pub suppressed_test_efficiency_findings: usize,
    pub unknowns: usize,
    pub unknowns_test_efficiency: usize,
    pub analyzed_findings: usize,
    pub analyzed_tests: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadgePolicy {
    pub include_unknowns: bool,
    pub fail_on_nonzero: bool,
    pub test_intent_path: String,
    pub suppressions_path: String,
}

impl Default for BadgePolicy {
    fn default() -> Self {
        Self {
            include_unknowns: false,
            fail_on_nonzero: false,
            test_intent_path: ".ripr/test_intent.toml".to_string(),
            suppressions_path: ".ripr/suppressions.toml".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadgeSummary {
    pub kind: BadgeKind,
    pub message: String,
    pub status: BadgeStatus,
    pub color: &'static str,
    pub counts: BadgeCounts,
    pub reason_counts: BTreeMap<&'static str, usize>,
    pub policy: BadgePolicy,
    /// Advisory warnings surfaced to the badge consumer — currently
    /// expired suppressions and unmatched suppression selectors. Empty
    /// for the common-case green badge.
    pub warnings: Vec<String>,
}

/// The schema_version of the native badge JSON. Bumping it is a public
/// contract change — call it out in the PR.
pub const BADGE_SCHEMA_VERSION: &str = "0.1";

/// All test-efficiency reason strings the badge JSON reports as zero
/// defaults until later PRs read the test-efficiency report. The order
/// matches `RIPR-SPEC-0004` and the existing emitter in `xtask`.
const BADGE_REASON_KEYS: &[&str] = &[
    "no_assertion_detected",
    "smoke_oracle_only",
    "relational_oracle",
    "broad_oracle",
    "assertion_may_not_match_detected_owner",
    "opaque_helper_or_fixture_boundary",
    "no_activation_literal_detected",
    "expected_value_computed_from_detected_owner_path",
    "duplicate_activation_and_oracle_shape",
];

/// Builds the `ripr` badge summary from a `CheckOutput`, applying any
/// `kind = "exposure_gap"` suppressions whose `finding_id` matches a
/// currently-counted exposure gap. Expired and unmatched suppressions
/// surface as `warnings` so silently-stale debt cannot keep the badge
/// green. `today` is the ISO date used for expiry comparison.
pub fn ripr_badge_summary_with_suppressions(
    output: &CheckOutput,
    suppressions: &[SuppressionEntry],
    today: &str,
    policy: BadgePolicy,
) -> BadgeSummary {
    let mut candidate_ids: Vec<String> = Vec::new();
    let mut unknowns = 0usize;
    let mut unique_tests: BTreeSet<(String, String, usize)> = BTreeSet::new();

    for finding in &output.findings {
        match finding.class {
            ExposureClass::WeaklyExposed
            | ExposureClass::ReachableUnrevealed
            | ExposureClass::NoStaticPath => {
                candidate_ids.push(finding.id.clone());
            }
            ExposureClass::InfectionUnknown
            | ExposureClass::PropagationUnknown
            | ExposureClass::StaticUnknown => {
                unknowns += 1;
            }
            ExposureClass::Exposed => {}
        }
        for test in &finding.related_tests {
            unique_tests.insert((
                test.file.to_string_lossy().into_owned(),
                test.name.clone(),
                test.line,
            ));
        }
    }

    let suppression_app = apply_exposure_suppressions(&candidate_ids, suppressions, today);
    let suppressed = suppression_app.suppressed_findings.len();
    let unsuppressed_exposure_gaps = candidate_ids.len().saturating_sub(suppressed);

    let counts = BadgeCounts {
        unsuppressed_exposure_gaps,
        unsuppressed_test_efficiency_findings: 0,
        intentional_test_efficiency_findings: 0,
        suppressed_exposure_gaps: suppressed,
        suppressed_test_efficiency_findings: 0,
        unknowns,
        unknowns_test_efficiency: 0,
        analyzed_findings: output.findings.len(),
        analyzed_tests: unique_tests.len(),
    };

    let mut reason_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for key in BADGE_REASON_KEYS {
        reason_counts.insert(key, 0);
    }

    let headline = counts.unsuppressed_exposure_gaps
        + if policy.include_unknowns {
            counts.unknowns
        } else {
            0
        };
    let (status, color) = badge_status_color(headline, policy.fail_on_nonzero);

    BadgeSummary {
        kind: BadgeKind::Ripr,
        message: headline.to_string(),
        status,
        color,
        counts,
        reason_counts,
        policy,
        warnings: suppression_app.warnings,
    }
}

/// Convenience wrapper: builds the `ripr` badge with no suppressions.
/// Equivalent to calling [`ripr_badge_summary_with_suppressions`] with
/// an empty slice. Test-only since production callers always go through
/// [`crate::app::render_check`] which threads the loaded suppressions.
#[cfg(test)]
pub fn ripr_badge_summary(output: &CheckOutput, policy: BadgePolicy) -> BadgeSummary {
    ripr_badge_summary_with_suppressions(output, &[], "", policy)
}

fn badge_status_color(count: usize, fail_on_nonzero: bool) -> (BadgeStatus, &'static str) {
    if fail_on_nonzero && count > 0 {
        return (BadgeStatus::Fail, "red");
    }
    match count {
        0 => (BadgeStatus::Pass, "brightgreen"),
        1..=3 => (BadgeStatus::Warn, "yellow"),
        _ => (BadgeStatus::Warn, "orange"),
    }
}

/// One actionable test-efficiency entry seen by the badge, retained so
/// suppressions can be applied per-`(test, path)` after the report is
/// parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestEfficiencyBadgeEntry {
    pub test: String,
    pub path: String,
    pub has_intent: bool,
}

/// Test-efficiency contribution to the `ripr+` badge. Built by parsing
/// `target/ripr/reports/test-efficiency.json`; the per-test ledger is
/// the source of truth because `declared_intent` exclusion is per-test
/// and cannot be derived from aggregate `class_counts` alone. Actionable
/// entries are kept on the side so the badge orchestrator can apply
/// suppressions after the parse.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TestEfficiencyBadgeSummary {
    pub unsuppressed_test_efficiency_findings: usize,
    pub intentional_test_efficiency_findings: usize,
    pub unknowns_test_efficiency: usize,
    pub analyzed_tests: usize,
    pub reason_counts: BTreeMap<&'static str, usize>,
    /// Actionable, non-intentional entries — i.e., the candidate set for
    /// `ripr+` suppression matching. Empty when no test-efficiency entry
    /// is actionable.
    pub actionable_entries: Vec<TestEfficiencyBadgeEntry>,
}

/// The test-efficiency `class` strings that contribute to `ripr+` when not
/// covered by `declared_intent`. Mirrors the locked vocabulary in
/// `docs/BADGE_POLICY.md`. `strong_discriminator` and `useful_but_broad`
/// never count by default; `opaque` flows into `unknowns_test_efficiency`
/// rather than the headline.
const ACTIONABLE_TE_CLASSES: &[&str] = &[
    "likely_vacuous",
    "possibly_circular",
    "smoke_only",
    "duplicative",
];

const NON_ACTIONABLE_TE_CLASSES: &[&str] = &["strong_discriminator", "useful_but_broad"];

/// Parses `target/ripr/reports/test-efficiency.json` into the
/// `ripr+`-shaped summary. Validates the schema_version, requires the
/// per-test ledger, and rejects unknown class strings so a class name
/// drift in the emitter surfaces as a parse error rather than a silent
/// undercount.
pub fn parse_test_efficiency_badge_summary(
    text: &str,
) -> Result<TestEfficiencyBadgeSummary, String> {
    let value: Value = serde_json::from_str(text)
        .map_err(|err| format!("test-efficiency.json is not valid JSON: {err}"))?;

    let schema_version = value
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| "test-efficiency.json is missing `schema_version`".to_string())?;
    if schema_version != "0.1" {
        return Err(format!(
            "test-efficiency.json schema_version `{schema_version}` is not supported (expected `0.1`)"
        ));
    }

    let tests = value
        .get("tests")
        .and_then(Value::as_array)
        .ok_or_else(|| "test-efficiency.json is missing the `tests` array".to_string())?;

    let mut unsuppressed = 0usize;
    let mut intentional = 0usize;
    let mut unknowns_te = 0usize;
    let mut actionable_entries: Vec<TestEfficiencyBadgeEntry> = Vec::new();

    for entry in tests {
        let class = entry
            .get("class")
            .and_then(Value::as_str)
            .ok_or_else(|| "test-efficiency entry is missing `class`".to_string())?;
        let has_intent = entry.get("declared_intent").is_some();

        if ACTIONABLE_TE_CLASSES.contains(&class) {
            if has_intent {
                intentional += 1;
            } else {
                unsuppressed += 1;
                let test_name = entry
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let path = entry
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                actionable_entries.push(TestEfficiencyBadgeEntry {
                    test: test_name,
                    path,
                    has_intent: false,
                });
            }
        } else if class == "opaque" {
            unknowns_te += 1;
        } else if NON_ACTIONABLE_TE_CLASSES.contains(&class) {
            // strong_discriminator / useful_but_broad: visible only.
        } else {
            return Err(format!(
                "test-efficiency entry has unknown class `{class}`; recognized classes are {}",
                [
                    ACTIONABLE_TE_CLASSES,
                    NON_ACTIONABLE_TE_CLASSES,
                    &["opaque"],
                ]
                .concat()
                .join(", ")
            ));
        }
    }

    let analyzed_tests = value
        .get("metrics")
        .and_then(|m| m.get("tests_scanned"))
        .and_then(Value::as_u64)
        .ok_or_else(|| "test-efficiency.json is missing `metrics.tests_scanned`".to_string())?
        as usize;

    let mut reason_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for key in BADGE_REASON_KEYS {
        reason_counts.insert(key, 0);
    }
    if let Some(counts) = value
        .get("metrics")
        .and_then(|m| m.get("reason_counts"))
        .and_then(Value::as_object)
    {
        for (key, value) in counts {
            if let Some(known) = BADGE_REASON_KEYS
                .iter()
                .find(|known| **known == key.as_str())
                && let Some(count) = value.as_u64()
            {
                reason_counts.insert(*known, count as usize);
            }
        }
    }

    Ok(TestEfficiencyBadgeSummary {
        unsuppressed_test_efficiency_findings: unsuppressed,
        intentional_test_efficiency_findings: intentional,
        unknowns_test_efficiency: unknowns_te,
        analyzed_tests,
        reason_counts,
        actionable_entries,
    })
}

/// Builds the `ripr+` badge summary from a `CheckOutput` plus a parsed
/// test-efficiency contribution and a slice of suppressions. Applies
/// `exposure_gap` suppressions to the exposure side and
/// `test_efficiency` suppressions to the actionable test-efficiency
/// entries; expired and unmatched selectors surface as `warnings`.
pub fn ripr_plus_badge_summary_with_suppressions(
    output: &CheckOutput,
    test_efficiency: TestEfficiencyBadgeSummary,
    suppressions: &[SuppressionEntry],
    today: &str,
    policy: BadgePolicy,
) -> BadgeSummary {
    let exposure =
        ripr_badge_summary_with_suppressions(output, suppressions, today, policy.clone());

    // Apply test-efficiency suppressions against the actionable entries
    // surfaced by the test-efficiency parser. Suppressed entries shift
    // from `unsuppressed_test_efficiency_findings` to
    // `suppressed_test_efficiency_findings`. `intentional_*` is
    // unaffected — declared intent and suppressions are distinct.
    let candidate_pairs: Vec<(String, String)> = test_efficiency
        .actionable_entries
        .iter()
        .map(|entry| (entry.test.clone(), entry.path.clone()))
        .collect();
    let te_application = apply_test_efficiency_suppressions(&candidate_pairs, suppressions, today);
    let suppressed_te = te_application.suppressed_tests.len();
    let unsuppressed_te = test_efficiency
        .unsuppressed_test_efficiency_findings
        .saturating_sub(suppressed_te);

    let counts = BadgeCounts {
        unsuppressed_exposure_gaps: exposure.counts.unsuppressed_exposure_gaps,
        unsuppressed_test_efficiency_findings: unsuppressed_te,
        intentional_test_efficiency_findings: test_efficiency.intentional_test_efficiency_findings,
        suppressed_exposure_gaps: exposure.counts.suppressed_exposure_gaps,
        suppressed_test_efficiency_findings: suppressed_te,
        unknowns: exposure.counts.unknowns,
        unknowns_test_efficiency: test_efficiency.unknowns_test_efficiency,
        analyzed_findings: exposure.counts.analyzed_findings,
        analyzed_tests: test_efficiency.analyzed_tests,
    };

    let unknown_contribution = if policy.include_unknowns {
        counts.unknowns + counts.unknowns_test_efficiency
    } else {
        0
    };
    let headline = counts.unsuppressed_exposure_gaps
        + counts.unsuppressed_test_efficiency_findings
        + unknown_contribution;
    let (status, color) = badge_status_color(headline, policy.fail_on_nonzero);

    let mut warnings = exposure.warnings;
    warnings.extend(te_application.warnings);

    BadgeSummary {
        kind: BadgeKind::RiprPlus,
        message: headline.to_string(),
        status,
        color,
        counts,
        reason_counts: test_efficiency.reason_counts,
        policy,
        warnings,
    }
}

/// Convenience wrapper: builds the `ripr+` badge with no suppressions.
/// Test-only — production calls
/// [`ripr_plus_badge_summary_with_suppressions`] via
/// [`crate::app::render_check`].
#[cfg(test)]
pub fn ripr_plus_badge_summary(
    output: &CheckOutput,
    test_efficiency: TestEfficiencyBadgeSummary,
    policy: BadgePolicy,
) -> BadgeSummary {
    ripr_plus_badge_summary_with_suppressions(output, test_efficiency, &[], "", policy)
}

/// Renders the native badge JSON (snake_case, full counts/reasons/policy).
pub fn render_native_json(summary: &BadgeSummary) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": \"{BADGE_SCHEMA_VERSION}\",\n"
    ));
    out.push_str(&format!("  \"kind\": \"{}\",\n", summary.kind.as_str()));
    out.push_str(&format!(
        "  \"label\": \"{}\",\n",
        json_escape(summary.kind.label())
    ));
    out.push_str(&format!(
        "  \"message\": \"{}\",\n",
        json_escape(&summary.message)
    ));
    out.push_str(&format!("  \"status\": \"{}\",\n", summary.status.as_str()));
    out.push_str(&format!("  \"color\": \"{}\",\n", summary.color));

    let counts = &summary.counts;
    out.push_str("  \"counts\": {\n");
    out.push_str(&format!(
        "    \"unsuppressed_exposure_gaps\": {},\n",
        counts.unsuppressed_exposure_gaps
    ));
    out.push_str(&format!(
        "    \"unsuppressed_test_efficiency_findings\": {},\n",
        counts.unsuppressed_test_efficiency_findings
    ));
    out.push_str(&format!(
        "    \"intentional_test_efficiency_findings\": {},\n",
        counts.intentional_test_efficiency_findings
    ));
    out.push_str(&format!(
        "    \"suppressed_exposure_gaps\": {},\n",
        counts.suppressed_exposure_gaps
    ));
    out.push_str(&format!(
        "    \"suppressed_test_efficiency_findings\": {},\n",
        counts.suppressed_test_efficiency_findings
    ));
    out.push_str(&format!("    \"unknowns\": {},\n", counts.unknowns));
    out.push_str(&format!(
        "    \"unknowns_test_efficiency\": {},\n",
        counts.unknowns_test_efficiency
    ));
    out.push_str(&format!(
        "    \"analyzed_findings\": {},\n",
        counts.analyzed_findings
    ));
    out.push_str(&format!(
        "    \"analyzed_tests\": {}\n",
        counts.analyzed_tests
    ));
    out.push_str("  },\n");

    out.push_str("  \"reason_counts\": {");
    if summary.reason_counts.is_empty() {
        out.push_str("},\n");
    } else {
        out.push('\n');
        // Render in the canonical order the badge reserves, not BTreeMap
        // alpha order, so consumers see the policy-aligned sequence.
        let mut wrote_any = false;
        for key in BADGE_REASON_KEYS {
            if let Some(count) = summary.reason_counts.get(*key) {
                if wrote_any {
                    out.push_str(",\n");
                }
                out.push_str(&format!("    \"{}\": {}", json_escape(key), count));
                wrote_any = true;
            }
        }
        out.push_str("\n  },\n");
    }

    let policy = &summary.policy;
    out.push_str("  \"policy\": {\n");
    out.push_str(&format!(
        "    \"include_unknowns\": {},\n",
        policy.include_unknowns
    ));
    out.push_str(&format!(
        "    \"fail_on_nonzero\": {},\n",
        policy.fail_on_nonzero
    ));
    out.push_str(&format!(
        "    \"test_intent_path\": \"{}\",\n",
        json_escape(&policy.test_intent_path)
    ));
    out.push_str(&format!(
        "    \"suppressions_path\": \"{}\"\n",
        json_escape(&policy.suppressions_path)
    ));
    out.push_str("  },\n");

    // Always emit `warnings` as an array (possibly empty) so consumers
    // can rely on a stable shape. Currently used for expired
    // suppressions and unmatched suppression selectors.
    out.push_str("  \"warnings\": [");
    if summary.warnings.is_empty() {
        out.push_str("]\n}\n");
    } else {
        out.push('\n');
        for (index, warning) in summary.warnings.iter().enumerate() {
            if index > 0 {
                out.push_str(",\n");
            }
            out.push_str(&format!("    \"{}\"", json_escape(warning)));
        }
        out.push_str("\n  ]\n}\n");
    }
    out
}

/// Renders the Shields-compatible projection: exactly four top-level
/// fields (`schemaVersion`, `label`, `message`, `color`).
pub fn render_shields_json(summary: &BadgeSummary) -> String {
    format!(
        "{{\n  \"schemaVersion\": 1,\n  \"label\": \"{}\",\n  \"message\": \"{}\",\n  \"color\": \"{}\"\n}}\n",
        json_escape(summary.kind.label()),
        json_escape(&summary.message),
        summary.color
    )
}

#[cfg(test)]
mod tests {
    use super::{
        BADGE_REASON_KEYS, BadgePolicy, BadgeStatus, TestEfficiencyBadgeSummary,
        badge_status_color, parse_test_efficiency_badge_summary, render_native_json,
        render_shields_json, ripr_badge_summary, ripr_plus_badge_summary,
    };
    use crate::app::{CheckInput, CheckOutput, Mode};
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, OracleKind,
        OracleStrength, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence,
        SourceLocation, StageEvidence, StageState, Summary,
    };
    use std::path::PathBuf;

    fn finding(class: ExposureClass, related: Vec<RelatedTest>) -> Finding {
        Finding {
            id: "probe:src_lib_rs:1:predicate".to_string(),
            probe: Probe {
                id: ProbeId("probe:src_lib_rs:1:predicate".to_string()),
                family: ProbeFamily::Predicate,
                location: SourceLocation::new("src/lib.rs", 1, 1),
                owner: None,
                delta: DeltaKind::Control,
                before: None,
                after: None,
                expression: "expr".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class,
            ripr: RiprEvidence {
                reach: StageEvidence::new(StageState::Yes, Confidence::Medium, "reached"),
                infect: StageEvidence::new(StageState::Weak, Confidence::Low, "infected"),
                propagate: StageEvidence::new(StageState::No, Confidence::Medium, "not propagated"),
                reveal: RevealEvidence {
                    observe: StageEvidence::new(StageState::Weak, Confidence::Low, "observed"),
                    discriminate: StageEvidence::new(
                        StageState::No,
                        Confidence::Medium,
                        "no discriminator",
                    ),
                },
            },
            confidence: 0.5,
            evidence: Vec::new(),
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: Vec::new(),
            related_tests: related,
            recommended_next_step: None,
        }
    }

    fn related_test(name: &str, file: &str, line: usize) -> RelatedTest {
        RelatedTest {
            name: name.to_string(),
            file: PathBuf::from(file),
            line,
            oracle: None,
            oracle_kind: OracleKind::Unknown,
            oracle_strength: OracleStrength::Weak,
        }
    }

    fn check_output(findings: Vec<Finding>) -> CheckOutput {
        let defaults = CheckInput::default();
        CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: defaults.root,
            base: defaults.base,
            summary: Summary::default(),
            findings,
        }
    }

    #[test]
    fn badge_summary_counts_weakly_exposed_reachable_unrevealed_and_no_static_path() {
        let output = check_output(vec![
            finding(ExposureClass::WeaklyExposed, vec![]),
            finding(ExposureClass::ReachableUnrevealed, vec![]),
            finding(ExposureClass::NoStaticPath, vec![]),
        ]);

        let summary = ripr_badge_summary(&output, BadgePolicy::default());

        assert_eq!(summary.counts.unsuppressed_exposure_gaps, 3);
        assert_eq!(summary.message, "3");
    }

    #[test]
    fn badge_summary_does_not_count_exposed_findings() {
        let output = check_output(vec![
            finding(ExposureClass::Exposed, vec![]),
            finding(ExposureClass::Exposed, vec![]),
        ]);

        let summary = ripr_badge_summary(&output, BadgePolicy::default());

        assert_eq!(summary.counts.unsuppressed_exposure_gaps, 0);
        assert_eq!(summary.counts.analyzed_findings, 2);
        assert_eq!(summary.message, "0");
        assert_eq!(summary.status, BadgeStatus::Pass);
        assert_eq!(summary.color, "brightgreen");
    }

    #[test]
    fn badge_summary_reports_unknowns_separately_from_headline() {
        let output = check_output(vec![
            finding(ExposureClass::InfectionUnknown, vec![]),
            finding(ExposureClass::PropagationUnknown, vec![]),
            finding(ExposureClass::StaticUnknown, vec![]),
            finding(ExposureClass::WeaklyExposed, vec![]),
        ]);

        let summary = ripr_badge_summary(&output, BadgePolicy::default());

        assert_eq!(summary.counts.unsuppressed_exposure_gaps, 1);
        assert_eq!(summary.counts.unknowns, 3);
        // Headline excludes unknowns by default.
        assert_eq!(summary.message, "1");
    }

    #[test]
    fn badge_summary_message_never_contains_a_denominator() {
        let output = check_output(vec![
            finding(ExposureClass::WeaklyExposed, vec![]),
            finding(ExposureClass::Exposed, vec![]),
            finding(ExposureClass::Exposed, vec![]),
        ]);

        let summary = ripr_badge_summary(&output, BadgePolicy::default());

        assert!(!summary.message.contains('/'), "no denominator");
        assert!(!summary.message.to_ascii_lowercase().contains("coverage"));
        assert!(!summary.message.to_ascii_lowercase().contains("uncovered"));
        assert_eq!(summary.message, "1");
    }

    #[test]
    fn badge_status_color_zero_is_pass_brightgreen() {
        assert_eq!(
            badge_status_color(0, false),
            (BadgeStatus::Pass, "brightgreen")
        );
    }

    #[test]
    fn badge_status_color_one_to_three_is_warn_yellow() {
        for count in 1..=3 {
            assert_eq!(
                badge_status_color(count, false),
                (BadgeStatus::Warn, "yellow"),
                "count {count}",
            );
        }
    }

    #[test]
    fn badge_status_color_four_or_more_is_warn_orange() {
        for count in [4, 5, 12, 100] {
            assert_eq!(
                badge_status_color(count, false),
                (BadgeStatus::Warn, "orange"),
                "count {count}",
            );
        }
    }

    #[test]
    fn badge_status_color_fail_on_nonzero_promotes_warn_to_fail_red() {
        assert_eq!(
            badge_status_color(1, true),
            (BadgeStatus::Fail, "red"),
            "fail_on_nonzero with count 1"
        );
        assert_eq!(
            badge_status_color(7, true),
            (BadgeStatus::Fail, "red"),
            "fail_on_nonzero with count 7"
        );
        // Zero remains pass even with fail_on_nonzero.
        assert_eq!(
            badge_status_color(0, true),
            (BadgeStatus::Pass, "brightgreen"),
            "zero stays pass even with fail_on_nonzero"
        );
    }

    #[test]
    fn badge_native_json_uses_snake_case_schema_version_and_all_required_fields() {
        let output = check_output(vec![finding(ExposureClass::WeaklyExposed, vec![])]);
        let summary = ripr_badge_summary(&output, BadgePolicy::default());
        let json = render_native_json(&summary);

        assert!(json.contains("\"schema_version\": \"0.1\""));
        assert!(!json.contains("\"schemaVersion\""));
        assert!(json.contains("\"kind\": \"ripr\""));
        assert!(json.contains("\"label\": \"ripr\""));
        assert!(json.contains("\"message\": \"1\""));
        assert!(json.contains("\"status\": \"warn\""));
        assert!(json.contains("\"color\": \"yellow\""));
        for key in [
            "unsuppressed_exposure_gaps",
            "unsuppressed_test_efficiency_findings",
            "intentional_test_efficiency_findings",
            "suppressed_exposure_gaps",
            "suppressed_test_efficiency_findings",
            "unknowns",
            "unknowns_test_efficiency",
            "analyzed_findings",
            "analyzed_tests",
        ] {
            assert!(
                json.contains(&format!("\"{key}\":")),
                "native JSON missing count key `{key}`"
            );
        }
        for key in [
            "include_unknowns",
            "fail_on_nonzero",
            "test_intent_path",
            "suppressions_path",
        ] {
            assert!(
                json.contains(&format!("\"{key}\":")),
                "native JSON missing policy key `{key}`"
            );
        }
    }

    #[test]
    fn badge_native_json_contains_all_nine_reason_defaults() {
        let output = check_output(vec![]);
        let summary = ripr_badge_summary(&output, BadgePolicy::default());
        let json = render_native_json(&summary);

        for reason in BADGE_REASON_KEYS {
            assert!(
                json.contains(&format!("\"{reason}\": 0")),
                "native JSON missing reason key `{reason}` with default 0"
            );
        }
        // Specifically sanity-check the new reason from #187/#188.
        assert!(json.contains("\"duplicate_activation_and_oracle_shape\": 0"));
    }

    #[test]
    fn badge_shields_projection_uses_camel_case_schema_version_key_and_exactly_four_fields() {
        let output = check_output(vec![finding(ExposureClass::WeaklyExposed, vec![])]);
        let summary = ripr_badge_summary(&output, BadgePolicy::default());
        let shields = render_shields_json(&summary);

        assert!(shields.contains("\"schemaVersion\": 1"));
        assert!(!shields.contains("\"schema_version\""));
        assert!(shields.contains("\"label\": \"ripr\""));
        assert!(shields.contains("\"message\": \"1\""));
        assert!(shields.contains("\"color\": \"yellow\""));

        // Exactly four top-level keys.
        let top_level_quoted_keys = shields
            .lines()
            .filter(|line| line.starts_with("  \""))
            .count();
        assert_eq!(
            top_level_quoted_keys, 4,
            "Shields projection must have exactly four top-level fields"
        );
        // No native-JSON-only fields leak in.
        for forbidden in ["counts", "reason_counts", "policy", "kind", "status"] {
            assert!(
                !shields.contains(&format!("\"{forbidden}\":")),
                "Shields projection must not include `{forbidden}`"
            );
        }
    }

    #[test]
    fn badge_summary_counts_unique_related_tests_by_file_name_line() {
        let test_a = related_test("test_one", "tests/a.rs", 10);
        let test_b = related_test("test_two", "tests/a.rs", 20);
        // Same identity — should dedupe across findings.
        let test_a_again = related_test("test_one", "tests/a.rs", 10);

        let output = check_output(vec![
            finding(ExposureClass::WeaklyExposed, vec![test_a, test_b]),
            finding(ExposureClass::WeaklyExposed, vec![test_a_again]),
        ]);

        let summary = ripr_badge_summary(&output, BadgePolicy::default());

        assert_eq!(
            summary.counts.analyzed_tests, 2,
            "analyzed_tests counts unique (file, name, line) identities"
        );
    }

    #[test]
    fn badge_include_unknowns_policy_adds_unknowns_to_headline() {
        let output = check_output(vec![
            finding(ExposureClass::WeaklyExposed, vec![]),
            finding(ExposureClass::InfectionUnknown, vec![]),
            finding(ExposureClass::StaticUnknown, vec![]),
        ]);

        let policy = BadgePolicy {
            include_unknowns: true,
            ..BadgePolicy::default()
        };
        let summary = ripr_badge_summary(&output, policy);

        // 1 exposure gap + 2 unknowns = 3.
        assert_eq!(summary.message, "3");
        // Counts still report them separately.
        assert_eq!(summary.counts.unsuppressed_exposure_gaps, 1);
        assert_eq!(summary.counts.unknowns, 2);
    }

    #[test]
    fn badge_test_efficiency_counts_are_zero_until_later_prs() {
        let output = check_output(vec![finding(ExposureClass::WeaklyExposed, vec![])]);
        let summary = ripr_badge_summary(&output, BadgePolicy::default());

        // This PR does not yet read the test-efficiency report. Future PRs
        // (`badge/ripr-plus-count-v1`, `test-intent/v1`, `suppressions/v1`)
        // will populate these.
        assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 0);
        assert_eq!(summary.counts.intentional_test_efficiency_findings, 0);
        assert_eq!(summary.counts.suppressed_test_efficiency_findings, 0);
        assert_eq!(summary.counts.suppressed_exposure_gaps, 0);
        assert_eq!(summary.counts.unknowns_test_efficiency, 0);
    }

    // -------- ripr+ test-efficiency parser --------

    fn te_json(tests_json: &str, reason_counts: &str) -> String {
        format!(
            r#"{{
  "schema_version": "0.1",
  "tests": [{tests_json}],
  "metrics": {{
    "tests_scanned": 42,
    "reason_counts": {{{reason_counts}}}
  }}
}}"#
        )
    }

    fn entry_json(class: &str, with_intent: bool) -> String {
        let intent = if with_intent {
            r#","declared_intent":{"intent":"smoke","owner":"x","reason":"y","source":".ripr/test_intent.toml"}"#
        } else {
            ""
        };
        format!(r#"{{"class":"{class}"{intent}}}"#)
    }

    #[test]
    fn badge_plus_parses_test_efficiency_metrics() -> Result<(), String> {
        let json = te_json(&entry_json("strong_discriminator", false), "");
        let summary = parse_test_efficiency_badge_summary(&json)?;

        assert_eq!(summary.analyzed_tests, 42);
        assert_eq!(summary.unsuppressed_test_efficiency_findings, 0);
        assert_eq!(summary.intentional_test_efficiency_findings, 0);
        assert_eq!(summary.unknowns_test_efficiency, 0);
        Ok(())
    }

    #[test]
    fn badge_plus_counts_actionable_classes() -> Result<(), String> {
        for class in [
            "likely_vacuous",
            "possibly_circular",
            "smoke_only",
            "duplicative",
        ] {
            let json = te_json(&entry_json(class, false), "");
            let summary = parse_test_efficiency_badge_summary(&json)?;
            assert_eq!(
                summary.unsuppressed_test_efficiency_findings, 1,
                "class `{class}` must count as actionable"
            );
            assert_eq!(summary.intentional_test_efficiency_findings, 0);
        }
        Ok(())
    }

    #[test]
    fn badge_plus_does_not_count_strong_discriminator_or_useful_but_broad() -> Result<(), String> {
        for class in ["strong_discriminator", "useful_but_broad"] {
            let json = te_json(&entry_json(class, false), "");
            let summary = parse_test_efficiency_badge_summary(&json)?;
            assert_eq!(
                summary.unsuppressed_test_efficiency_findings, 0,
                "class `{class}` must not count"
            );
            assert_eq!(summary.intentional_test_efficiency_findings, 0);
            assert_eq!(summary.unknowns_test_efficiency, 0);
        }
        Ok(())
    }

    #[test]
    fn badge_plus_reports_opaque_as_unknowns_test_efficiency() -> Result<(), String> {
        let json = te_json(&entry_json("opaque", false), "");
        let summary = parse_test_efficiency_badge_summary(&json)?;

        assert_eq!(summary.unsuppressed_test_efficiency_findings, 0);
        assert_eq!(summary.unknowns_test_efficiency, 1);
        Ok(())
    }

    #[test]
    fn badge_plus_declared_intent_excludes_actionable_finding() -> Result<(), String> {
        let json = te_json(&entry_json("smoke_only", true), "");
        let summary = parse_test_efficiency_badge_summary(&json)?;

        assert_eq!(
            summary.unsuppressed_test_efficiency_findings, 0,
            "declared intent must exclude the finding from unsuppressed"
        );
        assert_eq!(
            summary.intentional_test_efficiency_findings, 1,
            "declared intent must increment intentional count"
        );
        Ok(())
    }

    #[test]
    fn badge_plus_reason_counts_default_missing_keys_to_zero() -> Result<(), String> {
        let json = te_json(&entry_json("strong_discriminator", false), "");
        let summary = parse_test_efficiency_badge_summary(&json)?;

        for key in BADGE_REASON_KEYS {
            assert_eq!(
                summary.reason_counts.get(*key).copied(),
                Some(0),
                "reason `{key}` should default to 0"
            );
        }
        Ok(())
    }

    #[test]
    fn badge_plus_reason_counts_propagate_known_keys() -> Result<(), String> {
        let reasons =
            r#""smoke_oracle_only":4,"duplicate_activation_and_oracle_shape":2,"unrecognized":99"#;
        let json = te_json(&entry_json("strong_discriminator", false), reasons);
        let summary = parse_test_efficiency_badge_summary(&json)?;

        assert_eq!(
            summary.reason_counts.get("smoke_oracle_only").copied(),
            Some(4)
        );
        assert_eq!(
            summary
                .reason_counts
                .get("duplicate_activation_and_oracle_shape")
                .copied(),
            Some(2)
        );
        // Unknown reason names are silently dropped — they're not part of the
        // badge contract, only the nine allow-listed keys are.
        assert!(!summary.reason_counts.contains_key("unrecognized"));
        Ok(())
    }

    #[test]
    fn badge_plus_rejects_unknown_class_string() {
        let json = te_json(r#"{"class":"vibe_only"}"#, "");
        let result = parse_test_efficiency_badge_summary(&json);

        assert!(result.is_err(), "unknown class must fail parse");
        let err = result.err().unwrap_or_default();
        assert!(err.contains("vibe_only"));
    }

    #[test]
    fn badge_plus_rejects_unsupported_schema_version() {
        let json = r#"{"schema_version":"2.0","tests":[],"metrics":{"tests_scanned":0,"reason_counts":{}}}"#;
        let result = parse_test_efficiency_badge_summary(json);

        assert!(result.is_err());
        let err = result.err().unwrap_or_default();
        assert!(err.contains("schema_version"));
    }

    #[test]
    fn badge_plus_rejects_missing_metrics_tests_scanned() {
        let json = r#"{"schema_version":"0.1","tests":[],"metrics":{}}"#;
        let result = parse_test_efficiency_badge_summary(json);

        assert!(result.is_err());
        let err = result.err().unwrap_or_default();
        assert!(err.contains("metrics.tests_scanned"));
    }

    // -------- ripr+ summary builder + renderers --------

    #[test]
    fn ripr_plus_native_json_has_kind_ripr_plus_and_label_ripr_plus() {
        let summary = ripr_plus_badge_summary(
            &check_output(Vec::new()),
            TestEfficiencyBadgeSummary {
                unsuppressed_test_efficiency_findings: 0,
                intentional_test_efficiency_findings: 0,
                unknowns_test_efficiency: 0,
                analyzed_tests: 12,
                reason_counts: {
                    let mut m = std::collections::BTreeMap::new();
                    for k in BADGE_REASON_KEYS {
                        m.insert(*k, 0);
                    }
                    m
                },
                actionable_entries: Vec::new(),
            },
            BadgePolicy::default(),
        );
        let json = render_native_json(&summary);

        assert!(json.contains("\"kind\": \"ripr_plus\""));
        assert!(json.contains("\"label\": \"ripr+\""));
        assert!(json.contains("\"analyzed_tests\": 12"));
        assert!(json.contains("\"message\": \"0\""));
    }

    #[test]
    fn ripr_plus_message_sums_exposure_and_unsuppressed_test_efficiency() {
        // 1 weakly_exposed + 1 reachable_unrevealed = 2 exposure gaps.
        // 3 unsuppressed test-efficiency findings.
        // 2 declared intent (NOT in headline). Total: 2 + 3 = 5.
        let summary = ripr_plus_badge_summary(
            &check_output(vec![
                finding(ExposureClass::WeaklyExposed, vec![]),
                finding(ExposureClass::ReachableUnrevealed, vec![]),
                finding(ExposureClass::Exposed, vec![]),
            ]),
            TestEfficiencyBadgeSummary {
                unsuppressed_test_efficiency_findings: 3,
                intentional_test_efficiency_findings: 2,
                unknowns_test_efficiency: 1,
                analyzed_tests: 0,
                reason_counts: std::collections::BTreeMap::new(),
                actionable_entries: Vec::new(),
            },
            BadgePolicy::default(),
        );

        assert_eq!(summary.message, "5");
        assert_eq!(summary.counts.unsuppressed_exposure_gaps, 2);
        assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 3);
        assert_eq!(summary.counts.intentional_test_efficiency_findings, 2);
        assert_eq!(summary.counts.unknowns_test_efficiency, 1);
    }

    #[test]
    fn ripr_plus_shields_projection_has_exactly_four_fields_with_ripr_plus_label() {
        let summary = ripr_plus_badge_summary(
            &check_output(vec![finding(ExposureClass::WeaklyExposed, vec![])]),
            TestEfficiencyBadgeSummary::default(),
            BadgePolicy::default(),
        );
        let shields = render_shields_json(&summary);

        assert!(shields.contains("\"schemaVersion\": 1"));
        assert!(shields.contains("\"label\": \"ripr+\""));
        assert!(shields.contains("\"message\": \"1\""));
        assert!(shields.contains("\"color\":"));

        let top_level_quoted_keys = shields
            .lines()
            .filter(|line| line.starts_with("  \""))
            .count();
        assert_eq!(top_level_quoted_keys, 4);
        for forbidden in ["counts", "reason_counts", "policy", "kind", "status"] {
            assert!(
                !shields.contains(&format!("\"{forbidden}\":")),
                "ripr+ Shields projection must not contain `{forbidden}`"
            );
        }
    }

    #[test]
    fn ripr_plus_message_has_no_denominator_or_coverage_framing() {
        let summary = ripr_plus_badge_summary(
            &check_output(vec![
                finding(ExposureClass::WeaklyExposed, vec![]),
                finding(ExposureClass::Exposed, vec![]),
            ]),
            TestEfficiencyBadgeSummary {
                unsuppressed_test_efficiency_findings: 4,
                ..TestEfficiencyBadgeSummary::default()
            },
            BadgePolicy::default(),
        );
        let json = render_native_json(&summary);
        let shields = render_shields_json(&summary);

        for body in [&json, &shields] {
            let lower = body.to_ascii_lowercase();
            assert!(!lower.contains("coverage"));
            assert!(!lower.contains("uncovered"));
        }
        assert_eq!(summary.message, "5");
        assert!(!summary.message.contains('/'));
    }

    #[test]
    fn ripr_plus_include_unknowns_policy_adds_both_unknown_axes_to_headline() {
        let policy = BadgePolicy {
            include_unknowns: true,
            ..BadgePolicy::default()
        };
        let summary = ripr_plus_badge_summary(
            &check_output(vec![
                finding(ExposureClass::WeaklyExposed, vec![]), // 1 gap
                finding(ExposureClass::InfectionUnknown, vec![]), // 1 unknown
            ]),
            TestEfficiencyBadgeSummary {
                unsuppressed_test_efficiency_findings: 2,
                unknowns_test_efficiency: 3,
                ..TestEfficiencyBadgeSummary::default()
            },
            policy,
        );

        // 1 + 2 + 1 + 3 = 7
        assert_eq!(summary.message, "7");
    }

    // -------- suppressions wiring --------

    use super::{
        TestEfficiencyBadgeEntry, ripr_badge_summary_with_suppressions,
        ripr_plus_badge_summary_with_suppressions,
    };
    use crate::output::suppressions::{SuppressionEntry, SuppressionKind};

    fn finding_at_id(id: &str, class: ExposureClass) -> Finding {
        let mut f = finding(class, vec![]);
        f.id = id.to_string();
        f
    }

    fn exposure_suppression(finding_id: &str, expires: Option<&str>) -> SuppressionEntry {
        SuppressionEntry {
            kind: SuppressionKind::ExposureGap,
            finding_id: Some(finding_id.to_string()),
            test: None,
            path: None,
            reason: "x".to_string(),
            owner: "y".to_string(),
            expires: expires.map(str::to_string),
            block_line: 10,
        }
    }

    fn te_suppression(test: &str, path: Option<&str>, expires: Option<&str>) -> SuppressionEntry {
        SuppressionEntry {
            kind: SuppressionKind::TestEfficiency,
            finding_id: None,
            test: Some(test.to_string()),
            path: path.map(str::to_string),
            reason: "x".to_string(),
            owner: "y".to_string(),
            expires: expires.map(str::to_string),
            block_line: 20,
        }
    }

    #[test]
    fn ripr_badge_with_suppressions_moves_matched_findings_into_suppressed_bucket() {
        let output = check_output(vec![
            finding_at_id("probe:a", ExposureClass::WeaklyExposed),
            finding_at_id("probe:b", ExposureClass::ReachableUnrevealed),
            finding_at_id("probe:c", ExposureClass::NoStaticPath),
        ]);
        let suppressions = vec![exposure_suppression("probe:b", None)];

        let summary = ripr_badge_summary_with_suppressions(
            &output,
            &suppressions,
            "2026-05-03",
            BadgePolicy::default(),
        );

        assert_eq!(summary.counts.unsuppressed_exposure_gaps, 2);
        assert_eq!(summary.counts.suppressed_exposure_gaps, 1);
        assert_eq!(summary.message, "2");
        assert!(summary.warnings.is_empty());
    }

    #[test]
    fn ripr_badge_with_expired_suppression_keeps_finding_in_headline_and_warns() {
        let output = check_output(vec![finding_at_id("probe:a", ExposureClass::WeaklyExposed)]);
        let suppressions = vec![exposure_suppression("probe:a", Some("2025-01-01"))];

        let summary = ripr_badge_summary_with_suppressions(
            &output,
            &suppressions,
            "2026-05-03",
            BadgePolicy::default(),
        );

        // Expired suppression must NOT apply.
        assert_eq!(summary.counts.unsuppressed_exposure_gaps, 1);
        assert_eq!(summary.counts.suppressed_exposure_gaps, 0);
        // Warning surfaces so debt is visible.
        assert_eq!(summary.warnings.len(), 1);
        assert!(summary.warnings[0].contains("expired"));
        assert!(summary.warnings[0].contains("probe:a"));
    }

    #[test]
    fn ripr_plus_badge_with_test_efficiency_suppressions_moves_into_suppressed_bucket() {
        let te = TestEfficiencyBadgeSummary {
            unsuppressed_test_efficiency_findings: 2,
            actionable_entries: vec![
                TestEfficiencyBadgeEntry {
                    test: "alpha".to_string(),
                    path: "tests/a.rs".to_string(),
                    has_intent: false,
                },
                TestEfficiencyBadgeEntry {
                    test: "beta".to_string(),
                    path: "tests/b.rs".to_string(),
                    has_intent: false,
                },
            ],
            ..TestEfficiencyBadgeSummary::default()
        };
        let output = check_output(vec![]);
        let suppressions = vec![te_suppression("alpha", Some("tests/a.rs"), None)];

        let summary = ripr_plus_badge_summary_with_suppressions(
            &output,
            te,
            &suppressions,
            "2026-05-03",
            BadgePolicy::default(),
        );

        assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 1);
        assert_eq!(summary.counts.suppressed_test_efficiency_findings, 1);
        assert_eq!(summary.message, "1");
        assert!(summary.warnings.is_empty());
    }

    #[test]
    fn native_json_emits_warnings_array_always_even_when_empty() {
        let summary = ripr_badge_summary(&check_output(vec![]), BadgePolicy::default());
        let json = render_native_json(&summary);

        // Empty case still emits the field for stable shape.
        assert!(json.contains("\"warnings\": []"));
    }

    #[test]
    fn native_json_emits_warnings_when_suppressions_have_warnings() {
        let output = check_output(vec![finding_at_id("probe:a", ExposureClass::WeaklyExposed)]);
        let suppressions = vec![exposure_suppression("probe:a", Some("2025-01-01"))];
        let summary = ripr_badge_summary_with_suppressions(
            &output,
            &suppressions,
            "2026-05-03",
            BadgePolicy::default(),
        );
        let json = render_native_json(&summary);

        assert!(json.contains("\"warnings\": ["));
        assert!(json.contains("expired"));
        assert!(json.contains("probe:a"));
    }

    #[test]
    fn shields_projection_remains_four_fields_even_with_warnings_present() {
        let output = check_output(vec![finding_at_id("probe:a", ExposureClass::WeaklyExposed)]);
        let suppressions = vec![exposure_suppression("probe:does_not_match", None)];
        let summary = ripr_badge_summary_with_suppressions(
            &output,
            &suppressions,
            "2026-05-03",
            BadgePolicy::default(),
        );
        let shields = render_shields_json(&summary);

        // Warnings must NOT bleed into the Shields projection.
        assert!(!shields.contains("warnings"));
        assert!(!shields.contains("probe:does_not_match"));
        let top_level = shields.lines().filter(|l| l.starts_with("  \"")).count();
        assert_eq!(top_level, 4);
    }

    #[test]
    fn declared_intent_remains_distinct_from_suppression_in_counts() {
        // 1 unsuppressed actionable, 2 intentional, 0 unknowns_te.
        let te = TestEfficiencyBadgeSummary {
            unsuppressed_test_efficiency_findings: 1,
            intentional_test_efficiency_findings: 2,
            actionable_entries: vec![TestEfficiencyBadgeEntry {
                test: "alpha".to_string(),
                path: "tests/a.rs".to_string(),
                has_intent: false,
            }],
            ..TestEfficiencyBadgeSummary::default()
        };
        let output = check_output(vec![]);
        let suppressions = vec![te_suppression("alpha", Some("tests/a.rs"), None)];

        let summary = ripr_plus_badge_summary_with_suppressions(
            &output,
            te,
            &suppressions,
            "2026-05-03",
            BadgePolicy::default(),
        );

        // The actionable becomes suppressed, leaving 0 unsuppressed.
        assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 0);
        assert_eq!(summary.counts.suppressed_test_efficiency_findings, 1);
        // Intentional count is unaffected — intent and suppression are distinct.
        assert_eq!(summary.counts.intentional_test_efficiency_findings, 2);
    }
}
