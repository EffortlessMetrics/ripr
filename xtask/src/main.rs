#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::Instant;

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
    static_limitations: Vec<String>,
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
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(|s| s.as_str()) {
        Some("shape") => shape(),
        Some("fix-pr") => fix_pr(),
        Some("pr-summary") => pr_summary(),
        Some("precommit") => precommit(),
        Some("check-pr") => check_pr(),
        Some("fixtures") => fixtures(args.get(2)),
        Some("goldens") => goldens(&args[2..]),
        Some("metrics") => metrics_report(),
        Some("test-oracle-report") | Some("check-test-oracles") => test_oracle_report(),
        Some("test-efficiency-report") => test_efficiency_report(),
        Some("dogfood") => dogfood(),
        Some("critic") => critic(),
        Some("goals") => goals(&args[2..]),
        Some("reports") => reports(&args[2..]),
        Some("receipts") => receipts(&args[2..]),
        Some("golden-drift") => golden_drift(),
        Some("ci-fast") => ci_fast(),
        Some("ci-full") => ci_full(),
        Some("check-static-language") => check_static_language(),
        Some("check-no-panic-family") => check_no_panic_family(),
        Some("check-allow-attributes") => check_allow_attributes(),
        Some("check-local-context") => check_local_context(),
        Some("check-file-policy") => check_file_policy(),
        Some("check-executable-files") => check_executable_files(),
        Some("check-workflows") => check_workflows(),
        Some("check-spec-format") => check_spec_format(),
        Some("check-fixture-contracts") => check_fixture_contracts(),
        Some("check-traceability") | Some("check-spec-ids") | Some("check-behavior-manifest") => {
            check_traceability()
        }
        Some("check-capabilities") => check_capabilities(),
        Some("check-workspace-shape") => check_workspace_shape(),
        Some("check-architecture") => check_architecture(),
        Some("check-public-api") => check_public_api(),
        Some("check-output-contracts") => check_output_contracts(),
        Some("check-doc-index") => check_doc_index(),
        Some("check-readme-state") => check_readme_state(),
        Some("markdown-links") => markdown_links(),
        Some("check-campaign") | Some("check-goals") => check_campaign(),
        Some("check-pr-shape") => check_pr_shape(),
        Some("check-generated") => check_generated(),
        Some("check-dependencies") => check_dependencies(),
        Some("check-supply-chain") => check_supply_chain(),
        Some("check-process-policy") => check_process_policy(),
        Some("check-network-policy") => check_network_policy(),
        Some("package") => run("cargo", &["package", "-p", "ripr", "--list"]).map(|_| ()),
        Some("publish-dry-run") => {
            run("cargo", &["publish", "-p", "ripr", "--dry-run"]).map(|_| ())
        }
        Some("help") | None => {
            print_help();
            Ok(())
        }
        Some(other) => Err(format!("unknown xtask command {other}")),
    };
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
    run("cargo", &["package", "-p", "ripr", "--list"])?;
    run("cargo", &["publish", "-p", "ripr", "--dry-run"]).map(|_| ())
}

fn run(program: &str, args: &[&str]) -> Result<ExitStatus, String> {
    eprintln!("$ {} {}", program, args.join(" "));
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|err| format!("failed to run {program}: {err}"))?;
    if status.success() {
        Ok(status)
    } else {
        Err(format!("{program} {} failed with {status}", args.join(" ")))
    }
}

fn print_help() {
    println!(
        "xtask commands:\n  shape\n  fix-pr\n  pr-summary\n  precommit\n  check-pr\n  fixtures [name]\n  goldens check\n  goldens bless <name> --reason <reason>\n  golden-drift\n  metrics\n  test-oracle-report\n  check-test-oracles\n  test-efficiency-report\n  dogfood\n  critic\n  goals status|next|report\n  reports index\n  receipts [check]\n  ci-fast\n  ci-full\n  check-static-language\n  check-no-panic-family\n  check-allow-attributes\n  check-local-context\n  check-file-policy\n  check-executable-files\n  check-workflows\n  check-spec-format\n  check-fixture-contracts\n  check-traceability\n  check-spec-ids\n  check-behavior-manifest\n  check-capabilities\n  check-workspace-shape\n  check-architecture\n  check-public-api\n  check-output-contracts\n  check-doc-index\n  check-readme-state\n  markdown-links\n  check-campaign\n  check-goals\n  check-pr-shape\n  check-generated\n  check-dependencies\n  check-supply-chain\n  check-process-policy\n  check-network-policy\n  package\n  publish-dry-run"
    );
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

fn pr_summary() -> Result<(), String> {
    let changes = collect_pr_changes()?;
    let body = pr_summary_body(&changes);
    write_report("pr-summary.md", &body)
}

fn check_pr_shape() -> Result<(), String> {
    let changes = collect_pr_changes()?;
    let warnings = pr_shape_warnings(&changes);
    write_report("pr-shape.md", &pr_shape_report_body(&warnings))
}

fn critic() -> Result<(), String> {
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

fn reports(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("index") => reports_index(),
        Some(other) => Err(format!(
            "unknown reports command `{other}`\nusage: cargo xtask reports index"
        )),
        None => Err("missing reports command\nusage: cargo xtask reports index".to_string()),
    }
}

fn receipts(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None => receipts_write(),
        Some("check") => receipts_check(),
        Some(other) => Err(format!(
            "unknown receipts command `{other}`\nusage: cargo xtask receipts\n       cargo xtask receipts check"
        )),
    }
}

fn reports_index() -> Result<(), String> {
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

fn receipts_write() -> Result<(), String> {
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
    "# ripr precommit report\n\nStatus: pass\n\nChecks:\n\n- `cargo fmt --check`\n- `cargo xtask check-static-language`\n- `cargo xtask check-no-panic-family`\n- `cargo xtask check-allow-attributes`\n- `cargo xtask check-local-context`\n- `cargo xtask check-file-policy`\n- `cargo xtask check-executable-files`\n- `cargo xtask check-workflows`\n- `cargo xtask check-spec-format`\n- `cargo xtask check-fixture-contracts`\n- `cargo xtask check-traceability`\n- `cargo xtask check-capabilities`\n- `cargo xtask check-workspace-shape`\n- `cargo xtask check-architecture`\n- `cargo xtask check-public-api`\n- `cargo xtask check-output-contracts`\n- `cargo xtask check-doc-index`\n- `cargo xtask check-readme-state`\n- `cargo xtask markdown-links`\n- `cargo xtask check-campaign`\n- `cargo xtask check-pr-shape`\n- `cargo xtask check-generated`\n\nNext command:\n\n```bash\ncargo xtask check-pr\n```\n".to_string()
}

fn check_pr_report_body() -> String {
    "# ripr check-pr report\n\nStatus: pass\n\nChecks:\n\n- `cargo xtask ci-fast`\n- `cargo clippy --workspace --all-targets -- -D warnings`\n- `cargo doc --workspace --no-deps`\n- `cargo xtask pr-summary`\n\nReports:\n\n- `target/ripr/reports/pr-summary.md`\n- `target/ripr/reports/check-pr.md`\n\nRelease/package gates are intentionally left to `cargo xtask ci-full` or release-specific workflows.\n".to_string()
}

fn fixtures(name: Option<&String>) -> Result<(), String> {
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

fn goldens(args: &[String]) -> Result<(), String> {
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

fn golden_drift() -> Result<(), String> {
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
                expected_line,
                actual_line
            ));
        }
    }

    None
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

fn check_static_language() -> Result<(), String> {
    let allowed = read_path_allowlist(".ripr/static-language-allowlist.txt")?;
    let forbidden = forbidden_static_terms();
    let mut violations = Vec::new();

    for path in collect_files(Path::new("."))? {
        let normalized = normalize_path(&path);
        if !is_static_language_candidate(&normalized) || allowed.contains(&normalized) {
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

    finish_policy_report(
        PolicyReportSpec {
            report_file: "static-language.md",
            check: "check-static-language",
            why_it_matters: "Static output must preserve the boundary between draft exposure evidence and real mutation results.",
            fix_kind: FixKind::ReviewerDecisionRequired,
            recommended_fixes: &[
                "Rewrite static product output to use the approved exposure vocabulary.",
                "If this is explanatory documentation, add a narrow allowlist entry with a reason.",
            ],
            rerun_command: "cargo xtask check-static-language",
            exception_template: Some(".ripr/static-language-allowlist.txt entry:\npath/to/file.md"),
        },
        &violations,
    )
}

fn check_no_panic_family() -> Result<(), String> {
    let allowlist = read_count_allowlist(".ripr/no-panic-allowlist.txt")?;
    let roots = [
        Path::new("crates/ripr/src"),
        Path::new("crates/ripr/tests"),
        Path::new("xtask/src"),
    ];
    let patterns = forbidden_panic_patterns();
    let mut counts = BTreeMap::<(String, String), usize>::new();

    for root in roots {
        if !root.exists() {
            continue;
        }
        for path in collect_files(root)? {
            if path.extension().and_then(|value| value.to_str()) != Some("rs") {
                continue;
            }
            let normalized = normalize_path(&path);
            let text = read_text_lossy(&path)?;
            for pattern in &patterns {
                let count = text.matches(pattern).count();
                if count > 0 {
                    counts.insert((normalized.clone(), pattern.clone()), count);
                }
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

    for ((path, pattern), allowed) in &allowlist {
        let actual = counts
            .get(&(path.clone(), pattern.clone()))
            .copied()
            .unwrap_or(0);
        if actual > *allowed {
            violations.push(format!(
                "{path} contains `{pattern}` {actual} time(s), allowed {allowed}"
            ));
        }
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
                ".ripr/no-panic-allowlist.txt entry:\npath/to/file.rs|pattern|max_count|reason",
            ),
        },
        &violations,
    )
}

fn check_allow_attributes() -> Result<(), String> {
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

fn check_local_context() -> Result<(), String> {
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

fn check_file_policy() -> Result<(), String> {
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

fn check_executable_files() -> Result<(), String> {
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

fn check_workflows() -> Result<(), String> {
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

fn metrics_report() -> Result<(), String> {
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

fn test_oracle_report() -> Result<(), String> {
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

fn test_efficiency_report() -> Result<(), String> {
    let tests = collect_test_oracle_tests()?;
    let entries = tests.iter().map(test_efficiency_entry).collect::<Vec<_>>();
    write_report(
        "test-efficiency.md",
        &test_efficiency_report_markdown(&entries),
    )?;
    write_report(
        "test-efficiency.json",
        &test_efficiency_report_json(&entries),
    )
}

fn test_efficiency_entry(test: &TestOracleTest) -> TestEfficiencyEntry {
    let reached_owners = test_efficiency_reached_owners(test);
    let observed_values = test_efficiency_observed_values(test);
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
        class: test_efficiency_class(test, &reached_owners),
        oracle_kind: test_efficiency_oracle_kind(test).to_string(),
        oracle_strength: test.class.as_str(),
        reached_owners,
        observed_values,
        static_limitations,
    }
}

fn test_efficiency_class(test: &TestOracleTest, reached_owners: &[String]) -> &'static str {
    if reached_owners.is_empty() {
        return "opaque";
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
        ("opaque", 0usize),
    ]);
    for entry in entries {
        if let Some(count) = counts.get_mut(entry.class) {
            *count += 1;
        }
    }
    counts
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

fn test_efficiency_report_markdown(entries: &[TestEfficiencyEntry]) -> String {
    let counts = test_efficiency_counts(entries);
    let mut body = format!(
        "# ripr test efficiency report\n\nStatus: {}\n\nMode: advisory\n\nThis report builds a per-test evidence ledger from static Rust test facts. It records apparent owner calls, oracle shape, activation values, and static limitations so reviewers can spot low-discriminator patterns without making the report blocking.\n\n## Summary\n\n- Strong discriminator: {}\n- Useful but broad: {}\n- Smoke only: {}\n- Opaque: {}\n- Tests scanned: {}\n\n",
        test_efficiency_report_status(entries),
        counts.get("strong_discriminator").copied().unwrap_or(0),
        counts.get("useful_but_broad").copied().unwrap_or(0),
        counts.get("smoke_only").copied().unwrap_or(0),
        counts.get("opaque").copied().unwrap_or(0),
        entries.len(),
    );

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

    body.push_str("## Ledger\n\n| Test | Class | Oracle | Reached owners | Observed values | Static limitations |\n| --- | --- | --- | --- | --- | --- |\n");
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
        body.push_str(&format!(
            "| `{}`:{} `{}` | `{}` | `{}` / `{}` | {} | {} | {} |\n",
            normalize_path(&entry.path),
            entry.line,
            markdown_cell(&entry.name),
            entry.class,
            entry.oracle_kind,
            entry.oracle_strength,
            markdown_cell(&owners),
            markdown_cell(&values),
            markdown_cell(&limitations)
        ));
    }
    body
}

fn test_efficiency_report_json(entries: &[TestEfficiencyEntry]) -> String {
    let counts = test_efficiency_counts(entries);
    let mut body = format!(
        "{{\n  \"schema_version\": \"0.1\",\n  \"status\": \"{}\",\n  \"advisory\": true,\n  \"counts\": {{\n    \"strong_discriminator\": {},\n    \"useful_but_broad\": {},\n    \"smoke_only\": {},\n    \"opaque\": {}\n  }},\n  \"tests\": [\n",
        test_efficiency_report_status(entries),
        counts.get("strong_discriminator").copied().unwrap_or(0),
        counts.get("useful_but_broad").copied().unwrap_or(0),
        counts.get("smoke_only").copied().unwrap_or(0),
        counts.get("opaque").copied().unwrap_or(0)
    );

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
        body.push_str("]\n    }");
    }
    body.push_str("\n  ]\n}\n");
    body
}

fn dogfood() -> Result<(), String> {
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
    matches!(
        command,
        "shape"
            | "fix-pr"
            | "pr-summary"
            | "precommit"
            | "check-pr"
            | "fixtures"
            | "goldens"
            | "golden-drift"
            | "metrics"
            | "test-oracle-report"
            | "check-test-oracles"
            | "test-efficiency-report"
            | "dogfood"
            | "critic"
            | "goals"
            | "reports"
            | "receipts"
            | "ci-fast"
            | "ci-full"
            | "check-static-language"
            | "check-no-panic-family"
            | "check-allow-attributes"
            | "check-local-context"
            | "check-file-policy"
            | "check-executable-files"
            | "check-workflows"
            | "check-spec-format"
            | "check-fixture-contracts"
            | "check-traceability"
            | "check-spec-ids"
            | "check-behavior-manifest"
            | "check-capabilities"
            | "check-workspace-shape"
            | "check-architecture"
            | "check-public-api"
            | "check-output-contracts"
            | "check-doc-index"
            | "check-readme-state"
            | "markdown-links"
            | "check-campaign"
            | "check-goals"
            | "check-pr-shape"
            | "check-generated"
            | "check-dependencies"
            | "check-supply-chain"
            | "check-process-policy"
            | "check-network-policy"
            | "package"
            | "publish-dry-run"
    )
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
    let output = Command::new("cargo")
        .args(args)
        .output()
        .map_err(|err| format!("failed to run cargo deny: {err}"))?;

    let status = if output.status.success() {
        "pass"
    } else {
        "fail"
    };
    let stdout = redact_current_dir(&String::from_utf8_lossy(&output.stdout));
    let stderr = redact_current_dir(&String::from_utf8_lossy(&output.stderr));
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

fn check_process_policy() -> Result<(), String> {
    check_count_policy(
        "process policy",
        "policy/process_allowlist.txt",
        &process_policy_patterns(),
        is_process_policy_candidate,
    )
}

fn check_network_policy() -> Result<(), String> {
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
                path == ".ripr/static-language-allowlist.txt"
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

fn run_output(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run {program}: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} {} failed with {}",
            args.join(" "),
            output.status
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn run_output_owned(program: &str, args: &[String]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run {program}: {err}"))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{program} {} failed with {}\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            output.status,
            stdout.trim(),
            stderr.trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn run_output_optional(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run {program}: {err}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Ok(String::new())
    }
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

#[cfg(test)]
mod tests {
    use super::{
        CampaignManifest, Capability, ChangedPath, DogfoodRun, LocalContextAllow, MarkdownLink,
        ReceiptRecord, ReportIndexCampaign, ReportIndexEntry, TestOracleClass, critic_findings,
        dogfood_class_counts, dogfood_report_json, dogfood_report_markdown,
        extract_workflow_run_blocks, first_line_difference, glob_matches,
        golden_changes_without_blessing, golden_drift_semantics, guarded_allow_attribute_lints,
        guarded_allow_attributes_in_text, is_bdd_test_name, is_dependency_surface_candidate,
        is_evidence_path, is_generated_candidate, is_known_campaign_command, is_policy_path,
        is_production_path, is_receipt_status, is_snake_case_id, is_spec_id, json_escape,
        json_number_after, json_string_values_for_key, known_xtask_command,
        local_context_line_findings, local_markdown_target, markdown_links_in_text,
        next_checkpoints_from_capabilities, normalize_fixture_human_output,
        normalize_fixture_json_output, normalize_golden_text, parse_campaign_manifest,
        parse_inline_array, parse_reason, pr_shape_warnings, precommit_report_body,
        public_contract_rows, receipt_json, receipt_specs, receipt_status_from_reports,
        report_index_markdown, report_index_missing_expected, report_status_from_text,
        sorted_allowlist_content, spec_id_from_path, status_for_report,
        suspicious_runtime_file_names, test_efficiency_entry, test_efficiency_report_json,
        test_efficiency_report_markdown, test_oracle_report_json, test_oracle_report_markdown,
        test_oracle_tests_in_text, validate_local_context_allowlist, windows_absolute_path_tokens,
        workflow_runtime_violations,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::Path;

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
        assert!(parse_reason(&[]).is_err());
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
        assert!(parse_inline_array("[one]").is_err());
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
fn helper_driven_smoke() {
    build_fixture();
}
"#;
        let tests = test_oracle_tests_in_text(Path::new("crates/ripr/tests/example.rs"), source);
        let entry = test_efficiency_entry(&tests[0]);
        let markdown = test_efficiency_report_markdown(std::slice::from_ref(&entry));
        let json = test_efficiency_report_json(&[entry]);

        assert!(markdown.contains("Mode: advisory"));
        assert!(markdown.contains("helper_driven_smoke"));
        assert!(json.contains("\"advisory\": true"));
        assert!(json.contains("\"smoke_only\": 1"));
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
}
