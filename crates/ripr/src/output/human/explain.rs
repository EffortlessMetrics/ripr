//! The section `ripr explain` adds after the finding block (#5356).
//!
//! `check --format human-full` shows a window of the evidence. `explain` is
//! the drill-in, so it lists every retained test ripr examined with what it
//! concluded about each one, says what a test would need to change the
//! verdict, and spells out each stop reason.

use crate::domain::{
    ExposureClass, Finding, OracleStrength, RelatedTest, RelatedTestMiss, StopReason,
    exact_assertion_fact, input_boundary_fact,
};
use crate::output::path::display_path;
use crate::output::related_test_miss::{checked_assertion_text, related_test_miss_reason};

pub(crate) fn render_verdict_explanation(finding: &Finding) -> String {
    let mut out = String::from("\nWhy this verdict\n");
    let total = finding.related_tests_total();
    if finding.related_tests.is_empty() {
        out.push_str("  ripr found no test related to this change.\n");
    } else {
        // `related_tests` holds one row per matched assertion, so a test can
        // appear more than once; print each test once with every assertion
        // it was judged by.
        let groups = group_rows_by_test(&finding.related_tests);
        out.push_str(&format!(
            "  Tests examined: {} listed ({} assertion row(s) of {total})\n",
            groups.len(),
            finding.related_tests.len()
        ));
        for rows in groups {
            // The test is judged by its strongest assertion; a weaker row's
            // reason must not speak for a test that also asserts exactly.
            let lead = rows
                .iter()
                .copied()
                .min_by_key(|row| strength_order(&row.oracle_strength))
                .unwrap_or(rows[0]);
            let lead_verdict = row_verdict(lead, finding);
            out.push_str(&format!(
                "  - {}:{} {}: {lead_verdict}\n",
                display_path(&lead.file),
                lead.line,
                lead.name
            ));
            for row in &rows {
                if let Some(oracle) = &row.oracle {
                    let verdict = row_verdict(row, finding);
                    let note = if verdict == lead_verdict {
                        String::new()
                    } else {
                        format!(" ({verdict})")
                    };
                    out.push_str(&format!(
                        "      checked: {}{note}\n",
                        checked_assertion_text(oracle)
                    ));
                }
            }
        }
        if total > finding.related_tests.len() {
            out.push_str(&format!(
                "  ({} more row(s) examined; ripr keeps the {} most closely related)\n",
                total - finding.related_tests.len(),
                finding.related_tests.len()
            ));
        }
    }
    let needs = verdict_changers(finding);
    if !needs.is_empty() {
        out.push_str("  To change the verdict, a test needs to:\n");
        for need in needs {
            out.push_str(&format!("  - {need}\n"));
        }
    }
    if !finding.stop_reasons.is_empty() {
        out.push_str("  Stop reasons:\n");
        for reason in &finding.stop_reasons {
            out.push_str(&format!(
                "  - {}: {}\n",
                reason.as_str(),
                stop_reason_meaning(reason)
            ));
        }
    }
    out
}

/// What ripr concluded about one assertion row.
fn row_verdict(row: &RelatedTest, finding: &Finding) -> String {
    match related_test_miss_reason(row, &finding.activation.missing_discriminators) {
        Some(why) => format!(
            "{}: {why}",
            crate::output::related_test_miss::related_test_miss_label(row)
        ),
        None => match &row.oracle {
            Some(_) => format!(
                "{} {} oracle",
                row.oracle_strength.as_str(),
                row.oracle_kind.as_str().replace('_', " ")
            ),
            None => "no oracle row".to_string(),
        },
    }
}

/// Strongest first, so the minimum is the assertion a test is judged by.
fn strength_order(strength: &OracleStrength) -> u8 {
    match strength {
        OracleStrength::Strong => 0,
        OracleStrength::Medium => 1,
        OracleStrength::Weak => 2,
        OracleStrength::Smoke => 3,
        OracleStrength::Unknown => 4,
        OracleStrength::None => 5,
    }
}

/// Rows of the same test (name, file, line), in first-seen order.
fn group_rows_by_test(rows: &[RelatedTest]) -> Vec<Vec<&RelatedTest>> {
    let mut groups: Vec<Vec<&RelatedTest>> = Vec::new();
    for row in rows {
        match groups.iter_mut().find(|group| {
            group[0].name == row.name && group[0].file == row.file && group[0].line == row.line
        }) {
            Some(group) => group.push(row),
            None => groups.push(vec![row]),
        }
    }
    groups
}

/// What a test would have to do for ripr to see a discriminator, one line per
/// distinct miss. Only gap classes with a repair step get this list: for
/// `exposed` there is nothing to change, for the unknown classes ripr has not
/// established what is missing, and a gap whose next step was withheld (an
/// exact oracle already covers the direct sink) indicates no assertion repair.
fn verdict_changers(finding: &Finding) -> Vec<String> {
    if !matches!(
        finding.class,
        ExposureClass::NoStaticPath
            | ExposureClass::ReachableUnrevealed
            | ExposureClass::WeaklyExposed
    ) || finding.recommended_next_step.is_none()
    {
        return Vec::new();
    }
    let owner = finding
        .probe
        .owner
        .as_ref()
        .and_then(|owner| owner.0.rsplit("::").next().map(str::to_string))
        .filter(|name| !name.is_empty())
        .map_or_else(
            || "the changed code".to_string(),
            |name| format!("`{name}`"),
        );
    let mut needs: Vec<String> = Vec::new();
    let mut push = |need: String| {
        if !needs.contains(&need) {
            needs.push(need);
        }
    };
    if finding.related_tests.is_empty() {
        push(format!("call {owner} and assert on its result"));
    }
    for test in &finding.related_tests {
        let Some(miss) = test.miss else { continue };
        push(match miss {
            RelatedTestMiss::NoCallPath => format!("call {owner}, directly or through a helper"),
            RelatedTestMiss::NoAssertion | RelatedTestMiss::AssertionNotObserving => {
                format!("assert on what {owner} returns or changes")
            }
            RelatedTestMiss::AssertionNotCredited => {
                "use a plain, always-run `assert_eq!`/`assert!` ripr can resolve".to_string()
            }
            RelatedTestMiss::ObservationUnconfirmed => {
                "assert on the changed value itself, by name".to_string()
            }
            RelatedTestMiss::WeakAssertion => {
                "assert the exact value, not only success or presence".to_string()
            }
            // These needs are added once below, from the finding itself.
            RelatedTestMiss::MissingInput | RelatedTestMiss::MissingExactAssertion => continue,
        });
    }
    // A missing discriminator is a need whether or not any listed test got
    // far enough to be judged on it. A predicate boundary is an input a test
    // must use; an error variant or field value is an assertion it must make.
    let facts = &finding.activation.missing_discriminators;
    if let Some(fact) = input_boundary_fact(facts, &finding.probe.family) {
        push(format!("use an input that reaches `{}`", fact.value));
    }
    if let Some(fact) = exact_assertion_fact(facts, &finding.probe.family) {
        push(format!("assert the exact `{}`", fact.value));
    }
    needs
}

fn stop_reason_meaning(reason: &StopReason) -> &'static str {
    match reason {
        StopReason::MaxDepthReached => "the call walk hit its depth limit before reaching a test",
        StopReason::ExternalCrateBoundary => {
            "the path leaves this workspace; ripr does not follow it"
        }
        StopReason::DynamicDispatchUnresolved => {
            "the path goes through a trait object or generic call ripr cannot resolve"
        }
        StopReason::ProcMacroOpaque => "a procedural macro hides the code ripr would need to read",
        StopReason::FixtureOpaque => "a test fixture builds the input in a way ripr cannot read",
        StopReason::FeatureUnknown => "a cfg or feature gate may change whether the code runs",
        StopReason::AsyncBoundaryOpaque => {
            "the path crosses an async boundary ripr does not follow"
        }
        StopReason::NoChangedRustLine => "the diff changed no Rust line ripr can probe",
        StopReason::InfectionEvidenceUnknown => {
            "ripr could not tell whether a test input changes this value"
        }
        StopReason::PropagationEvidenceUnknown => {
            "ripr could not trace the changed value to anything a test observes"
        }
        StopReason::StaticProbeUnknown => "ripr has no probe shape for this kind of change",
        StopReason::TransitiveReachUnresolved => {
            "a test may reach this through a chain of internal calls ripr could not finish walking"
        }
        StopReason::MacroReachUnresolved => {
            "a test may reach this through a macro ripr does not expand"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{MissingDiscriminatorFact, OracleKind};
    use crate::output::perl_preview_card::tests::sample_perl_finding;
    use std::path::PathBuf;

    fn row(oracle: &str, strength: OracleStrength, miss: RelatedTestMiss) -> RelatedTest {
        RelatedTest {
            name: "parses_empty".to_string(),
            file: PathBuf::from("tests/parse.rs"),
            line: 7,
            oracle: Some(oracle.to_string()),
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: strength,
            relation_reason: None,
            relation_confidence: None,
            miss: Some(miss),
        }
    }

    #[test]
    fn a_test_is_judged_by_its_strongest_assertion_not_its_first_row() {
        let mut finding = sample_perl_finding();
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "len == 0".to_string(),
            reason: "no related test passes an empty input".to_string(),
            flow_sink: None,
        }];
        finding.related_tests = vec![
            row(
                "assert!(parse(\"a\").is_ok());",
                OracleStrength::Weak,
                RelatedTestMiss::WeakAssertion,
            ),
            row(
                "assert_eq!(parse(\"a\"), Ok(1));",
                OracleStrength::Strong,
                RelatedTestMiss::MissingInput,
            ),
        ];
        let text = render_verdict_explanation(&finding);
        assert!(
            text.contains(
                "  - tests/parse.rs:7 parses_empty: misses: no test input reaches `len == 0`\n"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "      checked: assert!(parse(\"a\").is_ok()) (misses: assertion too weak to tell the old behavior from the new)\n"
            ),
            "{text}"
        );
        assert!(
            text.contains("      checked: assert_eq!(parse(\"a\"), Ok(1))\n"),
            "{text}"
        );
    }

    /// #5508: a Perl row whose observation ripr could not confirm is an
    /// unknown, so the explanation does not introduce it as a miss.
    #[test]
    fn an_unconfirmed_observation_is_not_labelled_a_miss() {
        let mut finding = sample_perl_finding();
        finding.related_tests = vec![row(
            "is(My::App::discount(100), 10, 'discount threshold')",
            OracleStrength::Strong,
            RelatedTestMiss::ObservationUnconfirmed,
        )];
        let text = render_verdict_explanation(&finding);
        assert!(
            text.contains(
                "  - tests/parse.rs:7 parses_empty: unconfirmed: ripr could not confirm that this assertion observes the changed behavior\n"
            ),
            "{text}"
        );
        assert!(!text.contains("misses: ripr could not confirm"), "{text}");
    }
}
