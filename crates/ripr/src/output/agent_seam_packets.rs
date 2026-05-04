//! Render Voice B classified seam gaps as agent-ready packets per
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
//! The packet schema is **0.2**, intentionally distinct from the
//! repo-exposure report's 0.1, because the packet is a separate
//! contract aimed at coding agents rather than reviewers.

use crate::analysis::ClassifiedSeam;
use crate::analysis::seams::{ExpectedSink, RequiredDiscriminator, SeamGripClass, SeamKind};
use crate::analysis::test_grip_evidence::TestGripEvidence;
use crate::output::json::escape as json_escape;

pub(crate) const AGENT_SEAM_PACKET_SCHEMA_VERSION: &str = "0.2";

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

    out.push_str(&format!(
        "      \"missing_oracle_shape\": \"{}\",\n",
        json_escape(&missing_oracle_shape_for(seam.kind(), seam.expected_sink()))
    ));

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
                "\"evidence_summary\": \"{}\"",
                json_escape(grip.evidence_summary.as_str())
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
        "      \"runtime_confirmation\": \"{}\"\n",
        json_escape(RUNTIME_CONFIRMATION_NOTE)
    ));
    out.push_str("    }");
}

/// A flat (value, reason) record carried in the packet's
/// `missing_discriminators` array. Mirrors the field shape of
/// `MissingDiscriminatorFact` but excludes `flow_sink` because the
/// packet already carries the sink class via `missing_oracle_shape`.
struct MissingRecord {
    value: String,
    reason: String,
}

/// Build the `missing_discriminators` array carried in the packet,
/// pairing analyzer-emitted hypotheses with a predicate-boundary
/// fallback when the seam expression names a clear boundary.
fn missing_discriminator_records_for(entry: &ClassifiedSeam) -> Vec<MissingRecord> {
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
    fn schema_version_is_pinned_to_zero_two() {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        assert!(
            json.contains("\"schema_version\": \"0.2\""),
            "expected schema_version 0.2: {json}"
        );
    }

    #[test]
    fn empty_input_emits_well_formed_json() {
        let json = render_agent_seam_packets_json(&[]);
        assert!(json.contains("\"packets_total\": 0"));
        assert!(json.contains("\"packets\": []"));
        assert!(json.contains("\"schema_version\": \"0.2\""));
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
