use crate::app::{CheckOutput, Mode};
use crate::domain::{ExposureClass, LanguageId};
use crate::output::python_repair_card::{PythonRepairCard, python_repair_card};
use std::path::{Path, PathBuf};

pub(crate) const PILOT_SUMMARY_SCHEMA_VERSION: &str = "0.2";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PilotArtifacts {
    pub(crate) repo_exposure_json: PathBuf,
    pub(crate) repo_exposure_md: PathBuf,
    pub(crate) agent_seam_packets_json: PathBuf,
    pub(crate) pilot_summary_json: PathBuf,
    pub(crate) pilot_summary_md: PathBuf,
}

#[derive(Clone, Copy)]
pub(crate) struct PilotSummaryContext<'a> {
    pub(crate) root: &'a Path,
    pub(crate) mode: &'a Mode,
    pub(crate) config_path: Option<&'a Path>,
    pub(crate) max_seams: usize,
    pub(crate) timeout_ms: u64,
    pub(crate) artifacts: &'a PilotArtifacts,
    pub(crate) python_first_use: Option<&'a PilotPythonFirstUse>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PilotPythonFirstUse {
    pub(crate) status: PilotPythonFirstUseStatus,
    pub(crate) findings_total: usize,
    pub(crate) repair_cards_total: usize,
    pub(crate) limitation_count: usize,
    pub(crate) analysis_error: Option<String>,
    pub(crate) top_repair_card: Option<PythonRepairCard>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PilotPythonFirstUseStatus {
    AnalysisUnavailable,
    NoPythonFindings,
    NoRepairCards,
    Ready,
}

impl PilotPythonFirstUse {
    pub(crate) fn analysis_unavailable(error: String) -> Self {
        Self {
            status: PilotPythonFirstUseStatus::AnalysisUnavailable,
            findings_total: 0,
            repair_cards_total: 0,
            limitation_count: 1,
            analysis_error: Some(error),
            top_repair_card: None,
        }
    }

    pub(crate) fn from_check_output(output: &CheckOutput) -> Self {
        let python_findings = output
            .findings
            .iter()
            .filter(|finding| finding.language == Some(LanguageId::Python));
        let findings_total = python_findings.clone().count();
        let limitation_count = python_findings
            .filter(|finding| is_python_limitation(finding))
            .count();
        let cards = output
            .findings
            .iter()
            .filter_map(python_repair_card)
            .collect::<Vec<_>>();
        let repair_cards_total = cards.len();
        let top_repair_card = cards.into_iter().next();
        let status = if top_repair_card.is_some() {
            PilotPythonFirstUseStatus::Ready
        } else if findings_total == 0 {
            PilotPythonFirstUseStatus::NoPythonFindings
        } else {
            PilotPythonFirstUseStatus::NoRepairCards
        };

        Self {
            status,
            findings_total,
            repair_cards_total,
            limitation_count,
            analysis_error: None,
            top_repair_card,
        }
    }
}

impl PilotPythonFirstUseStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            PilotPythonFirstUseStatus::AnalysisUnavailable => "analysis_unavailable",
            PilotPythonFirstUseStatus::NoPythonFindings => "no_python_findings",
            PilotPythonFirstUseStatus::NoRepairCards => "no_repair_cards",
            PilotPythonFirstUseStatus::Ready => "ready",
        }
    }
}

fn is_python_limitation(finding: &crate::domain::Finding) -> bool {
    finding.static_limit_kind.is_some()
        || matches!(
            finding.class,
            ExposureClass::InfectionUnknown
                | ExposureClass::PropagationUnknown
                | ExposureClass::StaticUnknown
        )
}
