use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const REPORT_JSON: &str = "target/ripr/reports/precommit-v2.json";
const REPORT_MARKDOWN: &str = "target/ripr/reports/precommit-v2.md";

#[derive(Debug, Clone, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoPackage {
    name: String,
    manifest_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageRoot {
    name: String,
    relative_root: String,
}

#[derive(Debug, Clone, Serialize)]
struct CommandResult {
    command: String,
    outcome: String,
    detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PrecommitReport {
    schema_version: &'static str,
    status: String,
    repository_root: String,
    merge_base: String,
    changed_files: Vec<String>,
    impacted_packages: Vec<String>,
    workspace_clippy: bool,
    commands: Vec<CommandResult>,
    skipped: Vec<String>,
}

pub(super) fn run() -> Result<(), String> {
    let root = repository_root()?;
    let merge_base = resolve_merge_base(&root)?;

    let changed_files = discover_changed_files(&root, &merge_base)?;
    let workspace_clippy = needs_workspace_clippy(&changed_files);
    let package_roots = if changed_files.iter().any(|path| is_rust_relevant(path)) {
        load_package_roots(&root)?
    } else {
        Vec::new()
    };
    let impacted_packages = if workspace_clippy {
        Vec::new()
    } else {
        select_impacted_packages(&changed_files, &package_roots)
    };

    let mut report = PrecommitReport {
        schema_version: "0.1",
        status: "running".to_string(),
        repository_root: normalize_path(&root),
        merge_base,
        changed_files,
        impacted_packages,
        workspace_clippy,
        commands: Vec::new(),
        skipped: Vec::new(),
    };

    if let Err(error) = crate::precommit() {
        record_failure(&mut report, "existing repository policy precommit", &error);
        report.status = classify_failure(&error).to_string();
        write_report(&root, &report)?;
        return Err(error);
    }
    record_pass(&mut report, "existing repository policy precommit");

    let diff_checks = [
        vec![
            "diff".to_string(),
            "--check".to_string(),
            format!("{}...HEAD", report.merge_base),
        ],
        vec![
            "diff".to_string(),
            "--cached".to_string(),
            "--check".to_string(),
        ],
        vec!["diff".to_string(), "--check".to_string()],
    ];
    for args in diff_checks {
        let owned = git_args(&root, &args);
        if let Err(error) = crate::run::run_owned("git", &owned) {
            record_failure(&mut report, &format_command("git", &owned), &error);
            report.status = classify_failure(&error).to_string();
            write_report(&root, &report)?;
            return Err(error);
        }
        record_pass(&mut report, &format_command("git", &owned));
    }

    if report.workspace_clippy {
        let args = workspace_clippy_args(&root);
        run_clippy(&root, &mut report, args)?;
    } else if report.impacted_packages.is_empty() {
        report.skipped.push(
            "Clippy skipped: no Rust package or workspace-wide Rust surface changed.".to_string(),
        );
    } else {
        for package in report.impacted_packages.clone() {
            let args = package_clippy_args(&root, &package);
            run_clippy(&root, &mut report, args)?;
        }
    }

    report.status = "pass".to_string();
    write_report(&root, &report)
}

fn run_clippy(root: &Path, report: &mut PrecommitReport, args: Vec<String>) -> Result<(), String> {
    let command = format_command("cargo", &args);
    match crate::run::run_owned("cargo", &args) {
        Ok(()) => {
            record_pass(report, &command);
            Ok(())
        }
        Err(error) => {
            record_failure(report, &command, &error);
            report.status = classify_failure(&error).to_string();
            write_report(root, report)?;
            Err(error)
        }
    }
}

fn repository_root() -> Result<PathBuf, String> {
    let output = crate::run::run_output("git", &["rev-parse", "--show-toplevel"])?;
    let root = output.trim();
    if root.is_empty() {
        return Err("git rev-parse returned an empty repository root".to_string());
    }
    fs::canonicalize(root)
        .map_err(|error| format!("canonicalize repository root `{root}`: {error}"))
}

fn resolve_merge_base(root: &Path) -> Result<String, String> {
    let mut failures = Vec::new();
    for base in ["origin/main", "main"] {
        let args = vec![
            "merge-base".to_string(),
            base.to_string(),
            "HEAD".to_string(),
        ];
        match git_output(root, &args) {
            Ok(output) if !output.trim().is_empty() => return Ok(output.trim().to_string()),
            Ok(_) => failures.push(format!("{base}: empty merge-base output")),
            Err(error) => failures.push(format!("{base}: {error}")),
        }
    }
    Err(format!(
        "precommit could not resolve a merge base against `origin/main` or local `main`: {}",
        failures.join("; ")
    ))
}

fn discover_changed_files(root: &Path, merge_base: &str) -> Result<Vec<String>, String> {
    let mut paths = BTreeSet::new();
    let commands = [
        vec![
            "diff".to_string(),
            "--name-only".to_string(),
            "--diff-filter=ACMRD".to_string(),
            format!("{merge_base}...HEAD"),
        ],
        vec![
            "diff".to_string(),
            "--name-only".to_string(),
            "--cached".to_string(),
            "--diff-filter=ACMRD".to_string(),
        ],
        vec![
            "diff".to_string(),
            "--name-only".to_string(),
            "--diff-filter=ACMRD".to_string(),
        ],
        vec![
            "ls-files".to_string(),
            "--others".to_string(),
            "--exclude-standard".to_string(),
        ],
    ];
    for args in commands {
        let output = git_output(root, &args)?;
        for line in output.lines() {
            let normalized = normalize_repo_path(line);
            if !normalized.is_empty() {
                paths.insert(normalized);
            }
        }
    }
    Ok(paths.into_iter().collect())
}

fn git_output(root: &Path, args: &[String]) -> Result<String, String> {
    crate::run::run_output_owned("git", &git_args(root, args))
}

fn git_args(root: &Path, args: &[String]) -> Vec<String> {
    let mut owned = vec!["-C".to_string(), root.display().to_string()];
    owned.extend(args.iter().cloned());
    owned
}

fn load_package_roots(root: &Path) -> Result<Vec<PackageRoot>, String> {
    let manifest = root.join("Cargo.toml");
    let args = vec![
        "metadata".to_string(),
        "--format-version".to_string(),
        "1".to_string(),
        "--no-deps".to_string(),
        "--manifest-path".to_string(),
        manifest.display().to_string(),
    ];
    let output = crate::run::run_output_owned("cargo", &args)?;
    let metadata: CargoMetadata = serde_json::from_str(&output)
        .map_err(|error| format!("parse cargo metadata for precommit: {error}"))?;
    let mut roots = Vec::new();
    for package in metadata.packages {
        let Some(parent) = package.manifest_path.parent() else {
            return Err(format!("package `{}` manifest has no parent", package.name));
        };
        let canonical_parent = fs::canonicalize(parent).map_err(|error| {
            format!(
                "canonicalize package `{}` manifest root {}: {error}",
                package.name,
                parent.display()
            )
        })?;
        let relative = canonical_parent.strip_prefix(root).map_err(|error| {
            format!(
                "package `{}` manifest root {} is outside repository {}: {error}",
                package.name,
                canonical_parent.display(),
                root.display()
            )
        })?;
        roots.push(PackageRoot {
            name: package.name,
            relative_root: normalize_path(relative),
        });
    }
    roots.sort_by(|left, right| {
        right
            .relative_root
            .len()
            .cmp(&left.relative_root.len())
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(roots)
}

fn select_impacted_packages(changed: &[String], roots: &[PackageRoot]) -> Vec<String> {
    let mut packages = BTreeSet::new();
    for path in changed.iter().filter(|path| is_rust_relevant(path)) {
        for root in roots {
            if path_is_under(path, &root.relative_root) {
                packages.insert(root.name.clone());
                break;
            }
        }
    }
    packages.into_iter().collect()
}

fn path_is_under(path: &str, root: &str) -> bool {
    if root.is_empty() {
        return true;
    }
    path == format!("{root}/Cargo.toml") || path.starts_with(&format!("{root}/"))
}

fn is_rust_relevant(path: &str) -> bool {
    path.ends_with(".rs") || path.ends_with("Cargo.toml") || path.ends_with("build.rs")
}

fn needs_workspace_clippy(changed: &[String]) -> bool {
    changed.iter().any(|path| {
        matches!(
            path.as_str(),
            "Cargo.toml"
                | "Cargo.lock"
                | "rust-toolchain"
                | "rust-toolchain.toml"
                | "clippy.toml"
                | "rustfmt.toml"
                | "xtask/Cargo.toml"
                | "xtask/src/dispatch.rs"
                | "xtask/src/precommit_v2.rs"
        ) || path.starts_with(".cargo/")
            || path.starts_with("policy/clippy")
            || path.starts_with("policy/lint")
    })
}

fn package_clippy_args(root: &Path, package: &str) -> Vec<String> {
    vec![
        "clippy".to_string(),
        "--manifest-path".to_string(),
        root.join("Cargo.toml").display().to_string(),
        "-p".to_string(),
        package.to_string(),
        "--all-targets".to_string(),
        "--".to_string(),
        "-D".to_string(),
        "warnings".to_string(),
    ]
}

fn workspace_clippy_args(root: &Path) -> Vec<String> {
    vec![
        "clippy".to_string(),
        "--manifest-path".to_string(),
        root.join("Cargo.toml").display().to_string(),
        "--workspace".to_string(),
        "--all-targets".to_string(),
        "--".to_string(),
        "-D".to_string(),
        "warnings".to_string(),
    ]
}

fn record_pass(report: &mut PrecommitReport, command: &str) {
    report.commands.push(CommandResult {
        command: command.to_string(),
        outcome: "pass".to_string(),
        detail: None,
    });
}

fn record_failure(report: &mut PrecommitReport, command: &str, detail: &str) {
    report.commands.push(CommandResult {
        command: command.to_string(),
        outcome: "failed".to_string(),
        detail: Some(detail.to_string()),
    });
}

fn classify_failure(error: &str) -> &'static str {
    if error.contains("failed to run") || error.contains("timed out") {
        "infrastructure_failure"
    } else {
        "source_or_policy_failure"
    }
}

fn write_report(root: &Path, report: &PrecommitReport) -> Result<(), String> {
    let json_path = root.join(REPORT_JSON);
    let markdown_path = root.join(REPORT_MARKDOWN);
    let Some(parent) = json_path.parent() else {
        return Err("precommit report path has no parent".to_string());
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("create precommit report directory: {error}"))?;
    let json = serde_json::to_string_pretty(report)
        .map_err(|error| format!("serialize precommit report: {error}"))?;
    fs::write(&json_path, format!("{json}\n"))
        .map_err(|error| format!("write {}: {error}", json_path.display()))?;
    fs::write(&markdown_path, report_markdown(report))
        .map_err(|error| format!("write {}: {error}", markdown_path.display()))
}

fn report_markdown(report: &PrecommitReport) -> String {
    let mut out = format!(
        "# ripr precommit v2\n\n- Status: `{}`\n- Merge base: `{}`\n- Changed files: `{}`\n- Workspace Clippy: `{}`\n\n",
        report.status,
        report.merge_base,
        report.changed_files.len(),
        report.workspace_clippy
    );
    out.push_str("## Impacted packages\n\n");
    if report.impacted_packages.is_empty() {
        out.push_str("- none\n");
    } else {
        for package in &report.impacted_packages {
            out.push_str(&format!("- `{package}`\n"));
        }
    }
    out.push_str("\n## Commands\n\n");
    for command in &report.commands {
        out.push_str(&format!("- `{}`: `{}`\n", command.command, command.outcome));
    }
    out.push_str("\n## Skipped\n\n");
    if report.skipped.is_empty() {
        out.push_str("- none\n");
    } else {
        for reason in &report.skipped {
            out.push_str(&format!("- {reason}\n"));
        }
    }
    out
}

fn format_command(program: &str, args: &[String]) -> String {
    if args.is_empty() {
        program.to_string()
    } else {
        format!("{program} {}", args.join(" "))
    }
}

fn normalize_repo_path(value: &str) -> String {
    value.trim().replace('\\', "/")
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::{PackageRoot, needs_workspace_clippy, select_impacted_packages};

    #[test]
    fn docs_only_change_selects_no_package() {
        let changed = vec!["docs/AGENT_WORKFLOWS.md".to_string()];
        let roots = vec![PackageRoot {
            name: "ripr".to_string(),
            relative_root: "crates/ripr".to_string(),
        }];
        assert!(select_impacted_packages(&changed, &roots).is_empty());
        assert!(!needs_workspace_clippy(&changed));
    }

    #[test]
    fn rust_change_selects_nearest_package() {
        let changed = vec!["crates/ripr/src/lib.rs".to_string()];
        let roots = vec![
            PackageRoot {
                name: "ripr".to_string(),
                relative_root: "crates/ripr".to_string(),
            },
            PackageRoot {
                name: "xtask".to_string(),
                relative_root: "xtask".to_string(),
            },
        ];
        assert_eq!(
            select_impacted_packages(&changed, &roots),
            vec!["ripr".to_string()]
        );
        assert!(!needs_workspace_clippy(&changed));
    }

    #[test]
    fn root_package_receives_unclaimed_rust_file() {
        let changed = vec!["src/lib.rs".to_string()];
        let roots = vec![
            PackageRoot {
                name: "nested".to_string(),
                relative_root: "crates/nested".to_string(),
            },
            PackageRoot {
                name: "root-package".to_string(),
                relative_root: String::new(),
            },
        ];
        assert_eq!(
            select_impacted_packages(&changed, &roots),
            vec!["root-package".to_string()]
        );
    }

    #[test]
    fn workspace_manifest_and_planner_widen_clippy() {
        for path in ["Cargo.toml", "Cargo.lock", "xtask/src/precommit_v2.rs"] {
            assert!(needs_workspace_clippy(&[path.to_string()]));
        }
    }
}
