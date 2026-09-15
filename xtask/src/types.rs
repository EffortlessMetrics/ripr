use super::*;

#[derive(Debug)]
pub(crate) struct GlobAllow {
    pub(crate) glob: String,
}

#[derive(Debug, Default)]
pub(crate) struct FilePolicyAllowEntry {
    pub(crate) line: usize,
    pub(crate) glob: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) owner: Option<String>,
    pub(crate) surface: Option<String>,
    pub(crate) classification: Option<String>,
    pub(crate) reason: Option<String>,
    pub(crate) generated_by: Option<String>,
    pub(crate) covered_by: Option<Vec<String>>,
}

#[derive(Debug)]
pub(crate) struct WorkflowBudget {
    pub(crate) path: String,
    pub(crate) max_non_empty_lines: usize,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct RunBlock {
    pub(crate) line_number: usize,
    pub(crate) non_empty_lines: usize,
    pub(crate) text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RustConversionCandidate {
    pub(crate) path: String,
    pub(crate) line: Option<usize>,
    pub(crate) kind: String,
    pub(crate) priority: String,
    pub(crate) current_surface: String,
    pub(crate) recommendation: String,
    pub(crate) reason: String,
}

#[derive(Clone, Copy)]
pub(crate) struct CiFullEvidenceGate {
    pub(crate) name: &'static str,
    pub(crate) run: fn() -> Result<(), String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChangedPath {
    pub(crate) path: String,
    pub(crate) statuses: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorktreeDoctorSeverity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorktreeDoctorFinding {
    pub(crate) severity: WorktreeDoctorSeverity,
    pub(crate) message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrTriagePullRequest {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) is_draft: bool,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) merge_state_status: String,
    pub(crate) head_ref_name: String,
    pub(crate) base_ref_name: String,
    pub(crate) review_decision: String,
    pub(crate) labels: Vec<String>,
    pub(crate) files: Vec<String>,
    pub(crate) checks: Vec<PrTriageCheck>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrTriageCheck {
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) conclusion: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrTriageFinding {
    pub(crate) category: String,
    pub(crate) severity: String,
    pub(crate) message: String,
    pub(crate) prs: Vec<u64>,
    pub(crate) details: Vec<String>,
    pub(crate) recommended_action: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrTriageQueueDisposition {
    pub(crate) pr_number: u64,
    pub(crate) disposition: String,
    pub(crate) reason: String,
    pub(crate) recommended_action: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GhPrStatusPullRequest {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) is_draft: bool,
    pub(crate) merge_state_status: String,
    pub(crate) head_ref_name: String,
    pub(crate) base_ref_name: String,
    pub(crate) review_decision: String,
    pub(crate) checks: Vec<PrTriageCheck>,
    pub(crate) reviews: Vec<GhPrStatusReview>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GhPrStatusReview {
    pub(crate) author: String,
    pub(crate) state: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GhPrStatusReadiness {
    pub(crate) behind_main: bool,
    pub(crate) required_contexts_available: bool,
    pub(crate) required_checks_outstanding: Vec<String>,
    pub(crate) failed_checks: Vec<String>,
    pub(crate) pending_checks: Vec<String>,
    pub(crate) droid_checks: Vec<String>,
    pub(crate) safe_next_action: String,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct TraceBehavior {
    pub(crate) line: usize,
    pub(crate) id: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) spec: Option<String>,
    pub(crate) tests: Vec<String>,
    pub(crate) fixtures: Vec<String>,
    pub(crate) code: Vec<String>,
    pub(crate) outputs: Vec<String>,
    pub(crate) metrics: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct Capability {
    pub(crate) line: usize,
    pub(crate) id: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) spec: Option<String>,
    pub(crate) evidence: Vec<String>,
    pub(crate) fixtures: Vec<String>,
    pub(crate) next: Option<String>,
    pub(crate) metric: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MarkdownLink {
    pub(crate) line: usize,
    pub(crate) target: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TestOracleClass {
    Strong,
    Medium,
    Weak,
    Smoke,
}

impl TestOracleClass {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            TestOracleClass::Strong => "strong",
            TestOracleClass::Medium => "medium",
            TestOracleClass::Weak => "weak",
            TestOracleClass::Smoke => "smoke",
        }
    }

    pub(crate) fn rank(self) -> u8 {
        match self {
            TestOracleClass::Strong => 4,
            TestOracleClass::Medium => 3,
            TestOracleClass::Weak => 2,
            TestOracleClass::Smoke => 1,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TestOracleObservation {
    pub(crate) line: usize,
    pub(crate) class: TestOracleClass,
    pub(crate) pattern: String,
    pub(crate) detail: String,
}

#[derive(Clone, Debug)]
pub(crate) struct TestOracleTest {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) body_line: usize,
    pub(crate) body: String,
    pub(crate) class: TestOracleClass,
    pub(crate) observations: Vec<TestOracleObservation>,
}

#[derive(Clone, Debug)]
pub(crate) struct TestEfficiencyValue {
    pub(crate) line: usize,
    pub(crate) context: &'static str,
    pub(crate) value: String,
    pub(crate) text: String,
}

#[derive(Clone, Debug)]
pub(crate) struct TestEfficiencyEntry {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) class: &'static str,
    pub(crate) oracle_kind: String,
    pub(crate) oracle_strength: &'static str,
    pub(crate) reached_owners: Vec<String>,
    pub(crate) observed_values: Vec<TestEfficiencyValue>,
    pub(crate) reasons: Vec<String>,
    pub(crate) static_limitations: Vec<String>,
    pub(crate) duplicate_group_id: Option<String>,
    pub(crate) declared_intent: Option<DeclaredIntent>,
}

/// Intent declaration attached to a test-efficiency entry. The base
/// `class` and `reasons` are preserved; this is purely additive metadata
/// describing the author's stated reason for the test's shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeclaredIntent {
    pub(crate) intent: TestIntentKind,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TestIntentKind {
    Smoke,
    BusinessCaseDuplicate,
    OpaqueExternalOracle,
    IntegrationContract,
    PerformanceGuard,
    DocumentationExample,
}

impl TestIntentKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            TestIntentKind::Smoke => "smoke",
            TestIntentKind::BusinessCaseDuplicate => "business_case_duplicate",
            TestIntentKind::OpaqueExternalOracle => "opaque_external_oracle",
            TestIntentKind::IntegrationContract => "integration_contract",
            TestIntentKind::PerformanceGuard => "performance_guard",
            TestIntentKind::DocumentationExample => "documentation_example",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "smoke" => Some(Self::Smoke),
            "business_case_duplicate" => Some(Self::BusinessCaseDuplicate),
            "opaque_external_oracle" => Some(Self::OpaqueExternalOracle),
            "integration_contract" => Some(Self::IntegrationContract),
            "performance_guard" => Some(Self::PerformanceGuard),
            "documentation_example" => Some(Self::DocumentationExample),
            _ => None,
        }
    }

    pub(crate) fn supported() -> &'static [&'static str] {
        &[
            "smoke",
            "business_case_duplicate",
            "opaque_external_oracle",
            "integration_contract",
            "performance_guard",
            "documentation_example",
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TestIntentDeclaration {
    pub(crate) test: String,
    pub(crate) path: Option<String>,
    pub(crate) intent: TestIntentKind,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) block_line: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct DuplicateDiscriminatorGroup {
    pub(crate) id: String,
    pub(crate) members: Vec<DuplicateGroupMember>,
    pub(crate) shared_evidence: DuplicateGroupSharedEvidence,
    pub(crate) suggested_next_step: String,
}

#[derive(Clone, Debug)]
pub(crate) struct DuplicateGroupMember {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) line: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct DuplicateGroupSharedEvidence {
    pub(crate) owners: Vec<String>,
    pub(crate) oracle_kind: String,
    pub(crate) oracle_strength: &'static str,
    pub(crate) activation_signature: Vec<DuplicateGroupActivation>,
}

#[derive(Clone, Debug)]
pub(crate) struct DuplicateGroupActivation {
    pub(crate) context: &'static str,
    pub(crate) value: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ReportIndexEntry {
    pub(crate) file: String,
    pub(crate) path: String,
    pub(crate) status: String,
}

#[derive(Clone, Debug)]
pub(crate) struct RepoOpsPacketSpec {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) command: &'static str,
    pub(crate) description: &'static str,
    pub(crate) artifacts: &'static [&'static str],
}

#[derive(Clone, Debug)]
pub(crate) struct ReportIndexRepoOpsPacket {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) status: String,
    pub(crate) command: &'static str,
    pub(crate) description: &'static str,
    pub(crate) artifacts: Vec<ReportIndexRepoOpsArtifact>,
}

#[derive(Clone, Debug)]
pub(crate) struct ReportIndexRepoOpsArtifact {
    pub(crate) path: String,
    pub(crate) status: String,
    pub(crate) available: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ReceiptSpec {
    pub(crate) file: &'static str,
    pub(crate) command: &'static str,
    pub(crate) reports: &'static [&'static str],
}

#[derive(Clone, Debug)]
pub(crate) struct ReceiptRecord {
    pub(crate) file: String,
    pub(crate) command: String,
    pub(crate) status: String,
    pub(crate) reports: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StaticSeamRecord {
    pub(crate) seam_id: String,
    pub(crate) seam_kind: String,
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) seam_grip_class: String,
    pub(crate) oracle_kind: String,
    pub(crate) oracle_strength: String,
    pub(crate) observed_values: Vec<String>,
    pub(crate) missing_discriminators: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TargetedTestOutcomeArgs {
    pub(crate) before: PathBuf,
    pub(crate) after: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActionableGapOutcomesArgs {
    pub(crate) actionable_gaps: PathBuf,
    pub(crate) agent_receipt: Option<PathBuf>,
    pub(crate) targeted_test_outcome: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TargetedTestOutcomeReport {
    pub(crate) before_path: String,
    pub(crate) after_path: String,
    pub(crate) before_counts: BTreeMap<String, usize>,
    pub(crate) after_counts: BTreeMap<String, usize>,
    pub(crate) moved: Vec<TargetedTestOutcomeMovement>,
    pub(crate) unchanged: Vec<TargetedTestOutcomeMovement>,
    pub(crate) regressed: Vec<TargetedTestOutcomeMovement>,
    pub(crate) new: Vec<TargetedTestOutcomeSeam>,
    pub(crate) removed: Vec<TargetedTestOutcomeSeam>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TargetedTestOutcomeMovement {
    pub(crate) seam_id: String,
    pub(crate) seam_kind: String,
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) before: String,
    pub(crate) after: String,
    pub(crate) direction: String,
    pub(crate) evidence_delta: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TargetedTestOutcomeSeam {
    pub(crate) seam_id: String,
    pub(crate) seam_kind: String,
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) grip_class: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActionableGapOutcomesReport {
    pub(crate) actionable_gaps_path: String,
    pub(crate) agent_receipt_path: Option<String>,
    pub(crate) targeted_test_outcome_path: Option<String>,
    pub(crate) packets_total: usize,
    pub(crate) outcomes: Vec<ActionableGapOutcome>,
    pub(crate) orphaned_receipts: Vec<ActionableGapOrphanedReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActionableGapOutcomeMovementFront {
    pub(crate) current_actionable_count: usize,
    pub(crate) receipt_linked_actionable_delta: i64,
    pub(crate) resolved: usize,
    pub(crate) improved: usize,
    pub(crate) unchanged_after_attempt: usize,
    pub(crate) missing_receipts: usize,
    pub(crate) orphaned_receipts: usize,
    pub(crate) top_blocked_reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActionableGapOutcome {
    pub(crate) canonical_gap_id: String,
    pub(crate) evidence_class: String,
    pub(crate) repair_kind: String,
    pub(crate) source_file: String,
    pub(crate) verify_command: String,
    pub(crate) verify_result: Option<String>,
    pub(crate) receipt_command: Option<String>,
    pub(crate) receipt_command_or_path: Option<String>,
    pub(crate) receipt_state: String,
    pub(crate) outcome_state: String,
    pub(crate) timestamp: Option<String>,
    pub(crate) attempt_instance: Option<String>,
    pub(crate) seam_id: Option<String>,
    pub(crate) before: Option<String>,
    pub(crate) after: Option<String>,
    pub(crate) movement_source: Option<String>,
    pub(crate) movement_direction: Option<String>,
    pub(crate) evidence_delta: Vec<String>,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActionableGapOrphanedReceipt {
    pub(crate) receipt_id: String,
    pub(crate) seam_id: Option<String>,
    pub(crate) source_file: Option<String>,
    pub(crate) line: Option<usize>,
    pub(crate) movement_direction: Option<String>,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CriticFinding {
    pub(crate) id: &'static str,
    pub(crate) severity: &'static str,
    pub(crate) message: &'static str,
    pub(crate) evidence: Vec<String>,
    pub(crate) recommended_action: &'static str,
}

#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "test-only type used via super::* glob in tests.rs"
)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Clone, Debug)]
pub enum FixKind {
    #[allow(dead_code, reason = "test-only variant")]
    AutoFixable,
    AuthorDecisionRequired,
    ReviewerDecisionRequired,
    PolicyExceptionRequired,
}

#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "test-only type used via super::* glob in tests.rs"
)]
pub struct CheckViolation {
    pub check: String,
    pub path: Option<PathBuf>,
    pub line: Option<usize>,
    pub severity: CheckStatus,
    pub category: String,
    pub message: String,
    pub why_it_matters: String,
    pub fix_kind: FixKind,
    pub suggested_commands: Vec<String>,
    pub suggested_patch: Option<String>,
    pub exception_template: Option<String>,
}

#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "test-only type used via super::* glob in tests.rs"
)]
pub struct CheckReport {
    pub check: String,
    pub status: CheckStatus,
    pub violations: Vec<CheckViolation>,
}

pub(crate) struct PolicyReportSpec<'a> {
    pub(crate) report_file: &'a str,
    pub(crate) check: &'a str,
    pub(crate) why_it_matters: &'a str,
    pub(crate) fix_kind: FixKind,
    pub(crate) recommended_fixes: &'a [&'a str],
    pub(crate) rerun_command: &'a str,
    pub(crate) exception_template: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct LocalContextFinding {
    pub(crate) path: String,
    pub(crate) line: Option<usize>,
    pub(crate) pattern: String,
    pub(crate) problem: String,
}

#[derive(Clone, Debug)]
pub(crate) struct LocalContextAllow {
    pub(crate) path: String,
    pub(crate) pattern: String,
    pub(crate) max_count: usize,
    pub(crate) line: usize,
}
