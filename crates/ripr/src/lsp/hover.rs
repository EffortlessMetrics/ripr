use super::HOVER_TEXT;
use super::state::{AnalysisSnapshot, format_duration};
use crate::agent::loop_commands;
use crate::analysis::ClassifiedSeam;
use crate::domain::{Finding, StageEvidence, StageState};
use crate::output::agent_seam_packets::{
    suggested_assertion_for_classified_seam, targeted_test_brief_outline_for_classified_seam,
};
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

/// Seam evidence hover for seam diagnostics. Renders the RIPR
/// evidence path that produced the seam's grip class plus related-test
/// citations and the next-step suggestion. Looks up the seam by
/// `diagnostic.data.seam_id` rather than parsing the diagnostic
/// message — the lookup contract from
/// `state::classified_seam_for_diagnostic`.
pub(super) fn classified_seam_hover_response(
    seam: &ClassifiedSeam,
    diagnostic: &Diagnostic,
    snapshot: Option<&AnalysisSnapshot>,
) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: classified_seam_hover_markdown(seam, snapshot),
        }),
        range: Some(diagnostic.range),
    }
}

pub(super) fn hover_with_snapshot_status(mut hover: Hover, snapshot: &AnalysisSnapshot) -> Hover {
    let HoverContents::Markup(content) = &mut hover.contents else {
        return hover;
    };
    content.value.push_str("\n\n---\n");
    content.value.push_str("Analysis snapshot: generated ");
    let age_duration = snapshot.refresh.age();
    let age = age_duration
        .map(format_duration)
        .unwrap_or_else(|| "at an unknown time".to_string());
    content.value.push_str(&age);
    if age_duration.is_some() {
        content.value.push_str(" ago");
    }
    if let Some(duration) = snapshot.refresh.duration {
        content.value.push_str("; last refresh took ");
        content.value.push_str(&format_duration(duration));
    }
    content.value.push('.');
    hover
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

fn classified_seam_hover_markdown(
    entry: &ClassifiedSeam,
    snapshot: Option<&AnalysisSnapshot>,
) -> String {
    let seam = &entry.seam;
    let evidence = &entry.evidence;
    let next_step = seam_next_step_for(entry);
    let mut lines = vec![
        format!("**ripr** behavioral seam"),
        String::new(),
        format!("`{}`", seam.expression()),
        String::new(),
        format!("## Grip"),
        format!("`{}`", entry.class.as_str()),
        String::new(),
        "## Why this diagnostic?".to_string(),
        format!(
            "Grip class: `{}` — {}",
            entry.class.as_str(),
            seam_class_reason(entry)
        ),
    ];
    push_classification_explanation(&mut lines, entry, &next_step);
    lines.extend([
        String::new(),
        "## Evidence".to_string(),
        seam_stage_line("reach", &evidence.reach),
        seam_stage_line("activation", &evidence.activate),
        seam_stage_line("propagation", &evidence.propagate),
        seam_stage_line("observation", &evidence.observe),
        seam_stage_line("discrimination", &evidence.discriminate),
    ]);

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
            // Terse trailing tag — `oracle_kind/oracle_strength · reason/confidence`.
            // Density chosen for hover; full per-field detail belongs
            // in repo exposure JSON or the agent packet.
            lines.push(format!(
                "- `{}:{}` `{}` — {} / {} · {} / {}",
                display_hover_path(&grip.file),
                grip.line,
                grip.test_name,
                grip.oracle_kind.as_str(),
                grip.oracle_strength.as_str(),
                grip.relation_reason.as_str(),
                grip.relation_confidence.as_str()
            ));
        }
    }

    push_test_shape(&mut lines, entry);
    push_editor_commands(&mut lines, entry, snapshot);
    push_static_limits(&mut lines);

    lines.push(String::new());
    lines.push("## Next step".to_string());
    lines.push(next_step);

    lines.join("\n")
}

fn push_test_shape(lines: &mut Vec<String>, entry: &ClassifiedSeam) {
    let outline = targeted_test_brief_outline_for_classified_seam(entry);
    lines.push(String::new());
    lines.push("## Suggested test shape".to_string());
    lines.push(format!("- file: `{}`", outline.suggested_file));
    lines.push(format!("- name: `{}`", outline.suggested_name));
    if let Some(candidate) = outline.candidate_value {
        lines.push(format!("- candidate value: `{candidate}`"));
    }
    lines.push(format!("- assertion shape: {}", outline.assertion_shape));
    if let Some(assertion) = suggested_assertion_for_classified_seam(entry) {
        lines.push(format!("- assertion template: `{assertion}`"));
    }
}

fn push_editor_commands(
    lines: &mut Vec<String>,
    entry: &ClassifiedSeam,
    snapshot: Option<&AnalysisSnapshot>,
) {
    let mode = snapshot.map_or("draft", |snapshot| snapshot.mode.as_str());
    let seam_id = entry.seam.id().as_str();
    lines.push(String::new());
    lines.push("## Handoff, verify, and receipt commands".to_string());
    lines.push(format!(
        "- packet: `{}`",
        loop_commands::agent_packet_command(
            ".",
            seam_id,
            loop_commands::EDITOR_AGENT_PACKET_ARTIFACT,
        )
    ));
    lines.push(format!(
        "- brief: `{}`",
        loop_commands::agent_brief_command(
            ".",
            seam_id,
            loop_commands::EDITOR_AGENT_BRIEF_ARTIFACT,
        )
    ));
    lines.push(format!(
        "- after snapshot: `{}`",
        loop_commands::check_repo_exposure_command(
            ".",
            mode,
            loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
        )
    ));
    lines.push(format!(
        "- verify: `{}`",
        loop_commands::agent_verify_command(
            ".",
            loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
            loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
            Some(loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT),
        )
    ));
    lines.push(format!(
        "- receipt: `{}`",
        loop_commands::agent_receipt_command(
            ".",
            loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT,
            seam_id,
            Some(loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT),
        )
    ));
}

fn push_static_limits(lines: &mut Vec<String>) {
    lines.push(String::new());
    lines.push("## Limits".to_string());
    lines.push(
        "- Static evidence only; this hover does not run mutation testing or prove runtime adequacy."
            .to_string(),
    );
    lines.push(
        "- Suggested assertions are work-order guidance, not generated tests or source edits."
            .to_string(),
    );
}

fn display_hover_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn seam_stage_line(name: &str, stage: &StageEvidence) -> String {
    format!("* {name} {}: {}", stage.state.as_str(), stage.summary)
}

fn push_classification_explanation(
    lines: &mut Vec<String>,
    entry: &ClassifiedSeam,
    next_step: &str,
) {
    let evidence = &entry.evidence;
    let stages = [
        ("reach", &evidence.reach),
        ("activation", &evidence.activate),
        ("propagation", &evidence.propagate),
        ("observation", &evidence.observe),
        ("discrimination", &evidence.discriminate),
    ];

    lines.push(String::new());
    lines.push("Strong evidence:".to_string());
    let mut has_strong_evidence = false;
    for (name, stage) in stages {
        if stage.state == StageState::Yes {
            has_strong_evidence = true;
            lines.push(format!("- {name} yes: {}", stage.summary));
        }
    }
    if !has_strong_evidence {
        lines.push("- no yes-stage evidence recorded in the current snapshot".to_string());
    }

    lines.push(String::new());
    lines.push("Weak / missing evidence:".to_string());
    let mut has_gap_evidence = false;
    for (name, stage) in stages {
        if stage.state != StageState::Yes {
            has_gap_evidence = true;
            lines.push(format!(
                "- {name} {}: {}",
                stage.state.as_str(),
                stage.summary
            ));
        }
    }
    for missing in &evidence.missing_discriminators {
        has_gap_evidence = true;
        lines.push(format!(
            "- missing discriminator `{}`: {}",
            missing.value, missing.reason
        ));
    }
    if !has_gap_evidence {
        lines.push("- no weak or missing stage evidence recorded".to_string());
    }

    lines.push(String::new());
    lines.push(format!("Recommended next move: {next_step}"));
}

fn seam_class_reason(entry: &ClassifiedSeam) -> &'static str {
    use crate::analysis::seams::SeamGripClass;
    match entry.class {
        SeamGripClass::StronglyGripped => {
            "all RIPR stages are yes and no missing discriminator is recorded."
        }
        SeamGripClass::WeaklyGripped => {
            "the current static evidence has a weak discriminator or a named missing discriminator."
        }
        SeamGripClass::Ungripped => "reach evidence is missing for this seam.",
        SeamGripClass::ReachableUnrevealed => {
            "reach evidence exists, but discriminator evidence is absent."
        }
        SeamGripClass::ActivationUnknown => "activation evidence is unknown.",
        SeamGripClass::PropagationUnknown => "propagation evidence is unknown.",
        SeamGripClass::ObservationUnknown => "observation evidence is unknown.",
        SeamGripClass::DiscriminationUnknown => "discriminator evidence is unknown.",
        SeamGripClass::Opaque => "static evidence hit an opacity limit.",
        SeamGripClass::Intentional => "declared test intent marks this seam as intentional.",
        SeamGripClass::Suppressed => "a suppression marks this seam as intentionally hidden.",
    }
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
    use crate::app::Mode;
    use crate::domain::{
        Confidence, MissingDiscriminatorFact, OracleKind, OracleStrength, StageEvidence,
        StageState, ValueContext, ValueFact,
    };
    use std::collections::BTreeMap;
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
                relation_reason:
                    crate::analysis::test_grip_evidence::RelationReason::DirectOwnerCall,
                relation_confidence: crate::analysis::test_grip_evidence::RelationConfidence::High,
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

    fn sample_snapshot(mode: Mode) -> AnalysisSnapshot {
        AnalysisSnapshot {
            root: PathBuf::from("/workspace"),
            base: None,
            mode,
            refresh: super::super::state::RefreshMetadata::default(),
            findings: Vec::new(),
            classified_seams: Vec::new(),
            diagnostics_by_uri: BTreeMap::new(),
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
        let hover = classified_seam_hover_response(&seam, &diagnostic, None);
        let md = extract_markup(&hover)?;
        for needle in [
            "behavioral seam",
            "amount >= discount_threshold",
            "## Grip",
            "weakly_gripped",
            "## Why this diagnostic?",
            "Grip class: `weakly_gripped`",
            "Strong evidence:",
            "Weak / missing evidence:",
            "Recommended next move:",
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
        let hover = classified_seam_hover_response(&seam, &diagnostic, None);
        let md = extract_markup(&hover)?;
        if !md.contains("## Missing discriminator") {
            return Err(format!("missing section header in:\n{md}"));
        }
        if !md.contains("discount_threshold (equality boundary)") {
            return Err(format!("missing boundary value in:\n{md}"));
        }
        if !md.contains("missing discriminator `discount_threshold (equality boundary)`") {
            return Err(format!("missing classification explanation in:\n{md}"));
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
        let hover = classified_seam_hover_response(&seam, &diagnostic, None);
        let md = extract_markup(&hover)?;
        if !md.contains("## Related tests") {
            return Err(format!("missing related-tests section in:\n{md}"));
        }
        if !md.contains("tests/pricing.rs:12") {
            return Err(format!("missing related test location in:\n{md}"));
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
    fn given_seam_hover_when_rendered_then_suggested_test_shape_is_visible() -> Result<(), String> {
        let seam = weakly_gripped_classified();
        let diagnostic = sample_diagnostic();
        let hover = classified_seam_hover_response(&seam, &diagnostic, None);
        let md = extract_markup(&hover)?;
        for needle in [
            "## Suggested test shape",
            "- file: `tests/pricing.rs`",
            "- name: `discounted_total_boundary_discriminator`",
            "- candidate value: `discount_threshold (equality boundary)`",
            "- assertion shape: assert_eq!(discounted_total",
            "- assertion template: `assert_eq!(discounted_total",
        ] {
            if !md.contains(needle) {
                return Err(format!("missing {needle:?} in:\n{md}"));
            }
        }
        Ok(())
    }

    #[test]
    fn given_seam_hover_with_snapshot_when_rendered_then_verify_and_receipt_commands_match_mode()
    -> Result<(), String> {
        let seam = weakly_gripped_classified();
        let diagnostic = sample_diagnostic();
        let snapshot = sample_snapshot(Mode::Ready);
        let hover = classified_seam_hover_response(&seam, &diagnostic, Some(&snapshot));
        let md = extract_markup(&hover)?;
        for needle in [
            "## Handoff, verify, and receipt commands",
            "- packet: `ripr agent packet --root . --seam-id",
            "--json > target/ripr/agent/agent-packet.json",
            "- brief: `ripr agent brief --root . --seam-id",
            "--json > target/ripr/agent/agent-brief.json",
            "- after snapshot: `ripr check --root . --mode ready --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json`",
            "- verify: `ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json > target/ripr/agent/agent-verify.json`",
            "ripr agent receipt --root . --verify-json target/ripr/agent/agent-verify.json --seam-id",
            "--json --out target/ripr/agent/agent-receipt.json",
        ] {
            if !md.contains(needle) {
                return Err(format!("missing {needle:?} in:\n{md}"));
            }
        }
        Ok(())
    }

    #[test]
    fn given_seam_hover_when_rendered_then_static_limits_are_explicit() -> Result<(), String> {
        let seam = weakly_gripped_classified();
        let diagnostic = sample_diagnostic();
        let hover = classified_seam_hover_response(&seam, &diagnostic, None);
        let md = extract_markup(&hover)?;
        for needle in [
            "## Limits",
            "Static evidence only; this hover does not run mutation testing or prove runtime adequacy.",
            "Suggested assertions are work-order guidance, not generated tests or source edits.",
        ] {
            if !md.contains(needle) {
                return Err(format!("missing {needle:?} in:\n{md}"));
            }
        }
        Ok(())
    }

    #[test]
    fn given_lsp_hover_with_related_tests_when_rendered_then_relation_reason_is_visible()
    -> Result<(), String> {
        // Pin the terse trailing tag format chosen for hover:
        //   `test_name — oracle_kind/oracle_strength · reason/confidence`.
        let seam = weakly_gripped_classified();
        let diagnostic = sample_diagnostic();
        let hover = classified_seam_hover_response(&seam, &diagnostic, None);
        let md = extract_markup(&hover)?;
        if !md.contains("· direct_owner_call / high") {
            return Err(format!("hover should carry terse relation tag; got:\n{md}"));
        }
        Ok(())
    }

    #[test]
    fn seam_hover_class_reason_covers_each_grip_class() -> Result<(), String> {
        let mut seam = weakly_gripped_classified();
        for class in SeamGripClass::ALL {
            seam.class = class;
            let reason = seam_class_reason(&seam);
            if reason.is_empty() {
                return Err(format!("missing reason for {}", class.as_str()));
            }
        }
        Ok(())
    }

    #[test]
    fn strongly_gripped_seam_hover_explains_when_no_gap_evidence_is_recorded() -> Result<(), String>
    {
        let mut seam = weakly_gripped_classified();
        seam.class = SeamGripClass::StronglyGripped;
        seam.evidence.discriminate = stage(StageState::Yes, "Exact boundary assertion exists");
        seam.evidence.missing_discriminators.clear();
        let diagnostic = sample_diagnostic();
        let hover = classified_seam_hover_response(&seam, &diagnostic, None);
        let md = extract_markup(&hover)?;
        if !md.contains("no weak or missing stage evidence recorded") {
            return Err(format!("expected no-gap explanation in:\n{md}"));
        }
        Ok(())
    }

    #[test]
    fn ungripped_seam_hover_explains_when_no_yes_stage_evidence_is_recorded() -> Result<(), String>
    {
        let mut seam = weakly_gripped_classified();
        seam.class = SeamGripClass::Ungripped;
        seam.evidence.reach = stage(StageState::No, "No related test reaches discounted_total");
        seam.evidence.activate = stage(StageState::No, "No activation evidence recorded");
        seam.evidence.propagate = stage(StageState::No, "No propagation evidence recorded");
        seam.evidence.observe = stage(StageState::No, "No observation evidence recorded");
        seam.evidence.discriminate = stage(StageState::No, "No discriminator evidence recorded");
        seam.evidence.missing_discriminators.clear();
        let diagnostic = sample_diagnostic();
        let hover = classified_seam_hover_response(&seam, &diagnostic, None);
        let md = extract_markup(&hover)?;
        if !md.contains("no yes-stage evidence recorded in the current snapshot") {
            return Err(format!("expected no-yes-stage explanation in:\n{md}"));
        }
        Ok(())
    }

    #[test]
    fn opaque_seam_hover_advises_inspecting_static_limitation() -> Result<(), String> {
        let mut seam = weakly_gripped_classified();
        seam.class = SeamGripClass::Opaque;
        let diagnostic = sample_diagnostic();
        let hover = classified_seam_hover_response(&seam, &diagnostic, None);
        let md = extract_markup(&hover)?;
        if !md.contains("Inspect the static limitation") {
            return Err(format!("expected opaque next-step text in:\n{md}"));
        }
        Ok(())
    }
}
