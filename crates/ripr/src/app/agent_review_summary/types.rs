use crate::app::agent_status::AgentStatusCommand;

pub(crate) const AGENT_REVIEW_SUMMARY_SCHEMA_VERSION: &str = "0.1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewSummaryReport {
    pub(crate) schema_version: String,
    pub(crate) tool: String,
    pub(crate) status: String,
    pub(crate) root: String,
    pub(crate) target_seam: Option<AgentReviewTargetSeam>,
    pub(crate) static_movement: AgentReviewStaticMovement,
    pub(crate) next_command: Option<AgentStatusCommand>,
    pub(crate) surfaces: Vec<AgentReviewSurface>,
    pub(crate) ci_artifacts: Vec<AgentReviewArtifact>,
    pub(crate) reviewer_summary: AgentReviewTextSummary,
    pub(crate) limits: AgentReviewLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewTargetSeam {
    pub(crate) seam_id: String,
    pub(crate) source: String,
    pub(crate) file: Option<String>,
    pub(crate) line: Option<u64>,
    pub(crate) seam_kind: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewStaticMovement {
    pub(crate) state: String,
    pub(crate) before_class: Option<String>,
    pub(crate) after_class: Option<String>,
    pub(crate) grip_class: Option<String>,
    pub(crate) evidence_artifact: Option<String>,
    pub(crate) verify_artifact: Option<String>,
    pub(crate) summary: String,
    pub(crate) next_action: Option<AgentReviewNextAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewNextAction {
    pub(crate) kind: String,
    pub(crate) summary: String,
    pub(crate) recommended_action: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewSurface {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) path: Option<String>,
    pub(crate) state: String,
    pub(crate) status: String,
    pub(crate) required: bool,
    pub(crate) summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewArtifact {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) state: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewTextSummary {
    pub(crate) headline: String,
    pub(crate) what_changed: String,
    pub(crate) evidence: String,
    pub(crate) remaining: String,
    pub(crate) reviewer_should_inspect: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentReviewLimits {
    pub(crate) static_artifact_relationship: bool,
    pub(crate) runtime_mutation_execution: bool,
    pub(crate) automatic_edits: bool,
    pub(crate) generated_tests: bool,
}
