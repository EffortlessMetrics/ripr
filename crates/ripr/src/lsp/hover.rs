use super::HOVER_TEXT;
use crate::analysis::ClassifiedSeam;
use crate::domain::{Finding, StageEvidence};
use tower_lsp_server::ls_types::{
    Diagnostic, Hover, HoverContents, MarkupContent, MarkupKind, NumberOrString, Position, Range,
};

pub(super) fn hover_response() -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: HOVER_TEXT.to_string(),
        }),
        range: None,
    }
}

pub(super) fn diagnostic_hover_response(diagnostic: &Diagnostic) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: diagnostic_hover_markdown(diagnostic),
        }),
        range: Some(diagnostic.range),
    }
}

pub(super) fn finding_hover_response(finding: &Finding, diagnostic: &Diagnostic) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: finding_hover_markdown(diagnostic, finding),
        }),
        range: Some(diagnostic.range),
    }
}

/// Voice B hover for seam diagnostics. Renders the RIPR evidence
/// path that produced the seam's grip class plus related-test
/// citations and the next-step suggestion. Looks up the seam by
/// `diagnostic.data.seam_id` rather than parsing the diagnostic
/// message — the lookup contract from
/// `state::classified_seam_for_diagnostic`.
pub(super) fn classified_seam_hover_response(
    seam: &ClassifiedSeam,
    diagnostic: &Diagnostic,
) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: classified_seam_hover_markdown(diagnostic, seam),
        }),
        range: Some(diagnostic.range),
    }
}

pub(super) fn diagnostic_at_position<'a>(
    diagnostics: &'a [Diagnostic],
    position: &Position,
) -> Option<&'a Diagnostic> {
    diagnostics
        .iter()
        .find(|diagnostic| position_in_range(position, &diagnostic.range))
}

/// True if `diagnostic`'s range covers `position`. Useful for callers
/// that need to scan all overlapping diagnostics (e.g., backend hover
/// preferring seam-bearing diagnostics over finding-bearing ones).
pub(super) fn diagnostic_covers_position(diagnostic: &Diagnostic, position: &Position) -> bool {
    position_in_range(position, &diagnostic.range)
}

fn diagnostic_hover_markdown(diagnostic: &Diagnostic) -> String {
    let classification = diagnostic
        .code
        .as_ref()
        .map(number_or_string_label)
        .unwrap_or_else(|| "static exposure".to_string());
    let mut lines = vec![
        format!("**ripr** `{classification}`"),
        String::new(),
        diagnostic.message.clone(),
    ];
    if let Some(data) = &diagnostic.data {
        if let Some(finding_id) = data.get("finding_id").and_then(|value| value.as_str()) {
            lines.push(String::new());
            lines.push(format!("Finding: `{finding_id}`"));
        }
        if let Some(probe_id) = data.get("probe_id").and_then(|value| value.as_str()) {
            lines.push(format!("Probe: `{probe_id}`"));
        }
    }
    lines.join("\n")
}

fn finding_hover_markdown(diagnostic: &Diagnostic, finding: &Finding) -> String {
    let classification = diagnostic
        .code
        .as_ref()
        .map(number_or_string_label)
        .unwrap_or_else(|| "static exposure".to_string());
    let mut lines = vec![
        format!("**ripr** `{classification}`"),
        String::new(),
        diagnostic.message.clone(),
        String::new(),
        "## RIPR Evidence".to_string(),
        stage_line("reach", &finding.ripr.reach),
        stage_line("infection", &finding.ripr.infect),
        stage_line("propagation", &finding.ripr.propagate),
        stage_line("observation", &finding.ripr.reveal.observe),
        stage_line("discriminator", &finding.ripr.reveal.discriminate),
    ];

    if !finding.related_tests.is_empty() {
        lines.push(String::new());
        lines.push("## Related Tests".to_string());
        for test in &finding.related_tests {
            let oracle_text = match &test.oracle {
                Some(oracle) => format!(
                    " \u{2014} {} {} oracle: {}",
                    test.oracle_strength.as_str(),
                    test.oracle_kind.as_str(),
                    oracle
                ),
                None => String::new(),
            };
            lines.push(format!(
                "- `{}:{}` `{}`{}",
                test.file.display(),
                test.line,
                test.name,
                oracle_text
            ));
        }
    }

    if !finding.missing.is_empty() {
        lines.push(String::new());
        lines.push("## Weakness".to_string());
        for item in &finding.missing {
            lines.push(format!("- {item}"));
        }
    }

    lines.join("\n")
}

fn stage_line(name: &str, stage: &StageEvidence) -> String {
    format!("* {name} {}: {}", stage.state.as_str(), stage.summary)
}

fn number_or_string_label(value: &NumberOrString) -> String {
    match value {
        NumberOrString::Number(number) => number.to_string(),
        NumberOrString::String(text) => text.clone(),
    }
}

fn position_in_range(position: &Position, range: &Range) -> bool {
    position_is_after_or_equal(position, &range.start) && position_is_before(position, &range.end)
}

fn position_is_after_or_equal(position: &Position, start: &Position) -> bool {
    position.line > start.line
        || (position.line == start.line && position.character >= start.character)
}

fn position_is_before(position: &Position, end: &Position) -> bool {
    position.line < end.line || (position.line == end.line && position.character < end.character)
}

fn classified_seam_hover_markdown(diagnostic: &Diagnostic, entry: &ClassifiedSeam) -> String {
    let seam = &entry.seam;
    let evidence = &entry.evidence;
    let mut lines = vec![
        format!("**ripr** behavioral seam"),
        String::new(),
        format!("`{}`", seam.expression()),
        String::new(),
        format!("## Grip"),
        format!("`{}`", entry.class.as_str()),
        String::new(),
        "## Evidence".to_string(),
        seam_stage_line("reach", &evidence.reach),
        seam_stage_line("activation", &evidence.activate),
        seam_stage_line("propagation", &evidence.propagate),
        seam_stage_line("observation", &evidence.observe),
        seam_stage_line("discrimination", &evidence.discriminate),
    ];

    if !evidence.observed_values.is_empty() {
        lines.push(String::new());
        lines.push("## Observed values".to_string());
        for value in evidence.observed_values.iter().take(5) {
            lines.push(format!("- `{}`", value.value));
        }
    }

    if !evidence.missing_discriminators.is_empty() {
        lines.push(String::new());
        lines.push("## Missing discriminator".to_string());
        for missing in &evidence.missing_discriminators {
            lines.push(format!("- `{}` — {}", missing.value, missing.reason));
        }
    }

    if !evidence.related_tests.is_empty() {
        lines.push(String::new());
        lines.push("## Related tests".to_string());
        for grip in evidence.related_tests.iter().take(5) {
            lines.push(format!(
                "- `{}` — {} / {}",
                grip.test_name,
                grip.oracle_kind.as_str(),
                grip.oracle_strength.as_str()
            ));
        }
    }

    lines.push(String::new());
    lines.push("## Next step".to_string());
    lines.push(seam_next_step_for(entry));

    let _ = diagnostic;
    lines.join("\n")
}

fn seam_stage_line(name: &str, stage: &StageEvidence) -> String {
    format!("* {name} {}: {}", stage.state.as_str(), stage.summary)
}

/// Best-effort plain-language next-step prompt derived from the seam
/// kind and class. Mirrors the shape of `agent_seam_packets`'
/// `missing_oracle_shape` so hover and packets stay in sync.
fn seam_next_step_for(entry: &ClassifiedSeam) -> String {
    use crate::analysis::seams::{SeamGripClass, SeamKind};
    if matches!(entry.class, SeamGripClass::Opaque) {
        return "Inspect the static limitation: helper, macro, or fixture that hides evidence."
            .to_string();
    }
    match entry.seam.kind() {
        SeamKind::PredicateBoundary => {
            "Add an exact-value assertion for the equality boundary.".to_string()
        }
        SeamKind::ErrorVariant => {
            "Assert the exact error variant via `matches!` or `assert_matches!`.".to_string()
        }
        SeamKind::ReturnValue => "Add an exact-value assertion on the returned value.".to_string(),
        SeamKind::FieldConstruction => {
            "Assert on the specific field or use whole-object equality.".to_string()
        }
        SeamKind::SideEffect => {
            "Add a mock expectation, event observer, or persistence assertion.".to_string()
        }
        SeamKind::MatchArm => {
            "Drive an input that selects this arm and assert the result.".to_string()
        }
        SeamKind::CallPresence => {
            "Add a mock or spy assertion that the expected call happens.".to_string()
        }
    }
}

#[cfg(test)]
mod seam_hover_tests {
    use super::*;
    use crate::analysis::seams::{
        ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind,
    };
    use crate::analysis::test_grip_evidence::{RelatedTestGrip, TestGripEvidence};
    use crate::domain::{
        Confidence, MissingDiscriminatorFact, OracleKind, OracleStrength, StageEvidence,
        StageState, ValueContext, ValueFact,
    };
    use std::path::PathBuf;
    use tower_lsp_server::ls_types::{NumberOrString, Position, Range};

    fn stage(state: StageState, summary: &str) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, summary)
    }

    fn weakly_gripped_classified() -> ClassifiedSeam {
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
            related_tests: vec![RelatedTestGrip {
                test_name: "below_threshold_has_no_discount".to_string(),
                file: PathBuf::from("tests/pricing.rs"),
                line: 12,
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
                evidence_summary: "exact value assertion".to_string(),
            }],
            reach: stage(StageState::Yes, "Related tests reach discounted_total"),
            activate: stage(StageState::Yes, "Observed amount = 50, amount = 10000"),
            propagate: stage(StageState::Yes, "Seam flows to return_value"),
            observe: stage(StageState::Yes, "Exact value assertion exists"),
            discriminate: stage(StageState::Weak, "Equality boundary missing"),
            observed_values: vec![ValueFact {
                line: 12,
                text: "discounted_total(50, 100)".to_string(),
                value: "50".to_string(),
                context: ValueContext::FunctionArgument,
            }],
            missing_discriminators: vec![MissingDiscriminatorFact {
                value: "discount_threshold (equality boundary)".to_string(),
                reason: "observed values do not include the equality-boundary case".to_string(),
                flow_sink: None,
            }],
        };
        ClassifiedSeam {
            seam,
            evidence,
            class: SeamGripClass::WeaklyGripped,
        }
    }

    fn sample_diagnostic() -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position {
                    line: 87,
                    character: 0,
                },
                end: Position {
                    line: 87,
                    character: 120,
                },
            },
            severity: None,
            code: Some(NumberOrString::String(
                "ripr-seam-weakly-gripped".to_string(),
            )),
            code_description: None,
            source: Some("ripr".to_string()),
            message: "Weakly gripped behavioral seam".to_string(),
            related_information: None,
            tags: None,
            data: Some(serde_json::json!({"seam_id": "f3c9e4d21a0b7c88"})),
        }
    }

    fn extract_markup(hover: &Hover) -> Result<&str, String> {
        match &hover.contents {
            HoverContents::Markup(content) => Ok(content.value.as_str()),
            other => Err(format!("expected MarkupContent, got {other:?}")),
        }
    }

    #[test]
    fn given_seam_diagnostic_when_hover_is_requested_then_hover_renders_grip_evidence_path()
    -> Result<(), String> {
        let seam = weakly_gripped_classified();
        let diagnostic = sample_diagnostic();
        let hover = classified_seam_hover_response(&seam, &diagnostic);
        let md = extract_markup(&hover)?;
        for needle in [
            "behavioral seam",
            "amount >= discount_threshold",
            "## Grip",
            "weakly_gripped",
            "## Evidence",
            "reach yes:",
            "activation yes:",
            "propagation yes:",
            "observation yes:",
            "discrimination weak:",
        ] {
            if !md.contains(needle) {
                return Err(format!("missing {needle:?} in:\n{md}"));
            }
        }
        Ok(())
    }

    #[test]
    fn given_weakly_gripped_boundary_when_hover_is_rendered_then_missing_boundary_discriminator_is_shown()
    -> Result<(), String> {
        let seam = weakly_gripped_classified();
        let diagnostic = sample_diagnostic();
        let hover = classified_seam_hover_response(&seam, &diagnostic);
        let md = extract_markup(&hover)?;
        if !md.contains("## Missing discriminator") {
            return Err(format!("missing section header in:\n{md}"));
        }
        if !md.contains("discount_threshold (equality boundary)") {
            return Err(format!("missing boundary value in:\n{md}"));
        }
        if !md.contains("## Next step") {
            return Err(format!("missing next-step in:\n{md}"));
        }
        if !md.contains("equality boundary") {
            return Err(format!(
                "next-step should mention the equality boundary in:\n{md}"
            ));
        }
        Ok(())
    }

    #[test]
    fn given_seam_with_related_tests_when_hover_is_rendered_then_oracle_kind_and_strength_appear()
    -> Result<(), String> {
        let seam = weakly_gripped_classified();
        let diagnostic = sample_diagnostic();
        let hover = classified_seam_hover_response(&seam, &diagnostic);
        let md = extract_markup(&hover)?;
        if !md.contains("## Related tests") {
            return Err(format!("missing related-tests section in:\n{md}"));
        }
        if !md.contains("below_threshold_has_no_discount") {
            return Err(format!("missing related test name in:\n{md}"));
        }
        if !md.contains("exact_value / strong") {
            return Err(format!("missing oracle kind/strength in:\n{md}"));
        }
        Ok(())
    }

    #[test]
    fn opaque_seam_hover_advises_inspecting_static_limitation() -> Result<(), String> {
        let mut seam = weakly_gripped_classified();
        seam.class = SeamGripClass::Opaque;
        let diagnostic = sample_diagnostic();
        let hover = classified_seam_hover_response(&seam, &diagnostic);
        let md = extract_markup(&hover)?;
        if !md.contains("Inspect the static limitation") {
            return Err(format!("expected opaque next-step text in:\n{md}"));
        }
        Ok(())
    }
}
