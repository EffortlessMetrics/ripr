use serde::Serialize;

use crate::app::CheckOutput;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct DiffChangedFile {
    pub(crate) path: String,
    pub(crate) added_lines: Vec<usize>,
    pub(crate) removed_lines: Vec<usize>,
    pub(crate) added_count: usize,
    pub(crate) removed_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct DiffChangedSeam {
    pub(crate) seam_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) canonical_gap_id: Option<String>,
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) classification: String,
    pub(crate) evidence: Vec<String>,
    pub(crate) missing_discriminators: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recommended_next_step: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct DiffPhaseStatus {
    pub(crate) state: String,
    pub(crate) phase: String,
    pub(crate) changed_files: usize,
    pub(crate) changed_seams: usize,
    pub(crate) downstream_consumable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct FullRepoContextStatus {
    pub(crate) state: String,
    pub(crate) phase: String,
    pub(crate) limitation_category: String,
    pub(crate) downstream_consumable: bool,
    pub(crate) repair_route: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct DiffRuntimeStatus {
    pub(crate) state: String,
    pub(crate) diff: DiffPhaseStatus,
    pub(crate) full_repo_context: FullRepoContextStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct DiffReceiptStatus {
    pub(crate) state: String,
    pub(crate) path: String,
    pub(crate) outcome_hint: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct DiffSummary {
    pub(crate) changed_files: usize,
    pub(crate) changed_seams: usize,
    pub(crate) probes: usize,
    pub(crate) exposed: usize,
    pub(crate) weakly_exposed: usize,
    pub(crate) reachable_unrevealed: usize,
    pub(crate) no_static_path: usize,
    pub(crate) unknown: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct DiffReport {
    pub(crate) schema_version: String,
    pub(crate) kind: String,
    pub(crate) tool: String,
    pub(crate) run_status: String,
    pub(crate) root: String,
    pub(crate) base: String,
    pub(crate) head: String,
    pub(crate) mode: String,
    pub(crate) runtime_status: DiffRuntimeStatus,
    pub(crate) receipt: DiffReceiptStatus,
    pub(crate) summary: DiffSummary,
    pub(crate) changed_files: Vec<DiffChangedFile>,
    pub(crate) changed_seams: Vec<DiffChangedSeam>,
}

pub(crate) fn build_diff_report(
    output: &CheckOutput,
    base: &str,
    head: &str,
    changed_files: Vec<DiffChangedFile>,
    receipt_path: String,
) -> DiffReport {
    let changed_seams = output
        .findings
        .iter()
        .map(|finding| DiffChangedSeam {
            seam_id: finding.id.clone(),
            canonical_gap_id: finding
                .canonical_gap
                .as_ref()
                .map(|canonical| canonical.id.clone()),
            file: finding.probe.location.file.display().to_string(),
            line: finding.probe.location.line,
            classification: finding.class.as_str().to_string(),
            evidence: finding.evidence.clone(),
            missing_discriminators: finding.missing.clone(),
            recommended_next_step: finding.recommended_next_step.clone(),
        })
        .collect::<Vec<_>>();
    let changed_file_count = changed_files.len();
    let changed_seam_count = changed_seams.len();
    let unknown = output.summary.static_unknown
        + output.summary.infection_unknown
        + output.summary.propagation_unknown;

    DiffReport {
        schema_version: "0.1".to_string(),
        kind: "ripr_diff".to_string(),
        tool: output.tool.clone(),
        run_status: "diff_complete_full_repo_limited".to_string(),
        root: output.root.display().to_string(),
        base: base.to_string(),
        head: head.to_string(),
        mode: output.mode.as_str().to_string(),
        runtime_status: DiffRuntimeStatus {
            state: "diff_complete_full_repo_limited".to_string(),
            diff: DiffPhaseStatus {
                state: "diff_complete".to_string(),
                phase: "changed_surface_diff".to_string(),
                changed_files: changed_file_count,
                changed_seams: changed_seam_count,
                downstream_consumable: true,
            },
            full_repo_context: FullRepoContextStatus {
                state: "full_repo_limited".to_string(),
                phase: "full_repo_context".to_string(),
                limitation_category: "full_repo_context_not_run".to_string(),
                downstream_consumable: false,
                repair_route: "ripr check --format repo-exposure-summary-json".to_string(),
            },
        },
        receipt: DiffReceiptStatus {
            state: "not_written".to_string(),
            path: receipt_path,
            outcome_hint: "diff_complete/full_repo_limited".to_string(),
        },
        summary: DiffSummary {
            changed_files: changed_file_count,
            changed_seams: changed_seam_count,
            probes: output.summary.probes,
            exposed: output.summary.exposed,
            weakly_exposed: output.summary.weakly_exposed,
            reachable_unrevealed: output.summary.reachable_unrevealed,
            no_static_path: output.summary.no_static_path,
            unknown,
        },
        changed_files,
        changed_seams,
    }
}

pub(crate) fn render_diff_report_json(report: &DiffReport) -> Result<String, String> {
    super::json::render_pretty_with_newline(report, "ripr diff report")
}

pub(crate) fn render_diff_report_human(report: &DiffReport) -> String {
    let mut out = String::new();
    out.push_str("RIPR diff status: ");
    out.push_str(&report.run_status);
    out.push('\n');
    out.push_str(&format!("root: {}\n", report.root));
    out.push_str(&format!("range: {}...{}\n", report.base, report.head));
    out.push_str(&format!("mode: {}\n", report.mode));
    out.push_str(&format!(
        "changed files: {}\nchanged seams: {}\n",
        report.summary.changed_files, report.summary.changed_seams
    ));
    out.push_str(&format!(
        "full repo context: {} ({})\n",
        report.runtime_status.full_repo_context.state,
        report.runtime_status.full_repo_context.limitation_category
    ));
    out.push_str(&format!("receipt path: {}\n\n", report.receipt.path));

    out.push_str("Changed files\n");
    if report.changed_files.is_empty() {
        out.push_str("  none\n");
    } else {
        for file in &report.changed_files {
            out.push_str(&format!(
                "  - {} (+{} -{})\n",
                file.path, file.added_count, file.removed_count
            ));
        }
    }

    out.push_str("\nChanged seams\n");
    if report.changed_seams.is_empty() {
        out.push_str("  none\n");
    } else {
        for seam in &report.changed_seams {
            out.push_str(&format!(
                "  - {} {}:{} {}\n",
                seam.seam_id, seam.file, seam.line, seam.classification
            ));
            if let Some(canonical_gap_id) = &seam.canonical_gap_id {
                out.push_str(&format!("    canonical_gap_id: {canonical_gap_id}\n"));
            }
            if let Some(next_step) = &seam.recommended_next_step {
                out.push_str(&format!("    next action: {next_step}\n"));
            }
            if let Some(evidence) = seam.evidence.first() {
                out.push_str(&format!("    evidence: {evidence}\n"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CheckOutput, Mode};
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Probe, ProbeFamily, ProbeId,
        RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState, Summary, SymbolId,
    };
    use std::path::PathBuf;

    #[test]
    fn diff_report_preserves_diff_complete_full_repo_limited_status() -> Result<(), String> {
        let report = build_diff_report(
            &CheckOutput {
                schema_version: "0.1".to_string(),
                tool: "ripr".to_string(),
                mode: Mode::Draft,
                root: PathBuf::from("repo"),
                base: None,
                summary: Summary {
                    probes: 1,
                    weakly_exposed: 1,
                    ..Summary::default()
                },
                findings: vec![sample_finding()],
            },
            "origin/main",
            "HEAD",
            vec![DiffChangedFile {
                path: "src/lib.rs".to_string(),
                added_lines: vec![2],
                removed_lines: vec![2],
                added_count: 1,
                removed_count: 1,
            }],
            "target/ripr/receipts/diff-first-origin-main-HEAD.json".to_string(),
        );

        let json = render_diff_report_json(&report)?;
        assert!(json.contains(r#""run_status": "diff_complete_full_repo_limited""#));
        assert!(json.contains(r#""state": "diff_complete""#));
        assert!(json.contains(r#""state": "full_repo_limited""#));
        assert!(json.contains(r#""downstream_consumable": true"#));
        assert!(json.contains(r#""outcome_hint": "diff_complete/full_repo_limited""#));

        let human = render_diff_report_human(&report);
        assert!(human.contains("RIPR diff status: diff_complete_full_repo_limited"));
        assert!(human.contains("full repo context: full_repo_limited"));
        assert!(human.contains("receipt path: target/ripr/receipts/"));
        Ok(())
    }

    fn sample_finding() -> crate::domain::Finding {
        crate::domain::Finding {
            id: "probe:src_lib_rs:2:predicate".to_string(),
            canonical_gap: Some(crate::domain::FindingCanonicalGap {
                id: "gap:src-lib-rs:predicate".to_string(),
                language: "rust".to_string(),
                file: "src/lib.rs".to_string(),
                owner: "over_threshold".to_string(),
                behavior_kind: "predicate_boundary".to_string(),
                probe_kind: "predicate".to_string(),
                normalized_discriminator: "amount == threshold".to_string(),
            }),
            probe: Probe {
                id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
                location: SourceLocation::new("src/lib.rs", 2, 5),
                owner: Some(SymbolId("src::over_threshold".to_string())),
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: Some("amount >= threshold".to_string()),
                after: Some("amount > threshold".to_string()),
                expression: "amount >= threshold".to_string(),
                expected_sinks: vec!["return_value".to_string()],
                required_oracles: vec!["equality_boundary".to_string()],
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    "related test reaches owner",
                ),
                infect: StageEvidence::new(
                    StageState::Weak,
                    Confidence::Medium,
                    "changed value is not exact",
                ),
                propagate: StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    "return value is visible",
                ),
                reveal: RevealEvidence {
                    observe: StageEvidence::new(
                        StageState::Weak,
                        Confidence::Medium,
                        "broad assertion observes value",
                    ),
                    discriminate: StageEvidence::new(
                        StageState::No,
                        Confidence::Medium,
                        "missing equality boundary",
                    ),
                },
            },
            confidence: 0.70,
            evidence: vec!["related test reaches owner".to_string()],
            missing: vec!["missing discriminator amount == threshold".to_string()],
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: Vec::new(),
            related_tests: Vec::new(),
            recommended_next_step: Some("Add equality-boundary assertion.".to_string()),
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
        }
    }
}
