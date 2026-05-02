#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

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
        Some("ci-fast") => ci_fast(),
        Some("ci-full") => ci_full(),
        Some("check-static-language") => check_static_language(),
        Some("check-no-panic-family") => check_no_panic_family(),
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
        Some("check-generated") => check_generated(),
        Some("check-dependencies") => check_dependencies(),
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
    write_report("check-pr.md", &body)
}

fn run_policy_checks() -> Result<(), String> {
    check_static_language()?;
    check_no_panic_family()?;
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
        "xtask commands:\n  shape\n  fix-pr\n  pr-summary\n  precommit\n  check-pr\n  fixtures [name]\n  goldens check\n  goldens bless <name> --reason <reason>\n  metrics\n  ci-fast\n  ci-full\n  check-static-language\n  check-no-panic-family\n  check-file-policy\n  check-executable-files\n  check-workflows\n  check-spec-format\n  check-fixture-contracts\n  check-traceability\n  check-spec-ids\n  check-behavior-manifest\n  check-capabilities\n  check-workspace-shape\n  check-architecture\n  check-public-api\n  check-output-contracts\n  check-doc-index\n  check-generated\n  check-dependencies\n  check-process-policy\n  check-network-policy\n  package\n  publish-dry-run"
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

fn precommit_report_body() -> String {
    "# ripr precommit report\n\nStatus: pass\n\nChecks:\n\n- `cargo fmt --check`\n- `cargo xtask check-static-language`\n- `cargo xtask check-no-panic-family`\n- `cargo xtask check-file-policy`\n- `cargo xtask check-executable-files`\n- `cargo xtask check-workflows`\n- `cargo xtask check-spec-format`\n- `cargo xtask check-fixture-contracts`\n- `cargo xtask check-traceability`\n- `cargo xtask check-capabilities`\n- `cargo xtask check-workspace-shape`\n- `cargo xtask check-architecture`\n- `cargo xtask check-public-api`\n- `cargo xtask check-output-contracts`\n- `cargo xtask check-doc-index`\n- `cargo xtask check-generated`\n\nNext command:\n\n```bash\ncargo xtask check-pr\n```\n".to_string()
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
        violations.extend(fixture_contract_violations(path)?);
    }

    let body = fixture_report_body(name.map(String::as_str), &selected, &violations);
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
    let fixture_dirs = fixture_dirs()?;
    let mut violations = Vec::new();
    let mut golden_files = Vec::new();
    for fixture in &fixture_dirs {
        let expected = fixture.join("expected");
        if !expected.exists() {
            violations.push(format!("{} is missing expected/", normalize_path(fixture)));
            continue;
        }
        golden_files.extend(
            collect_files(&expected)?
                .into_iter()
                .map(|path| normalize_path(&path)),
        );
        let check_json = expected.join("check.json");
        if !check_json.exists() {
            violations.push(format!(
                "{} is missing expected/check.json",
                normalize_path(fixture)
            ));
        }
    }
    golden_files.sort();
    let body = goldens_check_report_body(&fixture_dirs, &golden_files, &violations);
    write_report("goldens.md", &body)?;
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "goldens check failed; see target/ripr/reports/goldens.md\n{}",
            violations.join("\n")
        ))
    }
}

fn goldens_bless(name: &str, reason: &str) -> Result<(), String> {
    let fixture = fixture_dir_for_name(name)?;
    if !fixture.exists() {
        return Err(format!(
            "fixture does not exist: {}",
            normalize_path(&fixture)
        ));
    }
    let expected = fixture.join("expected");
    if !expected.exists() {
        return Err(format!(
            "fixture is missing expected/: {}",
            normalize_path(&fixture)
        ));
    }
    let changelog = expected.join("CHANGELOG.md");
    let entry = format!(
        "\n## Pending\n\nReason:\n{reason}\n\nCommand:\n`cargo xtask goldens bless {name} --reason \"...\"`\n"
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
        "# ripr goldens bless report\n\nStatus: pass\n\nFixture:\n- `{}`\n\nReason:\n```text\n{reason}\n```\n\nUpdated:\n- `{}`\n",
        normalize_path(&fixture),
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

fn fixture_report_body(name: Option<&str>, selected: &[PathBuf], violations: &[String]) -> String {
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
    write_violations_section(&mut body, violations);
    body
}

fn goldens_check_report_body(
    fixtures: &[PathBuf],
    golden_files: &[String],
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
    body.push_str("## Golden Files\n\n");
    if golden_files.is_empty() {
        body.push_str("No golden files found.\n\n");
    } else {
        for path in golden_files {
            body.push_str(&format!("- `{path}`\n"));
        }
        body.push('\n');
    }
    write_violations_section(&mut body, violations);
    body
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
                "Add or adjust a workflow budget entry only when the workflow surface is intentionally larger.",
            ],
            rerun_command: "cargo xtask check-workflows",
            exception_template: Some(
                "policy/workflow_allowlist.txt entry:\n.github/workflows/name.yml|max_non_empty_lines|reason",
            ),
        },
        &violations,
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
    let domain = read_text_lossy(Path::new("crates/ripr/src/domain.rs"))?;
    let app = read_text_lossy(Path::new("crates/ripr/src/app.rs"))?;
    let json_output = read_text_lossy(Path::new("crates/ripr/src/output/json.rs"))?;
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
                    "crates/ripr/src/output/json.rs",
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
            "exposure_class" | "severity" | "probe_family" | "delta" | "stage_state"
            | "confidence" | "oracle_strength" => {
                require_contract_value(
                    "crates/ripr/src/domain.rs",
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
        "PR_AUTOMATION.md",
        "GOAL_MODE.md",
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
        "docs/PR_AUTOMATION.md",
        "docs/GOAL_MODE.md",
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

fn reports_dir() -> PathBuf {
    Path::new("target").join("ripr").join("reports")
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
                matches!(path, "crates/ripr/src/cli.rs" | "crates/ripr/src/main.rs")
                    || path.starts_with("docs/reference/cli")
            }),
        ),
        (
            "JSON",
            paths_matching(changes, |path| {
                path == "crates/ripr/src/output/json.rs" || path == "docs/OUTPUT_SCHEMA.md"
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
        Capability, ChangedPath, extract_workflow_run_blocks, glob_matches,
        is_dependency_surface_candidate, is_evidence_path, is_generated_candidate, is_policy_path,
        is_production_path, is_snake_case_id, is_spec_id, json_escape, parse_inline_array,
        parse_reason, precommit_report_body, public_contract_rows, sorted_allowlist_content,
        spec_id_from_path,
    };
    use std::collections::BTreeSet;
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
                path: "crates/ripr/src/output/json.rs".to_string(),
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

        assert_eq!(json, vec!["crates/ripr/src/output/json.rs (M)"]);
        assert_eq!(lsp, vec!["editors/vscode/src/client.ts (M)"]);
    }

    #[test]
    fn precommit_report_points_to_review_ready_gate() {
        let body = precommit_report_body();

        assert!(body.contains("cargo fmt --check"));
        assert!(body.contains("cargo xtask check-pr"));
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
}
