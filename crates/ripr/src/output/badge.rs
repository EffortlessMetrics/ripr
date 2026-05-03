//! Private badge summary model and renderer.
//!
//! This module is the rendering substrate for the `ripr` and (future)
//! `ripr+` badges. Its types are intentionally crate-private — the public
//! contract is the JSON wire shape, not the Rust types. See
//! [`docs/BADGE_POLICY.md`](../../../../../docs/BADGE_POLICY.md) for the
//! locked semantics, color thresholds, and JSON shape.
//!
//! The current implementation supports only the `ripr` badge (exposure-gap
//! counts). Test-efficiency, intent, suppressions, and the `ripr+` shape
//! are intentionally absent and will arrive in their own scoped PRs.

use crate::app::CheckOutput;
use crate::domain::ExposureClass;
use crate::output::json::escape as json_escape;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeKind {
    /// Counts unsuppressed static exposure gaps only.
    Ripr,
}

impl BadgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BadgeKind::Ripr => "ripr",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BadgeKind::Ripr => "ripr",
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

/// Builds the `ripr` badge summary from a `CheckOutput`. Counts only
/// exposure-gap classes; reports unknowns separately. Test-efficiency,
/// intent, and suppression fields stay zero in this PR.
pub fn ripr_badge_summary(output: &CheckOutput, policy: BadgePolicy) -> BadgeSummary {
    let mut unsuppressed_exposure_gaps = 0usize;
    let mut unknowns = 0usize;
    let mut unique_tests: BTreeSet<(String, String, usize)> = BTreeSet::new();

    for finding in &output.findings {
        match finding.class {
            ExposureClass::WeaklyExposed
            | ExposureClass::ReachableUnrevealed
            | ExposureClass::NoStaticPath => {
                unsuppressed_exposure_gaps += 1;
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

    let counts = BadgeCounts {
        unsuppressed_exposure_gaps,
        unsuppressed_test_efficiency_findings: 0,
        intentional_test_efficiency_findings: 0,
        suppressed_exposure_gaps: 0,
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
    }
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
    out.push_str("  }\n}\n");
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
        BADGE_REASON_KEYS, BadgePolicy, BadgeStatus, badge_status_color, render_native_json,
        render_shields_json, ripr_badge_summary,
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
}
