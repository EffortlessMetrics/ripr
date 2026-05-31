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
use crate::output::path::display_path;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

pub(crate) const REPO_EXPOSURE_SCHEMA_VERSION: &str = "0.3";
pub(crate) const REPO_EXPOSURE_SUMMARY_SCHEMA_VERSION: &str = "0.1";

/// Cap on related-tests rendered per seam in the JSON output. The
/// existing `test_grip_evidence::find_related_tests` heuristic is
/// permissive (any test whose file or name shares the seam owner
/// counts), so a single high-traffic file (e.g. `classifier.rs`) can
/// fan out to hundreds of tests per seam. The cap keeps the artifact
/// review-sized; tightening the heuristic itself is a future PR.
const MAX_RELATED_TESTS_PER_SEAM_JSON: usize = 8;
const MAX_TOP_FILES_SUMMARY_JSON: usize = 25;

/// Render the repo exposure JSON.
pub(crate) fn render_repo_exposure_json(classified: &[ClassifiedSeam]) -> String {
    let mut bytes = Vec::new();
    if write_repo_exposure_json(classified, &mut bytes).is_err() {
        return String::new();
    }
    match String::from_utf8(bytes) {
        Ok(json) => json,
        Err(err) => String::from_utf8_lossy(&err.into_bytes()).into_owned(),
    }
}

/// Stream the repo exposure JSON without materializing the whole artifact.
///
/// Repo exposure can be multi-gigabyte on dogfood-sized workspaces because
/// each seam carries its evidence record. CLI callers use this writer path so
/// the JSON schema stays unchanged while memory pressure scales with one seam
/// record rather than the full artifact.
pub(crate) fn write_repo_exposure_json<W: io::Write>(
    classified: &[ClassifiedSeam],
    out: &mut W,
) -> io::Result<()> {
    let metrics = ExposureMetrics::from(classified);
    let canonical_gaps = canonical_gap_identities(classified);

    writeln!(out, "{{")?;
    writeln!(
        out,
        "  \"schema_version\": \"{}\",",
        REPO_EXPOSURE_SCHEMA_VERSION
    )?;
    writeln!(out, "  \"scope\": \"repo\",")?;

    writeln!(out, "  \"metrics\": {{")?;
    writeln!(out, "    \"seams_total\": {},", metrics.seams_total)?;
    writeln!(
        out,
        "    \"headline_eligible\": {},",
        metrics.headline_eligible
    )?;
    let total_classes = SeamGripClass::ALL.len();
    for (idx, class) in SeamGripClass::ALL.iter().enumerate() {
        let count = metrics.count_for(*class);
        let trailing = if idx + 1 == total_classes { "" } else { "," };
        writeln!(out, "    \"{}\": {}{}", class.as_str(), count, trailing)?;
    }
    writeln!(out, "  }},")?;

    write!(out, "  \"seams\": [")?;
    for (idx, entry) in classified.iter().enumerate() {
        if idx == 0 {
            writeln!(out)?;
        }
        let mut seam_json = String::new();
        push_classified_json(&mut seam_json, entry, canonical_gaps.get(entry.seam.id()));
        out.write_all(seam_json.as_bytes())?;
        if idx + 1 != classified.len() {
            writeln!(out, ",")?;
        } else {
            writeln!(out)?;
        }
    }
    if !classified.is_empty() {
        write!(out, "  ")?;
    }
    writeln!(out, "]")?;
    writeln!(out, "}}")?;
    out.flush()
}

/// Render a bounded repo exposure summary JSON.
///
/// Unlike `repo-exposure-json`, this shape deliberately omits per-seam
/// evidence arrays and nested evidence records. It is for badge, queue, and
/// large-repo planning consumers that need aggregate counts and bounded top
/// files without multi-GB payloads.
pub(crate) fn render_repo_exposure_summary_json(
    classified: &[ClassifiedSeam],
    root: &Path,
    base: Option<&str>,
    mode: &str,
) -> String {
    let value = repo_exposure_summary_json_value(classified, root, base, mode);
    match serde_json::to_string_pretty(&value) {
        Ok(mut json) => {
            json.push('\n');
            json
        }
        Err(_) => "{}\n".to_string(),
    }
}

fn repo_exposure_summary_json_value(
    classified: &[ClassifiedSeam],
    root: &Path,
    base: Option<&str>,
    mode: &str,
) -> Value {
    let metrics = ExposureMetrics::from(classified);
    let canonical_gaps = canonical_gap_identities(classified);
    let mut canonical_gap_ids = BTreeSet::<String>::new();
    let mut actionable_gap_ids = BTreeSet::<String>::new();
    let mut raw_actionable_seam_records = 0usize;

    let mut actionability_counts = BTreeMap::<String, usize>::new();
    let mut actionable_seam_kind_counts = BTreeMap::<String, usize>::new();
    let mut gap_state_counts = BTreeMap::<String, usize>::new();
    let mut file_summaries = BTreeMap::<String, FileExposureSummary>::new();

    for entry in classified {
        let file = display_path(entry.seam.file());
        let file_summary = file_summaries.entry(file).or_default();
        file_summary.raw_seams += 1;
        increment(&mut file_summary.grip_class_counts, entry.class.as_str());
        if entry.class.is_headline_eligible() {
            file_summary.headline_eligible_seams += 1;
        }
        if entry.class == SeamGripClass::Suppressed {
            file_summary.suppressed_exposure_gaps += 1;
        }

        let canonical_gap = canonical_gaps.get(entry.seam.id());
        if let Some(gap) = canonical_gap {
            canonical_gap_ids.insert(gap.id.clone());
            file_summary.canonical_gap_ids.insert(gap.id.clone());
        }

        let record = evidence_record_for(entry, canonical_gap);
        increment(
            &mut gap_state_counts,
            record.canonical_item.gap_state.as_str(),
        );

        if let Some(gap_id) = record.canonical_item.canonical_gap_id.as_ref()
            && is_summary_actionable_canonical_item(&record.canonical_item)
        {
            raw_actionable_seam_records += 1;
            if actionable_gap_ids.insert(gap_id.clone()) {
                increment(
                    &mut actionability_counts,
                    record.canonical_item.actionability.as_str(),
                );
                increment(&mut actionable_seam_kind_counts, entry.seam.kind().as_str());
            }
            if file_summary.actionable_gap_ids.insert(gap_id.clone()) {
                increment(
                    &mut file_summary.actionability_counts,
                    record.canonical_item.actionability.as_str(),
                );
            }
        }
    }

    let grip_class_counts = grip_class_counts_json(&metrics);
    let top_files_total = file_summaries.len();
    let mut top_files = file_summaries.into_iter().collect::<Vec<_>>();
    top_files.sort_by(|(file_a, a), (file_b, b)| {
        b.actionable_gap_ids
            .len()
            .cmp(&a.actionable_gap_ids.len())
            .then(b.headline_eligible_seams.cmp(&a.headline_eligible_seams))
            .then(b.raw_seams.cmp(&a.raw_seams))
            .then(file_a.cmp(file_b))
    });
    let top_files_json = top_files
        .iter()
        .take(MAX_TOP_FILES_SUMMARY_JSON)
        .map(|(file, summary)| {
            json!({
                "file": file,
                "raw_seams": summary.raw_seams,
                "headline_eligible_seams": summary.headline_eligible_seams,
                "canonical_gap_records": summary.canonical_gap_ids.len(),
                "unsuppressed_exposure_gaps": summary.actionable_gap_ids.len(),
                "suppressed_exposure_gaps": summary.suppressed_exposure_gaps,
                "reason_breakdown": {
                    "actionability": summary.actionability_counts,
                    "grip_class": summary.grip_class_counts,
                }
            })
        })
        .collect::<Vec<_>>();

    json!({
        "schema_version": REPO_EXPOSURE_SUMMARY_SCHEMA_VERSION,
        "format": "repo-exposure-summary-json",
        "tool": "ripr",
        "ripr_version": env!("CARGO_PKG_VERSION"),
        "scope": "repo",
        "basis": "canonical_actionable_gap",
        "metadata": {
            "root": display_path(root),
            "base": base,
            "head": "HEAD",
            "mode": mode,
        },
        "metrics": {
            "raw_seams": metrics.seams_total,
            "headline_eligible_seams": metrics.headline_eligible,
            "canonical_gap_records": canonical_gap_ids.len(),
            "raw_actionable_seam_records": raw_actionable_seam_records,
            "unsuppressed_exposure_gaps": actionable_gap_ids.len(),
            "suppressed_exposure_gaps": metrics.count_for(SeamGripClass::Suppressed),
            "grip_class": grip_class_counts.clone(),
        },
        "reason_breakdown": {
            "actionability": actionability_counts,
            "gap_state": gap_state_counts,
            "seam_kind": actionable_seam_kind_counts,
            "grip_class": grip_class_counts,
        },
        "limits": {
            "top_files_limit": MAX_TOP_FILES_SUMMARY_JSON,
            "top_files_total": top_files_total,
            "top_files_truncated": top_files_total > MAX_TOP_FILES_SUMMARY_JSON,
        },
        "top_files": top_files_json,
    })
}

fn is_summary_actionable_canonical_item(
    item: &crate::output::evidence_record::EvidenceRecordCanonicalItem,
) -> bool {
    item.gap_state == "actionable" && item.repair_route.is_some() && item.verify_command.is_some()
}

#[derive(Default)]
struct FileExposureSummary {
    raw_seams: usize,
    headline_eligible_seams: usize,
    suppressed_exposure_gaps: usize,
    canonical_gap_ids: BTreeSet<String>,
    actionable_gap_ids: BTreeSet<String>,
    actionability_counts: BTreeMap<String, usize>,
    grip_class_counts: BTreeMap<String, usize>,
}

fn grip_class_counts_json(metrics: &ExposureMetrics) -> BTreeMap<String, usize> {
    SeamGripClass::ALL
        .into_iter()
        .map(|class| (class.as_str().to_string(), metrics.count_for(class)))
        .collect()
}

fn increment(counts: &mut BTreeMap<String, usize>, key: &str) {
    *counts.entry(key.to_string()).or_insert(0) += 1;
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
        classified_at(
            "src/pricing.rs",
            "pricing::discounted_total",
            42,
            SeamGripClass::WeaklyGripped,
        )
    }

    fn classified_at(file: &str, owner: &str, line: usize, class: SeamGripClass) -> ClassifiedSeam {
        let seam = RepoSeam::new(
            file,
            owner,
            SeamKind::PredicateBoundary,
            line * 10,
            line,
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
            class,
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
            "\"raw_findings\":[",
            "\"canonical_item\":",
            "\"gap_state\":\"actionable\"",
            "\"actionability\":\"extend_related_test\"",
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
    fn summary_json_contains_counts_and_omits_per_seam_payloads() -> Result<(), String> {
        let summary = render_repo_exposure_summary_json(
            &[weakly_gripped_classified()],
            std::path::Path::new("."),
            Some("origin/main"),
            "draft",
        );
        let value: serde_json::Value = serde_json::from_str(&summary)
            .map_err(|err| format!("parse summary JSON failed: {err}\n{summary}"))?;

        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["format"], "repo-exposure-summary-json");
        assert_eq!(value["scope"], "repo");
        assert_eq!(value["basis"], "canonical_actionable_gap");
        assert_eq!(value["metadata"]["base"], "origin/main");
        assert_eq!(value["metadata"]["head"], "HEAD");
        assert_eq!(value["metrics"]["raw_seams"], 1);
        assert_eq!(value["metrics"]["headline_eligible_seams"], 1);
        assert_eq!(value["metrics"]["canonical_gap_records"], 1);
        assert_eq!(value["metrics"]["unsuppressed_exposure_gaps"], 1);
        assert_eq!(
            value["reason_breakdown"]["actionability"]["extend_related_test"],
            1
        );
        assert_eq!(value["reason_breakdown"]["grip_class"]["weakly_gripped"], 1);
        let top_files = value["top_files"]
            .as_array()
            .ok_or_else(|| format!("top_files is not an array: {value}"))?;
        assert_eq!(top_files.len(), 1);
        assert_eq!(top_files[0]["file"], "src/pricing.rs");
        assert_eq!(top_files[0]["unsuppressed_exposure_gaps"], 1);

        assert!(value.get("seams").is_none());
        for forbidden in [
            "\"evidence_record\"",
            "\"related_tests\"",
            "\"observed_values\"",
            "\"missing_discriminators\"",
        ] {
            assert!(
                !summary.contains(forbidden),
                "summary should omit {forbidden}: {summary}"
            );
        }
        Ok(())
    }

    #[test]
    fn summary_json_bounds_top_files() -> Result<(), String> {
        let mut classified = Vec::new();
        for idx in 0..30 {
            classified.push(classified_at(
                &format!("src/file_{idx}.rs"),
                &format!("pricing::owner_{idx}"),
                idx + 1,
                SeamGripClass::WeaklyGripped,
            ));
        }

        let summary = render_repo_exposure_summary_json(
            &classified,
            std::path::Path::new("."),
            None,
            "ready",
        );
        let value: serde_json::Value = serde_json::from_str(&summary)
            .map_err(|err| format!("parse summary JSON failed: {err}\n{summary}"))?;
        let top_files = value["top_files"]
            .as_array()
            .ok_or_else(|| format!("top_files is not an array: {value}"))?;

        assert_eq!(top_files.len(), 25);
        assert_eq!(value["limits"]["top_files_limit"], 25);
        assert_eq!(value["limits"]["top_files_total"], 30);
        assert_eq!(value["limits"]["top_files_truncated"], true);
        Ok(())
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

    #[test]
    fn given_repo_exposure_related_tests_when_helper_owner_call_then_additive_reason_is_emitted() {
        let mut classified = weakly_gripped_classified();
        classified.evidence.related_tests[0].relation_reason =
            crate::analysis::test_grip_evidence::RelationReason::HelperOwnerCall;

        let json = render_repo_exposure_json(&[classified.clone()]);
        assert!(
            json.contains("\"relation_reason\": \"helper_owner_call\""),
            "JSON missing helper_owner_call relation_reason: {json}"
        );

        let md = render_repo_exposure_md(&[classified]);
        assert!(
            md.contains("helper_owner_call"),
            "Markdown missing helper_owner_call tag: {md}"
        );
    }
}
