use super::AGREEMENT_SAMPLE_LIMIT;
use super::parse::normalize_report_path;
use super::types::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn build_mutation_calibration_report(
    static_seams: Vec<StaticSeamRecord>,
    runtime_mutants: Vec<MutationOutcomeRecord>,
) -> MutationCalibrationReport {
    let mut static_by_id: BTreeMap<String, usize> = BTreeMap::new();
    let mut static_by_line: BTreeMap<(String, usize), Vec<usize>> = BTreeMap::new();
    for (idx, seam) in static_seams.iter().enumerate() {
        static_by_id.insert(seam.seam_id.clone(), idx);
        static_by_line
            .entry((normalize_report_path(&seam.file), seam.line))
            .or_default()
            .push(idx);
    }

    let mut matched_static_ids = BTreeSet::new();
    let mut ambiguous_static_ids = BTreeSet::new();
    let mut matched = Vec::new();
    let mut ambiguous_file_line = Vec::new();
    let mut unmatched_mutants = Vec::new();

    for mutation in runtime_mutants {
        let seam_match = mutation
            .seam_id
            .as_ref()
            .and_then(|seam_id| static_by_id.get(seam_id).copied())
            .map(|idx| ("seam_id", idx))
            .or_else(|| {
                let file = mutation.file.as_ref()?;
                let line = mutation.line?;
                let key = (normalize_report_path(file), line);
                let candidates = static_by_line.get(&key)?;
                (candidates.len() == 1).then_some(("file_line", candidates[0]))
            });

        match seam_match {
            Some((join_method, idx)) => {
                let seam = static_seams[idx].clone();
                matched_static_ids.insert(seam.seam_id.clone());
                matched.push(MutationCalibrationMatch {
                    join_method,
                    seam,
                    mutation,
                });
            }
            None => {
                let candidates = mutation
                    .file
                    .as_ref()
                    .and_then(|file| {
                        let line = mutation.line?;
                        let key = (normalize_report_path(file), line);
                        static_by_line.get(&key)
                    })
                    .filter(|candidates| candidates.len() > 1);

                if let Some(candidates) = candidates {
                    let candidates = candidates
                        .iter()
                        .map(|idx| {
                            let seam = static_seams[*idx].clone();
                            ambiguous_static_ids.insert(seam.seam_id.clone());
                            seam
                        })
                        .collect::<Vec<_>>();
                    ambiguous_file_line.push(AmbiguousMutationCalibrationMatch {
                        mutation,
                        candidates,
                    });
                } else {
                    unmatched_mutants.push(mutation);
                }
            }
        }
    }

    let static_without_runtime = static_seams
        .iter()
        .filter(|seam| {
            !matched_static_ids.contains(&seam.seam_id)
                && !ambiguous_static_ids.contains(&seam.seam_id)
        })
        .cloned()
        .collect::<Vec<_>>();

    let (agreement, precision_notes, missed_runtime_signals, static_only_findings) =
        mutation_calibration_agreement(
            &static_seams,
            &matched,
            &ambiguous_file_line,
            &unmatched_mutants,
        );

    MutationCalibrationReport {
        static_seams_total: static_seams.len(),
        mutants_total: matched.len() + ambiguous_file_line.len() + unmatched_mutants.len(),
        agreement,
        precision_notes,
        missed_runtime_signals,
        static_only_findings,
        matched,
        ambiguous_file_line,
        unmatched_mutants,
        static_without_runtime,
    }
}

fn mutation_calibration_agreement(
    static_seams: &[StaticSeamRecord],
    matched: &[MutationCalibrationMatch],
    ambiguous_file_line: &[AmbiguousMutationCalibrationMatch],
    unmatched_mutants: &[MutationOutcomeRecord],
) -> (
    MutationCalibrationAgreement,
    Vec<String>,
    Vec<MutationCalibrationRuntimeSignal>,
    Vec<MutationCalibrationStaticOnlyFinding>,
) {
    let mut matches_by_seam: BTreeMap<&str, Vec<&MutationCalibrationMatch>> = BTreeMap::new();
    for record in matched {
        matches_by_seam
            .entry(record.seam.seam_id.as_str())
            .or_default()
            .push(record);
    }

    let mut agreement = MutationCalibrationAgreement::default();
    let mut missed_runtime_signals = Vec::new();
    let mut static_only_findings = Vec::new();

    for seam in static_seams {
        let records = matches_by_seam
            .get(seam.seam_id.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let has_runtime_gap = records
            .iter()
            .any(|record| runtime_gap_signal(&record.mutation.runtime_outcome));
        let has_runtime_clean = records
            .iter()
            .any(|record| runtime_clean_signal(&record.mutation.runtime_outcome));
        let has_runtime_inconclusive = records.iter().any(|record| {
            !runtime_gap_signal(&record.mutation.runtime_outcome)
                && !runtime_clean_signal(&record.mutation.runtime_outcome)
        });
        let has_static_gap = static_gap_signal(seam);

        match (has_static_gap, has_runtime_gap, has_runtime_clean) {
            (true, true, _) => agreement.static_gap_and_runtime_signal += 1,
            (true, false, _) => {
                agreement.static_gap_without_runtime_signal += 1;
                static_only_findings.push(MutationCalibrationStaticOnlyFinding {
                    seam: seam.clone(),
                    confidence_label: static_only_confidence_label(records),
                    reason: static_only_reason(records),
                });
            }
            (false, true, _) => {
                agreement.runtime_signal_without_static_gap += 1;
                for record in records
                    .iter()
                    .filter(|record| runtime_gap_signal(&record.mutation.runtime_outcome))
                {
                    missed_runtime_signals.push(MutationCalibrationRuntimeSignal {
                        runtime: record.mutation.clone(),
                        static_seam: Some(seam.clone()),
                        confidence_label: "contradicts_static_clean",
                        reason: "runtime gap signal joined to a static-clean seam".to_string(),
                    });
                }
            }
            (false, false, true) => agreement.static_clean_and_runtime_clean += 1,
            (false, false, false) => {}
        }

        if has_runtime_inconclusive {
            agreement.runtime_inconclusive += 1;
        }
    }

    for record in unmatched_mutants
        .iter()
        .filter(|record| runtime_gap_signal(&record.runtime_outcome))
    {
        agreement.runtime_signal_without_static_gap += 1;
        missed_runtime_signals.push(MutationCalibrationRuntimeSignal {
            runtime: record.clone(),
            static_seam: None,
            confidence_label: "runtime_only_signal",
            reason: "runtime gap signal did not join to a static seam".to_string(),
        });
    }

    for record in ambiguous_file_line {
        if runtime_gap_signal(&record.mutation.runtime_outcome) {
            agreement.runtime_inconclusive += 1;
        }
    }

    missed_runtime_signals.truncate(AGREEMENT_SAMPLE_LIMIT);
    static_only_findings.truncate(AGREEMENT_SAMPLE_LIMIT);

    (
        agreement,
        mutation_calibration_precision_notes(),
        missed_runtime_signals,
        static_only_findings,
    )
}

fn mutation_calibration_precision_notes() -> Vec<String> {
    vec![
        "runtime gap signals are imported runtime labels such as missed, survived, not_caught, or uncaught".to_string(),
        "runtime clean signals are imported runtime labels such as caught or timeout".to_string(),
        "static_gap_without_runtime_signal includes static gap seams with no matched runtime gap signal in this import".to_string(),
        "ambiguous file/line runtime gap signals are counted as runtime_inconclusive until a seam_id or unambiguous location is available".to_string(),
    ]
}

fn static_only_reason(records: &[&MutationCalibrationMatch]) -> String {
    if records.is_empty() {
        "static gap seam has no matched runtime record in this import".to_string()
    } else if records
        .iter()
        .any(|record| runtime_clean_signal(&record.mutation.runtime_outcome))
    {
        "static gap seam matched runtime data without a runtime gap signal".to_string()
    } else {
        "static gap seam matched only runtime-inconclusive labels".to_string()
    }
}

fn static_only_confidence_label(records: &[&MutationCalibrationMatch]) -> &'static str {
    if records
        .iter()
        .any(|record| runtime_clean_signal(&record.mutation.runtime_outcome))
    {
        "contradicts_static_gap"
    } else {
        "no_runtime_data"
    }
}

fn static_gap_signal(seam: &StaticSeamRecord) -> bool {
    !matches!(
        seam.seam_grip_class.as_str(),
        "strongly_gripped" | "intentional" | "suppressed"
    )
}

fn runtime_gap_signal(outcome: &str) -> bool {
    matches!(
        normalize_runtime_label(outcome).as_str(),
        "missed" | "survived" | "survive" | "not_caught" | "uncaught"
    )
}

fn runtime_clean_signal(outcome: &str) -> bool {
    matches!(
        normalize_runtime_label(outcome).as_str(),
        "caught" | "timeout" | "timed_out" | "killed"
    )
}

pub(super) fn confidence_label_for_match(record: &MutationCalibrationMatch) -> &'static str {
    let has_static_gap = static_gap_signal(&record.seam);
    if runtime_gap_signal(&record.mutation.runtime_outcome) {
        if has_static_gap {
            "supports_static_gap"
        } else {
            "contradicts_static_clean"
        }
    } else if runtime_clean_signal(&record.mutation.runtime_outcome) {
        if has_static_gap {
            "contradicts_static_gap"
        } else {
            "supports_static_clean"
        }
    } else {
        "no_runtime_data"
    }
}

pub(super) fn runtime_outcome_counts(
    report: &MutationCalibrationReport,
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for record in report
        .matched
        .iter()
        .map(|matched| &matched.mutation)
        .chain(
            report
                .ambiguous_file_line
                .iter()
                .map(|ambiguous| &ambiguous.mutation),
        )
        .chain(report.unmatched_mutants.iter())
    {
        let key = normalize_runtime_label(&record.runtime_outcome);
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

pub(super) fn join_method_counts(
    report: &MutationCalibrationReport,
) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for record in &report.matched {
        *counts.entry(record.join_method).or_insert(0) += 1;
    }
    counts
}

fn normalize_runtime_label(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}
