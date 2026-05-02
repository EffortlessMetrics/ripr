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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(|s| s.as_str()) {
        Some("shape") => shape(),
        Some("fix-pr") => fix_pr(),
        Some("pr-summary") => pr_summary(),
        Some("ci-fast") => ci_fast(),
        Some("ci-full") => ci_full(),
        Some("check-static-language") => check_static_language(),
        Some("check-no-panic-family") => check_no_panic_family(),
        Some("check-file-policy") => check_file_policy(),
        Some("check-executable-files") => check_executable_files(),
        Some("check-workflows") => check_workflows(),
        Some("check-spec-format") => check_spec_format(),
        Some("check-fixture-contracts") => check_fixture_contracts(),
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
    check_static_language()?;
    check_no_panic_family()?;
    check_file_policy()?;
    check_executable_files()?;
    check_workflows()?;
    check_spec_format()?;
    check_fixture_contracts()?;
    check_generated()?;
    check_dependencies()?;
    check_process_policy()?;
    check_network_policy()
}

fn ci_full() -> Result<(), String> {
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
        "xtask commands:\n  shape\n  fix-pr\n  pr-summary\n  ci-fast\n  ci-full\n  check-static-language\n  check-no-panic-family\n  check-file-policy\n  check-executable-files\n  check-workflows\n  check-spec-format\n  check-fixture-contracts\n  check-generated\n  check-dependencies\n  check-process-policy\n  check-network-policy\n  package\n  publish-dry-run"
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
    let body = "# ripr fix-pr report\n\nStatus: pass\n\nActions:\n\n- Ran `cargo xtask shape`.\n- Ran `cargo xtask pr-summary`.\n\nReports:\n\n- `target/ripr/reports/shape.md`\n- `target/ripr/reports/pr-summary.md`\n\nNext commands:\n\n```bash\ncargo xtask ci-fast\n```\n";
    write_report("fix-pr.md", body)
}

fn pr_summary() -> Result<(), String> {
    let changes = collect_pr_changes()?;
    let body = pr_summary_body(&changes);
    write_report("pr-summary.md", &body)
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

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "static language check failed:\n{}",
            violations.join("\n")
        ))
    }
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

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "panic-family check failed:\n{}",
            violations.join("\n")
        ))
    }
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

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "file policy check failed:\n{}",
            violations.join("\n")
        ))
    }
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

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "executable-file check failed:\n{}",
            violations.join("\n")
        ))
    }
}

fn check_workflows() -> Result<(), String> {
    let budgets = read_workflow_budgets("policy/workflow_allowlist.txt")?;
    let mut violations = Vec::new();

    for path in collect_files(Path::new(".github/workflows"))? {
        let normalized = normalize_path(&path);
        if !(normalized.ends_with(".yml") || normalized.ends_with(".yaml")) {
            continue;
        }
        let budget = budgets.get(&normalized).ok_or_else(|| {
            format!("missing workflow budget for {normalized} in policy/workflow_allowlist.txt")
        })?;
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

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!("workflow check failed:\n{}", violations.join("\n")))
    }
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

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "spec format check failed:\n{}",
            violations.join("\n")
        ))
    }
}

fn check_fixture_contracts() -> Result<(), String> {
    let fixtures_dir = Path::new("fixtures");
    if !fixtures_dir.exists() {
        return Ok(());
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

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "fixture contract check failed:\n{}",
            violations.join("\n")
        ))
    }
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

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "generated-file check failed:\n{}",
            violations.join("\n")
        ))
    }
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

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "dependency policy check failed:\n{}",
            violations.join("\n")
        ))
    }
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

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!("{label} check failed:\n{}", violations.join("\n")))
    }
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
    body.push_str("- `cargo xtask ci-fast`\n");
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
        ChangedPath, extract_workflow_run_blocks, glob_matches, is_dependency_surface_candidate,
        is_evidence_path, is_generated_candidate, is_policy_path, is_production_path,
        public_contract_rows, sorted_allowlist_content,
    };
    use std::collections::BTreeSet;

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
}
