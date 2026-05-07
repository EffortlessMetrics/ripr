#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

mod command;
mod dispatch;
mod policy;
mod reports;
mod run;

#[cfg(test)]
use command::unknown_command_message;
use command::{XtaskCommand, known_command_root, known_commands};
use policy::{
    check_allow_attributes, check_droid_review_config, check_executable_files, check_file_policy,
    check_local_context, check_network_policy, check_no_panic_family, check_process_policy,
    check_static_language, check_workflows,
};
use reports::{
    dogfood, fixtures, metrics_report, pr_summary, receipts_write, repo_badge_artifacts,
    reports_index, test_oracle_report,
};
#[cfg(test)]
use reports::{lsp_cockpit_report, targeted_test_outcome};
use run::{
    TimedOutput, capture_output, capture_output_with_timeout, run, run_output, run_output_optional,
    run_output_owned,
};

#[derive(Debug)]
struct GlobAllow {
    glob: String,
}

#[derive(Debug)]
struct WorkflowBudget {
    path: String,
    max_non_empty_lines: usize,
    reason: String,
}

#[derive(Debug)]
struct RunBlock {
    line_number: usize,
    non_empty_lines: usize,
    text: String,
}

#[derive(Clone, Copy)]
struct CiFullEvidenceGate {
    name: &'static str,
    run: fn() -> Result<(), String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ChangedPath {
    path: String,
    statuses: BTreeSet<String>,
}

#[derive(Debug, Default)]
struct TraceBehavior {
    line: usize,
    id: Option<String>,
    name: Option<String>,
    spec: Option<String>,
    tests: Vec<String>,
    fixtures: Vec<String>,
    code: Vec<String>,
    outputs: Vec<String>,
    metrics: Vec<String>,
}

#[derive(Debug, Default)]
struct Capability {
    line: usize,
    id: Option<String>,
    name: Option<String>,
    status: Option<String>,
    spec: Option<String>,
    evidence: Vec<String>,
    fixtures: Vec<String>,
    next: Option<String>,
    metric: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MarkdownLink {
    line: usize,
    target: String,
}

#[derive(Debug, Default)]
struct CampaignManifest {
    id: Option<String>,
    title: Option<String>,
    status: Option<String>,
    end_state: Vec<String>,
    work_items: Vec<CampaignWorkItem>,
}

#[derive(Debug, Default)]
struct CampaignWorkItem {
    line: usize,
    id: Option<String>,
    status: Option<String>,
    branch: Option<String>,
    stackable: Option<bool>,
    requires_human_merge: Option<bool>,
    acceptance: Option<String>,
    commands: Vec<String>,
    blocked_by: Vec<String>,
    blocked_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestOracleClass {
    Strong,
    Medium,
    Weak,
    Smoke,
}

impl TestOracleClass {
    fn as_str(self) -> &'static str {
        match self {
            TestOracleClass::Strong => "strong",
            TestOracleClass::Medium => "medium",
            TestOracleClass::Weak => "weak",
            TestOracleClass::Smoke => "smoke",
        }
    }

    fn rank(self) -> u8 {
        match self {
            TestOracleClass::Strong => 4,
            TestOracleClass::Medium => 3,
            TestOracleClass::Weak => 2,
            TestOracleClass::Smoke => 1,
        }
    }
}

#[derive(Clone, Debug)]
struct TestOracleObservation {
    line: usize,
    class: TestOracleClass,
    pattern: String,
    detail: String,
}

#[derive(Clone, Debug)]
struct TestOracleTest {
    path: PathBuf,
    name: String,
    line: usize,
    body_line: usize,
    body: String,
    class: TestOracleClass,
    observations: Vec<TestOracleObservation>,
}

#[derive(Clone, Debug)]
struct TestEfficiencyValue {
    line: usize,
    context: &'static str,
    value: String,
    text: String,
}

#[derive(Clone, Debug)]
struct TestEfficiencyEntry {
    path: PathBuf,
    name: String,
    line: usize,
    class: &'static str,
    oracle_kind: String,
    oracle_strength: &'static str,
    reached_owners: Vec<String>,
    observed_values: Vec<TestEfficiencyValue>,
    reasons: Vec<String>,
    static_limitations: Vec<String>,
    duplicate_group_id: Option<String>,
    declared_intent: Option<DeclaredIntent>,
}

/// Intent declaration attached to a test-efficiency entry. The base
/// `class` and `reasons` are preserved; this is purely additive metadata
/// describing the author's stated reason for the test's shape.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DeclaredIntent {
    intent: TestIntentKind,
    owner: String,
    reason: String,
    source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestIntentKind {
    Smoke,
    BusinessCaseDuplicate,
    OpaqueExternalOracle,
    IntegrationContract,
    PerformanceGuard,
    DocumentationExample,
}

impl TestIntentKind {
    fn as_str(self) -> &'static str {
        match self {
            TestIntentKind::Smoke => "smoke",
            TestIntentKind::BusinessCaseDuplicate => "business_case_duplicate",
            TestIntentKind::OpaqueExternalOracle => "opaque_external_oracle",
            TestIntentKind::IntegrationContract => "integration_contract",
            TestIntentKind::PerformanceGuard => "performance_guard",
            TestIntentKind::DocumentationExample => "documentation_example",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
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

    fn supported() -> &'static [&'static str] {
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
struct TestIntentDeclaration {
    test: String,
    path: Option<String>,
    intent: TestIntentKind,
    owner: String,
    reason: String,
    block_line: usize,
}

#[derive(Clone, Debug)]
struct DuplicateDiscriminatorGroup {
    id: String,
    members: Vec<DuplicateGroupMember>,
    shared_evidence: DuplicateGroupSharedEvidence,
    suggested_next_step: String,
}

#[derive(Clone, Debug)]
struct DuplicateGroupMember {
    path: String,
    name: String,
    line: usize,
}

#[derive(Clone, Debug)]
struct DuplicateGroupSharedEvidence {
    owners: Vec<String>,
    oracle_kind: String,
    oracle_strength: &'static str,
    activation_signature: Vec<DuplicateGroupActivation>,
}

#[derive(Clone, Debug)]
struct DuplicateGroupActivation {
    context: &'static str,
    value: String,
}

#[derive(Debug)]
struct DogfoodScenario {
    name: String,
    root: PathBuf,
    diff: PathBuf,
}

#[derive(Debug)]
struct DogfoodRun {
    name: String,
    root: PathBuf,
    diff: PathBuf,
    actual_dir: PathBuf,
    duration_ms: u128,
    findings: usize,
    class_counts: BTreeMap<String, usize>,
    stop_reason_mentions: usize,
    errors: Vec<String>,
}

#[derive(Clone, Debug)]
struct ReportIndexEntry {
    file: String,
    path: String,
    status: String,
}

#[derive(Clone, Debug)]
struct ReceiptSpec {
    file: &'static str,
    command: &'static str,
    reports: &'static [&'static str],
}

#[derive(Clone, Debug)]
struct ReceiptRecord {
    file: String,
    command: String,
    status: String,
    reports: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MutationCalibrationArgs {
    root: String,
    mutants_json: PathBuf,
    repo_exposure_json: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StaticSeamRecord {
    seam_id: String,
    seam_kind: String,
    file: String,
    line: usize,
    seam_grip_class: String,
    oracle_kind: String,
    oracle_strength: String,
    observed_values: Vec<String>,
    missing_discriminators: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TargetedTestOutcomeArgs {
    before: PathBuf,
    after: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TargetedTestOutcomeReport {
    before_path: String,
    after_path: String,
    before_counts: BTreeMap<String, usize>,
    after_counts: BTreeMap<String, usize>,
    moved: Vec<TargetedTestOutcomeMovement>,
    unchanged: Vec<TargetedTestOutcomeMovement>,
    regressed: Vec<TargetedTestOutcomeMovement>,
    new: Vec<TargetedTestOutcomeSeam>,
    removed: Vec<TargetedTestOutcomeSeam>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TargetedTestOutcomeMovement {
    seam_id: String,
    seam_kind: String,
    file: String,
    line: usize,
    before: String,
    after: String,
    direction: String,
    evidence_delta: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TargetedTestOutcomeSeam {
    seam_id: String,
    seam_kind: String,
    file: String,
    line: usize,
    grip_class: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MutationOutcomeRecord {
    mutant_id: Option<String>,
    seam_id: Option<String>,
    file: Option<String>,
    line: Option<usize>,
    mutation_operator: String,
    runtime_outcome: String,
    duration: Option<String>,
    test_command: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MutationCalibrationReport {
    static_seams_total: usize,
    mutants_total: usize,
    agreement: MutationCalibrationAgreement,
    precision_notes: Vec<String>,
    missed_runtime_signals: Vec<MutationCalibrationRuntimeSignal>,
    static_only_findings: Vec<MutationCalibrationStaticOnlyFinding>,
    matched: Vec<MutationCalibrationMatch>,
    ambiguous_file_line: Vec<AmbiguousMutationCalibrationMatch>,
    unmatched_mutants: Vec<MutationOutcomeRecord>,
    static_without_runtime: Vec<StaticSeamRecord>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MutationCalibrationAgreement {
    static_gap_and_runtime_signal: usize,
    static_gap_without_runtime_signal: usize,
    runtime_signal_without_static_gap: usize,
    static_clean_and_runtime_clean: usize,
    runtime_inconclusive: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MutationCalibrationRuntimeSignal {
    runtime: MutationOutcomeRecord,
    static_seam: Option<StaticSeamRecord>,
    reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MutationCalibrationStaticOnlyFinding {
    seam: StaticSeamRecord,
    reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MutationCalibrationMatch {
    join_method: &'static str,
    seam: StaticSeamRecord,
    mutation: MutationOutcomeRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AmbiguousMutationCalibrationMatch {
    mutation: MutationOutcomeRecord,
    candidates: Vec<StaticSeamRecord>,
}

const MUTATION_CALIBRATION_STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT: usize = 50;
const MUTATION_CALIBRATION_AGREEMENT_SAMPLE_LIMIT: usize = 50;

#[derive(Clone, Debug, Eq, PartialEq)]
struct CriticFinding {
    id: &'static str,
    severity: &'static str,
    message: &'static str,
    evidence: Vec<String>,
    recommended_action: &'static str,
}

#[derive(Debug, Default)]
struct ReportIndexCampaign {
    id: String,
    title: String,
    status: String,
    ready_work_items: Vec<String>,
    issues: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Clone, Debug)]
pub enum FixKind {
    AutoFixable,
    AuthorDecisionRequired,
    ReviewerDecisionRequired,
    PolicyExceptionRequired,
}

#[derive(Clone, Debug)]
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
pub struct CheckReport {
    pub check: String,
    pub status: CheckStatus,
    pub violations: Vec<CheckViolation>,
}

struct PolicyReportSpec<'a> {
    report_file: &'a str,
    check: &'a str,
    why_it_matters: &'a str,
    fix_kind: FixKind,
    recommended_fixes: &'a [&'a str],
    rerun_command: &'a str,
    exception_template: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LocalContextFinding {
    path: String,
    line: Option<usize>,
    pattern: String,
    problem: String,
}

#[derive(Clone, Debug)]
struct LocalContextAllow {
    path: String,
    pattern: String,
    max_count: usize,
    line: usize,
}

fn main() {
    let command = XtaskCommand::parse(std::env::args().skip(1));
    let result = dispatch::execute(command);
    if let Err(err) = result {
        eprintln!("xtask: {err}");
        std::process::exit(1);
    }
}

fn ci_fast() -> Result<(), String> {
    run("cargo", &["fmt", "--check"])?;
    run("cargo", &["check", "--workspace", "--all-targets"])?;
    run("cargo", &["test", "--workspace"])?;
    run_policy_checks()
}

fn precommit() -> Result<(), String> {
    ensure_reports_dir()?;
    run("cargo", &["fmt", "--check"])?;
    check_static_language()?;
    check_no_panic_family()?;
    check_allow_attributes()?;
    check_local_context()?;
    check_file_policy()?;
    check_executable_files()?;
    check_workflows()?;
    check_droid_review_config()?;
    check_spec_format()?;
    check_fixture_contracts()?;
    check_traceability()?;
    check_capabilities()?;
    check_workspace_shape()?;
    check_architecture()?;
    check_public_api()?;
    check_output_contracts()?;
    check_doc_index()?;
    check_readme_state()?;
    markdown_links()?;
    check_campaign()?;
    check_pr_shape()?;
    check_generated()?;
    let body = precommit_report_body();
    write_report("precommit.md", &body)
}

fn check_pr() -> Result<(), String> {
    ensure_reports_dir()?;
    ci_fast()?;
    run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run("cargo", &["doc", "--workspace", "--no-deps"])?;
    pr_summary()?;
    let body = check_pr_report_body();
    write_report("check-pr.md", &body)?;
    receipts_write()?;
    pr_summary()?;
    reports_index()
}

fn run_policy_checks() -> Result<(), String> {
    check_static_language()?;
    check_no_panic_family()?;
    check_allow_attributes()?;
    check_local_context()?;
    check_file_policy()?;
    check_executable_files()?;
    check_workflows()?;
    check_droid_review_config()?;
    check_spec_format()?;
    check_fixture_contracts()?;
    check_traceability()?;
    check_capabilities()?;
    check_workspace_shape()?;
    check_architecture()?;
    check_public_api()?;
    check_output_contracts()?;
    check_doc_index()?;
    check_readme_state()?;
    markdown_links()?;
    check_campaign()?;
    check_pr_shape()?;
    check_generated()?;
    check_dependencies()?;
    check_process_policy()?;
    check_network_policy()
}

fn ci_full() -> Result<(), String> {
    check_pr()?;
    run_ci_full_evidence_gates(&ci_full_evidence_gates())?;
    run("cargo", &["package", "-p", "ripr", "--list"])?;
    run("cargo", &["publish", "-p", "ripr", "--dry-run"]).map(|_| ())
}

fn ci_full_evidence_gates() -> [CiFullEvidenceGate; 5] {
    [
        CiFullEvidenceGate {
            name: "fixtures",
            run: ci_full_fixtures,
        },
        CiFullEvidenceGate {
            name: "goldens check",
            run: goldens_check,
        },
        CiFullEvidenceGate {
            name: "test-oracle-report",
            run: test_oracle_report,
        },
        CiFullEvidenceGate {
            name: "dogfood",
            run: dogfood,
        },
        CiFullEvidenceGate {
            name: "metrics",
            run: metrics_report,
        },
    ]
}

fn ci_full_fixtures() -> Result<(), String> {
    fixtures(None)
}

fn run_ci_full_evidence_gates(gates: &[CiFullEvidenceGate]) -> Result<(), String> {
    for gate in gates {
        (gate.run)()
            .map_err(|err| format!("ci-full evidence gate `{}` failed: {err}", gate.name))?;
    }
    Ok(())
}

const RIPR_MANAGED_PRE_COMMIT_MARKER: &str = "# ripr-managed pre-commit hook";

fn install_hooks(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("install-hooks does not accept arguments".to_string());
    }

    let hook = install_hooks_in(Path::new("."))?;
    eprintln!("installed hook: {}", hook.display());
    Ok(())
}

fn install_hooks_in(root: &Path) -> Result<PathBuf, String> {
    let git_dir = root.join(".git");
    if !git_dir.is_dir() {
        return Err(format!(
            "missing .git directory under {}; run from a git worktree",
            root.display()
        ));
    }

    let hooks_dir = git_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)
        .map_err(|err| format!("failed to create {}: {err}", hooks_dir.display()))?;

    let hook = hooks_dir.join("pre-commit");
    if hook.exists() {
        let current = fs::read_to_string(&hook)
            .map_err(|err| format!("failed to read {}: {err}", hook.display()))?;
        if !is_ripr_managed_hook(&current) {
            return Err(format!(
                "refusing to overwrite unmanaged hook at {}; remove it or install the ripr precommit hook manually",
                hook.display()
            ));
        }
    }

    fs::write(&hook, ripr_pre_commit_hook())
        .map_err(|err| format!("failed to write {}: {err}", hook.display()))?;
    make_hook_executable(&hook)?;
    Ok(hook)
}

fn ripr_pre_commit_hook() -> String {
    format!("#!/usr/bin/env sh\n{RIPR_MANAGED_PRE_COMMIT_MARKER}\nset -eu\ncargo xtask precommit\n")
}

fn is_ripr_managed_hook(text: &str) -> bool {
    text.contains(RIPR_MANAGED_PRE_COMMIT_MARKER)
}

#[cfg(unix)]
fn make_hook_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .map_err(|err| format!("failed to read {} metadata: {err}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|err| format!("failed to set executable bit on {}: {err}", path.display()))
}

#[cfg(not(unix))]
fn make_hook_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn shape() -> Result<(), String> {
    ensure_reports_dir()?;
    run("cargo", &["fmt"])?;
    let sorted = sort_allowlist_files()?;
    let body = shape_report_body(&sorted);
    write_report("shape.md", &body)
}

fn fix_pr() -> Result<(), String> {
    shape()?;
    pr_summary()?;
    let body = "# ripr fix-pr report\n\nStatus: pass\n\nActions:\n\n- Ran `cargo xtask shape`.\n- Ran `cargo xtask pr-summary`.\n\nReports:\n\n- `target/ripr/reports/shape.md`\n- `target/ripr/reports/pr-summary.md`\n\nNext commands:\n\n```bash\ncargo xtask check-pr\n```\n";
    write_report("fix-pr.md", body)
}

pub(crate) fn pr_summary_impl() -> Result<(), String> {
    let changes = collect_pr_changes()?;
    let body = pr_summary_body(&changes);
    write_report("pr-summary.md", &body)
}

fn check_pr_shape() -> Result<(), String> {
    let changes = collect_pr_changes()?;
    let warnings = pr_shape_warnings(&changes);
    write_report("pr-shape.md", &pr_shape_report_body(&warnings))
}

pub(crate) fn critic_impl() -> Result<(), String> {
    ensure_reports_dir()?;
    let changes = collect_pr_changes()?;
    let reports = report_index_entries()?;
    let receipts = receipt_index_entries()?;
    let findings = critic_findings(&changes, &reports, &receipts);
    write_report(
        "critic.md",
        &critic_markdown(&findings, &reports, &receipts),
    )?;
    write_report("critic.json", &critic_json(&findings, &reports, &receipts))
}

pub(crate) fn reports_impl(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("index") => reports_index(),
        Some(other) => Err(format!(
            "unknown reports command `{other}`\nusage: cargo xtask reports index"
        )),
        None => Err("missing reports command\nusage: cargo xtask reports index".to_string()),
    }
}

pub(crate) fn receipts_impl(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None => receipts_write(),
        Some("check") => receipts_check(),
        Some(other) => Err(format!(
            "unknown receipts command `{other}`\nusage: cargo xtask receipts\n       cargo xtask receipts check"
        )),
    }
}

pub(crate) fn reports_index_impl() -> Result<(), String> {
    let changes = collect_pr_changes()?;
    let campaign = report_index_campaign();
    let reports = report_index_entries()?;
    let receipts = receipt_index_entries()?;
    let missing = report_index_missing_expected(&reports, &changes);
    let status = report_index_status(&reports, &missing, &campaign.issues);
    let next_commands = report_index_next_commands(&missing);

    let markdown = report_index_markdown(
        status,
        &campaign,
        &reports,
        &receipts,
        &missing,
        &next_commands,
    );
    let json = report_index_json(
        status,
        &campaign,
        &reports,
        &receipts,
        &missing,
        &next_commands,
    );
    write_report("index.md", &markdown)?;
    write_report("index.json", &json)
}

pub(crate) fn receipts_write_impl() -> Result<(), String> {
    ensure_receipts_dir()?;
    let git = receipt_git_metadata();
    let mut records = Vec::new();
    for spec in receipt_specs() {
        let reports = spec
            .reports
            .iter()
            .map(|report| format!("target/ripr/reports/{report}"))
            .collect::<Vec<_>>();
        let status = receipt_status_from_reports(&reports);
        let record = ReceiptRecord {
            file: spec.file.to_string(),
            command: spec.command.to_string(),
            status,
            reports,
        };
        write_receipt(spec.file, &receipt_json(&record, &git))?;
        records.push(record);
    }
    write_report(
        "receipts.md",
        &receipts_report_markdown("pass", &records, &[]),
    )
}

fn receipts_check() -> Result<(), String> {
    let violations = receipts_check_violations()?;
    let records = read_receipt_records();
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    write_report(
        "receipts.md",
        &receipts_report_markdown(status, &records, &violations),
    )?;
    if violations.is_empty() {
        Ok(())
    } else {
        Err("receipt validation failed; see target/ripr/reports/receipts.md".to_string())
    }
}

fn receipt_specs() -> Vec<ReceiptSpec> {
    vec![
        ReceiptSpec {
            file: "shape.json",
            command: "cargo xtask shape",
            reports: &["shape.md"],
        },
        ReceiptSpec {
            file: "fix-pr.json",
            command: "cargo xtask fix-pr",
            reports: &["fix-pr.md", "shape.md", "pr-summary.md"],
        },
        ReceiptSpec {
            file: "ci-fast.json",
            command: "cargo xtask ci-fast",
            reports: &[
                "static-language.md",
                "no-panic-family.md",
                "allow-attributes.md",
                "local-context.md",
                "local-context.json",
                "file-policy.md",
                "executable-files.md",
                "workflows.md",
                "spec-format.md",
                "fixture-contracts.md",
                "traceability.md",
                "capabilities.md",
                "workspace-shape.md",
                "architecture.md",
                "public-api.md",
                "output-contracts.md",
                "doc-index.md",
                "readme-state.md",
                "markdown-links.md",
                "campaign.md",
                "pr-shape.md",
                "generated.md",
                "dependencies.md",
                "process-policy.md",
                "network-policy.md",
            ],
        },
        ReceiptSpec {
            file: "check-pr.json",
            command: "cargo xtask check-pr",
            reports: &["check-pr.md", "pr-summary.md"],
        },
        ReceiptSpec {
            file: "fixtures.json",
            command: "cargo xtask fixtures",
            reports: &["fixtures.md"],
        },
        ReceiptSpec {
            file: "goldens.json",
            command: "cargo xtask goldens check",
            reports: &["goldens.md"],
        },
        ReceiptSpec {
            file: "test-oracles.json",
            command: "cargo xtask test-oracle-report",
            reports: &["test-oracles.md", "test-oracles.json"],
        },
        ReceiptSpec {
            file: "badge-artifacts.json",
            command: "cargo xtask badge-artifacts",
            reports: &[
                "ripr-badge.json",
                "ripr-badge-shields.json",
                "ripr-plus-badge.json",
                "ripr-plus-badge-shields.json",
                "ripr-badges.md",
            ],
        },
        ReceiptSpec {
            file: "repo-badge-artifacts.json",
            command: "cargo xtask repo-badge-artifacts",
            reports: &[
                "repo-ripr-badge.json",
                "repo-ripr-badge-shields.json",
                "repo-ripr-plus-badge.json",
                "repo-ripr-plus-badge-shields.json",
                "repo-ripr-badges.md",
            ],
        },
        ReceiptSpec {
            file: "dogfood.json",
            command: "cargo xtask dogfood",
            reports: &["dogfood.md", "dogfood.json"],
        },
        ReceiptSpec {
            file: "metrics.json",
            command: "cargo xtask metrics",
            reports: &["metrics.md", "metrics.json"],
        },
    ]
}

fn receipt_status_from_reports(reports: &[String]) -> String {
    let mut saw_report = false;
    let mut saw_warn = false;
    for report in reports {
        let path = Path::new(report);
        if !path.exists() {
            continue;
        }
        saw_report = true;
        match report_entry_status(path).as_str() {
            "fail" | "failed" => return "failed".to_string(),
            "warn" | "warning" => saw_warn = true,
            _ => {}
        }
    }
    if !saw_report {
        "missing".to_string()
    } else if saw_warn {
        "warn".to_string()
    } else {
        "passed".to_string()
    }
}

fn receipt_git_metadata() -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    values.insert(
        "branch".to_string(),
        git_value(&["rev-parse", "--abbrev-ref", "HEAD"]),
    );
    values.insert("commit".to_string(), git_value(&["rev-parse", "HEAD"]));
    values
}

fn git_value(args: &[&str]) -> String {
    let value = run_output_optional("git", args).unwrap_or_default();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_string()
    }
}

fn receipt_json(record: &ReceiptRecord, git: &BTreeMap<String, String>) -> String {
    let branch = git.get("branch").map(String::as_str).unwrap_or("unknown");
    let commit = git.get("commit").map(String::as_str).unwrap_or("unknown");
    let mut body = String::from("{\n");
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str(&format!(
        "  \"command\": \"{}\",\n",
        json_escape(&record.command)
    ));
    body.push_str(&format!(
        "  \"status\": \"{}\",\n",
        json_escape(&record.status)
    ));
    body.push_str("  \"duration_ms\": 0,\n");
    body.push_str("  \"git\": {\n");
    body.push_str(&format!("    \"branch\": \"{}\",\n", json_escape(branch)));
    body.push_str(&format!("    \"commit\": \"{}\"\n", json_escape(commit)));
    body.push_str("  },\n");
    body.push_str("  \"reports\": [");
    write_json_string_array(&mut body, &record.reports);
    body.push_str("]\n");
    body.push_str("}\n");
    body
}

fn receipts_check_violations() -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    for spec in receipt_specs() {
        let path = receipts_dir().join(spec.file);
        if !path.exists() {
            violations.push(format!("missing receipt `{}`", normalize_path(&path)));
            continue;
        }
        let text = read_text_lossy(&path)?;
        if !text.contains("\"schema_version\": \"0.1\"") {
            violations.push(format!(
                "`{}` is missing schema_version 0.1",
                normalize_path(&path)
            ));
        }
        if !text.contains("\"command\"") {
            violations.push(format!("`{}` is missing command", normalize_path(&path)));
        }
        if !text.contains("\"status\"") {
            violations.push(format!("`{}` is missing status", normalize_path(&path)));
        }
        if !text.contains("\"git\"") {
            violations.push(format!(
                "`{}` is missing git metadata",
                normalize_path(&path)
            ));
        }
        if !text.contains("\"reports\"") {
            violations.push(format!(
                "`{}` is missing report paths",
                normalize_path(&path)
            ));
        }
        if let Some(status) = report_status_from_text(&text) {
            if !is_receipt_status(&status) {
                violations.push(format!(
                    "`{}` has unknown status `{status}`",
                    normalize_path(&path)
                ));
            }
        } else {
            violations.push(format!(
                "`{}` has no parseable status",
                normalize_path(&path)
            ));
        }
    }
    Ok(violations)
}

fn is_receipt_status(status: &str) -> bool {
    matches!(status, "passed" | "warn" | "failed" | "missing")
}

fn read_receipt_records() -> Vec<ReceiptRecord> {
    let mut records = Vec::new();
    for spec in receipt_specs() {
        let path = receipts_dir().join(spec.file);
        let status = if path.exists() {
            report_entry_status(&path)
        } else {
            "missing".to_string()
        };
        let reports = spec
            .reports
            .iter()
            .map(|report| format!("target/ripr/reports/{report}"))
            .collect::<Vec<_>>();
        records.push(ReceiptRecord {
            file: spec.file.to_string(),
            command: spec.command.to_string(),
            status,
            reports,
        });
    }
    records
}

fn receipts_report_markdown(
    status: &str,
    records: &[ReceiptRecord],
    violations: &[String],
) -> String {
    let mut body = format!("# ripr receipts report\n\nStatus: {status}\n\n");
    body.push_str("Receipts are machine-readable evidence for gate and report runs.\n\n");
    body.push_str("## Receipts\n\n");
    body.push_str("| Receipt | Command | Status |\n| --- | --- | --- |\n");
    for record in records {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` |\n",
            markdown_cell(&format!("target/ripr/receipts/{}", record.file)),
            markdown_cell(&record.command),
            markdown_cell(&record.status)
        ));
    }
    body.push_str("\n## Validation\n\n");
    if violations.is_empty() {
        body.push_str("- All required receipts are present and structurally valid.\n");
    } else {
        for violation in violations {
            body.push_str(&format!("- {violation}\n"));
        }
    }
    body
}

fn precommit_report_body() -> String {
    "# ripr precommit report\n\nStatus: pass\n\nChecks:\n\n- `cargo fmt --check`\n- `cargo xtask check-static-language`\n- `cargo xtask check-no-panic-family`\n- `cargo xtask check-allow-attributes`\n- `cargo xtask check-local-context`\n- `cargo xtask check-file-policy`\n- `cargo xtask check-executable-files`\n- `cargo xtask check-workflows`\n- `cargo xtask check-droid-review-config`\n- `cargo xtask check-spec-format`\n- `cargo xtask check-fixture-contracts`\n- `cargo xtask check-traceability`\n- `cargo xtask check-capabilities`\n- `cargo xtask check-workspace-shape`\n- `cargo xtask check-architecture`\n- `cargo xtask check-public-api`\n- `cargo xtask check-output-contracts`\n- `cargo xtask check-doc-index`\n- `cargo xtask check-readme-state`\n- `cargo xtask markdown-links`\n- `cargo xtask check-campaign`\n- `cargo xtask check-pr-shape`\n- `cargo xtask check-generated`\n\nNext command:\n\n```bash\ncargo xtask check-pr\n```\n".to_string()
}

fn check_pr_report_body() -> String {
    "# ripr check-pr report\n\nStatus: pass\n\nChecks:\n\n- `cargo xtask ci-fast`\n- `cargo clippy --workspace --all-targets -- -D warnings`\n- `cargo doc --workspace --no-deps`\n- `cargo xtask pr-summary`\n\nReports:\n\n- `target/ripr/reports/pr-summary.md`\n- `target/ripr/reports/check-pr.md`\n\nRelease/package gates are intentionally left to `cargo xtask ci-full` or release-specific workflows.\n".to_string()
}

pub(crate) fn fixtures_impl(name: Option<&String>) -> Result<(), String> {
    let fixture_dirs = fixture_dirs()?;
    let selected = match name {
        Some(value) => vec![fixture_dir_for_name(value)?],
        None => fixture_dirs,
    };
    let mut violations = Vec::new();
    let mut runs = Vec::new();
    for path in &selected {
        if !path.exists() {
            violations.push(format!("fixture does not exist: {}", normalize_path(path)));
            continue;
        }
        if !path.is_dir() {
            violations.push(format!(
                "fixture is not a directory: {}",
                normalize_path(path)
            ));
            continue;
        }
        let contract_violations = fixture_contract_violations(path)?;
        if contract_violations.is_empty() {
            match run_fixture(path) {
                Ok(run) => {
                    violations.extend(run.comparison_violations());
                    runs.push(run);
                }
                Err(err) => violations.push(err),
            }
        } else {
            violations.extend(contract_violations);
        }
    }

    let body = fixture_report_body(name.map(String::as_str), &selected, &runs, &violations);
    write_report("fixtures.md", &body)?;

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "fixture command failed; see target/ripr/reports/fixtures.md\n{}",
            violations.join("\n")
        ))
    }
}

pub(crate) fn goldens_impl(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("check") => goldens_check(),
        Some("bless") => {
            let Some(name) = args.get(1) else {
                return Err(
                    "goldens bless requires a fixture name\nusage: cargo xtask goldens bless <name> --reason <reason>"
                        .to_string(),
                );
            };
            let reason = parse_reason(&args[2..])?;
            goldens_bless(name, &reason)
        }
        Some(other) => Err(format!(
            "unknown goldens command `{other}`\nusage: cargo xtask goldens check\n       cargo xtask goldens bless <name> --reason <reason>"
        )),
        None => Err(
            "missing goldens command\nusage: cargo xtask goldens check\n       cargo xtask goldens bless <name> --reason <reason>"
                .to_string(),
        ),
    }
}

fn goldens_check() -> Result<(), String> {
    let run_set = collect_golden_runs()?;
    write_golden_drift_reports(&run_set.runs, &run_set.violations)?;
    let body = goldens_check_report_body(&run_set.fixtures, &run_set.runs, &run_set.violations);
    write_report("goldens.md", &body)?;
    if run_set.violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "goldens check failed; see target/ripr/reports/goldens.md\n{}",
            run_set.violations.join("\n")
        ))
    }
}

pub(crate) fn golden_drift_impl() -> Result<(), String> {
    let run_set = collect_golden_runs()?;
    write_golden_drift_reports(&run_set.runs, &run_set.violations)?;
    if run_set
        .violations
        .iter()
        .any(|violation| !violation.contains("drift for fixture"))
    {
        Err(format!(
            "golden drift report had fixture errors; see target/ripr/reports/golden-drift.md\n{}",
            run_set.violations.join("\n")
        ))
    } else {
        Ok(())
    }
}

fn collect_golden_runs() -> Result<GoldenRunSet, String> {
    let fixture_dirs = fixture_dirs()?;
    let mut violations = Vec::new();
    let mut runs = Vec::new();
    for fixture in &fixture_dirs {
        let contract_violations = fixture_contract_violations(fixture)?;
        if !contract_violations.is_empty() {
            violations.extend(contract_violations);
            continue;
        }
        match run_fixture(fixture) {
            Ok(run) => {
                violations.extend(run.comparison_violations());
                runs.push(run);
            }
            Err(err) => violations.push(err),
        }
    }
    Ok(GoldenRunSet {
        fixtures: fixture_dirs,
        runs,
        violations,
    })
}

fn goldens_bless(name: &str, reason: &str) -> Result<(), String> {
    let fixture = fixture_dir_for_name(name)?;
    if !fixture.exists() {
        return Err(format!(
            "fixture does not exist: {}",
            normalize_path(&fixture)
        ));
    }
    let run = run_fixture_outputs(&fixture)?;
    let expected = fixture.join("expected");
    fs::create_dir_all(&expected)
        .map_err(|err| format!("failed to create {}: {err}", normalize_path(&expected)))?;
    fs::copy(&run.check_json, expected.join("check.json")).map_err(|err| {
        format!(
            "failed to update {} from {}: {err}",
            normalize_path(&expected.join("check.json")),
            normalize_path(&run.check_json)
        )
    })?;
    fs::copy(&run.human_txt, expected.join("human.txt")).map_err(|err| {
        format!(
            "failed to update {} from {}: {err}",
            normalize_path(&expected.join("human.txt")),
            normalize_path(&run.human_txt)
        )
    })?;
    let changelog = expected.join("CHANGELOG.md");
    let entry = format!(
        "\n## Pending\n\nReason:\n{reason}\n\nCommand:\n`cargo xtask goldens bless {name} --reason \"...\"`\n\nUpdated:\n- `expected/check.json`\n- `expected/human.txt`\n"
    );
    let mut text = if changelog.exists() {
        read_text_lossy(&changelog)?
    } else {
        "# Golden Output Changes\n".to_string()
    };
    text.push_str(&entry);
    fs::write(&changelog, text)
        .map_err(|err| format!("failed to write {}: {err}", normalize_path(&changelog)))?;
    let body = format!(
        "# ripr goldens bless report\n\nStatus: pass\n\nFixture:\n- `{}`\n\nReason:\n```text\n{reason}\n```\n\nActual outputs:\n- `{}`\n- `{}`\n\nUpdated:\n- `{}`\n- `{}`\n- `{}`\n",
        normalize_path(&fixture),
        normalize_path(&run.check_json),
        normalize_path(&run.human_txt),
        normalize_path(&expected.join("check.json")),
        normalize_path(&expected.join("human.txt")),
        normalize_path(&changelog)
    );
    write_report("goldens-bless.md", &body)
}

fn fixture_dirs() -> Result<Vec<PathBuf>, String> {
    let fixtures_dir = Path::new("fixtures");
    if !fixtures_dir.exists() {
        return Ok(Vec::new());
    }
    let mut fixtures = Vec::new();
    for entry in
        fs::read_dir(fixtures_dir).map_err(|err| format!("failed to read fixtures: {err}"))?
    {
        let entry = entry.map_err(|err| format!("failed to read fixtures: {err}"))?;
        let path = entry.path();
        if path.is_dir() {
            fixtures.push(path);
        }
    }
    fixtures.sort();
    Ok(fixtures)
}

fn fixture_dir_for_name(name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err(format!("invalid fixture name `{name}`"));
    }
    Ok(Path::new("fixtures").join(name))
}

fn fixture_contract_violations(path: &Path) -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    let normalized = normalize_path(path);
    let spec = path.join("SPEC.md");
    let diff = path.join("diff.patch");
    let expected_check = path.join("expected/check.json");

    if !spec.exists() {
        violations.push(format!("{normalized} is missing SPEC.md"));
        return Ok(violations);
    }
    if !diff.exists() {
        violations.push(format!("{normalized} is missing diff.patch"));
    }
    if !expected_check.exists() {
        violations.push(format!("{normalized} is missing expected/check.json"));
    }

    let text = read_text_lossy(&spec)?;
    if !text
        .lines()
        .any(|line| line.starts_with("Spec: RIPR-SPEC-"))
    {
        violations.push(format!(
            "{} is missing `Spec: RIPR-SPEC-NNNN`",
            normalize_path(&spec)
        ));
    }
    for heading in ["## Given", "## When", "## Then", "## Must Not"] {
        if !has_markdown_heading(&text, heading) {
            violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
        }
    }
    Ok(violations)
}

#[derive(Debug)]
struct FixtureRun {
    name: String,
    actual_dir: PathBuf,
    check_json: PathBuf,
    human_txt: PathBuf,
    comparisons: Vec<GoldenComparison>,
}

impl FixtureRun {
    fn comparison_violations(&self) -> Vec<String> {
        self.comparisons
            .iter()
            .filter(|comparison| !comparison.matches)
            .map(|comparison| {
                let difference_hint = comparison
                    .first_difference
                    .as_ref()
                    .map(|hint| format!("\n  diff:    {hint}"))
                    .unwrap_or_default();
                format!(
                    "{} drift for fixture `{}`\n  expected: {}\n  actual:   {}{}",
                    comparison.surface,
                    self.name,
                    normalize_path(&comparison.expected),
                    normalize_path(&comparison.actual),
                    difference_hint
                )
            })
            .collect()
    }
}

#[derive(Debug)]
struct GoldenComparison {
    surface: &'static str,
    expected: PathBuf,
    actual: PathBuf,
    matches: bool,
    first_difference: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GoldenDriftSemantics {
    added_finding_ids: Vec<String>,
    removed_finding_ids: Vec<String>,
    changed_exposure_classes: Vec<String>,
    changed_probe_families: Vec<String>,
    changed_oracle_kinds: Vec<String>,
    changed_oracle_strengths: Vec<String>,
    changed_stop_reasons: Vec<String>,
    changed_recommendations: Vec<String>,
    static_language_terms: Vec<String>,
    changed_line_count: usize,
}

#[derive(Clone, Debug)]
struct GoldenDriftEntry {
    fixture: String,
    surface: String,
    expected: String,
    actual: String,
    blessing_reason_required: bool,
    blessing_reason_present: bool,
    semantics: GoldenDriftSemantics,
}

#[derive(Debug)]
struct GoldenRunSet {
    fixtures: Vec<PathBuf>,
    runs: Vec<FixtureRun>,
    violations: Vec<String>,
}

fn run_fixture(path: &Path) -> Result<FixtureRun, String> {
    let run = run_fixture_outputs(path)?;
    let expected = path.join("expected");
    let comparisons = fixture_golden_comparisons(&expected, &run.check_json, &run.human_txt)?;
    Ok(FixtureRun { comparisons, ..run })
}

fn run_fixture_outputs(path: &Path) -> Result<FixtureRun, String> {
    let name = fixture_name(path)?;
    let diff = path.join("diff.patch");
    let input = path.join("input");
    if !diff.exists() {
        return Err(format!("{} is missing diff.patch", normalize_path(path)));
    }
    if !input.exists() {
        return Err(format!(
            "{} is missing input/ fixture workspace",
            normalize_path(path)
        ));
    }

    let actual_dir = Path::new("target")
        .join("ripr")
        .join("fixtures")
        .join(&name);
    fs::create_dir_all(&actual_dir).map_err(|err| {
        format!(
            "failed to create actual fixture output directory {}: {err}",
            normalize_path(&actual_dir)
        )
    })?;

    let check_json = actual_dir.join("check.json");
    let human_txt = actual_dir.join("human.txt");
    let root = normalize_path(&input);
    let diff_file = normalize_path(&diff);

    let json = normalize_fixture_json_output(&run_fixture_check(&root, &diff_file, true)?);
    fs::write(&check_json, json).map_err(|err| {
        format!(
            "failed to write actual fixture output {}: {err}",
            normalize_path(&check_json)
        )
    })?;

    let human = normalize_fixture_human_output(&run_fixture_check(&root, &diff_file, false)?);
    fs::write(&human_txt, human).map_err(|err| {
        format!(
            "failed to write actual fixture output {}: {err}",
            normalize_path(&human_txt)
        )
    })?;

    Ok(FixtureRun {
        name,
        actual_dir,
        check_json,
        human_txt,
        comparisons: Vec::new(),
    })
}

fn run_fixture_check(root: &str, diff_file: &str, json: bool) -> Result<String, String> {
    let mut args = vec![
        "run".to_string(),
        "-p".to_string(),
        "ripr".to_string(),
        "--".to_string(),
        "check".to_string(),
        "--root".to_string(),
        root.to_string(),
        "--diff".to_string(),
        diff_file.to_string(),
        "--mode".to_string(),
        "fast".to_string(),
    ];
    if json {
        args.push("--json".to_string());
    }
    run_output_owned("cargo", &args)
}

fn fixture_golden_comparisons(
    expected: &Path,
    check_json: &Path,
    human_txt: &Path,
) -> Result<Vec<GoldenComparison>, String> {
    let mut comparisons = Vec::new();
    comparisons.push(compare_golden(
        "check.json",
        &expected.join("check.json"),
        check_json,
    )?);

    let expected_human = expected.join("human.txt");
    if expected_human.exists() {
        comparisons.push(compare_golden("human.txt", &expected_human, human_txt)?);
    }
    Ok(comparisons)
}

fn compare_golden(
    surface: &'static str,
    expected: &Path,
    actual: &Path,
) -> Result<GoldenComparison, String> {
    let expected_text = read_text_lossy(expected)?;
    let actual_text = read_text_lossy(actual)?;
    let normalized_expected = normalize_golden_text(&expected_text);
    let normalized_actual = normalize_golden_text(&actual_text);
    Ok(GoldenComparison {
        surface,
        expected: expected.to_path_buf(),
        actual: actual.to_path_buf(),
        matches: normalized_expected == normalized_actual,
        first_difference: first_line_difference(&normalized_expected, &normalized_actual),
    })
}

fn normalize_golden_text(value: &str) -> String {
    let normalized = value.replace("\r\n", "\n");
    normalized
        .strip_suffix('\n')
        .unwrap_or(&normalized)
        .to_string()
}

fn first_line_difference(expected: &str, actual: &str) -> Option<String> {
    let expected_lines: Vec<&str> = expected.split('\n').collect();
    let actual_lines: Vec<&str> = actual.split('\n').collect();
    let max_len = expected_lines.len().max(actual_lines.len());

    for index in 0..max_len {
        let expected_line = expected_lines.get(index).copied().unwrap_or("<missing>");
        let actual_line = actual_lines.get(index).copied().unwrap_or("<missing>");
        if expected_line != actual_line {
            return Some(format!(
                "line {} expected `{}` vs actual `{}`",
                index + 1,
                snapshot_line_preview(expected_line),
                snapshot_line_preview(actual_line)
            ));
        }
    }

    None
}

fn snapshot_line_preview(line: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 120;

    if line == "<missing>" {
        return line.to_string();
    }

    let escaped = line.escape_debug().to_string().replace('`', "\\`");
    let mut preview: String = escaped.chars().take(MAX_PREVIEW_CHARS).collect();
    if escaped.chars().count() > MAX_PREVIEW_CHARS {
        preview.push('…');
    }
    preview
}

fn normalize_fixture_json_output(value: &str) -> String {
    value.replace("\\\\", "/")
}

fn normalize_fixture_human_output(value: &str) -> String {
    let normalized = value.replace('\\', "/");
    let trimmed = normalized.trim_end_matches(['\r', '\n']);
    let mut output = trimmed.to_string();
    output.push('\n');
    output
}

fn fixture_name(path: &Path) -> Result<String, String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("invalid fixture path {}", normalize_path(path)))
}

fn fixture_report_body(
    name: Option<&str>,
    selected: &[PathBuf],
    runs: &[FixtureRun],
    violations: &[String],
) -> String {
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let mut body = format!("# ripr fixtures report\n\nStatus: {status}\n\n");
    match name {
        Some(value) => body.push_str(&format!("Requested fixture: `{value}`\n\n")),
        None => body.push_str("Requested fixture: all fixtures\n\n"),
    }
    body.push_str("## Fixtures\n\n");
    if selected.is_empty() {
        body.push_str("No fixture directories found.\n\n");
    } else {
        for path in selected {
            body.push_str(&format!("- `{}`\n", normalize_path(path)));
        }
        body.push('\n');
    }
    write_fixture_runs_section(&mut body, runs);
    write_violations_section(&mut body, violations);
    body
}

fn goldens_check_report_body(
    fixtures: &[PathBuf],
    runs: &[FixtureRun],
    violations: &[String],
) -> String {
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let mut body = format!("# ripr goldens report\n\nStatus: {status}\n\n");
    body.push_str("## Fixtures\n\n");
    if fixtures.is_empty() {
        body.push_str("No fixture directories found.\n\n");
    } else {
        for fixture in fixtures {
            body.push_str(&format!("- `{}`\n", normalize_path(fixture)));
        }
        body.push('\n');
    }
    write_fixture_runs_section(&mut body, runs);
    write_violations_section(&mut body, violations);
    body
}

fn write_golden_drift_reports(runs: &[FixtureRun], violations: &[String]) -> Result<(), String> {
    let changed_paths = collect_changed_paths_set().unwrap_or_default();
    let entries = golden_drift_entries(runs, &changed_paths)?;
    let markdown = golden_drift_markdown(&entries, violations);
    let json = golden_drift_json(&entries, violations);
    write_report("golden-drift.md", &markdown)?;
    write_report("golden-drift.json", &json)
}

fn golden_drift_entries(
    runs: &[FixtureRun],
    changed_paths: &BTreeSet<String>,
) -> Result<Vec<GoldenDriftEntry>, String> {
    let mut entries = Vec::new();
    for run in runs {
        let changelog = Path::new("fixtures")
            .join(&run.name)
            .join("expected")
            .join("CHANGELOG.md");
        let blessing_reason_present = changed_paths.contains(&normalize_path(&changelog));
        for comparison in &run.comparisons {
            if comparison.matches {
                continue;
            }
            let expected = read_text_lossy(&comparison.expected)?;
            let actual = read_text_lossy(&comparison.actual)?;
            entries.push(GoldenDriftEntry {
                fixture: run.name.clone(),
                surface: comparison.surface.to_string(),
                expected: normalize_path(&comparison.expected),
                actual: normalize_path(&comparison.actual),
                blessing_reason_required: true,
                blessing_reason_present,
                semantics: golden_drift_semantics(comparison.surface, &expected, &actual),
            });
        }
    }
    Ok(entries)
}

fn golden_drift_semantics(surface: &str, expected: &str, actual: &str) -> GoldenDriftSemantics {
    let changed_line_count = changed_line_count(expected, actual);
    let static_language_terms = static_language_terms(expected, actual);
    if surface == "check.json" {
        let expected_ids = json_string_values_for_key(expected, "id")
            .into_iter()
            .filter(|value| value.starts_with("probe:"))
            .collect::<BTreeSet<_>>();
        let actual_ids = json_string_values_for_key(actual, "id")
            .into_iter()
            .filter(|value| value.starts_with("probe:"))
            .collect::<BTreeSet<_>>();
        let expected_classes = json_string_values_for_key(expected, "classification");
        let actual_classes = json_string_values_for_key(actual, "classification");
        let expected_families = json_string_values_for_key(expected, "family");
        let actual_families = json_string_values_for_key(actual, "family");
        let expected_oracles = json_string_values_for_key(expected, "oracle_strength");
        let actual_oracles = json_string_values_for_key(actual, "oracle_strength");
        let expected_oracle_kinds = json_string_values_for_key(expected, "oracle_kind");
        let actual_oracle_kinds = json_string_values_for_key(actual, "oracle_kind");
        let expected_stop_reasons = json_stop_reason_values(expected);
        let actual_stop_reasons = json_stop_reason_values(actual);
        let expected_recommendations =
            json_string_values_for_key(expected, "recommended_next_step");
        let actual_recommendations = json_string_values_for_key(actual, "recommended_next_step");

        GoldenDriftSemantics {
            added_finding_ids: set_difference(&actual_ids, &expected_ids),
            removed_finding_ids: set_difference(&expected_ids, &actual_ids),
            changed_exposure_classes: set_change_summary(&expected_classes, &actual_classes),
            changed_probe_families: set_change_summary(&expected_families, &actual_families),
            changed_oracle_kinds: set_change_summary(&expected_oracle_kinds, &actual_oracle_kinds),
            changed_oracle_strengths: set_change_summary(&expected_oracles, &actual_oracles),
            changed_stop_reasons: set_change_summary(&expected_stop_reasons, &actual_stop_reasons),
            changed_recommendations: set_change_summary(
                &expected_recommendations,
                &actual_recommendations,
            ),
            static_language_terms,
            changed_line_count,
        }
    } else {
        let expected_recommendations = human_recommended_next_steps(expected);
        let actual_recommendations = human_recommended_next_steps(actual);
        let expected_stop_reasons = human_stop_reason_lines(expected);
        let actual_stop_reasons = human_stop_reason_lines(actual);
        GoldenDriftSemantics {
            changed_stop_reasons: set_change_summary(&expected_stop_reasons, &actual_stop_reasons),
            changed_recommendations: set_change_summary(
                &expected_recommendations,
                &actual_recommendations,
            ),
            static_language_terms,
            changed_line_count,
            ..GoldenDriftSemantics::default()
        }
    }
}

fn golden_drift_markdown(entries: &[GoldenDriftEntry], violations: &[String]) -> String {
    let status = if violations
        .iter()
        .any(|violation| !violation.contains("drift for fixture"))
    {
        "fail"
    } else if entries.is_empty() {
        "pass"
    } else {
        "warn"
    };
    let mut body = format!("# ripr golden drift report\n\nStatus: {status}\n\n");
    body.push_str("This report summarizes expected-output drift for reviewer inspection. It never blesses goldens.\n\n");
    body.push_str("## Summary\n\n");
    body.push_str(&format!("- drift entries: {}\n", entries.len()));
    body.push_str(&format!(
        "- fixture errors: {}\n",
        fixture_error_count(violations)
    ));
    body.push_str("\n## Drift\n\n");
    if entries.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for entry in entries {
            body.push_str(&format!("### `{}` `{}`\n\n", entry.fixture, entry.surface));
            body.push_str(&format!("- expected: `{}`\n", entry.expected));
            body.push_str(&format!("- actual: `{}`\n", entry.actual));
            body.push_str(&format!(
                "- changed lines: {}\n",
                entry.semantics.changed_line_count
            ));
            write_optional_list(
                &mut body,
                "added finding IDs",
                &entry.semantics.added_finding_ids,
            );
            write_optional_list(
                &mut body,
                "removed finding IDs",
                &entry.semantics.removed_finding_ids,
            );
            write_optional_list(
                &mut body,
                "changed exposure classes",
                &entry.semantics.changed_exposure_classes,
            );
            write_optional_list(
                &mut body,
                "changed probe families",
                &entry.semantics.changed_probe_families,
            );
            write_optional_list(
                &mut body,
                "changed oracle strengths",
                &entry.semantics.changed_oracle_strengths,
            );
            write_optional_list(
                &mut body,
                "changed oracle kinds",
                &entry.semantics.changed_oracle_kinds,
            );
            write_optional_list(
                &mut body,
                "changed stop reasons",
                &entry.semantics.changed_stop_reasons,
            );
            write_optional_list(
                &mut body,
                "changed recommended next steps",
                &entry.semantics.changed_recommendations,
            );
            if entry.semantics.static_language_terms.is_empty() {
                body.push_str("- static-language boundary: pass\n");
            } else {
                body.push_str("- static-language boundary: fail\n");
                write_optional_list(
                    &mut body,
                    "static-language terms",
                    &entry.semantics.static_language_terms,
                );
            }
            body.push_str(&format!(
                "- blessing reason required: {}\n",
                yes_no(entry.blessing_reason_required)
            ));
            body.push_str(&format!(
                "- blessing reason present in PR: {}\n\n",
                yes_no(entry.blessing_reason_present)
            ));
        }
    }
    if !violations.is_empty() {
        body.push_str("## Fixture / Golden Violations\n\n");
        write_violations_section(&mut body, violations);
    }
    body
}

fn golden_drift_json(entries: &[GoldenDriftEntry], violations: &[String]) -> String {
    let status = if violations
        .iter()
        .any(|violation| !violation.contains("drift for fixture"))
    {
        "fail"
    } else if entries.is_empty() {
        "pass"
    } else {
        "warn"
    };
    let mut body = String::from("{\n");
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str(&format!("  \"status\": \"{}\",\n", json_escape(status)));
    body.push_str(&format!("  \"drift_count\": {},\n", entries.len()));
    body.push_str("  \"entries\": [\n");
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"fixture\": \"{}\",\n",
            json_escape(&entry.fixture)
        ));
        body.push_str(&format!(
            "      \"surface\": \"{}\",\n",
            json_escape(&entry.surface)
        ));
        body.push_str(&format!(
            "      \"expected\": \"{}\",\n",
            json_escape(&entry.expected)
        ));
        body.push_str(&format!(
            "      \"actual\": \"{}\",\n",
            json_escape(&entry.actual)
        ));
        body.push_str(&format!(
            "      \"changed_line_count\": {},\n",
            entry.semantics.changed_line_count
        ));
        body.push_str(&format!(
            "      \"blessing_reason_required\": {},\n",
            entry.blessing_reason_required
        ));
        body.push_str(&format!(
            "      \"blessing_reason_present\": {},\n",
            entry.blessing_reason_present
        ));
        write_json_field_array(
            &mut body,
            "added_finding_ids",
            &entry.semantics.added_finding_ids,
            true,
        );
        write_json_field_array(
            &mut body,
            "removed_finding_ids",
            &entry.semantics.removed_finding_ids,
            true,
        );
        write_json_field_array(
            &mut body,
            "changed_exposure_classes",
            &entry.semantics.changed_exposure_classes,
            true,
        );
        write_json_field_array(
            &mut body,
            "changed_probe_families",
            &entry.semantics.changed_probe_families,
            true,
        );
        write_json_field_array(
            &mut body,
            "changed_oracle_strengths",
            &entry.semantics.changed_oracle_strengths,
            true,
        );
        write_json_field_array(
            &mut body,
            "changed_oracle_kinds",
            &entry.semantics.changed_oracle_kinds,
            true,
        );
        write_json_field_array(
            &mut body,
            "changed_stop_reasons",
            &entry.semantics.changed_stop_reasons,
            true,
        );
        write_json_field_array(
            &mut body,
            "changed_recommendations",
            &entry.semantics.changed_recommendations,
            true,
        );
        write_json_field_array(
            &mut body,
            "static_language_terms",
            &entry.semantics.static_language_terms,
            false,
        );
        body.push_str("\n    }");
    }
    body.push_str("\n  ],\n");
    body.push_str("  \"violations\": [");
    write_json_string_array(&mut body, violations);
    body.push_str("]\n");
    body.push_str("}\n");
    body
}

fn write_fixture_runs_section(body: &mut String, runs: &[FixtureRun]) {
    body.push_str("## Actual Outputs\n\n");
    if runs.is_empty() {
        body.push_str("No fixture outputs generated.\n\n");
        return;
    }
    for run in runs {
        body.push_str(&format!(
            "- `{}` -> `{}`\n",
            run.name,
            normalize_path(&run.actual_dir)
        ));
        body.push_str(&format!("  - `{}`\n", normalize_path(&run.check_json)));
        body.push_str(&format!("  - `{}`\n", normalize_path(&run.human_txt)));
    }
    body.push('\n');

    body.push_str("## Golden Comparisons\n\n");
    for run in runs {
        if run.comparisons.is_empty() {
            body.push_str(&format!(
                "- `{}`: no expected outputs compared.\n",
                run.name
            ));
            continue;
        }
        for comparison in &run.comparisons {
            let status = if comparison.matches { "pass" } else { "fail" };
            body.push_str(&format!(
                "- `{}` `{}`: {status}\n  - expected: `{}`\n  - actual: `{}`\n",
                run.name,
                comparison.surface,
                normalize_path(&comparison.expected),
                normalize_path(&comparison.actual)
            ));
        }
    }
    body.push('\n');
}

fn write_optional_list(body: &mut String, label: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    body.push_str(&format!("- {label}:\n"));
    for value in values {
        body.push_str(&format!("  - `{}`\n", markdown_cell(value)));
    }
}

fn write_json_field_array(body: &mut String, key: &str, values: &[String], trailing_comma: bool) {
    body.push_str(&format!("      \"{}\": [", json_escape(key)));
    write_json_string_array(body, values);
    body.push(']');
    if trailing_comma {
        body.push(',');
    }
    body.push('\n');
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn fixture_error_count(violations: &[String]) -> usize {
    violations
        .iter()
        .filter(|violation| !violation.contains("drift for fixture"))
        .count()
}

fn changed_line_count(expected: &str, actual: &str) -> usize {
    let expected_lines = normalize_golden_text(expected)
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let actual_lines = normalize_golden_text(actual)
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let max_len = expected_lines.len().max(actual_lines.len());
    let mut changed = 0usize;
    for index in 0..max_len {
        if expected_lines.get(index) != actual_lines.get(index) {
            changed += 1;
        }
    }
    changed
}

fn static_language_terms(expected: &str, actual: &str) -> Vec<String> {
    let combined = format!("{expected}\n{actual}").to_ascii_lowercase();
    forbidden_static_terms()
        .into_iter()
        .filter(|term| contains_word(&combined, term))
        .collect()
}

fn set_difference(left: &BTreeSet<String>, right: &BTreeSet<String>) -> Vec<String> {
    left.difference(right).cloned().collect()
}

fn set_change_summary(expected: &BTreeSet<String>, actual: &BTreeSet<String>) -> Vec<String> {
    if expected == actual {
        Vec::new()
    } else {
        vec![format!(
            "expected [{}] -> actual [{}]",
            expected.iter().cloned().collect::<Vec<_>>().join(", "),
            actual.iter().cloned().collect::<Vec<_>>().join(", ")
        )]
    }
}

fn json_stop_reason_values(text: &str) -> BTreeSet<String> {
    let mut values = json_string_values_for_key(text, "stop_reason");
    values.extend(json_string_values_for_key(text, "stop_reasons"));
    values
}

fn json_string_values_for_key(text: &str, key: &str) -> BTreeSet<String> {
    let needle = format!("\"{key}\"");
    let mut values = BTreeSet::new();
    let mut multiline = String::new();
    let mut collecting = false;
    for line in text.lines() {
        if collecting {
            multiline.push(' ');
            multiline.push_str(line.trim());
            if line.contains(']') {
                values.extend(json_strings_in_fragment(&multiline));
                multiline.clear();
                collecting = false;
            }
            continue;
        }
        let Some((_, rest)) = line.split_once(&needle) else {
            continue;
        };
        let Some((_, value)) = rest.split_once(':') else {
            continue;
        };
        let trimmed = value.trim();
        if trimmed.starts_with('[') && !trimmed.contains(']') {
            multiline.push_str(trimmed);
            collecting = true;
        } else {
            values.extend(json_strings_in_fragment(trimmed));
        }
    }
    values
}

fn json_strings_in_fragment(value: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut chars = value.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch != '"' {
            continue;
        }
        let mut current = String::new();
        let mut escaped = false;
        for (_, next) in chars.by_ref() {
            if escaped {
                current.push(match next {
                    '"' => '"',
                    '\\' => '\\',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    other => other,
                });
                escaped = false;
                continue;
            }
            if next == '\\' {
                escaped = true;
                continue;
            }
            if next == '"' {
                strings.push(current);
                break;
            }
            current.push(next);
        }
    }
    strings
}

fn human_recommended_next_steps(text: &str) -> BTreeSet<String> {
    human_section_lines(text, "Recommended next step:")
}

fn human_stop_reason_lines(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("stop reason") || lower.contains("stop:")
        })
        .map(str::to_string)
        .collect()
}

fn human_section_lines(text: &str, heading: &str) -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    let mut capture = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if capture {
            if trimmed.is_empty() {
                capture = false;
            } else {
                values.insert(trimmed.to_string());
            }
            continue;
        }
        if trimmed == heading {
            capture = true;
        }
    }
    values
}

fn collect_changed_paths_set() -> Result<BTreeSet<String>, String> {
    Ok(collect_pr_changes()?
        .into_iter()
        .map(|change| change.path)
        .collect())
}

fn write_violations_section(body: &mut String, violations: &[String]) {
    body.push_str("## Violations\n\n");
    if violations.is_empty() {
        body.push_str("None detected.\n");
    } else {
        for violation in violations {
            body.push_str("```text\n");
            body.push_str(violation);
            body.push_str("\n```\n\n");
        }
    }
}

fn parse_reason(args: &[String]) -> Result<String, String> {
    let mut index = 0;
    while index < args.len() {
        let value = &args[index];
        if let Some(reason) = value.strip_prefix("--reason=") {
            return non_empty_reason(reason);
        }
        if value == "--reason" {
            let Some(reason) = args.get(index + 1) else {
                return Err("--reason requires a value".to_string());
            };
            return non_empty_reason(reason);
        }
        index += 1;
    }
    Err("goldens bless requires --reason <reason>".to_string())
}

fn non_empty_reason(value: &str) -> Result<String, String> {
    let reason = value.trim();
    if reason.is_empty() {
        Err("--reason must not be empty".to_string())
    } else {
        Ok(reason.to_string())
    }
}

fn finish_policy_report(spec: PolicyReportSpec<'_>, violations: &[String]) -> Result<(), String> {
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let mut body = format!("# {}\n\nStatus: {status}\n\n", spec.check);
    body.push_str("## Why This Matters\n\n");
    body.push_str(spec.why_it_matters);
    body.push_str("\n\n");

    if violations.is_empty() {
        body.push_str("## Violations\n\nNone detected.\n\n");
    } else {
        body.push_str("## Violations\n\n");
        for violation in violations {
            body.push_str("```text\n");
            body.push_str(violation);
            body.push_str("\n```\n\n");
        }
    }

    if !violations.is_empty() {
        body.push_str("## Fix Kind\n\n```text\n");
        body.push_str(fix_kind_name(&spec.fix_kind));
        body.push_str("\n```\n\n");

        body.push_str("## Recommended Fixes\n\n");
        for (index, fix) in spec.recommended_fixes.iter().enumerate() {
            body.push_str(&format!("{}. {fix}\n", index + 1));
        }
        body.push('\n');

        if let Some(template) = spec.exception_template {
            body.push_str("## Exception Template\n\n```text\n");
            body.push_str(template);
            body.push_str("\n```\n\n");
        }
    }

    body.push_str("## Rerun\n\n```bash\n");
    body.push_str(spec.rerun_command);
    body.push_str("\n```\n");

    write_report(spec.report_file, &body)?;

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} failed; see target/ripr/reports/{}\n{}",
            spec.check,
            spec.report_file,
            violations.join("\n")
        ))
    }
}

fn fix_kind_name(fix_kind: &FixKind) -> &'static str {
    match fix_kind {
        FixKind::AutoFixable => "auto_fixable",
        FixKind::AuthorDecisionRequired => "author_decision_required",
        FixKind::ReviewerDecisionRequired => "reviewer_decision_required",
        FixKind::PolicyExceptionRequired => "policy_exception_required",
    }
}

fn check_static_language_impl() -> Result<(), String> {
    let report_spec = PolicyReportSpec {
        report_file: "static-language.md",
        check: "check-static-language",
        why_it_matters: "Static output must preserve the boundary between draft exposure evidence and real mutation results.",
        fix_kind: FixKind::ReviewerDecisionRequired,
        recommended_fixes: &[
            "Rewrite static product output to use the approved exposure vocabulary.",
            "If this is explanatory documentation, add a reasoned `[[allow]]` entry to the static-language allowlist.",
        ],
        rerun_command: "cargo xtask check-static-language",
        exception_template: Some(
            ".ripr/static-language-allowlist.toml entry:\n[[allow]]\npath = \"path/to/file.md\"\nowner = \"team\"\nreason = \"why this file may quote prohibited vocabulary\"",
        ),
    };

    let allowed = match load_static_language_allowlist() {
        Ok(entries) => entries,
        Err(violations) => return finish_policy_report(report_spec, &violations),
    };
    let forbidden = forbidden_static_terms();
    let mut violations = Vec::new();

    for path in collect_files(Path::new("."))? {
        let normalized = normalize_path(&path);
        if !should_scan_static_language_path(&allowed, &normalized) {
            continue;
        }
        let text = read_text_lossy(&path)?;
        for (line_number, line) in text.lines().enumerate() {
            let lower = line.to_ascii_lowercase();
            for term in &forbidden {
                if contains_word(&lower, term) {
                    violations.push(format!(
                        "{normalized}:{} contains prohibited static-language term `{term}`",
                        line_number + 1
                    ));
                }
            }
        }
    }

    finish_policy_report(report_spec, &violations)
}

fn check_no_panic_family_impl() -> Result<(), String> {
    check_old_panic_allowlist_exists()?;

    let roots = [
        Path::new("crates/ripr/src"),
        Path::new("crates/ripr/tests"),
        Path::new("xtask/src"),
    ];
    let patterns = forbidden_panic_patterns();

    let mut findings = Vec::new();
    for root in roots {
        if !root.exists() {
            continue;
        }
        findings.extend(collect_panic_findings(root, &patterns)?);
    }

    // Determine schema version
    let allowlist_path = ".ripr/no-panic-allowlist.toml";
    let has_v02 = if Path::new(allowlist_path).exists() {
        let text = read_text_lossy(Path::new(allowlist_path))?;
        text.lines()
            .any(|line| line.trim() == "schema_version = \"0.2\"")
    } else {
        false
    };

    let mut violations = Vec::new();
    let mut advisories = Vec::new();

    if has_v02 {
        // v0.2 mode: use semantic findings and versioned parser
        let mut semantic_findings = Vec::new();
        for root in roots {
            if !root.exists() {
                continue;
            }
            semantic_findings.extend(collect_semantic_panic_findings(root, &patterns)?);
        }

        let versioned_entries = if Path::new(allowlist_path).exists() {
            parse_no_panic_allowlist_toml_v2(allowlist_path)?
        } else {
            Vec::new()
        };

        // Check each semantic finding against the allowlist
        for finding in &semantic_findings {
            let mut matched = false;
            for entry in &versioned_entries {
                match entry {
                    PanicAllowEntryVersioned::V2(v2) => {
                        if v2.path != finding.path || v2.family != finding.family {
                            continue;
                        }
                        if let Some(ref selector) = v2.selector
                            && semantic_selector_matches(selector, finding)
                        {
                            // Check last_seen drift
                            if let Some(ref ls) = v2.last_seen
                                && (ls.line != finding.line || ls.column != finding.column)
                            {
                                advisories.push(format!(
                                    "allowed by semantic selector; last_seen changed from line {} to line {} ({}:{}:{})",
                                    ls.line,
                                    finding.line,
                                    finding.path,
                                    finding.line,
                                    finding.column.unwrap_or(0),
                                ));
                            }
                            matched = true;
                            break;
                        }
                    }
                    PanicAllowEntryVersioned::V1(v1) => {
                        // For v0.1 entries in v0.2 file, try to match by finding a semantic finding
                        // that overlaps the v0.1 location
                        if v1.path == finding.path
                            && v1.family == finding.family
                            && v1.line == finding.line
                            && (v1.column.is_none() || v1.column == finding.column)
                        {
                            matched = true;
                            break;
                        }
                    }
                }
            }
            if !matched {
                violations.push(format!(
                    "{}:{}:{} contains unallowed panic-family '{}'; add exact allowlist entry with explanation",
                    finding.path,
                    finding.line,
                    finding.column.unwrap_or(0),
                    finding.family
                ));
            }
        }

        // Check for stale v0.1 entries
        for entry in &versioned_entries {
            match entry {
                PanicAllowEntryVersioned::V1(v1) => {
                    let matched = semantic_findings.iter().any(|f| {
                        f.path == v1.path
                            && f.family == v1.family
                            && f.line == v1.line
                            && (v1.column.is_none() || v1.column == f.column)
                    });
                    if !matched {
                        violations.push(format!(
                            "stale allowlist entry: {}:{}:{:?} ({}) does not match any current finding",
                            v1.path, v1.line, v1.column, v1.family
                        ));
                    }
                }
                PanicAllowEntryVersioned::V2(v2) => {
                    if let Some(ref selector) = v2.selector {
                        let matched = semantic_findings.iter().any(|f| {
                            v2.path == f.path
                                && v2.family == f.family
                                && semantic_selector_matches(selector, f)
                        });
                        if !matched {
                            violations.push(format!(
                                "stale v0.2 allowlist entry: {} ({}) classification={:?} [{}] selector does not match any current finding",
                                v2.path, v2.family, v2.classification, v2.explanation
                            ));
                        }
                    }
                }
            }
        }
    } else {
        // v0.1 mode: existing behavior
        let allowlist = if Path::new(allowlist_path).exists() {
            parse_no_panic_allowlist_toml(allowlist_path)?
        } else {
            Vec::new()
        };

        for finding in &findings {
            let matched = allowlist.iter().any(|e| {
                e.path == finding.path
                    && e.line == finding.line
                    && e.family == finding.family
                    && (e.column.is_none() || e.column == finding.column)
            });
            if !matched {
                violations.push(format!(
                    "{}:{}:{} contains unallowed panic-family '{}'; add exact allowlist entry with explanation",
                    finding.path,
                    finding.line,
                    finding.column.unwrap_or(0),
                    finding.family
                ));
            }
        }

        for entry in &allowlist {
            let matched = findings.iter().any(|f| {
                f.path == entry.path
                    && f.line == entry.line
                    && f.family == entry.family
                    && (entry.column.is_none() || entry.column == f.column)
            });
            if !matched {
                violations.push(format!(
                    "stale allowlist entry: {}:{}:{:?} ({}) does not match any current finding",
                    entry.path, entry.line, entry.column, entry.family
                ));
            }
        }
    }

    for advisory in &advisories {
        println!("advisory: {advisory}");
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "no-panic-family.md",
            check: "check-no-panic-family",
            why_it_matters: "Product and test code should surface failures explicitly instead of relying on panic-family shortcuts.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Return `Result` and propagate setup or IO failures.",
                "Pattern-match `Option` values and return explicit errors in tests.",
                "Use an allowlist entry only for reviewed legacy debt or intentional string detection.",
            ],
            rerun_command: "cargo xtask check-no-panic-family",
            exception_template: Some(
                ".ripr/no-panic-allowlist.toml entry:\n[[allow]]\npath = \"path/to/file.rs\"\nline = 123\ncolumn = 17\nfamily = \"unwrap\"\nexplanation = \"Human-readable reason\"",
            ),
        },
        &violations,
    )
}

fn check_allow_attributes_impl() -> Result<(), String> {
    let allowlist = read_count_allowlist(".ripr/allow-attributes.txt")?;
    let guarded = guarded_allow_attribute_lints();
    let mut counts = BTreeMap::<(String, String), Vec<usize>>::new();

    for path in tracked_files()? {
        if !path.ends_with(".rs") {
            continue;
        }
        let file_path = Path::new(&path);
        if !file_path.exists() {
            continue;
        }
        let text = read_text_lossy(file_path)?;
        for (line, attribute) in guarded_allow_attributes_in_text(&text, &guarded) {
            counts
                .entry((path.clone(), attribute))
                .or_default()
                .push(line);
        }
    }

    let mut violations = Vec::new();
    for ((path, attribute), lines) in &counts {
        let allowed = allowlist
            .get(&(path.clone(), attribute.clone()))
            .copied()
            .unwrap_or(0);
        if lines.len() > allowed {
            violations.push(format!(
                "{path}:{} contains `{attribute}` {} time(s), allowed {allowed}\n  preferred: fix the lint or add a narrow allowlist entry with a reason",
                allow_attribute_line_summary(lines),
                lines.len()
            ));
        }
    }

    for ((path, attribute), allowed) in &allowlist {
        if !guarded.contains(attribute_lint_name(attribute).unwrap_or(attribute)) {
            violations.push(format!(
                ".ripr/allow-attributes.txt contains unsupported guarded attribute `{attribute}` for {path}; remove stale or out-of-scope exceptions"
            ));
            continue;
        }
        let actual = counts
            .get(&(path.clone(), attribute.clone()))
            .map(Vec::len)
            .unwrap_or(0);
        if actual > *allowed {
            violations.push(format!(
                "{path} contains `{attribute}` {actual} time(s), allowed {allowed}"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "allow-attributes.md",
            check: "check-allow-attributes",
            why_it_matters: "Lint suppressions should not be used to hide repo guardrails. If a suppression is unavoidable, it needs a narrow reviewed exception with a reason.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Remove the lint suppression and fix the underlying warning.",
                "If the suppression is temporary and intentional, add a narrow allowlist entry with a reason.",
                "Do not allowlist panic-family, unsafe, or broad warning suppressions unless the PR explicitly owns that exception.",
            ],
            rerun_command: "cargo xtask check-allow-attributes",
            exception_template: Some(
                ".ripr/allow-attributes.txt entry:\npath/to/file.rs|allow(clippy::unwrap_used)|1|reason",
            ),
        },
        &violations,
    )
}

fn check_local_context_impl() -> Result<(), String> {
    let allowlist = read_local_context_allowlist("policy/local_context_allowlist.txt")?;
    let mut violations = validate_local_context_allowlist(&allowlist);
    let mut grouped = BTreeMap::<(String, String), (BTreeSet<String>, Vec<Option<usize>>)>::new();

    for path in tracked_files()? {
        let file_path = Path::new(&path);
        if !file_path.exists() || path == "policy/local_context_allowlist.txt" {
            continue;
        }

        for finding in local_context_findings_for_path(&path)? {
            let entry = grouped
                .entry((finding.path.clone(), finding.pattern.clone()))
                .or_insert_with(|| (BTreeSet::new(), Vec::new()));
            entry.0.insert(finding.problem);
            entry.1.push(finding.line);
        }
    }

    let allowed = allowlist
        .iter()
        .map(|entry| ((entry.path.clone(), entry.pattern.clone()), entry.max_count))
        .collect::<BTreeMap<_, _>>();

    for ((path, pattern), (problems, lines)) in grouped {
        let actual = lines.len();
        let allowed_count = allowed
            .get(&(path.clone(), pattern.clone()))
            .copied()
            .unwrap_or(0);
        if actual <= allowed_count {
            continue;
        }
        let line_summary = local_context_line_summary(&lines);
        violations.push(format!(
            "Path: {path}\nProblem: {}\nPattern: {pattern}\nCount: {actual}, allowed: {allowed_count}\nLines: {line_summary}\nWhy this matters: Repository docs should contain durable project state, not local runtime/session state from one machine or Codex run.\nRecommended fixes:\n1. Delete runtime/session artifacts instead of committing them.\n2. Move durable learnings to docs/LEARNINGS.md.\n3. Move generated state to target/ripr/reports, target/ripr/receipts, or target/ripr/learning.",
            problems.into_iter().collect::<Vec<_>>().join("; ")
        ));
    }

    write_local_context_json(&violations)?;
    finish_policy_report(
        PolicyReportSpec {
            report_file: "local-context.md",
            check: "check-local-context",
            why_it_matters: "Repository state must be durable and portable. Machine paths, Codex memory paths, sandbox references, local transcripts, and session-state documents belong in generated artifacts or local notes, not committed repo knowledge.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Delete committed runtime/session artifacts.",
                "Move durable repo knowledge to docs/LEARNINGS.md or campaign/capability metadata.",
                "Move generated state to target/ripr/reports, target/ripr/receipts, or target/ripr/learning.",
                "Use policy/local_context_allowlist.txt only for narrow generic examples with a reason.",
            ],
            rerun_command: "cargo xtask check-local-context",
            exception_template: Some(
                "policy/local_context_allowlist.txt entry:\npath|pattern|max_count|reason",
            ),
        },
        &violations,
    )
}

fn check_file_policy_impl() -> Result<(), String> {
    let allowlist = read_glob_allowlist("policy/non_rust_allowlist.txt")?;
    let mut violations = Vec::new();

    for path in collect_files(Path::new("."))? {
        let normalized = normalize_path(&path);
        if !is_file_policy_candidate(&normalized) {
            continue;
        }
        if normalized.ends_with(".rs") {
            continue;
        }
        if !matches_any_glob(&allowlist, &normalized) {
            violations.push(format!(
                "unapproved non-Rust programming/declarative file: {normalized}\n  preferred: implement automation in Rust/xtask or add a policy allowlist entry with owner and reason"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "file-policy.md",
            check: "check-file-policy",
            why_it_matters: "Rust and xtask are the default implementation surface so repo automation stays typed, tested, and reviewable.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Move implementation or automation logic into Rust/xtask.",
                "If the file belongs to an approved surface, add an allowlist entry with owner and reason.",
            ],
            rerun_command: "cargo xtask check-file-policy",
            exception_template: Some(
                "policy/non_rust_allowlist.txt entry:\nglob|kind|owner|reason",
            ),
        },
        &violations,
    )
}

fn check_executable_files_impl() -> Result<(), String> {
    let allowlist = read_path_allowlist_optional("policy/executable_allowlist.txt")?;
    let output = run_output("git", &["ls-files", "--stage"])?;
    let mut violations = Vec::new();

    for line in output.lines() {
        let Some((mode, path)) = parse_git_stage_line(line) else {
            continue;
        };
        let normalized = normalize_slashes(path);
        if mode == "100755" && !allowlist.contains(&normalized) {
            violations.push(format!(
                "checked-in executable file is not allowlisted: {normalized}\n  preferred: use cargo xtask instead of executable scripts"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "executable-files.md",
            check: "check-executable-files",
            why_it_matters: "Checked-in executable scripts make automation drift away from the Rust-first xtask surface.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Remove the executable bit from ordinary files.",
                "Move script behavior into xtask.",
                "If an executable file is truly required, add a reviewed allowlist entry.",
            ],
            rerun_command: "cargo xtask check-executable-files",
            exception_template: Some("policy/executable_allowlist.txt entry:\npath/to/file"),
        },
        &violations,
    )
}

fn check_workflows_impl() -> Result<(), String> {
    let budgets = read_workflow_budgets("policy/workflow_allowlist.txt")?;
    let runtime_allowlist = read_count_allowlist("policy/workflow_action_runtime_allowlist.txt")?;
    let mut violations = Vec::new();

    for path in collect_files(Path::new(".github/workflows"))? {
        let normalized = normalize_path(&path);
        if !(normalized.ends_with(".yml") || normalized.ends_with(".yaml")) {
            continue;
        }
        let Some(budget) = budgets.get(&normalized) else {
            violations.push(format!(
                "missing workflow budget for {normalized} in policy/workflow_allowlist.txt"
            ));
            continue;
        };
        let text = read_text_lossy(&path)?;
        violations.extend(workflow_runtime_violations(
            &normalized,
            &text,
            &runtime_allowlist,
        ));
        for block in extract_workflow_run_blocks(&text) {
            if block.non_empty_lines > budget.max_non_empty_lines {
                violations.push(format!(
                    "{normalized}:{} run block has {} non-empty line(s), allowed {} ({})",
                    block.line_number,
                    block.non_empty_lines,
                    budget.max_non_empty_lines,
                    budget.reason
                ));
            }
            let lower = block.text.to_ascii_lowercase();
            if lower.contains(shell_fetch_tool_name()) && lower.contains("| sh") {
                violations.push(format!(
                    "{normalized}:{} run block contains network fetch piped to sh",
                    block.line_number
                ));
            }
            if lower.contains(shell_fetch_tool_name()) && lower.contains("| bash") {
                violations.push(format!(
                    "{normalized}:{} run block contains network fetch piped to bash",
                    block.line_number
                ));
            }
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "workflows.md",
            check: "check-workflows",
            why_it_matters: "GitHub Actions should orchestrate xtask, Cargo, and npm commands instead of hiding complex shell logic in workflow YAML.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Move complex workflow logic into xtask or an npm script owned by the extension surface.",
                "Keep workflow run blocks under the documented line budget.",
                "Use Node-24-backed action majors where official releases exist.",
                "Use Node 24 for VS Code extension build and publish workflows.",
                "Add or adjust a workflow budget entry only when the workflow surface is intentionally larger.",
            ],
            rerun_command: "cargo xtask check-workflows",
            exception_template: Some(
                "policy/workflow_allowlist.txt entry:\n.github/workflows/name.yml|max_non_empty_lines|reason\n\npolicy/workflow_action_runtime_allowlist.txt entry:\n.github/workflows/name.yml|action/ref|max_count|reason",
            ),
        },
        &violations,
    )
}

fn workflow_runtime_violations(
    path: &str,
    text: &str,
    allowlist: &BTreeMap<(String, String), usize>,
) -> Vec<String> {
    let mut violations = Vec::new();
    for (old_ref, new_ref) in deprecated_workflow_action_refs() {
        let count = text.matches(old_ref).count();
        if count > 0 {
            violations.push(format!(
                "{path} uses deprecated action runtime ref `{old_ref}` {count} time(s); use `{new_ref}`"
            ));
        }
    }

    if is_extension_node_workflow(path) {
        for (line_number, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if matches!(
                trimmed,
                "node-version: 20" | "node-version: '20'" | "node-version: \"20\""
            ) {
                violations.push(format!(
                    "{path}:{} uses Node 20 for extension tooling; use Node 24",
                    line_number + 1
                ));
            }
        }
    }

    for pattern in workflow_runtime_exception_patterns() {
        let count = text.matches(pattern).count();
        if count == 0 {
            continue;
        }
        let allowed = allowlist
            .get(&(path.to_string(), pattern.to_string()))
            .copied()
            .unwrap_or(0);
        if count > allowed {
            violations.push(format!(
                "{path} uses `{pattern}` {count} time(s), allowed {allowed}; add a reviewed workflow action runtime exception or upgrade the action"
            ));
        }
    }

    for ((allowed_path, pattern), allowed) in allowlist {
        if allowed_path != path {
            continue;
        }
        if !workflow_runtime_exception_patterns().contains(&pattern.as_str()) {
            violations.push(format!(
                "policy/workflow_action_runtime_allowlist.txt has unsupported exception `{pattern}` for {allowed_path}"
            ));
            continue;
        }
        let count = text.matches(pattern).count();
        if count > *allowed {
            violations.push(format!(
                "{path} uses `{pattern}` {count} time(s), allowed {allowed}"
            ));
        }
    }

    violations.sort();
    violations.dedup();
    violations
}

fn deprecated_workflow_action_refs() -> &'static [(&'static str, &'static str)] {
    &[
        ("actions/checkout@v4", "actions/checkout@v6"),
        ("actions/setup-node@v4", "actions/setup-node@v6"),
        ("actions/upload-artifact@v4", "actions/upload-artifact@v7"),
        (
            "actions/download-artifact@v4",
            "actions/download-artifact@v8",
        ),
        ("codecov/codecov-action@v4", "codecov/codecov-action@v6"),
    ]
}

fn workflow_runtime_exception_patterns() -> &'static [&'static str] {
    &["actions/dependency-review-action@v4"]
}

fn is_extension_node_workflow(path: &str) -> bool {
    matches!(
        path,
        ".github/workflows/ci.yml" | ".github/workflows/publish-extension.yml"
    )
}

fn check_spec_format() -> Result<(), String> {
    let mut violations = Vec::new();
    let spec_dir = Path::new("docs/specs");
    for path in collect_files(spec_dir)? {
        let normalized = normalize_path(&path);
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !file_name.starts_with("RIPR-SPEC-") || !file_name.ends_with(".md") {
            continue;
        }
        let Some(spec_id) = spec_id_from_file_name(file_name) else {
            violations.push(format!("{normalized} has invalid RIPR-SPEC filename"));
            continue;
        };
        let text = read_text_lossy(&path)?;
        let first_line = text.lines().next().unwrap_or_default();
        if !first_line.starts_with(&format!("# {spec_id}: ")) {
            violations.push(format!(
                "{normalized}:1 title must start with `# {spec_id}: `"
            ));
        }
        let status = spec_status(&text);
        match status.as_deref() {
            Some("proposed" | "planned" | "accepted" | "deprecated") => {}
            Some(value) => violations.push(format!("{normalized} has invalid status `{value}`")),
            None => violations.push(format!("{normalized} is missing `Status: ...`")),
        }
        for heading in required_spec_headings() {
            if !has_markdown_heading(&text, heading) {
                violations.push(format!("{normalized} is missing `{heading}`"));
            }
        }
        if text.contains("- \n") {
            violations.push(format!(
                "{normalized} contains empty placeholder list items"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "spec-format.md",
            check: "check-spec-format",
            why_it_matters: "Specs are the behavior contracts that let humans and agents trace intent to tests, code, outputs, and metrics.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Update the spec to match docs/SPEC_FORMAT.md.",
                "Use docs/templates/SPEC_TEMPLATE.md for new behavior specs.",
                "Keep planned specs explicit when implementation mapping is not available yet.",
            ],
            rerun_command: "cargo xtask check-spec-format",
            exception_template: None,
        },
        &violations,
    )
}

fn check_fixture_contracts() -> Result<(), String> {
    let fixtures_dir = Path::new("fixtures");
    if !fixtures_dir.exists() {
        return finish_policy_report(
            PolicyReportSpec {
                report_file: "fixture-contracts.md",
                check: "check-fixture-contracts",
                why_it_matters: "Fixtures are the BDD control bench for analyzer behavior and output contracts.",
                fix_kind: FixKind::AuthorDecisionRequired,
                recommended_fixes: &[
                    "Add fixture directories only with SPEC.md, diff.patch, and expected/check.json.",
                    "Use Given/When/Then/Must Not sections for agent-readable fixture intent.",
                ],
                rerun_command: "cargo xtask check-fixture-contracts",
                exception_template: None,
            },
            &[],
        );
    }

    let mut violations = Vec::new();
    for entry in
        fs::read_dir(fixtures_dir).map_err(|err| format!("failed to read fixtures: {err}"))?
    {
        let entry = entry.map_err(|err| format!("failed to read fixtures: {err}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let normalized = normalize_path(&path);
        let spec = path.join("SPEC.md");
        let diff = path.join("diff.patch");
        let expected_check = path.join("expected/check.json");

        if !spec.exists() {
            violations.push(format!("{normalized} is missing SPEC.md"));
            continue;
        }
        if !diff.exists() {
            violations.push(format!("{normalized} is missing diff.patch"));
        }
        if !expected_check.exists() {
            violations.push(format!("{normalized} is missing expected/check.json"));
        }

        let text = read_text_lossy(&spec)?;
        if !text
            .lines()
            .any(|line| line.starts_with("Spec: RIPR-SPEC-"))
        {
            violations.push(format!(
                "{} is missing `Spec: RIPR-SPEC-NNNN`",
                normalize_path(&spec)
            ));
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !has_markdown_heading(&text, heading) {
                violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
            }
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "fixture-contracts.md",
            check: "check-fixture-contracts",
            why_it_matters: "Fixtures are the BDD control bench for analyzer behavior and output contracts.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Add missing fixture contract files.",
                "Use Given/When/Then/Must Not sections in fixture SPEC.md.",
                "Keep expected output files aligned with the fixture behavior.",
            ],
            rerun_command: "cargo xtask check-fixture-contracts",
            exception_template: None,
        },
        &violations,
    )
}

fn check_traceability() -> Result<(), String> {
    let manifest = Path::new(".ripr/traceability.toml");
    let mut violations = Vec::new();
    if !manifest.exists() {
        violations.push(".ripr/traceability.toml is missing".to_string());
        return finish_traceability_report(&violations);
    }

    let (behaviors, parse_violations) = parse_traceability_manifest(manifest)?;
    violations.extend(parse_violations);
    if behaviors.is_empty() {
        violations.push(".ripr/traceability.toml has no [[behavior]] entries".to_string());
    }

    let specs = collect_spec_statuses()?;
    let mut behavior_ids = BTreeSet::new();
    for behavior in &behaviors {
        validate_trace_behavior(behavior, &mut behavior_ids, &mut violations)?;
    }

    for spec_id in specs.keys() {
        if !behavior_ids.contains(spec_id) {
            violations.push(format!(
                "{spec_id} exists in docs/specs but is missing from .ripr/traceability.toml"
            ));
        }
    }

    validate_fixture_spec_references(&specs, &mut violations)?;
    finish_traceability_report(&violations)
}

fn finish_traceability_report(violations: &[String]) -> Result<(), String> {
    finish_policy_report(
        PolicyReportSpec {
            report_file: "traceability.md",
            check: "check-traceability",
            why_it_matters: "Traceability keeps behavior specs, tests, fixtures, code, outputs, and metrics discoverable for long-context human and agent work.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Add or update the matching [[behavior]] entry in .ripr/traceability.toml.",
                "Keep every docs/specs/RIPR-SPEC-*.md file represented in the manifest.",
                "Use valid RIPR-SPEC-NNNN IDs in specs, fixtures, and manifest entries.",
                "List only paths that exist, or leave planned fields empty until the artifact exists.",
            ],
            rerun_command: "cargo xtask check-traceability",
            exception_template: None,
        },
        violations,
    )
}

fn validate_trace_behavior(
    behavior: &TraceBehavior,
    behavior_ids: &mut BTreeSet<String>,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let Some(id) = behavior.id.as_ref() else {
        violations.push(format!(
            "behavior at line {} is missing `id`",
            behavior.line
        ));
        return Ok(());
    };
    if !is_spec_id(id) {
        violations.push(format!(
            "behavior at line {} has invalid spec id `{id}`",
            behavior.line
        ));
    }
    if !behavior_ids.insert(id.clone()) {
        violations.push(format!("duplicate traceability behavior id `{id}`"));
    }
    if behavior
        .name
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        violations.push(format!("{id} is missing a non-empty `name`"));
    }

    let spec_status = match behavior.spec.as_ref() {
        Some(spec) => validate_behavior_spec_path(id, spec, violations)?,
        None => {
            violations.push(format!("{id} is missing `spec`"));
            None
        }
    };

    validate_trace_paths(id, "tests", &behavior.tests, violations);
    validate_trace_paths(id, "fixtures", &behavior.fixtures, violations);
    validate_trace_paths(id, "code", &behavior.code, violations);
    validate_trace_paths(id, "outputs", &behavior.outputs, violations);

    if behavior.metrics.is_empty() {
        violations.push(format!("{id} has no metrics"));
    }
    for metric in &behavior.metrics {
        if metric.trim().is_empty() {
            violations.push(format!("{id} has an empty metric entry"));
        }
    }

    if spec_status.as_deref() == Some("accepted")
        && behavior.tests.is_empty()
        && behavior.fixtures.is_empty()
    {
        violations.push(format!(
            "{id} is accepted but has no current test or fixture mapping"
        ));
    }

    Ok(())
}

fn validate_behavior_spec_path(
    id: &str,
    spec: &str,
    violations: &mut Vec<String>,
) -> Result<Option<String>, String> {
    let path = Path::new(spec);
    if !path.exists() {
        violations.push(format!("{id} spec path does not exist: {spec}"));
        return Ok(None);
    }
    match spec_id_from_path(path) {
        Some(spec_id) if spec_id == id => {}
        Some(spec_id) => violations.push(format!(
            "{id} points at spec path with mismatched id {spec_id}: {spec}"
        )),
        None => violations.push(format!(
            "{id} spec path does not use RIPR-SPEC-NNNN filename: {spec}"
        )),
    }
    match spec_status_from_file(path)? {
        Some(status) => Ok(Some(status)),
        None => {
            violations.push(format!("{id} spec is missing `Status: ...`: {spec}"));
            Ok(None)
        }
    }
}

fn validate_trace_paths(id: &str, field: &str, values: &[String], violations: &mut Vec<String>) {
    for value in values {
        let path_text = trace_path_part(value);
        if path_text.trim().is_empty() {
            violations.push(format!("{id} has an empty `{field}` path"));
            continue;
        }
        if !Path::new(path_text).exists() {
            violations.push(format!("{id} `{field}` path does not exist: {path_text}"));
        }
    }
}

fn trace_path_part(value: &str) -> &str {
    match value.split_once("::") {
        Some((path, _)) => path,
        None => value,
    }
}

fn validate_fixture_spec_references(
    specs: &BTreeMap<String, String>,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    for fixture in fixture_dirs()? {
        let spec_path = fixture.join("SPEC.md");
        if !spec_path.exists() {
            continue;
        }
        let text = read_text_lossy(&spec_path)?;
        for line in text.lines() {
            let Some(value) = line.strip_prefix("Spec:") else {
                continue;
            };
            let spec_id = value.trim();
            if !is_spec_id(spec_id) {
                violations.push(format!(
                    "{} references invalid spec id `{spec_id}`",
                    normalize_path(&spec_path)
                ));
            } else if !specs.contains_key(spec_id) {
                violations.push(format!(
                    "{} references unknown spec id `{spec_id}`",
                    normalize_path(&spec_path)
                ));
            }
        }
    }
    Ok(())
}

fn collect_spec_statuses() -> Result<BTreeMap<String, String>, String> {
    let specs_dir = Path::new("docs/specs");
    let mut specs = BTreeMap::new();
    if !specs_dir.exists() {
        return Ok(specs);
    }
    for path in collect_files(specs_dir)? {
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let Some(spec_id) = spec_id_from_path(&path) else {
            continue;
        };
        let status = spec_status_from_file(&path)?.unwrap_or_else(|| "missing".to_string());
        specs.insert(spec_id, status);
    }
    Ok(specs)
}

fn spec_status_from_file(path: &Path) -> Result<Option<String>, String> {
    let text = read_text_lossy(path)?;
    for line in text.lines() {
        let Some(value) = line.strip_prefix("Status:") else {
            continue;
        };
        return Ok(Some(value.trim().to_string()));
    }
    Ok(None)
}

fn spec_id_from_path(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy();
    let mut parts = stem.split('-');
    let first = parts.next()?;
    let second = parts.next()?;
    let third = parts.next()?;
    if first == "RIPR" && second == "SPEC" && third.len() == 4 && is_ascii_digits(third) {
        Some(format!("RIPR-SPEC-{third}"))
    } else {
        None
    }
}

fn is_spec_id(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("RIPR-SPEC-") else {
        return false;
    };
    suffix.len() == 4 && is_ascii_digits(suffix)
}

fn is_ascii_digits(value: &str) -> bool {
    value.bytes().all(|byte| byte.is_ascii_digit())
}

fn parse_traceability_manifest(path: &Path) -> Result<(Vec<TraceBehavior>, Vec<String>), String> {
    let text = read_text_lossy(path)?;
    let mut behaviors = Vec::new();
    let mut violations = Vec::new();
    let mut current: Option<TraceBehavior> = None;
    let mut active_array: Option<(String, Vec<String>, usize)> = None;

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, values, start_line)) = active_array.as_mut() {
            if trimmed.starts_with(']') {
                let Some(mut behavior) = current.take() else {
                    violations.push(format!(
                        "{}:{} array `{key}` is outside a behavior entry",
                        normalize_path(path),
                        start_line
                    ));
                    active_array = None;
                    continue;
                };
                assign_trace_array(
                    &mut behavior,
                    key,
                    values.clone(),
                    *start_line,
                    &mut violations,
                );
                current = Some(behavior);
                active_array = None;
                continue;
            }
            match parse_array_item(trimmed) {
                Ok(Some(value)) => values.push(value),
                Ok(None) => {}
                Err(message) => {
                    violations.push(format!("{}:{line_number} {message}", normalize_path(path)))
                }
            }
            continue;
        }
        if trimmed == "[[behavior]]" {
            if let Some(behavior) = current.take() {
                behaviors.push(behavior);
            }
            current = Some(TraceBehavior {
                line: line_number,
                ..TraceBehavior::default()
            });
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            violations.push(format!(
                "{}:{line_number} expected `key = value`",
                normalize_path(path)
            ));
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let Some(behavior) = current.as_mut() else {
            violations.push(format!(
                "{}:{line_number} `{key}` appears outside a [[behavior]] entry",
                normalize_path(path)
            ));
            continue;
        };
        if value == "[" {
            active_array = Some((key.to_string(), Vec::new(), line_number));
            continue;
        }
        if value.starts_with('[') {
            match parse_inline_array(value) {
                Ok(values) => {
                    assign_trace_array(behavior, key, values, line_number, &mut violations)
                }
                Err(message) => {
                    violations.push(format!("{}:{line_number} {message}", normalize_path(path)))
                }
            }
            continue;
        }
        match parse_quoted_value(value) {
            Ok(parsed) => assign_trace_string(behavior, key, parsed, line_number, &mut violations),
            Err(message) => {
                violations.push(format!("{}:{line_number} {message}", normalize_path(path)))
            }
        }
    }

    if let Some((key, _, start_line)) = active_array {
        violations.push(format!(
            "{}:{start_line} array `{key}` is missing closing `]`",
            normalize_path(path)
        ));
    }
    if let Some(behavior) = current {
        behaviors.push(behavior);
    }
    Ok((behaviors, violations))
}

fn assign_trace_string(
    behavior: &mut TraceBehavior,
    key: &str,
    value: String,
    line_number: usize,
    violations: &mut Vec<String>,
) {
    match key {
        "id" => behavior.id = Some(value),
        "name" => behavior.name = Some(value),
        "spec" => behavior.spec = Some(value),
        _ => violations.push(format!(
            "traceability line {line_number} uses unsupported string field `{key}`"
        )),
    }
}

fn assign_trace_array(
    behavior: &mut TraceBehavior,
    key: &str,
    values: Vec<String>,
    line_number: usize,
    violations: &mut Vec<String>,
) {
    match key {
        "tests" => behavior.tests = values,
        "fixtures" => behavior.fixtures = values,
        "code" => behavior.code = values,
        "outputs" => behavior.outputs = values,
        "metrics" => behavior.metrics = values,
        _ => violations.push(format!(
            "traceability line {line_number} uses unsupported array field `{key}`"
        )),
    }
}

fn parse_inline_array(value: &str) -> Result<Vec<String>, String> {
    let trimmed = value.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err("expected string array".to_string());
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut values = Vec::new();
    for item in inner.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        values.push(parse_quoted_value(item)?);
    }
    Ok(values)
}

fn parse_array_item(value: &str) -> Result<Option<String>, String> {
    let trimmed = value.trim().trim_end_matches(',').trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        Ok(None)
    } else {
        parse_quoted_value(trimmed).map(Some)
    }
}

fn parse_quoted_value(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return Err(format!("expected quoted string, got `{trimmed}`"));
    }
    Ok(trimmed[1..trimmed.len() - 1].to_string())
}

pub(crate) fn metrics_report_impl() -> Result<(), String> {
    let (capabilities, violations) =
        parse_capabilities_manifest(Path::new("metrics/capabilities.toml"))?;
    if !violations.is_empty() {
        finish_capabilities_report(&violations)?;
        return Err(format!(
            "metrics source is invalid; see target/ripr/reports/capabilities.md\n{}",
            violations.join("\n")
        ));
    }
    write_report("metrics.md", &capability_metrics_markdown(&capabilities))?;
    write_report("metrics.json", &capability_metrics_json(&capabilities))
}

pub(crate) fn test_oracle_report_impl() -> Result<(), String> {
    let tests = collect_test_oracle_tests()?;
    write_report("test-oracles.md", &test_oracle_report_markdown(&tests))?;
    write_report("test-oracles.json", &test_oracle_report_json(&tests))
}

fn collect_test_oracle_tests() -> Result<Vec<TestOracleTest>, String> {
    let mut tests = Vec::new();
    for root in [
        Path::new("crates/ripr/src"),
        Path::new("crates/ripr/tests"),
        Path::new("xtask/src"),
    ] {
        if !root.exists() {
            continue;
        }
        for path in collect_files(root)? {
            if path.extension().and_then(|value| value.to_str()) != Some("rs") {
                continue;
            }
            let text = read_text_lossy(&path)?;
            tests.extend(test_oracle_tests_in_text(&path, &text));
        }
    }
    tests.sort_by(|left, right| {
        normalize_path(&left.path)
            .cmp(&normalize_path(&right.path))
            .then(left.line.cmp(&right.line))
            .then(left.name.cmp(&right.name))
    });
    Ok(tests)
}

fn test_oracle_tests_in_text(path: &Path, text: &str) -> Vec<TestOracleTest> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut tests = Vec::new();
    let mut pending_test_attr_line = None;
    let mut index = 0usize;

    while index < lines.len() {
        let trimmed = lines[index].trim();
        if is_test_attribute(trimmed) {
            pending_test_attr_line = Some(index + 1);
            index += 1;
            continue;
        }

        if let Some(attr_line) = pending_test_attr_line {
            if trimmed.is_empty() || trimmed.starts_with("#[") {
                index += 1;
                continue;
            }

            if let Some(name) = test_fn_name(trimmed) {
                let end = test_function_end(&lines, index);
                let observations = test_oracle_observations(&lines[index..=end], index + 1);
                let class = observations
                    .iter()
                    .map(|observation| observation.class)
                    .max_by_key(|class| class.rank())
                    .unwrap_or(TestOracleClass::Smoke);
                tests.push(TestOracleTest {
                    path: path.to_path_buf(),
                    name,
                    line: attr_line,
                    body_line: index + 1,
                    body: lines[index..=end].join("\n"),
                    class,
                    observations,
                });
                pending_test_attr_line = None;
                index = end + 1;
                continue;
            }

            if !trimmed.starts_with("//") {
                pending_test_attr_line = None;
            }
        }

        index += 1;
    }

    tests
}

fn is_test_attribute(trimmed: &str) -> bool {
    let compact = trimmed.replace(' ', "");
    if !compact.starts_with("#[") {
        return false;
    }
    compact == "#[test]"
        || compact.starts_with("#[tokio::test")
        || compact.starts_with("#[async_std::test")
        || compact.starts_with("#[rstest")
}

fn test_fn_name(trimmed: &str) -> Option<String> {
    let fn_pos = trimmed.find("fn ")?;
    let after_fn = &trimmed[fn_pos + 3..];
    let name = after_fn
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    if name.is_empty() { None } else { Some(name) }
}

fn test_function_end(lines: &[&str], start: usize) -> usize {
    let mut depth = 0isize;
    let mut saw_body = false;

    for (offset, line) in lines[start..].iter().enumerate() {
        for ch in line.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    saw_body = true;
                }
                '}' if saw_body => depth -= 1,
                _ => {}
            }
        }
        if saw_body && depth <= 0 {
            return start + offset;
        }
    }

    lines.len().saturating_sub(1)
}

fn test_oracle_observations(lines: &[&str], first_line: usize) -> Vec<TestOracleObservation> {
    let mut observations = Vec::new();
    for (offset, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        if let Some(observation) = test_oracle_observation(trimmed, first_line + offset) {
            observations.push(observation);
        }
    }

    if observations.is_empty() {
        observations.push(TestOracleObservation {
            line: first_line,
            class: TestOracleClass::Smoke,
            pattern: "no assertion".to_string(),
            detail: "test body has no detected assertion-like oracle".to_string(),
        });
    }

    observations
}

fn test_oracle_observation(trimmed: &str, line: usize) -> Option<TestOracleObservation> {
    if trimmed.is_empty() {
        return None;
    }

    if contains_any(trimmed, &["assert_eq!(", "assert_ne!(", "assert_matches!("]) {
        return Some(test_oracle_observation_for(
            line,
            TestOracleClass::Strong,
            "exact assertion",
            "exact equality, inequality, or variant assertion",
        ));
    }
    if trimmed.contains("matches!(") {
        return Some(test_oracle_observation_for(
            line,
            TestOracleClass::Strong,
            "matches!",
            "pattern assertion can discriminate an exact variant or shape",
        ));
    }
    if trimmed.contains("status.success()") {
        return Some(test_oracle_observation_for(
            line,
            TestOracleClass::Smoke,
            "status.success",
            "exit-status check proves execution but little behavior",
        ));
    }
    if contains_any(
        trimmed,
        &[
            ".is_ok()",
            ".is_err()",
            ".is_some()",
            ".is_none()",
            ".is_empty()",
            ".contains(",
            "contains(",
        ],
    ) {
        return Some(test_oracle_observation_for(
            line,
            TestOracleClass::Weak,
            "broad predicate",
            "broad predicate may miss changed behavior or exact discriminator drift",
        ));
    }
    if trimmed.contains("assert!(") && contains_any(trimmed, &[" == ", " != ", " >= ", " <= "]) {
        return Some(test_oracle_observation_for(
            line,
            TestOracleClass::Medium,
            "boolean comparison",
            "boolean comparison gives some discrimination without structured equality",
        ));
    }
    if trimmed.contains("assert!(") {
        return Some(test_oracle_observation_for(
            line,
            TestOracleClass::Weak,
            "generic assert",
            "generic boolean assertion needs review for discriminator strength",
        ));
    }

    None
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn test_oracle_observation_for(
    line: usize,
    class: TestOracleClass,
    pattern: &str,
    detail: &str,
) -> TestOracleObservation {
    TestOracleObservation {
        line,
        class,
        pattern: pattern.to_string(),
        detail: detail.to_string(),
    }
}

fn is_bdd_test_name(name: &str) -> bool {
    let compact = name.to_ascii_lowercase();
    if !compact.starts_with("given_") {
        return false;
    }

    let Some(when_index) = compact.find("_when_") else {
        return false;
    };
    let Some(then_index) = compact.find("_then_") else {
        return false;
    };

    when_index > "given_".len() && then_index > when_index + "_when_".len()
}

fn test_oracle_counts(tests: &[TestOracleTest]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::from([
        ("strong", 0usize),
        ("medium", 0usize),
        ("weak", 0usize),
        ("smoke", 0usize),
    ]);
    for test in tests {
        if let Some(count) = counts.get_mut(test.class.as_str()) {
            *count += 1;
        }
    }
    counts
}

fn test_oracle_report_status(tests: &[TestOracleTest]) -> &'static str {
    if tests
        .iter()
        .any(|test| matches!(test.class, TestOracleClass::Weak | TestOracleClass::Smoke))
    {
        "warn"
    } else {
        "pass"
    }
}

fn test_oracle_report_markdown(tests: &[TestOracleTest]) -> String {
    let counts = test_oracle_counts(tests);
    let bdd_named = tests
        .iter()
        .filter(|test| is_bdd_test_name(&test.name))
        .count();
    let mut body = format!(
        "# ripr test oracle report\n\nStatus: {}\n\nMode: advisory\n\nThis report measures the apparent discriminator strength of `ripr`'s own Rust tests. It does not fail existing debt yet.\n\n## Summary\n\n- Strong: {}\n- Medium: {}\n- Weak: {}\n- Smoke: {}\n- BDD-shaped names: {} / {}\n\n",
        test_oracle_report_status(tests),
        counts.get("strong").copied().unwrap_or(0),
        counts.get("medium").copied().unwrap_or(0),
        counts.get("weak").copied().unwrap_or(0),
        counts.get("smoke").copied().unwrap_or(0),
        bdd_named,
        tests.len(),
    );

    body.push_str("## Weak Or Smoke Tests\n\n");
    let weak_or_smoke = tests
        .iter()
        .filter(|test| matches!(test.class, TestOracleClass::Weak | TestOracleClass::Smoke))
        .collect::<Vec<_>>();
    if weak_or_smoke.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for test in weak_or_smoke {
            body.push_str(&format!(
                "- `{}`:{} `{}` classified `{}`\n",
                normalize_path(&test.path),
                test.line,
                test.name,
                test.class.as_str()
            ));
            for observation in &test.observations {
                body.push_str(&format!(
                    "  - line {}: `{}` - {}\n",
                    observation.line, observation.pattern, observation.detail
                ));
            }
        }
        body.push('\n');
    }

    body.push_str("## All Tests\n\n| Test | Class | Evidence |\n| --- | --- | --- |\n");
    for test in tests {
        let evidence = test
            .observations
            .iter()
            .map(|observation| format!("{}: {}", observation.line, observation.pattern))
            .collect::<Vec<_>>()
            .join("<br>");
        body.push_str(&format!(
            "| `{}`:{} `{}` | `{}` | {} |\n",
            normalize_path(&test.path),
            test.line,
            markdown_cell(&test.name),
            test.class.as_str(),
            markdown_cell(&evidence)
        ));
    }
    body
}

fn test_oracle_report_json(tests: &[TestOracleTest]) -> String {
    let counts = test_oracle_counts(tests);
    let mut body = format!(
        "{{\n  \"schema_version\": \"0.1\",\n  \"status\": \"{}\",\n  \"advisory\": true,\n  \"counts\": {{\n    \"strong\": {},\n    \"medium\": {},\n    \"weak\": {},\n    \"smoke\": {}\n  }},\n  \"tests\": [\n",
        test_oracle_report_status(tests),
        counts.get("strong").copied().unwrap_or(0),
        counts.get("medium").copied().unwrap_or(0),
        counts.get("weak").copied().unwrap_or(0),
        counts.get("smoke").copied().unwrap_or(0)
    );

    for (test_index, test) in tests.iter().enumerate() {
        if test_index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"path\": \"{}\",\n",
            json_escape(&normalize_path(&test.path))
        ));
        body.push_str(&format!(
            "      \"name\": \"{}\",\n",
            json_escape(&test.name)
        ));
        body.push_str(&format!("      \"line\": {},\n", test.line));
        body.push_str(&format!("      \"class\": \"{}\",\n", test.class.as_str()));
        body.push_str("      \"observations\": [\n");
        for (observation_index, observation) in test.observations.iter().enumerate() {
            if observation_index > 0 {
                body.push_str(",\n");
            }
            body.push_str("        {\n");
            body.push_str(&format!("          \"line\": {},\n", observation.line));
            body.push_str(&format!(
                "          \"class\": \"{}\",\n",
                observation.class.as_str()
            ));
            body.push_str(&format!(
                "          \"pattern\": \"{}\",\n",
                json_escape(&observation.pattern)
            ));
            body.push_str(&format!(
                "          \"detail\": \"{}\"\n",
                json_escape(&observation.detail)
            ));
            body.push_str("        }");
        }
        body.push_str("\n      ]\n    }");
    }
    body.push_str("\n  ]\n}\n");
    body
}

pub(crate) fn test_efficiency_report_impl() -> Result<(), String> {
    let tests = collect_test_oracle_tests()?;
    let mut entries = tests.iter().map(test_efficiency_entry).collect::<Vec<_>>();
    let duplicate_groups = apply_duplicate_discriminator_groups(&mut entries);
    let test_intent_summary = match load_test_intent_manifest() {
        Ok(declarations) => {
            let mut violations = validate_test_intent_paths_on_disk(&declarations);
            violations.extend(apply_test_intent_to_entries(&mut entries, &declarations));
            if !violations.is_empty() {
                return Err(format!(
                    "{TEST_INTENT_PATH} validation failed:\n{}",
                    violations.join("\n")
                ));
            }
            TestIntentReportSummary {
                declared: declarations.len(),
                matched: entries
                    .iter()
                    .filter(|e| e.declared_intent.is_some())
                    .count(),
            }
        }
        Err(violations) => {
            return Err(format!(
                "{TEST_INTENT_PATH} parse failed:\n{}",
                violations.join("\n")
            ));
        }
    };
    write_report(
        "test-efficiency.md",
        &test_efficiency_report_markdown(&entries, &duplicate_groups, &test_intent_summary),
    )?;
    write_report(
        "test-efficiency.json",
        &test_efficiency_report_json(&entries, &duplicate_groups, &test_intent_summary),
    )
}

fn test_efficiency_entry(test: &TestOracleTest) -> TestEfficiencyEntry {
    let reached_owners = test_efficiency_reached_owners(test);
    let observed_values = test_efficiency_observed_values(test);
    let reasons = test_efficiency_reasons(test, &reached_owners, &observed_values);
    let mut static_limitations = test_efficiency_static_limitations(test);
    if reached_owners.is_empty() {
        static_limitations.push(
            "no direct owner call detected; test may route through helpers, fixtures, or macros"
                .to_string(),
        );
    }
    if observed_values.is_empty() {
        static_limitations.push("no literal activation values detected".to_string());
    }

    TestEfficiencyEntry {
        path: test.path.clone(),
        name: test.name.clone(),
        line: test.line,
        class: test_efficiency_class(test, &reached_owners, &reasons),
        oracle_kind: test_efficiency_oracle_kind(test).to_string(),
        oracle_strength: test.class.as_str(),
        reached_owners,
        observed_values,
        reasons,
        static_limitations,
        duplicate_group_id: None,
        declared_intent: None,
    }
}

const DUPLICATE_DISCRIMINATOR_NEXT_STEP: &str = "Keep both if they document distinct business cases. Otherwise consider adding a different activation value or oracle shape.";

const DUPLICATE_ACTIVATION_AND_ORACLE_SHAPE_REASON: &str = "duplicate_activation_and_oracle_shape";

/// Groups eligible test-efficiency entries that share an owner set, an
/// activation signature, and an oracle shape. Mutates eligible entries in
/// place: their class becomes `"duplicative"`, the
/// `duplicate_activation_and_oracle_shape` reason is appended to their
/// reasons (preserving any existing reasons such as `smoke_oracle_only`),
/// and `duplicate_group_id` is set.
///
/// Eligibility is conservative: a test is eligible only if its base class is
/// `strong_discriminator`, `useful_but_broad`, or `smoke_only`. Tests already
/// classified `opaque`, `likely_vacuous`, or `possibly_circular` are kept on
/// their existing class because that signal is more actionable than
/// "duplicate." Tests with no observed activation literals are also excluded
/// — we cannot build a credible activation signature for them.
///
/// The activation signature is role-aware: it preserves the order and
/// `(context, value)` pairing of `observed_values`, so `score(2) == 3` and
/// `score(3) == 2` produce different signatures even though the raw value
/// set is identical.
///
/// In v1 the grouping key does not include explicit flow-sink evidence —
/// the test-efficiency ledger does not currently emit it. The role-aware
/// activation signature acts as a narrow proxy because the
/// `assertion_argument` context naturally captures the sink-side values of
/// the oracle. A future PR can promote explicit sink evidence into the
/// ledger and tighten the key.
fn apply_duplicate_discriminator_groups(
    entries: &mut [TestEfficiencyEntry],
) -> Vec<DuplicateDiscriminatorGroup> {
    let mut buckets: BTreeMap<DuplicateGroupKey, Vec<usize>> = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        if !is_duplicate_discriminator_eligible(entry) {
            continue;
        }
        let key = duplicate_discriminator_key(entry);
        buckets.entry(key).or_default().push(index);
    }

    let mut groups: Vec<(usize, DuplicateGroupKey, Vec<usize>)> = buckets
        .into_iter()
        .filter(|(_, members)| members.len() >= 2)
        .map(|(key, members)| {
            // Safe: filter above guarantees `members.len() >= 2`.
            let first = members[0];
            (first, key, members)
        })
        .collect();
    groups.sort_by_key(|(first, _, _)| *first);

    let mut rendered = Vec::with_capacity(groups.len());
    for (group_index, (_, key, members)) in groups.into_iter().enumerate() {
        let id = format!("duplicate_group_{}", group_index + 1);
        let group_members: Vec<DuplicateGroupMember> = members
            .iter()
            .map(|&i| DuplicateGroupMember {
                path: normalize_path(&entries[i].path),
                name: entries[i].name.clone(),
                line: entries[i].line,
            })
            .collect();
        for &i in &members {
            entries[i].class = "duplicative";
            if !entries[i]
                .reasons
                .iter()
                .any(|r| r == DUPLICATE_ACTIVATION_AND_ORACLE_SHAPE_REASON)
            {
                entries[i]
                    .reasons
                    .push(DUPLICATE_ACTIVATION_AND_ORACLE_SHAPE_REASON.to_string());
                entries[i].reasons.sort();
            }
            entries[i].duplicate_group_id = Some(id.clone());
        }
        let DuplicateGroupKey {
            owners,
            oracle_kind,
            oracle_strength,
            activation_signature,
        } = key;
        rendered.push(DuplicateDiscriminatorGroup {
            id,
            members: group_members,
            shared_evidence: DuplicateGroupSharedEvidence {
                owners,
                oracle_kind,
                oracle_strength,
                activation_signature: activation_signature
                    .into_iter()
                    .map(|(context, value)| DuplicateGroupActivation { context, value })
                    .collect(),
            },
            suggested_next_step: DUPLICATE_DISCRIMINATOR_NEXT_STEP.to_string(),
        });
    }
    rendered
}

fn is_duplicate_discriminator_eligible(entry: &TestEfficiencyEntry) -> bool {
    matches!(
        entry.class,
        "strong_discriminator" | "useful_but_broad" | "smoke_only"
    ) && !entry.reached_owners.is_empty()
        && !entry.observed_values.is_empty()
}

fn duplicate_discriminator_key(entry: &TestEfficiencyEntry) -> DuplicateGroupKey {
    let mut owners = entry.reached_owners.clone();
    owners.sort();
    owners.dedup();
    let activation_signature = entry
        .observed_values
        .iter()
        .map(|value| (value.context, value.value.clone()))
        .collect();
    DuplicateGroupKey {
        owners,
        oracle_kind: entry.oracle_kind.clone(),
        oracle_strength: entry.oracle_strength,
        activation_signature,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DuplicateGroupKey {
    owners: Vec<String>,
    oracle_kind: String,
    oracle_strength: &'static str,
    activation_signature: Vec<(&'static str, String)>,
}

const TEST_INTENT_PATH: &str = ".ripr/test_intent.toml";

/// Top-level summary of the test-intent layer rendered in both Markdown
/// and JSON. Always emitted, even when no manifest exists, so consumers
/// get a stable shape.
#[derive(Clone, Debug, Default)]
struct TestIntentReportSummary {
    declared: usize,
    matched: usize,
}

/// Parses the `.ripr/test_intent.toml` manifest text into declarations.
/// Returns the parsed declarations alongside any structural violations.
/// The parser is pure (no I/O) so it can be unit-tested directly.
fn parse_test_intent_manifest(text: &str) -> (Vec<TestIntentDeclaration>, Vec<String>) {
    let mut entries: Vec<TestIntentDeclaration> = Vec::new();
    let mut violations = Vec::new();
    let mut schema_seen = false;
    let mut current: Option<PendingTestIntent> = None;

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "[[test_intent]]" {
            if let Some(pending) = current.take() {
                finalize_test_intent_entry(pending, &mut entries, &mut violations);
            }
            current = Some(PendingTestIntent::new(line_number));
            continue;
        }
        let Some((key, raw_value)) = trimmed.split_once('=') else {
            violations.push(format!(
                "{TEST_INTENT_PATH}:{line_number} expected `key = value`"
            ));
            continue;
        };
        let key = key.trim();
        let raw_value = raw_value.trim();
        if let Some(pending) = current.as_mut() {
            match key {
                "test" => {
                    assign_test_intent_field(raw_value, line_number, &mut violations, |parsed| {
                        pending.test = Some((parsed, line_number))
                    })
                }
                "path" => {
                    assign_test_intent_field(raw_value, line_number, &mut violations, |parsed| {
                        pending.path = Some((parsed, line_number))
                    })
                }
                "intent" => {
                    assign_test_intent_field(raw_value, line_number, &mut violations, |parsed| {
                        pending.intent = Some((parsed, line_number))
                    })
                }
                "owner" => {
                    assign_test_intent_field(raw_value, line_number, &mut violations, |parsed| {
                        pending.owner = Some((parsed, line_number))
                    })
                }
                "reason" => {
                    assign_test_intent_field(raw_value, line_number, &mut violations, |parsed| {
                        pending.reason = Some((parsed, line_number))
                    })
                }
                _ => violations.push(format!(
                    "{TEST_INTENT_PATH}:{line_number} unsupported `[[test_intent]]` field `{key}`"
                )),
            }
        } else if key == "schema_version" {
            schema_seen = true;
            match raw_value.parse::<u32>() {
                Ok(1) => {}
                Ok(other) => violations.push(format!(
                    "{TEST_INTENT_PATH}:{line_number} schema_version = {other} is not supported (expected 1)"
                )),
                Err(_) => violations.push(format!(
                    "{TEST_INTENT_PATH}:{line_number} schema_version must be an integer literal"
                )),
            }
        } else {
            violations.push(format!(
                "{TEST_INTENT_PATH}:{line_number} unsupported top-level field `{key}`"
            ));
        }
    }

    if let Some(pending) = current.take() {
        finalize_test_intent_entry(pending, &mut entries, &mut violations);
    }

    if !schema_seen {
        violations.push(format!(
            "{TEST_INTENT_PATH} is missing required `schema_version = 1` header"
        ));
    }

    let mut seen: BTreeMap<(String, Option<String>), usize> = BTreeMap::new();
    for entry in &entries {
        let key = (entry.test.clone(), entry.path.clone());
        if let Some(&first) = seen.get(&key) {
            let location = match &entry.path {
                Some(path) => format!("`{}` at `{}`", entry.test, path),
                None => format!("`{}`", entry.test),
            };
            violations.push(format!(
                "{TEST_INTENT_PATH} duplicate selector {location} (first declared near line {first})"
            ));
        } else {
            seen.insert(key, entry.block_line);
        }
    }

    (entries, violations)
}

struct PendingTestIntent {
    block_line: usize,
    test: Option<(String, usize)>,
    path: Option<(String, usize)>,
    intent: Option<(String, usize)>,
    owner: Option<(String, usize)>,
    reason: Option<(String, usize)>,
}

impl PendingTestIntent {
    fn new(block_line: usize) -> Self {
        Self {
            block_line,
            test: None,
            path: None,
            intent: None,
            owner: None,
            reason: None,
        }
    }
}

fn assign_test_intent_field<F>(
    raw_value: &str,
    line_number: usize,
    violations: &mut Vec<String>,
    mut assign: F,
) where
    F: FnMut(String),
{
    match parse_quoted_value(raw_value) {
        Ok(parsed) => assign(parsed),
        Err(message) => violations.push(format!("{TEST_INTENT_PATH}:{line_number} {message}")),
    }
}

fn finalize_test_intent_entry(
    pending: PendingTestIntent,
    entries: &mut Vec<TestIntentDeclaration>,
    violations: &mut Vec<String>,
) {
    let block_line = pending.block_line;

    let test = match pending.test {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!("{TEST_INTENT_PATH}:{line} `test` is blank"));
                None
            } else {
                Some(value)
            }
        }
        None => {
            violations.push(format!(
                "{TEST_INTENT_PATH}:{block_line} `[[test_intent]]` entry is missing required `test`"
            ));
            None
        }
    };

    let path = match pending.path {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!("{TEST_INTENT_PATH}:{line} `path` is empty"));
                None
            } else if value.contains('\\') {
                violations.push(format!(
                    "{TEST_INTENT_PATH}:{line} `path` `{value}` uses backslashes; use `/` separators"
                ));
                None
            } else if is_absolute_path_like(&value) {
                violations.push(format!(
                    "{TEST_INTENT_PATH}:{line} `path` `{value}` is absolute; entries must be repository-relative"
                ));
                None
            } else {
                Some(value)
            }
        }
        None => None,
    };

    let intent = match pending.intent {
        Some((value, line)) => match TestIntentKind::from_str(&value) {
            Some(kind) => Some(kind),
            None => {
                violations.push(format!(
                    "{TEST_INTENT_PATH}:{line} unsupported intent `{value}`; supported: {}",
                    TestIntentKind::supported().join(", ")
                ));
                None
            }
        },
        None => {
            violations.push(format!(
                "{TEST_INTENT_PATH}:{block_line} `[[test_intent]]` entry is missing required `intent`"
            ));
            None
        }
    };

    let owner = match pending.owner {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!(
                    "{TEST_INTENT_PATH}:{line} `owner` is blank; name a responsible team or maintainer"
                ));
                None
            } else {
                Some(value)
            }
        }
        None => {
            violations.push(format!(
                "{TEST_INTENT_PATH}:{block_line} `[[test_intent]]` entry is missing required `owner`"
            ));
            None
        }
    };

    let reason = match pending.reason {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!(
                    "{TEST_INTENT_PATH}:{line} `reason` is blank; explain why this declaration exists"
                ));
                None
            } else {
                Some(value)
            }
        }
        None => {
            violations.push(format!(
                "{TEST_INTENT_PATH}:{block_line} `[[test_intent]]` entry is missing required `reason`"
            ));
            None
        }
    };

    if let (Some(test), Some(intent), Some(owner), Some(reason)) = (test, intent, owner, reason) {
        entries.push(TestIntentDeclaration {
            test,
            path,
            intent,
            owner,
            reason,
            block_line,
        });
    }
}

/// Loads the test-intent manifest from disk. Returns an empty list when
/// the file does not exist (this is a normal state — most projects will
/// have no declarations). Parse and validation violations are returned as
/// `Err` so the caller can surface them through the policy report.
fn load_test_intent_manifest() -> Result<Vec<TestIntentDeclaration>, Vec<String>> {
    let path = Path::new(TEST_INTENT_PATH);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = read_text_lossy(path).map_err(|err| vec![err])?;
    let (entries, violations) = parse_test_intent_manifest(&text);
    if violations.is_empty() {
        Ok(entries)
    } else {
        Err(violations)
    }
}

/// Path-existence guard for `path = "..."` declarations. Kept separate
/// from `apply_test_intent_to_entries` so the matcher stays hermetic for
/// unit tests; this function is the I/O-aware companion the orchestrator
/// runs against real declarations.
fn validate_test_intent_paths_on_disk(declarations: &[TestIntentDeclaration]) -> Vec<String> {
    declarations
        .iter()
        .filter_map(|declaration| {
            declaration.path.as_ref().and_then(|path| {
                if Path::new(path).exists() {
                    None
                } else {
                    Some(format!(
                        "{TEST_INTENT_PATH}:{} `path` `{}` does not exist on disk",
                        declaration.block_line, path
                    ))
                }
            })
        })
        .collect()
}

/// Applies test-intent declarations to a slice of entries, attaching
/// `declared_intent` metadata when a declaration matches a single entry.
/// Returns violations for unmatched declarations and ambiguous name-only
/// selectors. Path-existence is **not** checked here — see
/// `validate_test_intent_paths_on_disk` for the I/O-aware companion.
fn apply_test_intent_to_entries(
    entries: &mut [TestEfficiencyEntry],
    declarations: &[TestIntentDeclaration],
) -> Vec<String> {
    let mut violations = Vec::new();
    for declaration in declarations {
        let matches: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.name == declaration.test
                    && declaration
                        .path
                        .as_ref()
                        .map(|path| normalize_path(&entry.path) == *path)
                        .unwrap_or(true)
            })
            .map(|(index, _)| index)
            .collect();

        match matches.len() {
            0 => {
                let location = match &declaration.path {
                    Some(path) => format!("`{}` at `{}`", declaration.test, path),
                    None => format!("`{}`", declaration.test),
                };
                violations.push(format!(
                    "{TEST_INTENT_PATH}:{} test intent selector {location} did not match any test",
                    declaration.block_line
                ));
            }
            1 => {
                let index = matches[0];
                entries[index].declared_intent = Some(DeclaredIntent {
                    intent: declaration.intent,
                    owner: declaration.owner.clone(),
                    reason: declaration.reason.clone(),
                    source: TEST_INTENT_PATH.to_string(),
                });
            }
            _ if declaration.path.is_none() => {
                let candidates = matches
                    .iter()
                    .map(|&i| normalize_path(&entries[i].path))
                    .collect::<Vec<_>>()
                    .join(", ");
                violations.push(format!(
                    "{TEST_INTENT_PATH}:{} test intent selector `{}` matched multiple tests; add `path` to disambiguate (candidates: {candidates})",
                    declaration.block_line, declaration.test
                ));
            }
            _ => {
                // Multiple matches WITH path; attach to all of them so a
                // genuinely-shared name across files behaves predictably.
                // (In practice the path narrows to one file, so this is
                // rare; we still want determinism if it happens.)
                for &index in &matches {
                    entries[index].declared_intent = Some(DeclaredIntent {
                        intent: declaration.intent,
                        owner: declaration.owner.clone(),
                        reason: declaration.reason.clone(),
                        source: TEST_INTENT_PATH.to_string(),
                    });
                }
            }
        }
    }
    violations
}

fn test_efficiency_class(
    test: &TestOracleTest,
    reached_owners: &[String],
    reasons: &[String],
) -> &'static str {
    if reasons
        .iter()
        .any(|reason| reason == "expected_value_computed_from_detected_owner_path")
    {
        return "possibly_circular";
    }
    if reached_owners.is_empty() {
        return "opaque";
    }
    if reasons
        .iter()
        .any(|reason| reason == "no_assertion_detected")
    {
        return "likely_vacuous";
    }
    match test.class {
        TestOracleClass::Strong => "strong_discriminator",
        TestOracleClass::Medium | TestOracleClass::Weak => "useful_but_broad",
        TestOracleClass::Smoke => "smoke_only",
    }
}

fn test_efficiency_oracle_kind(test: &TestOracleTest) -> &'static str {
    test.observations
        .iter()
        .max_by_key(|observation| observation.class.rank())
        .map(|observation| match observation.pattern.as_str() {
            "exact assertion" => "exact assertion",
            "matches!" => "pattern assertion",
            "boolean comparison" => "relational check",
            "broad predicate" => "broad predicate",
            "status.success" => "smoke execution",
            "generic assert" => "generic boolean assertion",
            "no assertion" => "no assertion detected",
            _ => "opaque oracle",
        })
        .unwrap_or("opaque oracle")
}

fn test_efficiency_static_limitations(test: &TestOracleTest) -> Vec<String> {
    let mut limitations = Vec::new();
    if test
        .observations
        .iter()
        .any(|observation| observation.pattern == "no assertion")
    {
        limitations.push("no assertion-like oracle detected".to_string());
    }
    match test.class {
        TestOracleClass::Strong => {}
        TestOracleClass::Medium => limitations.push(
            "relational oracle; static ledger cannot confirm exact changed value".to_string(),
        ),
        TestOracleClass::Weak => limitations
            .push("broad oracle; static ledger may miss exact discriminator drift".to_string()),
        TestOracleClass::Smoke => limitations.push(
            "smoke-only oracle; static ledger sees execution but little discriminator detail"
                .to_string(),
        ),
    }
    limitations.sort();
    limitations.dedup();
    limitations
}

fn test_efficiency_reasons(
    test: &TestOracleTest,
    reached_owners: &[String],
    observed_values: &[TestEfficiencyValue],
) -> Vec<String> {
    let mut reasons = BTreeSet::new();
    if test
        .observations
        .iter()
        .any(|observation| observation.pattern == "no assertion")
    {
        reasons.insert("no_assertion_detected".to_string());
    }
    match test.class {
        TestOracleClass::Strong => {}
        TestOracleClass::Medium => {
            reasons.insert("relational_oracle".to_string());
        }
        TestOracleClass::Weak => {
            reasons.insert("broad_oracle".to_string());
            if !reached_owners.is_empty() {
                reasons.insert("assertion_may_not_match_detected_owner".to_string());
            }
        }
        TestOracleClass::Smoke => {
            reasons.insert("smoke_oracle_only".to_string());
        }
    }
    if reached_owners.is_empty() {
        reasons.insert("opaque_helper_or_fixture_boundary".to_string());
    }
    if observed_values.is_empty() {
        reasons.insert("no_activation_literal_detected".to_string());
    }
    if expected_value_uses_reached_owner(test, reached_owners) {
        reasons.insert("expected_value_computed_from_detected_owner_path".to_string());
    }
    reasons.into_iter().collect()
}

fn expected_value_uses_reached_owner(test: &TestOracleTest, reached_owners: &[String]) -> bool {
    if reached_owners.is_empty() {
        return false;
    }
    for line in test.body.lines() {
        let trimmed = line.trim();
        if (trimmed.starts_with("let expected") || trimmed.contains(" expected ="))
            && reached_owners
                .iter()
                .any(|owner| trimmed.contains(&format!("{owner}(")))
        {
            return true;
        }
        if let Some(arguments) = assert_eq_arguments(trimmed) {
            for expected_side in arguments.iter().skip(1) {
                if reached_owners
                    .iter()
                    .any(|owner| expected_side.contains(&format!("{owner}(")))
                {
                    return true;
                }
            }
        }
    }
    false
}

fn assert_eq_arguments(line: &str) -> Option<Vec<String>> {
    let marker = "assert_eq!(";
    let start = line.find(marker)? + marker.len();
    let mut depth = 0isize;
    let mut in_string = false;
    let mut escaped = false;
    let mut argument_start = start;
    let mut arguments = Vec::new();
    let bytes = line.as_bytes();
    let mut index = start;
    while index < bytes.len() {
        let ch = line[index..].chars().next()?;
        let ch_len = ch.len_utf8();
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            index += ch_len;
            continue;
        }

        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' if depth == 0 => {
                arguments.push(line[argument_start..index].trim().to_string());
                return Some(arguments);
            }
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                arguments.push(line[argument_start..index].trim().to_string());
                argument_start = index + ch_len;
            }
            _ => {}
        }
        index += ch_len;
    }
    None
}

fn test_efficiency_reached_owners(test: &TestOracleTest) -> Vec<String> {
    let mut calls = BTreeSet::new();
    for line in test.body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("async fn ")
        {
            continue;
        }
        for call in call_names_in_line(trimmed) {
            if !ignored_test_efficiency_call(&call) {
                calls.insert(call);
            }
        }
    }
    calls.into_iter().collect()
}

fn call_names_in_line(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut calls = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'(' {
            index += 1;
            continue;
        }
        if index > 0 && bytes[index - 1] == b'!' {
            index += 1;
            continue;
        }

        let mut start = index;
        while start > 0 && is_call_token_byte(bytes[start - 1]) {
            start -= 1;
        }
        if start == index {
            index += 1;
            continue;
        }
        let token = &line[start..index];
        if let Some(call) = normalized_call_name(token) {
            calls.push(call);
        }
        index += 1;
    }
    calls
}

fn is_call_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b':'
}

fn normalized_call_name(token: &str) -> Option<String> {
    let trimmed = token.trim_matches(':');
    if trimmed.is_empty() {
        return None;
    }
    if trimmed
        .split("::")
        .any(|segment| segment.chars().next().is_some_and(char::is_uppercase))
    {
        return None;
    }
    let last = trimmed.rsplit("::").next().unwrap_or(trimmed);
    if last.is_empty() || !last.chars().next().unwrap_or('_').is_ascii_alphabetic() {
        return None;
    }
    Some(trimmed.to_string())
}

fn ignored_test_efficiency_call(call: &str) -> bool {
    let last = call.rsplit("::").next().unwrap_or(call);
    matches!(
        last,
        "assert"
            | "assert_eq"
            | "assert_ne"
            | "assert_matches"
            | "matches"
            | "format"
            | "format_args"
            | "include_str"
            | "println"
            | "eprintln"
            | "panic"
            | "dbg"
            | "vec"
            | "default"
            | "new"
            | "join"
            | "to_string"
            | "to_owned"
            | "contains"
            | "starts_with"
            | "ends_with"
            | "is_ok"
            | "is_err"
            | "is_some"
            | "is_none"
            | "is_empty"
            | "unwrap"
            | "expect"
            | "clone"
            | "collect"
            | "map"
            | "filter"
            | "iter"
            | "into_iter"
            | "push"
            | "len"
            | "get"
            | "insert"
            | "from"
            | "write"
            | "read_to_string"
            | "test"
    )
}

fn test_efficiency_observed_values(test: &TestOracleTest) -> Vec<TestEfficiencyValue> {
    let mut values = Vec::new();
    for (offset, line) in test.body.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        let line_number = test.body_line + offset;
        let context = test_efficiency_value_context(trimmed);
        for value in string_literals_in_line(trimmed)
            .into_iter()
            .chain(number_literals_in_line(trimmed))
        {
            values.push(TestEfficiencyValue {
                line: line_number,
                context,
                value,
                text: trimmed.to_string(),
            });
        }
    }
    values
}

fn test_efficiency_value_context(line: &str) -> &'static str {
    if line.contains("assert") {
        "assertion_argument"
    } else if line.contains("vec![") || line.contains('[') {
        "table_or_collection"
    } else if line.contains('(') {
        "function_argument"
    } else {
        "literal"
    }
}

fn string_literals_in_line(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut values = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                index += 1;
                values.push(line[start..index].to_string());
                break;
            }
            index += 1;
        }
    }
    values
}

fn number_literals_in_line(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut values = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        let token_boundary = index == 0 || !is_identifier_byte(bytes[index - 1]);
        if !token_boundary || !bytes[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < bytes.len()
            && (bytes[index].is_ascii_digit() || bytes[index] == b'_' || bytes[index] == b'.')
        {
            index += 1;
        }
        if index == bytes.len() || !is_identifier_byte(bytes[index]) {
            values.push(line[start..index].to_string());
        }
    }
    values
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn test_efficiency_counts(entries: &[TestEfficiencyEntry]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::from([
        ("strong_discriminator", 0usize),
        ("useful_but_broad", 0usize),
        ("smoke_only", 0usize),
        ("likely_vacuous", 0usize),
        ("possibly_circular", 0usize),
        ("duplicative", 0usize),
        ("opaque", 0usize),
    ]);
    for entry in entries {
        if let Some(count) = counts.get_mut(entry.class) {
            *count += 1;
        }
    }
    counts
}

fn test_efficiency_reason_counts(entries: &[TestEfficiencyEntry]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for reason in entries.iter().flat_map(|entry| &entry.reasons) {
        *counts.entry(reason.clone()).or_insert(0) += 1;
    }
    counts
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TestEfficiencyMetrics {
    tests_scanned: usize,
    class_counts: BTreeMap<&'static str, usize>,
    reason_counts: BTreeMap<String, usize>,
    duplicate_discriminator_group_count: usize,
}

/// Builds the stable advisory metrics surface for the test-efficiency
/// report. Computed directly from the entries and groups already used to
/// render the report — the JSON and Markdown renderers do not parse their
/// own output to derive metrics.
///
/// `class_counts` is keyed by the seven emitted class strings and always
/// includes every class with a zero default. `reason_counts` is keyed by
/// the reason strings actually present in the entries. `tests_scanned` is
/// the total entry count. `duplicate_discriminator_group_count` is the
/// number of duplicate groups, **not** the number of tests classified
/// `duplicative` — those are reported separately as
/// `class_counts["duplicative"]`.
fn test_efficiency_metrics(
    entries: &[TestEfficiencyEntry],
    duplicate_groups: &[DuplicateDiscriminatorGroup],
) -> TestEfficiencyMetrics {
    TestEfficiencyMetrics {
        tests_scanned: entries.len(),
        class_counts: test_efficiency_counts(entries),
        reason_counts: test_efficiency_reason_counts(entries),
        duplicate_discriminator_group_count: duplicate_groups.len(),
    }
}

fn test_efficiency_report_status(entries: &[TestEfficiencyEntry]) -> &'static str {
    if entries
        .iter()
        .any(|entry| entry.class != "strong_discriminator")
    {
        "warn"
    } else {
        "pass"
    }
}

fn test_efficiency_report_markdown(
    entries: &[TestEfficiencyEntry],
    duplicate_groups: &[DuplicateDiscriminatorGroup],
    test_intent: &TestIntentReportSummary,
) -> String {
    let metrics = test_efficiency_metrics(entries, duplicate_groups);
    let counts = &metrics.class_counts;
    let reason_counts = &metrics.reason_counts;
    let mut body = format!(
        "# ripr test efficiency report\n\nStatus: {}\n\nMode: advisory\n\nThis report builds a per-test evidence ledger from static Rust test facts. It records apparent owner calls, oracle shape, activation values, and static limitations so reviewers can spot low-discriminator patterns without making the report blocking.\n\n## Summary\n\n- Strong discriminator: {}\n- Useful but broad: {}\n- Smoke only: {}\n- Likely vacuous: {}\n- Possibly circular: {}\n- Duplicative: {}\n- Opaque: {}\n- Duplicate discriminator groups: {}\n- Tests scanned: {}\n\n",
        test_efficiency_report_status(entries),
        counts.get("strong_discriminator").copied().unwrap_or(0),
        counts.get("useful_but_broad").copied().unwrap_or(0),
        counts.get("smoke_only").copied().unwrap_or(0),
        counts.get("likely_vacuous").copied().unwrap_or(0),
        counts.get("possibly_circular").copied().unwrap_or(0),
        counts.get("duplicative").copied().unwrap_or(0),
        counts.get("opaque").copied().unwrap_or(0),
        duplicate_groups.len(),
        entries.len(),
    );

    body.push_str("## Metrics\n\n| Metric | Value |\n| --- | ---: |\n");
    body.push_str(&format!("| Tests scanned | {} |\n", metrics.tests_scanned));
    body.push_str(&format!(
        "| Strong discriminator | {} |\n",
        counts.get("strong_discriminator").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Useful but broad | {} |\n",
        counts.get("useful_but_broad").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Smoke only | {} |\n",
        counts.get("smoke_only").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Likely vacuous | {} |\n",
        counts.get("likely_vacuous").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Possibly circular | {} |\n",
        counts.get("possibly_circular").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Duplicative | {} |\n",
        counts.get("duplicative").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Opaque | {} |\n",
        counts.get("opaque").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Duplicate discriminator groups | {} |\n\n",
        metrics.duplicate_discriminator_group_count
    ));

    body.push_str("## Signal Reasons\n\n");
    if reason_counts.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for (reason, count) in reason_counts {
            body.push_str(&format!("- `{reason}`: {count}\n"));
        }
        body.push('\n');
    }

    body.push_str("## Static Limitations\n\n");
    let limited_entries = entries
        .iter()
        .filter(|entry| !entry.static_limitations.is_empty())
        .collect::<Vec<_>>();
    if limited_entries.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for entry in limited_entries {
            body.push_str(&format!(
                "- `{}`:{} `{}` classified `{}`\n",
                normalize_path(&entry.path),
                entry.line,
                entry.name,
                entry.class
            ));
            for limitation in &entry.static_limitations {
                body.push_str(&format!("  - {limitation}\n"));
            }
        }
        body.push('\n');
    }

    body.push_str("## Declared Test Intent\n\n");
    body.push_str(&format!(
        "Source: `{TEST_INTENT_PATH}` · declared: {} · matched: {}\n\n",
        test_intent.declared, test_intent.matched
    ));
    let declared_entries = entries
        .iter()
        .filter(|entry| entry.declared_intent.is_some())
        .collect::<Vec<_>>();
    if declared_entries.is_empty() {
        body.push_str("None declared.\n\n");
    } else {
        body.push_str("| Test | Intent | Owner | Reason |\n| --- | --- | --- | --- |\n");
        for entry in declared_entries {
            if let Some(intent) = &entry.declared_intent {
                body.push_str(&format!(
                    "| `{}`:{} `{}` | `{}` | `{}` | {} |\n",
                    normalize_path(&entry.path),
                    entry.line,
                    markdown_cell(&entry.name),
                    intent.intent.as_str(),
                    markdown_cell(&intent.owner),
                    markdown_cell(&intent.reason)
                ));
            }
        }
        body.push('\n');
    }

    body.push_str("## Duplicate Discriminator Groups\n\n");
    if duplicate_groups.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for group in duplicate_groups {
            body.push_str(&format!("### {}\n\n", group.id));
            let owners = if group.shared_evidence.owners.is_empty() {
                "none detected".to_string()
            } else {
                group.shared_evidence.owners.join(", ")
            };
            body.push_str(&format!("- Owners: {owners}\n"));
            body.push_str(&format!(
                "- Oracle: `{}` / `{}`\n",
                group.shared_evidence.oracle_kind, group.shared_evidence.oracle_strength
            ));
            let activation = group
                .shared_evidence
                .activation_signature
                .iter()
                .map(|item| format!("{}=`{}`", item.context, item.value))
                .collect::<Vec<_>>()
                .join(", ");
            body.push_str(&format!("- Activation signature: {activation}\n"));
            body.push_str("- Members:\n");
            for member in &group.members {
                body.push_str(&format!(
                    "  - `{}`:{} `{}`\n",
                    member.path, member.line, member.name
                ));
            }
            body.push_str(&format!(
                "- Suggested next step: {}\n\n",
                group.suggested_next_step
            ));
        }
    }

    body.push_str("## Ledger\n\n| Test | Class | Reasons | Oracle | Reached owners | Observed values | Static limitations |\n| --- | --- | --- | --- | --- | --- | --- |\n");
    for entry in entries {
        let owners = if entry.reached_owners.is_empty() {
            "none detected".to_string()
        } else {
            entry.reached_owners.join("<br>")
        };
        let values = if entry.observed_values.is_empty() {
            "none detected".to_string()
        } else {
            entry
                .observed_values
                .iter()
                .map(|value| format!("{} `{}` ({})", value.line, value.value, value.context))
                .collect::<Vec<_>>()
                .join("<br>")
        };
        let limitations = if entry.static_limitations.is_empty() {
            "none".to_string()
        } else {
            entry.static_limitations.join("<br>")
        };
        let reasons = if entry.reasons.is_empty() {
            "none".to_string()
        } else {
            entry.reasons.join("<br>")
        };
        body.push_str(&format!(
            "| `{}`:{} `{}` | `{}` | {} | `{}` / `{}` | {} | {} | {} |\n",
            normalize_path(&entry.path),
            entry.line,
            markdown_cell(&entry.name),
            entry.class,
            markdown_cell(&reasons),
            entry.oracle_kind,
            entry.oracle_strength,
            markdown_cell(&owners),
            markdown_cell(&values),
            markdown_cell(&limitations)
        ));
    }
    body
}

fn test_efficiency_report_json(
    entries: &[TestEfficiencyEntry],
    duplicate_groups: &[DuplicateDiscriminatorGroup],
    test_intent: &TestIntentReportSummary,
) -> String {
    let metrics = test_efficiency_metrics(entries, duplicate_groups);
    let counts = &metrics.class_counts;
    let reason_counts = &metrics.reason_counts;
    let mut body = format!(
        "{{\n  \"schema_version\": \"0.1\",\n  \"status\": \"{}\",\n  \"advisory\": true,\n  \"counts\": {{\n    \"strong_discriminator\": {},\n    \"useful_but_broad\": {},\n    \"smoke_only\": {},\n    \"likely_vacuous\": {},\n    \"possibly_circular\": {},\n    \"duplicative\": {},\n    \"opaque\": {}\n  }},\n",
        test_efficiency_report_status(entries),
        counts.get("strong_discriminator").copied().unwrap_or(0),
        counts.get("useful_but_broad").copied().unwrap_or(0),
        counts.get("smoke_only").copied().unwrap_or(0),
        counts.get("likely_vacuous").copied().unwrap_or(0),
        counts.get("possibly_circular").copied().unwrap_or(0),
        counts.get("duplicative").copied().unwrap_or(0),
        counts.get("opaque").copied().unwrap_or(0)
    );

    body.push_str("  \"reason_counts\": {\n");
    for (index, (reason, count)) in reason_counts.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str(&format!("    \"{}\": {}", json_escape(reason), count));
    }
    body.push_str("\n  },\n  \"tests\": [\n");

    for (entry_index, entry) in entries.iter().enumerate() {
        if entry_index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"path\": \"{}\",\n",
            json_escape(&normalize_path(&entry.path))
        ));
        body.push_str(&format!(
            "      \"name\": \"{}\",\n",
            json_escape(&entry.name)
        ));
        body.push_str(&format!("      \"line\": {},\n", entry.line));
        body.push_str(&format!("      \"class\": \"{}\",\n", entry.class));
        body.push_str(&format!(
            "      \"oracle_kind\": \"{}\",\n",
            json_escape(&entry.oracle_kind)
        ));
        body.push_str(&format!(
            "      \"oracle_strength\": \"{}\",\n",
            entry.oracle_strength
        ));
        body.push_str("      \"reached_owners\": [");
        write_json_string_array(&mut body, &entry.reached_owners);
        body.push_str("],\n");
        body.push_str("      \"reasons\": [");
        write_json_string_array(&mut body, &entry.reasons);
        body.push_str("],\n");
        body.push_str("      \"observed_values\": [\n");
        for (value_index, value) in entry.observed_values.iter().enumerate() {
            if value_index > 0 {
                body.push_str(",\n");
            }
            body.push_str("        {\n");
            body.push_str(&format!("          \"line\": {},\n", value.line));
            body.push_str(&format!("          \"context\": \"{}\",\n", value.context));
            body.push_str(&format!(
                "          \"value\": \"{}\",\n",
                json_escape(&value.value)
            ));
            body.push_str(&format!(
                "          \"text\": \"{}\"\n",
                json_escape(&value.text)
            ));
            body.push_str("        }");
        }
        body.push_str("\n      ],\n");
        body.push_str("      \"static_limitations\": [");
        write_json_string_array(&mut body, &entry.static_limitations);
        body.push(']');
        if let Some(group_id) = &entry.duplicate_group_id {
            body.push_str(&format!(
                ",\n      \"duplicate_group_id\": \"{}\"",
                json_escape(group_id)
            ));
        }
        if let Some(intent) = &entry.declared_intent {
            body.push_str(",\n      \"declared_intent\": {\n");
            body.push_str(&format!(
                "        \"intent\": \"{}\",\n",
                intent.intent.as_str()
            ));
            body.push_str(&format!(
                "        \"owner\": \"{}\",\n",
                json_escape(&intent.owner)
            ));
            body.push_str(&format!(
                "        \"reason\": \"{}\",\n",
                json_escape(&intent.reason)
            ));
            body.push_str(&format!(
                "        \"source\": \"{}\"\n",
                json_escape(&intent.source)
            ));
            body.push_str("      }");
        }
        body.push_str("\n    }");
    }
    body.push_str("\n  ],\n  \"duplicate_groups\": [");
    if duplicate_groups.is_empty() {
        body.push(']');
    } else {
        body.push('\n');
        for (group_index, group) in duplicate_groups.iter().enumerate() {
            if group_index > 0 {
                body.push_str(",\n");
            }
            body.push_str("    {\n");
            body.push_str(&format!("      \"id\": \"{}\",\n", json_escape(&group.id)));
            body.push_str("      \"members\": [\n");
            for (member_index, member) in group.members.iter().enumerate() {
                if member_index > 0 {
                    body.push_str(",\n");
                }
                body.push_str("        {\n");
                body.push_str(&format!(
                    "          \"path\": \"{}\",\n",
                    json_escape(&member.path)
                ));
                body.push_str(&format!(
                    "          \"name\": \"{}\",\n",
                    json_escape(&member.name)
                ));
                body.push_str(&format!("          \"line\": {}\n", member.line));
                body.push_str("        }");
            }
            body.push_str("\n      ],\n      \"shared_evidence\": {\n");
            body.push_str("        \"owners\": [");
            write_json_string_array(&mut body, &group.shared_evidence.owners);
            body.push_str("],\n");
            body.push_str(&format!(
                "        \"oracle_kind\": \"{}\",\n",
                json_escape(&group.shared_evidence.oracle_kind)
            ));
            body.push_str(&format!(
                "        \"oracle_strength\": \"{}\",\n",
                group.shared_evidence.oracle_strength
            ));
            body.push_str("        \"activation_signature\": [\n");
            for (activation_index, activation) in group
                .shared_evidence
                .activation_signature
                .iter()
                .enumerate()
            {
                if activation_index > 0 {
                    body.push_str(",\n");
                }
                body.push_str("          {\n");
                body.push_str(&format!(
                    "            \"context\": \"{}\",\n",
                    activation.context
                ));
                body.push_str(&format!(
                    "            \"value\": \"{}\"\n",
                    json_escape(&activation.value)
                ));
                body.push_str("          }");
            }
            body.push_str("\n        ]\n      },\n");
            body.push_str(&format!(
                "      \"suggested_next_step\": \"{}\"\n",
                json_escape(&group.suggested_next_step)
            ));
            body.push_str("    }");
        }
        body.push_str("\n  ]");
    }
    body.push_str(",\n  \"test_intent\": {\n");
    body.push_str(&format!(
        "    \"path\": \"{}\",\n",
        json_escape(TEST_INTENT_PATH)
    ));
    body.push_str(&format!("    \"declared\": {},\n", test_intent.declared));
    body.push_str(&format!("    \"matched\": {}\n", test_intent.matched));
    body.push_str("  }");
    body.push_str(",\n  \"metrics\": {\n");
    body.push_str(&format!(
        "    \"tests_scanned\": {},\n",
        metrics.tests_scanned
    ));
    body.push_str("    \"class_counts\": {\n");
    body.push_str(&format!(
        "      \"strong_discriminator\": {},\n",
        counts.get("strong_discriminator").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "      \"useful_but_broad\": {},\n",
        counts.get("useful_but_broad").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "      \"smoke_only\": {},\n",
        counts.get("smoke_only").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "      \"likely_vacuous\": {},\n",
        counts.get("likely_vacuous").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "      \"possibly_circular\": {},\n",
        counts.get("possibly_circular").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "      \"duplicative\": {},\n",
        counts.get("duplicative").copied().unwrap_or(0)
    ));
    body.push_str(&format!(
        "      \"opaque\": {}\n",
        counts.get("opaque").copied().unwrap_or(0)
    ));
    body.push_str("    },\n    \"reason_counts\": {");
    for (index, (reason, count)) in reason_counts.iter().enumerate() {
        if index > 0 {
            body.push(',');
        }
        body.push_str(&format!("\n      \"{}\": {}", json_escape(reason), count));
    }
    if reason_counts.is_empty() {
        body.push('}');
    } else {
        body.push_str("\n    }");
    }
    body.push_str(&format!(
        ",\n    \"duplicate_discriminator_group_count\": {}\n",
        metrics.duplicate_discriminator_group_count
    ));
    body.push_str("  }\n}\n");
    body
}

pub(crate) fn badge_artifacts_impl() -> Result<(), String> {
    let badge_dir = Path::new("target").join("ripr");
    fs::create_dir_all(&badge_dir).map_err(|err| {
        format!(
            "failed to create badge directory {}: {err}",
            normalize_path(&badge_dir)
        )
    })?;

    let badge_input_path = badge_dir.join("badge-input.diff");
    let diff_output = run_output_optional("git", &["diff", "origin/main...HEAD"])?;
    fs::write(&badge_input_path, &diff_output).map_err(|err| {
        format!(
            "failed to write badge input diff {}: {err}",
            normalize_path(&badge_input_path)
        )
    })?;

    let mut ripr_native_json = String::new();
    let mut ripr_plus_native_json = String::new();

    for job in badge_artifact_jobs() {
        let args = badge_artifact_command_args(job.format);
        let output = run_output_owned("cargo", &args)?;
        write_report(job.output_file, &output)?;
        match badge_artifact_native_slot(job.format) {
            Some(BadgeNativeSlot::Ripr) => ripr_native_json = output,
            Some(BadgeNativeSlot::RiprPlus) => ripr_plus_native_json = output,
            None => {}
        }
    }

    let summary = badge_artifacts_summary_markdown(&ripr_native_json, &ripr_plus_native_json);
    write_report("ripr-badges.md", &summary)
}

#[derive(Debug, PartialEq, Eq)]
struct BadgeArtifactJob {
    format: &'static str,
    output_file: &'static str,
}

#[derive(Debug, PartialEq, Eq)]
enum BadgeNativeSlot {
    Ripr,
    RiprPlus,
}

fn badge_artifact_jobs() -> Vec<BadgeArtifactJob> {
    vec![
        BadgeArtifactJob {
            format: "badge-json",
            output_file: "ripr-badge.json",
        },
        BadgeArtifactJob {
            format: "badge-shields",
            output_file: "ripr-badge-shields.json",
        },
        BadgeArtifactJob {
            format: "badge-plus-json",
            output_file: "ripr-plus-badge.json",
        },
        BadgeArtifactJob {
            format: "badge-plus-shields",
            output_file: "ripr-plus-badge-shields.json",
        },
    ]
}

fn badge_artifact_command_args(format: &str) -> Vec<String> {
    vec![
        "run".to_string(),
        "-p".to_string(),
        "ripr".to_string(),
        "--quiet".to_string(),
        "--".to_string(),
        "check".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--diff".to_string(),
        "target/ripr/badge-input.diff".to_string(),
        "--format".to_string(),
        format.to_string(),
    ]
}

fn badge_artifact_native_slot(format: &str) -> Option<BadgeNativeSlot> {
    match format {
        "badge-json" | "repo-badge-json" => Some(BadgeNativeSlot::Ripr),
        "badge-plus-json" | "repo-badge-plus-json" => Some(BadgeNativeSlot::RiprPlus),
        _ => None,
    }
}

fn badge_artifacts_summary_markdown(ripr_native_json: &str, ripr_plus_native_json: &str) -> String {
    let mut markdown = String::from("# ripr badges\n\n");
    append_badge_section(&mut markdown, "ripr", ripr_native_json);
    append_badge_section(&mut markdown, "ripr+", ripr_plus_native_json);
    markdown.push_str("## Artifacts\n\n");
    markdown.push_str("- `ripr-badge.json` — native ripr badge\n");
    markdown.push_str("- `ripr-badge-shields.json` — Shields projection of ripr badge\n");
    markdown.push_str("- `ripr-plus-badge.json` — native ripr+ badge\n");
    markdown.push_str("- `ripr-plus-badge-shields.json` — Shields projection of ripr+ badge\n");
    markdown
}

/// Run the repo seam inventory and write
/// `target/ripr/reports/repo-seams.{json,md}` per RIPR-SPEC-0005.
/// Shells out to the ripr CLI's `check --format repo-seams-*` paths
/// (the inventory walker is crate-private, so xtask cannot call it
/// directly).
pub(crate) fn repo_seam_inventory_impl() -> Result<(), String> {
    let json_args = repo_seam_inventory_command_args("repo-seams-json");
    let json_output = run_output_owned("cargo", &json_args)?;
    write_report("repo-seams.json", &json_output)?;

    let md_args = repo_seam_inventory_command_args("repo-seams-md");
    let md_output = run_output_owned("cargo", &md_args)?;
    write_report("repo-seams.md", &md_output)
}

fn repo_seam_inventory_command_args(format: &str) -> Vec<String> {
    repo_seam_inventory_command_args_for_root(format, ".")
}

fn repo_seam_inventory_command_args_for_root(format: &str, root: &str) -> Vec<String> {
    // Mirrors `repo_badge_artifact_command_args`: no `--diff` / `--base`
    // because the seam inventory must not depend on
    // `git diff origin/main...HEAD`.
    vec![
        "run".to_string(),
        "-p".to_string(),
        "ripr".to_string(),
        "--quiet".to_string(),
        "--".to_string(),
        "check".to_string(),
        "--root".to_string(),
        root.to_string(),
        "--format".to_string(),
        format.to_string(),
    ]
}

/// Run the repo exposure report (classified seam inventory) and
/// write `target/ripr/reports/repo-exposure.{json,md}` per
/// RIPR-SPEC-0005. Same CLI shell-out pattern as
/// `repo_seam_inventory`, but routes through the
/// `repo-exposure-json|md` formats which compute test-grip evidence
/// and `SeamGripClass` per seam.
pub(crate) fn repo_exposure_report_impl() -> Result<(), String> {
    let json_args = repo_seam_inventory_command_args("repo-exposure-json");
    let json_output = run_output_owned("cargo", &json_args)?;
    write_report("repo-exposure.json", &json_output)?;

    let md_args = repo_seam_inventory_command_args("repo-exposure-md");
    let md_output = run_output_owned("cargo", &md_args)?;
    write_report("repo-exposure.md", &md_output)
}

const REPO_EXPOSURE_LATENCY_TRACE_ENV: &str = "RIPR_REPO_EXPOSURE_LATENCY_TRACE";
const REPO_EXPOSURE_LATENCY_TIMEOUT_ENV: &str = "RIPR_REPO_EXPOSURE_LATENCY_TIMEOUT_MS";
const REPO_EXPOSURE_LATENCY_DEFAULT_TIMEOUT_MS: u64 = 30_000;

#[derive(Clone, Debug)]
struct RepoExposureLatencyReport {
    status: String,
    timeout_ms: u64,
    binary: String,
    runs: Vec<RepoExposureLatencyRun>,
}

#[derive(Clone, Debug)]
struct RepoExposureLatencyRun {
    format: String,
    status: String,
    duration_ms: u128,
    exit_code: Option<i32>,
    stdout_bytes: usize,
    stderr_bytes: usize,
    trace: Vec<RepoExposureLatencyTrace>,
}

#[derive(Clone, Debug)]
struct RepoExposureLatencyTrace {
    phase: String,
    status: String,
    duration_ms: u128,
}

/// Write a bounded repo exposure latency report without changing the
/// repo-exposure JSON/Markdown schemas. This command is diagnostic:
/// it reports timeouts as `warn` in its own report instead of blocking
/// the operator lane indefinitely.
pub(crate) fn repo_exposure_latency_report_impl() -> Result<(), String> {
    let timeout_ms = repo_exposure_latency_timeout_ms();
    run("cargo", &["build", "-p", "ripr"])?;
    let binary = ripr_debug_binary();
    write_repo_exposure_latency_report(&binary, timeout_ms, repo_exposure_latency_run)
}

fn write_repo_exposure_latency_report<F>(
    binary: &Path,
    timeout_ms: u64,
    run_format: F,
) -> Result<(), String>
where
    F: FnMut(&Path, &str, Duration) -> Result<RepoExposureLatencyRun, String>,
{
    let report = build_repo_exposure_latency_report(binary, timeout_ms, run_format)?;
    write_report(
        "repo-exposure-latency.json",
        &repo_exposure_latency_json(&report),
    )?;
    write_report(
        "repo-exposure-latency.md",
        &repo_exposure_latency_markdown(&report),
    )
}

fn build_repo_exposure_latency_report<F>(
    binary: &Path,
    timeout_ms: u64,
    mut run_format: F,
) -> Result<RepoExposureLatencyReport, String>
where
    F: FnMut(&Path, &str, Duration) -> Result<RepoExposureLatencyRun, String>,
{
    let binary_display = binary.display().to_string();
    let timeout = Duration::from_millis(timeout_ms);

    let mut runs = Vec::new();
    let json_run = run_format(binary, "repo-exposure-json", timeout)?;
    let should_run_markdown = json_run.status != "timeout";
    runs.push(json_run);
    if should_run_markdown {
        runs.push(run_format(binary, "repo-exposure-md", timeout)?);
    } else {
        runs.push(RepoExposureLatencyRun {
            format: "repo-exposure-md".to_string(),
            status: "skipped_after_json_timeout".to_string(),
            duration_ms: 0,
            exit_code: None,
            stdout_bytes: 0,
            stderr_bytes: 0,
            trace: Vec::new(),
        });
    }

    let report = RepoExposureLatencyReport {
        status: repo_exposure_latency_status(&runs),
        timeout_ms,
        binary: binary_display,
        runs,
    };
    Ok(report)
}

fn repo_exposure_latency_timeout_ms() -> u64 {
    std::env::var(REPO_EXPOSURE_LATENCY_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(REPO_EXPOSURE_LATENCY_DEFAULT_TIMEOUT_MS)
}

fn ripr_debug_binary() -> PathBuf {
    let binary_name = format!("ripr{}", std::env::consts::EXE_SUFFIX);
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target"));
    target_dir.join("debug").join(binary_name)
}

fn repo_exposure_latency_run(
    binary: &Path,
    format: &str,
    timeout: Duration,
) -> Result<RepoExposureLatencyRun, String> {
    let args = vec![
        "check".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--format".to_string(),
        format.to_string(),
    ];
    let binary_text = binary.display().to_string();
    let envs = [(REPO_EXPOSURE_LATENCY_TRACE_ENV, "1")];
    let output = capture_output_with_timeout(&binary_text, &args, &envs, timeout, format)?;
    Ok(repo_exposure_latency_run_from_output(format, output))
}

fn repo_exposure_latency_run_from_output(
    format: &str,
    output: TimedOutput,
) -> RepoExposureLatencyRun {
    let status = if output.timed_out {
        "timeout"
    } else if output.status.is_some_and(|status| status.success()) {
        "pass"
    } else {
        "fail"
    };
    RepoExposureLatencyRun {
        format: format.to_string(),
        status: status.to_string(),
        duration_ms: output.duration.as_millis(),
        exit_code: output.status.and_then(|status| status.code()),
        stdout_bytes: output.stdout.len(),
        stderr_bytes: output.stderr.len(),
        trace: repo_exposure_latency_trace(&output.stderr),
    }
}

fn repo_exposure_latency_status(runs: &[RepoExposureLatencyRun]) -> String {
    if runs.iter().any(|run| run.status == "fail") {
        return "fail".to_string();
    }
    if runs
        .iter()
        .any(|run| run.status == "timeout" || run.status == "skipped_after_json_timeout")
    {
        return "warn".to_string();
    }
    "pass".to_string()
}

fn repo_exposure_latency_trace(stderr: &str) -> Vec<RepoExposureLatencyTrace> {
    stderr
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("ripr_repo_exposure_latency ")?;
            let mut phase: Option<String> = None;
            let mut status: Option<String> = None;
            let mut duration_ms: Option<u128> = None;
            for field in rest.split_whitespace() {
                if let Some(value) = field.strip_prefix("phase=") {
                    phase = Some(value.to_string());
                } else if let Some(value) = field.strip_prefix("status=") {
                    status = Some(value.to_string());
                } else if let Some(value) = field.strip_prefix("duration_ms=") {
                    duration_ms = value.parse::<u128>().ok();
                }
            }
            Some(RepoExposureLatencyTrace {
                phase: phase?,
                status: status?,
                duration_ms: duration_ms?,
            })
        })
        .collect()
}

fn repo_exposure_latency_json(report: &RepoExposureLatencyReport) -> String {
    let mut body = String::new();
    body.push_str("{\n");
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str("  \"tool\": \"ripr\",\n");
    body.push_str("  \"report\": \"repo-exposure-latency\",\n");
    body.push_str(&format!(
        "  \"status\": \"{}\",\n",
        json_escape(&report.status)
    ));
    body.push_str(&format!("  \"timeout_ms\": {},\n", report.timeout_ms));
    body.push_str(&format!(
        "  \"binary\": \"{}\",\n",
        json_escape(&normalize_report_path(&report.binary))
    ));
    body.push_str("  \"runs\": [\n");
    for (index, run) in report.runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"format\": \"{}\",\n",
            json_escape(&run.format)
        ));
        body.push_str(&format!(
            "      \"status\": \"{}\",\n",
            json_escape(&run.status)
        ));
        body.push_str(&format!("      \"duration_ms\": {},\n", run.duration_ms));
        match run.exit_code {
            Some(code) => body.push_str(&format!("      \"exit_code\": {},\n", code)),
            None => body.push_str("      \"exit_code\": null,\n"),
        }
        body.push_str(&format!("      \"stdout_bytes\": {},\n", run.stdout_bytes));
        body.push_str(&format!("      \"stderr_bytes\": {},\n", run.stderr_bytes));
        body.push_str("      \"trace\": [");
        for (trace_index, trace) in run.trace.iter().enumerate() {
            if trace_index > 0 {
                body.push_str(", ");
            }
            body.push_str(&format!(
                "{{\"phase\": \"{}\", \"status\": \"{}\", \"duration_ms\": {}}}",
                json_escape(&trace.phase),
                json_escape(&trace.status),
                trace.duration_ms
            ));
        }
        body.push_str("]\n");
        body.push_str("    }");
    }
    body.push_str("\n  ]\n");
    body.push_str("}\n");
    body
}

fn repo_exposure_latency_markdown(report: &RepoExposureLatencyReport) -> String {
    let mut body = String::new();
    body.push_str("# Repo Exposure Latency Report\n\n");
    body.push_str(&format!("Status: `{}`\n\n", report.status));
    body.push_str(&format!(
        "Timeout: `{}` ms per format\n\n",
        report.timeout_ms
    ));
    body.push_str(&format!(
        "Binary: `{}`\n\n",
        normalize_report_path(&report.binary)
    ));
    body.push_str("| Format | Status | Duration | Exit | Stdout | Stderr |\n");
    body.push_str("| --- | --- | ---: | ---: | ---: | ---: |\n");
    for run in &report.runs {
        let exit = run
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        body.push_str(&format!(
            "| `{}` | `{}` | {} ms | {} | {} bytes | {} bytes |\n",
            run.format, run.status, run.duration_ms, exit, run.stdout_bytes, run.stderr_bytes
        ));
    }
    body.push_str("\n## Analyzer Trace\n\n");
    if report.runs.iter().all(|run| run.trace.is_empty()) {
        body.push_str("No analyzer trace lines were captured before the command ended.\n");
    } else {
        for run in &report.runs {
            if run.trace.is_empty() {
                continue;
            }
            body.push_str(&format!("### `{}`\n\n", run.format));
            body.push_str("| Phase | Status | Duration |\n");
            body.push_str("| --- | --- | ---: |\n");
            for trace in &run.trace {
                body.push_str(&format!(
                    "| `{}` | `{}` | {} ms |\n",
                    trace.phase, trace.status, trace.duration_ms
                ));
            }
            body.push('\n');
        }
    }
    body.push_str("\n## Next Step\n\n");
    body.push_str(
        "Use this report to identify whether the repo-exposure path is waiting on \
cache collection, cache load, cold compute, cache store, or rendering before \
changing cache behavior.\n",
    );
    body
}

/// Run the agent seam packet renderer and write
/// `target/ripr/reports/agent-seam-packets.json`.
pub(crate) fn agent_seam_packets_report_impl(root: Option<&String>) -> Result<(), String> {
    let root = root.map_or(".", String::as_str);
    let json_args = repo_seam_inventory_command_args_for_root("agent-seam-packets-json", root);
    let json_output = run_output_owned("cargo", &json_args)?;
    write_report("agent-seam-packets.json", &json_output)
}

#[derive(Clone, Debug)]
struct LspCockpitReport {
    status: String,
    fixtures: Vec<LspCockpitFixture>,
    vscode: LspCockpitVscodeCoverage,
}

#[derive(Clone, Debug)]
struct LspCockpitFixture {
    fixture: String,
    diagnostics_path: String,
    code_actions_path: String,
    diagnostic_count: usize,
    seam_diagnostic_count: usize,
    finding_diagnostic_count: usize,
    seam_ids: Vec<String>,
    grip_classes: Vec<String>,
    action_titles: Vec<String>,
    action_commands: Vec<String>,
    action_argument_fields: Vec<String>,
    context: LspCockpitContext,
}

#[derive(Clone, Debug, Default)]
struct LspCockpitContext {
    seam_packet_available: bool,
    targeted_test_brief_available: bool,
    assertion_available: bool,
    related_test_available: bool,
    refresh_available: bool,
}

#[derive(Clone, Debug)]
struct LspCockpitVscodeCoverage {
    test_file: String,
    contributed_commands: Vec<String>,
    covered_commands: Vec<String>,
    covered_contributed_commands: Vec<String>,
    uncovered_contributed_commands: Vec<String>,
}

pub(crate) fn lsp_cockpit_report_impl() -> Result<(), String> {
    let report = build_lsp_cockpit_report()?;
    write_report("lsp-cockpit.json", &lsp_cockpit_report_json(&report)?)?;
    write_report("lsp-cockpit.md", &lsp_cockpit_report_markdown(&report))
}

fn build_lsp_cockpit_report() -> Result<LspCockpitReport, String> {
    let mut fixtures = Vec::new();
    for fixture in fixture_dirs()? {
        if let Some(report) = lsp_cockpit_fixture_report(&fixture)? {
            fixtures.push(report);
        }
    }
    let vscode = lsp_cockpit_vscode_coverage()?;
    let status = if fixtures.is_empty() || !vscode.uncovered_contributed_commands.is_empty() {
        "warn"
    } else {
        "pass"
    }
    .to_string();
    Ok(LspCockpitReport {
        status,
        fixtures,
        vscode,
    })
}

fn lsp_cockpit_fixture_report(fixture: &Path) -> Result<Option<LspCockpitFixture>, String> {
    let expected = fixture.join("expected");
    let diagnostics_path = expected.join("lsp-diagnostics.json");
    let code_actions_path = expected.join("lsp-code-actions.json");
    if !diagnostics_path.exists() && !code_actions_path.exists() {
        return Ok(None);
    }
    if !diagnostics_path.exists() || !code_actions_path.exists() {
        return Err(format!(
            "{} has partial LSP cockpit fixtures; expected both lsp-diagnostics.json and lsp-code-actions.json",
            normalize_path(fixture)
        ));
    }

    let diagnostics_json = read_lsp_cockpit_json_value(&diagnostics_path)?;
    let code_actions_json = read_lsp_cockpit_json_value(&code_actions_path)?;
    let diagnostics = diagnostics_json
        .get("diagnostics")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "{} is missing a diagnostics array",
                normalize_path(&diagnostics_path)
            )
        })?;
    let actions = code_actions_json
        .get("actions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "{} is missing an actions array",
                normalize_path(&code_actions_path)
            )
        })?;

    let fixture_name = fixture
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("invalid fixture path {}", fixture.display()))?
        .to_string();
    let mut seam_ids = BTreeSet::new();
    let mut grip_classes = BTreeSet::new();
    let mut seam_diagnostic_count = 0;
    let mut finding_diagnostic_count = 0;
    for diagnostic in diagnostics {
        let data = diagnostic.get("data").unwrap_or(&Value::Null);
        if let Some(seam_id) = json_str_field(data, "seam_id") {
            seam_diagnostic_count += 1;
            seam_ids.insert(seam_id.to_string());
        }
        if json_str_field(data, "finding_id").is_some() {
            finding_diagnostic_count += 1;
        }
        if let Some(class) =
            json_str_field(data, "grip_class").or_else(|| json_str_field(data, "classification"))
        {
            grip_classes.insert(class.to_string());
        }
    }

    let mut action_titles = Vec::new();
    let mut action_commands = Vec::new();
    let mut action_argument_fields = BTreeSet::new();
    let mut context = LspCockpitContext::default();
    for action in actions {
        let title = json_str_field(action, "title").unwrap_or("unknown");
        let command = json_str_field(action, "command").unwrap_or("unknown");
        action_titles.push(title.to_string());
        action_commands.push(command.to_string());
        if let Some(arguments) = action.get("arguments").and_then(Value::as_array) {
            for argument in arguments {
                if let Some(object) = argument.as_object() {
                    for key in object.keys() {
                        action_argument_fields.insert(key.clone());
                    }
                }
            }
        }
        match command {
            "ripr.copyContext" if title == "Copy seam packet" => {
                context.seam_packet_available = true;
            }
            "ripr.copyTargetedTestBrief" => {
                context.targeted_test_brief_available = action_has_string_argument(action, "brief");
            }
            "ripr.copySuggestedAssertion" => {
                context.assertion_available = action_has_string_argument(action, "assertion");
            }
            "ripr.openRelatedTest" => {
                context.related_test_available = action_has_string_argument(action, "uri");
            }
            "ripr.refresh" => {
                context.refresh_available = true;
            }
            _ => {}
        }
    }

    Ok(Some(LspCockpitFixture {
        fixture: fixture_name,
        diagnostics_path: normalize_path(&diagnostics_path),
        code_actions_path: normalize_path(&code_actions_path),
        diagnostic_count: diagnostics.len(),
        seam_diagnostic_count,
        finding_diagnostic_count,
        seam_ids: seam_ids.into_iter().collect(),
        grip_classes: grip_classes.into_iter().collect(),
        action_titles,
        action_commands,
        action_argument_fields: action_argument_fields.into_iter().collect(),
        context,
    }))
}

fn read_lsp_cockpit_json_value(path: &Path) -> Result<Value, String> {
    let text = read_text_lossy(path)?;
    serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse {} as JSON: {err}", normalize_path(path)))
}

fn json_str_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn action_has_string_argument(action: &Value, field: &str) -> bool {
    action
        .get("arguments")
        .and_then(Value::as_array)
        .is_some_and(|arguments| {
            arguments
                .iter()
                .any(|argument| json_str_field(argument, field).is_some())
        })
}

fn lsp_cockpit_vscode_coverage() -> Result<LspCockpitVscodeCoverage, String> {
    let test_file = Path::new("editors/vscode/test/suite/extension.test.ts");
    let test_text = read_text_lossy(test_file)?;
    let contributed_commands = vscode_contributed_commands()?;
    let covered_commands = ripr_command_literals_in_text(&test_text);
    let covered_set = covered_commands.iter().collect::<BTreeSet<_>>();
    let covered_contributed_commands = contributed_commands
        .iter()
        .filter(|command| covered_set.contains(command))
        .cloned()
        .collect::<Vec<_>>();
    let uncovered_contributed_commands = contributed_commands
        .iter()
        .filter(|command| !covered_set.contains(command))
        .cloned()
        .collect::<Vec<_>>();
    Ok(LspCockpitVscodeCoverage {
        test_file: normalize_path(test_file),
        contributed_commands,
        covered_commands,
        covered_contributed_commands,
        uncovered_contributed_commands,
    })
}

fn vscode_contributed_commands() -> Result<Vec<String>, String> {
    let package = read_lsp_cockpit_json_value(Path::new("editors/vscode/package.json"))?;
    let commands = package
        .get("contributes")
        .and_then(|value| value.get("commands"))
        .and_then(Value::as_array)
        .ok_or_else(|| "editors/vscode/package.json is missing contributes.commands".to_string())?;
    let mut out = BTreeSet::new();
    for command in commands {
        if let Some(id) = json_str_field(command, "command")
            && id.starts_with("ripr.")
        {
            out.insert(id.to_string());
        }
    }
    Ok(out.into_iter().collect())
}

fn ripr_command_literals_in_text(text: &str) -> Vec<String> {
    let mut out = BTreeSet::new();
    collect_quoted_prefixed_strings(text, "ripr.", '\'', &mut out);
    collect_quoted_prefixed_strings(text, "ripr.", '"', &mut out);
    out.into_iter().collect()
}

fn collect_quoted_prefixed_strings(
    text: &str,
    prefix: &str,
    quote: char,
    out: &mut BTreeSet<String>,
) {
    let marker = format!("{quote}{prefix}");
    let mut search_start = 0;
    while let Some(relative_start) = text[search_start..].find(&marker) {
        let value_start = search_start + relative_start + quote.len_utf8();
        let after_start = &text[value_start..];
        let Some(relative_end) = after_start.find(quote) else {
            break;
        };
        out.insert(after_start[..relative_end].to_string());
        search_start = value_start + relative_end + quote.len_utf8();
    }
}

fn lsp_cockpit_report_json(report: &LspCockpitReport) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "status": report.status.as_str(),
        "fixtures": report.fixtures.iter().map(|fixture| {
            serde_json::json!({
                "fixture": fixture.fixture.as_str(),
                "diagnostics_path": fixture.diagnostics_path.as_str(),
                "code_actions_path": fixture.code_actions_path.as_str(),
                "diagnostics": {
                    "total": fixture.diagnostic_count,
                    "seams": fixture.seam_diagnostic_count,
                    "findings": fixture.finding_diagnostic_count,
                    "seam_ids": fixture.seam_ids,
                    "grip_classes": fixture.grip_classes
                },
                "actions": {
                    "titles": fixture.action_titles,
                    "commands": fixture.action_commands,
                    "argument_fields": fixture.action_argument_fields
                },
                "context": {
                    "seam_packet_available": fixture.context.seam_packet_available,
                    "targeted_test_brief_available": fixture.context.targeted_test_brief_available,
                    "assertion_available": fixture.context.assertion_available,
                    "related_test_available": fixture.context.related_test_available,
                    "refresh_available": fixture.context.refresh_available
                }
            })
        }).collect::<Vec<_>>(),
        "vscode_e2e": {
            "test_file": report.vscode.test_file.as_str(),
            "contributed_commands": report.vscode.contributed_commands,
            "covered_commands": report.vscode.covered_commands,
            "covered_contributed_commands": report.vscode.covered_contributed_commands,
            "uncovered_contributed_commands": report.vscode.uncovered_contributed_commands
        }
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render LSP cockpit JSON: {err}"))
}

fn lsp_cockpit_report_markdown(report: &LspCockpitReport) -> String {
    let mut out = String::new();
    out.push_str("# ripr LSP cockpit report\n\n");
    out.push_str(&format!("Status: {}\n\n", report.status));
    if report.fixtures.is_empty() {
        out.push_str("No fixtures with pinned LSP diagnostics/actions were found.\n\n");
    }
    for fixture in &report.fixtures {
        out.push_str(&format!("## Fixture: {}\n\n", md_escape(&fixture.fixture)));
        out.push_str("Diagnostics:\n");
        out.push_str(&format!("- total: {}\n", fixture.diagnostic_count));
        out.push_str(&format!(
            "- seam diagnostics: {}\n",
            fixture.seam_diagnostic_count
        ));
        out.push_str(&format!(
            "- finding diagnostics: {}\n",
            fixture.finding_diagnostic_count
        ));
        push_markdown_list_line(&mut out, "seam ids", &fixture.seam_ids);
        push_markdown_list_line(&mut out, "grip classes", &fixture.grip_classes);

        out.push_str("\nActions:\n");
        for (title, command) in fixture.action_titles.iter().zip(&fixture.action_commands) {
            out.push_str(&format!(
                "- {} (`{}`)\n",
                md_escape(title),
                md_escape(command)
            ));
        }
        push_markdown_list_line(
            &mut out,
            "action argument fields",
            &fixture.action_argument_fields,
        );

        out.push_str("\nContext:\n");
        out.push_str(&format!(
            "- seam packet available: {}\n",
            yes_no(fixture.context.seam_packet_available)
        ));
        out.push_str(&format!(
            "- targeted test brief available: {}\n",
            yes_no(fixture.context.targeted_test_brief_available)
        ));
        out.push_str(&format!(
            "- assertion available: {}\n",
            yes_no(fixture.context.assertion_available)
        ));
        out.push_str(&format!(
            "- related test available: {}\n",
            yes_no(fixture.context.related_test_available)
        ));
        out.push_str(&format!(
            "- refresh available: {}\n",
            yes_no(fixture.context.refresh_available)
        ));
        out.push('\n');
    }

    out.push_str("## VS Code e2e\n\n");
    out.push_str(&format!(
        "- test file: `{}`\n",
        md_escape(&report.vscode.test_file)
    ));
    push_markdown_list_line(
        &mut out,
        "contributed commands",
        &report.vscode.contributed_commands,
    );
    push_markdown_list_line(
        &mut out,
        "covered commands",
        &report.vscode.covered_commands,
    );
    push_markdown_list_line(
        &mut out,
        "covered contributed commands",
        &report.vscode.covered_contributed_commands,
    );
    push_markdown_list_line(
        &mut out,
        "uncovered contributed commands",
        &report.vscode.uncovered_contributed_commands,
    );
    out
}

fn push_markdown_list_line(out: &mut String, label: &str, values: &[String]) {
    if values.is_empty() {
        out.push_str(&format!("- {label}: none\n"));
    } else {
        out.push_str(&format!(
            "- {label}: {}\n",
            values
                .iter()
                .map(|value| format!("`{}`", md_escape(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
}

const SEAM_GRIP_CLASS_ORDER: &[&str] = &[
    "strongly_gripped",
    "weakly_gripped",
    "ungripped",
    "reachable_unrevealed",
    "activation_unknown",
    "propagation_unknown",
    "observation_unknown",
    "discrimination_unknown",
    "opaque",
    "intentional",
    "suppressed",
];

pub(crate) fn targeted_test_outcome_impl(args: &[String]) -> Result<(), String> {
    let parsed = parse_targeted_test_outcome_args(args)?;
    let before_text = read_text_lossy(&parsed.before)?;
    let after_text = read_text_lossy(&parsed.after)?;
    let before = parse_repo_exposure_static_seams(&before_text)?;
    let after = parse_repo_exposure_static_seams(&after_text)?;
    let report = build_targeted_test_outcome_report(
        &before,
        &after,
        normalize_path(&parsed.before),
        normalize_path(&parsed.after),
    )?;
    write_report(
        "targeted-test-outcome.json",
        &targeted_test_outcome_report_json(&report)?,
    )?;
    write_report(
        "targeted-test-outcome.md",
        &targeted_test_outcome_report_markdown(&report),
    )
}

fn parse_targeted_test_outcome_args(args: &[String]) -> Result<TargetedTestOutcomeArgs, String> {
    let mut before: Option<PathBuf> = None;
    let mut after: Option<PathBuf> = None;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--before" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        targeted_test_outcome_usage()
                    ));
                };
                before = Some(PathBuf::from(path));
            }
            "--after" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        targeted_test_outcome_usage()
                    ));
                };
                after = Some(PathBuf::from(path));
            }
            "--help" | "-h" => return Err(targeted_test_outcome_usage()),
            flag if flag.starts_with('-') => {
                return Err(format!(
                    "unknown targeted-test-outcome option `{flag}`\n{}",
                    targeted_test_outcome_usage()
                ));
            }
            other => {
                return Err(format!(
                    "unexpected positional argument `{other}`\n{}",
                    targeted_test_outcome_usage()
                ));
            }
        }
        index += 1;
    }

    let Some(before) = before else {
        return Err(format!(
            "targeted-test-outcome requires `--before <path>`\n{}",
            targeted_test_outcome_usage()
        ));
    };
    let Some(after) = after else {
        return Err(format!(
            "targeted-test-outcome requires `--after <path>`\n{}",
            targeted_test_outcome_usage()
        ));
    };

    Ok(TargetedTestOutcomeArgs { before, after })
}

fn targeted_test_outcome_usage() -> String {
    "usage: cargo xtask targeted-test-outcome --before <repo-exposure-json> --after <repo-exposure-json>"
        .to_string()
}

fn build_targeted_test_outcome_report(
    before: &[StaticSeamRecord],
    after: &[StaticSeamRecord],
    before_path: String,
    after_path: String,
) -> Result<TargetedTestOutcomeReport, String> {
    let before_by_id = targeted_outcome_seams_by_id(before, "before")?;
    let after_by_id = targeted_outcome_seams_by_id(after, "after")?;
    let mut moved = Vec::new();
    let mut unchanged = Vec::new();
    let mut regressed = Vec::new();
    let mut removed = Vec::new();

    for (seam_id, before_seam) in &before_by_id {
        match after_by_id.get(seam_id) {
            Some(after_seam) => {
                let movement = targeted_test_outcome_movement(before_seam, after_seam);
                if movement.before == movement.after {
                    unchanged.push(movement);
                } else if targeted_outcome_grip_rank(&movement.after)
                    < targeted_outcome_grip_rank(&movement.before)
                {
                    regressed.push(movement);
                } else {
                    moved.push(movement);
                }
            }
            None => removed.push(targeted_test_outcome_seam(before_seam)),
        }
    }

    let mut new = Vec::new();
    for (seam_id, after_seam) in &after_by_id {
        if !before_by_id.contains_key(seam_id) {
            new.push(targeted_test_outcome_seam(after_seam));
        }
    }

    Ok(TargetedTestOutcomeReport {
        before_path,
        after_path,
        before_counts: targeted_outcome_class_counts(before),
        after_counts: targeted_outcome_class_counts(after),
        moved,
        unchanged,
        regressed,
        new,
        removed,
    })
}

fn targeted_outcome_seams_by_id(
    seams: &[StaticSeamRecord],
    label: &str,
) -> Result<BTreeMap<String, StaticSeamRecord>, String> {
    let mut out = BTreeMap::new();
    for seam in seams {
        if out.insert(seam.seam_id.clone(), seam.clone()).is_some() {
            return Err(format!(
                "{label} repo exposure JSON contains duplicate seam_id `{}`",
                seam.seam_id
            ));
        }
    }
    Ok(out)
}

fn targeted_outcome_class_counts(seams: &[StaticSeamRecord]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    counts.insert("seams_total".to_string(), seams.len());
    for class in SEAM_GRIP_CLASS_ORDER {
        counts.insert((*class).to_string(), 0);
    }
    for seam in seams {
        *counts.entry(seam.seam_grip_class.clone()).or_insert(0) += 1;
    }
    counts
}

fn targeted_test_outcome_movement(
    before: &StaticSeamRecord,
    after: &StaticSeamRecord,
) -> TargetedTestOutcomeMovement {
    let before_rank = targeted_outcome_grip_rank(&before.seam_grip_class);
    let after_rank = targeted_outcome_grip_rank(&after.seam_grip_class);
    let direction = if before.seam_grip_class == after.seam_grip_class {
        "unchanged"
    } else if after_rank > before_rank {
        "improved"
    } else if after_rank < before_rank {
        "regressed"
    } else {
        "changed"
    };
    let evidence_delta = targeted_outcome_evidence_delta(before, after);
    TargetedTestOutcomeMovement {
        seam_id: before.seam_id.clone(),
        seam_kind: before.seam_kind.clone(),
        file: before.file.clone(),
        line: before.line,
        before: before.seam_grip_class.clone(),
        after: after.seam_grip_class.clone(),
        direction: direction.to_string(),
        evidence_delta,
    }
}

fn targeted_test_outcome_seam(seam: &StaticSeamRecord) -> TargetedTestOutcomeSeam {
    TargetedTestOutcomeSeam {
        seam_id: seam.seam_id.clone(),
        seam_kind: seam.seam_kind.clone(),
        file: seam.file.clone(),
        line: seam.line,
        grip_class: seam.seam_grip_class.clone(),
    }
}

fn targeted_outcome_grip_rank(class: &str) -> u8 {
    match class {
        "strongly_gripped" | "intentional" | "suppressed" => 7,
        "weakly_gripped" => 5,
        "reachable_unrevealed" => 4,
        "activation_unknown"
        | "propagation_unknown"
        | "observation_unknown"
        | "discrimination_unknown" => 3,
        "opaque" => 2,
        "ungripped" => 1,
        _ => 0,
    }
}

fn targeted_outcome_evidence_delta(
    before: &StaticSeamRecord,
    after: &StaticSeamRecord,
) -> Vec<String> {
    let mut deltas = Vec::new();
    if before.seam_grip_class != after.seam_grip_class {
        deltas.push(format!(
            "grip class moved from {} to {}",
            before.seam_grip_class, after.seam_grip_class
        ));
    }

    let before_missing = before
        .missing_discriminators
        .iter()
        .collect::<BTreeSet<_>>();
    let after_missing = after.missing_discriminators.iter().collect::<BTreeSet<_>>();
    for value in before_missing.difference(&after_missing) {
        deltas.push(format!(
            "missing discriminator no longer reported: {}",
            md_escape(value)
        ));
    }
    for value in after_missing.difference(&before_missing) {
        deltas.push(format!(
            "new missing discriminator reported: {}",
            md_escape(value)
        ));
    }

    let before_values = before.observed_values.iter().collect::<BTreeSet<_>>();
    let after_values = after.observed_values.iter().collect::<BTreeSet<_>>();
    for value in after_values.difference(&before_values) {
        deltas.push(format!("new observed value: {}", md_escape(value)));
    }
    for value in before_values.difference(&after_values) {
        deltas.push(format!(
            "previous observed value absent: {}",
            md_escape(value)
        ));
    }

    let before_oracle_rank = oracle_strength_rank(&before.oracle_strength);
    let after_oracle_rank = oracle_strength_rank(&after.oracle_strength);
    if after_oracle_rank > before_oracle_rank {
        deltas.push(format!(
            "stronger related oracle visible: {} -> {}",
            before.oracle_strength, after.oracle_strength
        ));
    } else if after_oracle_rank < before_oracle_rank {
        deltas.push(format!(
            "related oracle strength decreased: {} -> {}",
            before.oracle_strength, after.oracle_strength
        ));
    } else if before.oracle_kind != after.oracle_kind {
        deltas.push(format!(
            "related oracle kind changed: {} -> {}",
            before.oracle_kind, after.oracle_kind
        ));
    }

    if deltas.is_empty() && before.seam_grip_class != after.seam_grip_class {
        deltas.push("grip class changed without rendered evidence details".to_string());
    }
    deltas
}

fn targeted_test_outcome_report_json(report: &TargetedTestOutcomeReport) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "status": "advisory",
        "inputs": {
            "before": report.before_path.as_str(),
            "after": report.after_path.as_str()
        },
        "before": report.before_counts,
        "after": report.after_counts,
        "summary": {
            "moved": report.moved.len(),
            "unchanged": report.unchanged.len(),
            "regressed": report.regressed.len(),
            "new": report.new.len(),
            "removed": report.removed.len()
        },
        "moved": report.moved.iter().map(targeted_test_outcome_movement_json).collect::<Vec<_>>(),
        "unchanged": report.unchanged.iter().map(targeted_test_outcome_movement_json).collect::<Vec<_>>(),
        "regressed": report.regressed.iter().map(targeted_test_outcome_movement_json).collect::<Vec<_>>(),
        "new": report.new.iter().map(targeted_test_outcome_seam_json).collect::<Vec<_>>(),
        "removed": report.removed.iter().map(targeted_test_outcome_seam_json).collect::<Vec<_>>()
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render targeted-test outcome JSON: {err}"))
}

fn targeted_test_outcome_movement_json(movement: &TargetedTestOutcomeMovement) -> Value {
    serde_json::json!({
        "seam_id": movement.seam_id.as_str(),
        "seam_kind": movement.seam_kind.as_str(),
        "file": movement.file.as_str(),
        "line": movement.line,
        "before": movement.before.as_str(),
        "after": movement.after.as_str(),
        "direction": movement.direction.as_str(),
        "evidence_delta": movement.evidence_delta
    })
}

fn targeted_test_outcome_seam_json(seam: &TargetedTestOutcomeSeam) -> Value {
    serde_json::json!({
        "seam_id": seam.seam_id.as_str(),
        "seam_kind": seam.seam_kind.as_str(),
        "file": seam.file.as_str(),
        "line": seam.line,
        "grip_class": seam.grip_class.as_str()
    })
}

fn targeted_test_outcome_report_markdown(report: &TargetedTestOutcomeReport) -> String {
    let mut out = String::new();
    out.push_str("# ripr targeted-test outcome report\n\n");
    out.push_str("Status: advisory\n\n");
    out.push_str("Inputs:\n");
    out.push_str(&format!("- before: `{}`\n", md_escape(&report.before_path)));
    out.push_str(&format!("- after: `{}`\n\n", md_escape(&report.after_path)));

    out.push_str("## Summary\n\n");
    out.push_str("| Bucket | Count |\n| --- | ---: |\n");
    out.push_str(&format!("| moved | {} |\n", report.moved.len()));
    out.push_str(&format!("| unchanged | {} |\n", report.unchanged.len()));
    out.push_str(&format!("| regressed | {} |\n", report.regressed.len()));
    out.push_str(&format!("| new | {} |\n", report.new.len()));
    out.push_str(&format!("| removed | {} |\n", report.removed.len()));

    out.push_str("\n## Grip Counts\n\n");
    out.push_str("| Class | Before | After |\n| --- | ---: | ---: |\n");
    for class in std::iter::once("seams_total").chain(SEAM_GRIP_CLASS_ORDER.iter().copied()) {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            class,
            report.before_counts.get(class).copied().unwrap_or(0),
            report.after_counts.get(class).copied().unwrap_or(0)
        ));
    }

    push_targeted_outcome_movements_md(&mut out, "Moved", &report.moved);
    push_targeted_outcome_movements_md(&mut out, "Unchanged", &report.unchanged);
    push_targeted_outcome_movements_md(&mut out, "Regressed", &report.regressed);
    push_targeted_outcome_seams_md(&mut out, "New", &report.new);
    push_targeted_outcome_seams_md(&mut out, "Removed", &report.removed);
    out.push_str(
        "\nThis report compares two static repo-exposure snapshots. It is advisory and does not run mutation testing.\n",
    );
    out
}

fn push_targeted_outcome_movements_md(
    out: &mut String,
    title: &str,
    movements: &[TargetedTestOutcomeMovement],
) {
    out.push_str(&format!("\n## {title}\n\n"));
    if movements.is_empty() {
        out.push_str("None.\n");
        return;
    }
    for movement in movements {
        out.push_str(&format!(
            "- `{}` {}:{} {} -> {} ({})\n",
            md_escape(&movement.seam_id),
            md_escape(&movement.file),
            movement.line,
            movement.before,
            movement.after,
            movement.direction
        ));
        for delta in &movement.evidence_delta {
            out.push_str(&format!("  - {}\n", md_escape(delta)));
        }
    }
}

fn push_targeted_outcome_seams_md(
    out: &mut String,
    title: &str,
    seams: &[TargetedTestOutcomeSeam],
) {
    out.push_str(&format!("\n## {title}\n\n"));
    if seams.is_empty() {
        out.push_str("None.\n");
        return;
    }
    for seam in seams {
        out.push_str(&format!(
            "- `{}` {}:{} {} ({})\n",
            md_escape(&seam.seam_id),
            md_escape(&seam.file),
            seam.line,
            seam.grip_class,
            seam.seam_kind
        ));
    }
}

pub(crate) fn mutation_calibration_impl(args: &[String]) -> Result<(), String> {
    let parsed = parse_mutation_calibration_args(args)?;
    let repo_exposure_json = match parsed.repo_exposure_json.as_ref() {
        Some(path) => read_text_lossy(path)?,
        None => {
            let json_args =
                repo_seam_inventory_command_args_for_root("repo-exposure-json", &parsed.root);
            let json_output = run_output_owned("cargo", &json_args)?;
            write_report("repo-exposure.json", &json_output)?;
            json_output
        }
    };
    let mutants_json = read_mutation_input_json(&parsed.mutants_json)?;
    let static_seams = parse_repo_exposure_static_seams(&repo_exposure_json)?;
    let runtime_mutants = parse_mutation_outcomes_json(&mutants_json)?;
    let report = build_mutation_calibration_report(static_seams, runtime_mutants);
    write_report(
        "mutation-calibration.json",
        &mutation_calibration_report_json(&report)?,
    )?;
    write_report(
        "mutation-calibration.md",
        &mutation_calibration_report_markdown(&report),
    )
}

fn parse_mutation_calibration_args(args: &[String]) -> Result<MutationCalibrationArgs, String> {
    let mut root: Option<String> = None;
    let mut mutants_json: Option<PathBuf> = None;
    let mut repo_exposure_json: Option<PathBuf> = None;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--mutants-json" | "--cargo-mutants-json" | "--input" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        mutation_calibration_usage()
                    ));
                };
                mutants_json = Some(PathBuf::from(path));
            }
            "--repo-exposure-json" | "--static-json" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        mutation_calibration_usage()
                    ));
                };
                repo_exposure_json = Some(PathBuf::from(path));
            }
            "--help" | "-h" => return Err(mutation_calibration_usage()),
            flag if flag.starts_with('-') => {
                return Err(format!(
                    "unknown mutation-calibration option `{flag}`\n{}",
                    mutation_calibration_usage()
                ));
            }
            positional => {
                if root.is_some() {
                    return Err(format!(
                        "unexpected extra positional argument `{positional}`\n{}",
                        mutation_calibration_usage()
                    ));
                }
                root = Some(positional.to_string());
            }
        }
        index += 1;
    }

    let Some(mutants_json) = mutants_json else {
        return Err(format!(
            "mutation-calibration requires `--mutants-json <path>`\n{}",
            mutation_calibration_usage()
        ));
    };

    Ok(MutationCalibrationArgs {
        root: root.unwrap_or_else(|| ".".to_string()),
        mutants_json,
        repo_exposure_json,
    })
}

fn mutation_calibration_usage() -> String {
    "usage: cargo xtask mutation-calibration [root] --mutants-json <path> [--repo-exposure-json <path>]"
        .to_string()
}

#[derive(Clone, Debug)]
struct SarifPolicyArgs {
    current: PathBuf,
    baseline: Option<PathBuf>,
    mode: SarifPolicyMode,
    threshold: SarifPolicyThreshold,
    missing_baseline: SarifMissingBaseline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SarifPolicyMode {
    Advisory,
    BaselineCheck,
    FailOnNewWarning,
}

impl SarifPolicyMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Advisory => "advisory",
            Self::BaselineCheck => "baseline-check",
            Self::FailOnNewWarning => "fail-on-new-warning",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "advisory" => Some(Self::Advisory),
            "baseline-check" => Some(Self::BaselineCheck),
            "fail-on-new-warning" => Some(Self::FailOnNewWarning),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SarifPolicyThreshold {
    Warning,
    Note,
}

impl SarifPolicyThreshold {
    fn as_str(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "warning" => Some(Self::Warning),
            "note" => Some(Self::Note),
            _ => None,
        }
    }

    fn includes(self, level: &str) -> bool {
        match self {
            Self::Warning => level == "warning",
            Self::Note => matches!(level, "warning" | "note"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SarifMissingBaseline {
    Advisory,
    Error,
}

impl SarifMissingBaseline {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "advisory" => Some(Self::Advisory),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SarifPolicyResult {
    key: String,
    rule_id: String,
    level: String,
    fingerprint: String,
    uri: String,
    line: Option<usize>,
    message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SarifPolicyReport {
    mode: SarifPolicyMode,
    threshold: SarifPolicyThreshold,
    status: String,
    current_path: String,
    baseline_path: Option<String>,
    baseline_missing: bool,
    current_results_total: usize,
    current_compared_results: usize,
    baseline_results_total: usize,
    baseline_compared_results: usize,
    new_results: Vec<SarifPolicyResult>,
}

pub(crate) fn sarif_policy_impl(args: &[String]) -> Result<(), String> {
    let parsed = parse_sarif_policy_args(args)?;
    let current_text = read_text_lossy(&parsed.current)?;
    let current_results = parse_sarif_policy_results(&current_text, "current SARIF")?;

    let (baseline_results, baseline_missing) = match parsed.baseline.as_ref() {
        Some(path) if path.exists() => {
            let baseline_text = read_text_lossy(path)?;
            (
                Some(parse_sarif_policy_results(
                    &baseline_text,
                    "baseline SARIF",
                )?),
                false,
            )
        }
        Some(_) | None => (None, true),
    };

    let report = build_sarif_policy_report(
        parsed.mode,
        parsed.threshold,
        normalize_path(&parsed.current),
        parsed.baseline.as_ref().map(|path| normalize_path(path)),
        &current_results,
        baseline_results.as_deref(),
        baseline_missing,
    );

    write_report("sarif-policy.json", &sarif_policy_report_json(&report)?)?;
    write_report("sarif-policy.md", &sarif_policy_report_markdown(&report))?;

    if report.baseline_missing && parsed.missing_baseline == SarifMissingBaseline::Error {
        return Err("SARIF policy baseline is missing".to_string());
    }
    if parsed.mode == SarifPolicyMode::FailOnNewWarning && !report.new_results.is_empty() {
        return Err(format!(
            "SARIF policy found {} new {} result(s)",
            report.new_results.len(),
            parsed.threshold.as_str()
        ));
    }
    Ok(())
}

fn parse_sarif_policy_args(args: &[String]) -> Result<SarifPolicyArgs, String> {
    let mut current: Option<PathBuf> = None;
    let mut baseline: Option<PathBuf> = None;
    let mut mode = SarifPolicyMode::Advisory;
    let mut threshold = SarifPolicyThreshold::Warning;
    let mut missing_baseline = SarifMissingBaseline::Advisory;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--current" | "--sarif" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        sarif_policy_usage()
                    ));
                };
                current = Some(PathBuf::from(path));
            }
            "--baseline" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        sarif_policy_usage()
                    ));
                };
                baseline = Some(PathBuf::from(path));
            }
            "--mode" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        sarif_policy_usage()
                    ));
                };
                let Some(parsed) = SarifPolicyMode::from_str(value) else {
                    return Err(format!(
                        "unsupported SARIF policy mode `{value}`; expected advisory, baseline-check, or fail-on-new-warning"
                    ));
                };
                mode = parsed;
            }
            "--threshold" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        sarif_policy_usage()
                    ));
                };
                let Some(parsed) = SarifPolicyThreshold::from_str(value) else {
                    return Err(
                        "unsupported SARIF policy threshold; expected warning or note".to_string(),
                    );
                };
                threshold = parsed;
            }
            "--missing-baseline" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        sarif_policy_usage()
                    ));
                };
                let Some(parsed) = SarifMissingBaseline::from_str(value) else {
                    return Err(
                        "unsupported missing-baseline behavior; expected advisory or error"
                            .to_string(),
                    );
                };
                missing_baseline = parsed;
            }
            "--help" | "-h" => return Err(sarif_policy_usage()),
            flag if flag.starts_with('-') => {
                return Err(format!(
                    "unknown sarif-policy option `{flag}`\n{}",
                    sarif_policy_usage()
                ));
            }
            other => {
                return Err(format!(
                    "unexpected positional argument `{other}`\n{}",
                    sarif_policy_usage()
                ));
            }
        }
        index += 1;
    }

    let Some(current) = current else {
        return Err(format!(
            "sarif-policy requires `--current <path>`\n{}",
            sarif_policy_usage()
        ));
    };

    Ok(SarifPolicyArgs {
        current,
        baseline,
        mode,
        threshold,
        missing_baseline,
    })
}

fn sarif_policy_usage() -> String {
    "usage: cargo xtask sarif-policy --current <path> [--baseline <path>] [--mode advisory|baseline-check|fail-on-new-warning] [--threshold warning|note] [--missing-baseline advisory|error]"
        .to_string()
}

fn parse_sarif_policy_results(text: &str, label: &str) -> Result<Vec<SarifPolicyResult>, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|err| format!("failed to parse {label}: {err}"))?;
    let runs = value
        .get("runs")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{label} is missing SARIF `runs` array"))?;
    let mut out = Vec::new();
    for run in runs {
        let Some(results) = run.get("results").and_then(Value::as_array) else {
            continue;
        };
        for result in results {
            if result_has_suppression(result) {
                continue;
            }
            let rule_id = json_string_field(result, "ruleId").unwrap_or_else(|| "unknown".into());
            let level = json_string_field(result, "level").unwrap_or_else(|| "warning".into());
            let fingerprint = sarif_policy_fingerprint(result);
            let location = sarif_policy_location(result);
            let message = result
                .get("message")
                .and_then(|message| json_string_field(message, "text"))
                .unwrap_or_else(|| "ripr SARIF result".to_string());
            let key = format!("{rule_id}|{fingerprint}");
            out.push(SarifPolicyResult {
                key,
                rule_id,
                level,
                fingerprint,
                uri: location.0,
                line: location.1,
                message,
            });
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out.dedup_by(|a, b| a.key == b.key);
    Ok(out)
}

fn build_sarif_policy_report(
    mode: SarifPolicyMode,
    threshold: SarifPolicyThreshold,
    current_path: String,
    baseline_path: Option<String>,
    current_results: &[SarifPolicyResult],
    baseline_results: Option<&[SarifPolicyResult]>,
    baseline_missing: bool,
) -> SarifPolicyReport {
    let current_compared = filtered_sarif_policy_results(current_results, threshold);
    let baseline_compared = baseline_results
        .map(|results| filtered_sarif_policy_results(results, threshold))
        .unwrap_or_default();
    let baseline_keys = baseline_compared
        .iter()
        .map(|result| result.key.as_str())
        .collect::<BTreeSet<_>>();
    let new_results = if baseline_missing {
        Vec::new()
    } else {
        current_compared
            .iter()
            .filter(|result| !baseline_keys.contains(result.key.as_str()))
            .map(|result| (*result).clone())
            .collect::<Vec<_>>()
    };
    let status = if baseline_missing {
        "advisory_missing_baseline"
    } else if new_results.is_empty() {
        "pass"
    } else if mode == SarifPolicyMode::FailOnNewWarning {
        "fail"
    } else {
        "new_results"
    };

    SarifPolicyReport {
        mode,
        threshold,
        status: status.to_string(),
        current_path,
        baseline_path,
        baseline_missing,
        current_results_total: current_results.len(),
        current_compared_results: current_compared.len(),
        baseline_results_total: baseline_results.map_or(0, <[SarifPolicyResult]>::len),
        baseline_compared_results: baseline_compared.len(),
        new_results,
    }
}

fn filtered_sarif_policy_results(
    results: &[SarifPolicyResult],
    threshold: SarifPolicyThreshold,
) -> Vec<&SarifPolicyResult> {
    results
        .iter()
        .filter(|result| threshold.includes(&result.level))
        .collect()
}

fn sarif_policy_report_json(report: &SarifPolicyReport) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "status": report.status,
        "mode": report.mode.as_str(),
        "threshold": report.threshold.as_str(),
        "current": {
            "path": report.current_path,
            "results_total": report.current_results_total,
            "compared_results": report.current_compared_results
        },
        "baseline": {
            "path": report.baseline_path,
            "missing": report.baseline_missing,
            "results_total": report.baseline_results_total,
            "compared_results": report.baseline_compared_results
        },
        "new_results_total": report.new_results.len(),
        "new_results": report.new_results.iter().map(|result| {
            serde_json::json!({
                "rule_id": result.rule_id,
                "level": result.level,
                "fingerprint": result.fingerprint,
                "uri": result.uri,
                "line": result.line,
                "message": result.message
            })
        }).collect::<Vec<_>>()
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render SARIF policy JSON: {err}"))
}

fn sarif_policy_report_markdown(report: &SarifPolicyReport) -> String {
    let mut out = String::new();
    out.push_str("# ripr SARIF policy report\n\n");
    out.push_str(&format!("Status: {}\n\n", report.status));
    out.push_str(&format!("- mode: `{}`\n", report.mode.as_str()));
    out.push_str(&format!("- threshold: `{}`\n", report.threshold.as_str()));
    out.push_str(&format!(
        "- current: `{}`\n",
        md_escape(&report.current_path)
    ));
    match &report.baseline_path {
        Some(path) => out.push_str(&format!("- baseline: `{}`\n", md_escape(path))),
        None => out.push_str("- baseline: not provided\n"),
    }
    out.push_str(&format!(
        "- current compared results: {}\n",
        report.current_compared_results
    ));
    out.push_str(&format!(
        "- baseline compared results: {}\n",
        report.baseline_compared_results
    ));
    if report.baseline_missing {
        out.push_str(
            "\nBaseline is missing; this is advisory unless `--missing-baseline error` is set.\n",
        );
        return out;
    }
    if report.new_results.is_empty() {
        out.push_str("\nNo new configured-threshold SARIF results were detected.\n");
        return out;
    }
    out.push_str("\n## New results\n\n");
    for result in &report.new_results {
        out.push_str(&format!(
            "- `{}` `{}` {}:{} — {}\n",
            result.rule_id,
            result.level,
            md_escape(&result.uri),
            result.line.map_or("?".to_string(), |line| line.to_string()),
            md_escape(&result.message)
        ));
    }
    out
}

fn result_has_suppression(result: &Value) -> bool {
    result
        .get("suppressions")
        .and_then(Value::as_array)
        .is_some_and(|suppressions| !suppressions.is_empty())
}

fn sarif_policy_fingerprint(result: &Value) -> String {
    if let Some(fingerprint) = result
        .get("partialFingerprints")
        .and_then(|fingerprints| json_string_field(fingerprints, "riprFingerprintV1"))
    {
        return fingerprint;
    }
    if let Some(fingerprints) = result.get("partialFingerprints").and_then(Value::as_object) {
        for value in fingerprints.values() {
            if let Some(fingerprint) = value.as_str() {
                return fingerprint.to_string();
            }
        }
    }
    let (uri, line) = sarif_policy_location(result);
    let message = result
        .get("message")
        .and_then(|message| json_string_field(message, "text"))
        .unwrap_or_default();
    format!(
        "{}|{}|{}",
        normalize_path(Path::new(&uri)),
        line.map_or(0, |line| line),
        message
    )
}

fn sarif_policy_location(result: &Value) -> (String, Option<usize>) {
    let Some(location) = result
        .get("locations")
        .and_then(Value::as_array)
        .and_then(|locations| locations.first())
    else {
        return ("unknown".to_string(), None);
    };
    let physical = location.get("physicalLocation");
    let uri = physical
        .and_then(|physical| physical.get("artifactLocation"))
        .and_then(|artifact| json_string_field(artifact, "uri"))
        .unwrap_or_else(|| "unknown".to_string());
    let line = physical
        .and_then(|physical| physical.get("region"))
        .and_then(|region| json_usize_field(region, "startLine"));
    (uri, line)
}

fn json_string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(json_scalar_as_string)
}

fn json_usize_field(value: &Value, key: &str) -> Option<usize> {
    value.get(key).and_then(json_scalar_as_usize)
}

fn md_escape(value: &str) -> String {
    value.replace('`', "\\`").replace(['\r', '\n'], " ")
}

fn read_mutation_input_json(path: &Path) -> Result<String, String> {
    if path.is_dir() {
        let outcomes_path = path.join("outcomes.json");
        let mutants_path = path.join("mutants.json");
        let outcomes_exists = outcomes_path.exists();
        let mutants_exists = mutants_path.exists();

        if outcomes_exists && mutants_exists {
            let outcomes = read_json_value(&outcomes_path)?;
            let mutants = read_json_value(&mutants_path)?;
            return serde_json::to_string(&Value::Array(vec![outcomes, mutants]))
                .map_err(|err| format!("failed to combine cargo-mutants directory JSON: {err}"));
        }

        if outcomes_exists {
            return read_text_lossy(&outcomes_path);
        }
        if mutants_exists {
            return read_text_lossy(&mutants_path);
        }
        return Err(format!(
            "{} is a directory but contains neither outcomes.json nor mutants.json",
            normalize_path(path)
        ));
    }
    read_text_lossy(path)
}

fn read_json_value(path: &Path) -> Result<Value, String> {
    let text = read_text_lossy(path)?;
    serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse JSON from {}: {err}", normalize_path(path)))
}

fn parse_repo_exposure_static_seams(json: &str) -> Result<Vec<StaticSeamRecord>, String> {
    let value: Value = serde_json::from_str(json)
        .map_err(|err| format!("failed to parse repo exposure JSON: {err}"))?;
    let seams = value
        .get("seams")
        .and_then(Value::as_array)
        .ok_or_else(|| "repo exposure JSON is missing `seams` array".to_string())?;

    let mut records = Vec::new();
    for seam in seams {
        let seam_id = required_json_string(seam, "seam_id")?;
        let seam_kind = required_json_string(seam, "kind")?;
        let file = normalize_report_path(&required_json_string(seam, "file")?);
        let line = required_json_usize(seam, "line")?;
        let seam_grip_class = required_json_string(seam, "grip_class")?;
        let (oracle_kind, oracle_strength) = strongest_related_oracle(seam);
        records.push(StaticSeamRecord {
            seam_id,
            seam_kind,
            file,
            line,
            seam_grip_class,
            oracle_kind,
            oracle_strength,
            observed_values: string_array_field(seam, "observed_values"),
            missing_discriminators: missing_discriminator_strings(seam),
        });
    }
    Ok(records)
}

fn required_json_string(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(json_scalar_as_string)
        .ok_or_else(|| format!("repo exposure seam is missing string field `{key}`"))
}

fn required_json_usize(value: &Value, key: &str) -> Result<usize, String> {
    value
        .get(key)
        .and_then(json_scalar_as_usize)
        .ok_or_else(|| format!("repo exposure seam is missing numeric field `{key}`"))
}

fn strongest_related_oracle(seam: &Value) -> (String, String) {
    let mut best_kind = "unknown".to_string();
    let mut best_strength = "unknown".to_string();
    let mut best_rank = 0;

    if let Some(related) = seam.get("related_tests").and_then(Value::as_array) {
        for test in related {
            let strength = test
                .get("oracle_strength")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let rank = oracle_strength_rank(strength);
            if rank > best_rank {
                best_rank = rank;
                best_strength = strength.to_string();
                best_kind = test
                    .get("oracle_kind")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
            }
        }
    }

    (best_kind, best_strength)
}

fn oracle_strength_rank(strength: &str) -> u8 {
    match strength {
        "strong" => 5,
        "medium" => 4,
        "weak" => 3,
        "smoke" => 2,
        "none" => 1,
        _ => 0,
    }
}

fn string_array_field(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(json_scalar_as_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn missing_discriminator_strings(seam: &Value) -> Vec<String> {
    seam.get("missing_discriminators")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if let Some(value) = json_scalar_as_string(item) {
                        return Some(value);
                    }
                    let value = item.get("value").and_then(json_scalar_as_string)?;
                    match item.get("reason").and_then(json_scalar_as_string) {
                        Some(reason) if !reason.is_empty() => Some(format!("{value} ({reason})")),
                        _ => Some(value),
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn parse_mutation_outcomes_json(json: &str) -> Result<Vec<MutationOutcomeRecord>, String> {
    let value: Value = serde_json::from_str(json)
        .map_err(|err| format!("failed to parse cargo-mutants JSON: {err}"))?;
    let mut records = Vec::new();
    collect_mutation_outcome_records(&value, &mut records);
    let mut records = merge_mutation_outcome_records(records);
    records.sort_by(|left, right| {
        left.seam_id
            .cmp(&right.seam_id)
            .then(left.file.cmp(&right.file))
            .then(left.line.cmp(&right.line))
            .then(left.mutation_operator.cmp(&right.mutation_operator))
            .then(left.runtime_outcome.cmp(&right.runtime_outcome))
    });
    Ok(records)
}

fn collect_mutation_outcome_records(value: &Value, records: &mut Vec<MutationOutcomeRecord>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_mutation_outcome_records(item, records);
            }
        }
        Value::Object(object) => {
            for key in [
                "outcomes",
                "mutants",
                "results",
                "mutations",
                "mutation_results",
            ] {
                if let Some(items) = object.get(key).and_then(Value::as_array) {
                    for item in items {
                        collect_mutation_outcome_records(item, records);
                    }
                }
            }
            if let Some(record) = mutation_outcome_record_from_object(object) {
                records.push(record);
            }
        }
        _ => {}
    }
}

fn mutation_outcome_record_from_object(
    object: &serde_json::Map<String, Value>,
) -> Option<MutationOutcomeRecord> {
    let mutant = nested_object(object, "mutant");
    let mutation = nested_object(object, "mutation");
    let location = nested_object(object, "location");
    let span = nested_object(object, "span")
        .or_else(|| mutant.and_then(|nested| nested_object(nested, "span")))
        .or_else(|| mutation.and_then(|nested| nested_object(nested, "span")))
        .or_else(|| location.and_then(|nested| nested_object(nested, "span")));

    let mutant_id = string_field_any(object, &["id", "mutant_id", "mutantId"]).or_else(|| {
        mutant.and_then(|nested| string_field_any(nested, &["id", "mutant_id", "mutantId"]))
    });
    let seam_id = string_field_any(object, &["seam_id", "seamId", "probe_id", "probeId"])
        .or_else(|| {
            mutant.and_then(|nested| {
                string_field_any(nested, &["seam_id", "seamId", "probe_id", "probeId"])
            })
        })
        .or_else(|| {
            mutation.and_then(|nested| {
                string_field_any(nested, &["seam_id", "seamId", "probe_id", "probeId"])
            })
        });
    let file = string_field_any(
        object,
        &["file", "path", "source_file", "src_file", "filename"],
    )
    .or_else(|| {
        mutant.and_then(|nested| {
            string_field_any(
                nested,
                &["file", "path", "source_file", "src_file", "filename"],
            )
        })
    })
    .or_else(|| {
        mutation.and_then(|nested| {
            string_field_any(
                nested,
                &["file", "path", "source_file", "src_file", "filename"],
            )
        })
    })
    .or_else(|| {
        location.and_then(|nested| {
            string_field_any(
                nested,
                &[
                    "file",
                    "path",
                    "source_file",
                    "src_file",
                    "filename",
                    "file_name",
                ],
            )
        })
    })
    .or_else(|| {
        span.and_then(|nested| {
            string_field_any(
                nested,
                &[
                    "file",
                    "path",
                    "source_file",
                    "src_file",
                    "filename",
                    "file_name",
                ],
            )
        })
    })
    .map(|path| normalize_report_path(&path));
    let line = usize_field_any(object, &["line", "line_start", "start_line", "startLine"])
        .or_else(|| {
            mutant.and_then(|nested| {
                usize_field_any(nested, &["line", "line_start", "start_line", "startLine"])
            })
        })
        .or_else(|| {
            mutation.and_then(|nested| {
                usize_field_any(nested, &["line", "line_start", "start_line", "startLine"])
            })
        })
        .or_else(|| {
            location.and_then(|nested| {
                usize_field_any(nested, &["line", "line_start", "start_line", "startLine"])
            })
        })
        .or_else(|| span.and_then(span_start_line));
    let mutation_operator = string_field_any(
        object,
        &[
            "operator",
            "mutation_operator",
            "mutator",
            "mutation",
            "description",
            "replacement",
            "name",
        ],
    )
    .or_else(|| {
        mutant.and_then(|nested| {
            string_field_any(
                nested,
                &[
                    "operator",
                    "mutation_operator",
                    "mutator",
                    "mutation",
                    "description",
                    "replacement",
                    "name",
                ],
            )
        })
    })
    .or_else(|| {
        mutation.and_then(|nested| {
            string_field_any(
                nested,
                &[
                    "operator",
                    "mutation_operator",
                    "mutator",
                    "mutation",
                    "description",
                    "replacement",
                    "name",
                ],
            )
        })
    })
    .unwrap_or_else(|| "unknown".to_string());
    let runtime_outcome =
        string_field_any(object, &["outcome", "status", "result", "summary", "state"])
            .unwrap_or_else(|| "unknown".to_string());
    let duration = string_field_any(
        object,
        &[
            "duration_ms",
            "durationMillis",
            "duration",
            "elapsed_ms",
            "elapsed",
        ],
    );
    let test_command = string_field_any(
        object,
        &["test_command", "testCommand", "command", "cmd", "test_cmd"],
    );

    let has_identity = mutant_id.is_some() || seam_id.is_some() || file.is_some() || line.is_some();
    let has_runtime_detail = runtime_outcome != "unknown"
        || mutation_operator != "unknown"
        || duration.is_some()
        || test_command.is_some();
    if !has_identity || !has_runtime_detail {
        return None;
    }

    Some(MutationOutcomeRecord {
        mutant_id,
        seam_id,
        file,
        line,
        mutation_operator,
        runtime_outcome,
        duration,
        test_command,
    })
}

fn nested_object<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Option<&'a serde_json::Map<String, Value>> {
    object.get(key).and_then(Value::as_object)
}

fn span_start_line(span: &serde_json::Map<String, Value>) -> Option<usize> {
    usize_field_any(span, &["line", "line_start", "start_line", "startLine"])
        .or_else(|| {
            nested_object(span, "start").and_then(|start| {
                usize_field_any(start, &["line", "line_start", "start_line", "startLine"])
            })
        })
        .or_else(|| {
            nested_object(span, "start_position").and_then(|start| {
                usize_field_any(start, &["line", "line_start", "start_line", "startLine"])
            })
        })
        .or_else(|| {
            nested_object(span, "lo").and_then(|start| {
                usize_field_any(start, &["line", "line_start", "start_line", "startLine"])
            })
        })
}

fn string_field_any(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(json_scalar_as_string))
        .filter(|value| !value.trim().is_empty())
}

fn usize_field_any(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<usize> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(json_scalar_as_usize))
}

fn json_scalar_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn json_scalar_as_usize(value: &Value) -> Option<usize> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .and_then(|value| usize::try_from(value).ok()),
        Value::String(text) => text.trim().parse::<usize>().ok(),
        _ => None,
    }
}

fn build_mutation_calibration_report(
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
            reason: "runtime gap signal did not join to a static seam".to_string(),
        });
    }

    for record in ambiguous_file_line {
        if runtime_gap_signal(&record.mutation.runtime_outcome) {
            agreement.runtime_inconclusive += 1;
        }
    }

    missed_runtime_signals.truncate(MUTATION_CALIBRATION_AGREEMENT_SAMPLE_LIMIT);
    static_only_findings.truncate(MUTATION_CALIBRATION_AGREEMENT_SAMPLE_LIMIT);

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

fn mutation_calibration_report_json(report: &MutationCalibrationReport) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": "0.1",
        "scope": "repo",
        "status": "advisory",
        "metrics": {
            "static_seams_total": report.static_seams_total,
            "mutants_total": report.mutants_total,
            "matched_total": report.matched.len(),
            "ambiguous_file_line_total": report.ambiguous_file_line.len(),
            "unmatched_mutants_total": report.unmatched_mutants.len(),
            "static_without_runtime_total": report.static_without_runtime.len(),
            "runtime_outcome_counts": runtime_outcome_counts(report),
            "join_method_counts": join_method_counts(report),
        },
        "agreement": mutation_calibration_agreement_json(&report.agreement),
        "precision_notes": &report.precision_notes,
        "missed_runtime_signals": report
            .missed_runtime_signals
            .iter()
            .map(mutation_calibration_runtime_signal_json)
            .collect::<Vec<_>>(),
        "static_only_findings": report
            .static_only_findings
            .iter()
            .map(mutation_calibration_static_only_json)
            .collect::<Vec<_>>(),
        "matches": report
            .matched
            .iter()
            .map(mutation_calibration_match_json)
            .collect::<Vec<_>>(),
        "ambiguous_file_line_matches": report
            .ambiguous_file_line
            .iter()
            .map(ambiguous_mutation_calibration_match_json)
            .collect::<Vec<_>>(),
        "unmatched_mutants": report
            .unmatched_mutants
            .iter()
            .map(mutation_outcome_json)
            .collect::<Vec<_>>(),
        "static_without_runtime_sample": report
            .static_without_runtime
            .iter()
            .take(MUTATION_CALIBRATION_STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT)
            .map(static_seam_json)
            .collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&value)
        .map(|mut json| {
            json.push('\n');
            json
        })
        .map_err(|err| format!("failed to render mutation calibration JSON: {err}"))
}

fn mutation_calibration_agreement_json(agreement: &MutationCalibrationAgreement) -> Value {
    serde_json::json!({
        "static_gap_and_runtime_signal": agreement.static_gap_and_runtime_signal,
        "static_gap_without_runtime_signal": agreement.static_gap_without_runtime_signal,
        "runtime_signal_without_static_gap": agreement.runtime_signal_without_static_gap,
        "static_clean_and_runtime_clean": agreement.static_clean_and_runtime_clean,
        "runtime_inconclusive": agreement.runtime_inconclusive,
    })
}

fn mutation_calibration_runtime_signal_json(record: &MutationCalibrationRuntimeSignal) -> Value {
    serde_json::json!({
        "runtime": mutation_outcome_json(&record.runtime),
        "static": record.static_seam.as_ref().map(static_seam_json),
        "reason": record.reason.as_str(),
    })
}

fn mutation_calibration_static_only_json(record: &MutationCalibrationStaticOnlyFinding) -> Value {
    serde_json::json!({
        "static": static_seam_json(&record.seam),
        "reason": record.reason.as_str(),
    })
}

fn mutation_calibration_match_json(record: &MutationCalibrationMatch) -> Value {
    serde_json::json!({
        "join_method": record.join_method,
        "static": static_seam_json(&record.seam),
        "runtime": mutation_outcome_json(&record.mutation),
    })
}

fn ambiguous_mutation_calibration_match_json(record: &AmbiguousMutationCalibrationMatch) -> Value {
    serde_json::json!({
        "runtime": mutation_outcome_json(&record.mutation),
        "candidates": record
            .candidates
            .iter()
            .map(static_seam_json)
            .collect::<Vec<_>>(),
    })
}

fn static_seam_json(record: &StaticSeamRecord) -> Value {
    serde_json::json!({
        "seam_id": record.seam_id.as_str(),
        "seam_kind": record.seam_kind.as_str(),
        "file": record.file.as_str(),
        "line": record.line,
        "seam_grip_class": record.seam_grip_class.as_str(),
        "oracle_kind": record.oracle_kind.as_str(),
        "oracle_strength": record.oracle_strength.as_str(),
        "observed_values": &record.observed_values,
        "missing_discriminators": &record.missing_discriminators,
    })
}

fn mutation_outcome_json(record: &MutationOutcomeRecord) -> Value {
    serde_json::json!({
        "mutant_id": record.mutant_id.as_deref(),
        "seam_id": record.seam_id.as_deref(),
        "file": record.file.as_deref(),
        "line": record.line,
        "mutation_operator": record.mutation_operator.as_str(),
        "runtime_outcome": record.runtime_outcome.as_str(),
        "duration": record.duration.as_deref(),
        "test_command": record.test_command.as_deref(),
    })
}

fn merge_mutation_outcome_records(
    records: Vec<MutationOutcomeRecord>,
) -> Vec<MutationOutcomeRecord> {
    let mut by_id: BTreeMap<String, MutationOutcomeRecord> = BTreeMap::new();
    let mut without_id = Vec::new();

    for record in records {
        match record.mutant_id.clone() {
            Some(id) => {
                if let Some(existing) = by_id.get_mut(&id) {
                    merge_mutation_outcome_record(existing, record);
                } else {
                    by_id.insert(id, record);
                }
            }
            None => without_id.push(record),
        }
    }

    by_id.into_values().chain(without_id).collect::<Vec<_>>()
}

fn merge_mutation_outcome_record(
    target: &mut MutationOutcomeRecord,
    source: MutationOutcomeRecord,
) {
    if target.seam_id.is_none() {
        target.seam_id = source.seam_id;
    }
    if target.file.is_none() {
        target.file = source.file;
    }
    if target.line.is_none() {
        target.line = source.line;
    }
    if target.mutation_operator == "unknown" && source.mutation_operator != "unknown" {
        target.mutation_operator = source.mutation_operator;
    }
    if target.runtime_outcome == "unknown" && source.runtime_outcome != "unknown" {
        target.runtime_outcome = source.runtime_outcome;
    }
    if target.duration.is_none() {
        target.duration = source.duration;
    }
    if target.test_command.is_none() {
        target.test_command = source.test_command;
    }
}

fn runtime_outcome_counts(report: &MutationCalibrationReport) -> BTreeMap<String, usize> {
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

fn join_method_counts(report: &MutationCalibrationReport) -> BTreeMap<&'static str, usize> {
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

fn mutation_calibration_report_markdown(report: &MutationCalibrationReport) -> String {
    let mut out = String::new();
    out.push_str("# ripr mutation calibration report\n\n");
    out.push_str("Status: advisory\n\n");
    out.push_str(
        "This report joins static seam evidence to supplied cargo-mutants runtime data. \
         Runtime outcome vocabulary in this report comes from that runtime data; static \
         ripr reports continue to use audit vocabulary only.\n\n",
    );
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n| --- | ---: |\n");
    out.push_str(&format!(
        "| static_seams_total | {} |\n",
        report.static_seams_total
    ));
    out.push_str(&format!("| mutants_total | {} |\n", report.mutants_total));
    out.push_str(&format!("| matched_total | {} |\n", report.matched.len()));
    out.push_str(&format!(
        "| ambiguous_file_line_total | {} |\n",
        report.ambiguous_file_line.len()
    ));
    out.push_str(&format!(
        "| unmatched_mutants_total | {} |\n",
        report.unmatched_mutants.len()
    ));
    out.push_str(&format!(
        "| static_without_runtime_total | {} |\n",
        report.static_without_runtime.len()
    ));

    out.push_str("\n## Static/runtime agreement\n\n");
    out.push_str("| Agreement bucket | Count |\n| --- | ---: |\n");
    out.push_str(&format!(
        "| static_gap_and_runtime_signal | {} |\n",
        report.agreement.static_gap_and_runtime_signal
    ));
    out.push_str(&format!(
        "| static_gap_without_runtime_signal | {} |\n",
        report.agreement.static_gap_without_runtime_signal
    ));
    out.push_str(&format!(
        "| runtime_signal_without_static_gap | {} |\n",
        report.agreement.runtime_signal_without_static_gap
    ));
    out.push_str(&format!(
        "| static_clean_and_runtime_clean | {} |\n",
        report.agreement.static_clean_and_runtime_clean
    ));
    out.push_str(&format!(
        "| runtime_inconclusive | {} |\n",
        report.agreement.runtime_inconclusive
    ));

    out.push_str("\nPrecision notes:\n\n");
    for note in &report.precision_notes {
        out.push_str(&format!("- {}\n", markdown_cell(note)));
    }

    out.push_str("\n### Runtime signals without static gaps\n\n");
    if report.missed_runtime_signals.is_empty() {
        out.push_str("No imported runtime gap signals lacked a matching static gap.\n");
    } else {
        out.push_str("| Runtime mutant | Location | Runtime outcome | Static class | Reason |\n");
        out.push_str("| --- | --- | --- | --- | --- |\n");
        for record in &report.missed_runtime_signals {
            let mutant = record.runtime.mutant_id.as_deref().unwrap_or("unknown");
            let location = mutation_location_label(&record.runtime);
            let static_class = record
                .static_seam
                .as_ref()
                .map(|seam| seam.seam_grip_class.as_str())
                .unwrap_or("unmatched");
            out.push_str(&format!(
                "| `{}` | {} | {} | `{}` | {} |\n",
                markdown_cell(mutant),
                markdown_cell(&location),
                markdown_cell(&record.runtime.runtime_outcome),
                markdown_cell(static_class),
                markdown_cell(&record.reason)
            ));
        }
    }

    out.push_str("\n### Static gaps without runtime signals\n\n");
    if report.static_only_findings.is_empty() {
        out.push_str("No static gap seams lacked a runtime gap signal in this import.\n");
    } else {
        out.push_str("| Seam | Class | Location | Reason |\n");
        out.push_str("| --- | --- | --- | --- |\n");
        for record in &report.static_only_findings {
            let location = format!("{}:{}", record.seam.file, record.seam.line);
            out.push_str(&format!(
                "| `{}` | `{}` | {} | {} |\n",
                markdown_cell(&record.seam.seam_id),
                markdown_cell(&record.seam.seam_grip_class),
                markdown_cell(&location),
                markdown_cell(&record.reason)
            ));
        }
    }

    out.push_str("\n## Runtime Outcome Counts\n\n");
    out.push_str("| Runtime outcome | Count |\n| --- | ---: |\n");
    let counts = runtime_outcome_counts(report);
    if counts.is_empty() {
        out.push_str("| none | 0 |\n");
    } else {
        for (outcome, count) in counts {
            out.push_str(&format!("| {} | {} |\n", markdown_cell(&outcome), count));
        }
    }

    out.push_str("\n## Matched Mutants\n\n");
    if report.matched.is_empty() {
        out.push_str("No runtime mutants matched static seams.\n");
    } else {
        out.push_str("| Seam | Class | Oracle | Mutation operator | Runtime outcome | Join |\n");
        out.push_str("| --- | --- | --- | --- | --- | --- |\n");
        for record in &report.matched {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}`/`{}` | {} | {} | `{}` |\n",
                markdown_cell(&record.seam.seam_id),
                markdown_cell(&record.seam.seam_grip_class),
                markdown_cell(&record.seam.oracle_kind),
                markdown_cell(&record.seam.oracle_strength),
                markdown_cell(&record.mutation.mutation_operator),
                markdown_cell(&record.mutation.runtime_outcome),
                record.join_method
            ));
        }
    }

    out.push_str("\n## Ambiguous File/Line Matches\n\n");
    if report.ambiguous_file_line.is_empty() {
        out.push_str(
            "No runtime mutants matched multiple static seams at the same file and line.\n",
        );
    } else {
        out.push_str("| Runtime mutant | Location | Runtime outcome | Candidate seams |\n");
        out.push_str("| --- | --- | --- | --- |\n");
        for record in &report.ambiguous_file_line {
            let mutant = record.mutation.mutant_id.as_deref().unwrap_or("unknown");
            let location = mutation_location_label(&record.mutation);
            let candidates = record
                .candidates
                .iter()
                .map(|candidate| format!("`{}`", candidate.seam_id))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                markdown_cell(mutant),
                markdown_cell(&location),
                markdown_cell(&record.mutation.runtime_outcome),
                markdown_cell(&candidates)
            ));
        }
    }

    out.push_str("\n## Unmatched Runtime Mutants\n\n");
    if report.unmatched_mutants.is_empty() {
        out.push_str("All imported runtime mutants matched a static seam.\n");
    } else {
        out.push_str("| Location | Mutation operator | Runtime outcome | Test command |\n");
        out.push_str("| --- | --- | --- | --- |\n");
        for record in &report.unmatched_mutants {
            let location = mutation_location_label(record);
            let command = record.test_command.as_deref().unwrap_or("unknown");
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                markdown_cell(&location),
                markdown_cell(&record.mutation_operator),
                markdown_cell(&record.runtime_outcome),
                markdown_cell(command)
            ));
        }
    }

    out.push_str("\n## Static Seams Without Runtime Data\n\n");
    if report.static_without_runtime.is_empty() {
        out.push_str(
            "Every static seam matched at least one runtime mutant in the imported data.\n",
        );
    } else {
        out.push_str(
            "Sample only; see JSON `static_without_runtime_total` for the full count.\n\n",
        );
        out.push_str("| Seam | Kind | Class | Location |\n");
        out.push_str("| --- | --- | --- | --- |\n");
        for seam in report
            .static_without_runtime
            .iter()
            .take(MUTATION_CALIBRATION_STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT)
        {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | {}:{} |\n",
                markdown_cell(&seam.seam_id),
                markdown_cell(&seam.seam_kind),
                markdown_cell(&seam.seam_grip_class),
                markdown_cell(&seam.file),
                seam.line
            ));
        }
    }

    out
}

fn mutation_location_label(record: &MutationOutcomeRecord) -> String {
    if let Some(seam_id) = record.seam_id.as_ref() {
        return format!("seam:{seam_id}");
    }
    match (&record.file, record.line) {
        (Some(file), Some(line)) => format!("{file}:{line}"),
        (Some(file), None) => file.clone(),
        (None, Some(line)) => format!("line {line}"),
        (None, None) => "unknown".to_string(),
    }
}

fn normalize_report_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .strip_prefix("./")
        .unwrap_or(normalized.as_str())
        .to_string()
}

pub(crate) fn repo_badge_artifacts_impl() -> Result<(), String> {
    let badge_dir = Path::new("target").join("ripr");
    fs::create_dir_all(&badge_dir).map_err(|err| {
        format!(
            "failed to create badge directory {}: {err}",
            normalize_path(&badge_dir)
        )
    })?;

    // Repo scope is intentionally diff-free: the badge formats render from
    // classified repo seams rather than `git diff origin/main...HEAD`.
    // Capturing a diff would silently make the artifact dependent on branch
    // state.
    let mut ripr_native_json = String::new();
    let mut ripr_plus_native_json = String::new();

    for job in repo_badge_artifact_jobs() {
        let args = repo_badge_artifact_command_args(job.format);
        let output = run_output_owned("cargo", &args)?;
        write_report(job.output_file, &output)?;
        match badge_artifact_native_slot(job.format) {
            Some(BadgeNativeSlot::Ripr) => ripr_native_json = output,
            Some(BadgeNativeSlot::RiprPlus) => ripr_plus_native_json = output,
            None => {}
        }
    }

    let summary = repo_badge_artifacts_summary_markdown(&ripr_native_json, &ripr_plus_native_json);
    write_report("repo-ripr-badges.md", &summary)
}

fn repo_badge_artifact_jobs() -> Vec<BadgeArtifactJob> {
    vec![
        BadgeArtifactJob {
            format: "repo-badge-json",
            output_file: "repo-ripr-badge.json",
        },
        BadgeArtifactJob {
            format: "repo-badge-shields",
            output_file: "repo-ripr-badge-shields.json",
        },
        BadgeArtifactJob {
            format: "repo-badge-plus-json",
            output_file: "repo-ripr-plus-badge.json",
        },
        BadgeArtifactJob {
            format: "repo-badge-plus-shields",
            output_file: "repo-ripr-plus-badge-shields.json",
        },
    ]
}

fn repo_badge_artifact_command_args(format: &str) -> Vec<String> {
    // Intentionally omits any `--diff` / `--base` argument: repo scope must
    // not consult `git diff origin/main...HEAD`. The regression test
    // `repo_badge_artifact_command_args_does_not_use_git_diff` pins this
    // contract.
    vec![
        "run".to_string(),
        "-p".to_string(),
        "ripr".to_string(),
        "--quiet".to_string(),
        "--".to_string(),
        "check".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--format".to_string(),
        format.to_string(),
    ]
}

fn repo_badge_artifacts_summary_markdown(
    ripr_native_json: &str,
    ripr_plus_native_json: &str,
) -> String {
    let mut markdown = String::from("# ripr repo badges\n\n");
    markdown.push_str(
        "Repo-scoped artifacts: rendered against classified repo seams, not \
against `git diff origin/main...HEAD`. Counts reflect seam-native unresolved \
exposure gaps and unsuppressed actionable test-efficiency findings under the \
configured policy. They are not runtime mutation confirmation.\n\n",
    );
    append_badge_section(&mut markdown, "ripr", ripr_native_json);
    append_badge_section(&mut markdown, "ripr+", ripr_plus_native_json);
    markdown.push_str("## Artifacts\n\n");
    markdown.push_str("- `repo-ripr-badge.json` — native repo-scoped ripr badge\n");
    markdown.push_str(
        "- `repo-ripr-badge-shields.json` — Shields projection of repo-scoped ripr badge\n",
    );
    markdown.push_str("- `repo-ripr-plus-badge.json` — native repo-scoped ripr+ badge\n");
    markdown.push_str(
        "- `repo-ripr-plus-badge-shields.json` — Shields projection of repo-scoped ripr+ badge\n",
    );
    markdown
}

/// Names of the two committed badge endpoint files served via
/// `raw.githubusercontent.com/.../main/badges/<file>`. The `ripr`
/// product contract is "ripr emits Shields-compatible JSON"; this is
/// just the v1 self-hosted dogfood path that copies the latest
/// repo-scoped Shields JSON into a stable repo-relative location.
/// See `docs/BADGE_POLICY.md` and `deferred/hosted-badge-service`.
const BADGE_ENDPOINT_FILES: &[(&str, &str)] = &[
    ("badges/ripr.json", "repo-ripr-badge-shields.json"),
    ("badges/ripr-plus.json", "repo-ripr-plus-badge-shields.json"),
];

/// Regenerates `target/ripr/reports/repo-ripr-{badge,plus-badge}-shields.json`
/// via `repo_badge_artifacts()` and copies the two Shields projections
/// into the committed `badges/` directory so the README endpoint URLs
/// reflect the latest repo-scoped state.
pub(crate) fn update_badge_endpoints_impl() -> Result<(), String> {
    repo_badge_artifacts()?;
    copy_badge_endpoints_from_reports(Path::new("target/ripr/reports"), Path::new("."))
}

/// Pure file-copy half of `update_badge_endpoints` — separated so the
/// path arithmetic and per-file error wrapping can be unit-tested
/// against tempdirs without invoking `cargo`.
fn copy_badge_endpoints_from_reports(reports_dir: &Path, repo_root: &Path) -> Result<(), String> {
    let badges_dir = repo_root.join("badges");
    fs::create_dir_all(&badges_dir).map_err(|err| {
        format!(
            "failed to create badges directory {}: {err}",
            normalize_path(&badges_dir)
        )
    })?;
    for (committed, source_name) in BADGE_ENDPOINT_FILES {
        let source = reports_dir.join(source_name);
        let bytes = fs::read(&source).map_err(|err| {
            format!(
                "failed to read {} (run `cargo xtask repo-badge-artifacts` first): {err}",
                normalize_path(&source)
            )
        })?;
        let dest = repo_root.join(committed);
        fs::write(&dest, &bytes)
            .map_err(|err| format!("failed to write {}: {err}", normalize_path(&dest)))?;
    }
    Ok(())
}

/// File-reading wrapper around `badge_endpoint_violation`. Walks
/// `BADGE_ENDPOINT_FILES`, reads each source from `reports_dir` and
/// each committed file from `repo_root`, and collects violations.
/// Splitting this out from `check_badge_endpoints` lets tests exercise
/// the file walk against tempdirs without invoking `cargo`.
fn compute_badge_endpoint_violations(
    reports_dir: &Path,
    repo_root: &Path,
) -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    for (committed, source_name) in BADGE_ENDPOINT_FILES {
        let source = reports_dir.join(source_name);
        let source_display = normalize_path(&source);
        let want =
            fs::read(&source).map_err(|err| format!("failed to read {source_display}: {err}"))?;
        let committed_path = repo_root.join(committed);
        let actual = fs::read(&committed_path).ok();
        if let Some(violation) =
            badge_endpoint_violation(committed, &source_display, &want, actual.as_deref())
        {
            violations.push(violation);
        }
    }
    Ok(violations)
}

/// Pure comparison helper for `check_badge_endpoints` — separated so
/// the violation-string contract is unit-testable without touching
/// the file system. Returns `None` when the committed file is in
/// sync, otherwise an actionable violation message.
fn badge_endpoint_violation(
    committed_path: &str,
    source_display: &str,
    expected_bytes: &[u8],
    actual_bytes: Option<&[u8]>,
) -> Option<String> {
    match actual_bytes {
        None => Some(format!(
            "missing badge endpoint file {committed_path}; run `cargo xtask update-badge-endpoints`"
        )),
        Some(actual) if actual != expected_bytes => Some(format!(
            "badge endpoint file {committed_path} is stale relative to {source_display}; run `cargo xtask update-badge-endpoints` and commit the diff"
        )),
        _ => None,
    }
}

/// Verifies that the committed `badges/*.json` files match the latest
/// `cargo xtask repo-badge-artifacts` output. Fails with an actionable
/// message pointing at `cargo xtask update-badge-endpoints` when stale.
/// Intentionally not added to the default CI gate set in v1 — the
/// endpoint count drifts whenever production code or tests change, and
/// requiring every PR to also update `badges/` is too much friction
/// before the headline stabilizes. Use locally before campaign
/// closeouts and after material analyzer changes.
pub(crate) fn check_badge_endpoints_impl() -> Result<(), String> {
    repo_badge_artifacts()?;
    let violations =
        compute_badge_endpoint_violations(Path::new("target/ripr/reports"), Path::new("."))?;
    finish_policy_report(
        PolicyReportSpec {
            report_file: "badge-endpoints.md",
            check: "check-badge-endpoints",
            why_it_matters: "The committed badges/*.json files are the public Shields endpoint surfaces; stale files cause the README badge to lie about repo state.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Run `cargo xtask update-badge-endpoints` and commit the resulting badges/*.json diff.",
                "If the drift is from an unrelated PR, run `cargo xtask update-badge-endpoints` on `main` and commit on its own scoped PR.",
                "Skip running this check on PRs that do not change the repo headline (it is not yet a hard CI gate).",
            ],
            rerun_command: "cargo xtask check-badge-endpoints",
            exception_template: None,
        },
        &violations,
    )
}

fn append_badge_section(markdown: &mut String, heading: &str, native_json: &str) {
    let message = extract_json_string(native_json, "\"message\":").unwrap_or_default();
    let color = extract_json_string(native_json, "\"color\":").unwrap_or_default();
    let counts = extract_json_object_usize_map(native_json, "\"counts\":");
    let reason_counts = extract_json_object_usize_map(native_json, "\"reason_counts\":");
    let warnings = extract_json_warnings(native_json);

    markdown.push_str(&format!("## {heading}\n\n"));
    markdown.push_str(&format!("- message: {message}\n"));
    markdown.push_str(&format!("- color: {color}\n"));
    markdown.push_str("- counts:\n");
    for (key, value) in &counts {
        markdown.push_str(&format!("  - {key}: {value}\n"));
    }
    markdown.push_str("- reason_counts:\n");
    for (key, value) in &reason_counts {
        markdown.push_str(&format!("  - {key}: {value}\n"));
    }
    if warnings.is_empty() {
        markdown.push_str("- warnings: none\n\n");
    } else {
        markdown.push_str("- warnings:\n");
        for warning in &warnings {
            markdown.push_str(&format!("  - {warning}\n"));
        }
        markdown.push('\n');
    }
}

fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let start = json.find(key)? + key.len();
    let remaining = &json[start..];
    let quote_start = remaining.find('"')?;
    let quote_end = remaining[quote_start + 1..].find('"')?;
    Some(remaining[quote_start + 1..quote_start + 1 + quote_end].to_string())
}

fn extract_json_object_usize_map(json: &str, key: &str) -> BTreeMap<String, usize> {
    let mut entries = BTreeMap::new();
    let object_start = match json.find(key) {
        Some(pos) => {
            let after_key = pos + key.len();
            let remaining = &json[after_key..];
            let brace_pos = remaining.find('{').unwrap_or(0);
            after_key + brace_pos + 1
        }
        None => return entries,
    };

    let object_slice = &json[object_start..];
    let object_end = match object_slice.find('}') {
        Some(pos) => pos,
        None => return entries,
    };

    let object_text = &object_slice[..object_end];
    for part in object_text.split(',') {
        if let Some(colon_pos) = part.find(':') {
            let key_part = part[..colon_pos].trim();
            let value_part = part[colon_pos + 1..].trim();

            if key_part.starts_with('"') && key_part.ends_with('"') {
                let entry_key = key_part[1..key_part.len() - 1].to_string();
                if let Ok(value) = value_part.parse::<usize>() {
                    entries.insert(entry_key, value);
                }
            }
        }
    }
    entries
}

fn extract_json_warnings(json: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    let needle = "\"warnings\":";
    let warnings_start = match json.find(needle) {
        Some(pos) => {
            let after_colon = pos + needle.len();
            let remaining = &json[after_colon..];
            let bracket_pos = remaining.find('[').unwrap_or(0);
            after_colon + bracket_pos + 1
        }
        None => return warnings,
    };

    let remaining = &json[warnings_start..];
    let end_bracket_pos = match remaining.find(']') {
        Some(pos) => pos,
        None => return warnings,
    };

    let warnings_content = &remaining[..end_bracket_pos];

    let mut i = 0;
    let chars: Vec<char> = warnings_content.chars().collect();

    while i < chars.len() {
        if chars[i] == '"' {
            i += 1;
            let mut warning_chars = Vec::new();

            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                }
                warning_chars.push(chars[i]);
                i += 1;
            }

            if i < chars.len() && chars[i] == '"' {
                let warning: String = warning_chars.into_iter().collect();
                warnings.push(warning);
            }
        }
        i += 1;
    }

    warnings
}

pub(crate) fn dogfood_impl() -> Result<(), String> {
    let runs = dogfood_scenarios()
        .into_iter()
        .map(|scenario| dogfood_run(&scenario))
        .collect::<Result<Vec<_>, _>>()?;
    write_report("dogfood.md", &dogfood_report_markdown(&runs))?;
    write_report("dogfood.json", &dogfood_report_json(&runs))
}

fn dogfood_scenarios() -> Vec<DogfoodScenario> {
    ["boundary_gap".to_string(), "weak_error_oracle".to_string()]
        .into_iter()
        .map(|name| {
            let base = Path::new("fixtures").join(&name);
            DogfoodScenario {
                name,
                root: base.join("input"),
                diff: base.join("diff.patch"),
            }
        })
        .collect()
}

fn dogfood_run(scenario: &DogfoodScenario) -> Result<DogfoodRun, String> {
    let started = Instant::now();
    let actual_dir = Path::new("target")
        .join("ripr")
        .join("dogfood")
        .join(&scenario.name);
    fs::create_dir_all(&actual_dir).map_err(|err| {
        format!(
            "failed to create dogfood output directory {}: {err}",
            normalize_path(&actual_dir)
        )
    })?;

    let mut errors = Vec::new();
    let mut findings = 0usize;
    let mut class_counts = BTreeMap::new();
    let mut stop_reason_mentions = 0usize;

    if !scenario.root.exists() {
        errors.push(format!(
            "fixture root does not exist: {}",
            normalize_path(&scenario.root)
        ));
    }
    if !scenario.diff.exists() {
        errors.push(format!(
            "fixture diff does not exist: {}",
            normalize_path(&scenario.diff)
        ));
    }

    if errors.is_empty() {
        let root = normalize_path(&scenario.root);
        let diff = normalize_path(&scenario.diff);
        match run_fixture_check(&root, &diff, true) {
            Ok(json) => {
                let normalized = normalize_fixture_json_output(&json);
                findings = json_number_after(&normalized, "\"findings\":").unwrap_or(0);
                class_counts = dogfood_class_counts(&normalized);
                stop_reason_mentions = normalized.matches("\"stop_reasons\"").count();
                let path = actual_dir.join("check.json");
                fs::write(&path, normalized).map_err(|err| {
                    format!(
                        "failed to write dogfood JSON output {}: {err}",
                        normalize_path(&path)
                    )
                })?;
            }
            Err(err) => errors.push(err),
        }

        match run_fixture_check(&root, &diff, false) {
            Ok(human) => {
                let normalized = normalize_fixture_human_output(&human);
                let path = actual_dir.join("human.txt");
                fs::write(&path, normalized).map_err(|err| {
                    format!(
                        "failed to write dogfood human output {}: {err}",
                        normalize_path(&path)
                    )
                })?;
            }
            Err(err) => errors.push(err),
        }
    }

    Ok(DogfoodRun {
        name: scenario.name.clone(),
        root: scenario.root.clone(),
        diff: scenario.diff.clone(),
        actual_dir,
        duration_ms: started.elapsed().as_millis(),
        findings,
        class_counts,
        stop_reason_mentions,
        errors,
    })
}

fn dogfood_class_counts(json: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for class in [
        "exposed",
        "weakly_exposed",
        "reachable_unrevealed",
        "no_static_path",
        "infection_unknown",
        "propagation_unknown",
        "static_unknown",
    ] {
        counts.insert(
            class.to_string(),
            json.matches(&format!("\"classification\": \"{class}\""))
                .count(),
        );
    }
    counts
}

fn json_number_after(text: &str, needle: &str) -> Option<usize> {
    let start = text.find(needle)? + needle.len();
    let digits = text[start..]
        .chars()
        .skip_while(|ch| ch.is_ascii_whitespace())
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        None
    } else {
        digits.parse::<usize>().ok()
    }
}

fn dogfood_report_status(runs: &[DogfoodRun]) -> &'static str {
    if runs.iter().any(|run| !run.errors.is_empty()) {
        "warn"
    } else {
        "pass"
    }
}

fn dogfood_report_markdown(runs: &[DogfoodRun]) -> String {
    let mut body = format!(
        "# ripr dogfood report\n\nStatus: {}\n\nMode: advisory\n\nThis report runs `ripr check --mode fast` against stable in-repo fixture diffs. It records current product output for review without making dogfood a blocking gate yet.\n\n## Summary\n\n",
        dogfood_report_status(runs)
    );
    for run in runs {
        body.push_str(&format!(
            "- `{}`: {} finding(s), {} stop-reason field(s), {} ms\n",
            run.name, run.findings, run.stop_reason_mentions, run.duration_ms
        ));
    }

    body.push_str("\n## Runs\n\n");
    for run in runs {
        body.push_str(&format!("### `{}`\n\n", run.name));
        body.push_str(&format!("- Root: `{}`\n", normalize_path(&run.root)));
        body.push_str(&format!("- Diff: `{}`\n", normalize_path(&run.diff)));
        body.push_str(&format!(
            "- Actual outputs: `{}`\n",
            normalize_path(&run.actual_dir)
        ));
        body.push_str(&format!("- Findings: {}\n", run.findings));
        body.push_str("- Exposure classes:\n");
        for (class, count) in &run.class_counts {
            body.push_str(&format!("  - `{class}`: {count}\n"));
        }
        if run.errors.is_empty() {
            body.push_str("- Errors: none\n\n");
        } else {
            body.push_str("- Errors:\n");
            for error in &run.errors {
                body.push_str(&format!("  - `{}`\n", markdown_cell(error)));
            }
            body.push('\n');
        }
    }
    body
}

fn dogfood_report_json(runs: &[DogfoodRun]) -> String {
    let mut body = format!(
        "{{\n  \"schema_version\": \"0.1\",\n  \"status\": \"{}\",\n  \"advisory\": true,\n  \"runs\": [\n",
        dogfood_report_status(runs)
    );
    for (index, run) in runs.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"name\": \"{}\",\n",
            json_escape(&run.name)
        ));
        body.push_str(&format!(
            "      \"root\": \"{}\",\n",
            json_escape(&normalize_path(&run.root))
        ));
        body.push_str(&format!(
            "      \"diff\": \"{}\",\n",
            json_escape(&normalize_path(&run.diff))
        ));
        body.push_str(&format!(
            "      \"actual_dir\": \"{}\",\n",
            json_escape(&normalize_path(&run.actual_dir))
        ));
        body.push_str(&format!("      \"duration_ms\": {},\n", run.duration_ms));
        body.push_str(&format!("      \"findings\": {},\n", run.findings));
        body.push_str(&format!(
            "      \"stop_reason_mentions\": {},\n",
            run.stop_reason_mentions
        ));
        body.push_str("      \"class_counts\": {");
        for (class_index, (class, count)) in run.class_counts.iter().enumerate() {
            if class_index > 0 {
                body.push_str(", ");
            }
            body.push_str(&format!("\"{}\": {}", json_escape(class), count));
        }
        body.push_str("},\n");
        body.push_str("      \"errors\": [");
        write_json_string_array(&mut body, &run.errors);
        body.push_str("]\n    }");
    }
    body.push_str("\n  ]\n}\n");
    body
}

fn check_capabilities() -> Result<(), String> {
    let manifest = Path::new("metrics/capabilities.toml");
    let mut violations = Vec::new();
    if !manifest.exists() {
        violations.push("metrics/capabilities.toml is missing".to_string());
        return finish_capabilities_report(&violations);
    }
    let (capabilities, parse_violations) = parse_capabilities_manifest(manifest)?;
    violations.extend(parse_violations);
    validate_capabilities(&capabilities, &mut violations)?;
    finish_capabilities_report(&violations)
}

fn finish_capabilities_report(violations: &[String]) -> Result<(), String> {
    finish_policy_report(
        PolicyReportSpec {
            report_file: "capabilities.md",
            check: "check-capabilities",
            why_it_matters: "Capability status should be a checked source of truth, not README prose that can drift from specs and fixtures.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Update metrics/capabilities.toml with status, spec, next checkpoint, and metric fields.",
                "Keep capability statuses to planned, alpha, stable, or calibrated.",
                "Reference only specs that exist in docs/specs.",
                "Use cargo xtask metrics to regenerate target/ripr/reports/metrics.md and metrics.json.",
            ],
            rerun_command: "cargo xtask check-capabilities",
            exception_template: None,
        },
        violations,
    )
}

fn validate_capabilities(
    capabilities: &[Capability],
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let specs = collect_spec_statuses()?;
    let mut ids = BTreeSet::new();
    if capabilities.is_empty() {
        violations.push("metrics/capabilities.toml has no [[capability]] entries".to_string());
    }
    for capability in capabilities {
        let Some(id) = capability.id.as_ref() else {
            violations.push(format!(
                "capability at line {} is missing `id`",
                capability.line
            ));
            continue;
        };
        if !is_snake_case_id(id) {
            violations.push(format!(
                "capability at line {} has invalid id `{id}`; use snake_case",
                capability.line
            ));
        }
        if !ids.insert(id.clone()) {
            violations.push(format!("duplicate capability id `{id}`"));
        }
        if capability
            .name
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            violations.push(format!("{id} is missing a non-empty `name`"));
        }
        match capability.status.as_deref() {
            Some("planned" | "alpha" | "stable" | "calibrated") => {}
            Some(status) => violations.push(format!("{id} has unsupported status `{status}`")),
            None => violations.push(format!("{id} is missing `status`")),
        }
        match capability.spec.as_ref() {
            Some(spec) if is_spec_id(spec) && specs.contains_key(spec) => {}
            Some(spec) if is_spec_id(spec) => {
                violations.push(format!("{id} references missing spec `{spec}`"));
            }
            Some(spec) => violations.push(format!("{id} has invalid spec id `{spec}`")),
            None => violations.push(format!("{id} is missing `spec`")),
        }
        if capability
            .next
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            violations.push(format!("{id} is missing `next`"));
        }
        if capability
            .metric
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            violations.push(format!("{id} is missing `metric`"));
        }
        if capability.status.as_deref() != Some("planned") && capability.evidence.is_empty() {
            violations.push(format!("{id} is not planned but has no evidence entries"));
        }
        for fixture in &capability.fixtures {
            if !fixture.trim().is_empty() && !Path::new(fixture).exists() {
                violations.push(format!("{id} fixture path does not exist: {fixture}"));
            }
        }
        if capability.status.as_deref() == Some("stable") && capability.fixtures.is_empty() {
            violations.push(format!("{id} is stable but has no fixture entries"));
        }
        if capability.status.as_deref() == Some("calibrated")
            && !capability
                .evidence
                .iter()
                .any(|value| value.contains("calibration"))
        {
            violations.push(format!(
                "{id} is calibrated but has no calibration evidence entry"
            ));
        }
    }
    Ok(())
}

fn parse_capabilities_manifest(path: &Path) -> Result<(Vec<Capability>, Vec<String>), String> {
    let text = read_text_lossy(path)?;
    let mut capabilities = Vec::new();
    let mut violations = Vec::new();
    let mut current: Option<Capability> = None;
    let mut active_array: Option<(String, Vec<String>, usize)> = None;

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, values, start_line)) = active_array.as_mut() {
            if trimmed.starts_with(']') {
                let Some(mut capability) = current.take() else {
                    violations.push(format!(
                        "{}:{} array `{key}` is outside a capability entry",
                        normalize_path(path),
                        start_line
                    ));
                    active_array = None;
                    continue;
                };
                assign_capability_array(
                    &mut capability,
                    key,
                    values.clone(),
                    *start_line,
                    &mut violations,
                );
                current = Some(capability);
                active_array = None;
                continue;
            }
            match parse_array_item(trimmed) {
                Ok(Some(value)) => values.push(value),
                Ok(None) => {}
                Err(message) => {
                    violations.push(format!("{}:{line_number} {message}", normalize_path(path)))
                }
            }
            continue;
        }
        if trimmed == "[[capability]]" {
            if let Some(capability) = current.take() {
                capabilities.push(capability);
            }
            current = Some(Capability {
                line: line_number,
                ..Capability::default()
            });
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            violations.push(format!(
                "{}:{line_number} expected `key = value`",
                normalize_path(path)
            ));
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let Some(capability) = current.as_mut() else {
            violations.push(format!(
                "{}:{line_number} `{key}` appears outside a [[capability]] entry",
                normalize_path(path)
            ));
            continue;
        };
        if value == "[" {
            active_array = Some((key.to_string(), Vec::new(), line_number));
            continue;
        }
        if value.starts_with('[') {
            match parse_inline_array(value) {
                Ok(values) => {
                    assign_capability_array(capability, key, values, line_number, &mut violations);
                }
                Err(message) => {
                    violations.push(format!("{}:{line_number} {message}", normalize_path(path)))
                }
            }
            continue;
        }
        match parse_quoted_value(value) {
            Ok(parsed) => {
                assign_capability_string(capability, key, parsed, line_number, &mut violations);
            }
            Err(message) => {
                violations.push(format!("{}:{line_number} {message}", normalize_path(path)))
            }
        }
    }

    if let Some((key, _, start_line)) = active_array {
        violations.push(format!(
            "{}:{start_line} array `{key}` is missing closing `]`",
            normalize_path(path)
        ));
    }
    if let Some(capability) = current {
        capabilities.push(capability);
    }
    Ok((capabilities, violations))
}

fn assign_capability_string(
    capability: &mut Capability,
    key: &str,
    value: String,
    line_number: usize,
    violations: &mut Vec<String>,
) {
    match key {
        "id" => capability.id = Some(value),
        "name" => capability.name = Some(value),
        "status" => capability.status = Some(value),
        "spec" => capability.spec = Some(value),
        "next" => capability.next = Some(value),
        "metric" => capability.metric = Some(value),
        _ => violations.push(format!(
            "capability line {line_number} uses unsupported string field `{key}`"
        )),
    }
}

fn assign_capability_array(
    capability: &mut Capability,
    key: &str,
    values: Vec<String>,
    line_number: usize,
    violations: &mut Vec<String>,
) {
    match key {
        "evidence" => capability.evidence = values,
        "fixtures" => capability.fixtures = values,
        _ => violations.push(format!(
            "capability line {line_number} uses unsupported array field `{key}`"
        )),
    }
}

fn capability_metrics_markdown(capabilities: &[Capability]) -> String {
    let mut body = "# ripr capability metrics\n\n".to_string();
    body.push_str("| Capability | Status | Spec | Evidence | Next | Metric |\n");
    body.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for capability in capabilities {
        body.push_str(&format!(
            "| {} | `{}` | `{}` | {} | `{}` | {} |\n",
            markdown_cell(capability.name.as_deref().unwrap_or("")),
            markdown_cell(capability.status.as_deref().unwrap_or("")),
            markdown_cell(capability.spec.as_deref().unwrap_or("")),
            markdown_cell(&capability.evidence.join(", ")),
            markdown_cell(capability.next.as_deref().unwrap_or("")),
            markdown_cell(capability.metric.as_deref().unwrap_or(""))
        ));
    }
    body
}

fn capability_metrics_json(capabilities: &[Capability]) -> String {
    let mut body = "{\n  \"schema_version\": \"0.1\",\n  \"capabilities\": [\n".to_string();
    for (index, capability) in capabilities.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"id\": \"{}\",\n",
            json_escape(capability.id.as_deref().unwrap_or(""))
        ));
        body.push_str(&format!(
            "      \"name\": \"{}\",\n",
            json_escape(capability.name.as_deref().unwrap_or(""))
        ));
        body.push_str(&format!(
            "      \"status\": \"{}\",\n",
            json_escape(capability.status.as_deref().unwrap_or(""))
        ));
        body.push_str(&format!(
            "      \"spec\": \"{}\",\n",
            json_escape(capability.spec.as_deref().unwrap_or(""))
        ));
        body.push_str(&format!(
            "      \"next\": \"{}\",\n",
            json_escape(capability.next.as_deref().unwrap_or(""))
        ));
        body.push_str(&format!(
            "      \"metric\": \"{}\",\n",
            json_escape(capability.metric.as_deref().unwrap_or(""))
        ));
        body.push_str("      \"evidence\": [");
        write_json_string_array(&mut body, &capability.evidence);
        body.push_str("],\n");
        body.push_str("      \"fixtures\": [");
        write_json_string_array(&mut body, &capability.fixtures);
        body.push_str("]\n    }");
    }
    body.push_str("\n  ]\n}\n");
    body
}

fn write_json_string_array(body: &mut String, values: &[String]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            body.push_str(", ");
        }
        body.push('"');
        body.push_str(&json_escape(value));
        body.push('"');
    }
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|")
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn is_snake_case_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && !value.starts_with('_')
        && !value.ends_with('_')
        && !value.contains("__")
}

fn check_workspace_shape() -> Result<(), String> {
    let records = read_pipe_records("policy/workspace_shape.txt", 3)?;
    let mut allowed_members = BTreeSet::new();
    let mut allowed_manifests = Vec::new();
    let mut violations = Vec::new();

    for record in records {
        match record[0].as_str() {
            "workspace_member" => {
                allowed_members.insert(record[1].clone());
            }
            "cargo_manifest" => allowed_manifests.push(GlobAllow {
                glob: record[1].clone(),
            }),
            other => violations.push(format!(
                "policy/workspace_shape.txt uses unsupported kind `{other}`"
            )),
        }
    }

    for member in workspace_members()? {
        if !allowed_members.contains(&member) {
            violations.push(format!(
                "workspace member is not allowlisted: {member}\n  preferred: keep one published `crates/ripr` package and `xtask` automation unless an ADR approves a new package"
            ));
        }
    }
    for member in &allowed_members {
        if !Path::new(member).exists() {
            violations.push(format!(
                "allowlisted workspace member does not exist: {member}"
            ));
        }
    }
    for file in tracked_files()? {
        if !file.ends_with("Cargo.toml") {
            continue;
        }
        if !matches_any_glob(&allowed_manifests, &file) {
            violations.push(format!(
                "Cargo manifest is not allowlisted by workspace shape policy: {file}"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "workspace-shape.md",
            check: "check-workspace-shape",
            why_it_matters: "ripr intentionally stays one published package with internal module seams; new packages need explicit review.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Keep product code inside crates/ripr unless an ADR approves a new package.",
                "Keep repo automation inside xtask.",
                "If a new Cargo manifest is truly needed, add a workspace-shape policy entry with owner and reason.",
            ],
            rerun_command: "cargo xtask check-workspace-shape",
            exception_template: Some("kind|path|reason"),
        },
        &violations,
    )
}

fn check_architecture() -> Result<(), String> {
    let rules = read_pipe_records("policy/architecture.txt", 3)?;
    let files = tracked_files()?;
    let mut violations = Vec::new();
    for rule in rules {
        let glob = &rule[0];
        let forbidden = &rule[1];
        let reason = &rule[2];
        for file in &files {
            if !glob_matches(glob, file) {
                continue;
            }
            let text = read_text_lossy(Path::new(file))?;
            if text.contains(forbidden) {
                violations.push(format!(
                    "{file} contains forbidden architecture pattern `{forbidden}`\n  reason: {reason}"
                ));
            }
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "architecture.md",
            check: "check-architecture",
            why_it_matters: "Internal module seams replace premature crate splits, so dependency direction has to be checked mechanically.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Move rendering logic into output modules.",
                "Keep domain model types independent from CLI, LSP, output, and JSON adapters.",
                "Keep analysis logic out of CLI, LSP, and output adapters.",
                "Update policy/architecture.txt only when the architecture rule itself changes.",
            ],
            rerun_command: "cargo xtask check-architecture",
            exception_template: Some("glob|forbidden_pattern|reason"),
        },
        &violations,
    )
}

fn check_public_api() -> Result<(), String> {
    let allowed = read_public_api_allowlist("policy/public_api.txt")?;
    let actual = public_api_exports(Path::new("crates/ripr/src/lib.rs"))?;
    let allowed_set = allowed.iter().cloned().collect::<BTreeSet<_>>();
    let actual_set = actual.iter().cloned().collect::<BTreeSet<_>>();
    let mut violations = Vec::new();

    for line in &actual {
        if !allowed_set.contains(line) {
            violations.push(format!(
                "public API export is not allowlisted: {line}\n  update policy/public_api.txt only when this is an intentional public contract"
            ));
        }
    }
    for line in &allowed {
        if !actual_set.contains(line) {
            violations.push(format!(
                "public API allowlist entry is missing from crates/ripr/src/lib.rs: {line}"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "public-api.md",
            check: "check-public-api",
            why_it_matters: "The crate is the published product surface, so accidental public exports create compatibility expectations.",
            fix_kind: FixKind::ReviewerDecisionRequired,
            recommended_fixes: &[
                "Keep new implementation modules private unless they are part of the crate contract.",
                "If the public export is intentional, update policy/public_api.txt and explain the contract in the PR.",
                "Prefer output DTOs and app APIs over exposing internal analyzer structures directly.",
            ],
            rerun_command: "cargo xtask check-public-api",
            exception_template: Some("pub mod example;"),
        },
        &violations,
    )
}

fn workspace_members() -> Result<Vec<String>, String> {
    let text = read_text_lossy(Path::new("Cargo.toml"))?;
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(value) = trimmed.strip_prefix("members") else {
            continue;
        };
        let Some((_, raw_array)) = value.split_once('=') else {
            continue;
        };
        return parse_inline_array(raw_array);
    }
    Ok(Vec::new())
}

fn read_public_api_allowlist(path: &str) -> Result<Vec<String>, String> {
    let mut entries = Vec::new();
    let text = read_text_lossy(Path::new(path))?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        entries.push(trimmed.to_string());
    }
    Ok(entries)
}

fn public_api_exports(path: &Path) -> Result<Vec<String>, String> {
    let text = read_text_lossy(path)?;
    let mut exports = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub mod ") || trimmed.starts_with("pub use ") {
            exports.push(trimmed.to_string());
        }
    }
    Ok(exports)
}

fn read_pipe_records(path: &str, field_count: usize) -> Result<Vec<Vec<String>>, String> {
    let text = read_text_lossy(Path::new(path))?;
    let mut records = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let fields = trimmed
            .split('|')
            .map(|field| field.trim().to_string())
            .collect::<Vec<_>>();
        if fields.len() != field_count || fields.iter().any(|field| field.is_empty()) {
            return Err(format!(
                "{}:{} expected {field_count} non-empty pipe-separated fields",
                path,
                index + 1
            ));
        }
        records.push(fields);
    }
    Ok(records)
}

fn check_output_contracts() -> Result<(), String> {
    let records = read_pipe_records("policy/output_contracts.txt", 3)?;
    let mut domain = String::new();
    for path in [
        "crates/ripr/src/domain/mod.rs",
        "crates/ripr/src/domain/classification.rs",
        "crates/ripr/src/domain/evidence.rs",
        "crates/ripr/src/domain/probe.rs",
        "crates/ripr/src/domain/summary.rs",
        "crates/ripr/src/domain/support.rs",
    ] {
        domain.push_str(&read_text_lossy(Path::new(path))?);
        domain.push('\n');
    }
    let app = read_text_lossy(Path::new("crates/ripr/src/app.rs"))?;
    let mut json_output = String::new();
    for path in [
        "crates/ripr/src/output/json/mod.rs",
        "crates/ripr/src/output/json/context_packet.rs",
        "crates/ripr/src/output/json/formatter.rs",
        "crates/ripr/src/output/json/report.rs",
    ] {
        json_output.push_str(&read_text_lossy(Path::new(path))?);
        json_output.push('\n');
    }
    let schema = read_text_lossy(Path::new("docs/OUTPUT_SCHEMA.md"))?;
    let mut violations = Vec::new();
    let mut seen = BTreeSet::new();

    for record in records {
        let kind = &record[0];
        let value = &record[1];
        if !seen.insert(format!("{kind}|{value}")) {
            violations.push(format!("duplicate output contract entry: {kind}|{value}"));
        }
        match kind.as_str() {
            "schema_version" => {
                require_contract_value(
                    "crates/ripr/src/app.rs",
                    &app,
                    value,
                    kind,
                    &mut violations,
                );
                require_contract_value(
                    "docs/OUTPUT_SCHEMA.md",
                    &schema,
                    value,
                    kind,
                    &mut violations,
                );
            }
            "context_version" => {
                require_contract_value(
                    "crates/ripr/src/output/json/",
                    &json_output,
                    value,
                    kind,
                    &mut violations,
                );
                require_contract_value(
                    "docs/OUTPUT_SCHEMA.md",
                    &schema,
                    value,
                    kind,
                    &mut violations,
                );
            }
            "exposure_class" | "severity" | "probe_family" | "delta" | "flow_sink"
            | "stage_state" | "confidence" | "oracle_kind" | "oracle_strength" | "stop_reason"
            | "value_context" => {
                require_contract_value(
                    "crates/ripr/src/domain/",
                    &domain,
                    value,
                    kind,
                    &mut violations,
                );
                require_contract_value(
                    "docs/OUTPUT_SCHEMA.md",
                    &schema,
                    value,
                    kind,
                    &mut violations,
                );
            }
            other => violations.push(format!(
                "policy/output_contracts.txt uses unsupported kind `{other}`"
            )),
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "output-contracts.md",
            check: "check-output-contracts",
            why_it_matters: "Output enum values and schema versions are integration contracts for CLI JSON, LSP diagnostics, CI, and agents.",
            fix_kind: FixKind::ReviewerDecisionRequired,
            recommended_fixes: &[
                "Update policy/output_contracts.txt when a new output enum value is intentionally added.",
                "Update docs/OUTPUT_SCHEMA.md when output values or schema versions change.",
                "Keep static output language within the registered conservative exposure classes.",
            ],
            rerun_command: "cargo xtask check-output-contracts",
            exception_template: Some("kind|value|reason"),
        },
        &violations,
    )
}

fn require_contract_value(
    path: &str,
    text: &str,
    value: &str,
    kind: &str,
    violations: &mut Vec<String>,
) {
    if !text.contains(value) {
        violations.push(format!(
            "{path} does not mention {kind} contract value `{value}`"
        ));
    }
}

fn check_doc_index() -> Result<(), String> {
    let mut violations = Vec::new();
    require_index_mentions_files(
        Path::new("docs/adr/README.md"),
        Path::new("docs/adr"),
        &["README.md"],
        &mut violations,
    )?;
    require_index_mentions_files(
        Path::new("docs/specs/README.md"),
        Path::new("docs/specs"),
        &["README.md"],
        &mut violations,
    )?;

    let documentation = read_text_lossy(Path::new("docs/DOCUMENTATION.md"))?;
    for required in [
        "CODEX_GOALS.md",
        "IMPLEMENTATION_CAMPAIGNS.md",
        "SCOPED_PR_CONTRACT.md",
        "PR_AUTOMATION.md",
        "CAPABILITY_MATRIX.md",
        "METRICS.md",
        "ROADMAP.md",
        "IMPLEMENTATION_PLAN.md",
        "adr/",
        "specs/",
    ] {
        if !documentation.contains(required) {
            violations.push(format!(
                "docs/DOCUMENTATION.md does not reference `{required}`"
            ));
        }
    }

    let readme = read_text_lossy(Path::new("README.md"))?;
    for required in [
        "docs/DOCUMENTATION.md",
        "docs/ROADMAP.md",
        "docs/IMPLEMENTATION_PLAN.md",
        "docs/IMPLEMENTATION_CAMPAIGNS.md",
        "docs/CODEX_GOALS.md",
        "docs/SCOPED_PR_CONTRACT.md",
        "docs/PR_AUTOMATION.md",
        "docs/specs/README.md",
        "docs/adr/README.md",
        "docs/METRICS.md",
        "docs/CAPABILITY_MATRIX.md",
    ] {
        if !readme.contains(required) {
            violations.push(format!("README.md does not reference `{required}`"));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "doc-index.md",
            check: "check-doc-index",
            why_it_matters: "Docs are the durable context for humans and long-context agents; indexes must expose current specs, ADRs, and front-door process docs.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Update docs/adr/README.md when adding or removing ADRs.",
                "Update docs/specs/README.md when adding or removing specs.",
                "Keep README.md and docs/DOCUMENTATION.md linked to the active planning, automation, metrics, ADR, and spec docs.",
            ],
            rerun_command: "cargo xtask check-doc-index",
            exception_template: None,
        },
        &violations,
    )
}

fn check_readme_state() -> Result<(), String> {
    let readme_path = Path::new("README.md");
    let readme = read_text_lossy(readme_path)?;
    let mut violations = Vec::new();

    for heading in [
        "# ripr",
        "## Current Scope",
        "## Current Capability Snapshot",
        "## Supporting Docs",
    ] {
        if !has_markdown_heading(&readme, heading) {
            violations.push(format!("README.md is missing `{heading}`"));
        }
    }

    if !readme.contains("| Capability | Current state | Next checkpoint |") {
        violations
            .push("README.md capability snapshot is missing the expected table header".to_string());
    }

    for capability in [
        "Distribution",
        "Diff analysis",
        "Test discovery",
        "Output",
        "LSP",
        "Agent context",
        "Calibration",
    ] {
        let marker = format!("| {capability} |");
        if !readme.contains(&marker) {
            violations.push(format!(
                "README.md capability snapshot is missing `{capability}`"
            ));
        }
    }

    for required in [
        "docs/METRICS.md",
        "docs/CAPABILITY_MATRIX.md",
        "docs/IMPLEMENTATION_CAMPAIGNS.md",
        "docs/CODEX_GOALS.md",
        "docs/SCOPED_PR_CONTRACT.md",
        "docs/PR_AUTOMATION.md",
        "docs/DOCUMENTATION.md",
    ] {
        if !readme.contains(required) {
            violations.push(format!("README.md does not reference `{required}`"));
        }
    }

    let capabilities_source = read_text_lossy(Path::new("metrics/capabilities.toml"))?;
    let matrix = read_text_lossy(Path::new("docs/CAPABILITY_MATRIX.md"))?;
    if !matrix.contains("metrics/capabilities.toml") {
        violations.push(
            "docs/CAPABILITY_MATRIX.md does not reference metrics/capabilities.toml".to_string(),
        );
    }
    for status in ["planned", "alpha", "stable", "calibrated"] {
        let marker = format!("`{status}`");
        if !matrix.contains(&marker) {
            violations.push(format!(
                "docs/CAPABILITY_MATRIX.md does not describe status `{status}`"
            ));
        }
    }
    for checkpoint in next_checkpoints_from_capabilities(&capabilities_source)? {
        if !checkpoint.trim().is_empty()
            && !readme.contains(&checkpoint)
            && !matrix.contains(&checkpoint)
        {
            violations.push(format!(
                "capability next checkpoint `{checkpoint}` is missing from README.md and docs/CAPABILITY_MATRIX.md"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "readme-state.md",
            check: "check-readme-state",
            why_it_matters: "README is the front door for humans and Codex Goals state recovery; it should summarize current capability without drifting from metrics and campaign docs.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Keep README.md linked to active planning, metrics, campaign, and automation docs.",
                "Keep README's capability snapshot compact and aligned with docs/CAPABILITY_MATRIX.md.",
                "Update metrics/capabilities.toml and docs/CAPABILITY_MATRIX.md when capability status or next checkpoints change.",
            ],
            rerun_command: "cargo xtask check-readme-state",
            exception_template: None,
        },
        &violations,
    )
}

fn markdown_links() -> Result<(), String> {
    let mut violations = Vec::new();
    for file in tracked_files()? {
        if !file.ends_with(".md") {
            continue;
        }
        if should_skip_path(&file) {
            continue;
        }
        let path = Path::new(&file);
        let text = read_text_lossy(path)?;
        for link in markdown_links_in_text(&text) {
            let Some(target_path) = local_markdown_target(&link.target) else {
                continue;
            };
            let resolved = resolve_markdown_link(path, &target_path);
            if !resolved.exists() {
                violations.push(format!(
                    "{file}:{} links to missing local target `{}`",
                    link.line, link.target
                ));
            }
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "markdown-links.md",
            check: "markdown-links",
            why_it_matters: "Markdown links are repo state for humans and long-context agents; deleted or renamed docs should fail before review.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Update links when docs are renamed or deleted.",
                "Use relative links for repo-local Markdown targets.",
                "Run cargo xtask markdown-links before opening docs-heavy PRs.",
            ],
            rerun_command: "cargo xtask markdown-links",
            exception_template: None,
        },
        &violations,
    )
}

fn next_checkpoints_from_capabilities(text: &str) -> Result<Vec<String>, String> {
    let mut checkpoints = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("next") {
            continue;
        }
        let Some((_, value)) = trimmed.split_once('=') else {
            return Err(format!(
                "metrics/capabilities.toml:{} expected `next = \"...\"`",
                line_number + 1
            ));
        };
        checkpoints.push(parse_quoted_value(value.trim()).map_err(|message| {
            format!("metrics/capabilities.toml:{} {message}", line_number + 1)
        })?);
    }
    Ok(checkpoints)
}

fn markdown_links_in_text(text: &str) -> Vec<MarkdownLink> {
    let mut links = Vec::new();
    let mut in_fence = false;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        links.extend(markdown_links_in_line(line, index + 1));
    }
    links
}

fn markdown_links_in_line(line: &str, line_number: usize) -> Vec<MarkdownLink> {
    let mut links = Vec::new();
    let mut offset = 0usize;
    while let Some(start) = line[offset..].find("](") {
        let target_start = offset + start + 2;
        let Some(end) = line[target_start..].find(')') else {
            break;
        };
        let target = line[target_start..target_start + end].trim();
        if !target.is_empty() {
            links.push(MarkdownLink {
                line: line_number,
                target: target.to_string(),
            });
        }
        offset = target_start + end + 1;
    }
    links
}

fn local_markdown_target(raw_target: &str) -> Option<String> {
    let mut target = raw_target.trim();
    if target.starts_with('<') {
        let end = target.find('>')?;
        target = &target[1..end];
    } else if let Some((first, _)) = target.split_once(char::is_whitespace) {
        target = first;
    }
    if target.is_empty() || target.starts_with('#') {
        return None;
    }
    let lower = target.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("app://")
        || lower.starts_with("plugin://")
    {
        return None;
    }
    let without_query = target.split('?').next().unwrap_or(target);
    let without_anchor = without_query.split('#').next().unwrap_or(without_query);
    let local = without_anchor.trim();
    if local.is_empty() {
        None
    } else {
        Some(local.trim_start_matches('/').to_string())
    }
}

fn resolve_markdown_link(source: &Path, target: &str) -> PathBuf {
    let target_path = Path::new(target);
    if target_path.is_absolute() {
        target_path.to_path_buf()
    } else {
        source
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(target_path)
    }
}

fn goals(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("status") | Some("report") | None => goals_status(),
        Some("next") => goals_next(),
        Some(other) => Err(format!(
            "unknown goals command `{other}`\nusage: cargo xtask goals status\n       cargo xtask goals next\n       cargo xtask goals report"
        )),
    }
}

fn check_campaign() -> Result<(), String> {
    let mut violations = Vec::new();
    let manifest_path = Path::new(".ripr/goals/active.toml");
    if !manifest_path.exists() {
        violations.push(".ripr/goals/active.toml is missing".to_string());
        return finish_campaign_report(&violations);
    }

    let (manifest, parse_violations) = parse_campaign_manifest(manifest_path)?;
    violations.extend(parse_violations);
    validate_campaign_manifest(&manifest, &mut violations)?;
    finish_campaign_report(&violations)
}

fn goals_status() -> Result<(), String> {
    let manifest_path = Path::new(".ripr/goals/active.toml");
    let (manifest, parse_violations) = parse_campaign_manifest(manifest_path)?;
    let mut violations = parse_violations;
    validate_campaign_manifest(&manifest, &mut violations)?;
    let body = campaign_status_report_body(&manifest, &violations);
    write_report("goals.md", &body)?;
    println!("{body}");
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "goals status found campaign issues; see target/ripr/reports/goals.md\n{}",
            violations.join("\n")
        ))
    }
}

fn goals_next() -> Result<(), String> {
    let manifest_path = Path::new(".ripr/goals/active.toml");
    let (manifest, parse_violations) = parse_campaign_manifest(manifest_path)?;
    let mut violations = parse_violations;
    validate_campaign_manifest(&manifest, &mut violations)?;
    let body = campaign_next_report_body(&manifest, &violations);
    write_report("goals-next.md", &body)?;
    println!("{body}");
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "goals next found campaign issues; see target/ripr/reports/goals-next.md\n{}",
            violations.join("\n")
        ))
    }
}

fn finish_campaign_report(violations: &[String]) -> Result<(), String> {
    finish_policy_report(
        PolicyReportSpec {
            report_file: "campaign.md",
            check: "check-campaign",
            why_it_matters: "Codex Goals use .ripr/goals/active.toml as the durable campaign queue; drift here sends agents toward the wrong work item.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Keep .ripr/goals/active.toml synchronized with docs/IMPLEMENTATION_CAMPAIGNS.md.",
                "Use only done, active, ready, or blocked work item statuses.",
                "Give every non-blocked work item a branch, acceptance claim, and valid command list.",
                "Use blocked_by or blocked_reason when a work item is blocked.",
            ],
            rerun_command: "cargo xtask check-campaign",
            exception_template: None,
        },
        violations,
    )
}

fn validate_campaign_manifest(
    manifest: &CampaignManifest,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let docs = read_text_lossy(Path::new("docs/IMPLEMENTATION_CAMPAIGNS.md"))?;
    let mut ids = BTreeSet::new();
    let mut statuses_by_id = BTreeMap::new();

    let Some(id) = manifest.id.as_ref() else {
        violations.push(".ripr/goals/active.toml is missing campaign `id`".to_string());
        return Ok(());
    };
    if !is_kebab_case_id(id) {
        violations.push(format!("campaign id `{id}` must use kebab-case"));
    }
    if !docs.contains(id) {
        violations.push(format!(
            "docs/IMPLEMENTATION_CAMPAIGNS.md does not mention active campaign id `{id}`"
        ));
    }
    match manifest.status.as_deref() {
        Some("active") => {}
        Some(status) => violations.push(format!("campaign has unsupported status `{status}`")),
        None => violations.push("campaign is missing `status`".to_string()),
    }
    if manifest
        .title
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        violations.push("campaign is missing non-empty `title`".to_string());
    }
    if manifest.end_state.is_empty() {
        violations.push("campaign has no end_state entries".to_string());
    }
    if manifest.work_items.is_empty() {
        violations.push("campaign has no [[work_item]] entries".to_string());
    }

    for item in &manifest.work_items {
        let Some(item_id) = item.id.as_ref() else {
            violations.push(format!("work item at line {} is missing `id`", item.line));
            continue;
        };
        if !ids.insert(item_id.clone()) {
            violations.push(format!("duplicate work item id `{item_id}`"));
        }
        if !is_work_item_id(item_id) {
            violations.push(format!(
                "work item id `{item_id}` must look like `scope/name`"
            ));
        }
        if !docs.contains(&format!("`{item_id}`")) {
            violations.push(format!(
                "docs/IMPLEMENTATION_CAMPAIGNS.md does not list work item `{item_id}`"
            ));
        }

        match item.status.as_deref() {
            Some("done" | "active" | "ready" | "blocked") => {
                if let Some(status) = item.status.as_ref() {
                    statuses_by_id.insert(item_id.clone(), status.clone());
                    let expected_row = format!("| `{item_id}` | {status} |");
                    if !docs.contains(&expected_row) {
                        violations.push(format!(
                            "docs/IMPLEMENTATION_CAMPAIGNS.md does not show `{item_id}` as `{status}`"
                        ));
                    }
                }
            }
            Some(status) => violations.push(format!(
                "{item_id} has unsupported status `{status}`; use done, active, ready, or blocked"
            )),
            None => violations.push(format!("{item_id} is missing `status`")),
        }

        if item
            .branch
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            violations.push(format!("{item_id} is missing `branch`"));
        }
        if item.stackable.is_none() {
            violations.push(format!("{item_id} is missing `stackable`"));
        }
        if item.requires_human_merge.is_none() {
            violations.push(format!("{item_id} is missing `requires_human_merge`"));
        }
        if item
            .acceptance
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            violations.push(format!("{item_id} is missing `acceptance`"));
        }
        if item.status.as_deref() != Some("blocked") && item.commands.is_empty() {
            violations.push(format!("{item_id} is missing command entries"));
        }
        for command in &item.commands {
            if !is_known_campaign_command(command) {
                violations.push(format!(
                    "{item_id} lists unknown or unsupported command `{command}`"
                ));
            }
        }
        if item.status.as_deref() == Some("blocked")
            && item.blocked_by.is_empty()
            && item
                .blocked_reason
                .as_ref()
                .is_none_or(|value| value.trim().is_empty())
        {
            violations.push(format!(
                "{item_id} is blocked but has no blocked_by or blocked_reason"
            ));
        }
    }

    for item in &manifest.work_items {
        let Some(item_id) = item.id.as_ref() else {
            continue;
        };
        for dependency in &item.blocked_by {
            match statuses_by_id.get(dependency) {
                Some(status) if status == "done" => {}
                Some(status) if item.status.as_deref() == Some("ready") => {
                    violations.push(format!(
                        "{item_id} is ready but dependency `{dependency}` is `{status}`"
                    ));
                }
                Some(_) => {}
                None => violations.push(format!(
                    "{item_id} references missing blocked_by item `{dependency}`"
                )),
            }
        }
    }

    let active_non_stackable = manifest
        .work_items
        .iter()
        .filter(|item| item.status.as_deref() == Some("active") && item.stackable != Some(true))
        .count();
    if active_non_stackable > 1 {
        violations.push(format!(
            "campaign has {active_non_stackable} active non-stackable work items; use at most one"
        ));
    }

    Ok(())
}

fn campaign_status_report_body(manifest: &CampaignManifest, violations: &[String]) -> String {
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let mut body = format!("# ripr goals status\n\nStatus: {status}\n\n");
    body.push_str("## Campaign\n\n");
    body.push_str(&format!(
        "- id: `{}`\n",
        manifest.id.as_deref().unwrap_or("<missing>")
    ));
    body.push_str(&format!(
        "- title: {}\n",
        manifest.title.as_deref().unwrap_or("<missing>")
    ));
    body.push_str(&format!(
        "- status: `{}`\n\n",
        manifest.status.as_deref().unwrap_or("<missing>")
    ));

    body.push_str("## Work Items\n\n");
    body.push_str("| Work item | Status | Branch | Stackable | Commands |\n");
    body.push_str("| --- | --- | --- | --- | ---: |\n");
    for item in &manifest.work_items {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | {} |\n",
            item.id.as_deref().unwrap_or("<missing>"),
            item.status.as_deref().unwrap_or("<missing>"),
            item.branch.as_deref().unwrap_or("<missing>"),
            item.stackable
                .map(|value| value.to_string())
                .unwrap_or_else(|| "<missing>".to_string()),
            item.commands.len()
        ));
    }
    body.push('\n');
    write_violations_section(&mut body, violations);
    body
}

fn campaign_next_report_body(manifest: &CampaignManifest, violations: &[String]) -> String {
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let mut body = format!("# ripr goals next\n\nStatus: {status}\n\n");
    body.push_str("## Ready Work Items\n\n");
    let ready = manifest
        .work_items
        .iter()
        .filter(|item| item.status.as_deref() == Some("ready"))
        .collect::<Vec<_>>();
    if ready.is_empty() {
        body.push_str("No ready work items.\n\n");
    } else {
        for item in ready {
            body.push_str(&format!(
                "- `{}` on branch `{}`\n",
                item.id.as_deref().unwrap_or("<missing>"),
                item.branch.as_deref().unwrap_or("<missing>")
            ));
            if let Some(acceptance) = item.acceptance.as_ref() {
                body.push_str(&format!("  acceptance: {acceptance}\n"));
            }
            if !item.commands.is_empty() {
                body.push_str("  commands:\n");
                for command in &item.commands {
                    body.push_str(&format!("  - `{command}`\n"));
                }
            }
        }
        body.push('\n');
    }
    write_violations_section(&mut body, violations);
    body
}

fn parse_campaign_manifest(path: &Path) -> Result<(CampaignManifest, Vec<String>), String> {
    let text = read_text_lossy(path)?;
    let mut manifest = CampaignManifest::default();
    let mut violations = Vec::new();
    let mut current: Option<CampaignWorkItem> = None;
    let mut active_array: Option<(String, Vec<String>, usize)> = None;
    let mut active_multiline: Option<(String, usize)> = None;

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if let Some((key, _start_line)) = active_multiline.clone() {
            if trimmed.contains("\"\"\"") {
                active_multiline = None;
            }
            if key != "objective" {
                violations.push(format!(
                    "{}:{} unsupported multiline field `{key}`",
                    normalize_path(path),
                    line_number
                ));
            }
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, values, start_line)) = active_array.as_mut() {
            if trimmed.starts_with(']') {
                assign_campaign_array(&mut manifest, &mut current, key, values.clone());
                active_array = None;
                continue;
            }
            match parse_array_item(trimmed) {
                Ok(Some(value)) => values.push(value),
                Ok(None) => {}
                Err(message) => {
                    violations.push(format!("{}:{line_number} {message}", normalize_path(path)))
                }
            }
            if line_number < *start_line {
                violations.push(format!(
                    "{}:{} invalid array state",
                    normalize_path(path),
                    line_number
                ));
            }
            continue;
        }
        if trimmed == "[[work_item]]" {
            if let Some(item) = current.take() {
                manifest.work_items.push(item);
            }
            current = Some(CampaignWorkItem {
                line: line_number,
                ..CampaignWorkItem::default()
            });
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            violations.push(format!(
                "{}:{line_number} expected `key = value`",
                normalize_path(path)
            ));
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if value.starts_with("\"\"\"") {
            if !value.trim_start_matches("\"\"\"").contains("\"\"\"") {
                active_multiline = Some((key.to_string(), line_number));
            }
            continue;
        }
        if value == "[" {
            active_array = Some((key.to_string(), Vec::new(), line_number));
            continue;
        }
        if value.starts_with('[') {
            match parse_inline_array(value) {
                Ok(values) => assign_campaign_array(&mut manifest, &mut current, key, values),
                Err(message) => {
                    violations.push(format!("{}:{line_number} {message}", normalize_path(path)))
                }
            }
            continue;
        }
        assign_campaign_scalar(
            &mut manifest,
            &mut current,
            key,
            value,
            line_number,
            &mut violations,
        );
    }

    if let Some((key, start_line)) = active_multiline {
        violations.push(format!(
            "{}:{start_line} multiline field `{key}` is missing closing triple quotes",
            normalize_path(path)
        ));
    }
    if let Some((key, _, start_line)) = active_array {
        violations.push(format!(
            "{}:{start_line} array `{key}` is missing closing `]`",
            normalize_path(path)
        ));
    }
    if let Some(item) = current {
        manifest.work_items.push(item);
    }
    Ok((manifest, violations))
}

fn assign_campaign_array(
    manifest: &mut CampaignManifest,
    current: &mut Option<CampaignWorkItem>,
    key: &str,
    values: Vec<String>,
) {
    if let Some(item) = current.as_mut() {
        match key {
            "commands" => item.commands = values,
            "blocked_by" => item.blocked_by = values,
            _ => {}
        }
    } else if key == "end_state" {
        manifest.end_state = values;
    }
}

fn assign_campaign_scalar(
    manifest: &mut CampaignManifest,
    current: &mut Option<CampaignWorkItem>,
    key: &str,
    value: &str,
    line_number: usize,
    violations: &mut Vec<String>,
) {
    if let Some(item) = current.as_mut() {
        match key {
            "id" => assign_quoted_campaign_value(value, line_number, violations, |parsed| {
                item.id = Some(parsed);
            }),
            "status" => assign_quoted_campaign_value(value, line_number, violations, |parsed| {
                item.status = Some(parsed);
            }),
            "branch" => assign_quoted_campaign_value(value, line_number, violations, |parsed| {
                item.branch = Some(parsed);
            }),
            "acceptance" => {
                assign_quoted_campaign_value(value, line_number, violations, |parsed| {
                    item.acceptance = Some(parsed);
                })
            }
            "blocked_reason" => {
                assign_quoted_campaign_value(value, line_number, violations, |parsed| {
                    item.blocked_reason = Some(parsed);
                });
            }
            "stackable" => item.stackable = parse_campaign_bool(value, line_number, violations),
            "requires_human_merge" => {
                item.requires_human_merge = parse_campaign_bool(value, line_number, violations);
            }
            _ => violations.push(format!(
                "campaign manifest line {line_number} uses unsupported work_item field `{key}`"
            )),
        }
    } else {
        match key {
            "id" => assign_quoted_campaign_value(value, line_number, violations, |parsed| {
                manifest.id = Some(parsed);
            }),
            "title" => assign_quoted_campaign_value(value, line_number, violations, |parsed| {
                manifest.title = Some(parsed);
            }),
            "status" => assign_quoted_campaign_value(value, line_number, violations, |parsed| {
                manifest.status = Some(parsed);
            }),
            _ => violations.push(format!(
                "campaign manifest line {line_number} uses unsupported campaign field `{key}`"
            )),
        }
    }
}

fn assign_quoted_campaign_value(
    value: &str,
    line_number: usize,
    violations: &mut Vec<String>,
    assign: impl FnOnce(String),
) {
    match parse_quoted_value(value) {
        Ok(parsed) => assign(parsed),
        Err(message) => violations.push(format!("campaign manifest line {line_number}: {message}")),
    }
}

fn parse_campaign_bool(
    value: &str,
    line_number: usize,
    violations: &mut Vec<String>,
) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        other => {
            violations.push(format!(
                "campaign manifest line {line_number}: expected boolean, got `{other}`"
            ));
            None
        }
    }
}

fn is_kebab_case_id(value: &str) -> bool {
    let mut previous_dash = false;
    let mut saw_char = false;
    for byte in value.bytes() {
        match byte {
            b'a'..=b'z' | b'0'..=b'9' => {
                saw_char = true;
                previous_dash = false;
            }
            b'-' if saw_char && !previous_dash => previous_dash = true,
            _ => return false,
        }
    }
    saw_char && !previous_dash
}

fn is_work_item_id(value: &str) -> bool {
    let Some((scope, name)) = value.split_once('/') else {
        return false;
    };
    is_kebab_case_id(scope) && is_kebab_case_id(name)
}

fn is_known_campaign_command(command: &str) -> bool {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return false;
    }
    if let Some(rest) = trimmed.strip_prefix("cargo xtask ") {
        let command_name = rest.split_whitespace().next().unwrap_or_default();
        return known_xtask_command(command_name);
    }
    trimmed.starts_with("cargo fmt")
        || trimmed.starts_with("cargo check")
        || trimmed.starts_with("cargo test")
        || trimmed.starts_with("cargo clippy")
        || trimmed.starts_with("cargo doc")
        || trimmed.starts_with("cargo package")
        || trimmed.starts_with("cargo publish")
        || trimmed.starts_with("npm ")
}

fn known_xtask_command(command: &str) -> bool {
    known_commands()
        .into_iter()
        .map(known_command_root)
        .any(|known| known == command)
}

fn require_index_mentions_files(
    index_path: &Path,
    directory: &Path,
    excluded_names: &[&str],
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let index = read_text_lossy(index_path)?;
    for path in collect_files(directory)? {
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if excluded_names.contains(&name) {
            continue;
        }
        if !index.contains(name) {
            violations.push(format!(
                "{} does not index {}",
                normalize_path(index_path),
                normalize_path(&path)
            ));
        }
    }
    Ok(())
}

fn check_generated() -> Result<(), String> {
    let allowlist = read_glob_allowlist("policy/generated_allowlist.txt")?;
    let mut violations = Vec::new();

    for normalized in tracked_files()? {
        if !is_generated_candidate(&normalized) {
            continue;
        }
        if !matches_any_glob(&allowlist, &normalized) {
            violations.push(format!(
                "tracked generated output is not allowlisted: {normalized}\n  preferred: keep generated outputs out of git unless they are an intentional lockfile or fixture golden"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "generated.md",
            check: "check-generated",
            why_it_matters: "Generated files should be reproducible and intentionally checked in only for approved surfaces such as lockfiles or fixture goldens.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Remove accidental build artifacts from git.",
                "Regenerate approved outputs from their source command.",
                "Add an allowlist entry only when the generated file is an intentional repository artifact.",
            ],
            rerun_command: "cargo xtask check-generated",
            exception_template: Some(
                "policy/generated_allowlist.txt entry:\nglob|kind|owner|reason",
            ),
        },
        &violations,
    )
}

fn check_dependencies() -> Result<(), String> {
    let allowlist = read_glob_allowlist("policy/dependency_allowlist.txt")?;
    let mut violations = Vec::new();

    for normalized in tracked_files()? {
        if !is_dependency_surface_candidate(&normalized) {
            continue;
        }
        if !matches_any_glob(&allowlist, &normalized) {
            violations.push(format!(
                "dependency surface is not allowlisted: {normalized}\n  preferred: keep dependency managers scoped to approved Cargo, VS Code, or fixture surfaces"
            ));
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "dependencies.md",
            check: "check-dependencies",
            why_it_matters: "Dependency manager surfaces change build and supply-chain behavior, so they need an explicit owner and reason.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Keep dependency files inside approved Cargo, VS Code, or fixture surfaces.",
                "Explain new dependency surfaces in the PR.",
                "Add an allowlist entry only when the surface is intentional.",
            ],
            rerun_command: "cargo xtask check-dependencies",
            exception_template: Some(
                "policy/dependency_allowlist.txt entry:\nglob|kind|owner|reason",
            ),
        },
        &violations,
    )
}

fn check_supply_chain() -> Result<(), String> {
    ensure_reports_dir()?;

    let args = ["deny", "check", "advisories", "licenses", "bans", "sources"];
    eprintln!("$ cargo {}", args.join(" "));
    let output = capture_output("cargo", &args, "cargo deny")?;

    let status = if output.status.success() {
        "pass"
    } else {
        "fail"
    };
    let stdout = redact_current_dir(&output.stdout);
    let stderr = redact_current_dir(&output.stderr);
    let mut body = format!(
        "# ripr supply-chain report\n\nStatus: {status}\n\nCommand:\n\n```bash\ncargo deny check advisories licenses bans sources\n```\n\n"
    );
    body.push_str("Policy:\n\n");
    body.push_str(
        "- advisories, licenses, bans, and source registries are checked by `cargo-deny`.\n",
    );
    body.push_str(
        "- duplicate dependency findings are warnings in `deny.toml` during baseline setup.\n\n",
    );
    body.push_str("Output:\n\n```text\n");
    if stdout.is_empty() && stderr.is_empty() {
        body.push_str("<no output>\n");
    } else {
        body.push_str(&stdout);
        if !stdout.ends_with('\n') && !stdout.is_empty() {
            body.push('\n');
        }
        body.push_str(&stderr);
        if !stderr.ends_with('\n') && !stderr.is_empty() {
            body.push('\n');
        }
    }
    body.push_str("```\n");
    write_report("supply-chain.md", &body)?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "cargo deny check advisories licenses bans sources failed with {}",
            output.status
        ))
    }
}

fn redact_current_dir(text: &str) -> String {
    let Ok(current_dir) = std::env::current_dir() else {
        return text.to_string();
    };
    let current_dir = current_dir.display().to_string();
    let slash_dir = current_dir.replace('\\', "/");
    text.replace(&current_dir, ".").replace(&slash_dir, ".")
}

fn check_process_policy_impl() -> Result<(), String> {
    check_count_policy(
        "process policy",
        "policy/process_allowlist.txt",
        &process_policy_patterns(),
        is_process_policy_candidate,
    )
}

fn check_network_policy_impl() -> Result<(), String> {
    check_count_policy(
        "network policy",
        "policy/network_allowlist.txt",
        &network_policy_patterns(),
        is_network_policy_candidate,
    )
}

fn check_count_policy(
    label: &str,
    allowlist_path: &str,
    patterns: &[String],
    is_candidate: fn(&str) -> bool,
) -> Result<(), String> {
    let allowlist = read_count_policy_allowlist(allowlist_path)?;
    let mut counts = BTreeMap::<(String, String), usize>::new();

    for normalized in tracked_files()? {
        if !is_candidate(&normalized) {
            continue;
        }
        let text = read_text_lossy(Path::new(&normalized))?;
        for pattern in patterns {
            let count = text.matches(pattern).count();
            if count > 0 {
                counts.insert((normalized.clone(), pattern.clone()), count);
            }
        }
    }

    let mut violations = Vec::new();
    for ((path, pattern), count) in &counts {
        let allowed = allowlist
            .get(&(path.clone(), pattern.clone()))
            .copied()
            .unwrap_or(0);
        if *count > allowed {
            violations.push(format!(
                "{path} contains `{pattern}` {count} time(s), allowed {allowed}"
            ));
        }
    }

    let report_file = format!("{}.md", label.replace(' ', "-"));
    let why = format!(
        "{label} entries are explicit because hidden side effects make automation and analyzer behavior harder to review."
    );
    let template = format!("{allowlist_path} entry:\npath|pattern|max_count|owner|reason");
    let check = format!("check-{}", label.replace(' ', "-"));
    let rerun_command = format!("cargo xtask {check}");
    finish_policy_report(
        PolicyReportSpec {
            report_file: &report_file,
            check: &check,
            why_it_matters: &why,
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Move the behavior to the approved adapter or automation surface.",
                "Reduce the process or network usage when it is not required.",
                "Add an allowlist entry only when the behavior is intentional and owned.",
            ],
            rerun_command: &rerun_command,
            exception_template: Some(&template),
        },
        &violations,
    )
}

fn sort_allowlist_files() -> Result<Vec<String>, String> {
    let mut changed = Vec::new();
    for root in [Path::new(".ripr"), Path::new("policy")] {
        if !root.exists() {
            continue;
        }
        for path in collect_files(root)? {
            if path.extension().and_then(|value| value.to_str()) != Some("txt") {
                continue;
            }
            let original = read_text_lossy(&path)?;
            let sorted = sorted_allowlist_content(&original);
            if sorted != original {
                fs::write(&path, sorted).map_err(|err| {
                    format!(
                        "failed to write sorted allowlist {}: {err}\nrerun with `cargo xtask shape` after fixing file permissions",
                        path.display()
                    )
                })?;
                changed.push(normalize_path(&path));
            }
        }
    }
    changed.sort();
    Ok(changed)
}

fn sorted_allowlist_content(text: &str) -> String {
    let mut prefix = Vec::new();
    let mut entries = Vec::new();
    let mut saw_entry = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if !saw_entry && (trimmed.is_empty() || trimmed.starts_with('#')) {
            prefix.push(line.trim_end().to_string());
            continue;
        }
        saw_entry = true;
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            entries.push(trimmed.to_string());
        }
    }

    entries.sort();
    let mut output = String::new();
    if !prefix.is_empty() {
        output.push_str(&prefix.join("\n"));
        output.push('\n');
    }
    if !entries.is_empty() {
        if !output.ends_with("\n\n") {
            output.push('\n');
        }
        output.push_str(&entries.join("\n"));
        output.push('\n');
    }
    if output.is_empty() {
        output.push('\n');
    }
    output
}

fn shape_report_body(sorted: &[String]) -> String {
    let mut body = String::from(
        "# ripr shape report\n\nStatus: pass\n\nActions:\n\n- Ran `cargo fmt`.\n- Ensured `target/ripr/reports` exists.\n",
    );
    if sorted.is_empty() {
        body.push_str("- Allowlist files were already sorted.\n");
    } else {
        body.push_str("- Sorted allowlist files:\n");
        for path in sorted {
            body.push_str(&format!("  - `{path}`\n"));
        }
    }
    body.push_str("\nNext commands:\n\n```bash\ncargo xtask ci-fast\n```\n");
    body
}

fn ensure_reports_dir() -> Result<(), String> {
    fs::create_dir_all(reports_dir()).map_err(|err| {
        format!(
            "failed to create {}: {err}\nrerun with `cargo xtask shape` after fixing directory permissions",
            reports_dir().display()
        )
    })
}

fn ensure_receipts_dir() -> Result<(), String> {
    fs::create_dir_all(receipts_dir()).map_err(|err| {
        format!(
            "failed to create {}: {err}\nrerun with `cargo xtask receipts` after fixing directory permissions",
            receipts_dir().display()
        )
    })
}

fn write_report(file_name: &str, body: &str) -> Result<(), String> {
    ensure_reports_dir()?;
    let path = reports_dir().join(file_name);
    fs::write(&path, body).map_err(|err| {
        format!(
            "failed to write {}: {err}\nrerun with `cargo xtask shape` after fixing file permissions",
            path.display()
        )
    })
}

fn write_receipt(file_name: &str, body: &str) -> Result<(), String> {
    ensure_receipts_dir()?;
    let path = receipts_dir().join(file_name);
    fs::write(&path, body).map_err(|err| {
        format!(
            "failed to write {}: {err}\nrerun with `cargo xtask receipts` after fixing file permissions",
            path.display()
        )
    })
}

fn reports_dir() -> PathBuf {
    Path::new("target").join("ripr").join("reports")
}

fn receipts_dir() -> PathBuf {
    Path::new("target").join("ripr").join("receipts")
}

fn report_index_campaign() -> ReportIndexCampaign {
    let path = Path::new(".ripr/goals/active.toml");
    match parse_campaign_manifest(path) {
        Ok((manifest, violations)) => {
            let ready_work_items = manifest
                .work_items
                .iter()
                .filter(|item| item.status.as_deref() == Some("ready"))
                .filter_map(|item| item.id.clone())
                .collect::<Vec<_>>();
            ReportIndexCampaign {
                id: manifest.id.unwrap_or_else(|| "unknown".to_string()),
                title: manifest.title.unwrap_or_else(|| "unknown".to_string()),
                status: manifest.status.unwrap_or_else(|| "unknown".to_string()),
                ready_work_items,
                issues: violations,
            }
        }
        Err(err) => ReportIndexCampaign {
            id: "unknown".to_string(),
            title: "unknown".to_string(),
            status: "unknown".to_string(),
            ready_work_items: Vec::new(),
            issues: vec![err],
        },
    }
}

fn report_index_entries() -> Result<Vec<ReportIndexEntry>, String> {
    file_index_entries(&reports_dir(), &["index.md", "index.json"])
}

fn receipt_index_entries() -> Result<Vec<ReportIndexEntry>, String> {
    file_index_entries(&receipts_dir(), &[])
}

fn file_index_entries(dir: &Path, exclude_names: &[&str]) -> Result<Vec<ReportIndexEntry>, String> {
    let mut entries = Vec::new();
    if !dir.exists() {
        return Ok(entries);
    }
    for entry in
        fs::read_dir(dir).map_err(|err| format!("failed to read {}: {err}", normalize_path(dir)))?
    {
        let entry = entry
            .map_err(|err| format!("failed to read entry under {}: {err}", normalize_path(dir)))?;
        let file_type = entry.file_type().map_err(|err| {
            format!(
                "failed to read file type for {}: {err}",
                normalize_path(&entry.path())
            )
        })?;
        if !file_type.is_file() {
            continue;
        }
        let file = entry.file_name().to_string_lossy().to_string();
        if exclude_names.iter().any(|name| *name == file) {
            continue;
        }
        let path = entry.path();
        entries.push(ReportIndexEntry {
            file,
            path: normalize_path(&path),
            status: report_entry_status(&path),
        });
    }
    entries.sort_by(|left, right| left.file.cmp(&right.file));
    Ok(entries)
}

fn report_entry_status(path: &Path) -> String {
    if normalize_path(path).ends_with("target/ripr/reports/metrics.json") {
        return "present".to_string();
    }
    match read_text_lossy(path) {
        Ok(text) => report_status_from_text(&text).unwrap_or_else(|| "present".to_string()),
        Err(_) => "unreadable".to_string(),
    }
}

fn report_status_from_text(text: &str) -> Option<String> {
    for line in text.lines().take(24) {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Status:") {
            return Some(normalize_report_status(rest));
        }
        if let Some((_, rest)) = trimmed.split_once("\"status\"")
            && let Some((_, value)) = rest.split_once(':')
        {
            return Some(normalize_report_status(value));
        }
    }
    None
}

fn normalize_report_status(value: &str) -> String {
    let cleaned = value.trim().trim_matches(|ch| {
        ch == '"'
            || ch == '\''
            || ch == '`'
            || ch == ','
            || ch == '{'
            || ch == '}'
            || ch == '['
            || ch == ']'
    });
    let lower = cleaned.to_ascii_lowercase();
    let mut token = String::new();
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            token.push(ch);
        } else {
            break;
        }
    }
    if token.is_empty() {
        "present".to_string()
    } else {
        token
    }
}

fn report_index_missing_expected(
    reports: &[ReportIndexEntry],
    changes: &[ChangedPath],
) -> Vec<String> {
    let existing = reports
        .iter()
        .map(|entry| entry.file.clone())
        .collect::<BTreeSet<_>>();
    let mut expected = BTreeSet::<String>::new();
    expected.insert("pr-summary.md".to_string());
    expected.insert("check-pr.md".to_string());

    if changes.iter().any(|change| is_docs_path(&change.path)) {
        expected.insert("doc-index.md".to_string());
        expected.insert("markdown-links.md".to_string());
    }
    if changes
        .iter()
        .any(|change| change.path == "README.md" || change.path == "docs/CAPABILITY_MATRIX.md")
    {
        expected.insert("readme-state.md".to_string());
    }
    if changes.iter().any(|change| is_campaign_path(&change.path)) {
        expected.insert("campaign.md".to_string());
        expected.insert("goals-next.md".to_string());
    }
    if changes.iter().any(|change| is_analysis_path(&change.path)) {
        expected.insert("pr-shape.md".to_string());
        expected.insert("fixtures.md".to_string());
        expected.insert("goldens.md".to_string());
        expected.insert("capabilities.md".to_string());
    }
    if changes
        .iter()
        .any(|change| is_output_surface_path(&change.path))
    {
        expected.insert("output-contracts.md".to_string());
        expected.insert("fixtures.md".to_string());
        expected.insert("goldens.md".to_string());
        expected.insert("golden-drift.md".to_string());
    }
    if changes.iter().any(|change| is_fixture_path(&change.path)) {
        expected.insert("fixtures.md".to_string());
        expected.insert("goldens.md".to_string());
        expected.insert("golden-drift.md".to_string());
    }
    if changes.iter().any(|change| is_metrics_path(&change.path)) {
        expected.insert("capabilities.md".to_string());
        expected.insert("metrics.md".to_string());
    }

    expected
        .into_iter()
        .filter(|file| !existing.contains(file))
        .map(|file| format!("target/ripr/reports/{file}"))
        .collect()
}

fn is_docs_path(path: &str) -> bool {
    path == "README.md"
        || path == "AGENTS.md"
        || path == "CONTRIBUTING.md"
        || path == "CHANGELOG.md"
        || path.starts_with("docs/")
}

fn is_campaign_path(path: &str) -> bool {
    path == ".ripr/goals/active.toml"
        || path == "docs/IMPLEMENTATION_CAMPAIGNS.md"
        || path == "docs/IMPLEMENTATION_PLAN.md"
}

fn is_analysis_path(path: &str) -> bool {
    path.starts_with("crates/ripr/src/analysis/")
}

fn is_output_surface_path(path: &str) -> bool {
    path.starts_with("crates/ripr/src/output/")
        || path.starts_with("crates/ripr/src/domain/")
        || path == "crates/ripr/src/lsp.rs"
        || path == "docs/OUTPUT_SCHEMA.md"
        || path == "policy/output_contracts.txt"
}

fn is_metrics_path(path: &str) -> bool {
    path.starts_with("metrics/") || path == "docs/CAPABILITY_MATRIX.md"
}

fn report_index_status(
    reports: &[ReportIndexEntry],
    missing: &[String],
    campaign_issues: &[String],
) -> &'static str {
    if reports.iter().any(|entry| entry.status == "fail") {
        return "fail";
    }
    if !missing.is_empty()
        || !campaign_issues.is_empty()
        || reports.iter().any(|entry| entry.status == "warn")
    {
        "warn"
    } else {
        "pass"
    }
}

fn report_index_next_commands(missing: &[String]) -> Vec<String> {
    let mut commands = BTreeSet::<String>::new();
    if missing
        .iter()
        .any(|path| path.ends_with("/pr-summary.md") || path.ends_with("\\pr-summary.md"))
    {
        commands.insert("cargo xtask pr-summary".to_string());
    }
    if missing
        .iter()
        .any(|path| path.ends_with("/fixtures.md") || path.ends_with("\\fixtures.md"))
    {
        commands.insert("cargo xtask fixtures".to_string());
    }
    if missing
        .iter()
        .any(|path| path.ends_with("/goldens.md") || path.ends_with("\\goldens.md"))
    {
        commands.insert("cargo xtask goldens check".to_string());
    }
    if missing
        .iter()
        .any(|path| path.ends_with("/golden-drift.md") || path.ends_with("\\golden-drift.md"))
    {
        commands.insert("cargo xtask golden-drift".to_string());
    }
    if missing
        .iter()
        .any(|path| path.ends_with("/campaign.md") || path.ends_with("\\campaign.md"))
    {
        commands.insert("cargo xtask check-campaign".to_string());
    }
    if missing
        .iter()
        .any(|path| path.ends_with("/goals-next.md") || path.ends_with("\\goals-next.md"))
    {
        commands.insert("cargo xtask goals next".to_string());
    }
    if missing
        .iter()
        .any(|path| path.ends_with("/metrics.md") || path.ends_with("\\metrics.md"))
    {
        commands.insert("cargo xtask metrics".to_string());
    }
    if missing
        .iter()
        .any(|path| path.ends_with("/capabilities.md") || path.ends_with("\\capabilities.md"))
    {
        commands.insert("cargo xtask check-capabilities".to_string());
    }
    commands.insert("cargo xtask check-pr".to_string());
    commands.insert("cargo xtask reports index".to_string());
    commands.into_iter().collect()
}

fn collect_pr_changes() -> Result<Vec<ChangedPath>, String> {
    let mut changes = BTreeMap::<String, BTreeSet<String>>::new();

    add_name_status_output(
        &mut changes,
        &run_output_optional("git", &["diff", "--name-status", "origin/main...HEAD"])?,
    );
    add_name_status_output(
        &mut changes,
        &run_output("git", &["diff", "--name-status"])?,
    );
    add_name_status_output(
        &mut changes,
        &run_output("git", &["diff", "--cached", "--name-status"])?,
    );
    add_short_status_output(&mut changes, &run_output("git", &["status", "--short"])?);

    Ok(changes
        .into_iter()
        .map(|(path, statuses)| ChangedPath { path, statuses })
        .collect())
}

fn add_name_status_output(changes: &mut BTreeMap<String, BTreeSet<String>>, output: &str) {
    for line in output.lines() {
        let parts = line.split('\t').collect::<Vec<_>>();
        if parts.len() < 2 {
            continue;
        }
        let status = parts[0].trim();
        let Some(path) = parts.last() else {
            continue;
        };
        add_changed_path(changes, path, status);
    }
}

fn add_short_status_output(changes: &mut BTreeMap<String, BTreeSet<String>>, output: &str) {
    for line in output.lines() {
        if line.len() < 4 {
            continue;
        }
        let status = line[..2].trim();
        let mut path = line[3..].trim();
        if let Some((_, new_path)) = path.split_once(" -> ") {
            path = new_path.trim();
        }
        if status.is_empty() {
            continue;
        }
        add_changed_path(changes, path, status);
    }
}

fn add_changed_path(changes: &mut BTreeMap<String, BTreeSet<String>>, path: &str, status: &str) {
    let normalized = normalize_slashes(path.trim().trim_matches('"'));
    if normalized.is_empty() {
        return;
    }
    changes
        .entry(normalized)
        .or_default()
        .insert(status.to_string());
}

fn pr_summary_body(changes: &[ChangedPath]) -> String {
    let mut body = String::from("# ripr PR readiness summary\n\n");
    body.push_str("## Scope\n\n");
    body.push_str("Production delta:\n");
    write_path_list(&mut body, &paths_matching(changes, is_production_path));
    body.push_str("\nEvidence/support delta:\n");
    write_path_list(&mut body, &paths_matching(changes, is_evidence_path));

    body.push_str("\n## Detected Surfaces\n\n");
    for (label, paths) in detected_surface_rows(changes) {
        body.push_str(&format!("{label}:\n"));
        write_path_list(&mut body, &paths);
        body.push('\n');
    }

    body.push_str("## Public Contracts Touched\n\n");
    for (label, paths) in public_contract_rows(changes) {
        body.push_str(&format!("{label}:\n"));
        write_path_list(&mut body, &paths);
        body.push('\n');
    }

    body.push_str("## Policy Exceptions\n\n");
    for (label, paths) in policy_exception_rows(changes) {
        body.push_str(&format!("{label}:\n"));
        write_path_list(&mut body, &paths);
        body.push('\n');
    }

    body.push_str("## Suggested Reviewer Focus\n\n");
    let focus = reviewer_focus(changes);
    if focus.is_empty() {
        body.push_str("- No changed files detected.\n");
    } else {
        for (index, path) in focus.iter().enumerate() {
            body.push_str(&format!("{}. `{path}`\n", index + 1));
        }
    }

    body.push_str("\n## Commands\n\n");
    body.push_str("- `cargo xtask fix-pr`\n");
    body.push_str("- `cargo xtask check-pr`\n");
    body.push_str("- `cargo xtask pr-summary`\n");

    body.push_str("\n## Receipts\n\n");
    for spec in receipt_specs() {
        let path = receipts_dir().join(spec.file);
        let status = if path.exists() {
            report_entry_status(&path)
        } else {
            "missing".to_string()
        };
        body.push_str(&format!("- `{}`: {status}\n", normalize_path(&path)));
    }
    body
}

fn report_index_markdown(
    status: &str,
    campaign: &ReportIndexCampaign,
    reports: &[ReportIndexEntry],
    receipts: &[ReportIndexEntry],
    missing: &[String],
    next_commands: &[String],
) -> String {
    let mut body = format!("# ripr report index\n\nStatus: {status}\n\n");
    body.push_str("This is the reviewer front door for generated `ripr` artifacts.\n\n");

    body.push_str("## Campaign\n\n");
    body.push_str(&format!("- id: `{}`\n", campaign.id));
    body.push_str(&format!("- title: {}\n", campaign.title));
    body.push_str(&format!("- status: `{}`\n", campaign.status));
    body.push_str("- ready work items:\n");
    if campaign.ready_work_items.is_empty() {
        body.push_str("  - None detected.\n");
    } else {
        for item in &campaign.ready_work_items {
            body.push_str(&format!("  - `{item}`\n"));
        }
    }
    if !campaign.issues.is_empty() {
        body.push_str("- campaign issues:\n");
        for issue in &campaign.issues {
            body.push_str(&format!("  - {issue}\n"));
        }
    }

    body.push_str("\n## Summary\n\n");
    body.push_str(&format!("- available reports: {}\n", reports.len()));
    body.push_str(&format!("- available receipts: {}\n", receipts.len()));
    body.push_str(&format!("- missing expected reports: {}\n", missing.len()));
    body.push_str(&format!(
        "- failed reports: {}\n",
        reports
            .iter()
            .filter(|entry| entry.status == "fail")
            .count()
    ));
    body.push_str(&format!(
        "- warning reports: {}\n",
        reports
            .iter()
            .filter(|entry| entry.status == "warn")
            .count()
    ));

    body.push_str("\n## Suggested Reviewer Path\n\n");
    body.push_str("1. Read `target/ripr/reports/pr-summary.md`.\n");
    body.push_str("2. Read `target/ripr/reports/critic.md`, if present.\n");
    body.push_str("3. Inspect `target/ripr/reports/fixtures.md` and `target/ripr/reports/goldens.md` when fixtures or output changed.\n");
    body.push_str(
        "4. Inspect `target/ripr/reports/golden-drift.md`, if present and output changed.\n",
    );
    body.push_str("5. Inspect `target/ripr/receipts/check-pr.json`, when receipts exist.\n");

    body.push_str("\n## Key Report Status\n\n");
    for file in [
        "pr-summary.md",
        "check-pr.md",
        "pr-shape.md",
        "fixtures.md",
        "goldens.md",
        "golden-drift.md",
        "allow-attributes.md",
        "local-context.md",
        "test-oracles.md",
        "dogfood.md",
        "metrics.md",
        "campaign.md",
        "goals-next.md",
    ] {
        body.push_str(&format!(
            "- `{file}`: {}\n",
            status_for_report(reports, file)
        ));
    }

    body.push_str("\n## Available Reports\n\n");
    if reports.is_empty() {
        body.push_str("- None detected.\n");
    } else {
        body.push_str("| Report | Status |\n| --- | --- |\n");
        for entry in reports {
            body.push_str(&format!(
                "| `{}` | `{}` |\n",
                markdown_cell(&entry.path),
                markdown_cell(&entry.status)
            ));
        }
    }

    body.push_str("\n## Missing Expected Reports\n\n");
    write_path_list(&mut body, missing);

    body.push_str("\n## Receipts\n\n");
    if receipts.is_empty() {
        body.push_str("- None detected.\n");
    } else {
        for receipt in receipts {
            body.push_str(&format!("- `{}`\n", receipt.path));
        }
    }

    body.push_str("\n## Suggested Next Commands\n\n");
    for command in next_commands {
        body.push_str(&format!("- `{command}`\n"));
    }

    body
}

fn report_index_json(
    status: &str,
    campaign: &ReportIndexCampaign,
    reports: &[ReportIndexEntry],
    receipts: &[ReportIndexEntry],
    missing: &[String],
    next_commands: &[String],
) -> String {
    let mut body = String::from("{\n");
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str(&format!("  \"status\": \"{}\",\n", json_escape(status)));
    body.push_str("  \"campaign\": {\n");
    body.push_str(&format!("    \"id\": \"{}\",\n", json_escape(&campaign.id)));
    body.push_str(&format!(
        "    \"title\": \"{}\",\n",
        json_escape(&campaign.title)
    ));
    body.push_str(&format!(
        "    \"status\": \"{}\",\n",
        json_escape(&campaign.status)
    ));
    body.push_str("    \"ready_work_items\": [");
    write_json_string_array(&mut body, &campaign.ready_work_items);
    body.push_str("],\n");
    body.push_str("    \"issues\": [");
    write_json_string_array(&mut body, &campaign.issues);
    body.push_str("]\n");
    body.push_str("  },\n");
    body.push_str("  \"reports\": [\n");
    write_report_index_entry_array(&mut body, reports);
    body.push_str("  ],\n");
    body.push_str("  \"receipts\": [\n");
    write_report_index_entry_array(&mut body, receipts);
    body.push_str("  ],\n");
    body.push_str("  \"missing_expected_reports\": [");
    write_json_string_array(&mut body, missing);
    body.push_str("],\n");
    body.push_str("  \"suggested_next_commands\": [");
    write_json_string_array(&mut body, next_commands);
    body.push_str("]\n");
    body.push_str("}\n");
    body
}

fn write_report_index_entry_array(body: &mut String, entries: &[ReportIndexEntry]) {
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"file\": \"{}\",\n",
            json_escape(&entry.file)
        ));
        body.push_str(&format!(
            "      \"path\": \"{}\",\n",
            json_escape(&entry.path)
        ));
        body.push_str(&format!(
            "      \"status\": \"{}\"\n",
            json_escape(&entry.status)
        ));
        body.push_str("    }");
    }
    if !entries.is_empty() {
        body.push('\n');
    }
}

fn status_for_report(reports: &[ReportIndexEntry], file: &str) -> String {
    reports
        .iter()
        .find(|entry| entry.file == file)
        .map(|entry| entry.status.clone())
        .unwrap_or_else(|| "missing".to_string())
}

fn pr_shape_warnings(changes: &[ChangedPath]) -> Vec<String> {
    let mut warnings = Vec::new();
    let has_production = changes
        .iter()
        .any(|change| is_production_path(&change.path));
    let has_evidence = changes
        .iter()
        .any(|change| is_evidence_path(&change.path) && !is_production_path(&change.path));
    if has_production && !has_evidence {
        warnings.push(
            "Production code changed without an evidence/support file. Add or update a spec, test, fixture, golden, metric, or doc, or explain why this is a pure refactor."
                .to_string(),
        );
    }

    let analysis_changed = changes
        .iter()
        .any(|change| change.path.starts_with("crates/ripr/src/analysis/"));
    let analysis_evidence = changes.iter().any(|change| {
        is_spec_path(&change.path)
            || is_test_path(&change.path)
            || is_fixture_path(&change.path)
            || change.path.starts_with("metrics/")
            || change.path == ".ripr/traceability.toml"
    });
    if analysis_changed && !analysis_evidence {
        warnings.push(
            "Analysis code changed without spec/test/fixture/metric/traceability evidence. Analyzer PRs should carry behavior evidence unless this is a narrow mechanical refactor."
                .to_string(),
        );
    }

    let output_changed = changes.iter().any(|change| {
        change.path.starts_with("crates/ripr/src/output/")
            || change.path.starts_with("crates/ripr/src/domain/")
            || change.path == "crates/ripr/src/lsp.rs"
    });
    let output_evidence = changes.iter().any(|change| {
        change.path == "docs/OUTPUT_SCHEMA.md"
            || change.path == "policy/output_contracts.txt"
            || is_golden_path(&change.path)
            || is_fixture_path(&change.path)
    });
    if output_changed && !output_evidence {
        warnings.push(
            "Output-facing code changed without output schema, contract registry, fixture, or golden evidence. Add the matching output evidence or explain why output is unchanged."
                .to_string(),
        );
    }

    let policy_changed = changes.iter().any(|change| is_policy_path(&change.path));
    let policy_docs_changed = changes.iter().any(|change| {
        matches!(
            change.path.as_str(),
            "AGENTS.md" | "CONTRIBUTING.md" | "README.md" | "docs/CI.md" | "docs/PR_AUTOMATION.md"
        )
    });
    if policy_changed && !policy_docs_changed {
        warnings.push(
            "Policy metadata changed without front-door process docs. Update AGENTS, CONTRIBUTING, README, docs/CI.md, or docs/PR_AUTOMATION.md when policy behavior changes."
                .to_string(),
        );
    }

    let xtask_changed = changes
        .iter()
        .any(|change| change.path.starts_with("xtask/"));
    let xtask_docs_changed = changes.iter().any(|change| {
        matches!(
            change.path.as_str(),
            "AGENTS.md"
                | "CONTRIBUTING.md"
                | "README.md"
                | "docs/CI.md"
                | "docs/PR_AUTOMATION.md"
                | "docs/TESTING.md"
        )
    });
    if xtask_changed && !xtask_docs_changed {
        warnings.push(
            "xtask behavior changed without command/process docs. Update the relevant front-door docs or explain why the command surface is unchanged."
                .to_string(),
        );
    }

    warnings
}

fn pr_shape_report_body(warnings: &[String]) -> String {
    let status = if warnings.is_empty() { "pass" } else { "warn" };
    let mut body = format!("# ripr PR shape report\n\nStatus: {status}\n\n");
    body.push_str(
        "This report is advisory. It highlights likely missing evidence before review.\n\n",
    );
    body.push_str("## Warnings\n\n");
    if warnings.is_empty() {
        body.push_str("None detected.\n");
    } else {
        for warning in warnings {
            body.push_str("```text\n");
            body.push_str(warning);
            body.push_str("\n```\n\n");
        }
    }
    body
}

fn critic_findings(
    changes: &[ChangedPath],
    reports: &[ReportIndexEntry],
    receipts: &[ReportIndexEntry],
) -> Vec<CriticFinding> {
    let mut findings = Vec::new();

    let analysis_changed = changes.iter().any(|change| is_analysis_path(&change.path));
    let analysis_evidence = changes.iter().any(|change| {
        is_spec_path(&change.path)
            || is_test_path(&change.path)
            || is_fixture_path(&change.path)
            || is_golden_path(&change.path)
            || change.path.starts_with("metrics/")
            || change.path == ".ripr/traceability.toml"
    });
    if analysis_changed && !analysis_evidence {
        findings.push(CriticFinding {
            id: "analysis_without_behavior_evidence",
            severity: "warn",
            message: "Analyzer code changed without spec, test, fixture, golden, metric, or traceability evidence.",
            evidence: paths_matching(changes, is_analysis_path),
            recommended_action:
                "Add focused behavior evidence or document why this is a mechanical refactor.",
        });
    }
    if analysis_changed && missing_or_bad_report(reports, "fixtures.md") {
        findings.push(CriticFinding {
            id: "analysis_missing_fixture_report",
            severity: "warn",
            message: "Analyzer code changed without a passing fixture report in target/ripr/reports.",
            evidence: vec![format_report_status(reports, "fixtures.md")],
            recommended_action: "Run `cargo xtask fixtures` before review.",
        });
    }
    if analysis_changed && missing_or_bad_report(reports, "goldens.md") {
        findings.push(CriticFinding {
            id: "analysis_missing_golden_report",
            severity: "warn",
            message: "Analyzer code changed without a passing golden report in target/ripr/reports.",
            evidence: vec![format_report_status(reports, "goldens.md")],
            recommended_action: "Run `cargo xtask goldens check` before review.",
        });
    }

    let output_changed = changes
        .iter()
        .any(|change| is_output_surface_path(&change.path));
    let output_evidence = changes.iter().any(|change| {
        change.path == "docs/OUTPUT_SCHEMA.md"
            || change.path == "policy/output_contracts.txt"
            || is_fixture_path(&change.path)
            || is_golden_path(&change.path)
    });
    if output_changed && !output_evidence {
        findings.push(CriticFinding {
            id: "output_without_contract_or_golden_evidence",
            severity: "warn",
            message: "Output-facing code changed without output schema, contract, fixture, or golden evidence.",
            evidence: paths_matching(changes, is_output_surface_path),
            recommended_action:
                "Add output-contract and fixture/golden evidence, or document why rendered output is unchanged.",
        });
    }
    if output_changed && missing_or_bad_report(reports, "output-contracts.md") {
        findings.push(CriticFinding {
            id: "output_missing_contract_report",
            severity: "warn",
            message: "Output-facing code changed without a passing output-contract report.",
            evidence: vec![format_report_status(reports, "output-contracts.md")],
            recommended_action: "Run `cargo xtask check-output-contracts` before review.",
        });
    }
    if output_changed && missing_or_bad_report(reports, "golden-drift.md") {
        findings.push(CriticFinding {
            id: "output_missing_golden_drift_report",
            severity: "warn",
            message: "Output-facing code changed without a semantic golden-drift report.",
            evidence: vec![format_report_status(reports, "golden-drift.md")],
            recommended_action: "Run `cargo xtask golden-drift` before review.",
        });
    }
    if output_changed
        && !changes
            .iter()
            .any(|change| change.path == "policy/output_contracts.txt")
    {
        findings.push(CriticFinding {
            id: "public_output_terms_without_registry_update",
            severity: "warn",
            message: "Public output surface changed without an output contract registry update.",
            evidence: paths_matching(changes, is_output_surface_path),
            recommended_action:
                "Confirm no public output terms changed, or update `policy/output_contracts.txt`.",
        });
    }

    let capability_docs_changed = changes
        .iter()
        .any(|change| change.path == "docs/CAPABILITY_MATRIX.md");
    let capability_metrics_changed = changes
        .iter()
        .any(|change| change.path == "metrics/capabilities.toml");
    if capability_docs_changed && !capability_metrics_changed {
        findings.push(CriticFinding {
            id: "capability_docs_without_metrics",
            severity: "warn",
            message: "Capability matrix changed without machine-readable capability metrics.",
            evidence: paths_matching(changes, |path| path == "docs/CAPABILITY_MATRIX.md"),
            recommended_action:
                "Update `metrics/capabilities.toml` or document why the change is prose-only.",
        });
    }
    if (capability_docs_changed || capability_metrics_changed)
        && missing_or_bad_report(reports, "capabilities.md")
    {
        findings.push(CriticFinding {
            id: "capability_missing_report",
            severity: "warn",
            message: "Capability state changed without a passing capability report.",
            evidence: vec![format_report_status(reports, "capabilities.md")],
            recommended_action: "Run `cargo xtask check-capabilities` before review.",
        });
    }

    let missing_blessings = golden_changes_without_blessing(changes);
    if !missing_blessings.is_empty() {
        findings.push(CriticFinding {
            id: "golden_changed_without_blessing_reason",
            severity: "warn",
            message: "Fixture expected output changed without a matching blessing reason changelog.",
            evidence: missing_blessings,
            recommended_action:
                "Record the intentional output change in the fixture expected-output changelog.",
        });
    }

    let campaign_changed = changes.iter().any(|change| is_campaign_path(&change.path));
    if campaign_changed && missing_or_bad_report(reports, "campaign.md") {
        findings.push(CriticFinding {
            id: "campaign_missing_check_report",
            severity: "warn",
            message: "Campaign state changed without a passing campaign report.",
            evidence: vec![format_report_status(reports, "campaign.md")],
            recommended_action: "Run `cargo xtask check-campaign` before review.",
        });
    }
    if campaign_changed && missing_or_bad_report(reports, "goals-next.md") {
        findings.push(CriticFinding {
            id: "campaign_missing_goals_next_report",
            severity: "warn",
            message: "Campaign state changed without a goals-next report.",
            evidence: vec![format_report_status(reports, "goals-next.md")],
            recommended_action: "Run `cargo xtask goals next` before review.",
        });
    }

    let policy_changed = changes.iter().any(|change| is_policy_path(&change.path));
    let process_docs_changed = changes.iter().any(|change| {
        matches!(
            change.path.as_str(),
            "AGENTS.md" | "CONTRIBUTING.md" | "README.md" | "docs/CI.md" | "docs/PR_AUTOMATION.md"
        )
    });
    if policy_changed && !process_docs_changed {
        findings.push(CriticFinding {
            id: "policy_without_process_docs",
            severity: "warn",
            message: "Policy files or workflows changed without process documentation.",
            evidence: paths_matching(changes, is_policy_path),
            recommended_action:
                "Update front-door process docs or document why behavior did not change.",
        });
    }

    let extension_changed = changes
        .iter()
        .any(|change| change.path.starts_with("editors/vscode/"));
    if extension_changed {
        findings.push(CriticFinding {
            id: "extension_requires_package_evidence",
            severity: "warn",
            message: "VS Code extension files changed; local xtask reports do not prove npm compile/package evidence.",
            evidence: paths_matching(changes, |path| path.starts_with("editors/vscode/")),
            recommended_action: "Verify `npm run compile` and `npm run package`, or inspect the CI vscode job.",
        });
    }

    if missing_or_bad_report(reports, "pr-summary.md") {
        findings.push(CriticFinding {
            id: "missing_pr_summary",
            severity: "warn",
            message: "The reviewer packet is missing a PR summary report.",
            evidence: vec![format_report_status(reports, "pr-summary.md")],
            recommended_action: "Run `cargo xtask pr-summary` before review.",
        });
    }
    if missing_or_bad_report(reports, "pr-shape.md") {
        findings.push(CriticFinding {
            id: "missing_pr_shape_report",
            severity: "warn",
            message: "The advisory PR shape report is missing or not passing.",
            evidence: vec![format_report_status(reports, "pr-shape.md")],
            recommended_action: "Run `cargo xtask check-pr-shape` before review.",
        });
    }
    if receipts.is_empty() {
        findings.push(CriticFinding {
            id: "missing_receipts",
            severity: "warn",
            message: "No machine-readable receipts were found for this reviewer packet.",
            evidence: vec!["target/ripr/receipts: missing or empty".to_string()],
            recommended_action: "Run `cargo xtask receipts` and `cargo xtask receipts check`.",
        });
    }

    findings
}

fn missing_or_bad_report(reports: &[ReportIndexEntry], file: &str) -> bool {
    !matches!(
        status_for_report(reports, file).as_str(),
        "pass" | "present"
    )
}

fn format_report_status(reports: &[ReportIndexEntry], file: &str) -> String {
    format!(
        "target/ripr/reports/{file}: {}",
        status_for_report(reports, file)
    )
}

fn golden_changes_without_blessing(changes: &[ChangedPath]) -> Vec<String> {
    let changed_paths = changes
        .iter()
        .map(|change| change.path.as_str())
        .collect::<BTreeSet<_>>();
    let mut missing = Vec::new();
    for change in changes.iter().filter(|change| is_golden_path(&change.path)) {
        let Some(fixture) = fixture_name_from_expected_output(&change.path) else {
            continue;
        };
        let changelog = format!("fixtures/{fixture}/expected/CHANGELOG.md");
        if !changed_paths.contains(changelog.as_str()) {
            missing.push(format!(
                "{} changed without `{changelog}`",
                format_changed_path(change)
            ));
        }
    }
    missing.sort();
    missing.dedup();
    missing
}

fn fixture_name_from_expected_output(path: &str) -> Option<String> {
    let rest = path.strip_prefix("fixtures/")?;
    let (fixture, after_fixture) = rest.split_once('/')?;
    if after_fixture.starts_with("expected/") && after_fixture != "expected/CHANGELOG.md" {
        Some(fixture.to_string())
    } else {
        None
    }
}

fn critic_markdown(
    findings: &[CriticFinding],
    reports: &[ReportIndexEntry],
    receipts: &[ReportIndexEntry],
) -> String {
    let status = if findings.is_empty() { "pass" } else { "warn" };
    let mut body = format!("# ripr critic report\n\nStatus: {status}\n\n");
    body.push_str("Mode: advisory\n\n");
    body.push_str("This report is a deterministic adversarial review packet. It flags likely missing evidence from the current diff, reports, and receipts. It does not block CI.\n\n");

    body.push_str("## Findings\n\n");
    if findings.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for finding in findings {
            body.push_str(&format!(
                "### {} ({})\n\n{}\n\n",
                finding.id, finding.severity, finding.message
            ));
            body.push_str("Evidence:\n\n");
            write_path_list(&mut body, &finding.evidence);
            body.push_str("\nRecommended action:\n\n```text\n");
            body.push_str(finding.recommended_action);
            body.push_str("\n```\n\n");
        }
    }

    body.push_str("## Inputs\n\n");
    body.push_str(&format!("- reports available: {}\n", reports.len()));
    body.push_str(&format!("- receipts available: {}\n\n", receipts.len()));
    body.push_str("## Next Commands\n\n");
    body.push_str("```bash\n");
    body.push_str("cargo xtask pr-summary\n");
    body.push_str("cargo xtask reports index\n");
    body.push_str("cargo xtask receipts\n");
    body.push_str("cargo xtask receipts check\n");
    body.push_str("```\n");
    body
}

fn critic_json(
    findings: &[CriticFinding],
    reports: &[ReportIndexEntry],
    receipts: &[ReportIndexEntry],
) -> String {
    let status = if findings.is_empty() { "pass" } else { "warn" };
    let mut body = String::new();
    body.push_str("{\n");
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str("  \"mode\": \"advisory\",\n");
    body.push_str(&format!("  \"status\": \"{status}\",\n"));
    body.push_str(&format!("  \"reports_available\": {},\n", reports.len()));
    body.push_str(&format!("  \"receipts_available\": {},\n", receipts.len()));
    body.push_str("  \"findings\": [\n");
    for (index, finding) in findings.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!("      \"id\": \"{}\",\n", json_escape(finding.id)));
        body.push_str(&format!(
            "      \"severity\": \"{}\",\n",
            json_escape(finding.severity)
        ));
        body.push_str(&format!(
            "      \"message\": \"{}\",\n",
            json_escape(finding.message)
        ));
        body.push_str("      \"evidence\": [");
        write_json_string_array(&mut body, &finding.evidence);
        body.push_str("],\n");
        body.push_str(&format!(
            "      \"recommended_action\": \"{}\"\n",
            json_escape(finding.recommended_action)
        ));
        body.push_str("    }");
    }
    if !findings.is_empty() {
        body.push('\n');
    }
    body.push_str("  ]\n");
    body.push_str("}\n");
    body
}

fn write_path_list(body: &mut String, paths: &[String]) {
    if paths.is_empty() {
        body.push_str("- None detected.\n");
        return;
    }
    for path in paths {
        body.push_str(&format!("- `{path}`\n"));
    }
}

fn paths_matching(changes: &[ChangedPath], predicate: fn(&str) -> bool) -> Vec<String> {
    changes
        .iter()
        .filter(|change| predicate(&change.path))
        .map(format_changed_path)
        .collect()
}

fn detected_surface_rows(changes: &[ChangedPath]) -> Vec<(&'static str, Vec<String>)> {
    vec![
        (
            "Rust product code",
            paths_matching(changes, |path| path.starts_with("crates/ripr/src/")),
        ),
        (
            "Rust tests",
            paths_matching(changes, |path| path.starts_with("crates/ripr/tests/")),
        ),
        (
            "Automation/tooling",
            paths_matching(changes, |path| path.starts_with("xtask/")),
        ),
        (
            "Fixtures",
            paths_matching(changes, |path| path.starts_with("fixtures/")),
        ),
        (
            "Goldens",
            paths_matching(changes, |path| {
                path.contains("/expected/") || path.contains("/golden")
            }),
        ),
        (
            "Docs",
            paths_matching(changes, |path| {
                path.starts_with("docs/")
                    || matches!(
                        path,
                        "README.md" | "AGENTS.md" | "CONTRIBUTING.md" | "CHANGELOG.md"
                    )
            }),
        ),
        (
            "Policies",
            paths_matching(changes, |path| {
                path.starts_with("policy/") || path.starts_with(".ripr/")
            }),
        ),
        (
            "Workflows",
            paths_matching(changes, |path| path.starts_with(".github/")),
        ),
        (
            "Extension",
            paths_matching(changes, |path| path.starts_with("editors/vscode/")),
        ),
    ]
}

fn public_contract_rows(changes: &[ChangedPath]) -> Vec<(&'static str, Vec<String>)> {
    vec![
        (
            "CLI",
            paths_matching(changes, |path| {
                path.starts_with("crates/ripr/src/cli/")
                    || path == "crates/ripr/src/main.rs"
                    || path.starts_with("docs/reference/cli")
            }),
        ),
        (
            "JSON",
            paths_matching(changes, |path| {
                path == "crates/ripr/src/output/json.rs"
                    || path.starts_with("crates/ripr/src/output/json/")
                    || path == "docs/OUTPUT_SCHEMA.md"
            }),
        ),
        (
            "Human output",
            paths_matching(changes, |path| path == "crates/ripr/src/output/human.rs"),
        ),
        (
            "LSP",
            paths_matching(changes, |path| {
                path == "crates/ripr/src/lsp.rs" || path.starts_with("editors/vscode/")
            }),
        ),
        (
            "GitHub/SARIF",
            paths_matching(changes, |path| {
                path == "crates/ripr/src/output/github.rs"
                    || path.to_ascii_lowercase().contains("sarif")
            }),
        ),
        (
            "Config",
            paths_matching(changes, |path| {
                path == "ripr.toml.example" || path.contains("config") || path.contains("ripr-toml")
            }),
        ),
        (
            "Docs",
            paths_matching(changes, |path| {
                path.starts_with("docs/")
                    || matches!(
                        path,
                        "README.md" | "AGENTS.md" | "CONTRIBUTING.md" | "CHANGELOG.md"
                    )
            }),
        ),
    ]
}

fn policy_exception_rows(changes: &[ChangedPath]) -> Vec<(&'static str, Vec<String>)> {
    vec![
        (
            "Non-Rust files",
            paths_matching(changes, |path| {
                is_file_policy_candidate(path) && !path.ends_with(".rs")
            }),
        ),
        (
            "Executable files",
            paths_matching(changes, |path| path == "policy/executable_allowlist.txt"),
        ),
        (
            "Panic-family allowlist",
            paths_matching(changes, |path| path == ".ripr/no-panic-allowlist.txt"),
        ),
        (
            "Static-language allowlist",
            paths_matching(changes, |path| {
                path == STATIC_LANGUAGE_ALLOWLIST_PATH
                    || path == STATIC_LANGUAGE_ALLOWLIST_LEGACY_PATH
            }),
        ),
        (
            "Workflow budget",
            paths_matching(changes, |path| path == "policy/workflow_allowlist.txt"),
        ),
        (
            "Dependencies",
            paths_matching(changes, |path| {
                path == "policy/dependency_allowlist.txt" || is_dependency_surface_candidate(path)
            }),
        ),
    ]
}

fn reviewer_focus(changes: &[ChangedPath]) -> Vec<String> {
    let mut focus = Vec::new();
    for predicate in [
        is_production_path as fn(&str) -> bool,
        is_test_path,
        is_spec_path,
        is_fixture_path,
        is_golden_path,
        is_automation_path,
        is_policy_path,
    ] {
        for path in paths_matching(changes, predicate) {
            let raw_path = strip_status_suffix(&path).to_string();
            if !focus.contains(&raw_path) {
                focus.push(raw_path);
            }
            if focus.len() >= 8 {
                return focus;
            }
        }
    }
    focus
}

fn is_production_path(path: &str) -> bool {
    path.starts_with("crates/ripr/src/") || path.starts_with("editors/vscode/src/")
}

fn is_evidence_path(path: &str) -> bool {
    is_test_path(path)
        || is_spec_path(path)
        || is_fixture_path(path)
        || is_golden_path(path)
        || is_automation_path(path)
        || is_policy_path(path)
        || path.starts_with("docs/")
        || path.starts_with("metrics/")
        || matches!(
            path,
            "README.md" | "AGENTS.md" | "CONTRIBUTING.md" | "CHANGELOG.md"
        )
}

fn is_test_path(path: &str) -> bool {
    path.starts_with("crates/ripr/tests/") || path.contains("/tests/")
}

fn is_spec_path(path: &str) -> bool {
    path.starts_with("docs/specs/") || path == "docs/SPEC_FORMAT.md"
}

fn is_fixture_path(path: &str) -> bool {
    path.starts_with("fixtures/")
}

fn is_golden_path(path: &str) -> bool {
    path.contains("/expected/") || path.contains("/golden")
}

fn is_automation_path(path: &str) -> bool {
    path.starts_with("xtask/")
}

fn is_policy_path(path: &str) -> bool {
    path.starts_with("policy/") || path.starts_with(".ripr/") || path.starts_with(".github/")
}

fn format_changed_path(change: &ChangedPath) -> String {
    let status = change
        .statuses
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    if status.is_empty() {
        change.path.clone()
    } else {
        format!("{} ({status})", change.path)
    }
}

fn strip_status_suffix(path: &str) -> &str {
    match path.rsplit_once(" (") {
        Some((raw_path, _)) => raw_path,
        None => path,
    }
}

fn read_path_allowlist(path: &str) -> Result<BTreeSet<String>, String> {
    let mut allowed = BTreeSet::new();
    let text = read_text_lossy(Path::new(path))?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        allowed.insert(normalize_slashes(trimmed));
    }
    Ok(allowed)
}

fn read_count_allowlist(path: &str) -> Result<BTreeMap<(String, String), usize>, String> {
    let mut allowed = BTreeMap::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(format!(
                "{path}:{} expected path|pattern|max_count|reason",
                line_number + 1
            ));
        }
        let max_count = parts[2]
            .parse::<usize>()
            .map_err(|err| format!("{path}:{} invalid max_count: {err}", line_number + 1))?;
        allowed.insert(
            (normalize_slashes(parts[0]), parts[1].to_string()),
            max_count,
        );
    }
    Ok(allowed)
}

fn read_count_policy_allowlist(path: &str) -> Result<BTreeMap<(String, String), usize>, String> {
    let mut allowed = BTreeMap::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 5 {
            return Err(format!(
                "{path}:{} expected path|pattern|max_count|owner|reason",
                line_number + 1
            ));
        }
        if parts[0].trim().is_empty()
            || parts[1].trim().is_empty()
            || parts[3].trim().is_empty()
            || parts[4].trim().is_empty()
        {
            return Err(format!(
                "{path}:{} allowlist entries require path, pattern, owner, and reason",
                line_number + 1
            ));
        }
        let max_count = parts[2]
            .parse::<usize>()
            .map_err(|err| format!("{path}:{} invalid max_count: {err}", line_number + 1))?;
        allowed.insert(
            (normalize_slashes(parts[0]), parts[1].to_string()),
            max_count,
        );
    }
    Ok(allowed)
}

fn read_local_context_allowlist(path: &str) -> Result<Vec<LocalContextAllow>, String> {
    let mut allowed = Vec::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(format!(
                "{path}:{} expected path|pattern|max_count|reason",
                line_number + 1
            ));
        }
        if parts[0].trim().is_empty() || parts[1].trim().is_empty() || parts[3].trim().is_empty() {
            return Err(format!(
                "{path}:{} allowlist entries require path, pattern, and reason",
                line_number + 1
            ));
        }
        let max_count = parts[2]
            .parse::<usize>()
            .map_err(|err| format!("{path}:{} invalid max_count: {err}", line_number + 1))?;
        allowed.push(LocalContextAllow {
            path: normalize_slashes(parts[0].trim()),
            pattern: parts[1].trim().to_string(),
            max_count,
            line: line_number + 1,
        });
    }
    Ok(allowed)
}

fn validate_local_context_allowlist(allowlist: &[LocalContextAllow]) -> Vec<String> {
    let mut violations = Vec::new();
    for entry in allowlist {
        if !is_local_context_candidate(&entry.path) {
            violations.push(format!(
                "Path: policy/local_context_allowlist.txt\nProblem: local context allowlist entry targets a file type that is not scanned\nPattern: {}\nCount: 1, allowed: 0\nLines: {}\nWhy this matters: Local context exceptions should stay narrow and reviewable.\nRecommended fixes:\n1. Remove the stale exception.\n2. If the file should be scanned, add its extension to the checker intentionally.",
                entry.pattern, entry.line
            ));
        }
        if forbidden_local_context_allowlist_pattern(&entry.pattern) {
            violations.push(format!(
                "Path: policy/local_context_allowlist.txt\nProblem: local context allowlist tries to permit real machine or session state\nPattern: {}\nCount: 1, allowed: 0\nLines: {}\nWhy this matters: Real machine paths, Codex memory paths, and sandbox paths must be removed, not allowlisted.\nRecommended fixes:\n1. Delete the local context from the committed file.\n2. Keep only generic examples in durable docs.",
                entry.pattern, entry.line
            ));
        }
    }
    violations
}

fn forbidden_local_context_allowlist_pattern(pattern: &str) -> bool {
    let lower = pattern.to_ascii_lowercase();
    if lower.contains(concat!(".", "codex"))
        || lower.contains(concat!("memory", ".md"))
        || lower.contains(concat!("sandbox:", "/mnt", "/data"))
        || lower.contains(concat!("/mnt", "/data"))
        || lower.contains(concat!("contentreference", "[oaicite"))
    {
        return true;
    }
    for token in windows_absolute_path_tokens(pattern) {
        let generic_example = token
            .to_ascii_lowercase()
            .replace('/', "\\")
            .contains(concat!(":\\", "path", "\\to\\"));
        if !generic_example {
            return true;
        }
    }
    !unix_home_path_tokens(pattern).is_empty()
}

fn read_glob_allowlist(path: &str) -> Result<Vec<GlobAllow>, String> {
    let mut allowed = Vec::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(format!(
                "{path}:{} expected glob|kind|owner|reason",
                line_number + 1
            ));
        }
        let entry = GlobAllow {
            glob: normalize_slashes(parts[0]),
        };
        if entry.glob.is_empty()
            || parts[1].trim().is_empty()
            || parts[2].trim().is_empty()
            || parts[3].trim().is_empty()
        {
            return Err(format!(
                "{path}:{} allowlist entries require glob, kind, owner, and reason",
                line_number + 1
            ));
        }
        allowed.push(entry);
    }
    Ok(allowed)
}

fn read_workflow_budgets(path: &str) -> Result<BTreeMap<String, WorkflowBudget>, String> {
    let mut budgets = BTreeMap::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 3 {
            return Err(format!(
                "{path}:{} expected path|max_non_empty_lines|reason",
                line_number + 1
            ));
        }
        let max_non_empty_lines = parts[1].parse::<usize>().map_err(|err| {
            format!(
                "{path}:{} invalid max_non_empty_lines: {err}",
                line_number + 1
            )
        })?;
        let budget = WorkflowBudget {
            path: normalize_slashes(parts[0]),
            max_non_empty_lines,
            reason: parts[2].trim().to_string(),
        };
        if budget.reason.is_empty() {
            return Err(format!(
                "{path}:{} reason must not be empty",
                line_number + 1
            ));
        }
        budgets.insert(budget.path.clone(), budget);
    }
    Ok(budgets)
}

fn read_path_allowlist_optional(path: &str) -> Result<BTreeSet<String>, String> {
    if Path::new(path).exists() {
        read_path_allowlist(path)
    } else {
        Ok(BTreeSet::new())
    }
}

fn spec_id_from_file_name(file_name: &str) -> Option<String> {
    let mut parts = file_name.split('-');
    let prefix = parts.next()?;
    let kind = parts.next()?;
    let number = parts.next()?;
    if prefix == "RIPR"
        && kind == "SPEC"
        && number.len() == 4
        && number.chars().all(|value| value.is_ascii_digit())
    {
        Some(format!("{prefix}-{kind}-{number}"))
    } else {
        None
    }
}

fn spec_status(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("Status: "))
        .map(|value| value.trim().to_string())
}

fn required_spec_headings() -> Vec<&'static str> {
    vec![
        "## Problem",
        "## Behavior",
        "## Required Evidence",
        "## Non-Goals",
        "## Acceptance Examples",
        "## Test Mapping",
        "## Implementation Mapping",
        "## Metrics",
    ]
}

fn has_markdown_heading(text: &str, heading: &str) -> bool {
    text.lines().any(|line| line.trim_end() == heading)
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_files_inner(root, &mut files)?;
    Ok(files)
}

fn collect_files_inner(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let normalized = normalize_path(path);
    if should_skip_path(&normalized) {
        return Ok(());
    }
    let metadata =
        fs::metadata(path).map_err(|err| format!("failed to inspect {normalized}: {err}"))?;
    if metadata.is_file() {
        files.push(path.to_path_buf());
        return Ok(());
    }
    if metadata.is_dir() {
        for entry in
            fs::read_dir(path).map_err(|err| format!("failed to read {normalized}: {err}"))?
        {
            let entry = entry.map_err(|err| format!("failed to read {normalized}: {err}"))?;
            collect_files_inner(&entry.path(), files)?;
        }
    }
    Ok(())
}

fn tracked_files() -> Result<Vec<String>, String> {
    let output = run_output("git", &["ls-files"])?;
    Ok(output
        .lines()
        .map(normalize_slashes)
        .filter(|path| !path.is_empty())
        .collect())
}

fn should_skip_path(path: &str) -> bool {
    path == ".git"
        || path.starts_with(".git/")
        || path == "target"
        || path.starts_with("target/")
        || path == ".ripr/release"
        || path.starts_with(".ripr/release/")
        || path.ends_with("/.vscode-test")
        || path.contains("/.vscode-test/")
        || path.ends_with("/node_modules")
        || path.contains("/node_modules/")
        || path.ends_with("/out")
        || path.contains("/out/")
        || path.ends_with("/dist")
        || path.contains("/dist/")
}

fn is_static_language_candidate(path: &str) -> bool {
    let extensions = [".md", ".rs", ".txt", ".json", ".toml", ".yml", ".yaml"];
    extensions.iter().any(|extension| path.ends_with(extension))
}

fn read_text_lossy(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn guarded_allow_attribute_lints() -> BTreeSet<&'static str> {
    [
        "clippy::unwrap_used",
        "clippy::expect_used",
        "clippy::panic",
        "clippy::todo",
        "clippy::unimplemented",
        "clippy::dbg_macro",
        "unwrap_used",
        "expect_used",
        "panic",
        "todo",
        "unimplemented",
        "dbg_macro",
        "unsafe_code",
        "dead_code",
        "unused_imports",
        "unused_variables",
        "warnings",
    ]
    .into_iter()
    .collect()
}

fn guarded_allow_attributes_in_text(
    text: &str,
    guarded: &BTreeSet<&'static str>,
) -> Vec<(usize, String)> {
    let bytes = text.as_bytes();
    let mut findings = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'#' {
            index += 1;
            continue;
        }

        let line = byte_line_number(text, index);
        let mut cursor = index + 1;
        if cursor < bytes.len() && bytes[cursor] == b'!' {
            cursor += 1;
        }
        cursor = skip_ascii_whitespace(bytes, cursor);
        if cursor >= bytes.len() || bytes[cursor] != b'[' {
            index += 1;
            continue;
        }
        cursor += 1;
        cursor = skip_ascii_whitespace(bytes, cursor);

        let ident_start = cursor;
        while cursor < bytes.len() && (bytes[cursor].is_ascii_alphabetic() || bytes[cursor] == b'_')
        {
            cursor += 1;
        }
        let kind = &text[ident_start..cursor];
        if kind != "allow" && kind != "expect" {
            index += 1;
            continue;
        }
        cursor = skip_ascii_whitespace(bytes, cursor);
        if cursor >= bytes.len() || bytes[cursor] != b'(' {
            index += 1;
            continue;
        }

        let Some((content_start, content_end, next_index)) = attribute_paren_span(bytes, cursor)
        else {
            index += 1;
            continue;
        };
        for lint in attribute_lints(&text[content_start..content_end]) {
            if guarded.contains(lint.as_str()) {
                findings.push((line, format!("{kind}({lint})")));
            }
        }
        index = next_index;
    }
    findings
}

fn attribute_paren_span(bytes: &[u8], open: usize) -> Option<(usize, usize, usize)> {
    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some((open + 1, index, index + 1));
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn attribute_lints(content: &str) -> Vec<String> {
    content
        .split(',')
        .filter_map(|part| {
            let lint = part.trim();
            if lint.is_empty() || lint.contains('=') {
                None
            } else {
                Some(lint.to_string())
            }
        })
        .collect()
}

fn attribute_lint_name(attribute: &str) -> Option<&str> {
    let (_, rest) = attribute.split_once('(')?;
    Some(rest.strip_suffix(')').unwrap_or(rest).trim())
}

fn skip_ascii_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn byte_line_number(text: &str, byte_index: usize) -> usize {
    text.as_bytes()[..byte_index]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

fn allow_attribute_line_summary(lines: &[usize]) -> String {
    let mut unique = lines.to_vec();
    unique.sort_unstable();
    unique.dedup();
    unique
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn local_context_findings_for_path(path: &str) -> Result<Vec<LocalContextFinding>, String> {
    let mut findings = Vec::new();
    let Some(file_name) = path.rsplit('/').next() else {
        return Ok(findings);
    };

    if suspicious_runtime_file_names()
        .iter()
        .any(|name| file_name.eq_ignore_ascii_case(name))
    {
        findings.push(LocalContextFinding {
            path: path.to_string(),
            line: None,
            pattern: file_name.to_string(),
            problem: "committed runtime/session artifact filename".to_string(),
        });
    }

    if !is_local_context_candidate(path) {
        return Ok(findings);
    }

    let text = read_text_lossy(Path::new(path))?;
    for (line_index, line) in text.lines().enumerate() {
        for (pattern, problem) in local_context_line_findings(line) {
            findings.push(LocalContextFinding {
                path: path.to_string(),
                line: Some(line_index + 1),
                pattern,
                problem,
            });
        }
    }
    Ok(findings)
}

fn local_context_line_findings(line: &str) -> Vec<(String, String)> {
    let mut findings = BTreeSet::<(String, String)>::new();

    for token in windows_absolute_path_tokens(line) {
        findings.insert((token, "local absolute Windows path".to_string()));
    }
    for token in unix_home_path_tokens(line) {
        findings.insert((token, "local absolute Unix home path".to_string()));
    }

    let lower = line.to_ascii_lowercase();
    for (marker, problem) in local_context_markers() {
        if lower.contains(&marker.to_ascii_lowercase()) {
            findings.insert((marker, problem));
        }
    }

    if contains_recorded_date(line) {
        findings.insert((
            recorded_on_pattern().to_string(),
            "session timestamp language".to_string(),
        ));
    }
    if lower.contains(concat!("working tree", " is dirty before")) {
        findings.insert((
            concat!("working tree", " is dirty before").to_string(),
            "transient local worktree state".to_string(),
        ));
    }
    if lower.contains(concat!("before any", " codex edits")) {
        findings.insert((
            concat!("before any", " Codex edits").to_string(),
            "transient Codex session state".to_string(),
        ));
    }
    if lower.contains(concat!("current local", " state")) {
        findings.insert((
            concat!("current local", " state").to_string(),
            "transient local state language".to_string(),
        ));
    }
    if lower.contains(concat!("current", " branch:")) {
        findings.insert((
            concat!("Current", " branch:").to_string(),
            "transient local branch state".to_string(),
        ));
    }

    for token in file_reference_tokens(line) {
        let problem = if token.starts_with("file_") {
            "opaque uploaded file artifact reference"
        } else {
            "chat transcript file reference"
        };
        findings.insert((token, problem.to_string()));
    }

    findings.into_iter().collect()
}

fn local_context_markers() -> Vec<(String, String)> {
    vec![
        (
            concat!(".", "codex").to_string(),
            "Codex local memory path".to_string(),
        ),
        (
            concat!("MEMORY", ".md").to_string(),
            "Codex memory artifact".to_string(),
        ),
        (
            concat!("sandbox:", "/mnt", "/data").to_string(),
            "sandbox runtime path".to_string(),
        ),
        (
            concat!("/mnt", "/data/").to_string(),
            "sandbox runtime path".to_string(),
        ),
        (
            concat!("contentReference", "[oaicite").to_string(),
            "chat citation artifact".to_string(),
        ),
    ]
}

fn suspicious_runtime_file_names() -> Vec<String> {
    vec![
        concat!("CURRENT", "_STATE.md").to_string(),
        concat!("SESSION", "_STATE.md").to_string(),
        "SCRATCHPAD.md".to_string(),
        concat!("NOTES", "_FROM", "_RUN.md").to_string(),
        concat!("CODEX", "_STATE.md").to_string(),
        concat!("codex", "-", "memory", ".md").to_string(),
        "transcript.md".to_string(),
        "chat.md".to_string(),
    ]
}

fn windows_absolute_path_tokens(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index + 2 < bytes.len() {
        let token_boundary = index == 0 || is_local_context_token_delimiter(bytes[index - 1]);
        if token_boundary
            && bytes[index].is_ascii_alphabetic()
            && bytes[index + 1] == b':'
            && (bytes[index + 2] == b'\\' || bytes[index + 2] == b'/')
        {
            let start = index;
            index += 3;
            while index < bytes.len() && !is_local_context_token_delimiter(bytes[index]) {
                index += 1;
            }
            tokens.push(line[start..index].to_string());
        } else {
            index += 1;
        }
    }
    tokens
}

fn unix_home_path_tokens(line: &str) -> Vec<String> {
    ["/Users/", "/home/"]
        .iter()
        .flat_map(|prefix| absolute_path_tokens_with_prefix(line, prefix))
        .collect()
}

fn absolute_path_tokens_with_prefix(line: &str, prefix: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut search_start = 0;
    while let Some(offset) = line[search_start..].find(prefix) {
        let start = search_start + offset;
        let mut end = start + prefix.len();
        let bytes = line.as_bytes();
        let name_start = end;
        while end < line.len()
            && bytes[end] != b'/'
            && !is_local_context_token_delimiter(bytes[end])
        {
            end += 1;
        }
        if end == name_start || end >= line.len() || bytes[end] != b'/' {
            search_start = end.max(start + prefix.len());
            continue;
        }
        end += 1;
        while end < line.len() && !is_local_context_token_delimiter(bytes[end]) {
            end += 1;
        }
        tokens.push(line[start..end].to_string());
        search_start = end;
    }
    tokens
}

fn is_local_context_token_delimiter(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || matches!(
            byte,
            b'`' | b'"' | b'\'' | b')' | b']' | b'}' | b'<' | b'>' | b',' | b';'
        )
}

fn contains_recorded_date(line: &str) -> bool {
    let marker = recorded_on_marker();
    let Some(offset) = line.find(marker) else {
        return false;
    };
    let date = &line[offset + marker.len()..];
    date.len() >= 10
        && date.as_bytes()[0..4].iter().all(u8::is_ascii_digit)
        && date.as_bytes()[4] == b'-'
        && date.as_bytes()[5..7].iter().all(u8::is_ascii_digit)
        && date.as_bytes()[7] == b'-'
        && date.as_bytes()[8..10].iter().all(u8::is_ascii_digit)
}

fn recorded_on_marker() -> &'static str {
    concat!("Recorded", " on ")
}

fn recorded_on_pattern() -> &'static str {
    concat!("Recorded", " on <date>")
}

fn file_reference_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"file_") {
            let start = index;
            index += "file_".len();
            let hex_start = index;
            while index < bytes.len() && bytes[index].is_ascii_hexdigit() {
                index += 1;
            }
            if index - hex_start >= 8 {
                tokens.push(line[start..index].to_string());
            }
            continue;
        }
        if bytes[index..].starts_with(b"turn") {
            let start = index;
            index += "turn".len();
            let digit_start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index > digit_start && bytes[index..].starts_with(b"file") {
                index += "file".len();
                let file_digit_start = index;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                if index > file_digit_start {
                    tokens.push(line[start..index].to_string());
                    continue;
                }
            }
            index = start + 1;
            continue;
        }
        index += 1;
    }
    tokens
}

fn local_context_line_summary(lines: &[Option<usize>]) -> String {
    let mut concrete = lines.iter().flatten().copied().collect::<Vec<_>>();
    concrete.sort_unstable();
    concrete.dedup();
    if concrete.is_empty() {
        "file name".to_string()
    } else {
        concrete
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn is_local_context_candidate(path: &str) -> bool {
    let extensions = [
        ".md", ".rs", ".txt", ".json", ".toml", ".yml", ".yaml", ".ts", ".tsx",
    ];
    extensions.iter().any(|extension| path.ends_with(extension))
}

fn write_local_context_json(violations: &[String]) -> Result<(), String> {
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let mut body = format!(
        "{{\n  \"schema_version\": \"0.1\",\n  \"status\": \"{status}\",\n  \"violation_count\": {},\n  \"violations\": [",
        violations.len()
    );
    for (index, violation) in violations.iter().enumerate() {
        if index > 0 {
            body.push(',');
        }
        body.push_str("\n    \"");
        body.push_str(&json_escape(violation));
        body.push('"');
    }
    if !violations.is_empty() {
        body.push('\n');
    }
    body.push_str("  ]\n}\n");
    write_report("local-context.json", &body)
}

fn normalize_path(path: &Path) -> String {
    normalize_slashes(&path.to_string_lossy())
        .trim_start_matches("./")
        .to_string()
}

fn normalize_slashes(value: &str) -> String {
    value.replace('\\', "/")
}

fn is_file_policy_candidate(path: &str) -> bool {
    let extensions = [
        ".bash", ".c", ".cjs", ".cpp", ".cs", ".go", ".h", ".hpp", ".java", ".js", ".json", ".kt",
        ".lua", ".mjs", ".php", ".pl", ".ps1", ".py", ".rb", ".sh", ".swift", ".toml", ".ts",
        ".tsx", ".yaml", ".yml", ".zsh",
    ];
    extensions.iter().any(|extension| path.ends_with(extension))
}

fn is_generated_candidate(path: &str) -> bool {
    path == "Cargo.lock"
        || path.ends_with("/package-lock.json")
        || path == "package-lock.json"
        || path.starts_with("target/")
        || path.contains("/target/")
        || path.starts_with(".ripr/release/")
        || path.starts_with("dist/")
        || path.contains("/dist/")
        || path.ends_with(".vsix")
        || path.ends_with(".zip")
        || path.ends_with(".tar.gz")
        || path.ends_with(".sha256")
}

fn is_dependency_surface_candidate(path: &str) -> bool {
    let Some(file_name) = path.rsplit('/').next() else {
        return false;
    };
    matches!(
        file_name,
        "Cargo.toml"
            | "Cargo.lock"
            | "package.json"
            | "package-lock.json"
            | "npm-shrinkwrap.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "requirements.txt"
            | "pyproject.toml"
            | "poetry.lock"
            | "Pipfile"
            | "Pipfile.lock"
            | "go.mod"
            | "go.sum"
            | "pom.xml"
            | "build.gradle"
            | "settings.gradle"
            | "gradle.lockfile"
            | "Gemfile"
            | "Gemfile.lock"
    )
}

fn is_process_policy_candidate(path: &str) -> bool {
    path.ends_with(".rs") || path.ends_with(".ts")
}

fn is_network_policy_candidate(path: &str) -> bool {
    path.ends_with(".rs")
        || path.ends_with(".ts")
        || path.ends_with(".yml")
        || path.ends_with(".yaml")
}

fn process_policy_patterns() -> Vec<String> {
    [
        concat!("use std::process::", "Command"),
        concat!("Command", "::new"),
        concat!("child", "_process"),
        concat!("cp.", "spawn"),
        concat!("cp.", "exec("),
        concat!("cp.", "execFile"),
    ]
    .iter()
    .map(|value| value.to_string())
    .collect()
}

fn network_policy_patterns() -> Vec<String> {
    [
        concat!("https", ".get"),
        concat!("fetch", "("),
        concat!("req", "west"),
        concat!("u", "req"),
        concat!("Tcp", "Stream"),
        concat!("cu", "rl"),
        concat!("w", "get"),
    ]
    .iter()
    .map(|value| value.to_string())
    .collect()
}

fn shell_fetch_tool_name() -> &'static str {
    concat!("cu", "rl")
}

fn matches_any_glob(allowlist: &[GlobAllow], path: &str) -> bool {
    allowlist
        .iter()
        .any(|entry| glob_matches(&entry.glob, path))
}

fn glob_matches(pattern: &str, path: &str) -> bool {
    let pattern_parts = pattern.split('/').collect::<Vec<_>>();
    let path_parts = path.split('/').collect::<Vec<_>>();
    glob_parts_match(&pattern_parts, &path_parts)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StaticLanguageMatcher {
    Path(String),
    Glob(String),
}

impl StaticLanguageMatcher {
    fn as_str(&self) -> &str {
        match self {
            StaticLanguageMatcher::Path(value) | StaticLanguageMatcher::Glob(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StaticLanguageAllowEntry {
    matcher: StaticLanguageMatcher,
    owner: String,
    reason: String,
}

#[cfg(test)]
impl StaticLanguageAllowEntry {
    fn new_path(
        path: impl Into<String>,
        owner: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            matcher: StaticLanguageMatcher::Path(path.into()),
            owner: owner.into(),
            reason: reason.into(),
        }
    }

    fn new_glob(
        glob: impl Into<String>,
        owner: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            matcher: StaticLanguageMatcher::Glob(glob.into()),
            owner: owner.into(),
            reason: reason.into(),
        }
    }
}

const STATIC_LANGUAGE_ALLOWLIST_PATH: &str = ".ripr/static-language-allowlist.toml";
const STATIC_LANGUAGE_ALLOWLIST_LEGACY_PATH: &str = ".ripr/static-language-allowlist.txt";
const STATIC_LANGUAGE_ALLOWED_GLOBS: &[&str] = &["docs/*.md", "docs/**/*.md"];

fn parse_static_language_allowlist(text: &str) -> (Vec<StaticLanguageAllowEntry>, Vec<String>) {
    let mut entries: Vec<StaticLanguageAllowEntry> = Vec::new();
    let mut violations = Vec::new();
    let mut schema_seen = false;
    let mut current: Option<PendingAllowEntry> = None;

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "[[allow]]" {
            if let Some(pending) = current.take() {
                finalize_static_language_entry(pending, &mut entries, &mut violations);
            }
            current = Some(PendingAllowEntry::new(line_number));
            continue;
        }
        let Some((key, raw_value)) = trimmed.split_once('=') else {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} expected `key = value`"
            ));
            continue;
        };
        let key = key.trim();
        let raw_value = raw_value.trim();
        if let Some(pending) = current.as_mut() {
            match key {
                "path" => assign_static_language_field(
                    raw_value,
                    line_number,
                    &mut violations,
                    |parsed| pending.path = Some((parsed, line_number)),
                ),
                "glob" => assign_static_language_field(
                    raw_value,
                    line_number,
                    &mut violations,
                    |parsed| pending.glob = Some((parsed, line_number)),
                ),
                "owner" => assign_static_language_field(
                    raw_value,
                    line_number,
                    &mut violations,
                    |parsed| pending.owner = Some((parsed, line_number)),
                ),
                "reason" => assign_static_language_field(
                    raw_value,
                    line_number,
                    &mut violations,
                    |parsed| pending.reason = Some((parsed, line_number)),
                ),
                _ => violations.push(format!(
                    "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} unsupported `[[allow]]` field `{key}`"
                )),
            }
        } else if key == "schema_version" {
            schema_seen = true;
            match raw_value.parse::<u32>() {
                Ok(1) => {}
                Ok(other) => violations.push(format!(
                    "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} schema_version = {other} is not supported (expected 1)"
                )),
                Err(_) => violations.push(format!(
                    "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} schema_version must be an integer literal"
                )),
            }
        } else {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} unsupported top-level field `{key}`"
            ));
        }
    }

    if let Some(pending) = current.take() {
        finalize_static_language_entry(pending, &mut entries, &mut violations);
    }

    if !schema_seen {
        violations.push(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH} is missing required `schema_version = 1` header"
        ));
    }

    let mut seen_matchers: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in &entries {
        let matcher = entry.matcher.as_str();
        if let Some(&first) = seen_matchers.get(matcher) {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH} matcher `{matcher}` is duplicated (first declared near line {first})"
            ));
        } else {
            seen_matchers.insert(matcher, 0);
        }
    }

    (entries, violations)
}

struct PendingAllowEntry {
    block_line: usize,
    path: Option<(String, usize)>,
    glob: Option<(String, usize)>,
    owner: Option<(String, usize)>,
    reason: Option<(String, usize)>,
}

impl PendingAllowEntry {
    fn new(block_line: usize) -> Self {
        Self {
            block_line,
            path: None,
            glob: None,
            owner: None,
            reason: None,
        }
    }
}

fn assign_static_language_field<F>(
    raw_value: &str,
    line_number: usize,
    violations: &mut Vec<String>,
    mut assign: F,
) where
    F: FnMut(String),
{
    match parse_quoted_value(raw_value) {
        Ok(parsed) => assign(parsed),
        Err(message) => violations.push(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} {message}"
        )),
    }
}

fn finalize_static_language_entry(
    pending: PendingAllowEntry,
    entries: &mut Vec<StaticLanguageAllowEntry>,
    violations: &mut Vec<String>,
) {
    let block_line = pending.block_line;
    let path_value = pending.path;
    let glob_value = pending.glob;
    let owner_value = pending.owner;
    let reason_value = pending.reason;

    let matcher = match (path_value, glob_value) {
        (Some((path, line)), None) => match validate_static_language_path(&path, line) {
            Ok(()) => Some(StaticLanguageMatcher::Path(path)),
            Err(message) => {
                violations.push(message);
                None
            }
        },
        (None, Some((glob, line))) => match validate_static_language_glob(&glob, line) {
            Ok(()) => Some(StaticLanguageMatcher::Glob(glob)),
            Err(message) => {
                violations.push(message);
                None
            }
        },
        (Some(_), Some(_)) => {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{block_line} `[[allow]]` entry has both `path` and `glob`; declare exactly one"
            ));
            None
        }
        (None, None) => {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{block_line} `[[allow]]` entry must declare either `path` or `glob`"
            ));
            None
        }
    };

    let owner = match owner_value {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!(
                    "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line} `owner` is blank; name a responsible team or maintainer"
                ));
                None
            } else {
                Some(value)
            }
        }
        None => {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{block_line} `[[allow]]` entry is missing required `owner`"
            ));
            None
        }
    };

    let reason = match reason_value {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!(
                    "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line} `reason` is blank; explain why this matcher is exempt"
                ));
                None
            } else {
                Some(value)
            }
        }
        None => {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{block_line} `[[allow]]` entry is missing required `reason`"
            ));
            None
        }
    };

    if let (Some(matcher), Some(owner), Some(reason)) = (matcher, owner, reason) {
        entries.push(StaticLanguageAllowEntry {
            matcher,
            owner,
            reason,
        });
    }
}

fn validate_static_language_path(path: &str, line_number: usize) -> Result<(), String> {
    if path.is_empty() {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `path` is empty"
        ));
    }
    if path.contains('\\') {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `path` `{path}` uses backslashes; use `/` separators"
        ));
    }
    if is_absolute_path_like(path) {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `path` `{path}` is absolute; entries must be repository-relative"
        ));
    }
    if path.contains('*') || path.contains('?') {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `path` `{path}` contains glob characters; use `glob = ...` instead"
        ));
    }
    Ok(())
}

fn validate_static_language_glob(glob: &str, line_number: usize) -> Result<(), String> {
    if glob.is_empty() {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `glob` is empty"
        ));
    }
    if glob.contains('\\') {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `glob` `{glob}` uses backslashes; use `/` separators"
        ));
    }
    if is_absolute_path_like(glob) {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `glob` `{glob}` is absolute; entries must be repository-relative"
        ));
    }
    if !STATIC_LANGUAGE_ALLOWED_GLOBS.contains(&glob) {
        return Err(format!(
            "{STATIC_LANGUAGE_ALLOWLIST_PATH}:{line_number} `glob` `{glob}` is not in the scoped set; current allowed globs: {}",
            STATIC_LANGUAGE_ALLOWED_GLOBS.join(", ")
        ));
    }
    Ok(())
}

fn is_absolute_path_like(value: &str) -> bool {
    if value.starts_with('/') {
        return true;
    }
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn load_static_language_allowlist() -> Result<Vec<StaticLanguageAllowEntry>, Vec<String>> {
    if Path::new(STATIC_LANGUAGE_ALLOWLIST_LEGACY_PATH).exists() {
        return Err(vec![format!(
            "{STATIC_LANGUAGE_ALLOWLIST_LEGACY_PATH} still exists; the static-language allowlist moved to {STATIC_LANGUAGE_ALLOWLIST_PATH}. Delete the legacy `.txt` file to avoid split-brain policy."
        )]);
    }
    let path = Path::new(STATIC_LANGUAGE_ALLOWLIST_PATH);
    let text = read_text_lossy(path).map_err(|err| vec![err])?;
    let (entries, mut violations) = parse_static_language_allowlist(&text);
    for entry in &entries {
        if let StaticLanguageMatcher::Path(value) = &entry.matcher
            && !Path::new(value).exists()
        {
            violations.push(format!(
                "{STATIC_LANGUAGE_ALLOWLIST_PATH} matcher `{value}` does not exist on disk"
            ));
        }
    }
    if violations.is_empty() {
        Ok(entries)
    } else {
        Err(violations)
    }
}

fn static_language_allowlist_covers(allowlist: &[StaticLanguageAllowEntry], path: &str) -> bool {
    allowlist.iter().any(|entry| match &entry.matcher {
        StaticLanguageMatcher::Path(value) => value == path,
        StaticLanguageMatcher::Glob(value) => glob_matches(value, path),
    })
}

fn should_scan_static_language_path(allowlist: &[StaticLanguageAllowEntry], path: &str) -> bool {
    is_static_language_candidate(path) && !static_language_allowlist_covers(allowlist, path)
}

fn glob_parts_match(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }
    if pattern[0] == "**" {
        return glob_parts_match(&pattern[1..], path)
            || (!path.is_empty() && glob_parts_match(pattern, &path[1..]));
    }
    if path.is_empty() {
        return false;
    }
    segment_matches(pattern[0], path[0]) && glob_parts_match(&pattern[1..], &path[1..])
}

fn segment_matches(pattern: &str, value: &str) -> bool {
    let pattern_chars = pattern.chars().collect::<Vec<_>>();
    let value_chars = value.chars().collect::<Vec<_>>();
    segment_parts_match(&pattern_chars, &value_chars)
}

fn segment_parts_match(pattern: &[char], value: &[char]) -> bool {
    if pattern.is_empty() {
        return value.is_empty();
    }
    if pattern[0] == '*' {
        return segment_parts_match(&pattern[1..], value)
            || (!value.is_empty() && segment_parts_match(pattern, &value[1..]));
    }
    !value.is_empty() && pattern[0] == value[0] && segment_parts_match(&pattern[1..], &value[1..])
}

fn parse_git_stage_line(line: &str) -> Option<(&str, &str)> {
    let mut parts = line.split_whitespace();
    let mode = parts.next()?;
    let _object_type = parts.next()?;
    let _hash = parts.next()?;
    let stage_and_path = line.split('\t').nth(1)?;
    Some((mode, stage_and_path))
}

fn extract_workflow_run_blocks(text: &str) -> Vec<RunBlock> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut blocks = Vec::new();
    let mut idx = 0usize;
    while idx < lines.len() {
        let line = lines[idx];
        let trimmed = line.trim_start();
        if let Some(rest) = workflow_run_value(trimmed) {
            let indent = line.len() - trimmed.len();
            let run_value = rest.trim();
            if run_value == "|" || run_value == ">" || run_value == "|-" || run_value == ">-" {
                let mut block_lines = Vec::new();
                let mut next_idx = idx + 1;
                while next_idx < lines.len() {
                    let next = lines[next_idx];
                    let next_trimmed = next.trim_start();
                    let next_indent = next.len() - next_trimmed.len();
                    if !next_trimmed.is_empty() && next_indent <= indent {
                        break;
                    }
                    block_lines.push(next_trimmed.to_string());
                    next_idx += 1;
                }
                let non_empty_lines = block_lines
                    .iter()
                    .filter(|value| !value.trim().is_empty())
                    .count();
                blocks.push(RunBlock {
                    line_number: idx + 1,
                    non_empty_lines,
                    text: block_lines.join("\n"),
                });
                idx = next_idx;
                continue;
            }
            blocks.push(RunBlock {
                line_number: idx + 1,
                non_empty_lines: usize::from(!run_value.is_empty()),
                text: run_value.to_string(),
            });
        }
        idx += 1;
    }
    blocks
}

fn workflow_run_value(trimmed_line: &str) -> Option<&str> {
    trimmed_line
        .strip_prefix("run:")
        .or_else(|| trimmed_line.strip_prefix("- run:"))
}

fn forbidden_static_terms() -> Vec<String> {
    ["killed", "survived", "untested", "proven", "adequate"]
        .iter()
        .map(|value| value.to_string())
        .collect()
}

fn forbidden_panic_patterns() -> Vec<String> {
    [
        concat!("unwrap", "("),
        concat!("expect", "("),
        concat!("panic", "!"),
        concat!("todo", "!"),
        concat!("unimplemented", "!"),
        concat!("unreachable", "!"),
    ]
    .iter()
    .map(|value| value.to_string())
    .collect()
}

fn contains_word(text: &str, word: &str) -> bool {
    let mut start = 0usize;
    while let Some(offset) = text[start..].find(word) {
        let idx = start + offset;
        let before = text[..idx].chars().next_back();
        let after = text[idx + word.len()..].chars().next();
        if !is_word_char(before) && !is_word_char(after) {
            return true;
        }
        start = idx + word.len();
    }
    false
}

fn is_word_char(value: Option<char>) -> bool {
    value
        .map(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        .unwrap_or(false)
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct PanicFinding {
    path: String,
    line: usize,
    column: Option<usize>,
    family: String,
}

#[derive(Debug, Clone)]
struct PanicAllowEntry {
    path: String,
    line: usize,
    column: Option<usize>,
    family: String,
    classification: Option<String>,
    explanation: String,
}

#[derive(Debug, Clone)]
struct PanicFamilySelector {
    kind: String,
    container: Option<String>,
    callee: Option<String>,
    receiver_fingerprint: Option<String>,
    text_contains: Option<String>,
}

#[derive(Debug, Clone)]
struct PanicFamilyLastSeen {
    line: usize,
    column: Option<usize>,
}

#[derive(Debug, Clone)]
struct PanicAllowEntryV2 {
    path: String,
    family: String,
    classification: Option<String>,
    explanation: String,
    selector: Option<PanicFamilySelector>,
    last_seen: Option<PanicFamilyLastSeen>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct SemanticPanicFinding {
    path: String,
    family: String,
    kind: String,
    line: usize,
    column: Option<usize>,
    container: Option<String>,
    callee: Option<String>,
    receiver_fingerprint: Option<String>,
    snippet_fingerprint: String,
}

fn panic_family_from_pattern(pattern: &str) -> &'static str {
    match pattern {
        s if s.contains("unwrap") => "unwrap",
        s if s.contains("expect") => "expect",
        s if s.contains("panic!") => "panic_macro",
        s if s.contains("todo!") => "todo",
        s if s.contains("unimplemented!") => "unimplemented",
        s if s.contains("unreachable!") => "unreachable",
        _ => "unknown",
    }
}

fn collect_panic_findings(root: &Path, patterns: &[String]) -> Result<Vec<PanicFinding>, String> {
    let mut findings = Vec::new();

    for path in collect_files(root)? {
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let normalized = normalize_path(&path);
        let text = read_text_lossy(&path)?;

        for (line_num, line) in text.lines().enumerate() {
            let line_number = line_num + 1;
            for pattern in patterns {
                let mut start = 0usize;
                while let Some(offset) = line[start..].find(pattern) {
                    let col = start + offset + 1;
                    findings.push(PanicFinding {
                        path: normalized.clone(),
                        line: line_number,
                        column: Some(col),
                        family: panic_family_from_pattern(pattern).to_string(),
                    });
                    start = col;
                }
            }
        }
    }

    findings.sort();
    Ok(findings)
}

fn collect_semantic_panic_findings(
    root: &Path,
    patterns: &[String],
) -> Result<Vec<SemanticPanicFinding>, String> {
    use ra_ap_syntax::{AstNode, Edition, SourceFile};

    let mut findings = Vec::new();

    for path in collect_files(root)? {
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let normalized = normalize_path(&path);
        let text = read_text_lossy(&path)?;

        let parse = SourceFile::parse(&text, Edition::Edition2024);
        let tree = parse.tree();
        let root_node = tree.syntax();
        extract_panic_calls_from_node(root_node, &text, &normalized, patterns, &mut findings);
    }

    findings.sort();
    Ok(findings)
}

fn extract_panic_calls_from_node(
    node: &ra_ap_syntax::SyntaxNode,
    text: &str,
    path: &str,
    patterns: &[String],
    findings: &mut Vec<SemanticPanicFinding>,
) {
    use ra_ap_syntax::ast::{self, AstNode};

    for child in node.children() {
        let matched = if let Some(call_expr) = ast::MethodCallExpr::cast(child.clone()) {
            call_expr.name_ref().and_then(|method_name| {
                let name = method_name.text().to_string();
                if pattern_matches_panic_call(patterns, &name) {
                    extract_call_metadata(call_expr.syntax(), text, path, &name, "method_call")
                } else {
                    None
                }
            })
        } else if let Some(call_expr) = ast::CallExpr::cast(child.clone()) {
            call_expr.expr().and_then(|expr| {
                let func_text = expr.syntax().text().to_string();
                let base_callee = base_name_from_callee_text(&func_text);
                if pattern_matches_panic_call(patterns, base_callee) {
                    extract_call_metadata(call_expr.syntax(), text, path, base_callee, "call")
                } else {
                    None
                }
            })
        } else if let Some(macro_call) = ast::MacroCall::cast(child.clone()) {
            macro_call
                .path()
                .and_then(|p| p.segment())
                .and_then(|path_seg| {
                    let name = path_seg
                        .name_ref()
                        .map(|n| n.text().to_string())
                        .unwrap_or_default();
                    let macro_name = format!("{}!", name);
                    if pattern_matches_panic_call(patterns, &macro_name) {
                        extract_call_metadata(
                            macro_call.syntax(),
                            text,
                            path,
                            &macro_name,
                            "macro_call",
                        )
                    } else {
                        None
                    }
                })
        } else {
            None
        };

        if let Some(finding) = matched {
            findings.push(finding);
        }

        extract_panic_calls_from_node(&child, text, path, patterns, findings);
    }
}

fn base_name_from_callee_text(callee_text: &str) -> &str {
    callee_text.rsplit("::").next().unwrap_or(callee_text)
}

fn pattern_matches_panic_call(patterns: &[String], text: &str) -> bool {
    for pattern in patterns {
        if pattern == text {
            return true;
        }
        let base = pattern.trim_end_matches('(').trim_end_matches('!');
        if base == text && !base.is_empty() {
            return true;
        }
    }
    false
}

fn extract_call_metadata(
    node: &ra_ap_syntax::SyntaxNode,
    text: &str,
    path: &str,
    family_name: &str,
    kind: &str,
) -> Option<SemanticPanicFinding> {
    let (line, column) = line_and_column_for_node(node, text);
    let family = panic_family_from_call_name(family_name).to_string();
    let snippet = node.text().to_string();
    let snippet_fingerprint = snippet.replace('\n', " ").trim().to_string();

    let receiver_fingerprint = if kind == "method_call" {
        extract_method_receiver_fingerprint(node)
    } else {
        None
    };

    Some(SemanticPanicFinding {
        path: path.to_string(),
        family,
        kind: kind.to_string(),
        line,
        column,
        container: extract_container_name(node),
        callee: Some(family_name.to_string()),
        receiver_fingerprint,
        snippet_fingerprint,
    })
}

fn extract_method_receiver_fingerprint(node: &ra_ap_syntax::SyntaxNode) -> Option<String> {
    use ra_ap_syntax::ast::{self, AstNode};

    let method_call = ast::MethodCallExpr::cast(node.clone())?;
    let receiver = method_call.receiver()?;
    let text = receiver.syntax().text().to_string();
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(normalized)
}

fn line_and_column_for_node(node: &ra_ap_syntax::SyntaxNode, text: &str) -> (usize, Option<usize>) {
    let offset: usize = node.text_range().start().into();
    let mut line = 1;
    let mut col = 0;

    for (byte_idx, ch) in text.char_indices() {
        if byte_idx > offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    (line, Some(col))
}

fn panic_family_from_call_name(name: &str) -> &'static str {
    match name {
        "unwrap" => "unwrap",
        "expect" => "expect",
        "panic!" => "panic_macro",
        "todo!" => "todo",
        "unimplemented!" => "unimplemented",
        "unreachable!" => "unreachable",
        s if s.starts_with("unwrap") && s.ends_with('(') => "unwrap",
        s if s.starts_with("expect") && s.ends_with('(') => "expect",
        s if s.starts_with("panic") && s.ends_with('!') => "panic_macro",
        s if s.starts_with("todo") && s.ends_with('!') => "todo",
        s if s.starts_with("unimplemented") && s.ends_with('!') => "unimplemented",
        s if s.starts_with("unreachable") && s.ends_with('!') => "unreachable",
        _ => "unknown",
    }
}

fn extract_container_name(node: &ra_ap_syntax::SyntaxNode) -> Option<String> {
    use ra_ap_syntax::ast::{self, AstNode, HasName};

    let mut current = node.parent();
    while let Some(parent) = current {
        let result = (|| {
            if let Some(func) = ast::Fn::cast(parent.clone()) {
                return func.name().map(|n| n.text().to_string());
            }
            if let Some(impl_block) = ast::Impl::cast(parent.clone()) {
                return impl_block.self_ty().and_then(|t| {
                    if let ast::Type::PathType(pt) = t {
                        pt.path().and_then(|p| {
                            p.segment()
                                .and_then(|s| s.name_ref().map(|n| n.text().to_string()))
                        })
                    } else {
                        None
                    }
                });
            }
            None
        })();
        if result.is_some() {
            return result;
        }
        current = parent.parent();
    }
    None
}

fn semantic_selector_matches(
    selector: &PanicFamilySelector,
    finding: &SemanticPanicFinding,
) -> bool {
    let valid_kind = matches!(
        selector.kind.as_str(),
        "method_call" | "call" | "macro_call" | "string_literal"
    );
    if !valid_kind {
        return false;
    }

    if selector.kind == "string_literal" {
        if finding.kind != "string_literal" {
            return false;
        }
        return selector
            .text_contains
            .as_ref()
            .is_some_and(|tc| finding.snippet_fingerprint.contains(tc));
    }

    selector.kind == finding.kind
        && (selector.container.is_none()
            || finding.container.as_ref() == selector.container.as_ref())
        && (selector.callee.is_none() || finding.callee.as_ref() == selector.callee.as_ref())
        && (selector.receiver_fingerprint.is_none()
            || finding.receiver_fingerprint.as_ref() == selector.receiver_fingerprint.as_ref())
}

fn parse_no_panic_allowlist_toml(path: &str) -> Result<Vec<PanicAllowEntry>, String> {
    let text = read_text_lossy(Path::new(path))?;
    let mut entries = Vec::new();
    let mut in_allow_section = false;
    let mut current_entry = PanicAllowEntry {
        path: String::new(),
        line: 0,
        column: None,
        family: String::new(),
        classification: None,
        explanation: String::new(),
    };
    let mut has_entry_started = false;
    let mut entry_start_line = 0;

    for (line_num, line) in text.lines().enumerate() {
        let line_number = line_num + 1;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed == "schema_version = \"0.1\"" {
            continue;
        }

        if trimmed == "[[allow]]" {
            if has_entry_started {
                validate_panic_allow_entry(&current_entry, path, entry_start_line)?;
                entries.push(current_entry.clone());
            }
            current_entry = PanicAllowEntry {
                path: String::new(),
                line: 0,
                column: None,
                family: String::new(),
                classification: None,
                explanation: String::new(),
            };
            has_entry_started = true;
            in_allow_section = true;
            entry_start_line = line_number;
            continue;
        }

        if !in_allow_section {
            return Err(format!(
                "{path}:{} unexpected content outside [[allow]] section",
                line_number
            ));
        }

        let Some((key, value)) = parse_toml_key_value(trimmed) else {
            return Err(format!(
                "{path}:{} invalid TOML syntax (expected key = value)",
                line_number
            ));
        };

        match key {
            "path" => current_entry.path = parse_string_value(value, path, line_number)?,
            "line" => current_entry.line = parse_usize_value(value, path, line_number)?,
            "column" => current_entry.column = Some(parse_usize_value(value, path, line_number)?),
            "family" => current_entry.family = parse_string_value(value, path, line_number)?,
            "classification" => {
                current_entry.classification = Some(parse_string_value(value, path, line_number)?)
            }
            "explanation" => {
                current_entry.explanation = parse_string_value(value, path, line_number)?
            }
            _ => {
                return Err(format!(
                    "{path}:{} unknown field '{key}' in [[allow]] section",
                    line_number
                ));
            }
        }
    }

    if has_entry_started {
        validate_panic_allow_entry(&current_entry, path, entry_start_line)?;
        entries.push(current_entry);
    }

    check_duplicate_panic_allow_entries(&entries, path)?;
    Ok(entries)
}

fn parse_toml_key_value(trimmed: &str) -> Option<(&str, &str)> {
    let equals_idx = trimmed.find('=')?;
    let key = trimmed[..equals_idx].trim();
    let value_part = trimmed[equals_idx + 1..].trim();
    Some((key, value_part))
}

fn parse_string_value(value: &str, path: &str, line_number: usize) -> Result<String, String> {
    let v = strip_toml_value_comment(value).trim();
    if v.starts_with('"') && v.ends_with('"') && v.len() >= 2 {
        Ok(unescape_toml_string(&v[1..v.len() - 1]))
    } else {
        Err(format!(
            "{path}:{} string value must be quoted (got: {value})",
            line_number
        ))
    }
}

fn strip_toml_value_comment(value: &str) -> &str {
    let mut in_double = false;
    let mut escaped = false;

    for (idx, ch) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if in_double && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_double = !in_double;
            continue;
        }
        if ch == '#' && !in_double {
            return &value[..idx];
        }
    }

    value
}

fn unescape_toml_string(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        let Some(next) = chars.next() else {
            out.push('\\');
            break;
        };
        match next {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }

    out
}

fn parse_usize_value(value: &str, path: &str, line_number: usize) -> Result<usize, String> {
    let v = value.split('#').next().unwrap_or(value).trim();
    v.parse::<usize>()
        .map_err(|_err| format!("{path}:{} invalid number (got: {value})", line_number))
}

fn validate_panic_allow_entry(
    entry: &PanicAllowEntry,
    path: &str,
    line_number: usize,
) -> Result<(), String> {
    if entry.path.is_empty() {
        return Err(format!(
            "{path}:{} missing required field: path",
            line_number
        ));
    }
    if entry.line == 0 {
        return Err(format!(
            "{path}:{} missing required field: line",
            line_number
        ));
    }
    if entry.family.is_empty() {
        return Err(format!(
            "{path}:{} missing required field: family",
            line_number
        ));
    }
    if entry.explanation.is_empty() {
        return Err(format!(
            "{path}:{} missing required field: explanation",
            line_number
        ));
    }
    Ok(())
}

fn check_duplicate_panic_allow_entries(
    entries: &[PanicAllowEntry],
    path: &str,
) -> Result<(), String> {
    let mut seen = BTreeMap::new();
    for entry in entries {
        let key = (
            entry.path.clone(),
            entry.line,
            entry.column,
            entry.family.clone(),
        );
        if seen.contains_key(&key) {
            return Err(format!(
                "{path}: duplicate allowlist entry for {}:{}:{:?} ({})",
                entry.path, entry.line, entry.column, entry.family
            ));
        }
        seen.insert(key, entry.line);
    }
    Ok(())
}

fn check_old_panic_allowlist_exists() -> Result<(), String> {
    if Path::new(".ripr/no-panic-allowlist.txt").exists() {
        return Err(
            ".ripr/no-panic-allowlist.txt still exists; use .ripr/no-panic-allowlist.toml instead"
                .to_string(),
        );
    }
    Ok(())
}

#[derive(Debug, Clone)]
enum PanicAllowEntryVersioned {
    V1(PanicAllowEntry),
    V2(PanicAllowEntryV2),
}

fn parse_no_panic_allowlist_toml_v2(path: &str) -> Result<Vec<PanicAllowEntryVersioned>, String> {
    let text = read_text_lossy(Path::new(path))?;
    let mut entries = Vec::new();

    // Accumulated fields for current entry
    let mut entry_path = String::new();
    let mut entry_line: usize = 0;
    let mut entry_column: Option<usize> = None;
    let mut entry_family = String::new();
    let mut entry_classification: Option<String> = None;
    let mut entry_explanation = String::new();
    let mut selector_kind = String::new();
    let mut selector_container: Option<String> = None;
    let mut selector_callee: Option<String> = None;
    let mut selector_receiver_fingerprint: Option<String> = None;
    let mut selector_text_contains: Option<String> = None;
    let mut last_seen_line: usize = 0;
    let mut last_seen_column: Option<usize> = None;

    let mut in_allow_section = false;
    let mut in_selector_section = false;
    let mut in_last_seen_section = false;
    let mut has_entry_started = false;
    let mut entry_start_line = 0;

    let flush_entry = |has_entry: bool,
                       e_path: &str,
                       e_line: usize,
                       e_column: Option<usize>,
                       e_family: &str,
                       e_classification: &Option<String>,
                       e_explanation: &str,
                       s_kind: &str,
                       s_container: &Option<String>,
                       s_callee: &Option<String>,
                       s_receiver_fp: &Option<String>,
                       s_text_contains: &Option<String>,
                       ls_line: usize,
                       ls_column: Option<usize>,
                       path: &str,
                       start_line: usize|
     -> Result<Option<PanicAllowEntryVersioned>, String> {
        if !has_entry {
            return Ok(None);
        }
        if e_path.is_empty() {
            return Err(format!("{path}:{start_line} missing required field: path"));
        }
        if e_family.is_empty() {
            return Err(format!(
                "{path}:{start_line} missing required field: family"
            ));
        }
        if e_explanation.is_empty() {
            return Err(format!(
                "{path}:{start_line} missing required field: explanation"
            ));
        }

        if !s_kind.is_empty() {
            // v0.2 entry with selector
            let selector = PanicFamilySelector {
                kind: s_kind.to_string(),
                container: s_container.clone(),
                callee: s_callee.clone(),
                receiver_fingerprint: s_receiver_fp.clone(),
                text_contains: s_text_contains.clone(),
            };
            let last_seen = if ls_line > 0 {
                Some(PanicFamilyLastSeen {
                    line: ls_line,
                    column: ls_column,
                })
            } else {
                None
            };
            let entry = PanicAllowEntryV2 {
                path: e_path.to_string(),
                family: e_family.to_string(),
                classification: e_classification.clone(),
                explanation: e_explanation.to_string(),
                selector: Some(selector),
                last_seen,
            };
            validate_panic_allow_entry_v2(&entry, path, start_line)?;
            Ok(Some(PanicAllowEntryVersioned::V2(entry)))
        } else if e_line > 0 {
            // v0.1 entry with line number
            let entry = PanicAllowEntry {
                path: e_path.to_string(),
                line: e_line,
                column: e_column,
                family: e_family.to_string(),
                classification: e_classification.clone(),
                explanation: e_explanation.to_string(),
            };
            validate_panic_allow_entry(&entry, path, start_line)?;
            Ok(Some(PanicAllowEntryVersioned::V1(entry)))
        } else {
            Err(format!(
                "{path}:{start_line} entry must have either a [allow.selector] or line number"
            ))
        }
    };

    for (line_num, line) in text.lines().enumerate() {
        let line_number = line_num + 1;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed == "schema_version = \"0.1\"" || trimmed == "schema_version = \"0.2\"" {
            continue;
        }

        if trimmed == "[allow.selector]" {
            in_selector_section = true;
            in_last_seen_section = false;
            in_allow_section = false;
            continue;
        }

        if trimmed == "[allow.last_seen]" {
            in_last_seen_section = true;
            in_selector_section = false;
            in_allow_section = false;
            continue;
        }

        if trimmed == "[[allow]]" {
            // Flush previous entry
            let result = flush_entry(
                has_entry_started,
                &entry_path,
                entry_line,
                entry_column,
                &entry_family,
                &entry_classification,
                &entry_explanation,
                &selector_kind,
                &selector_container,
                &selector_callee,
                &selector_receiver_fingerprint,
                &selector_text_contains,
                last_seen_line,
                last_seen_column,
                path,
                entry_start_line,
            )?;
            if let Some(entry) = result {
                entries.push(entry);
            }

            // Reset all fields for new entry
            entry_path = String::new();
            entry_line = 0;
            entry_column = None;
            entry_family = String::new();
            entry_classification = None;
            entry_explanation = String::new();
            selector_kind = String::new();
            selector_container = None;
            selector_callee = None;
            selector_receiver_fingerprint = None;
            selector_text_contains = None;
            last_seen_line = 0;
            last_seen_column = None;

            in_selector_section = false;
            in_last_seen_section = false;
            in_allow_section = true;
            has_entry_started = true;
            entry_start_line = line_number;
            continue;
        }

        if !in_allow_section && !in_selector_section && !in_last_seen_section {
            return Err(format!(
                "{path}:{line_number} unexpected content outside [[allow]] section"
            ));
        }

        let Some((key, value)) = parse_toml_key_value(trimmed) else {
            return Err(format!(
                "{path}:{line_number} invalid TOML syntax (expected key = value)"
            ));
        };

        if in_selector_section {
            match key {
                "kind" => {
                    selector_kind = parse_string_value(value, path, line_number)?;
                }
                "container" => {
                    selector_container = Some(parse_string_value(value, path, line_number)?);
                }
                "callee" => {
                    selector_callee = Some(parse_string_value(value, path, line_number)?);
                }
                "receiver_fingerprint" => {
                    selector_receiver_fingerprint =
                        Some(parse_string_value(value, path, line_number)?);
                }
                "text_contains" => {
                    selector_text_contains = Some(parse_string_value(value, path, line_number)?);
                }
                _ => {
                    return Err(format!(
                        "{path}:{line_number} unknown field '{key}' in [allow.selector] section"
                    ));
                }
            }
            continue;
        }

        if in_last_seen_section {
            match key {
                "line" => {
                    last_seen_line = parse_usize_value(value, path, line_number)?;
                }
                "column" => {
                    last_seen_column = Some(parse_usize_value(value, path, line_number)?);
                }
                _ => {
                    return Err(format!(
                        "{path}:{line_number} unknown field '{key}' in [allow.last_seen] section"
                    ));
                }
            }
            continue;
        }

        // In [[allow]] section
        match key {
            "path" => entry_path = parse_string_value(value, path, line_number)?,
            "line" => entry_line = parse_usize_value(value, path, line_number)?,
            "column" => entry_column = Some(parse_usize_value(value, path, line_number)?),
            "family" => entry_family = parse_string_value(value, path, line_number)?,
            "classification" => {
                entry_classification = Some(parse_string_value(value, path, line_number)?)
            }
            "explanation" => entry_explanation = parse_string_value(value, path, line_number)?,
            _ => {
                return Err(format!(
                    "{path}:{line_number} unknown field '{key}' in [[allow]] section"
                ));
            }
        }
    }

    // Flush final entry
    let result = flush_entry(
        has_entry_started,
        &entry_path,
        entry_line,
        entry_column,
        &entry_family,
        &entry_classification,
        &entry_explanation,
        &selector_kind,
        &selector_container,
        &selector_callee,
        &selector_receiver_fingerprint,
        &selector_text_contains,
        last_seen_line,
        last_seen_column,
        path,
        entry_start_line,
    )?;
    if let Some(entry) = result {
        entries.push(entry);
    }

    Ok(entries)
}

fn validate_panic_allow_entry_v2(
    entry: &PanicAllowEntryV2,
    path: &str,
    line_number: usize,
) -> Result<(), String> {
    if entry.path.is_empty() {
        return Err(format!("{path}:{line_number} missing required field: path"));
    }
    if entry.family.is_empty() {
        return Err(format!(
            "{path}:{line_number} missing required field: family"
        ));
    }
    if entry.explanation.is_empty() {
        return Err(format!(
            "{path}:{line_number} missing required field: explanation"
        ));
    }
    if let Some(ref selector) = entry.selector {
        if selector.kind.is_empty() {
            return Err(format!(
                "{path}:{line_number} selector missing required field: kind"
            ));
        }
        let supported_kinds = ["method_call", "macro_call", "call", "string_literal"];
        if !supported_kinds.contains(&selector.kind.as_str()) {
            return Err(format!(
                "{path}:{line_number} invalid selector kind '{}' in {path}; expected one of: {}",
                selector.kind,
                supported_kinds.join(", ")
            ));
        }
        if selector.kind == "string_literal" && selector.text_contains.is_none() {
            return Err(format!(
                "{path}:{line_number} string_literal selector requires text_contains"
            ));
        }
    }
    Ok(())
}

fn strip_yaml_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let chars: Vec<char> = line.chars().collect();
    for idx in 0..chars.len() {
        match chars[idx] {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => {
                let backslash_run = chars[..idx]
                    .iter()
                    .rev()
                    .take_while(|&&c| c == '\\')
                    .count();
                if backslash_run % 2 == 0 {
                    in_double = !in_double;
                }
            }
            '#' if !in_single && !in_double => return &line[..chars[idx].len_utf8() * idx],
            _ => {}
        }
    }
    line
}

fn active_yaml_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(strip_yaml_comment)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn has_active_line(lines: &[String], pattern: &str) -> bool {
    lines.iter().any(|line| line.contains(pattern))
}

fn forbids_active_line(lines: &[String], pattern: &str) -> bool {
    lines.iter().any(|line| line.contains(pattern))
}

fn check_droid_action_refs(violations: &mut Vec<String>, path_label: &str, text: &str) {
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        let after_comment = strip_yaml_comment(trimmed).trim();
        let after_uses = after_comment
            .strip_prefix("- uses: ")
            .or_else(|| after_comment.strip_prefix("uses: "));
        if let Some(after_uses) = after_uses
            && let Some(at_pos) = after_uses.find('@')
        {
            let action = &after_uses[..at_pos];
            let ref_part = after_uses[at_pos + 1..]
                .split_whitespace()
                .next()
                .unwrap_or("");
            if !(ref_part.len() == 40 && ref_part.chars().all(|c| c.is_ascii_hexdigit())) {
                violations.push(format!(
                    "{path_label}:{} action ref must use immutable commit SHA: {action}@{ref_part}",
                    line_number + 1
                ));
            }
        }
    }
}

fn check_droid_common(violations: &mut Vec<String>, path_label: &str, text: &str) {
    let lines = active_yaml_lines(text);

    if !has_active_line(&lines, "head.repo.full_name == github.repository") {
        violations.push(format!(
            "{path_label}: same-repo guard (head.repo.full_name == github.repository) is required"
        ));
    }

    if !has_active_line(&lines, "review_model: \"custom:MiniMax-M2.7-0\"") {
        violations.push(format!(
            "{path_label}: review_model must be custom:MiniMax-M2.7-0"
        ));
    }

    if !has_active_line(&lines, "security_model: \"custom:MiniMax-M2.7-0\"") {
        violations.push(format!(
            "{path_label}: security_model must be custom:MiniMax-M2.7-0"
        ));
    }

    if !has_active_line(&lines, "$HOME/.factory/settings.local.json") {
        violations.push(format!(
            "{path_label}: must write $HOME/.factory/settings.local.json"
        ));
    }

    if !has_active_line(&lines, "${MINIMAX_API_KEY}") {
        violations.push(format!(
            "{path_label}: must keep ${{MINIMAX_API_KEY}} literal in settings.local.json"
        ));
    }

    if forbids_active_line(&lines, "settings:") {
        violations.push(format!(
            "{path_label}: must not use the Droid Action settings: input for BYOK"
        ));
    }

    if forbids_active_line(&lines, "ANTHROPIC_AUTH_TOKEN")
        || forbids_active_line(&lines, "ANTHROPIC_BASE_URL")
    {
        violations.push(format!(
            "{path_label}: must not set ANTHROPIC_AUTH_TOKEN or ANTHROPIC_BASE_URL"
        ));
    }

    let lower_lines: Vec<String> = lines.iter().map(|l| l.to_ascii_lowercase()).collect();
    if has_active_line(&lower_lines, "show_full_output: true") {
        violations.push(format!("{path_label}: must not enable show_full_output"));
    }

    check_droid_action_refs(violations, path_label, text);
}

fn check_droid_review_config_impl() -> Result<(), String> {
    let mut violations = Vec::new();

    let droid_review_path = ".github/workflows/droid-review.yml";
    let droid_path = ".github/workflows/droid.yml";

    if let Ok(text) = read_text_lossy(Path::new(droid_review_path)) {
        let lines = active_yaml_lines(&text);

        if !has_active_line(&lines, "opened")
            || !has_active_line(&lines, "synchronize")
            || !has_active_line(&lines, "ready_for_review")
            || !has_active_line(&lines, "reopened")
        {
            violations.push(format!(
                "{droid_review_path}: pull_request types must include opened, synchronize, ready_for_review, reopened"
            ));
        }

        if lines
            .iter()
            .any(|line| line.to_ascii_lowercase().contains("draft"))
            && lines
                .iter()
                .any(|line| line.contains("if:") && line.to_ascii_lowercase().contains("draft"))
        {
            violations.push(format!(
                "{droid_review_path}: must not filter out draft PRs"
            ));
        }

        if !has_active_line(&lines, "cancel-in-progress: false") {
            violations.push(format!(
                "{droid_review_path}: concurrency cancel-in-progress must be false"
            ));
        }

        if !has_active_line(
            &lines,
            "droid-review-${{ github.repository }}-${{ github.event.pull_request.number }}",
        ) {
            violations.push(format!(
                "{droid_review_path}: concurrency group must be per repository and PR number"
            ));
        }

        if !has_active_line(&lines, "automatic_review: true") {
            violations.push(format!(
                "{droid_review_path}: automatic_review must be true"
            ));
        }

        if !has_active_line(&lines, "automatic_security_review: true") {
            violations.push(format!(
                "{droid_review_path}: automatic_security_review must be true"
            ));
        }

        if !has_active_line(&lines, "review_depth: shallow") {
            violations.push(format!(
                "{droid_review_path}: review_depth must be shallow unless intentionally changed"
            ));
        }

        if !has_active_line(&lines, "MINIMAX_API_KEY: ${{ secrets.MINIMAX_API_KEY }}") {
            violations.push(format!(
                "{droid_review_path}: MINIMAX_API_KEY must be job-level env"
            ));
        }

        check_droid_common(&mut violations, droid_review_path, &text);
    } else {
        violations.push(format!("{droid_review_path}: file not found or unreadable"));
    }

    if let Ok(text) = read_text_lossy(Path::new(droid_path)) {
        let lines = active_yaml_lines(&text);

        if !has_active_line(&lines, "OWNER")
            || !has_active_line(&lines, "MEMBER")
            || !has_active_line(&lines, "COLLABORATOR")
        {
            violations.push(format!(
                "{droid_path}: trusted actor guard (OWNER, MEMBER, COLLABORATOR) is required"
            ));
        }

        check_droid_common(&mut violations, droid_path, &text);
    } else {
        violations.push(format!("{droid_path}: file not found or unreadable"));
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "droid-review-config.md",
            check: "check-droid-review-config",
            why_it_matters: "Droid review workflows handle repository secrets and automated review output; invariant drift can expose secrets, break BYOK model selection, or degrade review quality.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Restore the required invariant in the workflow YAML.",
                "If the invariant is intentionally changed, update docs/agent-context/review-invariants.md and add an xtask exception only after repo review.",
            ],
            rerun_command: "cargo xtask check-droid-review-config",
            exception_template: None,
        },
        &violations,
    )
}

#[cfg(test)]
mod tests {
    use super::XtaskCommand;
    use super::dispatch;
    use super::run::{
        TimedOutput, capture_output, capture_output_with_timeout, run, run_output,
        run_output_optional, run_output_owned,
    };
    use super::{
        BadgeArtifactJob, BadgeNativeSlot, CampaignManifest, Capability, ChangedPath, CheckReport,
        CheckStatus, CheckViolation, CiFullEvidenceGate, DogfoodRun, FixKind, LocalContextAllow,
        MarkdownLink, ReceiptRecord, RepoExposureLatencyReport, RepoExposureLatencyRun,
        RepoExposureLatencyTrace, ReportIndexCampaign, ReportIndexEntry, SarifPolicyMode,
        SarifPolicyResult, SarifPolicyThreshold, StaticLanguageAllowEntry, StaticLanguageMatcher,
        TestOracleClass, badge_artifact_command_args, badge_artifact_jobs,
        badge_artifact_native_slot, badge_artifacts_summary_markdown, build_lsp_cockpit_report,
        build_repo_exposure_latency_report, build_targeted_test_outcome_report,
        check_allow_attributes, check_droid_review_config, check_executable_files,
        check_file_policy, check_local_context, check_network_policy, check_no_panic_family,
        check_process_policy, check_static_language, check_workflows, ci_full_evidence_gates,
        collect_panic_findings, collect_semantic_panic_findings, critic_findings,
        dogfood_class_counts, dogfood_report_json, dogfood_report_markdown,
        extract_json_object_usize_map, extract_json_string, extract_json_warnings,
        extract_workflow_run_blocks, first_line_difference, forbidden_panic_patterns, glob_matches,
        golden_changes_without_blessing, golden_drift_semantics, guarded_allow_attribute_lints,
        guarded_allow_attributes_in_text, install_hooks_in, is_bdd_test_name,
        is_dependency_surface_candidate, is_evidence_path, is_generated_candidate,
        is_known_campaign_command, is_policy_path, is_production_path, is_receipt_status,
        is_ripr_managed_hook, is_snake_case_id, is_spec_id, json_escape, json_number_after,
        json_string_values_for_key, known_commands, known_xtask_command,
        local_context_line_findings, local_markdown_target, lsp_cockpit_report,
        lsp_cockpit_report_json, lsp_cockpit_report_markdown, markdown_links_in_text,
        mutation_calibration_report_json, mutation_calibration_report_markdown,
        next_checkpoints_from_capabilities, normalize_fixture_human_output,
        normalize_fixture_json_output, normalize_golden_text, panic_family_from_pattern,
        parse_campaign_manifest, parse_inline_array, parse_mutation_calibration_args,
        parse_mutation_outcomes_json, parse_no_panic_allowlist_toml,
        parse_no_panic_allowlist_toml_v2, parse_reason, parse_repo_exposure_static_seams,
        parse_sarif_policy_args, parse_sarif_policy_results, parse_static_language_allowlist,
        parse_string_value, parse_targeted_test_outcome_args, pr_shape_warnings,
        precommit_report_body, public_contract_rows, read_mutation_input_json, receipt_json,
        receipt_specs, receipt_status_from_reports, repo_badge_artifact_command_args,
        repo_badge_artifact_jobs, repo_badge_artifacts_summary_markdown,
        repo_exposure_latency_json, repo_exposure_latency_markdown, repo_exposure_latency_run,
        repo_exposure_latency_run_from_output, repo_exposure_latency_status,
        repo_exposure_latency_trace, repo_seam_inventory_command_args_for_root,
        report_index_markdown, report_index_missing_expected, report_status_from_text,
        ripr_command_literals_in_text, ripr_debug_binary, ripr_pre_commit_hook,
        run_ci_full_evidence_gates, sarif_policy_report_json, sarif_policy_report_markdown,
        semantic_selector_matches, should_scan_static_language_path, should_skip_path,
        sorted_allowlist_content, spec_id_from_path, static_language_allowlist_covers,
        status_for_report, suspicious_runtime_file_names, targeted_test_outcome,
        targeted_test_outcome_report_json, targeted_test_outcome_report_markdown,
        test_efficiency_entry, test_efficiency_report_json, test_efficiency_report_markdown,
        test_oracle_report_json, test_oracle_report_markdown, test_oracle_tests_in_text,
        unknown_command_message, validate_local_context_allowlist, windows_absolute_path_tokens,
        workflow_runtime_violations, write_repo_exposure_latency_report,
    };
    use super::{
        DeclaredIntent, LocalContextFinding,
        MUTATION_CALIBRATION_STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT, TestEfficiencyEntry,
        TestEfficiencyValue, TestIntentDeclaration, TestIntentKind, TestIntentReportSummary,
        apply_duplicate_discriminator_groups, apply_test_intent_to_entries,
        build_mutation_calibration_report, parse_test_intent_manifest, test_efficiency_metrics,
    };
    use super::{MutationOutcomeRecord, StaticSeamRecord};
    use super::{PanicAllowEntryVersioned, PanicFamilySelector, SemanticPanicFinding};
    use super::{SarifMissingBaseline, build_sarif_policy_report};
    use super::{
        active_yaml_lines, check_droid_action_refs, check_droid_common, forbids_active_line,
        has_active_line, strip_yaml_comment,
    };
    use serde_json::Value;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    static CWD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-xtask-{name}-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    fn with_temp_cwd<T>(name: &str, f: impl FnOnce(&Path) -> T) -> T {
        let lock = CWD_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let old = std::env::current_dir().unwrap();
        let root = temp_dir(name);
        std::env::set_current_dir(&root).unwrap();

        let out = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&root))) {
            Ok(result) => result,
            Err(panic_payload) => {
                let _ = std::env::set_current_dir(&old);
                drop(lock);
                let _ = fs::remove_dir_all(&root);
                std::panic::resume_unwind(panic_payload);
            }
        };

        std::env::set_current_dir(old).unwrap();
        drop(lock);
        let _ = fs::remove_dir_all(&root);
        out
    }

    fn with_repo_cwd<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let mutex = CWD_LOCK.get_or_init(|| Mutex::new(()));
        let guard = mutex
            .lock()
            .map_err(|err| format!("failed to lock cwd mutex: {err}"))?;
        let old = std::env::current_dir()
            .map_err(|err| format!("failed to capture current dir: {err}"))?;
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let Some(repo_root) = manifest_dir.parent() else {
            drop(guard);
            return Err(format!(
                "failed to resolve repo root from {}",
                manifest_dir.display()
            ));
        };
        std::env::set_current_dir(repo_root)
            .map_err(|err| format!("failed to set repo cwd: {err}"))?;
        let result = f();
        let restore =
            std::env::set_current_dir(&old).map_err(|err| format!("failed to restore cwd: {err}"));
        drop(guard);
        restore?;
        result
    }

    fn targeted_static_seam(id: &str, grip_class: &str) -> StaticSeamRecord {
        StaticSeamRecord {
            seam_id: id.to_string(),
            seam_kind: "predicate_boundary".to_string(),
            file: "src/pricing.rs".to_string(),
            line: 42,
            seam_grip_class: grip_class.to_string(),
            oracle_kind: "exact_value".to_string(),
            oracle_strength: "weak".to_string(),
            observed_values: vec!["50".to_string()],
            missing_discriminators: Vec::new(),
        }
    }

    fn mutation_record(
        mutant_id: &str,
        seam_id: Option<&str>,
        runtime_outcome: &str,
    ) -> MutationOutcomeRecord {
        mutation_record_at(mutant_id, seam_id, "src/pricing.rs", 42, runtime_outcome)
    }

    fn mutation_record_at(
        mutant_id: &str,
        seam_id: Option<&str>,
        file: &str,
        line: usize,
        runtime_outcome: &str,
    ) -> MutationOutcomeRecord {
        MutationOutcomeRecord {
            mutant_id: Some(mutant_id.to_string()),
            seam_id: seam_id.map(str::to_string),
            file: Some(file.to_string()),
            line: Some(line),
            mutation_operator: "replace >= with >".to_string(),
            runtime_outcome: runtime_outcome.to_string(),
            duration: None,
            test_command: Some("cargo test pricing".to_string()),
        }
    }

    // ============================================================================
    // Panic allowlist TOML tests
    // ============================================================================

    #[test]
    fn parse_no_panic_allowlist_toml_parses_valid_entries() {
        with_temp_cwd("parse_valid", |root| {
            let toml_content = r#"schema_version = "0.1"

[[allow]]
path = "src/lib.rs"
line = 42
column = 10
family = "unwrap"
classification = "test_only"
explanation = "Test helper"
"#;
            write(&root.join("allowlist.toml"), toml_content);

            let result =
                parse_no_panic_allowlist_toml(root.join("allowlist.toml").to_str().unwrap());
            assert!(result.is_ok());
            let entries = result.unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].path, "src/lib.rs");
            assert_eq!(entries[0].line, 42);
            assert_eq!(entries[0].column, Some(10));
            assert_eq!(entries[0].family, "unwrap");
            assert_eq!(entries[0].classification, Some("test_only".to_string()));
            assert_eq!(entries[0].explanation, "Test helper");
        });
    }

    #[test]
    fn parse_no_panic_allowlist_toml_requires_path() {
        with_temp_cwd("missing_path", |root| {
            let toml_content = r#"schema_version = "0.1"

[[allow]]
line = 42
family = "unwrap"
explanation = "Missing path"
"#;
            write(&root.join("allowlist.toml"), toml_content);

            let result =
                parse_no_panic_allowlist_toml(root.join("allowlist.toml").to_str().unwrap());
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("missing required field: path"));
        });
    }

    #[test]
    fn parse_no_panic_allowlist_toml_requires_line() {
        with_temp_cwd("missing_line", |root| {
            let toml_content = r#"schema_version = "0.1"

[[allow]]
path = "src/lib.rs"
family = "unwrap"
explanation = "Missing line"
"#;
            write(&root.join("allowlist.toml"), toml_content);

            let result =
                parse_no_panic_allowlist_toml(root.join("allowlist.toml").to_str().unwrap());
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("missing required field: line"));
        });
    }

    #[test]
    fn parse_no_panic_allowlist_toml_requires_family() {
        with_temp_cwd("missing_family", |root| {
            let toml_content = r#"schema_version = "0.1"

[[allow]]
path = "src/lib.rs"
line = 42
explanation = "Missing family"
"#;
            write(&root.join("allowlist.toml"), toml_content);

            let result =
                parse_no_panic_allowlist_toml(root.join("allowlist.toml").to_str().unwrap());
            assert!(result.is_err());
            assert!(
                result
                    .unwrap_err()
                    .contains("missing required field: family")
            );
        });
    }

    #[test]
    fn parse_no_panic_allowlist_toml_requires_explanation() {
        with_temp_cwd("missing_explanation", |root| {
            let toml_content = r#"schema_version = "0.1"

[[allow]]
path = "src/lib.rs"
line = 42
family = "unwrap"
"#;
            write(&root.join("allowlist.toml"), toml_content);

            let result =
                parse_no_panic_allowlist_toml(root.join("allowlist.toml").to_str().unwrap());
            assert!(result.is_err());
            assert!(
                result
                    .unwrap_err()
                    .contains("missing required field: explanation")
            );
        });
    }

    #[test]
    fn parse_no_panic_allowlist_toml_rejects_unknown_fields() {
        with_temp_cwd("unknown_field", |root| {
            let toml_content = r#"schema_version = "0.1"

[[allow]]
path = "src/lib.rs"
line = 42
family = "unwrap"
explanation = "Test"
unknown_field = "value"
"#;
            write(&root.join("allowlist.toml"), toml_content);

            let result =
                parse_no_panic_allowlist_toml(root.join("allowlist.toml").to_str().unwrap());
            assert!(result.is_err());
            assert!(
                result
                    .unwrap_err()
                    .contains("unknown field 'unknown_field'")
            );
        });
    }

    #[test]
    fn parse_no_panic_allowlist_toml_rejects_duplicate_locations() {
        with_temp_cwd("duplicate", |root| {
            let toml_content = r#"schema_version = "0.1"

[[allow]]
path = "src/lib.rs"
line = 42
column = 10
family = "unwrap"
explanation = "First entry"

[[allow]]
path = "src/lib.rs"
line = 42
column = 10
family = "unwrap"
explanation = "Duplicate entry"
"#;
            write(&root.join("allowlist.toml"), toml_content);

            let result =
                parse_no_panic_allowlist_toml(root.join("allowlist.toml").to_str().unwrap());
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("duplicate allowlist entry"));
        });
    }

    #[test]
    fn panic_family_from_pattern_matches_all_families() {
        assert_eq!(panic_family_from_pattern("unwrap("), "unwrap");
        assert_eq!(panic_family_from_pattern("expect("), "expect");
        assert_eq!(panic_family_from_pattern("panic!"), "panic_macro");
        assert_eq!(panic_family_from_pattern("todo!"), "todo");
        assert_eq!(panic_family_from_pattern("unimplemented!"), "unimplemented");
        assert_eq!(panic_family_from_pattern("unreachable!"), "unreachable");
    }

    #[test]
    fn collect_panic_findings_finds_exact_locations() {
        with_temp_cwd("collect_findings", |root| {
            let rs_file = root.join("lib.rs");
            write(
                &rs_file,
                "fn test() {\n    let x = some_fn().unwrap();\n    let y = other().expect(\"msg\");\n}\n",
            );

            let patterns = vec!["unwrap(".to_string(), "expect(".to_string()];
            let findings = collect_panic_findings(root, &patterns).unwrap();

            // Should find unwrap( on line 2 and expect( on line 3
            assert!(findings.iter().any(|f| f.line == 2 && f.family == "unwrap"));
            assert!(findings.iter().any(|f| f.line == 3 && f.family == "expect"));
        });
    }

    #[test]
    fn parse_string_value_preserves_hashes_and_unescapes_quotes_inside_values() -> Result<(), String>
    {
        let parsed = parse_string_value(
            "\"fs::write( root.join(\\\"src/lib.rs\\\"), r#\\\" body \\\"#, )\" # trailing comment",
            "allowlist.toml",
            1,
        )?;
        assert_eq!(
            parsed,
            "fs::write( root.join(\"src/lib.rs\"), r#\" body \"#, )"
        );
        Ok(())
    }

    // ============================================================================
    // v0.2 semantic selector tests
    // ============================================================================

    #[test]
    fn v0_2_method_call_selector_allows_line_movement() -> Result<(), String> {
        let selector = PanicFamilySelector {
            kind: "method_call".to_string(),
            container: Some("my_test_fn".to_string()),
            callee: Some("unwrap".to_string()),
            receiver_fingerprint: None,
            text_contains: None,
        };
        let finding_at_10 = SemanticPanicFinding {
            path: "src/lib.rs".to_string(),
            family: "unwrap".to_string(),
            kind: "method_call".to_string(),
            line: 10,
            column: Some(5),
            container: Some("my_test_fn".to_string()),
            callee: Some("unwrap".to_string()),
            receiver_fingerprint: None,
            snippet_fingerprint: "x.unwrap()".to_string(),
        };
        let finding_at_25 = SemanticPanicFinding {
            path: "src/lib.rs".to_string(),
            family: "unwrap".to_string(),
            kind: "method_call".to_string(),
            line: 25,
            column: Some(12),
            container: Some("my_test_fn".to_string()),
            callee: Some("unwrap".to_string()),
            receiver_fingerprint: None,
            snippet_fingerprint: "y.unwrap()".to_string(),
        };
        if !semantic_selector_matches(&selector, &finding_at_10) {
            return Err("selector should match finding at line 10".to_string());
        }
        if !semantic_selector_matches(&selector, &finding_at_25) {
            return Err(
                "selector should match finding at line 25 (line movement allowed)".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn v0_2_receiver_fingerprint_disambiguates_same_container_calls() -> Result<(), String> {
        let selector = PanicFamilySelector {
            kind: "method_call".to_string(),
            container: Some("my_test_fn".to_string()),
            callee: Some("unwrap".to_string()),
            receiver_fingerprint: Some("left_side()".to_string()),
            text_contains: None,
        };
        let matching = SemanticPanicFinding {
            path: "src/lib.rs".to_string(),
            family: "unwrap".to_string(),
            kind: "method_call".to_string(),
            line: 10,
            column: Some(5),
            container: Some("my_test_fn".to_string()),
            callee: Some("unwrap".to_string()),
            receiver_fingerprint: Some("left_side()".to_string()),
            snippet_fingerprint: "left_side().unwrap()".to_string(),
        };
        let different_receiver = SemanticPanicFinding {
            receiver_fingerprint: Some("right_side()".to_string()),
            snippet_fingerprint: "right_side().unwrap()".to_string(),
            ..matching.clone()
        };
        if !semantic_selector_matches(&selector, &matching) {
            return Err("receiver fingerprint should match identical receiver".to_string());
        }
        if semantic_selector_matches(&selector, &different_receiver) {
            return Err("receiver fingerprint should reject a different receiver".to_string());
        }
        Ok(())
    }

    #[test]
    fn v0_2_macro_call_selector_matches_exact_macro() -> Result<(), String> {
        let selector = PanicFamilySelector {
            kind: "macro_call".to_string(),
            container: Some("test_fn".to_string()),
            callee: Some("panic!".to_string()),
            receiver_fingerprint: None,
            text_contains: None,
        };
        let finding = SemanticPanicFinding {
            path: "src/lib.rs".to_string(),
            family: "panic_macro".to_string(),
            kind: "macro_call".to_string(),
            line: 5,
            column: Some(9),
            container: Some("test_fn".to_string()),
            callee: Some("panic!".to_string()),
            receiver_fingerprint: None,
            snippet_fingerprint: "panic!(\"msg\")".to_string(),
        };
        if !semantic_selector_matches(&selector, &finding) {
            return Err("macro_call selector should match panic! finding".to_string());
        }
        let wrong_callee = SemanticPanicFinding {
            callee: Some("todo!".to_string()),
            family: "todo".to_string(),
            snippet_fingerprint: "todo!(\"msg\")".to_string(),
            ..finding.clone()
        };
        if semantic_selector_matches(&selector, &wrong_callee) {
            return Err("macro_call selector should not match different callee".to_string());
        }
        Ok(())
    }

    #[test]
    fn v0_2_call_selector_matches_exact_free_function() -> Result<(), String> {
        let selector = PanicFamilySelector {
            kind: "call".to_string(),
            container: Some("helper".to_string()),
            callee: Some("panic".to_string()),
            receiver_fingerprint: None,
            text_contains: None,
        };
        let finding = SemanticPanicFinding {
            path: "src/lib.rs".to_string(),
            family: "panic_macro".to_string(),
            kind: "call".to_string(),
            line: 3,
            column: Some(5),
            container: Some("helper".to_string()),
            callee: Some("panic".to_string()),
            receiver_fingerprint: None,
            snippet_fingerprint: "panic(\"msg\")".to_string(),
        };
        if !semantic_selector_matches(&selector, &finding) {
            return Err("call selector should match finding".to_string());
        }
        let wrong_kind = SemanticPanicFinding {
            kind: "method_call".to_string(),
            ..finding.clone()
        };
        if semantic_selector_matches(&selector, &wrong_kind) {
            return Err("call selector should not match method_call finding".to_string());
        }
        Ok(())
    }

    #[test]
    fn v0_2_string_literal_selector_requires_text_contains() -> Result<(), String> {
        let selector_with_text = PanicFamilySelector {
            kind: "string_literal".to_string(),
            container: None,
            callee: None,
            receiver_fingerprint: None,
            text_contains: Some("error".to_string()),
        };
        let finding = SemanticPanicFinding {
            path: "src/lib.rs".to_string(),
            family: "panic_macro".to_string(),
            kind: "string_literal".to_string(),
            line: 10,
            column: Some(5),
            container: None,
            callee: None,
            receiver_fingerprint: None,
            snippet_fingerprint: "panic!(\"error happened\")".to_string(),
        };
        if !semantic_selector_matches(&selector_with_text, &finding) {
            return Err("string_literal selector with text_contains should match".to_string());
        }
        let selector_no_text = PanicFamilySelector {
            kind: "string_literal".to_string(),
            container: None,
            callee: None,
            receiver_fingerprint: None,
            text_contains: None,
        };
        if semantic_selector_matches(&selector_no_text, &finding) {
            return Err(
                "string_literal selector without text_contains should not match".to_string(),
            );
        }
        let finding_no_match = SemanticPanicFinding {
            snippet_fingerprint: "panic!(\"other\")".to_string(),
            ..finding.clone()
        };
        if semantic_selector_matches(&selector_with_text, &finding_no_match) {
            return Err("string_literal selector should not match when text_contains is absent from snippet".to_string());
        }
        Ok(())
    }

    #[test]
    fn v0_2_selector_kind_mismatch_rejects() -> Result<(), String> {
        let selector = PanicFamilySelector {
            kind: "method_call".to_string(),
            container: None,
            callee: Some("unwrap".to_string()),
            receiver_fingerprint: None,
            text_contains: None,
        };
        let macro_finding = SemanticPanicFinding {
            path: "src/lib.rs".to_string(),
            family: "panic_macro".to_string(),
            kind: "macro_call".to_string(),
            line: 5,
            column: Some(9),
            container: None,
            callee: Some("panic!".to_string()),
            receiver_fingerprint: None,
            snippet_fingerprint: "panic!(\"msg\")".to_string(),
        };
        if semantic_selector_matches(&selector, &macro_finding) {
            return Err("method_call selector should reject macro_call finding".to_string());
        }
        let invalid_selector = PanicFamilySelector {
            kind: "invalid".to_string(),
            container: None,
            callee: None,
            receiver_fingerprint: None,
            text_contains: None,
        };
        if semantic_selector_matches(&invalid_selector, &macro_finding) {
            return Err("invalid selector kind should reject all findings".to_string());
        }
        Ok(())
    }

    #[test]
    fn v0_2_rejects_unknown_selector_kind() -> Result<(), String> {
        with_temp_cwd("reject_unknown_kind", |root| {
            let toml_content = r#"schema_version = "0.2"

[[allow]]
path = "src/lib.rs"
family = "unwrap"
classification = "test_only"
explanation = "Bad kind"

[allow.selector]
kind = "foo"
callee = "unwrap"
"#;
            write(&root.join("allowlist.toml"), toml_content);
            let toml_path = root
                .join("allowlist.toml")
                .to_str()
                .ok_or("non-UTF-8 path")?
                .to_string();
            let result = parse_no_panic_allowlist_toml_v2(&toml_path);
            let err = result
                .err()
                .ok_or("expected parse error for unknown selector kind")?;
            if !err.contains("invalid selector kind 'foo'") {
                return Err(format!("unexpected error message: {err}"));
            }
            if !err.contains("method_call, macro_call, call, string_literal") {
                return Err(format!("error should list supported kinds, got: {err}"));
            }
            Ok(())
        })
    }

    #[test]
    fn v0_2_call_selector_handles_associated_function_form() -> Result<(), String> {
        with_temp_cwd("associated_fn", |root| {
            // Option::unwrap(x) is a CallExpr, callee should be just "unwrap"
            let code = "fn demo() { Option::unwrap(some_opt) }\n";
            write(&root.join("lib.rs"), code);
            let patterns = vec!["unwrap(".to_string()];
            let findings = collect_semantic_panic_findings(root, &patterns)
                .map_err(|e| format!("collect failed: {e}"))?;
            if findings.is_empty() {
                return Err("expected to find Option::unwrap call".to_string());
            }
            let f = &findings[0];
            if f.kind != "call" {
                return Err(format!(
                    "Option::unwrap(x) should be kind=call, got kind={}",
                    f.kind
                ));
            }
            if f.callee.as_deref() != Some("unwrap") {
                return Err(format!("callee should be 'unwrap', got {:?}", f.callee));
            }
            if f.family != "unwrap" {
                return Err(format!("family should be 'unwrap', got {}", f.family));
            }
            Ok(())
        })
    }

    #[test]
    fn v0_2_call_selector_does_not_match_substring_helper_name() -> Result<(), String> {
        with_temp_cwd("no_substring_match", |root| {
            // A function named `panic_family_from_pattern` should NOT match
            // the panic-family patterns since its base callee name is
            // `panic_family_from_pattern`, not `panic`.
            let code = "fn demo() { panic_family_from_pattern(\"x\") }\n";
            write(&root.join("lib.rs"), code);
            let patterns = vec!["panic!".to_string()];
            let findings = collect_semantic_panic_findings(root, &patterns)
                .map_err(|e| format!("collect failed: {e}"))?;
            if !findings.is_empty() {
                return Err(format!(
                    "panic_family_from_pattern should not match as a panic-family call, got {} findings",
                    findings.len()
                ));
            }
            Ok(())
        })
    }

    #[test]
    fn v0_2_string_literal_still_requires_text_contains() -> Result<(), String> {
        with_temp_cwd("string_literal_text_contains", |root| {
            let toml_content = r#"schema_version = "0.2"

[[allow]]
path = "src/lib.rs"
family = "panic_macro"
classification = "test_only"
explanation = "Needs text_contains"

[allow.selector]
kind = "string_literal"
"#;
            write(&root.join("allowlist.toml"), toml_content);
            let toml_path = root
                .join("allowlist.toml")
                .to_str()
                .ok_or("non-UTF-8 path")?
                .to_string();
            let result = parse_no_panic_allowlist_toml_v2(&toml_path);
            let err = result
                .err()
                .ok_or("expected parse error for string_literal without text_contains")?;
            if !err.contains("string_literal selector requires text_contains") {
                return Err(format!("unexpected error message: {err}"));
            }
            Ok(())
        })
    }

    #[test]
    fn v0_2_kind_mismatch_reports_actionable_error() -> Result<(), String> {
        // A method_call selector must not match a call-type finding
        let selector = PanicFamilySelector {
            kind: "method_call".to_string(),
            container: Some("demo".to_string()),
            callee: Some("unwrap".to_string()),
            receiver_fingerprint: None,
            text_contains: None,
        };
        // Option::unwrap(x) produces kind=call, not method_call
        let call_finding = SemanticPanicFinding {
            path: "src/lib.rs".to_string(),
            family: "unwrap".to_string(),
            kind: "call".to_string(),
            line: 3,
            column: Some(5),
            container: Some("demo".to_string()),
            callee: Some("unwrap".to_string()),
            receiver_fingerprint: None,
            snippet_fingerprint: "Option::unwrap(x)".to_string(),
        };
        if semantic_selector_matches(&selector, &call_finding) {
            return Err(
                "method_call selector must not match a call-type finding (Option::unwrap)"
                    .to_string(),
            );
        }
        // Conversely, a method_call finding must not match a call selector
        let call_selector = PanicFamilySelector {
            kind: "call".to_string(),
            container: Some("demo".to_string()),
            callee: Some("unwrap".to_string()),
            receiver_fingerprint: None,
            text_contains: None,
        };
        let method_finding = SemanticPanicFinding {
            path: "src/lib.rs".to_string(),
            family: "unwrap".to_string(),
            kind: "method_call".to_string(),
            line: 5,
            column: Some(8),
            container: Some("demo".to_string()),
            callee: Some("unwrap".to_string()),
            receiver_fingerprint: None,
            snippet_fingerprint: "x.unwrap()".to_string(),
        };
        if semantic_selector_matches(&call_selector, &method_finding) {
            return Err(
                "call selector must not match a method_call finding (.unwrap())".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn v0_2_last_seen_drift_is_advisory_not_failure() -> Result<(), String> {
        with_temp_cwd("last_seen_drift", |root| {
            let toml_content = r#"schema_version = "0.2"

[[allow]]
path = "src/lib.rs"
family = "unwrap"
classification = "test_only"
explanation = "Test helper"

[allow.selector]
kind = "method_call"
container = "my_fn"
callee = "unwrap"

[allow.last_seen]
line = 10
column = 5
"#;
            write(&root.join("allowlist.toml"), toml_content);
            let entries =
                parse_no_panic_allowlist_toml_v2(root.join("allowlist.toml").to_str().unwrap())
                    .map_err(|e| format!("parse failed: {e}"))?;

            let entry_count = entries.len();
            if entry_count != 1 {
                return Err(format!("expected 1 entry, got {entry_count}"));
            }

            match &entries[0] {
                PanicAllowEntryVersioned::V2(v2) => {
                    let selector = v2.selector.as_ref().ok_or("missing selector")?;
                    let finding = SemanticPanicFinding {
                        path: "src/lib.rs".to_string(),
                        family: "unwrap".to_string(),
                        kind: "method_call".to_string(),
                        line: 20,
                        column: Some(8),
                        container: Some("my_fn".to_string()),
                        callee: Some("unwrap".to_string()),
                        receiver_fingerprint: None,
                        snippet_fingerprint: "x.unwrap()".to_string(),
                    };
                    if !semantic_selector_matches(selector, &finding) {
                        return Err("selector should match finding at different line".to_string());
                    }
                    let ls = v2.last_seen.as_ref().ok_or("missing last_seen")?;
                    if ls.line != 10 {
                        return Err(format!("expected last_seen.line 10, got {}", ls.line));
                    }
                }
                _ => return Err("expected V2 entry".to_string()),
            }
            Ok(())
        })
    }

    #[test]
    fn v0_1_entries_still_match_by_line_and_column() -> Result<(), String> {
        with_temp_cwd("v01_in_v02_file", |root| {
            let toml_content = r#"schema_version = "0.2"

[[allow]]
path = "src/lib.rs"
line = 42
column = 10
family = "unwrap"
explanation = "Legacy entry."
"#;
            write(&root.join("allowlist.toml"), toml_content);
            let entries =
                parse_no_panic_allowlist_toml_v2(root.join("allowlist.toml").to_str().unwrap())
                    .map_err(|e| format!("parse failed: {e}"))?;

            match &entries[0] {
                PanicAllowEntryVersioned::V1(v1) => {
                    if v1.line != 42 || v1.column != Some(10) || v1.family != "unwrap" {
                        return Err(format!(
                            "v0.1 entry mismatch: line={} col={:?} family={}",
                            v1.line, v1.column, v1.family
                        ));
                    }
                }
                _ => return Err("expected V1 entry".to_string()),
            }
            Ok(())
        })
    }

    #[test]
    fn v0_2_missing_selector_and_missing_coordinates_fails_clearly() -> Result<(), String> {
        with_temp_cwd("missing_both", |root| {
            let toml_content = r#"schema_version = "0.2"

[[allow]]
path = "src/lib.rs"
family = "unwrap"
explanation = "Entry with neither selector nor line."
"#;
            write(&root.join("allowlist.toml"), toml_content);
            let result =
                parse_no_panic_allowlist_toml_v2(root.join("allowlist.toml").to_str().unwrap());
            let err = result
                .err()
                .ok_or("expected parse error for entry with neither selector nor line")?;
            if !err.contains("either a [allow.selector] or line number") {
                return Err(format!("unexpected error message: {err}"));
            }
            Ok(())
        })
    }

    #[test]
    fn semantic_extractor_avoids_substring_false_positive_function_names() -> Result<(), String> {
        with_temp_cwd("substring_fp", |root| {
            // Code that contains "panic" in function/variable names but not as actual panic calls
            let code = r#"
fn panic_family_from_pattern() -> &'static str {
    "panic!"
}

fn has_unwrap_in_name() -> bool {
    true
}
"#;
            write(&root.join("lib.rs"), code);
            let patterns = forbidden_panic_patterns();
            let findings = collect_semantic_panic_findings(root, &patterns)
                .map_err(|e| format!("collect failed: {e}"))?;
            // Should find NO panic-family calls since these are just function names and strings
            if !findings.is_empty() {
                let lines: Vec<String> = findings
                    .iter()
                    .map(|f| {
                        format!(
                            "{}:{}:{} kind={}",
                            f.path,
                            f.line,
                            f.column.unwrap_or(0),
                            f.kind
                        )
                    })
                    .collect();
                return Err(format!("expected no findings, got: {:?}", lines));
            }
            Ok(())
        })
    }

    #[test]
    fn semantic_extractor_uses_byte_offsets_for_utf8_line_column() -> Result<(), String> {
        // Verify that line_and_column_for_node handles UTF-8 correctly
        let code = "fn test() {\n    let x = \"héllo\".unwrap();\n}\n";
        let patterns = vec!["unwrap(".to_string()];
        let root = temp_dir("utf8_offsets");
        write(&root.join("lib.rs"), code);
        let findings = collect_semantic_panic_findings(&root, &patterns)
            .map_err(|e| format!("collect failed: {e}"))?;
        let _ = fs::remove_dir_all(&root);

        if findings.is_empty() {
            return Err("expected to find unwrap call".to_string());
        }
        let f = &findings[0];
        if f.line != 2 {
            return Err(format!("expected line 2, got {}", f.line));
        }
        if f.family != "unwrap" {
            return Err(format!("expected family unwrap, got {}", f.family));
        }
        if f.kind != "method_call" {
            return Err(format!("expected kind method_call, got {}", f.kind));
        }
        Ok(())
    }

    // ============================================================================
    // Enum contract tests
    // ============================================================================

    #[test]
    fn test_oracle_class_labels_are_stable() {
        assert_eq!(TestOracleClass::Strong.as_str(), "strong");
        assert_eq!(TestOracleClass::Medium.as_str(), "medium");
        assert_eq!(TestOracleClass::Weak.as_str(), "weak");
        assert_eq!(TestOracleClass::Smoke.as_str(), "smoke");
    }

    #[test]
    fn test_oracle_class_rank_is_monotonic() {
        assert!(TestOracleClass::Strong.rank() > TestOracleClass::Medium.rank());
        assert!(TestOracleClass::Medium.rank() > TestOracleClass::Weak.rank());
        assert!(TestOracleClass::Weak.rank() > TestOracleClass::Smoke.rank());
    }

    #[test]
    fn test_intent_kind_round_trips_supported_values() {
        for value in TestIntentKind::supported() {
            let parsed = TestIntentKind::from_str(value).expect("supported intent should parse");
            assert_eq!(parsed.as_str(), *value);
        }
    }

    #[test]
    fn test_intent_kind_rejects_unknown_values() {
        assert_eq!(TestIntentKind::from_str("not_a_real_intent"), None);
        assert_eq!(TestIntentKind::from_str(""), None);
        assert_eq!(TestIntentKind::from_str("SMOKE"), None);
    }

    #[test]
    fn test_intent_kind_supported_list_has_expected_values() {
        let supported = TestIntentKind::supported();
        assert!(supported.contains(&"smoke"));
        assert!(supported.contains(&"business_case_duplicate"));
        assert!(supported.contains(&"opaque_external_oracle"));
        assert!(supported.contains(&"integration_contract"));
        assert!(supported.contains(&"performance_guard"));
        assert!(supported.contains(&"documentation_example"));
        assert_eq!(supported.len(), 6);
    }

    // ============================================================================
    // Receipt and status utility tests
    // ============================================================================

    #[test]
    fn receipt_status_vocabulary_is_locked() {
        assert!(is_receipt_status("passed"));
        assert!(is_receipt_status("warn"));
        assert!(is_receipt_status("failed"));
        assert!(is_receipt_status("missing"));

        assert!(!is_receipt_status("pass"));
        assert!(!is_receipt_status("warning"));
        assert!(!is_receipt_status("fail"));
        assert!(!is_receipt_status("unknown"));
        assert!(!is_receipt_status(""));
    }

    #[test]
    fn receipt_json_escapes_and_includes_metadata() {
        let record = ReceiptRecord {
            file: "shape.json".to_string(),
            command: "cargo xtask shape".to_string(),
            status: "passed".to_string(),
            reports: vec![
                "target/ripr/reports/shape.md".to_string(),
                "target/ripr/reports/pr-summary.md".to_string(),
            ],
        };
        let mut git = BTreeMap::new();
        git.insert("branch".to_string(), "main".to_string());
        git.insert("commit".to_string(), "abc123".to_string());

        let json = receipt_json(&record, &git);

        assert!(json.contains("\"schema_version\": \"0.1\""));
        assert!(json.contains("\"command\": \"cargo xtask shape\""));
        assert!(json.contains("\"status\": \"passed\""));
        assert!(json.contains("\"branch\": \"main\""));
        assert!(json.contains("\"commit\": \"abc123\""));
        assert!(json.contains("target/ripr/reports/shape.md"));
    }

    #[test]
    fn receipt_json_escapes_quotes_and_backslashes() {
        let record = ReceiptRecord {
            file: "report.json".to_string(),
            command: "cargo run -- check --diff \"path\\to\\file.diff\"".to_string(),
            status: "passed".to_string(),
            reports: vec![],
        };
        let git = BTreeMap::new();

        let json = receipt_json(&record, &git);

        assert!(json.contains("cargo run -- check --diff \\\"path\\\\to\\\\file.diff\\\""));
    }

    #[test]
    fn receipt_status_from_reports_detects_missing_files() {
        with_temp_cwd("receipt-status-missing", |_root| {
            let result = receipt_status_from_reports(&["missing.md".to_string()]);
            assert_eq!(result, "missing");
        });
    }

    #[test]
    fn receipt_specs_returns_expected_reports() {
        let specs = receipt_specs();
        assert!(!specs.is_empty());

        // Spot-check that we have some expected specs
        let spec_commands: Vec<&str> = specs.iter().map(|s| s.command).collect();
        assert!(spec_commands.contains(&"cargo xtask shape"));
        assert!(spec_commands.contains(&"cargo xtask check-pr"));
    }

    #[test]
    fn glob_match_supports_recursive_segments_and_star_suffixes() {
        assert!(glob_matches(
            "editors/vscode/**/*.ts",
            "editors/vscode/src/client.ts"
        ));
        assert!(glob_matches("*.md", "README.md"));
        assert!(glob_matches(
            "fixtures/**",
            "fixtures/boundary_gap/input/src/lib.rs"
        ));
        assert!(!glob_matches(
            "editors/vscode/**/*.ts",
            "docs/examples/client.ts"
        ));
    }

    fn allowlist_with_docs_markdown_globs() -> Vec<StaticLanguageAllowEntry> {
        vec![
            StaticLanguageAllowEntry::new_path("AGENTS.md", "test", "test fixture"),
            StaticLanguageAllowEntry::new_glob("docs/*.md", "test", "test fixture"),
            StaticLanguageAllowEntry::new_glob("docs/**/*.md", "test", "test fixture"),
        ]
    }

    #[test]
    fn static_language_allowlist_covers_exact_paths_and_scoped_doc_globs() {
        let allowlist = allowlist_with_docs_markdown_globs();

        assert!(static_language_allowlist_covers(&allowlist, "AGENTS.md"));
        assert!(static_language_allowlist_covers(
            &allowlist,
            "docs/BADGE_POLICY.md"
        ));
        assert!(static_language_allowlist_covers(
            &allowlist,
            "docs/specs/RIPR-SPEC-0004-test-efficiency.md"
        ));
        assert!(!static_language_allowlist_covers(
            &allowlist,
            "crates/ripr/src/lib.rs"
        ));
        assert!(!static_language_allowlist_covers(
            &allowlist,
            "fixtures/boundary_gap/input/src/lib.rs"
        ));
        assert!(!static_language_allowlist_covers(
            &allowlist,
            "docs/generated/output.json"
        ));
    }

    #[test]
    fn should_scan_static_language_path_combines_candidate_check_and_allowlist() {
        let allowlist = allowlist_with_docs_markdown_globs();

        // Non-candidate files (not in the watched extensions list) are never
        // scanned, regardless of allowlist contents.
        assert!(!should_scan_static_language_path(
            &allowlist,
            "docs/generated/output.png"
        ));
        assert!(!should_scan_static_language_path(
            &allowlist,
            "fixtures/boundary_gap/expected/output.bin"
        ));

        // Candidate files covered by an exact allowlist entry are not scanned.
        assert!(!should_scan_static_language_path(&allowlist, "AGENTS.md"));

        // Candidate files covered by the docs Markdown globs are not scanned.
        assert!(!should_scan_static_language_path(
            &allowlist,
            "docs/BADGE_POLICY.md"
        ));
        assert!(!should_scan_static_language_path(
            &allowlist,
            "docs/specs/RIPR-SPEC-0004-test-efficiency.md"
        ));

        // Non-allowlisted candidate Rust source IS scanned.
        assert!(should_scan_static_language_path(
            &allowlist,
            "crates/ripr/src/lib.rs"
        ));
        assert!(should_scan_static_language_path(
            &allowlist,
            "fixtures/boundary_gap/input/src/lib.rs"
        ));
    }

    #[test]
    fn should_skip_path_ignores_generated_editor_test_artifacts() {
        assert!(should_skip_path("editors/vscode/.vscode-test"));
        assert!(should_skip_path(
            "editors/vscode/.vscode-test/vscode-win32-x64-archive-1.119.0/resources/app/package.json"
        ));
        assert!(should_skip_path(
            "editors/vscode/node_modules/vscode/package.json"
        ));
        assert!(should_skip_path("editors/vscode/out/src/extension.js"));
        assert!(should_skip_path("editors/vscode/dist/ripr-0.3.0.vsix"));
        assert!(!should_skip_path("editors/vscode/src/config.ts"));
    }

    #[test]
    fn glob_matches_distinguishes_single_star_from_double_star_segments() {
        // docs/*.md must NOT match a nested path: a single `*` cannot cross `/`.
        assert!(!glob_matches(
            "docs/*.md",
            "docs/specs/RIPR-SPEC-0004-test-efficiency.md"
        ));

        // docs/**/*.md must match a top-level docs file because ** consumes zero segments.
        assert!(glob_matches("docs/**/*.md", "docs/BADGE_POLICY.md"));

        // docs/**/*.md must match deeply nested paths.
        assert!(glob_matches("docs/**/*.md", "docs/adr/sub/0001-x.md"));

        // Empty path against a non-empty pattern → false.
        assert!(!glob_matches("docs/*.md", ""));

        // A pattern with no wildcards behaves as exact match (case-sensitive).
        assert!(glob_matches("AGENTS.md", "AGENTS.md"));
        assert!(!glob_matches("AGENTS.md", "agents.md"));

        // A `*` segment cannot cross `/`, so it matches one path component only.
        assert!(glob_matches(
            "crates/*/src/lib.rs",
            "crates/ripr/src/lib.rs"
        ));
        assert!(!glob_matches(
            "crates/*/src/lib.rs",
            "crates/ripr/src/main.rs"
        ));
    }

    #[test]
    fn static_language_allowlist_covers_handles_empty_and_glob_only_lists() {
        // An empty allowlist covers nothing.
        let empty: Vec<StaticLanguageAllowEntry> = Vec::new();
        assert!(!static_language_allowlist_covers(&empty, "AGENTS.md"));
        assert!(!static_language_allowlist_covers(&empty, "docs/X.md"));

        // Glob-only allowlist: only paths matching the glob are covered.
        let globs_only = vec![StaticLanguageAllowEntry::new_glob(
            "docs/**/*.md",
            "test",
            "test fixture",
        )];
        assert!(static_language_allowlist_covers(
            &globs_only,
            "docs/specs/X.md"
        ));
        assert!(static_language_allowlist_covers(
            &globs_only,
            "docs/BADGE_POLICY.md"
        ));
        assert!(!static_language_allowlist_covers(&globs_only, "AGENTS.md"));
        assert!(!static_language_allowlist_covers(
            &globs_only,
            "crates/ripr/README.md"
        ));

        // Exact-only allowlist: no glob behavior applied to non-glob entries.
        let exact_only = vec![
            StaticLanguageAllowEntry::new_path("AGENTS.md", "test", "test fixture"),
            StaticLanguageAllowEntry::new_path("README.md", "test", "test fixture"),
        ];
        assert!(static_language_allowlist_covers(&exact_only, "AGENTS.md"));
        assert!(!static_language_allowlist_covers(
            &exact_only,
            "docs/AGENTS.md"
        ));
        assert!(!static_language_allowlist_covers(&exact_only, "docs/X.md"));
    }

    #[test]
    fn static_language_allowlist_parses_reasoned_entries() {
        let text = r#"
schema_version = 1

[[allow]]
path = "AGENTS.md"
owner = "maintainers"
reason = "Agent instructions define the static-language boundary."

[[allow]]
glob = "docs/**/*.md"
owner = "docs"
reason = "Nested documentation may describe policy vocabulary."
"#;
        let (entries, violations) = parse_static_language_allowlist(text);
        assert!(
            violations.is_empty(),
            "unexpected violations: {violations:?}"
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0].matcher,
            StaticLanguageMatcher::Path("AGENTS.md".to_string())
        );
        assert_eq!(entries[0].owner, "maintainers");
        assert_eq!(
            entries[1].matcher,
            StaticLanguageMatcher::Glob("docs/**/*.md".to_string())
        );
        assert_eq!(entries[1].owner, "docs");
    }

    #[test]
    fn static_language_allowlist_requires_schema_version() {
        let text = r#"
[[allow]]
path = "AGENTS.md"
owner = "maintainers"
reason = "Agent instructions describe policy vocabulary."
"#;
        let (_, violations) = parse_static_language_allowlist(text);
        assert!(
            violations.iter().any(|v| v.contains("schema_version = 1")),
            "expected schema_version violation, got {violations:?}"
        );
    }

    #[test]
    fn static_language_allowlist_requires_reason() {
        let text = r#"
schema_version = 1

[[allow]]
path = "AGENTS.md"
owner = "maintainers"
"#;
        let (entries, violations) = parse_static_language_allowlist(text);
        assert!(entries.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("missing required `reason`")),
            "expected missing-reason violation, got {violations:?}"
        );
    }

    #[test]
    fn static_language_allowlist_requires_owner() {
        let text = r#"
schema_version = 1

[[allow]]
path = "AGENTS.md"
reason = "Agent instructions describe policy vocabulary."
"#;
        let (entries, violations) = parse_static_language_allowlist(text);
        assert!(entries.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("missing required `owner`")),
            "expected missing-owner violation, got {violations:?}"
        );
    }

    #[test]
    fn static_language_allowlist_rejects_blank_reason() {
        let text = r#"
schema_version = 1

[[allow]]
path = "AGENTS.md"
owner = "maintainers"
reason = "   "
"#;
        let (entries, violations) = parse_static_language_allowlist(text);
        assert!(entries.is_empty());
        assert!(
            violations.iter().any(|v| v.contains("`reason` is blank")),
            "expected blank-reason violation, got {violations:?}"
        );
    }

    #[test]
    fn static_language_allowlist_rejects_path_and_glob_together() {
        let text = r#"
schema_version = 1

[[allow]]
path = "AGENTS.md"
glob = "docs/*.md"
owner = "maintainers"
reason = "Mixed entry should be rejected."
"#;
        let (entries, violations) = parse_static_language_allowlist(text);
        assert!(entries.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("both `path` and `glob`")),
            "expected path+glob violation, got {violations:?}"
        );
    }

    #[test]
    fn static_language_allowlist_rejects_missing_path_and_glob() {
        let text = r#"
schema_version = 1

[[allow]]
owner = "maintainers"
reason = "Entry without a matcher is meaningless."
"#;
        let (entries, violations) = parse_static_language_allowlist(text);
        assert!(entries.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("must declare either `path` or `glob`")),
            "expected missing-matcher violation, got {violations:?}"
        );
    }

    #[test]
    fn static_language_allowlist_rejects_duplicate_matcher() {
        let text = r#"
schema_version = 1

[[allow]]
path = "AGENTS.md"
owner = "maintainers"
reason = "First declaration."

[[allow]]
path = "AGENTS.md"
owner = "maintainers"
reason = "Second declaration."
"#;
        let (entries, violations) = parse_static_language_allowlist(text);
        assert_eq!(entries.len(), 2, "both entries should still parse");
        assert!(
            violations
                .iter()
                .any(|v| v.contains("`AGENTS.md` is duplicated")),
            "expected duplicate violation, got {violations:?}"
        );
    }

    #[test]
    fn static_language_allowlist_rejects_absolute_path() {
        let unix = r#"
schema_version = 1

[[allow]]
path = "/abs/path.md"
owner = "maintainers"
reason = "Absolute path attempt."
"#;
        let (entries, violations) = parse_static_language_allowlist(unix);
        assert!(entries.is_empty());
        assert!(
            violations.iter().any(|v| v.contains("is absolute")),
            "expected absolute-path violation, got {violations:?}"
        );

        // Windows drive letter form is also rejected. The drive prefix is
        // assembled at runtime so the literal pattern never appears in this
        // source file (`cargo xtask check-local-context` flags `<alpha>:/`).
        let drive = "Z";
        let sep = ":/";
        let win = format!(
            r#"
schema_version = 1

[[allow]]
path = "{drive}{sep}abs/path.md"
owner = "maintainers"
reason = "Windows absolute path attempt."
"#
        );
        let (entries, violations) = parse_static_language_allowlist(&win);
        assert!(entries.is_empty());
        assert!(
            violations.iter().any(|v| v.contains("is absolute")),
            "expected windows-absolute-path violation, got {violations:?}"
        );
    }

    #[test]
    fn static_language_allowlist_rejects_backslash_path() {
        let text = r#"
schema_version = 1

[[allow]]
path = "docs\\BADGE_POLICY.md"
owner = "maintainers"
reason = "Backslash path attempt."
"#;
        let (entries, violations) = parse_static_language_allowlist(text);
        assert!(entries.is_empty());
        assert!(
            violations.iter().any(|v| v.contains("backslashes")),
            "expected backslash-path violation, got {violations:?}"
        );
    }

    #[test]
    fn static_language_allowlist_rejects_repo_wide_markdown_glob() {
        let cases = [
            ("*.md", "repo-wide single-segment markdown glob"),
            ("**/*.md", "repo-wide recursive markdown glob"),
            ("crates/**/*.md", "non-docs glob outside the scoped set"),
        ];
        for (glob, label) in cases {
            let text = format!(
                r#"
schema_version = 1

[[allow]]
glob = "{glob}"
owner = "maintainers"
reason = "{label}"
"#
            );
            let (entries, violations) = parse_static_language_allowlist(&text);
            assert!(entries.is_empty(), "glob `{glob}` should not parse");
            assert!(
                violations
                    .iter()
                    .any(|v| v.contains("not in the scoped set")),
                "expected scoped-glob violation for `{glob}`, got {violations:?}"
            );
        }
    }

    #[test]
    fn allow_attribute_detection_flags_guarded_lint_suppressions() {
        let text = format!(
            "{}\nfn f() {{}}\n{}",
            concat!("#[", "allow", "(clippy::unwrap_used, dead_code)]"),
            concat!("#![", "expect", "(warnings, reason = \"temporary\")]")
        );

        let findings = guarded_allow_attributes_in_text(&text, &guarded_allow_attribute_lints());

        assert_eq!(
            findings,
            vec![
                (1, "allow(clippy::unwrap_used)".to_string()),
                (1, "allow(dead_code)".to_string()),
                (3, concat!("expect", "(warnings)").to_string()),
            ]
        );
    }

    #[test]
    fn allow_attribute_detection_ignores_untracked_lints() {
        let text = format!(
            "{}\nfn f() {{}}\n",
            concat!("#[", "allow", "(clippy::module_name_repetitions)]")
        );

        let findings = guarded_allow_attributes_in_text(&text, &guarded_allow_attribute_lints());

        assert!(findings.is_empty());
    }

    #[test]
    fn local_context_detection_flags_machine_and_session_artifacts() {
        let machine_path = concat!("H:", "\\Code\\Rust\\ripr");
        let line = format!(
            "{}2026-05-01 from `{machine_path}`.",
            concat!("Recorded", " on ")
        );

        let findings = local_context_line_findings(&line);

        assert!(findings.iter().any(|(pattern, _)| pattern == machine_path));
        assert!(
            findings
                .iter()
                .any(|(pattern, _)| pattern == concat!("Recorded", " on <date>"))
        );
    }

    #[test]
    fn local_context_detection_flags_codex_memory_and_chat_artifacts() {
        let memory_path = format!(
            "{}{}",
            concat!("C:", "\\Users\\steven\\"),
            concat!(".", "codex\\memories\\", "MEMORY", ".md")
        );
        let line = format!(
            "See {memory_path}, {}, and {}.",
            concat!("turn", "110", "file", "4"),
            concat!("file_", "00000000abcdef")
        );

        let findings = local_context_line_findings(&line);

        assert!(findings.iter().any(|(pattern, _)| pattern == &memory_path));
        assert!(
            findings
                .iter()
                .any(|(pattern, _)| pattern == concat!(".", "codex"))
        );
        assert!(
            findings
                .iter()
                .any(|(pattern, _)| pattern == concat!("MEMORY", ".md"))
        );
        assert!(
            findings
                .iter()
                .any(|(pattern, _)| pattern == concat!("turn", "110", "file", "4"))
        );
        assert!(
            findings
                .iter()
                .any(|(pattern, _)| pattern == concat!("file_", "00000000abcdef"))
        );
    }

    #[test]
    fn local_context_allowlist_rejects_real_machine_paths() {
        let allowlist = vec![LocalContextAllow {
            path: "docs/example.md".to_string(),
            pattern: concat!("H:", "\\Code\\Rust\\ripr").to_string(),
            max_count: 1,
            line: 7,
        }];

        let violations = validate_local_context_allowlist(&allowlist);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("real machine or session state"));
    }

    #[test]
    fn suspicious_runtime_file_names_include_current_state_notes() {
        assert!(
            suspicious_runtime_file_names()
                .iter()
                .any(|name| name == concat!("CURRENT", "_STATE.md"))
        );
    }

    #[test]
    fn windows_absolute_path_tokens_find_tokens_without_trailing_punctuation() {
        let path = concat!("C:", "\\path\\to\\ripr.exe");
        let line = format!("Use `{path}`.");

        assert_eq!(windows_absolute_path_tokens(&line), vec![path.to_string()]);
    }

    #[test]
    fn workflow_run_extraction_handles_step_shorthand_and_blocks() {
        let workflow = r#"
jobs:
  test:
    steps:
      - run: cargo fmt --check
      - name: block
        run: |
          cargo check
          cargo test
      - uses: actions/checkout@v4
"#;

        let blocks = extract_workflow_run_blocks(workflow);

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].line_number, 5);
        assert_eq!(blocks[0].non_empty_lines, 1);
        assert_eq!(blocks[0].text, "cargo fmt --check");
        assert_eq!(blocks[1].line_number, 7);
        assert_eq!(blocks[1].non_empty_lines, 2);
        assert!(blocks[1].text.contains("cargo check"));
        assert!(blocks[1].text.contains("cargo test"));
    }

    #[test]
    fn workflow_runtime_policy_flags_old_action_refs_and_node20_extension_builds() {
        let workflow = r#"
jobs:
  vscode:
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - uses: actions/upload-artifact@v4
"#;
        let violations =
            workflow_runtime_violations(".github/workflows/ci.yml", workflow, &BTreeMap::new());

        assert!(violations.iter().any(|violation| {
            violation.contains("actions/checkout@v4") && violation.contains("actions/checkout@v6")
        }));
        assert!(violations.iter().any(|violation| {
            violation.contains("actions/setup-node@v4")
                && violation.contains("actions/setup-node@v6")
        }));
        assert!(violations.iter().any(|violation| {
            violation.contains("actions/upload-artifact@v4")
                && violation.contains("actions/upload-artifact@v7")
        }));
        assert!(
            violations
                .iter()
                .any(|violation| { violation.contains("uses Node 20 for extension tooling") })
        );
    }

    #[test]
    fn workflow_runtime_policy_allows_documented_dependency_review_exception() {
        let workflow = r#"
jobs:
  dependency-review:
    steps:
      - uses: actions/checkout@v6
      - uses: actions/dependency-review-action@v4
"#;
        let mut allowlist = BTreeMap::new();
        allowlist.insert(
            (
                ".github/workflows/security.yml".to_string(),
                "actions/dependency-review-action@v4".to_string(),
            ),
            1,
        );

        assert!(
            workflow_runtime_violations(".github/workflows/security.yml", workflow, &allowlist,)
                .is_empty()
        );
        assert!(
            !workflow_runtime_violations(
                ".github/workflows/security.yml",
                workflow,
                &BTreeMap::new(),
            )
            .is_empty()
        );
    }

    #[test]
    fn workflow_runtime_policy_flags_remaining_old_action_refs() {
        let workflow = r#"
jobs:
  release:
    steps:
      - uses: actions/download-artifact@v4
      - uses: codecov/codecov-action@v4
"#;
        let violations = workflow_runtime_violations(
            ".github/workflows/release-server-binaries.yml",
            workflow,
            &BTreeMap::new(),
        );

        assert!(violations.iter().any(|violation| {
            violation.contains("actions/download-artifact@v4")
                && violation.contains("actions/download-artifact@v8")
        }));
        assert!(violations.iter().any(|violation| {
            violation.contains("codecov/codecov-action@v4")
                && violation.contains("codecov/codecov-action@v6")
        }));
    }

    #[test]
    fn workflow_runtime_policy_ignores_node20_outside_extension_workflows() {
        let workflow = r#"
jobs:
  coverage:
    steps:
      - uses: actions/setup-node@v6
        with:
          node-version: 20
"#;

        assert!(
            workflow_runtime_violations(
                ".github/workflows/coverage.yml",
                workflow,
                &BTreeMap::new(),
            )
            .is_empty()
        );
    }

    #[test]
    fn workflow_runtime_policy_rejects_unsupported_allowlist_patterns() {
        let workflow = r#"
jobs:
  security:
    steps:
      - uses: actions/checkout@v6
"#;
        let mut allowlist = BTreeMap::new();
        allowlist.insert(
            (
                ".github/workflows/security.yml".to_string(),
                "actions/unknown-action@v1".to_string(),
            ),
            1,
        );

        let violations =
            workflow_runtime_violations(".github/workflows/security.yml", workflow, &allowlist);

        assert!(violations.iter().any(|violation| {
            violation.contains("unsupported exception")
                && violation.contains("actions/unknown-action@v1")
        }));
    }

    #[test]
    fn workflow_runtime_policy_rejects_dependency_review_over_allowlisted_count() {
        let workflow = r#"
jobs:
  dependency-review:
    steps:
      - uses: actions/dependency-review-action@v4
      - uses: actions/dependency-review-action@v4
"#;
        let mut allowlist = BTreeMap::new();
        allowlist.insert(
            (
                ".github/workflows/security.yml".to_string(),
                "actions/dependency-review-action@v4".to_string(),
            ),
            1,
        );

        let violations =
            workflow_runtime_violations(".github/workflows/security.yml", workflow, &allowlist);

        assert!(violations.iter().any(|violation| {
            violation.contains("uses `actions/dependency-review-action@v4` 2 time(s), allowed 1")
        }));
    }

    #[test]
    fn generated_policy_detects_lockfiles_and_release_artifacts() {
        assert!(is_generated_candidate("Cargo.lock"));
        assert!(is_generated_candidate("editors/vscode/package-lock.json"));
        assert!(is_generated_candidate("editors/vscode/dist/ripr.vsix"));
        assert!(is_generated_candidate(".ripr/release/ripr.zip"));
        assert!(!is_generated_candidate("assets/logo/ripr-icon-dark.png"));
    }

    #[test]
    fn dependency_policy_detects_package_manager_surfaces() {
        assert!(is_dependency_surface_candidate("Cargo.toml"));
        assert!(is_dependency_surface_candidate("xtask/Cargo.toml"));
        assert!(is_dependency_surface_candidate(
            "editors/vscode/package.json"
        ));
        assert!(is_dependency_surface_candidate(
            "fixtures/example/input/Cargo.toml"
        ));
        assert!(is_dependency_surface_candidate(
            "tools/example/requirements.txt"
        ));
        assert!(!is_dependency_surface_candidate("docs/DEPENDENCIES.md"));
    }

    #[test]
    fn sorted_allowlist_content_preserves_header_and_sorts_entries() {
        let input = "# Header\n# More\n\nz|kind|owner|reason\na|kind|owner|reason\n";
        let sorted = sorted_allowlist_content(input);

        assert_eq!(
            sorted,
            "# Header\n# More\n\na|kind|owner|reason\nz|kind|owner|reason\n"
        );
    }

    #[test]
    fn golden_text_comparison_normalizes_line_endings_and_final_newline() {
        assert_eq!(
            normalize_golden_text("one\r\ntwo\r\n"),
            normalize_golden_text("one\ntwo\n")
        );
        assert_eq!(
            normalize_golden_text("one\ntwo\n"),
            normalize_golden_text("one\ntwo")
        );
        assert_ne!(
            normalize_golden_text("one\ntwo\n\n"),
            normalize_golden_text("one\ntwo\n")
        );
        assert_ne!(
            normalize_golden_text("one\ntwo\n\n"),
            normalize_golden_text("one\ntwo")
        );
    }

    #[test]
    fn first_line_difference_reports_snapshot_context() {
        assert_eq!(
            first_line_difference("a\nb\nc", "a\nx\nc"),
            Some("line 2 expected `b` vs actual `x`".to_string())
        );
        assert_eq!(
            first_line_difference("a\nb", "a\nb\nc"),
            Some("line 3 expected `<missing>` vs actual `c`".to_string())
        );
        assert_eq!(first_line_difference("a\nb", "a\nb"), None);
    }

    #[test]
    fn first_line_difference_escapes_control_characters_for_snapshot_diffs() {
        assert_eq!(
            first_line_difference("ok\nline\tone", "ok\nline\rone"),
            Some("line 2 expected `line\\tone` vs actual `line\\rone`".to_string())
        );
    }

    #[test]
    fn first_line_difference_escapes_backticks_for_snapshot_diffs() {
        assert_eq!(
            first_line_difference("value with `tick`", "value with plain tick"),
            Some(
                "line 1 expected `value with \\`tick\\`` vs actual `value with plain tick`"
                    .to_string()
            )
        );
    }

    #[test]
    fn first_line_difference_truncates_very_long_snapshot_lines() {
        let expected = format!("ok\n{}", "a".repeat(130));
        let actual = format!("ok\n{}", "b".repeat(130));
        let diff = first_line_difference(&expected, &actual);
        assert!(matches!(
            diff.as_deref(),
            Some(message)
                if message.contains("expected `") && message.contains("…` vs actual `")
        ));
    }

    #[test]
    fn fixture_output_normalization_keeps_json_escapes_readable() {
        let json =
            r#"{"file":"fixtures/example/input\\src/lib.rs","oracle":"assert!(value == \"x\")"}"#;

        assert_eq!(
            normalize_fixture_json_output(json),
            r#"{"file":"fixtures/example/input/src/lib.rs","oracle":"assert!(value == \"x\")"}"#
        );
        assert_eq!(
            normalize_fixture_human_output("fixtures\\example\\input\\src/lib.rs"),
            "fixtures/example/input/src/lib.rs\n"
        );
    }

    #[test]
    fn path_classification_separates_production_evidence_and_policy() {
        assert!(is_production_path("crates/ripr/src/analysis/mod.rs"));
        assert!(is_production_path("editors/vscode/src/client.ts"));
        assert!(is_evidence_path(
            "docs/specs/RIPR-SPEC-0001-static-exposure-loop.md"
        ));
        assert!(is_evidence_path("fixtures/boundary_gap/SPEC.md"));
        assert!(is_evidence_path("metrics/capabilities.toml"));
        assert!(is_evidence_path("xtask/src/main.rs"));
        assert!(is_policy_path(".github/workflows/ci.yml"));
        assert!(is_policy_path("policy/non_rust_allowlist.txt"));
        assert!(!is_production_path("docs/ENGINEERING.md"));
    }

    #[test]
    fn public_contract_rows_detect_json_and_lsp_surfaces() {
        let changes = vec![
            ChangedPath {
                path: "crates/ripr/src/output/json/report.rs".to_string(),
                statuses: BTreeSet::from(["M".to_string()]),
            },
            ChangedPath {
                path: "editors/vscode/src/client.ts".to_string(),
                statuses: BTreeSet::from(["M".to_string()]),
            },
        ];

        let rows = public_contract_rows(&changes);
        let json = rows
            .iter()
            .find(|(label, _)| *label == "JSON")
            .map(|(_, paths)| paths.clone())
            .unwrap_or_default();
        let lsp = rows
            .iter()
            .find(|(label, _)| *label == "LSP")
            .map(|(_, paths)| paths.clone())
            .unwrap_or_default();

        assert_eq!(json, vec!["crates/ripr/src/output/json/report.rs (M)"]);
        assert_eq!(lsp, vec!["editors/vscode/src/client.ts (M)"]);
    }

    #[test]
    fn pr_shape_warnings_flag_analysis_without_evidence() {
        let changes = vec![ChangedPath {
            path: "crates/ripr/src/analysis/classifier.rs".to_string(),
            statuses: BTreeSet::from(["M".to_string()]),
        }];
        let warnings = pr_shape_warnings(&changes);
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("Analysis code changed"))
        );

        let with_evidence = vec![
            ChangedPath {
                path: "crates/ripr/src/analysis/classifier.rs".to_string(),
                statuses: BTreeSet::from(["M".to_string()]),
            },
            ChangedPath {
                path: "docs/specs/RIPR-SPEC-0001-static-exposure-loop.md".to_string(),
                statuses: BTreeSet::from(["M".to_string()]),
            },
        ];
        assert!(pr_shape_warnings(&with_evidence).is_empty());
    }

    #[test]
    fn critic_flags_analysis_without_fixture_or_golden_reports() {
        let changes = vec![ChangedPath {
            path: "crates/ripr/src/analysis/classifier.rs".to_string(),
            statuses: BTreeSet::from(["M".to_string()]),
        }];
        let reports = vec![ReportIndexEntry {
            file: "pr-summary.md".to_string(),
            path: "target/ripr/reports/pr-summary.md".to_string(),
            status: "pass".to_string(),
        }];
        let findings = critic_findings(&changes, &reports, &[]);

        assert!(
            findings
                .iter()
                .any(|finding| finding.id == "analysis_without_behavior_evidence")
        );
        assert!(
            findings
                .iter()
                .any(|finding| finding.id == "analysis_missing_fixture_report")
        );
        assert!(
            findings
                .iter()
                .any(|finding| finding.id == "missing_receipts")
        );
    }

    #[test]
    fn critic_accepts_analysis_with_evidence_reports_and_receipts() {
        let changes = vec![
            ChangedPath {
                path: "crates/ripr/src/analysis/classifier.rs".to_string(),
                statuses: BTreeSet::from(["M".to_string()]),
            },
            ChangedPath {
                path: "fixtures/boundary_gap/SPEC.md".to_string(),
                statuses: BTreeSet::from(["M".to_string()]),
            },
        ];
        let reports = ["pr-summary.md", "pr-shape.md", "fixtures.md", "goldens.md"]
            .into_iter()
            .map(|file| ReportIndexEntry {
                file: file.to_string(),
                path: format!("target/ripr/reports/{file}"),
                status: "pass".to_string(),
            })
            .collect::<Vec<_>>();
        let receipts = vec![ReportIndexEntry {
            file: "check-pr.json".to_string(),
            path: "target/ripr/receipts/check-pr.json".to_string(),
            status: "present".to_string(),
        }];

        let findings = critic_findings(&changes, &reports, &receipts);

        assert!(
            !findings
                .iter()
                .any(|finding| finding.id.starts_with("analysis_"))
        );
        assert!(
            !findings
                .iter()
                .any(|finding| finding.id == "missing_receipts")
        );
    }

    #[test]
    fn critic_flags_golden_changes_without_blessing_reason() {
        let changes = vec![ChangedPath {
            path: "fixtures/boundary_gap/expected/check.json".to_string(),
            statuses: BTreeSet::from(["M".to_string()]),
        }];

        let missing = golden_changes_without_blessing(&changes);

        assert_eq!(missing.len(), 1);
        assert!(missing[0].contains("fixtures/boundary_gap/expected/CHANGELOG.md"));
    }

    #[test]
    fn precommit_report_points_to_review_ready_gate() {
        let body = precommit_report_body();

        assert!(body.contains("cargo fmt --check"));
        assert!(body.contains("cargo xtask check-pr"));
    }

    #[test]
    fn report_status_parser_reads_markdown_and_json() {
        assert_eq!(
            report_status_from_text("# Report\n\nStatus: warn\n\nBody"),
            Some("warn".to_string())
        );
        assert_eq!(
            report_status_from_text("{\n  \"status\": \"pass\"\n}"),
            Some("pass".to_string())
        );
        assert_eq!(report_status_from_text("# Report\n\nBody"), None);
    }

    #[test]
    fn report_index_missing_expected_tracks_changed_surfaces() {
        let reports = vec![ReportIndexEntry {
            file: "pr-summary.md".to_string(),
            path: "target/ripr/reports/pr-summary.md".to_string(),
            status: "pass".to_string(),
        }];
        let changes = vec![
            ChangedPath {
                path: "docs/PR_AUTOMATION.md".to_string(),
                statuses: BTreeSet::from(["M".to_string()]),
            },
            ChangedPath {
                path: ".ripr/goals/active.toml".to_string(),
                statuses: BTreeSet::from(["M".to_string()]),
            },
        ];

        let missing = report_index_missing_expected(&reports, &changes);

        assert!(missing.contains(&"target/ripr/reports/check-pr.md".to_string()));
        assert!(missing.contains(&"target/ripr/reports/doc-index.md".to_string()));
        assert!(missing.contains(&"target/ripr/reports/markdown-links.md".to_string()));
        assert!(missing.contains(&"target/ripr/reports/campaign.md".to_string()));
        assert!(missing.contains(&"target/ripr/reports/goals-next.md".to_string()));
    }

    #[test]
    fn report_index_expects_golden_drift_for_output_changes() {
        let reports = vec![
            ReportIndexEntry {
                file: "pr-summary.md".to_string(),
                path: "target/ripr/reports/pr-summary.md".to_string(),
                status: "pass".to_string(),
            },
            ReportIndexEntry {
                file: "check-pr.md".to_string(),
                path: "target/ripr/reports/check-pr.md".to_string(),
                status: "pass".to_string(),
            },
        ];
        let changes = vec![ChangedPath {
            path: "crates/ripr/src/output/human.rs".to_string(),
            statuses: BTreeSet::from(["M".to_string()]),
        }];

        let missing = report_index_missing_expected(&reports, &changes);

        assert!(missing.contains(&"target/ripr/reports/golden-drift.md".to_string()));
    }

    #[test]
    fn report_index_markdown_includes_review_path_and_receipts() {
        let campaign = ReportIndexCampaign {
            id: "evidence-quality".to_string(),
            title: "Make ripr findings evidence-first".to_string(),
            status: "active".to_string(),
            ready_work_items: vec!["output/unknown-stop-reason-invariant".to_string()],
            issues: Vec::new(),
        };
        let reports = vec![ReportIndexEntry {
            file: "pr-summary.md".to_string(),
            path: "target/ripr/reports/pr-summary.md".to_string(),
            status: "pass".to_string(),
        }];
        let receipts = vec![ReportIndexEntry {
            file: "check-pr.json".to_string(),
            path: "target/ripr/receipts/check-pr.json".to_string(),
            status: "pass".to_string(),
        }];
        let body = report_index_markdown(
            "pass",
            &campaign,
            &reports,
            &receipts,
            &[],
            &["cargo xtask check-pr".to_string()],
        );

        assert!(body.contains("# ripr report index"));
        assert!(body.contains("output/unknown-stop-reason-invariant"));
        assert!(body.contains("Suggested Reviewer Path"));
        assert_eq!(status_for_report(&reports, "pr-summary.md"), "pass");
        assert_eq!(status_for_report(&reports, "missing.md"), "missing");
    }

    #[test]
    fn receipt_status_aggregates_report_statuses() {
        assert_eq!(receipt_status_from_reports(&[]), "missing");
        assert_eq!(
            receipt_status_from_reports(&["target/ripr/reports/not-present.md".to_string()]),
            "missing"
        );

        let pass = ReceiptRecord {
            file: "check-pr.json".to_string(),
            command: "cargo xtask check-pr".to_string(),
            status: "passed".to_string(),
            reports: vec!["target/ripr/reports/check-pr.md".to_string()],
        };
        let git = BTreeMap::from([
            ("branch".to_string(), "test-branch".to_string()),
            ("commit".to_string(), "abc123".to_string()),
        ]);
        let json = receipt_json(&pass, &git);

        assert!(json.contains("\"schema_version\": \"0.1\""));
        assert!(json.contains("\"command\": \"cargo xtask check-pr\""));
        assert!(json.contains("\"status\": \"passed\""));
        assert!(json.contains("\"branch\": \"test-branch\""));
        assert!(json.contains("target/ripr/reports/check-pr.md"));
    }

    #[test]
    fn receipt_statuses_accept_expected_values() {
        assert!(is_receipt_status("passed"));
        assert!(is_receipt_status("warn"));
        assert!(is_receipt_status("failed"));
        assert!(is_receipt_status("missing"));
        assert!(!is_receipt_status("unknown"));
    }

    #[test]
    fn receipt_specs_cover_required_gates() {
        let files = receipt_specs()
            .into_iter()
            .map(|spec| spec.file.to_string())
            .collect::<BTreeSet<_>>();

        assert!(files.contains("shape.json"));
        assert!(files.contains("fix-pr.json"));
        assert!(files.contains("ci-fast.json"));
        assert!(files.contains("check-pr.json"));
        assert!(files.contains("fixtures.json"));
        assert!(files.contains("goldens.json"));
        assert!(files.contains("test-oracles.json"));
        assert!(files.contains("badge-artifacts.json"));
        assert!(files.contains("repo-badge-artifacts.json"));
        assert!(files.contains("dogfood.json"));
        assert!(files.contains("metrics.json"));
        assert!(known_xtask_command("receipts"));
    }

    #[test]
    fn golden_drift_semantics_summarize_json_changes() {
        let expected = r#"{
  "findings": [
    {"id":"probe:src_lib.rs:1:predicate","classification":"infection_unknown","probe":{"family":"predicate"},"related_tests":[{"oracle_kind":"exact_value","oracle_strength":"strong"}],"recommended_next_step":"Add a boundary test"}
  ]
}"#;
        let actual = r#"{
  "findings": [
    {"id":"probe:src_lib.rs:1:predicate","classification":"weakly_exposed","probe":{"family":"predicate"},"related_tests":[{"oracle_kind":"smoke_only","oracle_strength":"smoke"}],"stop_reasons":["opaque fixture"],"recommended_next_step":"Assert the exact variant"},
    {"id":"probe:src_lib.rs:2:error_path","classification":"weakly_exposed","probe":{"family":"error_path"},"related_tests":[{"oracle_kind":"broad_error","oracle_strength":"smoke"}],"recommended_next_step":"Assert the exact variant"}
  ]
}"#;

        let semantics = golden_drift_semantics("check.json", expected, actual);

        assert!(
            semantics
                .added_finding_ids
                .contains(&"probe:src_lib.rs:2:error_path".to_string())
        );
        assert!(semantics.changed_exposure_classes.iter().any(|value| {
            value.contains("infection_unknown") && value.contains("weakly_exposed")
        }));
        assert!(
            semantics
                .changed_probe_families
                .iter()
                .any(|value| value.contains("error_path"))
        );
        assert!(
            semantics
                .changed_oracle_strengths
                .iter()
                .any(|value| value.contains("smoke"))
        );
        assert!(
            semantics
                .changed_oracle_kinds
                .iter()
                .any(|value| value.contains("broad_error"))
        );
        assert!(
            semantics
                .changed_stop_reasons
                .iter()
                .any(|value| value.contains("opaque fixture"))
        );
        assert!(
            semantics
                .changed_recommendations
                .iter()
                .any(|value| value.contains("Assert the exact variant"))
        );
    }

    #[test]
    fn json_string_values_for_key_reads_multiline_arrays() {
        let text = r#"{
  "stop_reasons": [
    "opaque fixture",
    "missing owner"
  ]
}"#;

        let values = json_string_values_for_key(text, "stop_reasons");

        assert_eq!(
            values,
            BTreeSet::from(["missing owner".to_string(), "opaque fixture".to_string()])
        );
    }

    #[test]
    fn parse_reason_accepts_flag_forms() {
        let spaced = vec!["--reason".to_string(), "intentional update".to_string()];
        let equals = vec!["--reason=intentional update".to_string()];

        assert_eq!(parse_reason(&spaced), Ok("intentional update".to_string()));
        assert_eq!(parse_reason(&equals), Ok("intentional update".to_string()));
        parse_reason(&[]).expect_err("empty args should fail to produce a reason");
    }

    #[test]
    fn spec_id_helpers_accept_only_ripr_spec_ids() {
        assert!(is_spec_id("RIPR-SPEC-0001"));
        assert!(!is_spec_id("RIPR-SPEC-001"));
        assert!(!is_spec_id("SPEC-0001"));
        assert_eq!(
            spec_id_from_path(Path::new("docs/specs/RIPR-SPEC-0004-predicate-boundary.md")),
            Some("RIPR-SPEC-0004".to_string())
        );
        assert_eq!(spec_id_from_path(Path::new("docs/specs/README.md")), None);
    }

    #[test]
    fn traceability_array_parser_reads_inline_values() {
        assert_eq!(
            parse_inline_array("[\"one\", \"two\"]"),
            Ok(vec!["one".to_string(), "two".to_string()])
        );
        assert_eq!(parse_inline_array("[]"), Ok(Vec::new()));
        parse_inline_array("[one]").expect_err("unquoted token should fail array parse");
    }

    #[test]
    fn capability_helpers_validate_ids_and_escape_json() {
        assert!(is_snake_case_id("agent_context_v2"));
        assert!(!is_snake_case_id("AgentContextV2"));
        assert!(!is_snake_case_id("agent__context"));
        assert_eq!(json_escape("a\"b\\c\n"), "a\\\"b\\\\c\\n");

        let capability = Capability {
            id: Some("agent_context_v2".to_string()),
            name: Some("Agent context v2".to_string()),
            status: Some("planned".to_string()),
            spec: Some("RIPR-SPEC-0003".to_string()),
            evidence: vec!["agent context spec".to_string()],
            fixtures: Vec::new(),
            next: Some("agent-context-v2".to_string()),
            metric: Some("context packets with suggested assertions".to_string()),
            line: 1,
        };
        let json = super::capability_metrics_json(&[capability]);
        assert!(json.contains("\"id\": \"agent_context_v2\""));
        assert!(json.contains("\"schema_version\": \"0.1\""));
    }

    #[test]
    fn markdown_link_helpers_skip_fences_and_external_targets() {
        let text = "[Docs](docs/README.md#top)\n```md\n[Ignored](missing.md)\n```\n[External](https://example.com)\n[Anchor](#section)\n";

        let links = markdown_links_in_text(text);

        assert_eq!(
            links,
            vec![
                MarkdownLink {
                    line: 1,
                    target: "docs/README.md#top".to_string()
                },
                MarkdownLink {
                    line: 5,
                    target: "https://example.com".to_string()
                },
                MarkdownLink {
                    line: 6,
                    target: "#section".to_string()
                }
            ]
        );
        assert_eq!(
            local_markdown_target("docs/README.md#top"),
            Some("docs/README.md".to_string())
        );
        assert_eq!(
            local_markdown_target("<docs/My File.md>"),
            Some("docs/My File.md".to_string())
        );
        assert_eq!(local_markdown_target("https://example.com"), None);
        assert_eq!(local_markdown_target("#section"), None);
    }

    #[test]
    fn capability_next_checkpoint_parser_reads_source_values() {
        let source = "next = \"fixture-laboratory\"\nmetric = \"fixture pass rate\"\nnext = \"agent-context-v2\"\n";

        assert_eq!(
            next_checkpoints_from_capabilities(source),
            Ok(vec![
                "fixture-laboratory".to_string(),
                "agent-context-v2".to_string()
            ])
        );
    }

    #[test]
    fn campaign_manifest_parser_reads_work_items() -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "ripr-campaign-manifest-test-{}.toml",
            std::process::id()
        ));
        let source = r#"id = "agentic-devex-foundation"
title = "Agentic DevEx Foundation"
status = "active"

objective = """
Build the repo operating system.
"""

end_state = [
  "architecture guard exists"
]

[[work_item]]
id = "fixtures/first-two-goldens"
status = "ready"
branch = "fixtures/first-two-goldens"
stackable = false
requires_human_merge = true
acceptance = "fixtures pass"
commands = [
  "cargo xtask fixtures",
  "cargo xtask check-pr"
]
"#;
        fs::write(&path, source)?;

        let parsed = parse_campaign_manifest(&path);
        let _ = fs::remove_file(&path);
        let (manifest, violations) = parsed?;

        assert!(violations.is_empty());
        assert_eq!(manifest.id, Some("agentic-devex-foundation".to_string()));
        assert_eq!(manifest.work_items.len(), 1);
        assert_eq!(
            manifest.work_items[0].id,
            Some("fixtures/first-two-goldens".to_string())
        );
        assert_eq!(manifest.work_items[0].commands.len(), 2);
        Ok(())
    }

    #[test]
    fn campaign_command_validator_accepts_known_repo_commands() {
        assert!(is_known_campaign_command("cargo xtask check-pr"));
        assert!(is_known_campaign_command("cargo xtask goals status"));
        assert!(is_known_campaign_command("cargo xtask reports index"));
        assert!(is_known_campaign_command("cargo xtask receipts check"));
        assert!(is_known_campaign_command("cargo xtask golden-drift"));
        assert!(is_known_campaign_command(
            "cargo xtask check-allow-attributes"
        ));
        assert!(is_known_campaign_command("cargo xtask test-oracle-report"));
        assert!(is_known_campaign_command(
            "cargo xtask test-efficiency-report"
        ));
        assert!(is_known_campaign_command("cargo xtask dogfood"));
        assert!(is_known_campaign_command("cargo test --workspace"));
        assert!(!is_known_campaign_command("cargo xtask missing-command"));
        assert!(!is_known_campaign_command(""));

        let manifest = CampaignManifest::default();
        assert!(manifest.work_items.is_empty());
    }

    #[test]
    fn test_oracle_parser_classifies_strong_weak_and_smoke_tests() {
        let source = r#"
#[test]
fn exact_json_contract() {
    assert_eq!(actual_json, expected_json);
}

#[test]
fn weak_status_check() {
    assert!(result.is_err());
}

#[test]
fn smoke_command_runs() {
    assert!(status.success());
}
"#;

        let tests = test_oracle_tests_in_text(Path::new("crates/ripr/tests/example.rs"), source);

        assert_eq!(tests.len(), 3);
        assert_eq!(tests[0].name, "exact_json_contract");
        assert_eq!(tests[0].class, TestOracleClass::Strong);
        assert_eq!(tests[1].name, "weak_status_check");
        assert_eq!(tests[1].class, TestOracleClass::Weak);
        assert_eq!(tests[2].name, "smoke_command_runs");
        assert_eq!(tests[2].class, TestOracleClass::Smoke);
    }

    #[test]
    fn test_oracle_reports_include_advisory_debt() {
        let source = r#"
#[test]
fn weak_contains() {
    assert!(stdout.contains("warning"));
}
"#;
        let tests = test_oracle_tests_in_text(Path::new("crates/ripr/tests/example.rs"), source);
        let markdown = test_oracle_report_markdown(&tests);
        let json = test_oracle_report_json(&tests);

        assert!(markdown.contains("Status: warn"));
        assert!(markdown.contains("Weak Or Smoke Tests"));
        assert!(markdown.contains("BDD-shaped names: 0 / 1"));
        assert!(json.contains("\"advisory\": true"));
        assert!(json.contains("\"weak\": 1"));
    }

    #[test]
    fn test_efficiency_ledger_records_owner_oracle_values_and_limitations() {
        let source = r#"
#[test]
fn creates_invoice_smoke() {
    let invoice = create_invoice("acct-1", 100);
    assert!(invoice.is_ok());
}
"#;
        let tests = test_oracle_tests_in_text(Path::new("crates/ripr/tests/example.rs"), source);
        let entry = test_efficiency_entry(&tests[0]);

        assert_eq!(entry.name, "creates_invoice_smoke");
        assert_eq!(entry.class, "useful_but_broad");
        assert_eq!(entry.oracle_kind, "broad predicate");
        assert_eq!(entry.oracle_strength, "weak");
        assert!(entry.reasons.contains(&"broad_oracle".to_string()));
        assert!(
            entry
                .reasons
                .contains(&"assertion_may_not_match_detected_owner".to_string())
        );
        assert!(entry.reached_owners.contains(&"create_invoice".to_string()));
        assert!(
            entry
                .observed_values
                .iter()
                .any(|value| value.value == "\"acct-1\"")
        );
        assert!(
            entry
                .observed_values
                .iter()
                .any(|value| value.value == "100")
        );
        assert!(
            entry
                .static_limitations
                .iter()
                .any(|limitation| limitation.contains("broad oracle"))
        );
    }

    #[test]
    fn test_efficiency_reports_are_advisory() {
        let source = r#"
#[test]
fn smoke_status_check() {
    let status = run_cli();
    assert!(status.success());
}
"#;
        let tests = test_oracle_tests_in_text(Path::new("crates/ripr/tests/example.rs"), source);
        let entry = test_efficiency_entry(&tests[0]);
        let markdown = test_efficiency_report_markdown(
            std::slice::from_ref(&entry),
            &[],
            &TestIntentReportSummary::default(),
        );
        let json = test_efficiency_report_json(
            std::slice::from_ref(&entry),
            &[],
            &TestIntentReportSummary::default(),
        );

        assert_eq!(entry.class, "smoke_only");
        assert!(entry.reasons.contains(&"smoke_oracle_only".to_string()));
        assert!(markdown.contains("Mode: advisory"));
        assert!(markdown.contains("smoke_status_check"));
        assert!(json.contains("\"advisory\": true"));
        assert!(json.contains("\"smoke_only\": 1"));
    }

    #[test]
    fn test_efficiency_signals_likely_vacuous_and_possibly_circular_tests() {
        let source = r#"
#[test]
fn creates_invoice_without_assertion() {
    create_invoice("acct-1", 100);
}

#[test]
fn expected_uses_same_owner_path() {
    assert_eq!(actual_invoice, create_invoice("acct-1", 100));
}
"#;
        let tests = test_oracle_tests_in_text(Path::new("crates/ripr/tests/example.rs"), source);
        let vacuous = test_efficiency_entry(&tests[0]);
        let circular = test_efficiency_entry(&tests[1]);
        let json = test_efficiency_report_json(
            &[vacuous.clone(), circular.clone()],
            &[],
            &TestIntentReportSummary::default(),
        );

        assert_eq!(vacuous.class, "likely_vacuous");
        assert!(
            vacuous
                .reasons
                .contains(&"no_assertion_detected".to_string())
        );
        assert_eq!(circular.class, "possibly_circular");
        assert!(
            circular
                .reasons
                .contains(&"expected_value_computed_from_detected_owner_path".to_string())
        );
        assert!(json.contains("\"likely_vacuous\": 1"));
        assert!(json.contains("\"possibly_circular\": 1"));
        assert!(json.contains("\"reason_counts\""));
    }

    #[test]
    fn test_efficiency_does_not_treat_actual_side_owner_call_as_circular() {
        let source = r#"
#[test]
fn exact_owner_call_has_external_expected_value() {
    assert_eq!(create_invoice("acct-1", 100), expected_invoice);
}
"#;
        let tests = test_oracle_tests_in_text(Path::new("crates/ripr/tests/example.rs"), source);
        let entry = test_efficiency_entry(&tests[0]);

        assert_eq!(entry.class, "strong_discriminator");
        assert!(
            !entry
                .reasons
                .contains(&"expected_value_computed_from_detected_owner_path".to_string())
        );
    }

    #[test]
    fn bdd_name_helper_accepts_given_when_then_pattern() {
        assert!(is_bdd_test_name(
            "given_invalid_token_when_authenticate_then_returns_revoked_error"
        ));
        assert!(!is_bdd_test_name(
            "given_invalid_token_then_returns_revoked_error_when_authenticate"
        ));
        assert!(!is_bdd_test_name(
            "given__when_authenticate_then_returns_revoked_error"
        ));
        assert!(!is_bdd_test_name(
            "given_invalid_token_when__then_returns_revoked_error"
        ));
        assert!(!is_bdd_test_name("authenticate_rejects_invalid_token"));
    }

    #[test]
    fn dogfood_helpers_summarize_json_output() {
        let json = r#"{
  "summary": {"findings":2},
  "findings": [
    {"classification": "weakly_exposed", "stop_reasons": []},
    {"classification": "static_unknown", "stop_reasons": ["syntax unknown"]}
  ]
}"#;

        let counts = dogfood_class_counts(json);

        assert_eq!(json_number_after(json, "\"findings\":"), Some(2));
        assert_eq!(counts.get("weakly_exposed").copied(), Some(1));
        assert_eq!(counts.get("static_unknown").copied(), Some(1));
    }

    #[test]
    fn dogfood_reports_are_advisory() {
        let run = DogfoodRun {
            name: "boundary_gap".to_string(),
            root: Path::new("fixtures/boundary_gap/input").to_path_buf(),
            diff: Path::new("fixtures/boundary_gap/diff.patch").to_path_buf(),
            actual_dir: Path::new("target/ripr/dogfood/boundary_gap").to_path_buf(),
            duration_ms: 10,
            findings: 1,
            class_counts: [("weakly_exposed".to_string(), 1usize)]
                .into_iter()
                .collect(),
            stop_reason_mentions: 1,
            errors: Vec::new(),
        };

        let markdown = dogfood_report_markdown(&[run]);
        let json = dogfood_report_json(&[]);

        assert!(markdown.contains("Mode: advisory"));
        assert!(markdown.contains("boundary_gap"));
        assert!(json.contains("\"advisory\": true"));
    }

    fn duplicate_entry(
        name: &str,
        line: usize,
        class: &'static str,
        oracle_kind: &str,
        oracle_strength: &'static str,
        owners: &[&str],
        activations: &[(&'static str, &str)],
    ) -> TestEfficiencyEntry {
        TestEfficiencyEntry {
            path: Path::new("tests/example.rs").to_path_buf(),
            name: name.to_string(),
            line,
            class,
            oracle_kind: oracle_kind.to_string(),
            oracle_strength,
            reached_owners: owners.iter().map(|s| s.to_string()).collect(),
            observed_values: activations
                .iter()
                .enumerate()
                .map(|(index, (context, value))| TestEfficiencyValue {
                    line: line + index,
                    context,
                    value: (*value).to_string(),
                    text: (*value).to_string(),
                })
                .collect(),
            reasons: Vec::new(),
            static_limitations: Vec::new(),
            duplicate_group_id: None,
            declared_intent: None,
        }
    }

    #[test]
    fn duplicate_grouping_groups_structurally_identical_eligible_tests() {
        let mut entries = vec![
            duplicate_entry(
                "discount_above_threshold",
                10,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::discounted_total"],
                &[("function_argument", "100"), ("assertion_argument", "90")],
            ),
            duplicate_entry(
                "vip_discount_above_threshold",
                30,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::discounted_total"],
                &[("function_argument", "100"), ("assertion_argument", "90")],
            ),
        ];

        let groups = apply_duplicate_discriminator_groups(&mut entries);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id, "duplicate_group_1");
        assert_eq!(groups[0].members.len(), 2);
        for entry in &entries {
            assert_eq!(entry.class, "duplicative");
            assert!(
                entry
                    .reasons
                    .contains(&"duplicate_activation_and_oracle_shape".to_string())
            );
            assert_eq!(
                entry.duplicate_group_id.as_deref(),
                Some("duplicate_group_1")
            );
        }
    }

    #[test]
    fn duplicate_grouping_does_not_group_different_owners() {
        let mut entries = vec![
            duplicate_entry(
                "a",
                10,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::a"],
                &[("function_argument", "1")],
            ),
            duplicate_entry(
                "b",
                30,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::b"],
                &[("function_argument", "1")],
            ),
        ];

        let groups = apply_duplicate_discriminator_groups(&mut entries);

        assert!(groups.is_empty());
        for entry in &entries {
            assert_eq!(entry.class, "strong_discriminator");
            assert!(entry.duplicate_group_id.is_none());
        }
    }

    #[test]
    fn duplicate_grouping_does_not_group_different_activation_values() {
        let mut entries = vec![
            duplicate_entry(
                "a",
                10,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::p"],
                &[("function_argument", "1")],
            ),
            duplicate_entry(
                "b",
                30,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::p"],
                &[("function_argument", "2")],
            ),
        ];

        let groups = apply_duplicate_discriminator_groups(&mut entries);

        assert!(groups.is_empty());
    }

    #[test]
    fn duplicate_grouping_does_not_group_swapped_function_and_assertion_values() {
        let mut entries = vec![
            duplicate_entry(
                "score_two_equals_three",
                10,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::score"],
                &[("function_argument", "2"), ("assertion_argument", "3")],
            ),
            duplicate_entry(
                "score_three_equals_two",
                30,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::score"],
                &[("function_argument", "3"), ("assertion_argument", "2")],
            ),
        ];

        let groups = apply_duplicate_discriminator_groups(&mut entries);

        assert!(
            groups.is_empty(),
            "swapped activation/expected values must not group: same raw value set, different roles"
        );
    }

    #[test]
    fn duplicate_grouping_does_not_group_swapped_function_argument_order() {
        let mut entries = vec![
            duplicate_entry(
                "f_one_two",
                10,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::f"],
                &[("function_argument", "1"), ("function_argument", "2")],
            ),
            duplicate_entry(
                "f_two_one",
                30,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::f"],
                &[("function_argument", "2"), ("function_argument", "1")],
            ),
        ];

        let groups = apply_duplicate_discriminator_groups(&mut entries);

        assert!(groups.is_empty(), "argument order must matter");
    }

    #[test]
    fn duplicate_grouping_does_not_group_different_oracle_kind() {
        let mut entries = vec![
            duplicate_entry(
                "a",
                10,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::p"],
                &[("function_argument", "1")],
            ),
            duplicate_entry(
                "b",
                30,
                "useful_but_broad",
                "broad predicate",
                "strong",
                &["pricing::p"],
                &[("function_argument", "1")],
            ),
        ];

        let groups = apply_duplicate_discriminator_groups(&mut entries);

        assert!(groups.is_empty());
    }

    #[test]
    fn duplicate_grouping_does_not_group_different_oracle_strength() {
        let mut entries = vec![
            duplicate_entry(
                "a",
                10,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::p"],
                &[("function_argument", "1")],
            ),
            duplicate_entry(
                "b",
                30,
                "useful_but_broad",
                "exact assertion",
                "weak",
                &["pricing::p"],
                &[("function_argument", "1")],
            ),
        ];

        let groups = apply_duplicate_discriminator_groups(&mut entries);

        assert!(groups.is_empty());
    }

    #[test]
    fn duplicate_grouping_excludes_opaque_entries() {
        let mut entries = vec![
            duplicate_entry(
                "a",
                10,
                "opaque",
                "exact assertion",
                "strong",
                &[],
                &[("function_argument", "1")],
            ),
            duplicate_entry(
                "b",
                30,
                "opaque",
                "exact assertion",
                "strong",
                &[],
                &[("function_argument", "1")],
            ),
        ];

        let groups = apply_duplicate_discriminator_groups(&mut entries);

        assert!(groups.is_empty());
        for entry in &entries {
            assert_eq!(entry.class, "opaque");
            assert!(entry.duplicate_group_id.is_none());
        }
    }

    #[test]
    fn duplicate_grouping_keeps_likely_vacuous_priority() {
        let mut a = duplicate_entry(
            "a",
            10,
            "likely_vacuous",
            "exact assertion",
            "strong",
            &["pricing::p"],
            &[("function_argument", "1")],
        );
        a.reasons.push("no_assertion_detected".to_string());
        let mut b = duplicate_entry(
            "b",
            30,
            "likely_vacuous",
            "exact assertion",
            "strong",
            &["pricing::p"],
            &[("function_argument", "1")],
        );
        b.reasons.push("no_assertion_detected".to_string());
        let mut entries = vec![a, b];

        let groups = apply_duplicate_discriminator_groups(&mut entries);

        assert!(groups.is_empty());
        for entry in &entries {
            assert_eq!(entry.class, "likely_vacuous");
        }
    }

    #[test]
    fn duplicate_grouping_keeps_possibly_circular_priority() {
        let mut a = duplicate_entry(
            "a",
            10,
            "possibly_circular",
            "exact assertion",
            "strong",
            &["pricing::p"],
            &[("function_argument", "1")],
        );
        a.reasons
            .push("expected_value_computed_from_detected_owner_path".to_string());
        let mut b = duplicate_entry(
            "b",
            30,
            "possibly_circular",
            "exact assertion",
            "strong",
            &["pricing::p"],
            &[("function_argument", "1")],
        );
        b.reasons
            .push("expected_value_computed_from_detected_owner_path".to_string());
        let mut entries = vec![a, b];

        let groups = apply_duplicate_discriminator_groups(&mut entries);

        assert!(groups.is_empty());
        for entry in &entries {
            assert_eq!(entry.class, "possibly_circular");
        }
    }

    #[test]
    fn duplicate_grouping_promotes_smoke_duplicates_and_keeps_smoke_reason() {
        let mut a = duplicate_entry(
            "a",
            10,
            "smoke_only",
            "smoke only",
            "smoke",
            &["pricing::p"],
            &[("function_argument", "1")],
        );
        a.reasons.push("smoke_oracle_only".to_string());
        let mut b = duplicate_entry(
            "b",
            30,
            "smoke_only",
            "smoke only",
            "smoke",
            &["pricing::p"],
            &[("function_argument", "1")],
        );
        b.reasons.push("smoke_oracle_only".to_string());
        let mut entries = vec![a, b];

        let groups = apply_duplicate_discriminator_groups(&mut entries);

        assert_eq!(groups.len(), 1);
        for entry in &entries {
            assert_eq!(entry.class, "duplicative");
            assert!(
                entry.reasons.contains(&"smoke_oracle_only".to_string()),
                "smoke duplicate must retain smoke_oracle_only reason"
            );
            assert!(
                entry
                    .reasons
                    .contains(&"duplicate_activation_and_oracle_shape".to_string()),
                "smoke duplicate must add duplicate reason"
            );
        }
    }

    #[test]
    fn duplicate_grouping_ignores_single_candidate() {
        let mut entries = vec![duplicate_entry(
            "a",
            10,
            "strong_discriminator",
            "exact assertion",
            "strong",
            &["pricing::p"],
            &[("function_argument", "1")],
        )];

        let groups = apply_duplicate_discriminator_groups(&mut entries);

        assert!(groups.is_empty());
        assert_eq!(entries[0].class, "strong_discriminator");
        assert!(entries[0].duplicate_group_id.is_none());
    }

    #[test]
    fn duplicate_grouping_excludes_entries_with_no_activation_literals() {
        let mut entries = vec![
            duplicate_entry(
                "a",
                10,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::p"],
                &[],
            ),
            duplicate_entry(
                "b",
                30,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::p"],
                &[],
            ),
        ];

        let groups = apply_duplicate_discriminator_groups(&mut entries);

        assert!(
            groups.is_empty(),
            "tests with no activation literals must not group; signature would be empty"
        );
    }

    #[test]
    fn duplicate_grouping_assigns_deterministic_group_ids() {
        let build = || {
            vec![
                duplicate_entry(
                    "a",
                    10,
                    "strong_discriminator",
                    "exact assertion",
                    "strong",
                    &["pricing::a"],
                    &[("function_argument", "1")],
                ),
                duplicate_entry(
                    "b",
                    20,
                    "strong_discriminator",
                    "exact assertion",
                    "strong",
                    &["pricing::a"],
                    &[("function_argument", "1")],
                ),
                duplicate_entry(
                    "c",
                    30,
                    "strong_discriminator",
                    "exact assertion",
                    "strong",
                    &["pricing::b"],
                    &[("function_argument", "1")],
                ),
                duplicate_entry(
                    "d",
                    40,
                    "strong_discriminator",
                    "exact assertion",
                    "strong",
                    &["pricing::b"],
                    &[("function_argument", "1")],
                ),
            ]
        };

        let mut first = build();
        let groups_first = apply_duplicate_discriminator_groups(&mut first);
        let mut second = build();
        let groups_second = apply_duplicate_discriminator_groups(&mut second);

        assert_eq!(groups_first.len(), 2);
        assert_eq!(groups_first[0].id, "duplicate_group_1");
        assert_eq!(groups_first[1].id, "duplicate_group_2");
        for (a, b) in groups_first.iter().zip(groups_second.iter()) {
            assert_eq!(a.id, b.id);
            let names_a: Vec<_> = a.members.iter().map(|m| &m.name).collect();
            let names_b: Vec<_> = b.members.iter().map(|m| &m.name).collect();
            assert_eq!(names_a, names_b);
        }
    }

    #[test]
    fn duplicate_grouping_emits_per_test_id_linked_to_top_level_groups() {
        let mut entries = vec![
            duplicate_entry(
                "a",
                10,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::p"],
                &[("function_argument", "1")],
            ),
            duplicate_entry(
                "b",
                30,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::p"],
                &[("function_argument", "1")],
            ),
        ];
        let groups = apply_duplicate_discriminator_groups(&mut entries);
        let json =
            test_efficiency_report_json(&entries, &groups, &TestIntentReportSummary::default());

        assert!(json.contains("\"duplicate_group_id\": \"duplicate_group_1\""));
        assert!(json.contains("\"duplicate_groups\": ["));
        assert!(json.contains("\"id\": \"duplicate_group_1\""));
        assert!(json.contains("\"duplicative\": 2"));
        assert!(json.contains("\"duplicate_activation_and_oracle_shape\": 2"));
    }

    #[test]
    fn duplicate_grouping_markdown_renders_groups_without_deletion_language() {
        let mut entries = vec![
            duplicate_entry(
                "a",
                10,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::p"],
                &[("function_argument", "1")],
            ),
            duplicate_entry(
                "b",
                30,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::p"],
                &[("function_argument", "1")],
            ),
        ];
        let groups = apply_duplicate_discriminator_groups(&mut entries);
        let markdown =
            test_efficiency_report_markdown(&entries, &groups, &TestIntentReportSummary::default());

        assert!(markdown.contains("## Duplicate Discriminator Groups"));
        assert!(markdown.contains("duplicate_group_1"));
        assert!(markdown.contains("Duplicate discriminator groups: 1"));
        assert!(
            !markdown.to_ascii_lowercase().contains("delete"),
            "report must not recommend deleting tests"
        );
    }

    #[test]
    fn metrics_helper_counts_all_seven_classes_with_zero_default() {
        let entries = vec![
            duplicate_entry(
                "strong",
                10,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::p"],
                &[("function_argument", "1")],
            ),
            duplicate_entry(
                "broad",
                20,
                "useful_but_broad",
                "broad predicate",
                "weak",
                &["pricing::p"],
                &[("function_argument", "2")],
            ),
            duplicate_entry("opaque", 30, "opaque", "smoke only", "smoke", &[], &[]),
        ];
        let metrics = test_efficiency_metrics(&entries, &[]);

        assert_eq!(metrics.tests_scanned, 3);
        // Every class is present with a zero default, even unused ones.
        for class in [
            "strong_discriminator",
            "useful_but_broad",
            "smoke_only",
            "likely_vacuous",
            "possibly_circular",
            "duplicative",
            "opaque",
        ] {
            assert!(
                metrics.class_counts.contains_key(class),
                "metrics.class_counts is missing `{class}`"
            );
        }
        assert_eq!(metrics.class_counts["strong_discriminator"], 1);
        assert_eq!(metrics.class_counts["useful_but_broad"], 1);
        assert_eq!(metrics.class_counts["opaque"], 1);
        assert_eq!(metrics.class_counts["smoke_only"], 0);
        assert_eq!(metrics.class_counts["duplicative"], 0);
    }

    #[test]
    fn metrics_helper_counts_reason_strings_from_entries() {
        let mut a = duplicate_entry(
            "a",
            10,
            "useful_but_broad",
            "broad predicate",
            "weak",
            &["pricing::p"],
            &[("function_argument", "1")],
        );
        a.reasons.push("broad_oracle".to_string());
        a.reasons
            .push("assertion_may_not_match_detected_owner".to_string());
        let mut b = duplicate_entry(
            "b",
            20,
            "useful_but_broad",
            "broad predicate",
            "weak",
            &["pricing::p"],
            &[("function_argument", "2")],
        );
        b.reasons.push("broad_oracle".to_string());

        let metrics = test_efficiency_metrics(&[a, b], &[]);

        assert_eq!(metrics.reason_counts["broad_oracle"], 2);
        assert_eq!(
            metrics.reason_counts["assertion_may_not_match_detected_owner"],
            1
        );
        assert!(!metrics.reason_counts.contains_key("nonexistent_reason"));
    }

    #[test]
    fn metrics_helper_distinguishes_duplicative_test_count_from_group_count() {
        // Two disjoint groups, each with two members + one with three members.
        // Total duplicative tests = 2 + 2 + 3 = 7. Total groups = 3.
        let mut entries = Vec::new();
        for owner in &["pricing::a", "pricing::b"] {
            for index in 0..2 {
                entries.push(duplicate_entry(
                    owner,
                    10 + index,
                    "strong_discriminator",
                    "exact assertion",
                    "strong",
                    &[owner],
                    &[("function_argument", "1")],
                ));
            }
        }
        for index in 0..3 {
            entries.push(duplicate_entry(
                "pricing::c",
                100 + index,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::c"],
                &[("function_argument", "9")],
            ));
        }

        let groups = apply_duplicate_discriminator_groups(&mut entries);
        let metrics = test_efficiency_metrics(&entries, &groups);

        assert_eq!(groups.len(), 3, "three distinct duplicate groups");
        assert_eq!(
            metrics.duplicate_discriminator_group_count, 3,
            "duplicate_discriminator_group_count must count groups"
        );
        assert_eq!(
            metrics.class_counts["duplicative"], 7,
            "class_counts.duplicative must count tests, not groups"
        );
    }

    #[test]
    fn metrics_json_contains_metrics_object_with_exact_keys() {
        let entries = vec![duplicate_entry(
            "alone",
            10,
            "strong_discriminator",
            "exact assertion",
            "strong",
            &["pricing::p"],
            &[("function_argument", "1")],
        )];
        let json = test_efficiency_report_json(&entries, &[], &TestIntentReportSummary::default());

        assert!(json.contains("\"metrics\": {"));
        assert!(json.contains("\"tests_scanned\": 1"));
        assert!(json.contains("\"class_counts\": {"));
        // Every required class key is present in the metrics.class_counts.
        for class in [
            "strong_discriminator",
            "useful_but_broad",
            "smoke_only",
            "likely_vacuous",
            "possibly_circular",
            "duplicative",
            "opaque",
        ] {
            assert!(
                json.contains(&format!("\"{class}\":")),
                "metrics JSON is missing class `{class}`"
            );
        }
        assert!(json.contains("\"duplicate_discriminator_group_count\": 0"));
    }

    #[test]
    fn metrics_json_keeps_existing_top_level_counts_and_reason_counts() {
        let mut entry = duplicate_entry(
            "x",
            10,
            "useful_but_broad",
            "broad predicate",
            "weak",
            &["pricing::p"],
            &[("function_argument", "1")],
        );
        entry.reasons.push("broad_oracle".to_string());
        let json = test_efficiency_report_json(&[entry], &[], &TestIntentReportSummary::default());

        // Existing top-level surfaces must remain for backward compatibility.
        assert!(json.contains("\"counts\": {"));
        assert!(json.contains("\"reason_counts\": {"));
        // The new metrics object lives alongside, not instead.
        assert!(json.contains("\"metrics\": {"));
        // Both surfaces report the same broad_oracle count.
        let broad_oracle_count = json.matches("\"broad_oracle\": 1").count();
        assert!(
            broad_oracle_count >= 2,
            "expected `broad_oracle: 1` to appear in both top-level reason_counts and metrics.reason_counts; saw {broad_oracle_count}"
        );
    }

    #[test]
    fn metrics_markdown_contains_metrics_table_with_group_count_separate_from_test_count() {
        let mut entries = vec![
            duplicate_entry(
                "a",
                10,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::p"],
                &[("function_argument", "1")],
            ),
            duplicate_entry(
                "b",
                30,
                "strong_discriminator",
                "exact assertion",
                "strong",
                &["pricing::p"],
                &[("function_argument", "1")],
            ),
        ];
        let groups = apply_duplicate_discriminator_groups(&mut entries);
        let markdown =
            test_efficiency_report_markdown(&entries, &groups, &TestIntentReportSummary::default());

        assert!(markdown.contains("## Metrics"));
        assert!(markdown.contains("| Metric | Value |"));
        assert!(markdown.contains("| Tests scanned | 2 |"));
        assert!(markdown.contains("| Duplicative | 2 |"));
        assert!(
            markdown.contains("| Duplicate discriminator groups | 1 |"),
            "metrics table must distinguish group count from duplicative test count"
        );
    }

    #[test]
    fn metrics_report_uses_only_emitted_vocabulary_strings() {
        let entries = vec![duplicate_entry(
            "x",
            10,
            "strong_discriminator",
            "exact assertion",
            "strong",
            &["pricing::p"],
            &[("function_argument", "1")],
        )];
        let json = test_efficiency_report_json(&entries, &[], &TestIntentReportSummary::default());

        // Aliases that must NOT appear in the metrics surface — ensures we
        // do not introduce a parallel vocabulary.
        for forbidden in [
            "\"broad\":",
            "\"circular\":",
            "\"vacuous\":",
            "\"smoke\":",
            "\"duplicate\":",
            "\"duplicate_discriminator\":",
            "\"discriminator\":",
        ] {
            assert!(
                !json.contains(forbidden),
                "metrics JSON must not contain alias `{forbidden}`"
            );
        }
    }

    #[test]
    fn metrics_report_has_no_coverage_or_denominator_language() {
        let entries = vec![duplicate_entry(
            "x",
            10,
            "strong_discriminator",
            "exact assertion",
            "strong",
            &["pricing::p"],
            &[("function_argument", "1")],
        )];
        let markdown =
            test_efficiency_report_markdown(&entries, &[], &TestIntentReportSummary::default());
        let json = test_efficiency_report_json(&entries, &[], &TestIntentReportSummary::default());

        for body in [&markdown, &json] {
            let lower = body.to_ascii_lowercase();
            assert!(
                !lower.contains("coverage"),
                "test-efficiency report must not use coverage framing"
            );
            // The badge contract is inbox-zero; no denominators in this report.
            assert!(
                !lower.contains("uncovered"),
                "test-efficiency report must not use uncovered framing"
            );
        }
    }

    // -------- test-intent v1 --------

    fn intent_entry(
        name: &str,
        path: &str,
        line: usize,
        class: &'static str,
    ) -> TestEfficiencyEntry {
        TestEfficiencyEntry {
            path: Path::new(path).to_path_buf(),
            name: name.to_string(),
            line,
            class,
            oracle_kind: "exact assertion".to_string(),
            oracle_strength: "strong",
            reached_owners: Vec::new(),
            observed_values: Vec::new(),
            reasons: Vec::new(),
            static_limitations: Vec::new(),
            duplicate_group_id: None,
            declared_intent: None,
        }
    }

    fn declaration(
        test: &str,
        path: Option<&str>,
        intent: TestIntentKind,
    ) -> TestIntentDeclaration {
        TestIntentDeclaration {
            test: test.to_string(),
            path: path.map(|p| p.to_string()),
            intent,
            owner: "team".to_string(),
            reason: "stated reason".to_string(),
            block_line: 10,
        }
    }

    #[test]
    fn test_intent_parses_valid_reasoned_entries() {
        let text = r#"
schema_version = 1

[[test_intent]]
test = "cli_prints_help"
intent = "smoke"
reason = "CLI startup and help text smoke test."
owner = "devtools"

[[test_intent]]
test = "escapes_json"
path = "crates/ripr/src/output/json/mod.rs"
intent = "business_case_duplicate"
reason = "These duplicate-looking tests document distinct escaping cases."
owner = "output"
"#;
        let (entries, violations) = parse_test_intent_manifest(text);
        assert!(
            violations.is_empty(),
            "unexpected violations: {violations:?}"
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].test, "cli_prints_help");
        assert_eq!(entries[0].intent, TestIntentKind::Smoke);
        assert_eq!(entries[0].owner, "devtools");
        assert!(entries[0].path.is_none());
        assert_eq!(entries[1].intent, TestIntentKind::BusinessCaseDuplicate);
        assert_eq!(
            entries[1].path.as_deref(),
            Some("crates/ripr/src/output/json/mod.rs")
        );
    }

    #[test]
    fn test_intent_requires_schema_version() {
        let text = r#"
[[test_intent]]
test = "cli_prints_help"
intent = "smoke"
reason = "CLI startup and help text smoke test."
owner = "devtools"
"#;
        let (_, violations) = parse_test_intent_manifest(text);
        assert!(
            violations.iter().any(|v| v.contains("schema_version = 1")),
            "expected schema_version violation, got {violations:?}"
        );
    }

    #[test]
    fn test_intent_requires_test_field() {
        let text = r#"
schema_version = 1

[[test_intent]]
intent = "smoke"
reason = "..."
owner = "devtools"
"#;
        let (entries, violations) = parse_test_intent_manifest(text);
        assert!(entries.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("missing required `test`")),
            "expected missing-test violation, got {violations:?}"
        );
    }

    #[test]
    fn test_intent_requires_intent_field() {
        let text = r#"
schema_version = 1

[[test_intent]]
test = "x"
reason = "y"
owner = "z"
"#;
        let (entries, violations) = parse_test_intent_manifest(text);
        assert!(entries.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("missing required `intent`")),
            "expected missing-intent violation, got {violations:?}"
        );
    }

    #[test]
    fn test_intent_requires_owner_and_reason() {
        let missing_owner = r#"
schema_version = 1

[[test_intent]]
test = "x"
intent = "smoke"
reason = "y"
"#;
        let (entries, violations) = parse_test_intent_manifest(missing_owner);
        assert!(entries.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("missing required `owner`")),
            "expected missing-owner violation, got {violations:?}"
        );

        let missing_reason = r#"
schema_version = 1

[[test_intent]]
test = "x"
intent = "smoke"
owner = "z"
"#;
        let (entries, violations) = parse_test_intent_manifest(missing_reason);
        assert!(entries.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("missing required `reason`")),
            "expected missing-reason violation, got {violations:?}"
        );
    }

    #[test]
    fn test_intent_rejects_blank_owner_or_reason() {
        let text = r#"
schema_version = 1

[[test_intent]]
test = "x"
intent = "smoke"
owner = "  "
reason = "  "
"#;
        let (entries, violations) = parse_test_intent_manifest(text);
        assert!(entries.is_empty());
        assert!(violations.iter().any(|v| v.contains("`owner` is blank")));
        assert!(violations.iter().any(|v| v.contains("`reason` is blank")));
    }

    #[test]
    fn test_intent_rejects_unknown_intent_value() {
        let text = r#"
schema_version = 1

[[test_intent]]
test = "x"
intent = "vibe_check"
owner = "z"
reason = "y"
"#;
        let (entries, violations) = parse_test_intent_manifest(text);
        assert!(entries.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.contains("unsupported intent `vibe_check`")),
            "expected unsupported-intent violation, got {violations:?}"
        );
    }

    #[test]
    fn test_intent_rejects_unknown_fields() {
        let text = r#"
schema_version = 1

[[test_intent]]
test = "x"
intent = "smoke"
owner = "z"
reason = "y"
priority = "high"
"#;
        let (_, violations) = parse_test_intent_manifest(text);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("unsupported `[[test_intent]]` field `priority`")),
            "expected unknown-field violation, got {violations:?}"
        );
    }

    #[test]
    fn test_intent_rejects_absolute_and_backslash_paths() {
        let abs_unix = r#"
schema_version = 1

[[test_intent]]
test = "x"
path = "/abs/path.rs"
intent = "smoke"
owner = "z"
reason = "y"
"#;
        let (_, violations) = parse_test_intent_manifest(abs_unix);
        assert!(
            violations.iter().any(|v| v.contains("is absolute")),
            "expected absolute-path violation, got {violations:?}"
        );

        let drive = "Z";
        let sep = ":/";
        let win = format!(
            r#"
schema_version = 1

[[test_intent]]
test = "x"
path = "{drive}{sep}abs/path.rs"
intent = "smoke"
owner = "z"
reason = "y"
"#
        );
        let (_, violations) = parse_test_intent_manifest(&win);
        assert!(violations.iter().any(|v| v.contains("is absolute")));

        let backslash = r#"
schema_version = 1

[[test_intent]]
test = "x"
path = "crates\\ripr\\src\\lib.rs"
intent = "smoke"
owner = "z"
reason = "y"
"#;
        let (_, violations) = parse_test_intent_manifest(backslash);
        assert!(
            violations.iter().any(|v| v.contains("backslashes")),
            "expected backslash-path violation, got {violations:?}"
        );
    }

    #[test]
    fn test_intent_rejects_duplicate_selector() {
        let text = r#"
schema_version = 1

[[test_intent]]
test = "cli_prints_help"
intent = "smoke"
owner = "devtools"
reason = "first"

[[test_intent]]
test = "cli_prints_help"
intent = "smoke"
owner = "devtools"
reason = "second"
"#;
        let (_, violations) = parse_test_intent_manifest(text);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("duplicate selector `cli_prints_help`")),
            "expected duplicate-selector violation, got {violations:?}"
        );
    }

    #[test]
    fn test_intent_matches_by_test_name_when_unique() {
        let mut entries = vec![intent_entry(
            "cli_prints_help",
            "tests/cli.rs",
            12,
            "smoke_only",
        )];
        let decls = vec![declaration("cli_prints_help", None, TestIntentKind::Smoke)];

        let violations = apply_test_intent_to_entries(&mut entries, &decls);

        assert!(
            violations.is_empty(),
            "unexpected violations: {violations:?}"
        );
        assert_eq!(
            entries[0].declared_intent.as_ref().map(|i| i.intent),
            Some(TestIntentKind::Smoke)
        );
        assert_eq!(
            entries[0]
                .declared_intent
                .as_ref()
                .map(|i| i.source.as_str()),
            Some(".ripr/test_intent.toml")
        );
    }

    #[test]
    fn test_intent_matches_by_test_name_and_path() {
        let mut entries = vec![
            intent_entry(
                "escapes_json",
                "crates/ripr/src/output/json/mod.rs",
                22,
                "useful_but_broad",
            ),
            intent_entry(
                "escapes_json",
                "crates/ripr/src/output/json/formatter.rs",
                74,
                "useful_but_broad",
            ),
        ];
        let decls = vec![declaration(
            "escapes_json",
            Some("crates/ripr/src/output/json/formatter.rs"),
            TestIntentKind::BusinessCaseDuplicate,
        )];

        let violations = apply_test_intent_to_entries(&mut entries, &decls);

        assert!(
            violations.is_empty(),
            "unexpected violations: {violations:?}"
        );
        assert!(entries[0].declared_intent.is_none());
        assert_eq!(
            entries[1].declared_intent.as_ref().map(|i| i.intent),
            Some(TestIntentKind::BusinessCaseDuplicate)
        );
    }

    #[test]
    fn test_intent_rejects_unmatched_selector() {
        let mut entries = vec![intent_entry("real_test", "tests/a.rs", 1, "smoke_only")];
        let decls = vec![declaration(
            "test_that_does_not_exist",
            None,
            TestIntentKind::Smoke,
        )];

        let violations = apply_test_intent_to_entries(&mut entries, &decls);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("did not match any test"));
        assert!(entries[0].declared_intent.is_none());
    }

    #[test]
    fn test_intent_rejects_ambiguous_name_only_selector() {
        let mut entries = vec![
            intent_entry("escapes_json", "crates/a/mod.rs", 1, "useful_but_broad"),
            intent_entry("escapes_json", "crates/b/mod.rs", 1, "useful_but_broad"),
        ];
        let decls = vec![declaration(
            "escapes_json",
            None,
            TestIntentKind::BusinessCaseDuplicate,
        )];

        let violations = apply_test_intent_to_entries(&mut entries, &decls);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("matched multiple tests"));
        assert!(violations[0].contains("add `path`"));
        assert!(violations[0].contains("crates/a/mod.rs"));
        assert!(violations[0].contains("crates/b/mod.rs"));
        assert!(entries[0].declared_intent.is_none());
        assert!(entries[1].declared_intent.is_none());
    }

    #[test]
    fn test_intent_keeps_declared_tests_visible_with_original_class() {
        let mut entries = vec![intent_entry(
            "cli_prints_help",
            "tests/cli.rs",
            12,
            "smoke_only",
        )];
        let decls = vec![declaration("cli_prints_help", None, TestIntentKind::Smoke)];

        let violations = apply_test_intent_to_entries(&mut entries, &decls);
        assert!(violations.is_empty());

        // Class is preserved — intent is additive metadata, not a replacement.
        assert_eq!(entries[0].class, "smoke_only");
        assert!(entries[0].declared_intent.is_some());
    }

    #[test]
    fn test_intent_json_marks_declared_test_with_full_metadata() {
        let mut entries = vec![intent_entry(
            "cli_prints_help",
            "tests/cli.rs",
            12,
            "smoke_only",
        )];
        let decls = vec![declaration("cli_prints_help", None, TestIntentKind::Smoke)];
        let violations = apply_test_intent_to_entries(&mut entries, &decls);
        assert!(violations.is_empty());

        let json = test_efficiency_report_json(
            &entries,
            &[],
            &TestIntentReportSummary {
                declared: 1,
                matched: 1,
            },
        );

        assert!(json.contains("\"declared_intent\":"));
        assert!(json.contains("\"intent\": \"smoke\""));
        assert!(json.contains("\"owner\": \"team\""));
        assert!(json.contains("\"reason\": \"stated reason\""));
        assert!(json.contains("\"source\": \".ripr/test_intent.toml\""));
        assert!(json.contains("\"test_intent\":"));
        assert!(json.contains("\"declared\": 1"));
        assert!(json.contains("\"matched\": 1"));
        // Original class is preserved.
        assert!(json.contains("\"class\": \"smoke_only\""));
    }

    #[test]
    fn test_intent_markdown_lists_declared_intent_section() {
        let mut entries = vec![intent_entry(
            "cli_prints_help",
            "tests/cli.rs",
            12,
            "smoke_only",
        )];
        let decls = vec![declaration("cli_prints_help", None, TestIntentKind::Smoke)];
        let _ = apply_test_intent_to_entries(&mut entries, &decls);

        let markdown = test_efficiency_report_markdown(
            &entries,
            &[],
            &TestIntentReportSummary {
                declared: 1,
                matched: 1,
            },
        );

        assert!(markdown.contains("## Declared Test Intent"));
        assert!(markdown.contains("declared: 1 \u{00b7} matched: 1"));
        assert!(markdown.contains("`cli_prints_help`"));
        assert!(markdown.contains("`smoke`"));
        assert!(markdown.contains("`team`"));
        assert!(markdown.contains("stated reason"));
    }

    #[test]
    fn test_intent_report_with_no_file_has_zero_declared_summary() {
        let entries: Vec<TestEfficiencyEntry> = Vec::new();
        let summary = TestIntentReportSummary::default();
        assert_eq!(summary.declared, 0);
        assert_eq!(summary.matched, 0);

        let json = test_efficiency_report_json(&entries, &[], &summary);
        assert!(json.contains("\"test_intent\":"));
        assert!(json.contains("\"declared\": 0"));
        assert!(json.contains("\"matched\": 0"));

        let markdown = test_efficiency_report_markdown(&entries, &[], &summary);
        assert!(markdown.contains("## Declared Test Intent"));
        assert!(markdown.contains("None declared."));
    }

    #[test]
    fn test_intent_does_not_mutate_existing_class_strings() {
        let mut entries = vec![
            intent_entry("smoke_test", "tests/a.rs", 1, "smoke_only"),
            intent_entry("dup_test", "tests/b.rs", 1, "duplicative"),
            intent_entry("opaque_test", "tests/c.rs", 1, "opaque"),
        ];
        let decls = vec![
            declaration("smoke_test", None, TestIntentKind::Smoke),
            declaration("dup_test", None, TestIntentKind::BusinessCaseDuplicate),
            declaration("opaque_test", None, TestIntentKind::OpaqueExternalOracle),
        ];
        let violations = apply_test_intent_to_entries(&mut entries, &decls);
        assert!(violations.is_empty());

        assert_eq!(entries[0].class, "smoke_only");
        assert_eq!(entries[1].class, "duplicative");
        assert_eq!(entries[2].class, "opaque");
        for entry in &entries {
            assert!(entry.declared_intent.is_some());
        }

        let intent_strings: Vec<&'static str> = entries
            .iter()
            .filter_map(|e| e.declared_intent.as_ref().map(|i| i.intent.as_str()))
            .collect();
        assert_eq!(
            intent_strings,
            vec!["smoke", "business_case_duplicate", "opaque_external_oracle"]
        );

        let _ = DeclaredIntent {
            intent: TestIntentKind::IntegrationContract,
            owner: "x".to_string(),
            reason: "y".to_string(),
            source: ".ripr/test_intent.toml".to_string(),
        };
    }

    const STUB_RIPR_NATIVE_JSON: &str = r#"{
  "schema_version": "0.1",
  "kind": "ripr",
  "label": "ripr",
  "message": "3",
  "status": "warn",
  "color": "yellow",
  "counts": {
    "unsuppressed_exposure_gaps": 3,
    "unsuppressed_test_efficiency_findings": 0,
    "intentional_test_efficiency_findings": 0,
    "suppressed_exposure_gaps": 1,
    "suppressed_test_efficiency_findings": 0,
    "unknowns": 0,
    "unknowns_test_efficiency": 0,
    "analyzed_findings": 4,
    "analyzed_tests": 0
  },
  "reason_counts": {
    "no_assertion_detected": 0,
    "smoke_oracle_only": 0,
    "relational_oracle": 0,
    "broad_oracle": 0,
    "assertion_may_not_match_detected_owner": 0,
    "opaque_helper_or_fixture_boundary": 0,
    "no_activation_literal_detected": 0,
    "expected_value_computed_from_detected_owner_path": 0,
    "duplicate_activation_and_oracle_shape": 0
  },
  "policy": {
    "include_unknowns": false,
    "fail_on_nonzero": false,
    "test_intent_path": ".ripr/test_intent.toml",
    "suppressions_path": ".ripr/suppressions.toml"
  },
  "warnings": ["first warning", "second warning"]
}"#;

    const STUB_RIPR_PLUS_NATIVE_JSON: &str = r#"{
  "schema_version": "0.1",
  "kind": "ripr_plus",
  "label": "ripr+",
  "message": "7",
  "status": "warn",
  "color": "orange",
  "counts": {
    "unsuppressed_exposure_gaps": 0,
    "unsuppressed_test_efficiency_findings": 7,
    "intentional_test_efficiency_findings": 2,
    "suppressed_exposure_gaps": 0,
    "suppressed_test_efficiency_findings": 1,
    "unknowns": 0,
    "unknowns_test_efficiency": 3,
    "analyzed_findings": 0,
    "analyzed_tests": 308
  },
  "reason_counts": {
    "no_assertion_detected": 1,
    "smoke_oracle_only": 1,
    "relational_oracle": 1,
    "broad_oracle": 4,
    "assertion_may_not_match_detected_owner": 0,
    "opaque_helper_or_fixture_boundary": 3,
    "no_activation_literal_detected": 3,
    "expected_value_computed_from_detected_owner_path": 0,
    "duplicate_activation_and_oracle_shape": 1
  },
  "policy": {
    "include_unknowns": false,
    "fail_on_nonzero": false,
    "test_intent_path": ".ripr/test_intent.toml",
    "suppressions_path": ".ripr/suppressions.toml"
  },
  "warnings": []
}"#;

    #[test]
    fn badge_artifacts_summary_markdown_includes_counts_and_warnings() -> Result<(), String> {
        let markdown =
            badge_artifacts_summary_markdown(STUB_RIPR_NATIVE_JSON, STUB_RIPR_PLUS_NATIVE_JSON);

        let expectations = [
            "## ripr",
            "## ripr+",
            "- message: 3",
            "- message: 7",
            "- color: yellow",
            "- color: orange",
            "  - unsuppressed_exposure_gaps: 3",
            "  - unsuppressed_test_efficiency_findings: 7",
            "  - broad_oracle: 4",
            "  - first warning",
            "  - second warning",
        ];
        for expected in expectations {
            if !markdown.contains(expected) {
                return Err(format!(
                    "expected '{expected}' in markdown, got:\n{markdown}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn badge_artifacts_summary_markdown_omits_forbidden_terms() -> Result<(), String> {
        let markdown =
            badge_artifacts_summary_markdown(STUB_RIPR_NATIVE_JSON, STUB_RIPR_PLUS_NATIVE_JSON);

        let forbidden = ["/", "coverage", "uncovered", "killed", "proven", "adequate"];
        let lower = markdown.to_lowercase();
        for term in forbidden {
            if lower.contains(&term.to_lowercase()) {
                return Err(format!(
                    "forbidden term '{term}' found in markdown:\n{markdown}"
                ));
            }
        }
        // The "survived" check is intentionally substring-strict: the badge
        // schema's `unsuppressed_*` keys legitimately contain neither
        // "killed" nor "survived", but a future schema change could add
        // mutation-runtime language. Guard with a word-boundary heuristic
        // by requiring the leading character to be alphabetic-or-start.
        for forbidden_word in ["survived", "untested"] {
            for window in lower.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if window == forbidden_word {
                    return Err(format!(
                        "forbidden word '{forbidden_word}' found in markdown:\n{markdown}"
                    ));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn badge_artifacts_summary_markdown_is_deterministic_with_sorted_keys() -> Result<(), String> {
        let unsorted_ripr = r#"{
  "label": "ripr",
  "message": "0",
  "color": "brightgreen",
  "counts": {
    "unsuppressed_test_efficiency_findings": 0,
    "analyzed_tests": 0,
    "unsuppressed_exposure_gaps": 0,
    "intentional_test_efficiency_findings": 0,
    "suppressed_exposure_gaps": 0,
    "unknowns": 0,
    "analyzed_findings": 0,
    "suppressed_test_efficiency_findings": 0,
    "unknowns_test_efficiency": 0
  },
  "reason_counts": {
    "smoke_oracle_only": 0,
    "no_assertion_detected": 0,
    "broad_oracle": 0,
    "relational_oracle": 0
  },
  "warnings": []
}"#;
        let unsorted_ripr_plus = r#"{
  "label": "ripr+",
  "message": "0",
  "color": "brightgreen",
  "counts": {
    "analyzed_tests": 308,
    "unsuppressed_test_efficiency_findings": 0,
    "unknowns_test_efficiency": 0,
    "analyzed_findings": 0,
    "suppressed_test_efficiency_findings": 0,
    "unsuppressed_exposure_gaps": 0,
    "unknowns": 0,
    "intentional_test_efficiency_findings": 0,
    "suppressed_exposure_gaps": 0
  },
  "reason_counts": {
    "smoke_oracle_only": 0,
    "broad_oracle": 0,
    "no_assertion_detected": 0,
    "relational_oracle": 0
  },
  "warnings": []
}"#;

        let markdown_a = badge_artifacts_summary_markdown(unsorted_ripr, unsorted_ripr_plus);
        let markdown_b = badge_artifacts_summary_markdown(unsorted_ripr, unsorted_ripr_plus);
        if markdown_a != markdown_b {
            return Err(format!(
                "markdown not deterministic across calls:\nA:\n{markdown_a}\nB:\n{markdown_b}"
            ));
        }

        // Within each `counts:` and `reason_counts:` block, bullet lines
        // (those starting with "  - ") must appear in lexicographic order.
        let mut in_block = false;
        let mut prev: Option<String> = None;
        for line in markdown_a.lines() {
            if line.starts_with("- counts:") || line.starts_with("- reason_counts:") {
                in_block = true;
                prev = None;
                continue;
            }
            if line.starts_with("- ") {
                in_block = false;
                prev = None;
                continue;
            }
            if in_block && line.starts_with("  - ") {
                let curr = line.to_string();
                if let Some(prior) = &prev
                    && prior > &curr
                {
                    return Err(format!(
                        "block keys not lexicographically sorted:\n{prior}\n{curr}"
                    ));
                }
                prev = Some(curr);
            }
        }
        Ok(())
    }

    #[test]
    fn extract_json_string_returns_value_for_present_key() -> Result<(), String> {
        let json = r#"{"label": "ripr", "color": "brightgreen"}"#;
        match extract_json_string(json, "\"color\":") {
            Some(value) if value == "brightgreen" => Ok(()),
            other => Err(format!("expected Some(\"brightgreen\"), got {other:?}")),
        }
    }

    #[test]
    fn extract_json_string_returns_none_for_missing_key() -> Result<(), String> {
        let json = r#"{"label": "ripr"}"#;
        match extract_json_string(json, "\"color\":") {
            None => Ok(()),
            other => Err(format!("expected None, got {other:?}")),
        }
    }

    #[test]
    fn extract_json_string_returns_none_for_unterminated_value() -> Result<(), String> {
        let json = r#"{"label": "ripr"#;
        match extract_json_string(json, "\"label\":") {
            None => Ok(()),
            other => Err(format!(
                "expected None for unterminated value, got {other:?}"
            )),
        }
    }

    #[test]
    fn extract_json_object_usize_map_reads_flat_object() -> Result<(), String> {
        let json = r#"{"counts": {"a": 1, "b": 2, "c": 3}}"#;
        let map = extract_json_object_usize_map(json, "\"counts\":");
        let expected: std::collections::BTreeMap<String, usize> = [("a", 1), ("b", 2), ("c", 3)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        if map == expected {
            Ok(())
        } else {
            Err(format!("expected {expected:?}, got {map:?}"))
        }
    }

    #[test]
    fn extract_json_object_usize_map_returns_empty_for_missing_key() -> Result<(), String> {
        let json = r#"{"label": "ripr"}"#;
        let map = extract_json_object_usize_map(json, "\"counts\":");
        if map.is_empty() {
            Ok(())
        } else {
            Err(format!("expected empty map, got {map:?}"))
        }
    }

    #[test]
    fn extract_json_object_usize_map_skips_non_numeric_values() -> Result<(), String> {
        let json = r#"{"counts": {"a": 1, "b": "two", "c": 3}}"#;
        let map = extract_json_object_usize_map(json, "\"counts\":");
        if map.contains_key("b") {
            return Err(format!(
                "non-numeric value 'two' should be skipped: {map:?}"
            ));
        }
        if map.get("a") != Some(&1) || map.get("c") != Some(&3) {
            return Err(format!("expected a=1 and c=3, got {map:?}"));
        }
        Ok(())
    }

    #[test]
    fn extract_json_object_usize_map_orders_keys_lexicographically() -> Result<(), String> {
        let json = r#"{"counts": {"zeta": 1, "alpha": 2, "mu": 3}}"#;
        let map = extract_json_object_usize_map(json, "\"counts\":");
        let keys: Vec<String> = map.keys().cloned().collect();
        let expected = vec!["alpha".to_string(), "mu".to_string(), "zeta".to_string()];
        if keys == expected {
            Ok(())
        } else {
            Err(format!(
                "expected lexicographic order {expected:?}, got {keys:?}"
            ))
        }
    }

    #[test]
    fn extract_json_warnings_returns_empty_for_empty_array() -> Result<(), String> {
        let json = r#"{"warnings": []}"#;
        let warnings = extract_json_warnings(json);
        if warnings.is_empty() {
            Ok(())
        } else {
            Err(format!("expected empty Vec, got {warnings:?}"))
        }
    }

    #[test]
    fn extract_json_warnings_returns_empty_for_missing_key() -> Result<(), String> {
        let json = r#"{"label": "ripr"}"#;
        let warnings = extract_json_warnings(json);
        if warnings.is_empty() {
            Ok(())
        } else {
            Err(format!("expected empty Vec, got {warnings:?}"))
        }
    }

    #[test]
    fn extract_json_warnings_returns_all_entries() -> Result<(), String> {
        let json = r#"{"warnings": ["first", "second", "third"]}"#;
        let warnings = extract_json_warnings(json);
        let expected = vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ];
        if warnings == expected {
            Ok(())
        } else {
            Err(format!("expected {expected:?}, got {warnings:?}"))
        }
    }

    #[test]
    fn extract_json_warnings_unescapes_backslash_pairs() -> Result<(), String> {
        // The substring extractor consumes the backslash-escape and keeps the
        // following character — so `\"` becomes a literal quote inside the
        // captured warning, and `\\` becomes a single backslash.
        let json = r#"{"warnings": ["with \"quote\"", "with \\path"]}"#;
        let warnings = extract_json_warnings(json);
        let expected = vec!["with \"quote\"".to_string(), "with \\path".to_string()];
        if warnings == expected {
            Ok(())
        } else {
            Err(format!("expected {expected:?}, got {warnings:?}"))
        }
    }

    #[test]
    fn badge_artifact_jobs_has_exactly_four_entries_in_documented_order() -> Result<(), String> {
        let jobs = badge_artifact_jobs();
        let expected = vec![
            BadgeArtifactJob {
                format: "badge-json",
                output_file: "ripr-badge.json",
            },
            BadgeArtifactJob {
                format: "badge-shields",
                output_file: "ripr-badge-shields.json",
            },
            BadgeArtifactJob {
                format: "badge-plus-json",
                output_file: "ripr-plus-badge.json",
            },
            BadgeArtifactJob {
                format: "badge-plus-shields",
                output_file: "ripr-plus-badge-shields.json",
            },
        ];
        if jobs == expected {
            Ok(())
        } else {
            Err(format!("expected {expected:?}, got {jobs:?}"))
        }
    }

    #[test]
    fn badge_artifact_command_args_matches_documented_invocation() -> Result<(), String> {
        let args = badge_artifact_command_args("badge-plus-json");
        let expected: Vec<String> = [
            "run",
            "-p",
            "ripr",
            "--quiet",
            "--",
            "check",
            "--root",
            ".",
            "--diff",
            "target/ripr/badge-input.diff",
            "--format",
            "badge-plus-json",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        if args == expected {
            Ok(())
        } else {
            Err(format!("expected {expected:?}, got {args:?}"))
        }
    }

    #[test]
    fn badge_artifact_command_args_substitutes_format_only() -> Result<(), String> {
        for format in ["badge-json", "badge-shields", "badge-plus-shields"] {
            let args = badge_artifact_command_args(format);
            let last = args.last().cloned().unwrap_or_default();
            if last != format {
                return Err(format!(
                    "expected last arg to be {format:?}, got {last:?} (full args: {args:?})"
                ));
            }
            // The static prefix must be byte-identical across formats.
            let prefix = &args[..args.len() - 1];
            let expected_prefix: Vec<String> = [
                "run",
                "-p",
                "ripr",
                "--quiet",
                "--",
                "check",
                "--root",
                ".",
                "--diff",
                "target/ripr/badge-input.diff",
                "--format",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect();
            if prefix != expected_prefix.as_slice() {
                return Err(format!(
                    "static arg prefix changed for {format:?}: expected {expected_prefix:?}, got {prefix:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn badge_artifact_native_slot_maps_native_formats_only() -> Result<(), String> {
        let cases = [
            ("badge-json", Some(BadgeNativeSlot::Ripr)),
            ("badge-plus-json", Some(BadgeNativeSlot::RiprPlus)),
            ("badge-shields", None),
            ("badge-plus-shields", None),
            ("garbage", None),
        ];
        for (format, expected) in cases {
            let actual = badge_artifact_native_slot(format);
            if actual != expected {
                return Err(format!(
                    "for format {format:?} expected {expected:?}, got {actual:?}"
                ));
            }
        }
        Ok(())
    }

    // ============================================================================
    // Markdown link and local context tests
    // ============================================================================

    #[test]
    fn markdown_links_extracts_relative_targets() {
        let text = "[link](relative/path.md) and [another](../sibling.md)";
        let links = markdown_links_in_text(text);
        assert!(links.iter().any(|l| l.target == "relative/path.md"));
        assert!(links.iter().any(|l| l.target == "../sibling.md"));
    }

    #[test]
    fn markdown_links_extracts_all_link_targets_including_urls() {
        let text = "[http](https://example.com) [anchor](#section) [mail](mailto:test@example.com)";
        let links = markdown_links_in_text(text);
        assert!(links.iter().any(|l| l.target == "https://example.com"));
        assert!(links.iter().any(|l| l.target == "#section"));
        assert!(links.iter().any(|l| l.target == "mailto:test@example.com"));
    }

    #[test]
    fn markdown_links_includes_line_numbers() {
        let text = "Line 1\n[link](target.md)\nLine 3";
        let links = markdown_links_in_text(text);
        assert!(links.iter().any(|l| l.line == 2));
    }

    #[test]
    fn local_markdown_target_filters_absolute_urls() {
        assert_eq!(local_markdown_target("https://example.com"), None);
        assert_eq!(local_markdown_target("http://example.com"), None);
        assert_eq!(local_markdown_target("mailto:test@example.com"), None);
        assert_eq!(local_markdown_target("#anchor"), None);
    }

    #[test]
    fn local_markdown_target_returns_relative_local_paths() {
        let target = local_markdown_target("relative/path.md");
        assert_eq!(target, Some("relative/path.md".to_string()));

        let target = local_markdown_target("../sibling.md");
        assert_eq!(target, Some("../sibling.md".to_string()));
    }

    #[test]
    fn campaign_manifest_parses_valid_file() {
        with_temp_cwd("campaign-manifest", |root| {
            let manifest_path = root.join("campaign.toml");
            write(
                &manifest_path,
                r#"
id = "campaign-01"
title = "Add test coverage"
status = "in_progress"

[[work_item]]
id = "item-1"
status = "ready"
stackable = true
requires_human_merge = false
"#,
            );
            let result = parse_campaign_manifest(&manifest_path);
            assert!(result.is_ok());
            let (manifest, _violations) = result.unwrap();
            assert_eq!(manifest.id, Some("campaign-01".to_string()));
            assert_eq!(manifest.title, Some("Add test coverage".to_string()));
            assert_eq!(manifest.status, Some("in_progress".to_string()));
            assert_eq!(manifest.work_items.len(), 1);
            assert_eq!(manifest.work_items[0].id, Some("item-1".to_string()));
        });
    }

    #[test]
    fn campaign_manifest_reports_violations_for_invalid_file() {
        with_temp_cwd("campaign-invalid", |root| {
            let manifest_path = root.join("campaign.toml");
            write(&manifest_path, "this is not valid toml [ invalid");
            let result = parse_campaign_manifest(&manifest_path);
            // Invalid TOML should return Ok with violations, not an error
            assert!(
                result.is_ok(),
                "invalid TOML should return Ok with violations"
            );
            let (_manifest, violations) = result.unwrap();
            assert!(
                !violations.is_empty(),
                "invalid TOML should produce violations"
            );
        });
    }

    #[test]
    fn local_context_findings_are_sorted_deterministically() {
        let mut findings = [
            LocalContextFinding {
                path: "b.rs".to_string(),
                line: Some(2),
                pattern: "pat".to_string(),
                problem: "prob".to_string(),
            },
            LocalContextFinding {
                path: "a.rs".to_string(),
                line: Some(1),
                pattern: "pat".to_string(),
                problem: "prob".to_string(),
            },
        ];
        findings.sort();
        assert_eq!(findings[0].path, "a.rs");
        assert_eq!(findings[1].path, "b.rs");
    }

    #[test]
    fn local_context_allow_entries_track_path_pattern_and_max_count() {
        let entry = LocalContextAllow {
            path: "crates/ripr/src/analysis/classifier.rs".to_string(),
            pattern: "unwrap()".to_string(),
            max_count: 3,
            line: 42,
        };
        assert_eq!(entry.path, "crates/ripr/src/analysis/classifier.rs");
        assert_eq!(entry.pattern, "unwrap()");
        assert_eq!(entry.max_count, 3);
    }

    // ============================================================================
    // Test efficiency and declared intent tests
    // ============================================================================

    #[test]
    fn declared_intent_structure_holds_intent_and_metadata() {
        let intent = DeclaredIntent {
            intent: TestIntentKind::Smoke,
            owner: "docs".to_string(),
            reason: "example".to_string(),
            source: "docs/example.md".to_string(),
        };
        assert_eq!(intent.intent, TestIntentKind::Smoke);
        assert_eq!(intent.owner, "docs");
        assert_eq!(intent.reason, "example");
    }

    #[test]
    fn test_efficiency_entry_structure_matches_expected_shape() {
        let entry = TestEfficiencyEntry {
            path: PathBuf::from("tests/test.rs"),
            name: "test_foo".to_string(),
            line: 5,
            class: "smoke_only",
            oracle_kind: "assert".to_string(),
            oracle_strength: "smoke",
            reached_owners: vec!["parse".to_string()],
            observed_values: vec![],
            reasons: vec!["no explicit discriminator".to_string()],
            static_limitations: vec![],
            duplicate_group_id: None,
            declared_intent: None,
        };
        assert_eq!(entry.class, "smoke_only");
        assert_eq!(entry.oracle_strength, "smoke");
    }

    #[test]
    fn test_efficiency_value_tracks_context_and_line() {
        let value = TestEfficiencyValue {
            line: 10,
            context: "activation",
            value: "foo()".to_string(),
            text: "let x = foo();".to_string(),
        };
        assert_eq!(value.line, 10);
        assert_eq!(value.context, "activation");
        assert_eq!(value.value, "foo()");
    }

    // ============================================================================
    // Report rendering and status tests
    // ============================================================================

    #[test]
    fn report_index_entries_contain_file_path_and_status() {
        let entry = ReportIndexEntry {
            file: "shape.md".to_string(),
            path: "target/ripr/reports/shape.md".to_string(),
            status: "pass".to_string(),
        };
        assert_eq!(entry.file, "shape.md");
        assert_eq!(entry.path, "target/ripr/reports/shape.md");
        assert_eq!(entry.status, "pass");
    }

    #[test]
    fn report_index_campaign_tracks_id_title_and_ready_items() {
        let campaign = ReportIndexCampaign {
            id: "test-coverage".to_string(),
            title: "Improve test coverage".to_string(),
            status: "in_progress".to_string(),
            ready_work_items: vec!["item-1".to_string(), "item-2".to_string()],
            issues: vec![],
        };
        assert_eq!(campaign.id, "test-coverage");
        assert_eq!(campaign.ready_work_items.len(), 2);
    }

    #[test]
    fn check_status_enum_distinguishes_pass_warn_fail() {
        let statuses = [CheckStatus::Pass, CheckStatus::Warn, CheckStatus::Fail];
        for status in &statuses {
            let debug_str = format!("{:?}", status);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn repo_badge_artifact_jobs_has_exactly_four_entries_with_repo_prefix() -> Result<(), String> {
        let jobs = repo_badge_artifact_jobs();
        let expected = vec![
            BadgeArtifactJob {
                format: "repo-badge-json",
                output_file: "repo-ripr-badge.json",
            },
            BadgeArtifactJob {
                format: "repo-badge-shields",
                output_file: "repo-ripr-badge-shields.json",
            },
            BadgeArtifactJob {
                format: "repo-badge-plus-json",
                output_file: "repo-ripr-plus-badge.json",
            },
            BadgeArtifactJob {
                format: "repo-badge-plus-shields",
                output_file: "repo-ripr-plus-badge-shields.json",
            },
        ];
        if jobs == expected {
            Ok(())
        } else {
            Err(format!("expected {expected:?}, got {jobs:?}"))
        }
    }

    #[test]
    fn check_violation_structure_holds_check_path_and_message() {
        let violation = CheckViolation {
            check: "test-coverage".to_string(),
            path: Some(PathBuf::from("src/lib.rs")),
            line: Some(42),
            severity: CheckStatus::Fail,
            category: "coverage".to_string(),
            message: "Missing test".to_string(),
            why_it_matters: "Ensures code quality".to_string(),
            fix_kind: FixKind::AuthorDecisionRequired,
            suggested_commands: vec!["cargo test".to_string()],
            suggested_patch: Some("patch content".to_string()),
            exception_template: None,
        };
        assert_eq!(violation.check, "test-coverage");
        assert_eq!(violation.line, Some(42));
        assert_eq!(violation.message, "Missing test");
    }

    #[test]
    fn check_report_aggregates_violations_and_status() {
        let report = CheckReport {
            check: "coverage".to_string(),
            status: CheckStatus::Fail,
            violations: vec![],
        };
        assert_eq!(report.check, "coverage");
        assert!(
            matches!(report.status, CheckStatus::Fail),
            "expected Fail status"
        );
    }

    // ============================================================================
    // Identifier validation tests
    // ============================================================================

    #[test]
    fn is_spec_id_matches_expected_format() {
        assert!(is_spec_id("RIPR-SPEC-0001"));
        assert!(is_spec_id("RIPR-SPEC-9999"));
        assert!(is_spec_id("RIPR-SPEC-0000"));

        assert!(!is_spec_id("RIPR-SPEC-001")); // too short
        assert!(!is_spec_id("RIPR-SPEC-00001")); // too long
        assert!(!is_spec_id("RIPR-SPEC-abc1")); // non-digits
        assert!(!is_spec_id("ripr-spec-0001")); // lowercase
        assert!(!is_spec_id("RIPR-0001")); // missing SPEC
        assert!(!is_spec_id(""));
    }

    #[test]
    fn is_snake_case_id_validates_naming_rules() {
        assert!(is_snake_case_id("valid_id"));
        assert!(is_snake_case_id("also_valid_123"));
        assert!(is_snake_case_id("a"));
        assert!(is_snake_case_id("test123test"));

        assert!(!is_snake_case_id("")); // empty
        assert!(!is_snake_case_id("_starts_with")); // starts with underscore
        assert!(!is_snake_case_id("ends_with_")); // ends with underscore
        assert!(!is_snake_case_id("double__underscore")); // double underscore
        assert!(!is_snake_case_id("CamelCase")); // uppercase
        assert!(!is_snake_case_id("with-dash")); // non-alphanumeric
    }

    #[test]
    fn is_bdd_test_name_matches_given_when_then_pattern() {
        assert!(is_bdd_test_name(
            "given_a_user_when_logged_in_then_access_granted"
        ));
        assert!(is_bdd_test_name(
            "Given_Some_Context_When_Something_Happens_Then_Result"
        ));
        assert!(is_bdd_test_name("given_x_when_y_then_z"));

        assert!(!is_bdd_test_name("when_no_given")); // missing given
        assert!(!is_bdd_test_name("given_no_when_then")); // missing when
        assert!(!is_bdd_test_name("given_when_no_then")); // missing then
        assert!(!is_bdd_test_name("given__when__then")); // missing content between parts
        assert!(!is_bdd_test_name("regular_test_name")); // not BDD format
        assert!(!is_bdd_test_name(""));
    }

    #[test]
    fn repo_badge_artifact_command_args_does_not_use_git_diff() -> Result<(), String> {
        // Load-bearing regression: repo-scope artifacts MUST NOT depend on
        // `git diff origin/main...HEAD`. On `main`, that diff is empty, so a
        // diff-driven repo badge would always report 0 regardless of repo
        // state. badge_artifacts_summary_markdown_includes the diff in
        // diff-scope, so this test pins the absence here.
        for format in [
            "repo-badge-json",
            "repo-badge-shields",
            "repo-badge-plus-json",
            "repo-badge-plus-shields",
        ] {
            let args = repo_badge_artifact_command_args(format);
            for arg in &args {
                if arg == "--diff" || arg == "--base" {
                    return Err(format!(
                        "repo-scope command args must not contain `--diff` or `--base` for {format:?}: {args:?}"
                    ));
                }
                if arg.contains("origin/main") || arg.contains("badge-input.diff") {
                    return Err(format!(
                        "repo-scope command args must not reference origin/main or the diff input for {format:?}: {args:?}"
                    ));
                }
            }
            if args.last().map(String::as_str) != Some(format) {
                return Err(format!(
                    "expected last arg to be {format:?}, got {:?}",
                    args.last()
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn diff_badge_artifact_command_args_still_use_git_diff_input() -> Result<(), String> {
        // Companion to the repo regression: the diff-scope contract is that
        // the args DO consult `git diff origin/main...HEAD` (captured to
        // target/ripr/badge-input.diff before render). Pinning this here
        // catches any accidental drift to a unified scope path.
        for format in [
            "badge-json",
            "badge-shields",
            "badge-plus-json",
            "badge-plus-shields",
        ] {
            let args = badge_artifact_command_args(format);
            if !args.iter().any(|arg| arg == "--diff") {
                return Err(format!(
                    "diff-scope command args must contain `--diff` for {format:?}: {args:?}"
                ));
            }
            if !args.iter().any(|arg| arg.contains("badge-input.diff")) {
                return Err(format!(
                    "diff-scope command args must reference badge-input.diff for {format:?}: {args:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn repo_badge_artifact_native_slot_reuses_diff_mapping() -> Result<(), String> {
        // The native-slot mapping is keyed on the format's `*-json` suffix,
        // not on diff-vs-repo prefix; repo formats reuse it so badge_artifacts
        // / repo_badge_artifacts share the same slotting helper.
        let cases = [
            ("repo-badge-json", Some(BadgeNativeSlot::Ripr)),
            ("repo-badge-plus-json", Some(BadgeNativeSlot::RiprPlus)),
            ("repo-badge-shields", None),
            ("repo-badge-plus-shields", None),
        ];
        for (format, expected) in cases {
            let actual = badge_artifact_native_slot(format);
            if actual != expected {
                return Err(format!(
                    "for format {format:?} expected {expected:?}, got {actual:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn repo_badge_artifacts_summary_markdown_carries_baseline_disclaimer() -> Result<(), String> {
        let markdown = repo_badge_artifacts_summary_markdown(
            STUB_RIPR_NATIVE_JSON,
            STUB_RIPR_PLUS_NATIVE_JSON,
        );

        let must_contain = [
            "# ripr repo badges",
            "classified repo seams",
            "not against `git diff origin/main...HEAD`",
            "## ripr",
            "## ripr+",
            "- `repo-ripr-badge.json`",
            "- `repo-ripr-plus-badge-shields.json`",
        ];
        for expected in must_contain {
            if !markdown.contains(expected) {
                return Err(format!(
                    "expected '{expected}' in repo markdown, got:\n{markdown}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn badge_endpoint_files_pair_committed_paths_with_repo_shields_sources() {
        // Pin both the count and the (committed -> source) mapping so a
        // future rename of either side trips the test, not the badge URL.
        assert_eq!(super::BADGE_ENDPOINT_FILES.len(), 2);
        let pairs: Vec<(&str, &str)> = super::BADGE_ENDPOINT_FILES.to_vec();
        assert!(pairs.contains(&("badges/ripr.json", "repo-ripr-badge-shields.json")));
        assert!(pairs.contains(&("badges/ripr-plus.json", "repo-ripr-plus-badge-shields.json")));
    }

    #[test]
    fn badge_endpoint_violation_reports_missing_committed_file() -> Result<(), String> {
        let violation = super::badge_endpoint_violation(
            "badges/ripr.json",
            "target/ripr/reports/repo-ripr-badge-shields.json",
            b"{\"schemaVersion\":1}",
            None,
        );
        let message =
            violation.ok_or_else(|| "missing file should produce a violation".to_string())?;
        assert!(
            message.contains("missing badge endpoint file badges/ripr.json"),
            "violation should name the missing file: {message}"
        );
        assert!(
            message.contains("cargo xtask update-badge-endpoints"),
            "violation should point at the refresh command: {message}"
        );
        Ok(())
    }

    #[test]
    fn badge_endpoint_violation_reports_stale_committed_file() -> Result<(), String> {
        let violation = super::badge_endpoint_violation(
            "badges/ripr-plus.json",
            "target/ripr/reports/repo-ripr-plus-badge-shields.json",
            b"{\"schemaVersion\":1,\"label\":\"ripr+\",\"message\":\"163\",\"color\":\"orange\"}",
            Some(b"{\"schemaVersion\":1,\"label\":\"ripr+\",\"message\":\"317\",\"color\":\"orange\"}"),
        );
        let message =
            violation.ok_or_else(|| "stale file should produce a violation".to_string())?;
        assert!(
            message.contains("badges/ripr-plus.json is stale"),
            "violation should describe the stale committed path: {message}"
        );
        assert!(
            message.contains("repo-ripr-plus-badge-shields.json"),
            "violation should name the source-of-truth file: {message}"
        );
        assert!(
            message.contains("cargo xtask update-badge-endpoints"),
            "violation should point at the refresh command: {message}"
        );
        assert!(
            message.contains("commit the diff"),
            "violation should remind the author to commit: {message}"
        );
        Ok(())
    }

    #[test]
    fn badge_endpoint_violation_returns_none_when_committed_file_matches() {
        let bytes =
            b"{\"schemaVersion\":1,\"label\":\"ripr\",\"message\":\"0\",\"color\":\"brightgreen\"}";
        let violation = super::badge_endpoint_violation(
            "badges/ripr.json",
            "target/ripr/reports/repo-ripr-badge-shields.json",
            bytes,
            Some(bytes),
        );
        assert!(
            violation.is_none(),
            "matching content must not produce a violation: {violation:?}"
        );
    }

    fn badge_endpoint_tempdir(label: &str) -> Result<std::path::PathBuf, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("ripr-badge-endpoint-{label}-{stamp}-{pid}"));
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir {label}: {err}"))?;
        Ok(dir)
    }

    fn write_fixture_shields(dir: &std::path::Path, name: &str, body: &[u8]) -> Result<(), String> {
        std::fs::write(dir.join(name), body).map_err(|err| format!("write {name}: {err}"))
    }

    #[test]
    fn copy_badge_endpoints_from_reports_writes_both_files() -> Result<(), String> {
        let reports = badge_endpoint_tempdir("copy-reports")?;
        let repo_root = badge_endpoint_tempdir("copy-root")?;
        write_fixture_shields(
            &reports,
            "repo-ripr-badge-shields.json",
            b"{\"schemaVersion\":1,\"label\":\"ripr\",\"message\":\"7\",\"color\":\"yellow\"}",
        )?;
        write_fixture_shields(
            &reports,
            "repo-ripr-plus-badge-shields.json",
            b"{\"schemaVersion\":1,\"label\":\"ripr+\",\"message\":\"7\",\"color\":\"yellow\"}",
        )?;

        super::copy_badge_endpoints_from_reports(&reports, &repo_root)?;

        let ripr = std::fs::read(repo_root.join("badges/ripr.json"))
            .map_err(|err| format!("read written ripr.json: {err}"))?;
        let ripr_plus = std::fs::read(repo_root.join("badges/ripr-plus.json"))
            .map_err(|err| format!("read written ripr-plus.json: {err}"))?;
        assert_eq!(
            ripr,
            b"{\"schemaVersion\":1,\"label\":\"ripr\",\"message\":\"7\",\"color\":\"yellow\"}"
        );
        assert_eq!(
            ripr_plus,
            b"{\"schemaVersion\":1,\"label\":\"ripr+\",\"message\":\"7\",\"color\":\"yellow\"}"
        );

        let _ = std::fs::remove_dir_all(&reports);
        let _ = std::fs::remove_dir_all(&repo_root);
        Ok(())
    }

    #[test]
    fn copy_badge_endpoints_from_reports_creates_badges_dir_when_missing() -> Result<(), String> {
        let reports = badge_endpoint_tempdir("copy-mkdir-reports")?;
        let repo_root = badge_endpoint_tempdir("copy-mkdir-root")?;
        write_fixture_shields(
            &reports,
            "repo-ripr-badge-shields.json",
            b"{\"schemaVersion\":1,\"label\":\"ripr\",\"message\":\"0\",\"color\":\"brightgreen\"}",
        )?;
        write_fixture_shields(
            &reports,
            "repo-ripr-plus-badge-shields.json",
            b"{\"schemaVersion\":1,\"label\":\"ripr+\",\"message\":\"0\",\"color\":\"brightgreen\"}",
        )?;
        // badges/ subdir does not exist yet; copy_badge_endpoints_from_reports must create it.
        assert!(!repo_root.join("badges").exists());

        super::copy_badge_endpoints_from_reports(&reports, &repo_root)?;

        assert!(repo_root.join("badges").is_dir());
        assert!(repo_root.join("badges/ripr.json").exists());
        assert!(repo_root.join("badges/ripr-plus.json").exists());

        let _ = std::fs::remove_dir_all(&reports);
        let _ = std::fs::remove_dir_all(&repo_root);
        Ok(())
    }

    #[test]
    fn copy_badge_endpoints_from_reports_errors_when_source_missing() -> Result<(), String> {
        let reports = badge_endpoint_tempdir("copy-err-reports")?;
        let repo_root = badge_endpoint_tempdir("copy-err-root")?;
        // Write only the ripr source; ripr-plus is missing.
        write_fixture_shields(
            &reports,
            "repo-ripr-badge-shields.json",
            b"{\"schemaVersion\":1}",
        )?;

        let result = super::copy_badge_endpoints_from_reports(&reports, &repo_root);
        let err = result.err().ok_or_else(|| {
            "missing source must produce an error from copy_badge_endpoints_from_reports"
                .to_string()
        })?;
        assert!(
            err.contains("repo-ripr-plus-badge-shields.json"),
            "error should name the missing source file: {err}"
        );
        assert!(
            err.contains("cargo xtask repo-badge-artifacts"),
            "error should suggest regenerating the source: {err}"
        );

        let _ = std::fs::remove_dir_all(&reports);
        let _ = std::fs::remove_dir_all(&repo_root);
        Ok(())
    }

    #[test]
    fn compute_badge_endpoint_violations_returns_empty_when_in_sync() -> Result<(), String> {
        let reports = badge_endpoint_tempdir("compute-sync-reports")?;
        let repo_root = badge_endpoint_tempdir("compute-sync-root")?;
        let ripr_body =
            b"{\"schemaVersion\":1,\"label\":\"ripr\",\"message\":\"3\",\"color\":\"yellow\"}";
        let plus_body =
            b"{\"schemaVersion\":1,\"label\":\"ripr+\",\"message\":\"3\",\"color\":\"yellow\"}";
        write_fixture_shields(&reports, "repo-ripr-badge-shields.json", ripr_body)?;
        write_fixture_shields(&reports, "repo-ripr-plus-badge-shields.json", plus_body)?;
        std::fs::create_dir_all(repo_root.join("badges"))
            .map_err(|err| format!("mkdir badges: {err}"))?;
        std::fs::write(repo_root.join("badges/ripr.json"), ripr_body)
            .map_err(|err| format!("write committed ripr.json: {err}"))?;
        std::fs::write(repo_root.join("badges/ripr-plus.json"), plus_body)
            .map_err(|err| format!("write committed ripr-plus.json: {err}"))?;

        let violations = super::compute_badge_endpoint_violations(&reports, &repo_root)?;
        assert!(
            violations.is_empty(),
            "in-sync committed files must produce no violations: {violations:?}"
        );

        let _ = std::fs::remove_dir_all(&reports);
        let _ = std::fs::remove_dir_all(&repo_root);
        Ok(())
    }

    #[test]
    fn compute_badge_endpoint_violations_flags_missing_committed_file() -> Result<(), String> {
        let reports = badge_endpoint_tempdir("compute-missing-reports")?;
        let repo_root = badge_endpoint_tempdir("compute-missing-root")?;
        write_fixture_shields(
            &reports,
            "repo-ripr-badge-shields.json",
            b"{\"schemaVersion\":1,\"label\":\"ripr\",\"message\":\"0\",\"color\":\"brightgreen\"}",
        )?;
        write_fixture_shields(
            &reports,
            "repo-ripr-plus-badge-shields.json",
            b"{\"schemaVersion\":1,\"label\":\"ripr+\",\"message\":\"0\",\"color\":\"brightgreen\"}",
        )?;
        // No committed badges/*.json files exist on the temp repo root.

        let violations = super::compute_badge_endpoint_violations(&reports, &repo_root)?;
        assert_eq!(
            violations.len(),
            2,
            "both committed files missing must flag both: {violations:?}"
        );
        for violation in &violations {
            assert!(
                violation.contains("missing badge endpoint file"),
                "violation should be the missing-file shape: {violation}"
            );
        }

        let _ = std::fs::remove_dir_all(&reports);
        let _ = std::fs::remove_dir_all(&repo_root);
        Ok(())
    }

    #[test]
    fn compute_badge_endpoint_violations_flags_stale_committed_file() -> Result<(), String> {
        let reports = badge_endpoint_tempdir("compute-stale-reports")?;
        let repo_root = badge_endpoint_tempdir("compute-stale-root")?;
        let want =
            b"{\"schemaVersion\":1,\"label\":\"ripr\",\"message\":\"5\",\"color\":\"orange\"}";
        let stale =
            b"{\"schemaVersion\":1,\"label\":\"ripr\",\"message\":\"99\",\"color\":\"orange\"}";
        write_fixture_shields(&reports, "repo-ripr-badge-shields.json", want)?;
        write_fixture_shields(
            &reports,
            "repo-ripr-plus-badge-shields.json",
            b"{\"schemaVersion\":1,\"label\":\"ripr+\",\"message\":\"5\",\"color\":\"orange\"}",
        )?;
        std::fs::create_dir_all(repo_root.join("badges"))
            .map_err(|err| format!("mkdir badges: {err}"))?;
        // ripr.json is stale; ripr-plus.json is in sync.
        std::fs::write(repo_root.join("badges/ripr.json"), stale)
            .map_err(|err| format!("write stale ripr.json: {err}"))?;
        std::fs::write(
            repo_root.join("badges/ripr-plus.json"),
            b"{\"schemaVersion\":1,\"label\":\"ripr+\",\"message\":\"5\",\"color\":\"orange\"}",
        )
        .map_err(|err| format!("write fresh ripr-plus.json: {err}"))?;

        let violations = super::compute_badge_endpoint_violations(&reports, &repo_root)?;
        assert_eq!(violations.len(), 1, "exactly one stale: {violations:?}");
        assert!(
            violations[0].contains("badges/ripr.json is stale"),
            "violation should name the stale file: {}",
            violations[0]
        );

        let _ = std::fs::remove_dir_all(&reports);
        let _ = std::fs::remove_dir_all(&repo_root);
        Ok(())
    }

    #[test]
    fn compute_badge_endpoint_violations_errors_when_source_missing() -> Result<(), String> {
        let reports = badge_endpoint_tempdir("compute-noreport-reports")?;
        let repo_root = badge_endpoint_tempdir("compute-noreport-root")?;
        // No source files at all.

        let result = super::compute_badge_endpoint_violations(&reports, &repo_root);
        let err = result.err().ok_or_else(|| {
            "missing reports directory contents should produce a hard error".to_string()
        })?;
        assert!(
            err.contains("failed to read"),
            "error should describe the read failure: {err}"
        );

        let _ = std::fs::remove_dir_all(&reports);
        let _ = std::fs::remove_dir_all(&repo_root);
        Ok(())
    }

    #[test]
    fn badge_endpoint_violation_treats_byte_diff_as_stale() {
        // Even a single-byte difference (e.g. trailing newline) trips the
        // staleness check so committed files stay byte-equal with the
        // source-of-truth Shields JSON.
        let want = b"{\"schemaVersion\":1,\"label\":\"ripr\",\"message\":\"0\",\"color\":\"brightgreen\"}\n";
        let got =
            b"{\"schemaVersion\":1,\"label\":\"ripr\",\"message\":\"0\",\"color\":\"brightgreen\"}";
        let violation = super::badge_endpoint_violation(
            "badges/ripr.json",
            "target/ripr/reports/repo-ripr-badge-shields.json",
            want,
            Some(got),
        );
        assert!(
            violation.is_some(),
            "trailing-byte difference must be flagged as stale"
        );
    }

    #[test]
    fn repo_badge_artifacts_summary_markdown_omits_forbidden_terms() -> Result<(), String> {
        let markdown = repo_badge_artifacts_summary_markdown(
            STUB_RIPR_NATIVE_JSON,
            STUB_RIPR_PLUS_NATIVE_JSON,
        );

        // Repo badge output is public-facing: it must not borrow runtime
        // mutation language ("killed", "survived", "proven", "adequate")
        // and must not pretend to be a coverage metric.
        let lower = markdown.to_lowercase();
        for term in ["coverage", "uncovered", "killed", "proven", "adequate"] {
            if lower.contains(term) {
                return Err(format!(
                    "forbidden term '{term}' found in repo markdown:\n{markdown}"
                ));
            }
        }
        for forbidden_word in ["survived", "untested"] {
            for window in lower.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if window == forbidden_word {
                    return Err(format!(
                        "forbidden word '{forbidden_word}' found in repo markdown:\n{markdown}"
                    ));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn strip_yaml_comment_removes_trailing_comment() {
        assert_eq!(strip_yaml_comment("key: value # comment"), "key: value ");
    }

    #[test]
    fn strip_yaml_comment_preserves_hash_in_strings() {
        assert_eq!(
            strip_yaml_comment("key: \"value#with hash\""),
            "key: \"value#with hash\""
        );
        assert_eq!(
            strip_yaml_comment("key: 'value#with hash'"),
            "key: 'value#with hash'"
        );
    }

    #[test]
    fn strip_yaml_comment_handles_full_comment_line() {
        assert_eq!(strip_yaml_comment("# this is a comment"), "");
    }

    #[test]
    fn strip_yaml_comment_preserves_line_without_comment() {
        assert_eq!(
            strip_yaml_comment("review_model: \"custom:MiniMax-M2.7-0\""),
            "review_model: \"custom:MiniMax-M2.7-0\""
        );
    }

    #[test]
    fn strip_yaml_comment_handles_escaped_quote() {
        assert_eq!(
            strip_yaml_comment("key: \"value\\\" # not a comment"),
            "key: \"value\\\" # not a comment"
        );
    }

    #[test]
    fn strip_yaml_comment_handles_double_backslash_before_quote() {
        assert_eq!(
            strip_yaml_comment("key: \"value\\\\\" # real comment"),
            "key: \"value\\\\\" "
        );
    }

    #[test]
    fn active_yaml_lines_strips_comments_and_blanks() {
        let input = "\
# commented out
key: value
  # indented comment

active: true
";
        let lines = active_yaml_lines(input);
        assert_eq!(lines, vec!["key: value", "active: true"]);
    }

    #[test]
    fn has_active_line_finds_active_content() {
        let lines = active_yaml_lines("review_model: \"custom:MiniMax-M2.7-0\"");
        assert!(has_active_line(
            &lines,
            "review_model: \"custom:MiniMax-M2.7-0\""
        ));
    }

    #[test]
    fn has_active_line_ignores_commented_content() {
        let lines = active_yaml_lines("# review_model: \"custom:MiniMax-M2.7-0\"");
        assert!(!has_active_line(
            &lines,
            "review_model: \"custom:MiniMax-M2.7-0\""
        ));
    }

    #[test]
    fn forbids_active_line_catches_active_forbidden_content() {
        let lines = active_yaml_lines("settings: |");
        assert!(forbids_active_line(&lines, "settings:"));
    }

    #[test]
    fn forbids_active_line_ignores_commented_forbidden_content() {
        let lines = active_yaml_lines("# settings: |");
        assert!(!forbids_active_line(&lines, "settings:"));
    }

    #[test]
    fn check_droid_action_refs_rejects_non_sha_refs() {
        let mut violations = Vec::new();
        check_droid_action_refs(
            &mut violations,
            "test.yml",
            "      - uses: Factory-AI/droid-action@v5\n",
        );
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("immutable commit SHA"));
    }

    #[test]
    fn check_droid_action_refs_accepts_sha_refs() {
        let mut violations = Vec::new();
        check_droid_action_refs(
            &mut violations,
            "test.yml",
            "      - uses: Factory-AI/droid-action@e3d1f5e7861c36fe4a9c4dca3edec87b964b2bc4\n",
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn check_droid_action_refs_ignores_commented_uses() {
        let mut violations = Vec::new();
        check_droid_action_refs(
            &mut violations,
            "test.yml",
            "      # - uses: Factory-AI/droid-action@v5\n",
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn check_droid_action_refs_rejects_main_ref() {
        let mut violations = Vec::new();
        check_droid_action_refs(
            &mut violations,
            "test.yml",
            "      - uses: Factory-AI/droid-action@main\n",
        );
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn check_droid_common_flags_missing_same_repo_guard() {
        let mut violations = Vec::new();
        let yaml = "\
review_model: \"custom:MiniMax-M2.7-0\"
security_model: \"custom:MiniMax-M2.7-0\"
";
        check_droid_common(&mut violations, "test.yml", yaml);
        assert!(violations.iter().any(|v| v.contains("same-repo guard")));
    }

    #[test]
    fn check_droid_common_flags_anthropic_tokens_on_active_lines_only() {
        let mut violations = Vec::new();
        let yaml = "\
# ANTHROPIC_AUTH_TOKEN: something
review_model: \"custom:MiniMax-M2.7-0\"
security_model: \"custom:MiniMax-M2.7-0\"
";
        check_droid_common(&mut violations, "test.yml", yaml);
        assert!(!violations.iter().any(|v| v.contains("ANTHROPIC")));
    }

    #[test]
    fn check_droid_common_flags_active_anthropic_tokens() {
        let mut violations = Vec::new();
        let yaml = "\
ANTHROPIC_AUTH_TOKEN: something
review_model: \"custom:MiniMax-M2.7-0\"
security_model: \"custom:MiniMax-M2.7-0\"
";
        check_droid_common(&mut violations, "test.yml", yaml);
        assert!(violations.iter().any(|v| v.contains("ANTHROPIC")));
    }

    #[test]
    fn check_droid_common_flags_settings_input_only_when_active() {
        let mut violations = Vec::new();
        let yaml = "\
# settings: |
review_model: \"custom:MiniMax-M2.7-0\"
";
        check_droid_common(&mut violations, "test.yml", yaml);
        assert!(!violations.iter().any(|v| v.contains("settings:")));
    }

    #[test]
    fn check_droid_common_flags_active_settings_input() {
        let mut violations = Vec::new();
        let yaml = "\
settings: |
  some: config
";
        check_droid_common(&mut violations, "test.yml", yaml);
        assert!(violations.iter().any(|v| v.contains("settings:")));
    }

    #[test]
    fn check_droid_common_flags_show_full_output_only_when_active() {
        let mut violations = Vec::new();
        let yaml = "# show_full_output: true\n";
        check_droid_common(&mut violations, "test.yml", yaml);
        assert!(!violations.iter().any(|v| v.contains("show_full_output")));
    }

    #[test]
    fn check_droid_common_flags_missing_review_model() {
        let mut violations = Vec::new();
        let yaml = "security_model: \"custom:MiniMax-M2.7-0\"\n";
        check_droid_common(&mut violations, "test.yml", yaml);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("review_model must be custom:MiniMax-M2.7-0"))
        );
    }

    #[test]
    fn check_droid_common_flags_missing_security_model() {
        let mut violations = Vec::new();
        let yaml = "review_model: \"custom:MiniMax-M2.7-0\"\n";
        check_droid_common(&mut violations, "test.yml", yaml);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("security_model must be custom:MiniMax-M2.7-0"))
        );
    }

    #[test]
    fn check_droid_common_flags_missing_settings_local_json() {
        let mut violations = Vec::new();
        let yaml = "review_model: \"custom:MiniMax-M2.7-0\"\n";
        check_droid_common(&mut violations, "test.yml", yaml);
        assert!(violations.iter().any(|v| v.contains("settings.local.json")));
    }

    #[test]
    fn check_droid_common_flags_missing_literal_minimax_key() {
        let mut violations = Vec::new();
        let yaml = "review_model: \"custom:MiniMax-M2.7-0\"\n";
        check_droid_common(&mut violations, "test.yml", yaml);
        assert!(violations.iter().any(|v| v.contains("${MINIMAX_API_KEY}")));
    }

    #[test]
    fn check_droid_common_flags_active_anthropic_base_url() {
        let mut violations = Vec::new();
        let yaml = "ANTHROPIC_BASE_URL: https://example.com\n";
        check_droid_common(&mut violations, "test.yml", yaml);
        assert!(violations.iter().any(|v| v.contains("ANTHROPIC")));
    }

    #[test]
    fn check_droid_common_flags_active_show_full_output() {
        let mut violations = Vec::new();
        let yaml = "show_full_output: true\n";
        check_droid_common(&mut violations, "test.yml", yaml);
        assert!(violations.iter().any(|v| v.contains("show_full_output")));
    }

    #[test]
    fn check_droid_action_refs_rejects_short_sha() {
        let mut violations = Vec::new();
        check_droid_action_refs(
            &mut violations,
            "test.yml",
            "      - uses: actions/checkout@abc123\n",
        );
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("immutable commit SHA"));
    }

    #[test]
    fn check_droid_action_refs_handles_dash_uses_with_non_sha() {
        let mut violations = Vec::new();
        check_droid_action_refs(&mut violations, "test.yml", "- uses: some/action@v1.2.3\n");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn check_droid_action_refs_handles_bare_uses_with_sha() {
        let mut violations = Vec::new();
        check_droid_action_refs(
            &mut violations,
            "test.yml",
            "        uses: actions/checkout@93cb6efe18208431cddfb8368fd83d5badbf9bfd\n",
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn check_droid_action_refs_ignores_line_without_at_sign() {
        let mut violations = Vec::new();
        check_droid_action_refs(&mut violations, "test.yml", "      - uses: local-action\n");
        assert!(violations.is_empty());
    }

    #[test]
    fn check_droid_action_refs_strips_inline_comment_before_checking() {
        let mut violations = Vec::new();
        check_droid_action_refs(
            &mut violations,
            "test.yml",
            "      - uses: actions/checkout@v5 # old ref\n",
        );
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn unknown_command_message_suggests_nearest_match() {
        let message = unknown_command_message("chek-pr");
        assert!(message.contains("Did you mean `check-pr`?"));
    }

    #[test]
    fn unknown_command_message_includes_help_without_nearby_match() {
        let message = unknown_command_message("totally-unknown-command");
        assert!(!message.contains("Did you mean"));
        assert!(message.contains("cargo xtask help"));
    }

    #[test]
    fn xtask_command_parse_preserves_subcommand_arguments() {
        assert_eq!(
            XtaskCommand::parse([
                "goldens".to_string(),
                "bless".to_string(),
                "boundary_gap".to_string(),
            ]),
            XtaskCommand::Goldens(vec!["bless".to_string(), "boundary_gap".to_string()])
        );
        assert_eq!(
            XtaskCommand::parse(["fixtures".to_string(), "boundary_gap".to_string()]),
            XtaskCommand::Fixtures(Some("boundary_gap".to_string()))
        );
    }

    #[test]
    fn xtask_command_parse_preserves_compatibility_aliases() {
        assert_eq!(
            XtaskCommand::parse(["check-test-oracles".to_string()]),
            XtaskCommand::TestOracleReport
        );
        assert_eq!(
            XtaskCommand::parse(["check-spec-ids".to_string()]),
            XtaskCommand::CheckTraceability
        );
        assert_eq!(
            XtaskCommand::parse(["check-goals".to_string()]),
            XtaskCommand::CheckCampaign
        );
        assert_eq!(
            XtaskCommand::parse(["operator-cockpit".to_string()]),
            XtaskCommand::OperatorCockpitReport
        );
        assert_eq!(
            XtaskCommand::parse(["operator-cockpit-report".to_string()]),
            XtaskCommand::OperatorCockpitReport
        );
        assert_eq!(
            XtaskCommand::parse(std::iter::empty::<String>()),
            XtaskCommand::Help
        );
    }

    #[test]
    fn report_commands_dispatch_through_report_facades() -> Result<(), String> {
        with_temp_cwd("report-dispatch", |_| {
            let commands = vec![
                XtaskCommand::PrSummary,
                XtaskCommand::Fixtures(Some("missing".to_string())),
                XtaskCommand::Goldens(vec!["unknown".to_string()]),
                XtaskCommand::Metrics,
                XtaskCommand::TestOracleReport,
                XtaskCommand::TestEfficiencyReport,
                XtaskCommand::BadgeArtifacts,
                XtaskCommand::RepoBadgeArtifacts,
                XtaskCommand::RepoSeamInventory,
                XtaskCommand::RepoExposureReport,
                XtaskCommand::RepoExposureLatencyReport,
                XtaskCommand::AgentSeamPackets(Some(".".to_string())),
                XtaskCommand::LspCockpitReport,
                XtaskCommand::OperatorCockpitReport,
                XtaskCommand::TargetedTestOutcome(Vec::new()),
                XtaskCommand::MutationCalibration(Vec::new()),
                XtaskCommand::SarifPolicy(Vec::new()),
                XtaskCommand::UpdateBadgeEndpoints,
                XtaskCommand::CheckBadgeEndpoints,
                XtaskCommand::Dogfood,
                XtaskCommand::Critic,
                XtaskCommand::Reports(vec!["index".to_string()]),
                XtaskCommand::Receipts(Vec::new()),
                XtaskCommand::GoldenDrift,
            ];

            for command in commands {
                let label = format!("{command:?}");
                match dispatch::execute(command) {
                    Ok(()) => {}
                    Err(message) if !message.is_empty() => {}
                    Err(_) => {
                        return Err(format!(
                            "{label} should either succeed or return an actionable error"
                        ));
                    }
                }
            }

            Ok(())
        })
    }

    #[test]
    fn xtask_run_helpers_report_success_failure_and_optional_output() -> Result<(), String> {
        let version = run_output("cargo", &["--version"])?;
        if !version.contains("cargo") {
            return Err(format!("expected cargo version output, got {version:?}"));
        }

        let owned_version = run_output_owned("cargo", &["--version".to_string()])?;
        if !owned_version.contains("cargo") {
            return Err(format!(
                "expected owned cargo version output, got {owned_version:?}"
            ));
        }

        let status = run("cargo", &["--version"])?;
        if !status.success() {
            return Err(format!("expected cargo --version to succeed, got {status}"));
        }

        let captured = capture_output("cargo", &["--version"], "cargo version")?;
        if !captured.status.success() || !captured.stdout.contains("cargo") {
            return Err(format!(
                "expected captured cargo version output, got status={} stdout={:?}",
                captured.status, captured.stdout
            ));
        }

        let failed_capture = capture_output(
            "cargo",
            &["--definitely-not-a-real-cargo-flag"],
            "cargo invalid flag",
        )?;
        if failed_capture.status.success() {
            return Err("expected invalid cargo flag to fail".to_string());
        }

        let optional = run_output_optional("cargo", &["--definitely-not-a-real-cargo-flag"])?;
        if !optional.is_empty() {
            return Err(format!(
                "expected optional failure to return empty output, got {optional:?}"
            ));
        }

        let failure = run_output("cargo", &["--definitely-not-a-real-cargo-flag"]).is_err();
        if !failure {
            return Err("expected run_output to report non-zero exit".to_string());
        }

        let owned_failure =
            run_output_owned("cargo", &["--definitely-not-a-real-cargo-flag".to_string()]).is_err();
        if !owned_failure {
            return Err("expected run_output_owned to report non-zero exit".to_string());
        }

        let missing_program = capture_output(
            "definitely-missing-ripr-test-binary",
            &[],
            "missing test binary",
        );
        if missing_program.is_ok() {
            return Err("expected missing executable to report spawn error".to_string());
        }

        Ok(())
    }

    #[test]
    fn policy_checker_facade_runs_current_repo_checks() -> Result<(), String> {
        with_repo_cwd(|| {
            check_static_language()?;
            check_no_panic_family()?;
            check_allow_attributes()?;
            check_local_context()?;
            check_file_policy()?;
            check_executable_files()?;
            check_workflows()?;
            check_droid_review_config()?;
            check_process_policy()?;
            check_network_policy()
        })
    }

    #[test]
    fn known_xtask_command_accepts_every_help_catalog_root() {
        let commands = known_commands();
        assert!(
            commands
                .iter()
                .map(|command| command.split_once(' ').map_or(*command, |(root, _)| root))
                .all(known_xtask_command)
        );
    }

    #[test]
    fn known_commands_has_no_duplicate_entries() {
        let commands = known_commands();
        let unique = commands.iter().collect::<BTreeSet<_>>();
        assert_eq!(commands.len(), unique.len());
    }

    #[test]
    fn known_commands_include_current_report_and_policy_commands() {
        let commands = known_commands();
        assert!(commands.contains(&"install-hooks"));
        assert!(commands.contains(&"repo-seam-inventory"));
        assert!(commands.contains(&"repo-exposure-report"));
        assert!(commands.contains(&"repo-exposure-latency-report"));
        assert!(commands.contains(&"agent-seam-packets [root]"));
        assert!(commands.contains(&"lsp-cockpit-report"));
        assert!(commands.contains(&"operator-cockpit"));
        assert!(commands.contains(&"operator-cockpit-report"));
        assert!(commands.contains(&"targeted-test-outcome --before <path> --after <path>"));
        assert!(commands.contains(&"mutation-calibration [root] --mutants-json <path>"));
        assert!(commands.contains(&"sarif-policy --current <path> [--baseline <path>]"));
        assert!(commands.contains(&"check-droid-review-config"));
    }

    #[test]
    fn lsp_cockpit_report_reads_boundary_gap_fixture_expectations() -> Result<(), String> {
        let report = with_repo_cwd(build_lsp_cockpit_report)?;
        let Some(boundary_gap) = report
            .fixtures
            .iter()
            .find(|fixture| fixture.fixture == "boundary_gap")
        else {
            return Err("expected boundary_gap LSP cockpit fixture".to_string());
        };

        assert_eq!(boundary_gap.diagnostic_count, 1);
        assert_eq!(boundary_gap.seam_diagnostic_count, 1);
        assert!(
            boundary_gap
                .action_titles
                .contains(&"Copy seam packet".to_string())
        );
        assert!(
            boundary_gap
                .action_titles
                .contains(&"Copy targeted test brief".to_string())
        );
        assert!(
            boundary_gap
                .action_titles
                .contains(&"Open best related test".to_string())
        );
        assert!(boundary_gap.context.seam_packet_available);
        assert!(boundary_gap.context.targeted_test_brief_available);
        assert!(boundary_gap.context.assertion_available);
        assert!(boundary_gap.context.related_test_available);
        assert!(boundary_gap.context.refresh_available);
        assert!(
            boundary_gap
                .action_argument_fields
                .contains(&"seam_id".to_string())
        );
        Ok(())
    }

    #[test]
    fn lsp_cockpit_report_json_and_markdown_are_structured() -> Result<(), String> {
        let report = with_repo_cwd(build_lsp_cockpit_report)?;
        let json = lsp_cockpit_report_json(&report)?;
        let value: Value = serde_json::from_str(&json)
            .map_err(|err| format!("cockpit JSON should parse: {err}"))?;
        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["tool"], "ripr");
        assert!(json.contains("Copy targeted test brief"));

        let markdown = lsp_cockpit_report_markdown(&report);
        assert!(markdown.contains("# ripr LSP cockpit report"));
        assert!(markdown.contains("## Fixture: boundary_gap"));
        assert!(markdown.contains("seam packet available: yes"));
        assert!(markdown.contains("ripr.collectContext"));
        Ok(())
    }

    #[test]
    fn repo_exposure_latency_trace_parses_phase_lines() -> Result<(), String> {
        let traces = repo_exposure_latency_trace(
            "noise\nripr_repo_exposure_latency phase=cache_load status=hit duration_ms=7\n",
        );
        if traces.len() != 1 {
            return Err(format!("expected one trace line, got {}", traces.len()));
        }
        assert_eq!(traces[0].phase, "cache_load");
        assert_eq!(traces[0].status, "hit");
        assert_eq!(traces[0].duration_ms, 7);
        Ok(())
    }

    #[test]
    fn repo_exposure_latency_report_json_and_markdown_are_structured() -> Result<(), String> {
        let runs = vec![
            RepoExposureLatencyRun {
                format: "repo-exposure-json".to_string(),
                status: "timeout".to_string(),
                duration_ms: 30_000,
                exit_code: None,
                stdout_bytes: 0,
                stderr_bytes: 91,
                trace: vec![
                    RepoExposureLatencyTrace {
                        phase: "cache_load".to_string(),
                        status: "miss".to_string(),
                        duration_ms: 2,
                    },
                    RepoExposureLatencyTrace {
                        phase: "cold_compute".to_string(),
                        status: "ok".to_string(),
                        duration_ms: 29_998,
                    },
                ],
            },
            RepoExposureLatencyRun {
                format: "repo-exposure-md".to_string(),
                status: "skipped_after_json_timeout".to_string(),
                duration_ms: 0,
                exit_code: None,
                stdout_bytes: 0,
                stderr_bytes: 0,
                trace: Vec::new(),
            },
        ];
        let report = RepoExposureLatencyReport {
            status: repo_exposure_latency_status(&runs),
            timeout_ms: 30_000,
            binary: "target/debug/ripr".to_string(),
            runs,
        };

        let json = repo_exposure_latency_json(&report);
        let value: Value = serde_json::from_str(&json)
            .map_err(|err| format!("latency JSON should parse: {err}"))?;
        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["report"], "repo-exposure-latency");
        assert_eq!(value["status"], "warn");
        assert_eq!(value["runs"][0]["trace"][0]["phase"], "cache_load");
        assert_eq!(value["runs"][0]["trace"][1]["phase"], "cold_compute");

        let markdown = repo_exposure_latency_markdown(&report);
        assert!(markdown.contains("# Repo Exposure Latency Report"));
        assert!(markdown.contains("`repo-exposure-json`"));
        assert!(markdown.contains("`skipped_after_json_timeout`"));
        assert!(markdown.contains("`cache_load`"));
        Ok(())
    }

    #[test]
    fn repo_exposure_latency_debug_binary_uses_debug_ripr_name() -> Result<(), String> {
        let binary = ripr_debug_binary();
        let file_name = binary
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("debug binary should have a file name: {binary:?}"))?;
        let parent = binary
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("debug binary should have a parent directory: {binary:?}"))?;

        assert_eq!(file_name, format!("ripr{}", std::env::consts::EXE_SUFFIX));
        assert_eq!(parent, "debug");
        Ok(())
    }

    #[test]
    fn repo_exposure_latency_report_json_records_exit_codes() -> Result<(), String> {
        let report = RepoExposureLatencyReport {
            status: "fail".to_string(),
            timeout_ms: 10,
            binary: "target/debug/ripr".to_string(),
            runs: vec![RepoExposureLatencyRun {
                format: "repo-exposure-json".to_string(),
                status: "fail".to_string(),
                duration_ms: 3,
                exit_code: Some(101),
                stdout_bytes: 4,
                stderr_bytes: 9,
                trace: Vec::new(),
            }],
        };

        let json = repo_exposure_latency_json(&report);
        let value: Value = serde_json::from_str(&json)
            .map_err(|err| format!("latency JSON should parse: {err}"))?;
        assert_eq!(value["runs"][0]["exit_code"], 101);
        assert_eq!(value["runs"][0]["stdout_bytes"], 4);
        assert_eq!(value["runs"][0]["stderr_bytes"], 9);

        let markdown = repo_exposure_latency_markdown(&report);
        assert!(
            markdown.contains("| `repo-exposure-json` | `fail` | 3 ms | 101 | 4 bytes | 9 bytes |")
        );
        Ok(())
    }

    #[test]
    fn repo_exposure_latency_report_builder_skips_markdown_after_json_timeout() -> Result<(), String>
    {
        let mut formats = Vec::new();
        let report = build_repo_exposure_latency_report(
            Path::new("target/debug/ripr"),
            2_000,
            |_, format, _| {
                formats.push(format.to_string());
                Ok(latency_run_with_status(format, "timeout"))
            },
        )?;

        assert_eq!(formats, vec!["repo-exposure-json".to_string()]);
        assert_eq!(report.status, "warn");
        assert_eq!(report.timeout_ms, 2_000);
        assert_eq!(report.runs.len(), 2);
        assert_eq!(report.runs[0].status, "timeout");
        assert_eq!(report.runs[1].format, "repo-exposure-md");
        assert_eq!(report.runs[1].status, "skipped_after_json_timeout");
        Ok(())
    }

    #[test]
    fn repo_exposure_latency_report_builder_runs_markdown_after_json_pass() -> Result<(), String> {
        let mut formats = Vec::new();
        let report = build_repo_exposure_latency_report(
            Path::new("target/debug/ripr"),
            30_000,
            |_, format, timeout| {
                formats.push(format.to_string());
                let mut run = latency_run_with_status(format, "pass");
                run.duration_ms = timeout.as_millis();
                Ok(run)
            },
        )?;

        assert_eq!(
            formats,
            vec![
                "repo-exposure-json".to_string(),
                "repo-exposure-md".to_string()
            ]
        );
        assert_eq!(report.status, "pass");
        assert_eq!(report.runs.len(), 2);
        assert_eq!(report.runs[0].duration_ms, 30_000);
        assert_eq!(report.runs[1].duration_ms, 30_000);
        Ok(())
    }

    #[test]
    fn repo_exposure_latency_write_report_writes_markdown_and_json() -> Result<(), String> {
        with_temp_cwd("repo-exposure-latency-write", |_| {
            write_repo_exposure_latency_report(
                Path::new("target/debug/ripr"),
                12,
                |_, format, _| Ok(latency_run_with_status(format, "pass")),
            )?;

            let json = fs::read_to_string("target/ripr/reports/repo-exposure-latency.json")
                .map_err(|err| format!("failed to read latency JSON: {err}"))?;
            let markdown = fs::read_to_string("target/ripr/reports/repo-exposure-latency.md")
                .map_err(|err| format!("failed to read latency markdown: {err}"))?;

            assert!(json.contains("\"report\": \"repo-exposure-latency\""));
            assert!(markdown.contains("# Repo Exposure Latency Report"));
            Ok(())
        })
    }

    #[test]
    fn repo_exposure_latency_run_invokes_binary_and_maps_failure() -> Result<(), String> {
        let run = repo_exposure_latency_run(
            Path::new("rustc"),
            "repo-exposure-json",
            Duration::from_secs(5),
        )?;

        assert_eq!(run.format, "repo-exposure-json");
        assert_eq!(run.status, "fail");
        assert!(!run.trace.iter().any(|trace| trace.phase.is_empty()));
        assert!(run.stderr_bytes > 0);
        Ok(())
    }

    #[test]
    fn repo_exposure_latency_run_from_output_maps_status_and_trace() -> Result<(), String> {
        let args = vec!["--version".to_string()];
        let output = capture_output_with_timeout(
            "rustc",
            &args,
            &[],
            Duration::from_secs(5),
            "rustc version",
        )?;
        let pass_run = repo_exposure_latency_run_from_output("repo-exposure-json", output);
        assert_eq!(pass_run.status, "pass");
        assert_eq!(pass_run.format, "repo-exposure-json");
        assert!(pass_run.exit_code.is_some());
        assert!(pass_run.stdout_bytes > 0);

        let timeout_run = repo_exposure_latency_run_from_output(
            "repo-exposure-md",
            TimedOutput {
                status: None,
                stdout: "partial".to_string(),
                stderr: "ripr_repo_exposure_latency phase=cold_compute status=ok duration_ms=17\n"
                    .to_string(),
                duration: Duration::from_millis(17),
                timed_out: true,
            },
        );
        assert_eq!(timeout_run.status, "timeout");
        assert_eq!(timeout_run.exit_code, None);
        assert_eq!(timeout_run.stdout_bytes, "partial".len());
        assert_eq!(timeout_run.trace[0].phase, "cold_compute");

        let fail_run = repo_exposure_latency_run_from_output(
            "repo-exposure-md",
            TimedOutput {
                status: None,
                stdout: String::new(),
                stderr: "failed".to_string(),
                duration: Duration::from_millis(3),
                timed_out: false,
            },
        );
        assert_eq!(fail_run.status, "fail");
        assert_eq!(fail_run.stderr_bytes, "failed".len());
        Ok(())
    }

    #[test]
    fn repo_exposure_latency_status_and_empty_trace_markdown_are_stable() {
        let pass = latency_run_with_status("repo-exposure-json", "pass");
        let fail = latency_run_with_status("repo-exposure-json", "fail");
        let timeout = latency_run_with_status("repo-exposure-json", "timeout");
        let skipped = latency_run_with_status("repo-exposure-md", "skipped_after_json_timeout");

        assert_eq!(
            repo_exposure_latency_status(std::slice::from_ref(&pass)),
            "pass"
        );
        assert_eq!(repo_exposure_latency_status(&[timeout]), "warn");
        assert_eq!(repo_exposure_latency_status(&[skipped]), "warn");
        assert_eq!(
            repo_exposure_latency_status(&[
                latency_run_with_status("repo-exposure-json", "pass"),
                fail
            ]),
            "fail"
        );

        let report = RepoExposureLatencyReport {
            status: "pass".to_string(),
            timeout_ms: 30_000,
            binary: "target/debug/ripr".to_string(),
            runs: vec![pass],
        };
        let markdown = repo_exposure_latency_markdown(&report);
        assert!(markdown.contains("No analyzer trace lines were captured"));
    }

    fn latency_run_with_status(format: &str, status: &str) -> RepoExposureLatencyRun {
        RepoExposureLatencyRun {
            format: format.to_string(),
            status: status.to_string(),
            duration_ms: 1,
            exit_code: None,
            stdout_bytes: 0,
            stderr_bytes: 0,
            trace: Vec::new(),
        }
    }

    #[test]
    fn lsp_cockpit_report_command_writes_markdown_and_json() -> Result<(), String> {
        with_repo_cwd(|| {
            lsp_cockpit_report()?;
            let markdown = fs::read_to_string("target/ripr/reports/lsp-cockpit.md")
                .map_err(|err| format!("failed to read lsp cockpit markdown: {err}"))?;
            let json = fs::read_to_string("target/ripr/reports/lsp-cockpit.json")
                .map_err(|err| format!("failed to read lsp cockpit JSON: {err}"))?;
            assert!(markdown.contains("# ripr LSP cockpit report"));
            assert!(json.contains("\"schema_version\": \"0.1\""));
            Ok(())
        })
    }

    #[test]
    fn defaults_first_example_corpus_index_names_required_operator_artifacts() -> Result<(), String>
    {
        with_repo_cwd(|| {
            let text = fs::read_to_string("fixtures/EXAMPLE_CORPUS.md")
                .map_err(|err| format!("failed to read example corpus index: {err}"))?;
            for required_text in [
                "Boundary gap",
                "Missing equality boundary",
                "Weak oracle",
                "Exact error variant",
                "Opaque fixture/builder",
                "Optional calibration",
                "targeted-test-outcome.json",
                "mutation-calibration.json",
                "lsp-code-actions.json",
            ] {
                assert!(
                    text.contains(required_text),
                    "example corpus index should mention {required_text}"
                );
            }
            for required_path in [
                "fixtures/boundary_gap/expected/lsp-diagnostics.json",
                "fixtures/boundary_gap/expected/lsp-code-actions.json",
                "fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json",
                "fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json",
                "fixtures/boundary_gap/calibration/targeted-test-outcome.json",
                "fixtures/boundary_gap/calibration/targeted-test-outcome.md",
                "fixtures/boundary_gap/calibration/runtime-mutants.json",
                "fixtures/boundary_gap/calibration/mutation-calibration.json",
                "fixtures/boundary_gap/calibration/mutation-calibration.md",
                "fixtures/opaque_fixture_builder/expected/check.json",
                "fixtures/opaque_fixture_builder/expected/human.txt",
            ] {
                assert!(
                    Path::new(required_path).exists(),
                    "example corpus artifact should exist: {required_path}"
                );
            }
            Ok(())
        })
    }

    #[test]
    fn vscode_command_literal_extraction_finds_ripr_commands() {
        let commands = ripr_command_literals_in_text(
            "await vscode.commands.executeCommand('ripr.copyContext');\ncommand: \"ripr.collectContext\"",
        );
        assert_eq!(
            commands,
            vec![
                "ripr.collectContext".to_string(),
                "ripr.copyContext".to_string()
            ]
        );
    }

    #[test]
    fn targeted_test_outcome_args_parse_before_and_after() -> Result<(), String> {
        let args = vec![
            "--before".to_string(),
            "before.json".to_string(),
            "--after".to_string(),
            "after.json".to_string(),
        ];
        let parsed = parse_targeted_test_outcome_args(&args)?;
        assert_eq!(parsed.before, PathBuf::from("before.json"));
        assert_eq!(parsed.after, PathBuf::from("after.json"));
        Ok(())
    }

    #[test]
    fn targeted_test_outcome_report_buckets_seam_movement() -> Result<(), String> {
        let mut before_moved = targeted_static_seam("seam-moved", "weakly_gripped");
        before_moved.missing_discriminators = vec!["threshold equality".to_string()];
        before_moved.oracle_strength = "weak".to_string();
        let before = vec![
            before_moved,
            targeted_static_seam("seam-regressed", "weakly_gripped"),
            targeted_static_seam("seam-same", "strongly_gripped"),
            targeted_static_seam("seam-removed", "ungripped"),
        ];

        let mut after_moved = targeted_static_seam("seam-moved", "strongly_gripped");
        after_moved.observed_values = vec!["50".to_string(), "100".to_string()];
        after_moved.oracle_strength = "strong".to_string();
        let after = vec![
            after_moved,
            targeted_static_seam("seam-regressed", "ungripped"),
            targeted_static_seam("seam-same", "strongly_gripped"),
            targeted_static_seam("seam-new", "weakly_gripped"),
        ];

        let report = build_targeted_test_outcome_report(
            &before,
            &after,
            "before.json".to_string(),
            "after.json".to_string(),
        )?;
        assert_eq!(report.moved.len(), 1);
        assert_eq!(report.moved[0].seam_id, "seam-moved");
        assert_eq!(report.moved[0].direction, "improved");
        assert!(
            report.moved[0]
                .evidence_delta
                .iter()
                .any(|delta| delta.contains("missing discriminator no longer reported"))
        );
        assert!(
            report.moved[0]
                .evidence_delta
                .iter()
                .any(|delta| delta.contains("stronger related oracle visible"))
        );
        assert_eq!(report.regressed.len(), 1);
        assert_eq!(report.unchanged.len(), 1);
        assert_eq!(report.new.len(), 1);
        assert_eq!(report.removed.len(), 1);
        assert_eq!(report.before_counts.get("weakly_gripped"), Some(&2));
        assert_eq!(report.after_counts.get("strongly_gripped"), Some(&2));
        Ok(())
    }

    #[test]
    fn targeted_test_outcome_json_and_markdown_are_structured() -> Result<(), String> {
        let before = vec![
            targeted_static_seam("seam-a", "weakly_gripped"),
            targeted_static_seam("seam-same", "weakly_gripped"),
        ];
        let mut after_same = targeted_static_seam("seam-same", "weakly_gripped");
        after_same.observed_values = vec!["50".to_string(), "100".to_string()];
        let after = vec![
            targeted_static_seam("seam-a", "strongly_gripped"),
            after_same,
        ];
        let report = build_targeted_test_outcome_report(
            &before,
            &after,
            "target/ripr/before.json".to_string(),
            "target/ripr/after.json".to_string(),
        )?;

        let json = targeted_test_outcome_report_json(&report)?;
        let value: Value = serde_json::from_str(&json)
            .map_err(|err| format!("targeted-test outcome JSON should parse: {err}"))?;
        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["status"], "advisory");
        assert_eq!(value["summary"]["moved"], 1);

        let markdown = targeted_test_outcome_report_markdown(&report);
        assert!(markdown.contains("# ripr targeted-test outcome report"));
        assert!(markdown.contains("| moved | 1 |"));
        assert!(markdown.contains("## Unchanged"));
        assert!(markdown.contains("seam-same"));
        assert!(markdown.contains("new observed value: 100"));
        assert!(markdown.contains("weakly_gripped -> strongly_gripped"));
        Ok(())
    }

    #[test]
    fn targeted_test_outcome_command_writes_markdown_and_json() -> Result<(), String> {
        with_temp_cwd("targeted-test-outcome", |_root| {
            let before = r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "weakly_gripped",
      "related_tests": [
        {"oracle_kind": "exact_value", "oracle_strength": "weak"}
      ],
      "observed_values": ["50"],
      "missing_discriminators": [
        {"value": "threshold equality", "reason": "not observed"}
      ]
    }
  ]
}"#;
            let after = r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "strongly_gripped",
      "related_tests": [
        {"oracle_kind": "exact_value", "oracle_strength": "strong"}
      ],
      "observed_values": ["50", "100"],
      "missing_discriminators": []
    }
  ]
}"#;
            write(Path::new("before.json"), before);
            write(Path::new("after.json"), after);
            targeted_test_outcome(&[
                "--before".to_string(),
                "before.json".to_string(),
                "--after".to_string(),
                "after.json".to_string(),
            ])?;
            let markdown = fs::read_to_string("target/ripr/reports/targeted-test-outcome.md")
                .map_err(|err| format!("failed to read targeted-test outcome markdown: {err}"))?;
            let json = fs::read_to_string("target/ripr/reports/targeted-test-outcome.json")
                .map_err(|err| format!("failed to read targeted-test outcome JSON: {err}"))?;
            assert!(markdown.contains("# ripr targeted-test outcome report"));
            assert!(json.contains("\"schema_version\": \"0.1\""));
            assert!(json.contains("\"moved\": 1"));
            Ok(())
        })
    }

    fn ci_full_ok_gate() -> Result<(), String> {
        if std::env::var_os("RIPR_XTASK_TEST_FAIL_OK_GATE").is_some() {
            return Err("unexpected test env".to_string());
        }
        Ok(())
    }

    fn ci_full_err_gate() -> Result<(), String> {
        Err("boom".to_string())
    }

    #[test]
    fn ci_full_evidence_gates_pin_release_review_order() {
        let names = ci_full_evidence_gates()
            .iter()
            .map(|gate| gate.name)
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "fixtures",
                "goldens check",
                "test-oracle-report",
                "dogfood",
                "metrics"
            ]
        );
    }

    #[test]
    fn ci_full_evidence_gate_runner_accepts_successful_gates() -> Result<(), String> {
        let gates = [CiFullEvidenceGate {
            name: "ok",
            run: ci_full_ok_gate,
        }];

        run_ci_full_evidence_gates(&gates)
    }

    #[test]
    fn ci_full_evidence_gate_runner_names_failing_gate() -> Result<(), String> {
        let gates = [
            CiFullEvidenceGate {
                name: "ok",
                run: ci_full_ok_gate,
            },
            CiFullEvidenceGate {
                name: "bad",
                run: ci_full_err_gate,
            },
        ];

        let error = run_ci_full_evidence_gates(&gates)
            .err()
            .ok_or_else(|| "expected failing gate".to_string())?;

        assert!(error.contains("`bad`"));
        assert!(error.contains("boom"));
        Ok(())
    }

    #[test]
    fn sarif_policy_passes_when_no_new_results() {
        let current = vec![sarif_policy_result(
            "ripr.finding.weakly_exposed",
            "warning",
            "same",
        )];
        let baseline = current.clone();
        let report = build_sarif_policy_report(
            SarifPolicyMode::BaselineCheck,
            SarifPolicyThreshold::Warning,
            "current.sarif.json".to_string(),
            Some("baseline.sarif.json".to_string()),
            &current,
            Some(&baseline),
            false,
        );

        assert_eq!(report.status, "pass");
        assert!(report.new_results.is_empty());
        assert_eq!(report.current_compared_results, 1);
        assert_eq!(report.baseline_compared_results, 1);
    }

    #[test]
    fn sarif_policy_flags_new_warning_result() {
        let current = vec![sarif_policy_result(
            "ripr.seam.weakly_gripped",
            "warning",
            "new",
        )];
        let baseline = vec![sarif_policy_result(
            "ripr.seam.weakly_gripped",
            "warning",
            "old",
        )];
        let report = build_sarif_policy_report(
            SarifPolicyMode::BaselineCheck,
            SarifPolicyThreshold::Warning,
            "current.sarif.json".to_string(),
            Some("baseline.sarif.json".to_string()),
            &current,
            Some(&baseline),
            false,
        );

        assert_eq!(report.status, "new_results");
        assert_eq!(report.new_results.len(), 1);
        assert_eq!(report.new_results[0].fingerprint, "new");
    }

    #[test]
    fn sarif_policy_ignores_note_when_threshold_warning() {
        let current = vec![sarif_policy_result("ripr.seam.opaque", "note", "new-note")];
        let baseline: Vec<SarifPolicyResult> = Vec::new();
        let report = build_sarif_policy_report(
            SarifPolicyMode::BaselineCheck,
            SarifPolicyThreshold::Warning,
            "current.sarif.json".to_string(),
            Some("baseline.sarif.json".to_string()),
            &current,
            Some(&baseline),
            false,
        );

        assert_eq!(report.status, "pass");
        assert!(report.new_results.is_empty());
        assert_eq!(report.current_compared_results, 0);
    }

    #[test]
    fn sarif_policy_missing_baseline_is_advisory_by_default() {
        let current = vec![sarif_policy_result("ripr.seam.ungripped", "warning", "new")];
        let report = build_sarif_policy_report(
            SarifPolicyMode::FailOnNewWarning,
            SarifPolicyThreshold::Warning,
            "current.sarif.json".to_string(),
            Some("missing-baseline.sarif.json".to_string()),
            &current,
            None,
            true,
        );

        assert_eq!(report.status, "advisory_missing_baseline");
        assert!(report.new_results.is_empty());
        assert!(report.baseline_missing);
    }

    #[test]
    fn sarif_policy_parses_results_and_skips_suppressions() -> Result<(), String> {
        let text = sarif_policy_test_sarif(vec![
            sarif_policy_json_result("ripr.finding.weakly_exposed", "warning", "visible", false),
            sarif_policy_json_result("ripr.finding.weakly_exposed", "warning", "hidden", true),
        ])?;
        let results = parse_sarif_policy_results(&text, "test SARIF")?;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fingerprint, "visible");
        assert_eq!(results[0].uri, "src/lib.rs");
        assert_eq!(results[0].line, Some(12));
        Ok(())
    }

    #[test]
    fn sarif_policy_args_parse_mode_threshold_and_missing_baseline() -> Result<(), String> {
        let args = vec![
            "--current".to_string(),
            "current.sarif.json".to_string(),
            "--baseline".to_string(),
            "baseline.sarif.json".to_string(),
            "--mode".to_string(),
            "fail-on-new-warning".to_string(),
            "--threshold".to_string(),
            "note".to_string(),
            "--missing-baseline".to_string(),
            "error".to_string(),
        ];

        let parsed = parse_sarif_policy_args(&args)?;

        assert_eq!(parsed.current, PathBuf::from("current.sarif.json"));
        assert_eq!(parsed.baseline, Some(PathBuf::from("baseline.sarif.json")));
        assert_eq!(parsed.mode, SarifPolicyMode::FailOnNewWarning);
        assert_eq!(parsed.threshold, SarifPolicyThreshold::Note);
        assert_eq!(parsed.missing_baseline, SarifMissingBaseline::Error);
        Ok(())
    }

    #[test]
    fn sarif_policy_report_json_and_markdown_are_structured() -> Result<(), String> {
        let current = vec![sarif_policy_result(
            "ripr.seam.weakly_gripped",
            "warning",
            "new",
        )];
        let baseline: Vec<SarifPolicyResult> = Vec::new();
        let report = build_sarif_policy_report(
            SarifPolicyMode::BaselineCheck,
            SarifPolicyThreshold::Warning,
            "current.sarif.json".to_string(),
            Some("baseline.sarif.json".to_string()),
            &current,
            Some(&baseline),
            false,
        );

        let json = sarif_policy_report_json(&report)?;
        let markdown = sarif_policy_report_markdown(&report);

        assert!(json.contains("\"schema_version\": \"0.1\""));
        assert!(json.contains("\"new_results_total\": 1"));
        assert!(markdown.contains("# ripr SARIF policy report"));
        assert!(markdown.contains("ripr.seam.weakly_gripped"));
        Ok(())
    }

    fn sarif_policy_result(rule_id: &str, level: &str, fingerprint: &str) -> SarifPolicyResult {
        SarifPolicyResult {
            key: format!("{rule_id}|{fingerprint}"),
            rule_id: rule_id.to_string(),
            level: level.to_string(),
            fingerprint: fingerprint.to_string(),
            uri: "src/lib.rs".to_string(),
            line: Some(12),
            message: "static exposure result".to_string(),
        }
    }

    fn sarif_policy_json_result(
        rule_id: &str,
        level: &str,
        fingerprint: &str,
        suppressed: bool,
    ) -> Value {
        let mut result = serde_json::json!({
            "ruleId": rule_id,
            "level": level,
            "message": { "text": "static exposure result" },
            "partialFingerprints": {
                "riprFingerprintV1": fingerprint
            },
            "locations": [
                {
                    "physicalLocation": {
                        "artifactLocation": { "uri": "src/lib.rs" },
                        "region": { "startLine": 12 }
                    }
                }
            ]
        });
        if suppressed && let Some(object) = result.as_object_mut() {
            object.insert(
                "suppressions".to_string(),
                serde_json::json!([{ "kind": "external" }]),
            );
        }
        result
    }

    fn sarif_policy_test_sarif(results: Vec<Value>) -> Result<String, String> {
        let value = serde_json::json!({
            "version": "2.1.0",
            "runs": [
                {
                    "results": results
                }
            ]
        });
        serde_json::to_string(&value).map_err(|err| err.to_string())
    }

    #[test]
    fn install_hooks_creates_missing_hook() -> Result<(), String> {
        let root = temp_dir("install-hooks-create");
        fs::create_dir(root.join(".git")).map_err(|err| err.to_string())?;

        let hook = install_hooks_in(&root)?;
        let text = fs::read_to_string(&hook).map_err(|err| err.to_string())?;

        assert_eq!(hook, root.join(".git").join("hooks").join("pre-commit"));
        assert!(is_ripr_managed_hook(&text));
        assert!(text.contains("cargo xtask precommit"));
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn install_hooks_is_idempotent_for_managed_hook() -> Result<(), String> {
        let root = temp_dir("install-hooks-idempotent");
        let hook = root.join(".git").join("hooks").join("pre-commit");
        let stale_managed_hook =
            ripr_pre_commit_hook().replace("cargo xtask precommit", "echo old");
        write(&hook, &stale_managed_hook);

        let first = install_hooks_in(&root)?;
        let second = install_hooks_in(&root)?;
        let text = fs::read_to_string(&hook).map_err(|err| err.to_string())?;

        assert_eq!(first, hook);
        assert_eq!(second, hook);
        assert_eq!(text, ripr_pre_commit_hook());
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn install_hooks_refuses_unmanaged_existing_hook() -> Result<(), String> {
        let root = temp_dir("install-hooks-unmanaged");
        let hook = root.join(".git").join("hooks").join("pre-commit");
        let user_hook = "#!/usr/bin/env sh\necho user hook\n";
        write(&hook, user_hook);

        let error = install_hooks_in(&root)
            .err()
            .ok_or_else(|| "expected unmanaged hook refusal".to_string())?;
        let text = fs::read_to_string(&hook).map_err(|err| err.to_string())?;

        assert!(error.contains("refusing to overwrite unmanaged hook"));
        assert_eq!(text, user_hook);
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn install_hooks_errors_outside_git_worktree() -> Result<(), String> {
        let root = temp_dir("install-hooks-outside-git");

        let error = install_hooks_in(&root)
            .err()
            .ok_or_else(|| "expected missing git worktree error".to_string())?;

        assert!(error.contains("missing .git directory"));
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn agent_seam_packet_command_args_use_requested_root_and_format() {
        let args =
            repo_seam_inventory_command_args_for_root("agent-seam-packets-json", "fixtures/demo");
        assert_eq!(
            args,
            vec![
                "run",
                "-p",
                "ripr",
                "--quiet",
                "--",
                "check",
                "--root",
                "fixtures/demo",
                "--format",
                "agent-seam-packets-json",
            ]
        );
        assert!(known_xtask_command("agent-seam-packets"));
    }

    #[test]
    fn mutation_calibration_args_parse_root_and_input_paths() -> Result<(), String> {
        let args = vec![
            "fixtures/demo".to_string(),
            "--mutants-json".to_string(),
            "target/mutants/outcomes.json".to_string(),
            "--repo-exposure-json".to_string(),
            "target/ripr/reports/repo-exposure.json".to_string(),
        ];

        let parsed = parse_mutation_calibration_args(&args)?;

        assert_eq!(parsed.root, "fixtures/demo");
        assert_eq!(
            parsed.mutants_json,
            PathBuf::from("target/mutants/outcomes.json")
        );
        assert_eq!(
            parsed.repo_exposure_json,
            Some(PathBuf::from("target/ripr/reports/repo-exposure.json"))
        );
        assert!(known_xtask_command("mutation-calibration"));
        Ok(())
    }

    #[test]
    fn mutation_calibration_imports_static_seams_and_runtime_outcomes() -> Result<(), String> {
        let static_json = r#"{
          "schema_version": "0.2",
          "scope": "repo",
          "seams": [
            {
              "seam_id": "abc123",
              "kind": "predicate_boundary",
              "file": "src/pricing.rs",
              "line": 42,
              "grip_class": "weakly_gripped",
              "related_tests": [
                {
                  "oracle_kind": "broad_error",
                  "oracle_strength": "weak"
                },
                {
                  "oracle_kind": "exact_value",
                  "oracle_strength": "strong"
                }
              ],
              "observed_values": ["50", "10000"],
              "missing_discriminators": [
                {"value": "amount == discount_threshold", "reason": "equality boundary"}
              ]
            }
          ]
        }"#;
        let runtime_json = r#"{
          "outcomes": [
            {
              "mutant": {
                "id": "m1",
                "seam_id": "abc123",
                "file": "src/pricing.rs",
                "line": 42,
                "operator": "replace >= with >"
              },
              "outcome": "caught",
              "duration_ms": 123,
              "test_command": "cargo test pricing"
            }
          ]
        }"#;

        let seams = parse_repo_exposure_static_seams(static_json)?;
        let mutants = parse_mutation_outcomes_json(runtime_json)?;

        assert_eq!(seams.len(), 1);
        assert_eq!(seams[0].oracle_kind, "exact_value");
        assert_eq!(seams[0].oracle_strength, "strong");
        assert_eq!(
            seams[0].missing_discriminators,
            vec!["amount == discount_threshold (equality boundary)"]
        );
        assert_eq!(mutants.len(), 1);
        assert_eq!(mutants[0].mutation_operator, "replace >= with >");
        assert_eq!(mutants[0].runtime_outcome, "caught");
        assert_eq!(mutants[0].duration, Some("123".to_string()));
        Ok(())
    }

    #[test]
    fn mutation_calibration_merges_mutants_and_outcomes_by_mutant_id() -> Result<(), String> {
        let runtime_json = r#"{
          "mutants": [
            {
              "id": "m1",
              "file": "src/pricing.rs",
              "line": 42,
              "operator": "replace >= with >"
            }
          ],
          "outcomes": [
            {
              "mutant_id": "m1",
              "outcome": "caught",
              "duration_ms": 123,
              "test_command": "cargo test pricing"
            }
          ]
        }"#;

        let mutants = parse_mutation_outcomes_json(runtime_json)?;

        assert_eq!(mutants.len(), 1);
        assert_eq!(mutants[0].mutant_id, Some("m1".to_string()));
        assert_eq!(mutants[0].file, Some("src/pricing.rs".to_string()));
        assert_eq!(mutants[0].line, Some(42));
        assert_eq!(mutants[0].mutation_operator, "replace >= with >");
        assert_eq!(mutants[0].runtime_outcome, "caught");
        assert_eq!(mutants[0].duration, Some("123".to_string()));
        Ok(())
    }

    #[test]
    fn mutation_calibration_imports_span_based_mutant_locations() -> Result<(), String> {
        let runtime_json = r#"{
          "mutants": [
            {
              "id": "m1",
              "operator": "replace >= with >",
              "span": {
                "file_name": "src/pricing.rs",
                "start": { "line": 42, "column": 13 },
                "end": { "line": 42, "column": 15 }
              }
            }
          ],
          "outcomes": [
            {
              "mutant_id": "m1",
              "outcome": "caught"
            }
          ]
        }"#;

        let mutants = parse_mutation_outcomes_json(runtime_json)?;

        assert_eq!(mutants.len(), 1);
        assert_eq!(mutants[0].file, Some("src/pricing.rs".to_string()));
        assert_eq!(mutants[0].line, Some(42));
        assert_eq!(mutants[0].runtime_outcome, "caught");
        Ok(())
    }

    #[test]
    fn mutation_calibration_directory_input_combines_outcomes_and_mutants() -> Result<(), String> {
        let dir = temp_dir("mutation-calibration-dir");
        write(
            &dir.join("mutants.json"),
            r#"{
              "mutants": [
                {
                  "id": "m1",
                  "file": "src/pricing.rs",
                  "line": 42,
                  "operator": "replace >= with >"
                }
              ]
            }"#,
        );
        write(
            &dir.join("outcomes.json"),
            r#"{
              "outcomes": [
                {
                  "mutant_id": "m1",
                  "outcome": "caught",
                  "duration_ms": 123
                }
              ]
            }"#,
        );

        let input = read_mutation_input_json(&dir)?;
        let mutants = parse_mutation_outcomes_json(&input)?;
        let remove_result = fs::remove_dir_all(&dir);
        if let Err(err) = remove_result {
            return Err(format!(
                "failed to remove temp dir {}: {err}",
                dir.display()
            ));
        }

        assert_eq!(mutants.len(), 1);
        assert_eq!(mutants[0].file, Some("src/pricing.rs".to_string()));
        assert_eq!(mutants[0].runtime_outcome, "caught");
        Ok(())
    }

    #[test]
    fn mutation_calibration_joins_by_seam_id_then_file_line() {
        let static_seams = vec![
            StaticSeamRecord {
                seam_id: "seam-a".to_string(),
                seam_kind: "predicate_boundary".to_string(),
                file: "src/pricing.rs".to_string(),
                line: 42,
                seam_grip_class: "weakly_gripped".to_string(),
                oracle_kind: "exact_value".to_string(),
                oracle_strength: "strong".to_string(),
                observed_values: vec!["50".to_string()],
                missing_discriminators: vec!["amount == discount_threshold".to_string()],
            },
            StaticSeamRecord {
                seam_id: "seam-b".to_string(),
                seam_kind: "error_variant".to_string(),
                file: "src/auth.rs".to_string(),
                line: 11,
                seam_grip_class: "ungripped".to_string(),
                oracle_kind: "unknown".to_string(),
                oracle_strength: "unknown".to_string(),
                observed_values: Vec::new(),
                missing_discriminators: Vec::new(),
            },
        ];
        let runtime_mutants = vec![
            MutationOutcomeRecord {
                mutant_id: Some("m1".to_string()),
                seam_id: Some("seam-a".to_string()),
                file: None,
                line: None,
                mutation_operator: "replace >= with >".to_string(),
                runtime_outcome: "caught".to_string(),
                duration: Some("55".to_string()),
                test_command: Some("cargo test".to_string()),
            },
            MutationOutcomeRecord {
                mutant_id: Some("m2".to_string()),
                seam_id: None,
                file: Some(".\\src\\auth.rs".to_string()),
                line: Some(11),
                mutation_operator: "replace error variant".to_string(),
                runtime_outcome: "timeout".to_string(),
                duration: None,
                test_command: None,
            },
            MutationOutcomeRecord {
                mutant_id: Some("m3".to_string()),
                seam_id: None,
                file: Some("src/other.rs".to_string()),
                line: Some(99),
                mutation_operator: "replace value".to_string(),
                runtime_outcome: "caught".to_string(),
                duration: None,
                test_command: None,
            },
        ];

        let report = build_mutation_calibration_report(static_seams, runtime_mutants);

        assert_eq!(report.matched.len(), 2);
        assert_eq!(report.matched[0].join_method, "seam_id");
        assert_eq!(report.matched[1].join_method, "file_line");
        assert_eq!(report.unmatched_mutants.len(), 1);
        assert!(report.static_without_runtime.is_empty());
    }

    #[test]
    fn mutation_calibration_summarizes_static_runtime_agreement() -> Result<(), String> {
        let static_seams = vec![
            targeted_static_seam("gap-runtime-signal", "weakly_gripped"),
            targeted_static_seam("gap-runtime-clean", "ungripped"),
            targeted_static_seam("gap-inconclusive", "reachable_unrevealed"),
            targeted_static_seam("clean-runtime-clean", "strongly_gripped"),
            targeted_static_seam("clean-runtime-signal", "strongly_gripped"),
        ];
        let runtime_mutants = vec![
            mutation_record("m1", Some("gap-runtime-signal"), "missed"),
            mutation_record("m2", Some("gap-runtime-clean"), "caught"),
            mutation_record("m3", Some("gap-inconclusive"), "unviable"),
            mutation_record("m4", Some("clean-runtime-clean"), "caught"),
            mutation_record("m5", Some("clean-runtime-signal"), "missed"),
            mutation_record_at("m6", None, "src/other.rs", 99, "missed"),
        ];

        let report = build_mutation_calibration_report(static_seams, runtime_mutants);

        assert_eq!(report.agreement.static_gap_and_runtime_signal, 1);
        assert_eq!(report.agreement.static_gap_without_runtime_signal, 2);
        assert_eq!(report.agreement.runtime_signal_without_static_gap, 2);
        assert_eq!(report.agreement.static_clean_and_runtime_clean, 1);
        assert_eq!(report.agreement.runtime_inconclusive, 1);
        assert_eq!(report.static_only_findings.len(), 2);
        assert_eq!(report.missed_runtime_signals.len(), 2);

        let json = mutation_calibration_report_json(&report)?;
        let value: serde_json::Value = serde_json::from_str(&json)
            .map_err(|err| format!("failed to parse calibration JSON: {err}"))?;
        assert_eq!(value["agreement"]["static_gap_and_runtime_signal"], 1);
        assert_eq!(value["agreement"]["runtime_signal_without_static_gap"], 2);
        assert_eq!(
            value["missed_runtime_signals"].as_array().map(Vec::len),
            Some(2)
        );
        assert_eq!(
            value["static_only_findings"].as_array().map(Vec::len),
            Some(2)
        );

        let markdown = mutation_calibration_report_markdown(&report);
        assert!(markdown.contains("## Static/runtime agreement"));
        assert!(markdown.contains("static_gap_and_runtime_signal"));
        assert!(markdown.contains("Runtime signals without static gaps"));
        assert!(markdown.contains("Static gaps without runtime signals"));
        Ok(())
    }

    #[test]
    fn mutation_calibration_reports_ambiguous_file_line_without_selecting_first() {
        let static_seams = vec![
            StaticSeamRecord {
                seam_id: "seam-a".to_string(),
                seam_kind: "predicate_boundary".to_string(),
                file: "src/pricing.rs".to_string(),
                line: 42,
                seam_grip_class: "weakly_gripped".to_string(),
                oracle_kind: "exact_value".to_string(),
                oracle_strength: "strong".to_string(),
                observed_values: Vec::new(),
                missing_discriminators: Vec::new(),
            },
            StaticSeamRecord {
                seam_id: "seam-b".to_string(),
                seam_kind: "return_value".to_string(),
                file: "src/pricing.rs".to_string(),
                line: 42,
                seam_grip_class: "ungripped".to_string(),
                oracle_kind: "unknown".to_string(),
                oracle_strength: "unknown".to_string(),
                observed_values: Vec::new(),
                missing_discriminators: Vec::new(),
            },
        ];
        let runtime_mutants = vec![MutationOutcomeRecord {
            mutant_id: Some("m1".to_string()),
            seam_id: None,
            file: Some("src/pricing.rs".to_string()),
            line: Some(42),
            mutation_operator: "replace >= with >".to_string(),
            runtime_outcome: "caught".to_string(),
            duration: None,
            test_command: None,
        }];

        let report = build_mutation_calibration_report(static_seams, runtime_mutants);

        assert!(report.matched.is_empty());
        assert_eq!(report.ambiguous_file_line.len(), 1);
        assert_eq!(report.ambiguous_file_line[0].candidates.len(), 2);
        assert!(report.unmatched_mutants.is_empty());
        assert!(report.static_without_runtime.is_empty());
    }

    #[test]
    fn mutation_calibration_uses_same_static_without_runtime_sample_limit_for_json_and_markdown()
    -> Result<(), String> {
        let seams = (0..51)
            .map(|idx| StaticSeamRecord {
                seam_id: format!("seam-{idx:02}"),
                seam_kind: "predicate_boundary".to_string(),
                file: "src/pricing.rs".to_string(),
                line: idx + 1,
                seam_grip_class: "weakly_gripped".to_string(),
                oracle_kind: "exact_value".to_string(),
                oracle_strength: "strong".to_string(),
                observed_values: Vec::new(),
                missing_discriminators: Vec::new(),
            })
            .collect::<Vec<_>>();
        let report = build_mutation_calibration_report(seams, Vec::new());

        let json = mutation_calibration_report_json(&report)?;
        let value: serde_json::Value = serde_json::from_str(&json)
            .map_err(|err| format!("failed to parse calibration JSON: {err}"))?;
        let Some(sample) = value["static_without_runtime_sample"].as_array() else {
            return Err("missing static_without_runtime_sample array".to_string());
        };
        let markdown = mutation_calibration_report_markdown(&report);
        let Some(static_without_runtime_section) = markdown
            .split("## Static Seams Without Runtime Data")
            .nth(1)
        else {
            return Err("missing Static Seams Without Runtime Data section".to_string());
        };
        let markdown_rows = static_without_runtime_section
            .lines()
            .filter(|line| line.starts_with("| `seam-"))
            .count();

        assert_eq!(
            sample.len(),
            MUTATION_CALIBRATION_STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT
        );
        assert_eq!(
            markdown_rows,
            MUTATION_CALIBRATION_STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT
        );
        Ok(())
    }

    #[test]
    fn mutation_calibration_reports_are_advisory_and_structured() -> Result<(), String> {
        let report = build_mutation_calibration_report(
            vec![StaticSeamRecord {
                seam_id: "seam-a".to_string(),
                seam_kind: "predicate_boundary".to_string(),
                file: "src/pricing.rs".to_string(),
                line: 42,
                seam_grip_class: "weakly_gripped".to_string(),
                oracle_kind: "exact_value".to_string(),
                oracle_strength: "strong".to_string(),
                observed_values: vec!["50".to_string()],
                missing_discriminators: vec!["amount == discount_threshold".to_string()],
            }],
            vec![MutationOutcomeRecord {
                mutant_id: Some("m1".to_string()),
                seam_id: Some("seam-a".to_string()),
                file: Some("src/pricing.rs".to_string()),
                line: Some(42),
                mutation_operator: "replace >= with >".to_string(),
                runtime_outcome: "caught".to_string(),
                duration: Some("55".to_string()),
                test_command: Some("cargo test pricing".to_string()),
            }],
        );

        let json = mutation_calibration_report_json(&report)?;
        let value: serde_json::Value = serde_json::from_str(&json)
            .map_err(|err| format!("failed to parse calibration JSON: {err}"))?;
        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["status"], "advisory");
        assert_eq!(value["metrics"]["matched_total"], 1);
        assert_eq!(value["metrics"]["ambiguous_file_line_total"], 0);
        assert_eq!(
            value["matches"][0]["static"]["missing_discriminators"][0],
            "amount == discount_threshold"
        );
        assert_eq!(
            value["matches"][0]["runtime"]["test_command"],
            "cargo test pricing"
        );

        let markdown = mutation_calibration_report_markdown(&report);
        assert!(markdown.contains("Status: advisory"));
        assert!(markdown.contains("weakly_gripped"));
        assert!(markdown.contains("replace >= with >"));
        Ok(())
    }
}
