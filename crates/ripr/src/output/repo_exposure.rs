//! Render the classified seam inventory as a repo exposure report
//! (JSON + Markdown).
//!
//! Schema is documented in `docs/OUTPUT_SCHEMA.md` under
//! `repo-exposure.json`. The schema version pin is
//! `REPO_EXPOSURE_SCHEMA_VERSION`; bumping it requires updating the
//! doc and any downstream consumers in lockstep.

use crate::analysis::ClassifiedSeam;
use crate::analysis::canonical_gap::{CanonicalGapIdentity, canonical_gap_identities};
use crate::analysis::seams::SeamGripClass;
use crate::output::evidence_record::{evidence_record_for, evidence_record_json_value};
use crate::output::json::escape as json_escape;

pub(crate) const REPO_EXPOSURE_SCHEMA_VERSION: &str = "0.3";

/// Cap on related-tests rendered per seam in the JSON output. The
/// existing `test_grip_evidence::find_related_tests` heuristic is
/// permissive (any test whose file or name shares the seam owner
/// counts), so a single high-traffic file (e.g. `classifier.rs`) can
/// fan out to hundreds of tests per seam. The cap keeps the artifact
/// review-sized; tightening the heuristic itself is a future PR.
const MAX_RELATED_TESTS_PER_SEAM_JSON: usize = 8;

/// Render the repo exposure JSON.
pub(crate) fn render_repo_exposure_json(classified: &[ClassifiedSeam]) -> String {
    let metrics = ExposureMetrics::from(classified);
    let canonical_gaps = canonical_gap_identities(classified);

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": \"{}\",\n",
        REPO_EXPOSURE_SCHEMA_VERSION
    ));
    out.push_str("  \"scope\": \"repo\",\n");

    out.push_str("  \"metrics\": {\n");
    out.push_str(&format!("    \"seams_total\": {},\n", metrics.seams_total));
    out.push_str(&format!(
        "    \"headline_eligible\": {},\n",
        metrics.headline_eligible
    ));
    let total_classes = SeamGripClass::ALL.len();
    for (idx, class) in SeamGripClass::ALL.iter().enumerate() {
        let count = metrics.count_for(*class);
        let trailing = if idx + 1 == total_classes { "" } else { "," };
        out.push_str(&format!(
            "    \"{}\": {}{}\n",
            class.as_str(),
            count,
            trailing
        ));
    }
    out.push_str("  },\n");

    out.push_str("  \"seams\": [");
    for (idx, entry) in classified.iter().enumerate() {
        if idx == 0 {
            out.push('\n');
        }
        push_classified_json(&mut out, entry, canonical_gaps.get(entry.seam.id()));
        if idx + 1 != classified.len() {
            out.push_str(",\n");
        } else {
            out.push('\n');
        }
    }
    if !classified.is_empty() {
        out.push_str("  ");
    }
    out.push_str("]\n");
    out.push_str("}\n");
    out
}

fn push_classified_json(
    out: &mut String,
    entry: &ClassifiedSeam,
    canonical_gap: Option<&CanonicalGapIdentity>,
) {
    let seam = &entry.seam;
    let evidence = &entry.evidence;
    out.push_str("    {\n");
    out.push_str(&format!(
        "      \"seam_id\": \"{}\",\n",
        json_escape(seam.id().as_str())
    ));
    out.push_str(&format!("      \"kind\": \"{}\",\n", seam.kind().as_str()));
    out.push_str(&format!(
        "      \"file\": \"{}\",\n",
        json_escape(&seam.file().to_string_lossy())
    ));
    out.push_str(&format!("      \"line\": {},\n", seam.display_line()));
    out.push_str(&format!(
        "      \"owner\": \"{}\",\n",
        json_escape(seam.owner())
    ));
    out.push_str(&format!(
        "      \"expression\": \"{}\",\n",
        json_escape(seam.expression())
    ));
    out.push_str(&format!(
        "      \"grip_class\": \"{}\",\n",
        entry.class.as_str()
    ));
    out.push_str(&format!(
        "      \"headline_eligible\": {},\n",
        entry.class.is_headline_eligible()
    ));

    out.push_str("      \"evidence\": {\n");
    out.push_str(&format!(
        "        \"reach\": \"{}\",\n",
        evidence.reach.state.as_str()
    ));
    out.push_str(&format!(
        "        \"activate\": \"{}\",\n",
        evidence.activate.state.as_str()
    ));
    out.push_str(&format!(
        "        \"propagate\": \"{}\",\n",
        evidence.propagate.state.as_str()
    ));
    out.push_str(&format!(
        "        \"observe\": \"{}\",\n",
        evidence.observe.state.as_str()
    ));
    out.push_str(&format!(
        "        \"discriminate\": \"{}\"\n",
        evidence.discriminate.state.as_str()
    ));
    out.push_str("      },\n");

    let related_total = evidence.related_tests.len();
    let related_rendered = related_total.min(MAX_RELATED_TESTS_PER_SEAM_JSON);
    out.push_str(&format!(
        "      \"related_tests_total\": {related_total},\n"
    ));
    out.push_str("      \"related_tests\": [");
    if related_rendered > 0 {
        out.push('\n');
        for (idx, grip) in evidence
            .related_tests
            .iter()
            .take(related_rendered)
            .enumerate()
        {
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
            if idx + 1 != related_rendered {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("      ");
    }
    out.push_str("],\n");

    out.push_str("      \"observed_values\": [");
    for (idx, value) in evidence.observed_values.iter().enumerate() {
        out.push_str(&format!("\"{}\"", json_escape(value.value.as_str())));
        if idx + 1 != evidence.observed_values.len() {
            out.push_str(", ");
        }
    }
    out.push_str("],\n");

    out.push_str("      \"missing_discriminators\": [");
    if !evidence.missing_discriminators.is_empty() {
        out.push('\n');
        for (idx, missing) in evidence.missing_discriminators.iter().enumerate() {
            out.push_str(&format!(
                "        {{\"value\": \"{}\", \"reason\": \"{}\"}}",
                json_escape(missing.value.as_str()),
                json_escape(missing.reason.as_str())
            ));
            if idx + 1 != evidence.missing_discriminators.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("      ");
    }
    out.push_str("],\n");
    let record = evidence_record_for(entry, canonical_gap);
    out.push_str("      \"evidence_record\": ");
    out.push_str(&evidence_record_json_value(&record).to_string());
    out.push('\n');
    out.push_str("    }");
}

/// Render the repo exposure Markdown report. The output uses the
/// static seam evidence vocabulary only — no runtime-mutation outcome
/// words per RIPR-SPEC-0005 § Static-Language Boundaries.
pub(crate) fn render_repo_exposure_md(classified: &[ClassifiedSeam]) -> String {
    let metrics = ExposureMetrics::from(classified);
    let mut out = String::new();
    out.push_str("# ripr repo exposure report\n\n");
    out.push_str(&format!(
        "Schema version: {}\n",
        REPO_EXPOSURE_SCHEMA_VERSION
    ));
    out.push_str("Scope: repo\n\n");

    out.push_str("## Summary\n\n");
    out.push_str("| Class | Count |\n| --- | --- |\n");
    out.push_str(&format!("| seams_total | {} |\n", metrics.seams_total));
    out.push_str(&format!(
        "| headline_eligible | {} |\n",
        metrics.headline_eligible
    ));
    for class in SeamGripClass::ALL {
        out.push_str(&format!(
            "| {} | {} |\n",
            class.as_str(),
            metrics.count_for(class)
        ));
    }

    if classified.is_empty() {
        out.push_str(
            "\nNo classified seams. The repo seam inventory is empty or no \
             production seams were detected.\n",
        );
        return out;
    }

    out.push_str("\n## Top gaps\n\n");
    let mut top_gaps: Vec<&ClassifiedSeam> = classified
        .iter()
        .filter(|entry| entry.class.is_headline_eligible())
        .collect();
    top_gaps.sort_by(|a, b| {
        a.seam
            .file()
            .cmp(b.seam.file())
            .then(a.seam.display_line().cmp(&b.seam.display_line()))
            .then(a.seam.id().as_str().cmp(b.seam.id().as_str()))
    });
    if top_gaps.is_empty() {
        out.push_str(
            "No headline-eligible seams. Static seam evidence reports no \
             detected grip gaps at the moment; runtime confirmation via \
             `cargo-mutants` remains a separate calibration step.\n",
        );
        return out;
    }
    let preview = top_gaps.iter().take(50);
    for entry in preview {
        push_top_gap_md(&mut out, entry);
    }
    if top_gaps.len() > 50 {
        out.push_str(&format!(
            "\n_... {} additional headline-eligible seams omitted; see \
            `repo-exposure.json` for the full list._\n",
            top_gaps.len() - 50
        ));
    }

    out.push_str(
        "\n_This report shows static test-grip evidence for repo seams. \
        Runtime confirmation (e.g. `cargo-mutants`) is a separate \
        calibration step. Static-language constraints from RIPR-SPEC-0005 \
        still apply._\n",
    );
    out
}

fn push_top_gap_md(out: &mut String, entry: &ClassifiedSeam) {
    let seam = &entry.seam;
    let evidence = &entry.evidence;
    out.push_str(&format!(
        "### {}:{} {}\n\n",
        md_escape(&seam.file().to_string_lossy()),
        seam.display_line(),
        seam.kind().as_str()
    ));
    out.push_str(&format!("- seam: `{}`\n", md_escape(seam.expression())));
    out.push_str(&format!("- owner: `{}`\n", md_escape(seam.owner())));
    out.push_str(&format!("- grip: {}\n", entry.class.as_str()));
    out.push_str("- evidence:\n");
    out.push_str(&format!("  - reach: {}\n", evidence.reach.state.as_str()));
    out.push_str(&format!(
        "  - activate: {}\n",
        evidence.activate.state.as_str()
    ));
    out.push_str(&format!(
        "  - propagate: {}\n",
        evidence.propagate.state.as_str()
    ));
    out.push_str(&format!(
        "  - observe: {}\n",
        evidence.observe.state.as_str()
    ));
    out.push_str(&format!(
        "  - discriminate: {}\n",
        evidence.discriminate.state.as_str()
    ));

    if !evidence.related_tests.is_empty() {
        out.push_str("- related tests:\n");
        for grip in evidence.related_tests.iter().take(5) {
            out.push_str(&format!(
                "  - `{}` ({}, {}) · {} / {}\n",
                md_escape(grip.test_name.as_str()),
                grip.oracle_kind.as_str(),
                grip.oracle_strength.as_str(),
                grip.relation_reason.as_str(),
                grip.relation_confidence.as_str()
            ));
        }
    }
    if !evidence.observed_values.is_empty() {
        out.push_str("- observed values:\n");
        for value in evidence.observed_values.iter().take(5) {
            out.push_str(&format!("  - `{}`\n", md_escape(value.value.as_str())));
        }
    }
    if !evidence.missing_discriminators.is_empty() {
        out.push_str("- missing discriminators:\n");
        for missing in &evidence.missing_discriminators {
            out.push_str(&format!(
                "  - `{}` — {}\n",
                md_escape(missing.value.as_str()),
                md_escape_paragraph(missing.reason.as_str())
            ));
        }
    }
    out.push('\n');
}

/// Escape values that get wrapped in inline-code spans. Inside
/// backticks every character is literal except the closing backtick
/// and the table-cell pipe, so we only swap those plus newlines.
/// Backslash-escaping `*`/`_`/`[`/`]` here would render as literal
/// `\*` in the inline-code span — see `md_escape_paragraph` for the
/// non-code variant.
fn md_escape(value: &str) -> String {
    value
        .replace('`', "\u{2018}")
        .replace('|', "\\|")
        .replace('\n', " ")
}

/// Escape values that appear in paragraph text (no surrounding
/// backticks). Adds backslash escapes for emphasis and link tokens so
/// a future analyzer-emitted reason string containing snake_case or
/// `*` does not silently trigger italic/bold/link rendering.
fn md_escape_paragraph(value: &str) -> String {
    md_escape(value)
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

/// Per-class metric bucket for the repo exposure report.
struct ExposureMetrics {
    seams_total: usize,
    headline_eligible: usize,
    counts: [(SeamGripClass, usize); 11],
}

impl ExposureMetrics {
    fn from(classified: &[ClassifiedSeam]) -> Self {
        let mut counts: [(SeamGripClass, usize); 11] = [
            (SeamGripClass::StronglyGripped, 0),
            (SeamGripClass::WeaklyGripped, 0),
            (SeamGripClass::Ungripped, 0),
            (SeamGripClass::ReachableUnrevealed, 0),
            (SeamGripClass::ActivationUnknown, 0),
            (SeamGripClass::PropagationUnknown, 0),
            (SeamGripClass::ObservationUnknown, 0),
            (SeamGripClass::DiscriminationUnknown, 0),
            (SeamGripClass::Opaque, 0),
            (SeamGripClass::Intentional, 0),
            (SeamGripClass::Suppressed, 0),
        ];
        let mut headline_eligible = 0;
        for entry in classified {
            for bucket in counts.iter_mut() {
                if bucket.0 == entry.class {
                    bucket.1 += 1;
                    break;
                }
            }
            if entry.class.is_headline_eligible() {
                headline_eligible += 1;
            }
        }
        ExposureMetrics {
            seams_total: classified.len(),
            headline_eligible,
            counts,
        }
    }

    fn count_for(&self, class: SeamGripClass) -> usize {
        self.counts
            .iter()
            .find(|(c, _)| *c == class)
            .map(|(_, n)| *n)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
    use crate::analysis::test_grip_evidence::{RelatedTestGrip, TestGripEvidence};
    use crate::domain::{
        Confidence, MissingDiscriminatorFact, OracleKind, OracleStrength, StageEvidence,
        StageState, ValueFact,
    };

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, "test stage")
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
                file: std::path::PathBuf::from("tests/pricing_tests.rs"),
                line: 5,
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
                line: 5,
                text: "discounted_total(50, 100)".to_string(),
                value: "50".to_string(),
                context: crate::domain::ValueContext::FunctionArgument,
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

    #[test]
    fn json_carries_schema_version_scope_and_metrics() {
        let json = render_repo_exposure_json(&[weakly_gripped_classified()]);
        for needle in [
            "\"schema_version\": \"0.3\"",
            "\"scope\": \"repo\"",
            "\"seams_total\": 1",
            "\"headline_eligible\": 1",
            "\"weakly_gripped\": 1",
            "\"strongly_gripped\": 0",
        ] {
            assert!(json.contains(needle), "missing {needle:?} in json:\n{json}");
        }
    }

    #[test]
    fn json_carries_full_classified_record() {
        let json = render_repo_exposure_json(&[weakly_gripped_classified()]);
        for needle in [
            "\"seam_id\":",
            "\"kind\": \"predicate_boundary\"",
            "\"grip_class\": \"weakly_gripped\"",
            "\"headline_eligible\": true",
            "\"reach\": \"yes\"",
            "\"discriminate\": \"yes\"",
            "\"evidence_record\":",
            "\"schema_version\":\"0.1\"",
            "\"canonical_gap_id\":\"gap:",
            "\"canonical_gap_group_size\":1",
            "\"canonical_gap_reason\":\"same owner, seam kind, flow sink, missing discriminator, and assertion shape\"",
            "\"evidence_path\":",
            "\"actionable_related_test_extension\"",
            "\"agreement\":\"no_runtime_data\"",
            "below_threshold_has_no_discount",
            "exact_value",
            "discount_threshold (equality boundary)",
        ] {
            assert!(json.contains(needle), "missing {needle:?} in json:\n{json}");
        }
    }

    #[test]
    fn json_emits_empty_seams_array_when_inventory_is_empty() {
        let json = render_repo_exposure_json(&[]);
        assert!(json.contains("\"seams\": []"));
        assert!(json.contains("\"seams_total\": 0"));
    }

    #[test]
    fn markdown_renders_summary_table_and_top_gaps() {
        let md = render_repo_exposure_md(&[weakly_gripped_classified()]);
        assert!(md.contains("# ripr repo exposure report"));
        assert!(md.contains("## Summary"));
        assert!(md.contains("| seams_total | 1 |"));
        assert!(md.contains("| weakly_gripped | 1 |"));
        assert!(md.contains("## Top gaps"));
        assert!(md.contains("predicate_boundary"));
        assert!(md.contains("amount >= discount_threshold"));
        assert!(md.contains("discount_threshold (equality boundary)"));
    }

    #[test]
    fn markdown_explains_when_inventory_is_empty() {
        let md = render_repo_exposure_md(&[]);
        assert!(md.contains("repo seam inventory is empty"));
    }

    #[test]
    fn markdown_explains_when_no_headline_gaps() {
        // A classification record with no headline-eligible class leaves
        // the Top gaps section empty.
        let mut entry = weakly_gripped_classified();
        entry.class = SeamGripClass::StronglyGripped;
        let md = render_repo_exposure_md(&[entry]);
        assert!(md.contains("No headline-eligible seams"));
    }

    #[test]
    fn markdown_uses_static_exposure_vocabulary() {
        // Pin seam evidence framing strings; the repo-wide
        // check-static-language gate enforces forbidden-token absence.
        let md = render_repo_exposure_md(&[weakly_gripped_classified()]);
        assert!(md.contains("ripr repo exposure report"));
        assert!(md.contains("Runtime confirmation"));
        assert!(md.contains("cargo-mutants"));
    }

    #[test]
    fn given_repo_exposure_related_tests_when_rendered_then_relation_reason_and_confidence_are_present()
     {
        // Both JSON and Markdown emit the relation_reason +
        // relation_confidence fields per related test. Pinned by
        // schema bump 0.1 → 0.2.
        let json = render_repo_exposure_json(&[weakly_gripped_classified()]);
        assert!(
            json.contains("\"relation_reason\": \"direct_owner_call\""),
            "JSON missing relation_reason: {json}"
        );
        assert!(
            json.contains("\"relation_confidence\": \"high\""),
            "JSON missing relation_confidence: {json}"
        );

        let md = render_repo_exposure_md(&[weakly_gripped_classified()]);
        assert!(
            md.contains("direct_owner_call"),
            "Markdown missing direct_owner_call tag: {md}"
        );
        assert!(md.contains("high"), "Markdown missing confidence tag: {md}");
    }
}
