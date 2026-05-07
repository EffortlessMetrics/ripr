//! Render classified seam gaps as agent-ready packets per
//! RIPR-SPEC-0005 (and the agent-packet shape in
//! `docs/OUTPUT_SCHEMA.md` § "Agent Seam Packets").
//!
//! Packets are emitted for actionable classes:
//!
//! - Headline-eligible classes (`Ungripped`, `WeaklyGripped`,
//!   `ReachableUnrevealed`, the four `*_unknown` classes) emit a
//!   `task: "write_targeted_test"` packet.
//! - `Opaque` emits a conservative `task: "inspect_static_limitation"`
//!   packet so the agent at least sees the static boundary.
//!
//! `StronglyGripped`, `Intentional`, and `Suppressed` produce no
//! packet — there is nothing for the agent to do.
//!
//! The packet schema is **0.3**, intentionally distinct from the
//! repo-exposure report's 0.1, because the packet is a separate
//! contract aimed at coding agents rather than reviewers.

use crate::analysis::ClassifiedSeam;
use crate::analysis::seams::{ExpectedSink, RequiredDiscriminator, SeamGripClass, SeamKind};
use crate::analysis::test_grip_evidence::TestGripEvidence;
use crate::output::json::escape as json_escape;

pub(crate) const AGENT_SEAM_PACKET_SCHEMA_VERSION: &str = "0.3";

/// Cap on related-tests rendered per packet. Mirrors the JSON-side
/// limit in `output::repo_exposure` so an agent inspecting the same
/// seam from either artifact sees the same evidence size.
const MAX_RELATED_TESTS_PER_PACKET: usize = 8;

/// Boilerplate string surfaced under `runtime_confirmation` to remind
/// agents that static evidence is preflight, not proof.
const RUNTIME_CONFIRMATION_NOTE: &str =
    "optional cargo-mutants confirmation; ripr reports static evidence only";

/// Render every actionable `ClassifiedSeam` in `classified` as an agent
/// packet, returning a JSON object with a `packets` array. Strongly-gripped,
/// intentional, and suppressed seams are skipped. `Opaque` seams emit a
/// conservative `inspect_static_limitation` packet so the agent at least
/// sees the static boundary that hides evidence.
pub(crate) fn render_agent_seam_packets_json(classified: &[ClassifiedSeam]) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": \"{}\",\n",
        AGENT_SEAM_PACKET_SCHEMA_VERSION
    ));
    out.push_str("  \"scope\": \"repo\",\n");

    let actionable: Vec<&ClassifiedSeam> = classified
        .iter()
        .filter(|entry| is_actionable(entry.class))
        .collect();

    out.push_str(&format!("  \"packets_total\": {},\n", actionable.len()));
    out.push_str("  \"packets\": [");
    for (idx, entry) in actionable.iter().enumerate() {
        if idx == 0 {
            out.push('\n');
        }
        push_packet_json(&mut out, entry);
        if idx + 1 != actionable.len() {
            out.push_str(",\n");
        } else {
            out.push('\n');
        }
    }
    if !actionable.is_empty() {
        out.push_str("  ");
    }
    out.push_str("]\n");
    out.push_str("}\n");
    out
}

/// Render the existing agent seam packet JSON envelope for one seam.
pub(crate) fn render_agent_seam_packet_json(entry: &ClassifiedSeam) -> String {
    render_agent_seam_packets_json(std::slice::from_ref(entry))
}

/// Return the first concrete assertion example carried by the agent
/// seam packet v2 shape. This follows the packet content itself:
/// any seam with a concrete assertion template can expose the editor
/// action, while prose-only guidance remains hidden.
pub(crate) fn suggested_assertion_for_classified_seam(entry: &ClassifiedSeam) -> Option<String> {
    suggested_assertions_for(entry.seam.kind(), entry.seam.owner(), &entry.evidence)
        .into_iter()
        .find(|suggestion| {
            let trimmed = suggestion.trim_start();
            !trimmed.starts_with("//") && trimmed.contains("assert")
        })
}

/// Render a compact human/agent work order for the next targeted test.
/// This is intentionally derived from the same fields as the structured
/// agent seam packet so editor actions and JSON packets stay aligned.
pub(crate) fn targeted_test_brief_for_classified_seam(entry: &ClassifiedSeam) -> String {
    let seam = &entry.seam;
    let evidence = &entry.evidence;
    let missing = missing_discriminator_records_for(entry);
    let patterns_to_imitate = patterns_to_imitate_for(evidence);
    let patterns_to_avoid = patterns_to_avoid_for(entry);
    let outline = targeted_test_brief_outline_for_classified_seam(entry);

    let mut out = String::new();
    out.push_str("Target seam:\n");
    out.push_str(&format!(
        "- {}:{}\n",
        display_path(seam.file()),
        seam.display_line()
    ));
    out.push_str(&format!("- {}\n", seam.kind().as_str()));
    out.push_str(&format!("- {}\n", entry.class.as_str()));
    out.push_str(&format!("- owner: {}\n", seam.owner()));

    out.push_str("\nWhy it matters:\n");
    if let Some(test) = evidence.related_tests.first() {
        out.push_str(&format!(
            "- Related test evidence: {} uses {} {} oracle.\n",
            test.test_name,
            test.oracle_strength.as_str(),
            test.oracle_kind.as_str()
        ));
    } else {
        out.push_str("- No related test location is visible in saved-workspace analysis.\n");
    }
    out.push_str(&format!(
        "- Static discriminator summary: {}\n",
        evidence.discriminate.summary
    ));
    for record in missing.iter().take(3) {
        out.push_str(&format!(
            "- Missing discriminator: {} ({})\n",
            record.value, record.reason
        ));
    }

    out.push_str("\nAdd a targeted test:\n");
    out.push_str(&format!(
        "- Suggested file: {}\n",
        display_path_text(&outline.suggested_file)
    ));
    out.push_str(&format!("- Suggested name: {}\n", outline.suggested_name));
    if let Some(value) = outline.candidate_value.as_ref() {
        out.push_str(&format!("- Candidate value: {value}\n"));
    }
    out.push_str(&format!("- Assertion shape: {}\n", outline.assertion_shape));

    if !patterns_to_imitate.is_empty() {
        out.push_str("\nImitate:\n");
        for pattern in patterns_to_imitate.iter().take(3) {
            out.push_str(&format!(
                "- {} ({})\n",
                pattern.test.test_name, pattern.reason
            ));
        }
    }

    if !patterns_to_avoid.is_empty() {
        out.push_str("\nAvoid:\n");
        for pattern in patterns_to_avoid.iter().take(3) {
            out.push_str(&format!("- {} ({})\n", pattern.pattern, pattern.reason));
        }
    }

    out
}

pub(crate) struct TargetedTestBriefOutline {
    pub(crate) suggested_file: String,
    pub(crate) suggested_name: String,
    pub(crate) candidate_value: Option<String>,
    pub(crate) assertion_shape: String,
}

pub(crate) fn targeted_test_brief_outline_for_classified_seam(
    entry: &ClassifiedSeam,
) -> TargetedTestBriefOutline {
    let recommended = recommended_test_for(entry);
    let missing = missing_discriminator_records_for(entry);
    let candidate_value = candidate_values_for(entry, &missing)
        .into_iter()
        .next()
        .map(|value| value.value);
    let assertion_shape =
        assertion_shape_for(entry.seam.kind(), entry.seam.owner(), &entry.evidence);

    TargetedTestBriefOutline {
        suggested_file: recommended.file,
        suggested_name: recommended.name,
        candidate_value,
        assertion_shape: assertion_shape.example,
    }
}

fn display_path(path: &std::path::Path) -> String {
    display_path_text(&path.to_string_lossy())
}

fn display_path_text(path: &str) -> String {
    path.replace('\\', "/")
}

fn is_actionable(class: SeamGripClass) -> bool {
    // Headline-eligible classes are the natural agent targets.
    // `Opaque` is also actionable as `inspect_static_limitation`.
    // `Intentional` and `Suppressed` are governance classes; the
    // agent should not be told to "fix" them.
    class.is_headline_eligible() || matches!(class, SeamGripClass::Opaque)
}

fn task_for(class: SeamGripClass) -> &'static str {
    match class {
        SeamGripClass::Opaque => "inspect_static_limitation",
        _ => "write_targeted_test",
    }
}

fn push_packet_json(out: &mut String, entry: &ClassifiedSeam) {
    let seam = &entry.seam;
    let evidence = &entry.evidence;
    out.push_str("    {\n");
    out.push_str(&format!("      \"task\": \"{}\",\n", task_for(entry.class)));
    out.push_str(&format!(
        "      \"seam_id\": \"{}\",\n",
        json_escape(seam.id().as_str())
    ));
    out.push_str(&format!(
        "      \"owner\": \"{}\",\n",
        json_escape(seam.owner())
    ));
    out.push_str(&format!(
        "      \"seam_kind\": \"{}\",\n",
        seam.kind().as_str()
    ));
    out.push_str(&format!(
        "      \"file\": \"{}\",\n",
        json_escape(&seam.file().to_string_lossy())
    ));
    out.push_str(&format!("      \"line\": {},\n", seam.display_line()));
    out.push_str(&format!(
        "      \"changed_expression\": \"{}\",\n",
        json_escape(seam.expression())
    ));
    out.push_str(&format!(
        "      \"current_grip\": \"{}\",\n",
        entry.class.as_str()
    ));
    out.push_str(&format!(
        "      \"headline_eligible\": {},\n",
        entry.class.is_headline_eligible()
    ));

    let recommended = recommended_test_for(entry);
    out.push_str("      \"recommended_test\": {");
    out.push_str(&format!(
        "\"name\": \"{}\", ",
        json_escape(recommended.name.as_str())
    ));
    out.push_str(&format!(
        "\"file\": \"{}\", ",
        json_escape(recommended.file.as_str())
    ));
    out.push_str(&format!(
        "\"reason\": \"{}\"",
        json_escape(recommended.reason.as_str())
    ));
    out.push_str("},\n");

    let nearest_strong = nearest_strong_test_to_imitate(evidence);
    out.push_str("      \"nearest_strong_test_to_imitate\": ");
    if let Some(test) = nearest_strong {
        push_related_test_reference(out, test, "nearest strong related test by ranked evidence");
    } else {
        out.push_str("null");
    }
    out.push_str(",\n");

    out.push_str("      \"evidence\": {");
    out.push_str(&format!(
        "\"reach\": \"{}\", ",
        evidence.reach.state.as_str()
    ));
    out.push_str(&format!(
        "\"activate\": \"{}\", ",
        evidence.activate.state.as_str()
    ));
    out.push_str(&format!(
        "\"propagate\": \"{}\", ",
        evidence.propagate.state.as_str()
    ));
    out.push_str(&format!(
        "\"observe\": \"{}\", ",
        evidence.observe.state.as_str()
    ));
    out.push_str(&format!(
        "\"discriminate\": \"{}\"",
        evidence.discriminate.state.as_str()
    ));
    out.push_str("},\n");

    out.push_str("      \"observed_values\": [");
    for (idx, value) in evidence.observed_values.iter().enumerate() {
        out.push_str(&format!("\"{}\"", json_escape(value.value.as_str())));
        if idx + 1 != evidence.observed_values.len() {
            out.push_str(", ");
        }
    }
    out.push_str("],\n");

    let missing = missing_discriminator_records_for(entry);
    let candidate_values = candidate_values_for(entry, &missing);
    out.push_str("      \"missing_discriminators\": [");
    if !missing.is_empty() {
        out.push('\n');
        for (idx, record) in missing.iter().enumerate() {
            out.push_str(&format!(
                "        {{\"value\": \"{}\", \"reason\": \"{}\"}}",
                json_escape(record.value.as_str()),
                json_escape(record.reason.as_str())
            ));
            if idx + 1 != missing.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("      ");
    }
    out.push_str("],\n");

    out.push_str("      \"candidate_values\": [");
    if !candidate_values.is_empty() {
        out.push('\n');
        for (idx, value) in candidate_values.iter().enumerate() {
            out.push_str(&format!(
                "        {{\"value\": \"{}\", \"reason\": \"{}\"}}",
                json_escape(value.value.as_str()),
                json_escape(value.reason.as_str())
            ));
            if idx + 1 != candidate_values.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("      ");
    }
    out.push_str("],\n");

    out.push_str(&format!(
        "      \"missing_oracle_shape\": \"{}\",\n",
        json_escape(&missing_oracle_shape_for(seam.kind(), seam.expected_sink()))
    ));

    let assertion_shape = assertion_shape_for(seam.kind(), seam.owner(), evidence);
    out.push_str("      \"assertion_shape\": {");
    out.push_str(&format!("\"kind\": \"{}\", ", assertion_shape.kind));
    out.push_str(&format!(
        "\"example\": \"{}\"",
        json_escape(assertion_shape.example.as_str())
    ));
    out.push_str("},\n");

    out.push_str("      \"related_existing_tests\": [");
    if !evidence.related_tests.is_empty() {
        out.push('\n');
        let cap = evidence
            .related_tests
            .len()
            .min(MAX_RELATED_TESTS_PER_PACKET);
        for (idx, grip) in evidence.related_tests.iter().take(cap).enumerate() {
            out.push_str("        {");
            out.push_str(&format!(
                "\"name\": \"{}\", ",
                json_escape(grip.test_name.as_str())
            ));
            out.push_str(&format!(
                "\"file\": \"{}\", ",
                json_escape(&grip.file.to_string_lossy())
            ));
            out.push_str(&format!("\"line\": {}, ", grip.line));
            out.push_str(&format!(
                "\"oracle_kind\": \"{}\", ",
                grip.oracle_kind.as_str()
            ));
            out.push_str(&format!(
                "\"oracle_strength\": \"{}\", ",
                grip.oracle_strength.as_str()
            ));
            out.push_str(&format!(
                "\"evidence_summary\": \"{}\", ",
                json_escape(grip.evidence_summary.as_str())
            ));
            out.push_str(&format!(
                "\"relation_reason\": \"{}\", ",
                grip.relation_reason.as_str()
            ));
            out.push_str(&format!(
                "\"relation_confidence\": \"{}\"",
                grip.relation_confidence.as_str()
            ));
            out.push('}');
            if idx + 1 != cap {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("      ");
    }
    out.push_str("],\n");

    let patterns_to_imitate = patterns_to_imitate_for(evidence);
    out.push_str("      \"patterns_to_imitate\": [");
    if !patterns_to_imitate.is_empty() {
        out.push('\n');
        for (idx, pattern) in patterns_to_imitate.iter().enumerate() {
            out.push_str("        ");
            push_related_test_reference(out, pattern.test, pattern.reason.as_str());
            if idx + 1 != patterns_to_imitate.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("      ");
    }
    out.push_str("],\n");

    let patterns_to_avoid = patterns_to_avoid_for(entry);
    out.push_str("      \"patterns_to_avoid\": [");
    if !patterns_to_avoid.is_empty() {
        out.push('\n');
        for (idx, pattern) in patterns_to_avoid.iter().enumerate() {
            out.push_str(&format!(
                "        {{\"pattern\": \"{}\", \"reason\": \"{}\"}}",
                json_escape(pattern.pattern.as_str()),
                json_escape(pattern.reason.as_str())
            ));
            if idx + 1 != patterns_to_avoid.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("      ");
    }
    out.push_str("],\n");

    let suggested = suggested_assertions_for(seam.kind(), seam.owner(), evidence);
    out.push_str("      \"suggested_assertions\": [");
    for (idx, suggestion) in suggested.iter().enumerate() {
        out.push_str(&format!("\"{}\"", json_escape(suggestion)));
        if idx + 1 != suggested.len() {
            out.push_str(", ");
        }
    }
    out.push_str("],\n");
    out.push_str(&format!(
        "      \"confidence\": \"{}\",\n",
        packet_confidence_for(entry)
    ));
    out.push_str(&format!(
        "      \"runtime_confirmation\": \"{}\"\n",
        json_escape(RUNTIME_CONFIRMATION_NOTE)
    ));
    out.push_str("    }");
}

/// A flat (value, reason) record carried in the packet's
/// `missing_discriminators` array. Mirrors the field shape of
/// `MissingDiscriminatorFact` but excludes `flow_sink` because the
/// packet already carries the sink class via `missing_oracle_shape`.
pub(crate) struct MissingRecord {
    pub(crate) value: String,
    pub(crate) reason: String,
}

pub(crate) struct CandidateValue {
    pub(crate) value: String,
    pub(crate) reason: String,
}

pub(crate) struct RecommendedTest {
    pub(crate) name: String,
    pub(crate) file: String,
    pub(crate) reason: String,
}

pub(crate) struct AssertionShape {
    pub(crate) kind: &'static str,
    pub(crate) example: String,
}

struct ImitationPattern<'a> {
    test: &'a crate::analysis::test_grip_evidence::RelatedTestGrip,
    reason: String,
}

struct AvoidPattern {
    pattern: String,
    reason: String,
}

pub(crate) fn recommended_test_for(entry: &ClassifiedSeam) -> RecommendedTest {
    let owner_short = owner_short(entry.seam.owner());
    let name = format!(
        "{}_{}",
        snake_case_token(owner_short),
        test_name_suffix_for(entry.seam.kind())
    );
    if let Some(test) = nearest_strong_test_to_imitate(&entry.evidence) {
        return RecommendedTest {
            name,
            file: test.file.to_string_lossy().to_string(),
            reason: "place the new targeted test next to the nearest strong related test"
                .to_string(),
        };
    }
    if let Some(test) = entry.evidence.related_tests.first() {
        return RecommendedTest {
            name,
            file: test.file.to_string_lossy().to_string(),
            reason: "place the new targeted test next to the highest-confidence related test"
                .to_string(),
        };
    }
    RecommendedTest {
        name,
        file: inferred_test_file(entry.seam.file(), owner_short),
        reason: "no related test file was visible; inferred from the production seam file"
            .to_string(),
    }
}

fn owner_short(owner: &str) -> &str {
    owner.rsplit("::").next().unwrap_or(owner)
}

fn test_name_suffix_for(kind: SeamKind) -> &'static str {
    match kind {
        SeamKind::PredicateBoundary => "boundary_discriminator",
        SeamKind::ErrorVariant => "exact_error_variant",
        SeamKind::ReturnValue => "return_value_discriminator",
        SeamKind::FieldConstruction => "field_discriminator",
        SeamKind::SideEffect => "side_effect_observer",
        SeamKind::MatchArm => "match_arm_discriminator",
        SeamKind::CallPresence => "call_presence_observer",
    }
}

fn inferred_test_file(file: &std::path::Path, owner_short: &str) -> String {
    let stem = file
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(owner_short);
    format!("tests/{}_tests.rs", snake_case_token(stem))
}

fn snake_case_token(raw: &str) -> String {
    let mut out = String::new();
    let mut previous_was_sep = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            previous_was_sep = false;
        } else if !previous_was_sep && !out.is_empty() {
            out.push('_');
            previous_was_sep = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "targeted".to_string()
    } else {
        out
    }
}

pub(crate) fn nearest_strong_test_to_imitate(
    evidence: &TestGripEvidence,
) -> Option<&crate::analysis::test_grip_evidence::RelatedTestGrip> {
    evidence
        .related_tests
        .iter()
        .find(|test| test.oracle_strength == crate::domain::OracleStrength::Strong)
}

fn push_related_test_reference(
    out: &mut String,
    test: &crate::analysis::test_grip_evidence::RelatedTestGrip,
    reason: &str,
) {
    out.push('{');
    out.push_str(&format!(
        "\"name\": \"{}\", ",
        json_escape(test.test_name.as_str())
    ));
    out.push_str(&format!(
        "\"file\": \"{}\", ",
        json_escape(&test.file.to_string_lossy())
    ));
    out.push_str(&format!("\"line\": {}, ", test.line));
    out.push_str(&format!(
        "\"oracle_kind\": \"{}\", ",
        test.oracle_kind.as_str()
    ));
    out.push_str(&format!(
        "\"oracle_strength\": \"{}\", ",
        test.oracle_strength.as_str()
    ));
    out.push_str(&format!(
        "\"relation_reason\": \"{}\", ",
        test.relation_reason.as_str()
    ));
    out.push_str(&format!(
        "\"relation_confidence\": \"{}\", ",
        test.relation_confidence.as_str()
    ));
    out.push_str(&format!("\"reason\": \"{}\"", json_escape(reason)));
    out.push('}');
}

pub(crate) fn candidate_values_for(
    entry: &ClassifiedSeam,
    missing: &[MissingRecord],
) -> Vec<CandidateValue> {
    let mut out: Vec<CandidateValue> = missing
        .iter()
        .map(|record| CandidateValue {
            value: record.value.clone(),
            reason: record.reason.clone(),
        })
        .collect();
    if out.is_empty() {
        out.push(candidate_value_from_required(
            entry.seam.required_discriminator(),
        ));
    }
    out
}

fn candidate_value_from_required(required: &RequiredDiscriminator) -> CandidateValue {
    match required {
        RequiredDiscriminator::BoundaryValue { description } => CandidateValue {
            value: format!("input that exercises {description}"),
            reason: "exercise the predicate boundary named by the seam".to_string(),
        },
        RequiredDiscriminator::ErrorVariant { variant } => CandidateValue {
            value: format!("input that triggers {variant}"),
            reason: "force the exact error variant rather than any error".to_string(),
        },
        RequiredDiscriminator::ReturnValue { description } => CandidateValue {
            value: format!("input that changes {description}"),
            reason: "observe the returned value sink named by the seam".to_string(),
        },
        RequiredDiscriminator::FieldValue { field } => CandidateValue {
            value: format!("input that sets {field}"),
            reason: "observe the constructed field value".to_string(),
        },
        RequiredDiscriminator::Effect { sink } => CandidateValue {
            value: format!("input that produces {sink}"),
            reason: "observe the side effect sink".to_string(),
        },
        RequiredDiscriminator::MatchArmTaken { arm } => CandidateValue {
            value: format!("input that selects {arm}"),
            reason: "exercise the changed match arm".to_string(),
        },
        RequiredDiscriminator::CallSite { target } => CandidateValue {
            value: format!("input that reaches call {target}"),
            reason: "observe the call site with a mock or spy".to_string(),
        },
    }
}

pub(crate) fn assertion_shape_for(
    kind: SeamKind,
    owner: &str,
    evidence: &TestGripEvidence,
) -> AssertionShape {
    let example = suggested_assertions_for(kind, owner, evidence)
        .into_iter()
        .next()
        .unwrap_or_else(|| "assert_eq!(actual, expected)".to_string());
    AssertionShape {
        kind: assertion_shape_kind_for(kind),
        example,
    }
}

fn assertion_shape_kind_for(kind: SeamKind) -> &'static str {
    match kind {
        SeamKind::PredicateBoundary => "exact_return_value",
        SeamKind::ErrorVariant => "exact_error_variant",
        SeamKind::ReturnValue => "exact_return_value",
        SeamKind::FieldConstruction => "field_equality",
        SeamKind::SideEffect => "side_effect_observer",
        SeamKind::MatchArm => "match_result",
        SeamKind::CallPresence => "call_expectation",
    }
}

fn patterns_to_imitate_for(evidence: &TestGripEvidence) -> Vec<ImitationPattern<'_>> {
    evidence
        .related_tests
        .iter()
        .filter(|test| {
            matches!(
                test.oracle_strength,
                crate::domain::OracleStrength::Strong | crate::domain::OracleStrength::Medium
            )
        })
        .take(3)
        .map(|test| ImitationPattern {
            test,
            reason: format!(
                "{} {} oracle with {} relation",
                test.oracle_strength.as_str(),
                test.oracle_kind.as_str(),
                test.relation_confidence.as_str()
            ),
        })
        .collect()
}

fn patterns_to_avoid_for(entry: &ClassifiedSeam) -> Vec<AvoidPattern> {
    let mut out: Vec<AvoidPattern> = entry
        .evidence
        .related_tests
        .iter()
        .filter(|test| {
            matches!(
                test.oracle_strength,
                crate::domain::OracleStrength::Weak
                    | crate::domain::OracleStrength::Smoke
                    | crate::domain::OracleStrength::None
                    | crate::domain::OracleStrength::Unknown
            )
        })
        .take(3)
        .map(|test| AvoidPattern {
            pattern: format!(
                "{} in {}",
                test.oracle_kind.as_str(),
                test.test_name.as_str()
            ),
            reason: "this related test reaches nearby behavior but lacks an exact discriminator"
                .to_string(),
        })
        .collect();
    if !entry.evidence.missing_discriminators.is_empty() {
        out.push(AvoidPattern {
            pattern: "adding another test with only already-observed values".to_string(),
            reason: "candidate values should include the missing discriminator".to_string(),
        });
    }
    if out.is_empty() && matches!(entry.class, SeamGripClass::Ungripped) {
        out.push(AvoidPattern {
            pattern: "copying a smoke-only test shape".to_string(),
            reason: "ungripped seams need a meaningful observer, not just execution".to_string(),
        });
    }
    out
}

fn packet_confidence_for(entry: &ClassifiedSeam) -> &'static str {
    if matches!(entry.class, SeamGripClass::Opaque) {
        return "unknown";
    }
    if entry.evidence.related_tests.iter().any(|test| {
        test.relation_confidence == crate::analysis::test_grip_evidence::RelationConfidence::High
    }) {
        return "high";
    }
    if entry.evidence.related_tests.iter().any(|test| {
        test.relation_confidence == crate::analysis::test_grip_evidence::RelationConfidence::Medium
    }) || !entry.evidence.missing_discriminators.is_empty()
    {
        return "medium";
    }
    "low"
}

/// Build the `missing_discriminators` array carried in the packet,
/// pairing analyzer-emitted hypotheses with a predicate-boundary
/// fallback when the seam expression names a clear boundary.
pub(crate) fn missing_discriminator_records_for(entry: &ClassifiedSeam) -> Vec<MissingRecord> {
    let mut out: Vec<MissingRecord> = entry
        .evidence
        .missing_discriminators
        .iter()
        .map(|m| MissingRecord {
            value: m.value.clone(),
            reason: m.reason.clone(),
        })
        .collect();
    // For predicate-boundary seams, surface the boundary expression
    // explicitly even when the analyzer hypothesis only names the RHS
    // token (or hasn't fired). This pins the most common ask.
    if matches!(entry.seam.kind(), SeamKind::PredicateBoundary)
        && let RequiredDiscriminator::BoundaryValue { description } =
            entry.seam.required_discriminator()
        && !out.iter().any(|r| r.value.contains(description.as_str()))
    {
        out.push(MissingRecord {
            value: format!("input that hits the boundary: {description}"),
            reason: "predicate uses an equality-bearing operator; tests should exercise the boundary case"
                .to_string(),
        });
    }
    out
}

/// Suggest the oracle *shape* a test should use, derived from the
/// seam's kind and expected sink. The returned string is human-facing
/// guidance — the suggested-assertion list carries the literal
/// templates.
fn missing_oracle_shape_for(kind: SeamKind, sink: ExpectedSink) -> String {
    match kind {
        SeamKind::PredicateBoundary => {
            "exact returned value assertion at the equality boundary".to_string()
        }
        SeamKind::ErrorVariant => {
            "exact error-variant assertion (matches! / assert_matches!)".to_string()
        }
        SeamKind::ReturnValue => "exact value assertion on the returned value".to_string(),
        SeamKind::FieldConstruction => "field equality or whole-object assertion".to_string(),
        SeamKind::SideEffect => format!(
            "mock expectation, event/state observer, or persistence assertion ({})",
            sink.as_str()
        ),
        SeamKind::MatchArm => "exact value assertion on the match result".to_string(),
        SeamKind::CallPresence => "mock or spy assertion on the call site".to_string(),
    }
}

/// Best-effort assertion templates the agent can fill in. These are
/// guidance, not generated tests — placeholders are intentional.
fn suggested_assertions_for(
    kind: SeamKind,
    owner: &str,
    evidence: &TestGripEvidence,
) -> Vec<String> {
    let owner_short = owner.rsplit("::").next().unwrap_or(owner);
    match kind {
        SeamKind::PredicateBoundary => {
            // Suggest the equality-boundary case using the missing
            // discriminator hypothesis when present.
            if let Some(missing) = evidence.missing_discriminators.first() {
                vec![format!(
                    "assert_eq!({owner_short}(/* {} */), /* expected */)",
                    missing.value
                )]
            } else {
                vec![format!(
                    "assert_eq!({owner_short}(/* boundary input */), /* expected */)"
                )]
            }
        }
        SeamKind::ErrorVariant => vec![format!(
            "assert!(matches!({owner_short}(/* trigger */), Err(/* exact variant */)))"
        )],
        SeamKind::ReturnValue => vec![format!(
            "assert_eq!({owner_short}(/* input */), /* expected */)"
        )],
        SeamKind::FieldConstruction => vec![format!(
            "let result = {owner_short}(/* input */); assert_eq!(result.field, /* expected */);"
        )],
        SeamKind::SideEffect => vec![format!(
            "// arrange a mock/observer; assert {owner_short}(...) produced the expected effect"
        )],
        SeamKind::MatchArm => vec![format!(
            "assert_eq!({owner_short}(/* input selecting this arm */), /* expected */)"
        )],
        SeamKind::CallPresence => vec![format!(
            "// assert that {owner_short} called the expected target"
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
    use crate::analysis::test_grip_evidence::{RelatedTestGrip, TestGripEvidence};
    use crate::domain::{
        Confidence, MissingDiscriminatorFact, OracleKind, OracleStrength, StageEvidence,
        StageState, ValueContext, ValueFact,
    };
    use std::path::PathBuf;

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, "test stage")
    }

    fn boundary_seam() -> RepoSeam {
        RepoSeam::new(
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
        )
    }

    fn seam_with(
        owner: &str,
        kind: SeamKind,
        required: RequiredDiscriminator,
        sink: ExpectedSink,
    ) -> RepoSeam {
        RepoSeam::new(
            "src/service.rs",
            owner,
            kind,
            7,
            14,
            "changed expression",
            required,
            sink,
        )
    }

    fn related_test_with(
        name: &str,
        oracle_kind: OracleKind,
        oracle_strength: OracleStrength,
        relation_confidence: crate::analysis::test_grip_evidence::RelationConfidence,
    ) -> RelatedTestGrip {
        RelatedTestGrip {
            test_name: name.to_string(),
            file: PathBuf::from("tests/service.rs"),
            line: 21,
            oracle_kind,
            oracle_strength,
            evidence_summary: "related oracle evidence".to_string(),
            relation_reason: crate::analysis::test_grip_evidence::RelationReason::DirectOwnerCall,
            relation_confidence,
        }
    }

    fn classified_with(
        seam: RepoSeam,
        class: SeamGripClass,
        related_tests: Vec<RelatedTestGrip>,
    ) -> ClassifiedSeam {
        let seam_id = seam.id().clone();
        ClassifiedSeam {
            seam,
            evidence: TestGripEvidence {
                seam_id,
                related_tests,
                reach: stage(StageState::Yes),
                activate: stage(StageState::Yes),
                propagate: stage(StageState::Weak),
                observe: stage(StageState::Weak),
                discriminate: stage(StageState::No),
                observed_values: Vec::new(),
                missing_discriminators: Vec::new(),
            },
            class,
        }
    }

    fn weakly_gripped_classified() -> ClassifiedSeam {
        let seam = boundary_seam();
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
            reach: stage(StageState::Yes),
            activate: stage(StageState::Yes),
            propagate: stage(StageState::Yes),
            observe: stage(StageState::Yes),
            discriminate: stage(StageState::Yes),
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

    fn ungripped_classified() -> ClassifiedSeam {
        let seam = boundary_seam();
        let evidence = TestGripEvidence {
            seam_id: seam.id().clone(),
            related_tests: Vec::new(),
            reach: stage(StageState::No),
            activate: stage(StageState::No),
            propagate: stage(StageState::No),
            observe: stage(StageState::No),
            discriminate: stage(StageState::No),
            observed_values: Vec::new(),
            missing_discriminators: Vec::new(),
        };
        ClassifiedSeam {
            seam,
            evidence,
            class: SeamGripClass::Ungripped,
        }
    }

    fn strongly_gripped_classified() -> ClassifiedSeam {
        let seam = boundary_seam();
        let evidence = TestGripEvidence {
            seam_id: seam.id().clone(),
            related_tests: Vec::new(),
            reach: stage(StageState::Yes),
            activate: stage(StageState::Yes),
            propagate: stage(StageState::Yes),
            observe: stage(StageState::Yes),
            discriminate: stage(StageState::Yes),
            observed_values: Vec::new(),
            missing_discriminators: Vec::new(),
        };
        ClassifiedSeam {
            seam,
            evidence,
            class: SeamGripClass::StronglyGripped,
        }
    }

    #[test]
    fn given_weakly_gripped_boundary_seam_when_packet_is_rendered_then_missing_boundary_value_is_present()
    -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        if !json.contains("\"current_grip\": \"weakly_gripped\"") {
            return Err(format!("missing current_grip in: {json}"));
        }
        if !json.contains("\"headline_eligible\": true") {
            return Err(format!("missing headline_eligible: {json}"));
        }
        if !json.contains("discount_threshold (equality boundary)") {
            return Err(format!(
                "expected boundary value in missing_discriminators: {json}"
            ));
        }
        if !json.contains("\"missing_oracle_shape\": \"exact returned value assertion") {
            return Err(format!("expected predicate-boundary oracle shape: {json}"));
        }
        if !json.contains("\"runtime_confirmation\":") {
            return Err(format!("missing runtime_confirmation: {json}"));
        }
        Ok(())
    }

    #[test]
    fn missing_discriminators_carry_value_and_reason_objects() -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        if !json.contains(
            "{\"value\": \"discount_threshold (equality boundary)\", \"reason\": \"observed values do not include the equality-boundary case\"}",
        ) {
            return Err(format!(
                "expected structured missing_discriminator record in: {json}"
            ));
        }
        Ok(())
    }

    #[test]
    fn given_opaque_seam_when_packet_is_rendered_then_task_is_inspect_static_limitation()
    -> Result<(), String> {
        let mut entry = weakly_gripped_classified();
        entry.class = SeamGripClass::Opaque;
        let json = render_agent_seam_packets_json(&[entry]);
        if !json.contains("\"task\": \"inspect_static_limitation\"") {
            return Err(format!(
                "expected task=inspect_static_limitation for opaque seam: {json}"
            ));
        }
        if !json.contains("\"current_grip\": \"opaque\"") {
            return Err(format!("missing current_grip=opaque: {json}"));
        }
        if !json.contains("\"headline_eligible\": false") {
            return Err(format!(
                "expected headline_eligible=false for opaque: {json}"
            ));
        }
        Ok(())
    }

    #[test]
    fn predicate_boundary_fallback_emits_when_no_analyzer_hypothesis_fired() -> Result<(), String> {
        // Construct a weakly-gripped predicate seam with EMPTY
        // missing_discriminators (no analyzer hypothesis). The packet
        // should still surface the equality-boundary fallback so an
        // agent has something to act on.
        let mut entry = weakly_gripped_classified();
        entry.evidence.missing_discriminators = Vec::new();
        let json = render_agent_seam_packets_json(&[entry]);
        if !json
            .contains("\"value\": \"input that hits the boundary: amount >= discount_threshold\"")
        {
            return Err(format!(
                "expected predicate-boundary fallback record when analyzer hypothesis is empty: {json}"
            ));
        }
        if !json.contains("predicate uses an equality-bearing operator") {
            return Err(format!("expected fallback reason text: {json}"));
        }
        Ok(())
    }

    #[test]
    fn given_intentional_seam_when_packets_are_requested_then_no_packet_is_emitted()
    -> Result<(), String> {
        let mut entry = weakly_gripped_classified();
        entry.class = SeamGripClass::Intentional;
        let json = render_agent_seam_packets_json(&[entry]);
        if !json.contains("\"packets_total\": 0") {
            return Err(format!("intentional seam should produce no packet: {json}"));
        }
        Ok(())
    }

    #[test]
    fn given_ungripped_seam_when_packet_is_rendered_then_task_is_write_targeted_test()
    -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[ungripped_classified()]);
        if !json.contains("\"task\": \"write_targeted_test\"") {
            return Err(format!("missing task field: {json}"));
        }
        if !json.contains("\"current_grip\": \"ungripped\"") {
            return Err(format!("missing current_grip ungripped: {json}"));
        }
        Ok(())
    }

    #[test]
    fn given_strongly_gripped_seam_when_packets_are_requested_then_no_actionable_packet_is_emitted()
    -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[strongly_gripped_classified()]);
        if !json.contains("\"packets_total\": 0") {
            return Err(format!(
                "expected packets_total=0 for strongly-gripped input: {json}"
            ));
        }
        if !json.contains("\"packets\": []") {
            return Err(format!("expected empty packets array: {json}"));
        }
        Ok(())
    }

    #[test]
    fn given_related_tests_when_packet_is_rendered_then_oracle_kind_and_strength_are_present()
    -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        for needle in [
            "\"name\": \"below_threshold_has_no_discount\"",
            "\"oracle_kind\": \"exact_value\"",
            "\"oracle_strength\": \"strong\"",
        ] {
            if !json.contains(needle) {
                return Err(format!("missing {needle:?} in: {json}"));
            }
        }
        Ok(())
    }

    #[test]
    fn given_agent_packet_with_related_tests_when_rendered_then_relation_fields_are_emitted() {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        assert!(
            json.contains("\"relation_reason\": \"direct_owner_call\""),
            "relation_reason missing: {json}"
        );
        assert!(
            json.contains("\"relation_confidence\": \"high\""),
            "relation_confidence missing: {json}"
        );
    }

    #[test]
    fn given_agent_packet_with_related_tests_when_rendered_then_highest_confidence_test_is_first()
    -> Result<(), String> {
        // Build an evidence record with two related tests where the
        // first by file/name order is low-confidence and the second is
        // high-confidence. The renderer iterates `related_tests` in
        // order, so the *order in the vec* is what determines which
        // appears first in the packet — confirm the Vec is already
        // ranked.
        use crate::analysis::test_grip_evidence::{RelationConfidence, RelationReason};
        let mut entry = weakly_gripped_classified();
        let high = RelatedTestGrip {
            test_name: "z_high_confidence".to_string(),
            file: PathBuf::from("tests/zeta.rs"),
            line: 1,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            evidence_summary: "exact value assertion".to_string(),
            relation_reason: RelationReason::DirectOwnerCall,
            relation_confidence: RelationConfidence::High,
        };
        let low = RelatedTestGrip {
            test_name: "a_low_confidence".to_string(),
            file: PathBuf::from("tests/alpha.rs"),
            line: 1,
            oracle_kind: OracleKind::Unknown,
            oracle_strength: OracleStrength::None,
            evidence_summary: "no oracle in test body".to_string(),
            relation_reason: RelationReason::FixtureOwnerAffinity,
            relation_confidence: RelationConfidence::Low,
        };
        // Caller provides a ranked vec — `evidence_for_seam` always
        // emits ranked, so this mirrors the production path.
        entry.evidence.related_tests = vec![high, low];

        let json = render_agent_seam_packets_json(&[entry]);
        let high_idx = json
            .find("\"name\": \"z_high_confidence\"")
            .ok_or_else(|| "high-confidence test missing".to_string())?;
        let low_idx = json
            .find("\"name\": \"a_low_confidence\"")
            .ok_or_else(|| "low-confidence test missing".to_string())?;
        if high_idx >= low_idx {
            return Err(format!(
                "high-confidence test must render before low-confidence; \
                 high@{high_idx} low@{low_idx}"
            ));
        }
        Ok(())
    }

    #[test]
    fn packet_v2_carries_recommended_test_candidate_values_assertion_shape_and_confidence()
    -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        for needle in [
            "\"recommended_test\": {\"name\": \"discounted_total_boundary_discriminator\"",
            "\"file\": \"tests/pricing.rs\"",
            "\"nearest_strong_test_to_imitate\": {\"name\": \"below_threshold_has_no_discount\"",
            "\"candidate_values\": [",
            "\"value\": \"discount_threshold (equality boundary)\"",
            "\"assertion_shape\": {\"kind\": \"exact_return_value\"",
            "\"example\": \"assert_eq!(discounted_total(/* discount_threshold (equality boundary) */), /* expected */)\"",
            "\"confidence\": \"high\"",
        ] {
            if !json.contains(needle) {
                return Err(format!("missing v2 field {needle:?} in: {json}"));
            }
        }
        Ok(())
    }

    #[test]
    fn packet_v2_carries_patterns_to_imitate_and_avoid() -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        for needle in [
            "\"patterns_to_imitate\": [",
            "\"reason\": \"strong exact_value oracle with high relation\"",
            "\"patterns_to_avoid\": [",
            "\"pattern\": \"adding another test with only already-observed values\"",
            "\"reason\": \"candidate values should include the missing discriminator\"",
        ] {
            if !json.contains(needle) {
                return Err(format!("missing pattern field {needle:?} in: {json}"));
            }
        }
        Ok(())
    }

    #[test]
    fn targeted_test_brief_carries_plain_text_work_order() -> Result<(), String> {
        let brief = targeted_test_brief_for_classified_seam(&weakly_gripped_classified());
        for needle in [
            "Target seam:",
            "- src/pricing.rs:88",
            "- predicate_boundary",
            "- weakly_gripped",
            "- owner: pricing::discounted_total",
            "Why it matters:",
            "- Related test evidence: below_threshold_has_no_discount uses strong exact_value oracle.",
            "- Missing discriminator: discount_threshold (equality boundary)",
            "Add a targeted test:",
            "- Suggested file: tests/pricing.rs",
            "- Suggested name: discounted_total_boundary_discriminator",
            "- Candidate value: discount_threshold (equality boundary)",
            "- Assertion shape: assert_eq!(discounted_total(/* discount_threshold (equality boundary) */), /* expected */)",
            "Imitate:",
            "- below_threshold_has_no_discount (strong exact_value oracle with high relation)",
            "Avoid:",
            "- adding another test with only already-observed values",
        ] {
            if !brief.contains(needle) {
                return Err(format!("missing brief text {needle:?} in:\n{brief}"));
            }
        }
        Ok(())
    }

    #[test]
    fn targeted_test_brief_uses_inferred_file_when_no_related_test_exists() -> Result<(), String> {
        let brief = targeted_test_brief_for_classified_seam(&ungripped_classified());
        for needle in [
            "- No related test location is visible in saved-workspace analysis.",
            "- Suggested file: tests/pricing_tests.rs",
            "- Candidate value: input that hits the boundary: amount >= discount_threshold",
            "- copying a smoke-only test shape",
        ] {
            if !brief.contains(needle) {
                return Err(format!(
                    "missing inferred brief text {needle:?} in:\n{brief}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn packet_v2_recommends_inferred_test_file_when_no_related_test_exists() -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[ungripped_classified()]);
        for needle in [
            "\"recommended_test\": {\"name\": \"discounted_total_boundary_discriminator\"",
            "\"file\": \"tests/pricing_tests.rs\"",
            "\"nearest_strong_test_to_imitate\": null",
            "\"confidence\": \"low\"",
        ] {
            if !json.contains(needle) {
                return Err(format!(
                    "missing inferred recommendation {needle:?} in: {json}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn packet_v2_carries_exact_error_variant_guidance_for_error_seams() -> Result<(), String> {
        let seam = seam_with(
            "auth::authenticate",
            SeamKind::ErrorVariant,
            RequiredDiscriminator::ErrorVariant {
                variant: "AuthError::RevokedToken".to_string(),
            },
            ExpectedSink::ErrorChannel,
        );
        let related = related_test_with(
            "empty_token_is_rejected",
            OracleKind::BroadError,
            OracleStrength::Weak,
            crate::analysis::test_grip_evidence::RelationConfidence::High,
        );
        let json = render_agent_seam_packets_json(&[classified_with(
            seam,
            SeamGripClass::WeaklyGripped,
            vec![related],
        )]);
        for needle in [
            "\"name\": \"authenticate_exact_error_variant\"",
            "\"candidate_values\": [",
            "\"value\": \"input that triggers AuthError::RevokedToken\"",
            "\"missing_oracle_shape\": \"exact error-variant assertion",
            "\"assertion_shape\": {\"kind\": \"exact_error_variant\"",
            "assert!(matches!(authenticate(/* trigger */), Err(/* exact variant */)))",
            "\"pattern\": \"broad_error in empty_token_is_rejected\"",
        ] {
            if !json.contains(needle) {
                return Err(format!(
                    "missing error-variant guidance {needle:?} in: {json}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn packet_v2_carries_side_effect_and_call_observer_guidance() -> Result<(), String> {
        let side_effect = seam_with(
            "billing::charge_customer",
            SeamKind::SideEffect,
            RequiredDiscriminator::Effect {
                sink: "payment event".to_string(),
            },
            ExpectedSink::SideEffect,
        );
        let call_presence = seam_with(
            "billing::sync_invoice",
            SeamKind::CallPresence,
            RequiredDiscriminator::CallSite {
                target: "repository.save".to_string(),
            },
            ExpectedSink::SideEffect,
        );
        let json = render_agent_seam_packets_json(&[
            classified_with(side_effect, SeamGripClass::Ungripped, Vec::new()),
            classified_with(call_presence, SeamGripClass::Ungripped, Vec::new()),
        ]);
        for needle in [
            "\"name\": \"charge_customer_side_effect_observer\"",
            "\"value\": \"input that produces payment event\"",
            "\"assertion_shape\": {\"kind\": \"side_effect_observer\"",
            "\"name\": \"sync_invoice_call_presence_observer\"",
            "\"value\": \"input that reaches call repository.save\"",
            "\"assertion_shape\": {\"kind\": \"call_expectation\"",
            "\"pattern\": \"copying a smoke-only test shape\"",
        ] {
            if !json.contains(needle) {
                return Err(format!(
                    "missing effect/call guidance {needle:?} in: {json}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn packet_v2_reports_medium_and_unknown_confidence_cases() -> Result<(), String> {
        let medium_related = related_test_with(
            "helper_test_observes_output",
            OracleKind::RelationalCheck,
            OracleStrength::Medium,
            crate::analysis::test_grip_evidence::RelationConfidence::Medium,
        );
        let mut opaque = weakly_gripped_classified();
        opaque.class = SeamGripClass::Opaque;
        let json = render_agent_seam_packets_json(&[
            classified_with(
                seam_with(
                    "math::score",
                    SeamKind::ReturnValue,
                    RequiredDiscriminator::ReturnValue {
                        description: "score".to_string(),
                    },
                    ExpectedSink::ReturnValue,
                ),
                SeamGripClass::WeaklyGripped,
                vec![medium_related],
            ),
            opaque,
        ]);
        for needle in [
            "\"confidence\": \"medium\"",
            "\"reason\": \"medium relational_check oracle with medium relation\"",
            "\"task\": \"inspect_static_limitation\"",
            "\"confidence\": \"unknown\"",
        ] {
            if !json.contains(needle) {
                return Err(format!("missing confidence case {needle:?} in: {json}"));
            }
        }
        Ok(())
    }

    #[test]
    fn suggested_assertion_helper_keeps_setup_assertions_but_omits_comment_guidance()
    -> Result<(), String> {
        let field = classified_with(
            seam_with(
                "pricing::build_quote",
                SeamKind::FieldConstruction,
                RequiredDiscriminator::FieldValue {
                    field: "quote.total".to_string(),
                },
                ExpectedSink::OutputField,
            ),
            SeamGripClass::WeaklyGripped,
            Vec::new(),
        );
        let opaque_field = classified_with(
            seam_with(
                "pricing::build_quote",
                SeamKind::FieldConstruction,
                RequiredDiscriminator::FieldValue {
                    field: "quote.total".to_string(),
                },
                ExpectedSink::OutputField,
            ),
            SeamGripClass::Opaque,
            Vec::new(),
        );
        let side_effect = classified_with(
            seam_with(
                "service::publish_event",
                SeamKind::SideEffect,
                RequiredDiscriminator::Effect {
                    sink: "event bus publish".to_string(),
                },
                ExpectedSink::SideEffect,
            ),
            SeamGripClass::WeaklyGripped,
            Vec::new(),
        );

        let Some(assertion) = suggested_assertion_for_classified_seam(&field) else {
            return Err("expected field construction assertion".to_string());
        };
        assert!(
            assertion.contains("assert_eq!(result.field"),
            "unexpected field assertion: {assertion}"
        );
        assert!(
            suggested_assertion_for_classified_seam(&opaque_field).is_some(),
            "opaque packet with concrete assertion guidance should expose the same assertion action"
        );
        assert!(suggested_assertion_for_classified_seam(&side_effect).is_none());
        Ok(())
    }

    #[test]
    fn schema_version_is_pinned_to_zero_three() {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        assert!(
            json.contains("\"schema_version\": \"0.3\""),
            "expected schema_version 0.3: {json}"
        );
    }

    #[test]
    fn empty_input_emits_well_formed_json() {
        let json = render_agent_seam_packets_json(&[]);
        assert!(json.contains("\"packets_total\": 0"));
        assert!(json.contains("\"packets\": []"));
        assert!(json.contains("\"schema_version\": \"0.3\""));
    }

    #[test]
    fn suggested_assertion_for_predicate_boundary_uses_owner_and_missing_value() {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        // owner short name is `discounted_total`.
        assert!(
            json.contains(
                "assert_eq!(discounted_total(/* discount_threshold (equality boundary) */"
            ),
            "expected templated assert_eq! suggestion: {json}"
        );
    }
}
