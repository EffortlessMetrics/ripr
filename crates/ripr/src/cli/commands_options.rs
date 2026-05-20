use crate::app::Mode;
use crate::config::CheckInputExplicit;
use crate::output;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct InitOptions {
    pub(crate) root: PathBuf,
    pub(crate) dry_run: bool,
    pub(crate) force: bool,
    pub(crate) ci: Option<InitCi>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InitCi {
    Github,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PilotOptions {
    pub(crate) root: PathBuf,
    pub(crate) out_dir: PathBuf,
    pub(crate) mode: Mode,
    pub(crate) explicit: CheckInputExplicit,
    pub(crate) max_seams: usize,
    pub(crate) timeout_ms: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct OutcomeOptions {
    pub(crate) before: PathBuf,
    pub(crate) after: PathBuf,
    pub(crate) format: OutcomeFormat,
    pub(crate) out: Option<PathBuf>,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct EvidenceHealthOptions {
    pub(crate) root: PathBuf,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
    pub(crate) mutation_calibration: Option<PathBuf>,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ReviewCommentsOptions {
    pub(crate) root: PathBuf,
    pub(crate) base: String,
    pub(crate) head: String,
    pub(crate) gap_ledger: Option<PathBuf>,
    pub(crate) out: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GateOptions {
    pub(crate) input: output::gate::GateEvaluateInput,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct BaselineCreateOptions {
    pub(crate) from: PathBuf,
    pub(crate) out: PathBuf,
    pub(crate) dry_run: bool,
    pub(crate) force: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct BaselineDiffOptions {
    pub(crate) baseline: PathBuf,
    pub(crate) current: PathBuf,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct BaselineUpdateOptions {
    pub(crate) baseline: PathBuf,
    pub(crate) current: PathBuf,
    pub(crate) out: Option<PathBuf>,
    pub(crate) remove_resolved: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RiprZeroStatusOptions {
    pub(crate) baseline: Option<PathBuf>,
    pub(crate) delta: PathBuf,
    pub(crate) gap_ledger: Option<PathBuf>,
    pub(crate) gate: Option<PathBuf>,
    pub(crate) pr_guidance: Option<PathBuf>,
    pub(crate) recommendation_calibration: Option<PathBuf>,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PolicyReadinessOptions {
    pub(crate) root: String,
    pub(crate) gate_decision: Option<PathBuf>,
    pub(crate) baseline_delta: Option<PathBuf>,
    pub(crate) recommendation_calibration: Option<PathBuf>,
    pub(crate) mutation_calibration: Option<PathBuf>,
    pub(crate) waiver_aging: Option<PathBuf>,
    pub(crate) suppression_health: Option<PathBuf>,
    pub(crate) repo_config: Option<PathBuf>,
    pub(crate) previous_readiness: Option<PathBuf>,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PolicyOperationsOptions {
    pub(crate) root: String,
    pub(crate) policy_readiness: Option<PathBuf>,
    pub(crate) waiver_aging: Option<PathBuf>,
    pub(crate) suppression_health: Option<PathBuf>,
    pub(crate) baseline_delta: Option<PathBuf>,
    pub(crate) gate_decision: Option<PathBuf>,
    pub(crate) recommendation_calibration: Option<PathBuf>,
    pub(crate) mutation_calibration: Option<PathBuf>,
    pub(crate) preview_boundary: Option<PathBuf>,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PolicyHistoryOptions {
    pub(crate) root: String,
    pub(crate) current: PathBuf,
    pub(crate) history: Option<PathBuf>,
    pub(crate) commit: Option<String>,
    pub(crate) pr_number: Option<String>,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PolicyPromotionOptions {
    pub(crate) root: String,
    pub(crate) target_mode: String,
    pub(crate) operations: PathBuf,
    pub(crate) history: Option<PathBuf>,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PolicyPreviewPromotionOptions {
    pub(crate) root: String,
    pub(crate) language: String,
    pub(crate) candidate_class: String,
    pub(crate) evidence: Option<PathBuf>,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PolicyWaiverAgingOptions {
    pub(crate) root: String,
    pub(crate) ledger: Option<PathBuf>,
    pub(crate) history: Option<PathBuf>,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PolicySuppressionHealthOptions {
    pub(crate) root: PathBuf,
    pub(crate) manifest: PathBuf,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PrEvidenceLedgerOptions {
    pub(crate) pr_number: String,
    pub(crate) base: String,
    pub(crate) head: String,
    pub(crate) labels: Vec<String>,
    pub(crate) gate: Option<PathBuf>,
    pub(crate) baseline_delta: Option<PathBuf>,
    pub(crate) zero_status: Option<PathBuf>,
    pub(crate) pr_guidance: Option<PathBuf>,
    pub(crate) gap_ledger: Option<PathBuf>,
    pub(crate) recommendation_calibration: Option<PathBuf>,
    pub(crate) agent_receipt: Option<PathBuf>,
    pub(crate) coverage: Option<PathBuf>,
    pub(crate) history: Option<PathBuf>,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PrCommentsPlanOptions {
    pub(crate) root: String,
    pub(crate) pr_guidance: Option<PathBuf>,
    pub(crate) existing_comments: Option<PathBuf>,
    pub(crate) mode: output::pr_inline_comment_publish_plan::CommentMode,
    pub(crate) pull_request: Option<u64>,
    pub(crate) event_name: Option<String>,
    pub(crate) head_repo: Option<String>,
    pub(crate) base_repo: Option<String>,
    pub(crate) token_available: bool,
    pub(crate) write_permission: bool,
    pub(crate) max_inline_comments: usize,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PrReviewFrontPanelOptions {
    pub(crate) root: String,
    pub(crate) pr_guidance: Option<PathBuf>,
    pub(crate) first_action: Option<PathBuf>,
    pub(crate) assistant_proof: Option<PathBuf>,
    pub(crate) assistant_health: Option<PathBuf>,
    pub(crate) ledger: Option<PathBuf>,
    pub(crate) baseline_delta: Option<PathBuf>,
    pub(crate) zero_status: Option<PathBuf>,
    pub(crate) gate_decision: Option<PathBuf>,
    pub(crate) recommendation_calibration: Option<PathBuf>,
    pub(crate) mutation_calibration: Option<PathBuf>,
    pub(crate) coverage_frontier: Option<PathBuf>,
    pub(crate) receipt: Option<PathBuf>,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ReportPacketIndexOptions {
    pub(crate) root: String,
    pub(crate) reports_dir: PathBuf,
    pub(crate) review_dir: PathBuf,
    pub(crate) receipts_dir: PathBuf,
    pub(crate) workflow_dir: PathBuf,
    pub(crate) agent_dir: PathBuf,
    pub(crate) pilot_dir: PathBuf,
    pub(crate) ci_dir: PathBuf,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GapDecisionLedgerOptions {
    pub(crate) root: String,
    pub(crate) source: GapDecisionLedgerSource,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GapDecisionLedgerSource {
    Records(PathBuf),
    RepoExposure(PathBuf),
    CheckOutput(PathBuf),
}
impl GapDecisionLedgerSource {
    pub(crate) fn path(&self) -> &Path {
        match self {
            Self::Records(path) | Self::RepoExposure(path) | Self::CheckOutput(path) => path,
        }
    }
    pub(crate) fn kind(&self) -> output::gap_decision_ledger::GapDecisionLedgerSourceKind {
        match self {
            Self::Records(_) => output::gap_decision_ledger::GapDecisionLedgerSourceKind::Records,
            Self::RepoExposure(_) => {
                output::gap_decision_ledger::GapDecisionLedgerSourceKind::RepoExposure
            }
            Self::CheckOutput(_) => {
                output::gap_decision_ledger::GapDecisionLedgerSourceKind::CheckOutput
            }
        }
    }
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Records(_) => "gap records",
            Self::RepoExposure(_) => "repo exposure",
            Self::CheckOutput(_) => "check output",
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CoverageGripFrontierOptions {
    pub(crate) coverage: Option<PathBuf>,
    pub(crate) ledger: Option<PathBuf>,
    pub(crate) baseline_delta: Option<PathBuf>,
    pub(crate) zero_status: Option<PathBuf>,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct AssistantLoopProofOptions {
    pub(crate) root: String,
    pub(crate) pr_guidance: Option<PathBuf>,
    pub(crate) agent_packet: Option<PathBuf>,
    pub(crate) before: Option<PathBuf>,
    pub(crate) after: Option<PathBuf>,
    pub(crate) receipt: Option<PathBuf>,
    pub(crate) ledger: Option<PathBuf>,
    pub(crate) coverage_frontier: Option<PathBuf>,
    pub(crate) gate_decision: Option<PathBuf>,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct AssistantLoopHealthOptions {
    pub(crate) root: String,
    pub(crate) proofs: Vec<PathBuf>,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FirstActionOptions {
    pub(crate) root: String,
    pub(crate) pr_guidance: Option<PathBuf>,
    pub(crate) assistant_proof: Option<PathBuf>,
    pub(crate) gap_ledger: Option<PathBuf>,
    pub(crate) ledger: Option<PathBuf>,
    pub(crate) baseline_delta: Option<PathBuf>,
    pub(crate) receipt: Option<PathBuf>,
    pub(crate) gate_decision: Option<PathBuf>,
    pub(crate) coverage_frontier: Option<PathBuf>,
    pub(crate) editor_context: Option<PathBuf>,
    pub(crate) out: PathBuf,
    pub(crate) out_md: PathBuf,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum OutcomeFormat {
    Markdown,
    Json,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CalibrateOptions {
    pub(crate) mutants_json: PathBuf,
    pub(crate) repo_exposure_json: PathBuf,
    pub(crate) format: CalibrateFormat,
    pub(crate) out: Option<PathBuf>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CalibrateFormat {
    Markdown,
    Json,
}
