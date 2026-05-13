#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StaticSeamRecord {
    pub(super) seam_id: String,
    pub(super) seam_kind: String,
    pub(super) file: String,
    pub(super) line: usize,
    pub(super) seam_grip_class: String,
    pub(super) oracle_kind: String,
    pub(super) oracle_strength: String,
    pub(super) observed_values: Vec<String>,
    pub(super) missing_discriminators: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MutationOutcomeRecord {
    pub(super) mutant_id: Option<String>,
    pub(super) seam_id: Option<String>,
    pub(super) file: Option<String>,
    pub(super) line: Option<usize>,
    pub(super) mutation_operator: String,
    pub(super) runtime_outcome: String,
    pub(super) duration: Option<String>,
    pub(super) test_command: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MutationCalibrationReport {
    pub(super) static_seams_total: usize,
    pub(super) mutants_total: usize,
    pub(super) agreement: MutationCalibrationAgreement,
    pub(super) precision_notes: Vec<String>,
    pub(super) missed_runtime_signals: Vec<MutationCalibrationRuntimeSignal>,
    pub(super) static_only_findings: Vec<MutationCalibrationStaticOnlyFinding>,
    pub(super) matched: Vec<MutationCalibrationMatch>,
    pub(super) ambiguous_file_line: Vec<AmbiguousMutationCalibrationMatch>,
    pub(super) unmatched_mutants: Vec<MutationOutcomeRecord>,
    pub(super) static_without_runtime: Vec<StaticSeamRecord>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct MutationCalibrationAgreement {
    pub(super) static_gap_and_runtime_signal: usize,
    pub(super) static_gap_without_runtime_signal: usize,
    pub(super) runtime_signal_without_static_gap: usize,
    pub(super) static_clean_and_runtime_clean: usize,
    pub(super) runtime_inconclusive: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MutationCalibrationRuntimeSignal {
    pub(super) runtime: MutationOutcomeRecord,
    pub(super) static_seam: Option<StaticSeamRecord>,
    pub(super) confidence_label: &'static str,
    pub(super) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MutationCalibrationStaticOnlyFinding {
    pub(super) seam: StaticSeamRecord,
    pub(super) confidence_label: &'static str,
    pub(super) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MutationCalibrationMatch {
    pub(super) join_method: &'static str,
    pub(super) seam: StaticSeamRecord,
    pub(super) mutation: MutationOutcomeRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AmbiguousMutationCalibrationMatch {
    pub(super) mutation: MutationOutcomeRecord,
    pub(super) candidates: Vec<StaticSeamRecord>,
}
