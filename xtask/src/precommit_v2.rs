use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::run::{ProcessErrorKind, capture_output_with_timeout, capture_process_output};

#[cfg(test)]
use crate::run::run_process_status;

const REPORT_JSON: &str = "target/ripr/reports/precommit-v2.json";
const REPORT_MARKDOWN: &str = "target/ripr/reports/precommit-v2.md";
const PRECOMMIT_COMMAND_TIMEOUT: Duration = Duration::from_mins(15);

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum PrecommitFailureKind {
    SourceOrPolicy,
    Infrastructure,
    IncompleteEvidence,
}

impl PrecommitFailureKind {
    fn status(self) -> &'static str {
        match self {
            Self::SourceOrPolicy => "source_or_policy_failure",
            Self::Infrastructure => "infrastructure_failure",
            Self::IncompleteEvidence => "incomplete_evidence",
        }
    }
}

#[derive(Debug)]
struct PrecommitError {
    kind: PrecommitFailureKind,
    message: String,
}

impl PrecommitError {
    fn new(kind: PrecommitFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ChangedPath {
    kind: String,
    path: String,
    old_path: Option<String>,
    origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ChangeSet {
    changes: Vec<ChangedPath>,
    complete: bool,
    limitations: Vec<String>,
}

impl ChangeSet {
    fn empty() -> Self {
        Self {
            changes: Vec::new(),
            complete: false,
            limitations: Vec::new(),
        }
    }

    fn paths(&self) -> Vec<String> {
        let mut paths = BTreeSet::new();
        for change in &self.changes {
            paths.insert(change.path.clone());
            if let Some(old_path) = &change.old_path {
                paths.insert(old_path.clone());
            }
        }
        paths.into_iter().collect()
    }
}

#[derive(Debug, Clone, Serialize)]
struct ImpactPlan {
    impacted_packages: Vec<String>,
    workspace_clippy: bool,
    workspace_widening_reason: Option<String>,
    skipped_reason: Option<String>,
}

impl ImpactPlan {
    fn empty() -> Self {
        Self {
            impacted_packages: Vec::new(),
            workspace_clippy: false,
            workspace_widening_reason: None,
            skipped_reason: None,
        }
    }
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
    failure_kind: Option<PrecommitFailureKind>,
    repository_root: String,
    head_sha: Option<String>,
    merge_base_ref: Option<String>,
    merge_base_sha: Option<String>,
    change_set: ChangeSet,
    impact_plan: ImpactPlan,
    commands: Vec<CommandResult>,
    skipped: Vec<String>,
    limitations: Vec<String>,
    report_digest: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    manifest_path: PathBuf,
    dependencies: Vec<CargoDependency>,
    targets: Vec<CargoTarget>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoDependency {
    name: String,
    package: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoTarget {
    kind: Vec<String>,
}

#[derive(Debug, Clone)]
struct PackageRoot {
    name: String,
    relative_root: String,
    dependencies: Vec<String>,
    proc_macro: bool,
}

pub(super) fn run() -> Result<(), String> {
    let root = repository_root().map_err(|error| error.message)?;
    let mut report = PrecommitReport {
        schema_version: "0.2",
        status: "running".to_string(),
        failure_kind: None,
        repository_root: "$REPO".to_string(),
        head_sha: None,
        merge_base_ref: None,
        merge_base_sha: None,
        change_set: ChangeSet::empty(),
        impact_plan: ImpactPlan::empty(),
        commands: Vec::new(),
        skipped: Vec::new(),
        limitations: Vec::new(),
        report_digest: String::new(),
    };

    let head_sha = match git_text(&root, &["rev-parse", "HEAD"], "read HEAD") {
        Ok(value) if is_sha(value.trim()) => value.trim().to_string(),
        Ok(value) => {
            return fail_with_report(
                &root,
                &mut report,
                PrecommitFailureKind::IncompleteEvidence,
                "read HEAD",
                format!("git returned a non-SHA HEAD value `{}`", value.trim()),
            );
        }
        Err(error) => {
            return fail_with_report(&root, &mut report, error.kind, "read HEAD", error.message);
        }
    };
    report.head_sha = Some(head_sha);

    let (merge_base_ref, merge_base_sha) = match resolve_merge_base(&root) {
        Ok(value) => value,
        Err(error) => {
            return fail_with_report(
                &root,
                &mut report,
                error.kind,
                "resolve accepted merge base",
                error.message,
            );
        }
    };
    report.merge_base_ref = Some(merge_base_ref);
    report.merge_base_sha = Some(merge_base_sha.clone());

    let change_set = match discover_change_set(&root, &merge_base_sha) {
        Ok(value) => value,
        Err(error) => {
            return fail_with_report(
                &root,
                &mut report,
                error.kind,
                "discover complete change set",
                error.message,
            );
        }
    };
    if !change_set.complete {
        return fail_with_report(
            &root,
            &mut report,
            PrecommitFailureKind::IncompleteEvidence,
            "discover complete change set",
            change_set.limitations.join("; "),
        );
    }
    report.change_set = change_set;

    let package_roots = if report
        .change_set
        .paths()
        .iter()
        .any(|path| is_rust_relevant(path))
    {
        let metadata_args = cargo_metadata_args(&root);
        let metadata_command = format_command(
            "cargo",
            &metadata_args.iter().map(String::as_str).collect::<Vec<_>>(),
            &root,
        );
        match load_package_roots(&root) {
            Ok(value) => {
                record_pass(&mut report, &metadata_command);
                value
            }
            Err(error) => {
                return fail_with_report(
                    &root,
                    &mut report,
                    error.kind,
                    metadata_command,
                    error.message,
                );
            }
        }
    } else {
        Vec::new()
    };
    report.impact_plan = build_impact_plan(&report.change_set, &package_roots);

    if let Err(error) = crate::precommit() {
        return fail_with_report(
            &root,
            &mut report,
            PrecommitFailureKind::SourceOrPolicy,
            "existing repository policy precommit",
            error,
        );
    }
    record_pass(&mut report, "existing repository policy precommit");

    let committed_diff = format!("{}...HEAD", merge_base_sha);
    let diff_checks = [
        vec!["diff".to_string(), "--check".to_string(), committed_diff],
        vec![
            "diff".to_string(),
            "--cached".to_string(),
            "--check".to_string(),
        ],
        vec!["diff".to_string(), "--check".to_string()],
    ];
    for args in diff_checks {
        let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        let command = format_command("git", &arg_refs, &root);
        match run_status("git", &git_args_owned(&root, &args)) {
            Ok(()) => record_pass(&mut report, &command),
            Err(error) => {
                return fail_with_report(&root, &mut report, error.kind, command, error.message);
            }
        }
    }

    if report.impact_plan.workspace_clippy {
        let args = workspace_clippy_args(&root);
        run_clippy(&root, &mut report, args)?;
    } else if report.impact_plan.impacted_packages.is_empty() {
        let reason = report
            .impact_plan
            .skipped_reason
            .clone()
            .unwrap_or_else(|| "no impacted package".to_string());
        report.skipped.push(reason);
    } else {
        for package in report.impact_plan.impacted_packages.clone() {
            run_clippy(&root, &mut report, package_clippy_args(&root, &package))?;
        }
    }

    report.status = "pass".to_string();
    write_report(&root, &report)
}

fn fail_with_report(
    root: &Path,
    report: &mut PrecommitReport,
    kind: PrecommitFailureKind,
    command: impl Into<String>,
    detail: impl Into<String>,
) -> Result<(), String> {
    let detail = detail.into();
    report.status = kind.status().to_string();
    report.failure_kind = Some(kind);
    report.limitations.push(detail.clone());
    record_failure(report, &command.into(), &detail);
    let report_error = write_report(root, report).err();
    match report_error {
        Some(error) => Err(format!(
            "{detail}; failed to publish current report: {error}"
        )),
        None => Err(detail),
    }
}

fn run_clippy(root: &Path, report: &mut PrecommitReport, args: Vec<String>) -> Result<(), String> {
    let command = format_command(
        "cargo",
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
        root,
    );
    match run_status("cargo", &args) {
        Ok(()) => {
            record_pass(report, &command);
            Ok(())
        }
        Err(error) => fail_with_report(root, report, error.kind, command, error.message),
    }
}

fn repository_root() -> Result<PathBuf, PrecommitError> {
    let args = vec!["rev-parse".to_string(), "--show-toplevel".to_string()];
    let output = capture_process_output("git", &args).map_err(|error| {
        PrecommitError::new(
            match error.kind {
                ProcessErrorKind::Launch => PrecommitFailureKind::Infrastructure,
                ProcessErrorKind::Exit => PrecommitFailureKind::IncompleteEvidence,
            },
            format!("git could not resolve repository root: {}", error.message),
        )
    })?;
    let root = String::from_utf8_lossy(&output).trim().to_string();
    if root.is_empty() {
        return Err(PrecommitError::new(
            PrecommitFailureKind::IncompleteEvidence,
            "git returned an empty repository root",
        ));
    }
    fs::canonicalize(&root).map_err(|error| {
        PrecommitError::new(
            PrecommitFailureKind::Infrastructure,
            format!("canonicalize repository root `{root}`: {error}"),
        )
    })
}

fn resolve_merge_base(root: &Path) -> Result<(String, String), PrecommitError> {
    let mut failures = Vec::new();
    let mut infrastructure_failures = 0;
    for base in ["origin/main", "main"] {
        match git_text(root, &["merge-base", base, "HEAD"], "resolve merge base") {
            Ok(value) if is_sha(value.trim()) => {
                return Ok((base.to_string(), value.trim().to_string()));
            }
            Ok(value) => failures.push(format!("{base}: malformed output `{}`", value.trim())),
            Err(error) => {
                if matches!(error.kind, PrecommitFailureKind::Infrastructure) {
                    infrastructure_failures += 1;
                    failures.push(format!("{base}: {}", error.message));
                } else {
                    failures.push(format!(
                        "{base}: unavailable or incomplete ({})",
                        error.message
                    ));
                }
            }
        }
    }
    Err(PrecommitError::new(
        if infrastructure_failures == 2 {
            PrecommitFailureKind::Infrastructure
        } else {
            PrecommitFailureKind::IncompleteEvidence
        },
        format!(
            "no accepted merge base was available: {}",
            failures.join("; ")
        ),
    ))
}

fn discover_change_set(root: &Path, merge_base: &str) -> Result<ChangeSet, PrecommitError> {
    let committed_range = format!("{merge_base}...HEAD");
    let commands = [
        (
            "committed",
            vec![
                "diff",
                "--name-status",
                "-z",
                "--find-renames",
                committed_range.as_str(),
            ],
        ),
        (
            "staged",
            vec!["diff", "--cached", "--name-status", "-z", "--find-renames"],
        ),
        (
            "unstaged",
            vec!["diff", "--name-status", "-z", "--find-renames"],
        ),
    ];
    let mut changes = Vec::new();
    for (origin, args) in commands {
        let bytes = git_bytes(root, &args, "discover git change set")?;
        changes.extend(parse_name_status_z(&bytes, origin)?);
    }
    let untracked = git_bytes(
        root,
        &["ls-files", "-z", "--others", "--exclude-standard"],
        "discover untracked files",
    )?;
    for path in split_nul(&untracked) {
        if !path.is_empty() {
            changes.push(ChangedPath {
                kind: "untracked".to_string(),
                path: normalize_repo_path(path),
                old_path: None,
                origins: vec!["untracked".to_string()],
            });
        }
    }
    Ok(ChangeSet {
        changes: deduplicate_changes(changes),
        complete: true,
        limitations: Vec::new(),
    })
}

fn parse_name_status_z(bytes: &[u8], origin: &str) -> Result<Vec<ChangedPath>, PrecommitError> {
    let fields = split_nul(bytes);
    let mut changes = Vec::new();
    let mut index = 0;
    while index < fields.len() {
        let status = fields[index];
        index += 1;
        if status.is_empty() {
            continue;
        }
        let status_text = String::from_utf8_lossy(status);
        let code = status_text.chars().next().unwrap_or(' ');
        let kind = match code {
            'A' => "added",
            'M' => "modified",
            'D' => "deleted",
            'R' => "renamed",
            'C' => "copied",
            _ => {
                return Err(PrecommitError::new(
                    PrecommitFailureKind::IncompleteEvidence,
                    format!("malformed Git name-status code `{status_text}`"),
                ));
            }
        };
        if matches!(code, 'R' | 'C') {
            let old = fields.get(index).copied().unwrap_or_default();
            let new = fields.get(index + 1).copied().unwrap_or_default();
            index += 2;
            if old.is_empty() || new.is_empty() {
                return Err(PrecommitError::new(
                    PrecommitFailureKind::IncompleteEvidence,
                    format!("Git {kind} record omitted one side of a path identity"),
                ));
            }
            changes.push(ChangedPath {
                kind: kind.to_string(),
                path: normalize_repo_path(new),
                old_path: Some(normalize_repo_path(old)),
                origins: vec![origin.to_string()],
            });
        } else {
            let path = fields.get(index).copied().unwrap_or_default();
            index += 1;
            if path.is_empty() {
                return Err(PrecommitError::new(
                    PrecommitFailureKind::IncompleteEvidence,
                    format!("Git {kind} record omitted its path"),
                ));
            }
            changes.push(ChangedPath {
                kind: kind.to_string(),
                path: normalize_repo_path(path),
                old_path: None,
                origins: vec![origin.to_string()],
            });
        }
    }
    Ok(changes)
}

fn split_nul(bytes: &[u8]) -> Vec<&[u8]> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .collect()
}

fn deduplicate_changes(changes: Vec<ChangedPath>) -> Vec<ChangedPath> {
    let mut merged: BTreeMap<(String, String, Option<String>), BTreeSet<String>> = BTreeMap::new();
    for change in changes {
        merged
            .entry((change.kind, change.path, change.old_path))
            .or_default()
            .extend(change.origins);
    }
    merged
        .into_iter()
        .map(|((kind, path, old_path), origins)| ChangedPath {
            kind,
            path,
            old_path,
            origins: origins.into_iter().collect(),
        })
        .collect()
}

fn load_package_roots(root: &Path) -> Result<Vec<PackageRoot>, PrecommitError> {
    let args = cargo_metadata_args(root);
    let output = cargo_bytes(&args, "load cargo metadata")?;
    let metadata: CargoMetadata = serde_json::from_slice(&output).map_err(|error| {
        PrecommitError::new(
            PrecommitFailureKind::IncompleteEvidence,
            format!("cargo metadata returned malformed JSON: {error}"),
        )
    })?;
    let mut roots = Vec::new();
    let workspace_members = metadata
        .workspace_members
        .into_iter()
        .collect::<BTreeSet<_>>();
    let package_names = metadata
        .packages
        .iter()
        .map(|package| (package.id.clone(), package.name.clone()))
        .collect::<BTreeMap<_, _>>();
    for package in metadata
        .packages
        .into_iter()
        .filter(|package| workspace_members.contains(&package.id))
    {
        let Some(parent) = package.manifest_path.parent() else {
            return Err(PrecommitError::new(
                PrecommitFailureKind::IncompleteEvidence,
                format!("package `{}` manifest has no parent", package.name),
            ));
        };
        let canonical_parent = fs::canonicalize(parent).map_err(|error| {
            PrecommitError::new(
                PrecommitFailureKind::IncompleteEvidence,
                format!("canonicalize package `{}` path: {error}", package.name),
            )
        })?;
        let relative = canonical_parent.strip_prefix(root).map_err(|error| {
            PrecommitError::new(
                PrecommitFailureKind::IncompleteEvidence,
                format!("package `{}` is outside repository: {error}", package.name),
            )
        })?;
        let dependencies = package
            .dependencies
            .into_iter()
            .map(|dependency| {
                dependency
                    .package
                    .and_then(|id| package_names.get(&id).cloned())
                    .unwrap_or(dependency.name)
            })
            .collect();
        let proc_macro = package
            .targets
            .iter()
            .any(|target| target.kind.iter().any(|kind| kind == "proc-macro"));
        roots.push(PackageRoot {
            name: package.name,
            relative_root: normalize_path(relative),
            dependencies,
            proc_macro,
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

fn cargo_metadata_args(root: &Path) -> Vec<String> {
    vec![
        "metadata".to_string(),
        "--format-version".to_string(),
        "1".to_string(),
        "--manifest-path".to_string(),
        root.join("Cargo.toml").display().to_string(),
    ]
}

fn build_impact_plan(change_set: &ChangeSet, roots: &[PackageRoot]) -> ImpactPlan {
    let paths = change_set.paths();
    if let Some(reason) = workspace_widening_reason(&paths) {
        return ImpactPlan {
            impacted_packages: Vec::new(),
            workspace_clippy: true,
            workspace_widening_reason: Some(reason),
            skipped_reason: None,
        };
    }
    let mut packages = BTreeSet::new();
    let mut widen_dependents = BTreeSet::new();
    for path in &paths {
        if !is_rust_relevant(path) {
            continue;
        }
        if let Some(owner) = nearest_package(path, roots) {
            packages.insert(owner.name.clone());
            if path.ends_with(".rs")
                || path.ends_with("Cargo.toml")
                || path.ends_with("build.rs")
                || owner.proc_macro
            {
                widen_dependents.insert(owner.name.clone());
            }
        }
    }
    for package in widen_dependents {
        packages.extend(reverse_dependents(&package, roots));
    }
    let skipped_reason = if packages.is_empty() {
        Some("Clippy skipped: no Rust package or workspace-wide Rust surface changed.".to_string())
    } else {
        None
    };
    ImpactPlan {
        impacted_packages: packages.into_iter().collect(),
        workspace_clippy: false,
        workspace_widening_reason: None,
        skipped_reason,
    }
}

fn nearest_package<'a>(path: &str, roots: &'a [PackageRoot]) -> Option<&'a PackageRoot> {
    roots
        .iter()
        .find(|root| path_is_under(path, &root.relative_root))
}

fn reverse_dependents(package: &str, roots: &[PackageRoot]) -> BTreeSet<String> {
    let mut affected = BTreeSet::from([package.to_string()]);
    loop {
        let before = affected.len();
        for root in roots {
            if root
                .dependencies
                .iter()
                .any(|dependency| affected.contains(dependency))
            {
                affected.insert(root.name.clone());
            }
        }
        if affected.len() == before {
            return affected;
        }
    }
}

fn workspace_widening_reason(paths: &[String]) -> Option<String> {
    paths.iter().find_map(|path| {
        if matches!(
            path.as_str(),
            "Cargo.toml"
                | "Cargo.lock"
                | "rust-toolchain"
                | "rust-toolchain.toml"
                | "clippy.toml"
                | "rustfmt.toml"
        ) {
            Some(format!(
                "workspace-wide Rust control surface changed: {path}"
            ))
        } else if path.starts_with(".cargo/")
            || path.starts_with("policy/clippy")
            || path.starts_with("policy/lint")
        {
            Some(format!("workspace-wide Rust policy changed: {path}"))
        } else if matches!(
            path.as_str(),
            "xtask/src/dispatch.rs" | "xtask/src/precommit_v2.rs"
        ) {
            Some(format!("precommit implementation surface changed: {path}"))
        } else {
            None
        }
    })
}

fn path_is_under(path: &str, root: &str) -> bool {
    root.is_empty() || path == format!("{root}/Cargo.toml") || path.starts_with(&format!("{root}/"))
}

fn is_rust_relevant(path: &str) -> bool {
    path.ends_with(".rs") || path.ends_with("Cargo.toml") || path.ends_with("build.rs")
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

fn git_args(root: &Path, args: &[&str]) -> Vec<String> {
    let mut owned = vec!["-C".to_string(), root.display().to_string()];
    owned.extend(args.iter().map(|arg| (*arg).to_string()));
    owned
}

fn git_args_owned(root: &Path, args: &[String]) -> Vec<String> {
    let mut owned = vec!["-C".to_string(), root.display().to_string()];
    owned.extend(args.iter().cloned());
    owned
}

fn git_text(root: &Path, args: &[&str], context: &str) -> Result<String, PrecommitError> {
    let bytes = git_bytes(root, args, context)?;
    String::from_utf8(bytes).map_err(|error| {
        PrecommitError::new(
            PrecommitFailureKind::IncompleteEvidence,
            format!("{context} returned non-UTF-8 output: {error}"),
        )
    })
}

fn git_bytes(root: &Path, args: &[&str], context: &str) -> Result<Vec<u8>, PrecommitError> {
    let owned = git_args(root, args);
    capture_process_output("git", &owned).map_err(|error| {
        PrecommitError::new(
            match error.kind {
                ProcessErrorKind::Launch => PrecommitFailureKind::Infrastructure,
                ProcessErrorKind::Exit => PrecommitFailureKind::IncompleteEvidence,
            },
            format!("{context}: {}", error.message),
        )
    })
}

fn cargo_bytes(args: &[String], context: &str) -> Result<Vec<u8>, PrecommitError> {
    let output =
        capture_output_with_timeout("cargo", args, &[], PRECOMMIT_COMMAND_TIMEOUT, context)
            .map_err(|message| {
                PrecommitError::new(PrecommitFailureKind::Infrastructure, message)
            })?;
    if output.timed_out {
        return Err(PrecommitError::new(
            PrecommitFailureKind::Infrastructure,
            format!(
                "{context} exceeded the {} second precommit timeout",
                PRECOMMIT_COMMAND_TIMEOUT.as_secs()
            ),
        ));
    }
    let Some(status) = output.status else {
        return Err(PrecommitError::new(
            PrecommitFailureKind::Infrastructure,
            format!("{context} produced no process status"),
        ));
    };
    if status.success() {
        return Ok(output.stdout.into_bytes());
    }
    let detail = [output.stdout.trim(), output.stderr.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    Err(PrecommitError::new(
        PrecommitFailureKind::IncompleteEvidence,
        format!("{context} failed with {status}: {detail}"),
    ))
}

fn run_status(program: &str, args: &[String]) -> Result<(), PrecommitError> {
    let output = capture_output_with_timeout(
        program,
        args,
        &[],
        PRECOMMIT_COMMAND_TIMEOUT,
        "precommit command",
    )
    .map_err(|message| PrecommitError::new(PrecommitFailureKind::Infrastructure, message))?;
    if output.timed_out {
        return Err(PrecommitError::new(
            PrecommitFailureKind::Infrastructure,
            format!(
                "{program} {} exceeded the {} second precommit timeout",
                args.join(" "),
                PRECOMMIT_COMMAND_TIMEOUT.as_secs()
            ),
        ));
    }
    let Some(status) = output.status else {
        return Err(PrecommitError::new(
            PrecommitFailureKind::Infrastructure,
            format!("{program} {} produced no process status", args.join(" ")),
        ));
    };
    if status.success() {
        return Ok(());
    }
    let detail = [output.stdout.trim(), output.stderr.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    Err(PrecommitError::new(
        PrecommitFailureKind::SourceOrPolicy,
        format!(
            "{program} {} failed with {status}{}",
            args.join(" "),
            if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            }
        ),
    ))
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

fn write_report(root: &Path, report: &PrecommitReport) -> Result<(), String> {
    let mut canonical = report.clone();
    canonical.report_digest.clear();
    let bytes = serde_json::to_vec(&canonical)
        .map_err(|error| format!("serialize report digest input: {error}"))?;
    let digest = Sha256::digest(bytes);
    canonical.report_digest = format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let json = serde_json::to_string_pretty(&canonical)
        .map_err(|error| format!("serialize precommit JSON: {error}"))?
        + "\n";
    let markdown = report_markdown(&canonical);
    atomic_write(&root.join(REPORT_JSON), &json)?;
    atomic_write(&root.join(REPORT_MARKDOWN), &markdown)
}

fn atomic_write(path: &Path, contents: &str) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!("report path has no parent: {}", path.display()));
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("create report directory {}: {error}", parent.display()))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let temp = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("report"),
        nonce
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| format!("create temporary report {}: {error}", temp.display()))?;
        file.write_all(contents.as_bytes())
            .map_err(|error| format!("write temporary report {}: {error}", temp.display()))?;
        file.sync_all()
            .map_err(|error| format!("flush temporary report {}: {error}", temp.display()))?;
        #[cfg(windows)]
        if path.exists() {
            fs::remove_file(path)
                .map_err(|error| format!("replace existing report {}: {error}", path.display()))?;
        }
        fs::rename(&temp, path)
            .map_err(|error| format!("publish report {}: {error}", path.display()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn report_markdown(report: &PrecommitReport) -> String {
    let mut out = format!(
        "# ripr precommit v2\n\n- Status: `{}`\n- Failure kind: `{}`\n- Head: `{}`\n- Merge base: `{}`\n- Changed records: `{}`\n- Report digest: `{}`\n\n",
        report.status,
        report
            .failure_kind
            .map(|kind| kind.status())
            .unwrap_or("none"),
        report.head_sha.as_deref().unwrap_or("unknown"),
        report.merge_base_sha.as_deref().unwrap_or("unknown"),
        report.change_set.changes.len(),
        report.report_digest
    );
    out.push_str("## Changed files\n\n");
    if report.change_set.changes.is_empty() {
        out.push_str("- none\n");
    } else {
        for change in &report.change_set.changes {
            let old = change.old_path.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "- `{}` `{}` -> `{}` ({})\n",
                change.kind,
                old,
                change.path,
                change.origins.join(", ")
            ));
        }
    }
    out.push_str("\n## Impact plan\n\n");
    if report.impact_plan.workspace_clippy {
        out.push_str("- workspace Clippy: `true`\n");
        if let Some(reason) = &report.impact_plan.workspace_widening_reason {
            out.push_str(&format!("- reason: {reason}\n"));
        }
    } else if report.impact_plan.impacted_packages.is_empty() {
        out.push_str("- packages: none\n");
    } else {
        for package in &report.impact_plan.impacted_packages {
            out.push_str(&format!("- package: `{package}`\n"));
        }
    }
    out.push_str("\n## Commands\n\n");
    for command in &report.commands {
        out.push_str(&format!("- `{}`: `{}`\n", command.command, command.outcome));
    }
    out.push_str("\n## Limitations\n\n");
    if report.limitations.is_empty() {
        out.push_str("- none\n");
    } else {
        for limitation in &report.limitations {
            out.push_str(&format!("- {limitation}\n"));
        }
    }
    out
}

fn format_command(program: &str, args: &[&str], root: &Path) -> String {
    let root_text = root.to_string_lossy().replace('\\', "/");
    let rendered = args
        .iter()
        .map(|arg| arg.replace('\\', "/").replace(&root_text, "$REPO"))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{program} {rendered}")
}

fn normalize_repo_path(value: &[u8]) -> String {
    String::from_utf8_lossy(value).replace('\\', "/")
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn is_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
        condition.then_some(()).ok_or_else(|| message.into())
    }

    fn sample_report(status: &str) -> PrecommitReport {
        PrecommitReport {
            schema_version: "0.2",
            status: status.to_string(),
            failure_kind: None,
            repository_root: "$REPO".to_string(),
            head_sha: Some("0123456789012345678901234567890123456789".to_string()),
            merge_base_ref: Some("origin/main".to_string()),
            merge_base_sha: Some("0123456789012345678901234567890123456789".to_string()),
            change_set: ChangeSet::empty(),
            impact_plan: ImpactPlan::empty(),
            commands: Vec::new(),
            skipped: Vec::new(),
            limitations: Vec::new(),
            report_digest: String::new(),
        }
    }

    #[test]
    fn nul_name_status_keeps_both_rename_paths() -> Result<(), String> {
        let bytes = b"R100\0crates/a/src/lib.rs\0crates/b/src/lib.rs\0";
        let changes = parse_name_status_z(bytes, "committed").map_err(|error| error.message)?;
        require(changes.len() == 1, "rename record count changed")?;
        require(changes[0].kind == "renamed", "rename kind missing")?;
        require(
            changes[0].old_path.as_deref() == Some("crates/a/src/lib.rs"),
            "old rename path missing",
        )?;
        require(
            changes[0].path == "crates/b/src/lib.rs",
            "new rename path missing",
        )
    }

    #[test]
    fn malformed_nul_name_status_is_incomplete_evidence() -> Result<(), String> {
        let error = parse_name_status_z(b"R100\0only-one-side\0", "staged")
            .err()
            .ok_or_else(|| "malformed rename was accepted".to_string())?;
        require(
            matches!(error.kind, PrecommitFailureKind::IncompleteEvidence),
            "wrong failure kind",
        )
    }

    #[test]
    fn paths_with_spaces_and_non_ascii_are_preserved() -> Result<(), String> {
        let bytes = "M\0crates/a/src/ name-é.rs\0".as_bytes();
        let changes = parse_name_status_z(bytes, "unstaged").map_err(|error| error.message)?;
        require(
            changes[0].path == "crates/a/src/ name-é.rs",
            "path normalization changed a valid path",
        )
    }

    #[test]
    fn impact_plan_widens_reverse_dependents_for_build_and_proc_macro() -> Result<(), String> {
        let roots = vec![
            PackageRoot {
                name: "app".to_string(),
                relative_root: "crates/app".to_string(),
                dependencies: vec!["macro".to_string()],
                proc_macro: false,
            },
            PackageRoot {
                name: "macro".to_string(),
                relative_root: "crates/macro".to_string(),
                dependencies: Vec::new(),
                proc_macro: true,
            },
        ];
        let changes = ChangeSet {
            changes: vec![ChangedPath {
                kind: "modified".to_string(),
                path: "crates/macro/src/lib.rs".to_string(),
                old_path: None,
                origins: vec!["unstaged".to_string()],
            }],
            complete: true,
            limitations: Vec::new(),
        };
        let plan = build_impact_plan(&changes, &roots);
        require(
            plan.impacted_packages == vec!["app".to_string(), "macro".to_string()],
            "reverse dependent closure missing",
        )
    }

    #[test]
    fn ordinary_library_source_widens_reverse_dependents() -> Result<(), String> {
        let roots = vec![
            PackageRoot {
                name: "app".to_string(),
                relative_root: "crates/app".to_string(),
                dependencies: vec!["lib".to_string()],
                proc_macro: false,
            },
            PackageRoot {
                name: "lib".to_string(),
                relative_root: "crates/lib".to_string(),
                dependencies: Vec::new(),
                proc_macro: false,
            },
        ];
        let changes = ChangeSet {
            changes: vec![ChangedPath {
                kind: "modified".to_string(),
                path: "crates/lib/src/lib.rs".to_string(),
                old_path: None,
                origins: vec!["unstaged".to_string()],
            }],
            complete: true,
            limitations: Vec::new(),
        };
        let plan = build_impact_plan(&changes, &roots);
        require(
            plan.impacted_packages == vec!["app".to_string(), "lib".to_string()],
            "ordinary library source did not include reverse dependents",
        )
    }

    #[test]
    fn cross_package_rename_impacts_both_package_owners() -> Result<(), String> {
        let roots = vec![
            PackageRoot {
                name: "a".to_string(),
                relative_root: "crates/a".to_string(),
                dependencies: Vec::new(),
                proc_macro: false,
            },
            PackageRoot {
                name: "b".to_string(),
                relative_root: "crates/b".to_string(),
                dependencies: Vec::new(),
                proc_macro: false,
            },
        ];
        let changes = ChangeSet {
            changes: vec![ChangedPath {
                kind: "renamed".to_string(),
                path: "crates/b/src/lib.rs".to_string(),
                old_path: Some("crates/a/src/lib.rs".to_string()),
                origins: vec!["staged".to_string()],
            }],
            complete: true,
            limitations: Vec::new(),
        };
        let plan = build_impact_plan(&changes, &roots);
        require(
            plan.impacted_packages == vec!["a".to_string(), "b".to_string()],
            "cross-package rename did not retain both owners",
        )
    }

    #[test]
    fn docs_skip_and_workspace_controls_widen_clippy() -> Result<(), String> {
        let docs = ChangeSet {
            changes: vec![ChangedPath {
                kind: "modified".to_string(),
                path: "docs/guide.md".to_string(),
                old_path: None,
                origins: vec!["unstaged".to_string()],
            }],
            complete: true,
            limitations: Vec::new(),
        };
        let docs_plan = build_impact_plan(&docs, &[]);
        require(
            docs_plan.impacted_packages.is_empty()
                && docs_plan.skipped_reason.is_some()
                && !docs_plan.workspace_clippy,
            "docs-only change did not skip Clippy explicitly",
        )?;
        let workspace = ChangeSet {
            changes: vec![ChangedPath {
                kind: "modified".to_string(),
                path: "Cargo.lock".to_string(),
                old_path: None,
                origins: vec!["committed".to_string()],
            }],
            complete: true,
            limitations: Vec::new(),
        };
        let workspace_plan = build_impact_plan(&workspace, &[]);
        require(
            workspace_plan.workspace_clippy && workspace_plan.workspace_widening_reason.is_some(),
            "workspace control change did not widen Clippy",
        )
    }

    #[test]
    fn merge_base_falls_back_to_local_main_and_fails_closed_without_one() -> Result<(), String> {
        let fixture = TestRepo::new()?;
        fixture.write("tracked.txt", "base\n")?;
        fixture.git(&["add", "tracked.txt"])?;
        fixture.git(&["commit", "-m", "base"])?;
        let (base_ref, base_sha) =
            resolve_merge_base(&fixture.path).map_err(|error| error.message)?;
        require(base_ref == "main", "local main fallback was not selected")?;
        require(is_sha(&base_sha), "local main fallback was not a SHA")?;

        let empty = TestRepo::new()?;
        let error = resolve_merge_base(&empty.path)
            .err()
            .ok_or_else(|| "merge-base unexpectedly succeeded without a base".to_string())?;
        require(
            matches!(error.kind, PrecommitFailureKind::IncompleteEvidence),
            "missing merge base was not incomplete evidence",
        )
    }

    #[test]
    fn workspace_metadata_maps_dependency_ids_to_reverse_dependents() -> Result<(), String> {
        let fixture = TestRepo::new()?;
        fixture.write(
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\nresolver = \"2\"\n",
        )?;
        fixture.write(
            "crates/a/Cargo.toml",
            "[package]\nname = \"a\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )?;
        fixture.write("crates/a/src/lib.rs", "pub fn a() {}\n")?;
        fixture.write(
            "crates/b/Cargo.toml",
            "[package]\nname = \"b\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\na = { path = \"../a\" }\n",
        )?;
        fixture.write("crates/b/src/lib.rs", "pub fn b() { a::a(); }\n")?;
        fixture.git(&["add", "."])?;
        fixture.git(&["commit", "-m", "workspace"])?;

        let root = fs::canonicalize(&fixture.path).map_err(|error| error.to_string())?;
        let roots = load_package_roots(&root).map_err(|error| error.message)?;
        let changes = ChangeSet {
            changes: vec![ChangedPath {
                kind: "modified".to_string(),
                path: "crates/a/Cargo.toml".to_string(),
                old_path: None,
                origins: vec!["unstaged".to_string()],
            }],
            complete: true,
            limitations: Vec::new(),
        };
        let plan = build_impact_plan(&changes, &roots);
        require(
            plan.impacted_packages == vec!["a".to_string(), "b".to_string()],
            "metadata dependency IDs did not widen to reverse dependents",
        )
    }

    #[test]
    fn process_runner_distinguishes_launch_and_exit_failures() -> Result<(), String> {
        let missing = run_process_status("ripr-precommit-command-that-does-not-exist", &[])
            .err()
            .ok_or_else(|| "missing process unexpectedly launched".to_string())?;
        require(
            matches!(missing.kind, ProcessErrorKind::Launch),
            "missing process was not a launch failure",
        )?;

        #[cfg(windows)]
        let (program, args) = (
            "cmd",
            vec!["/C".to_string(), "exit".to_string(), "7".to_string()],
        );
        #[cfg(not(windows))]
        let (program, args) = ("sh", vec!["-c".to_string(), "exit 7".to_string()]);
        let exited = run_process_status(program, &args)
            .err()
            .ok_or_else(|| "failing process unexpectedly succeeded".to_string())?;
        require(
            matches!(exited.kind, ProcessErrorKind::Exit),
            "non-zero process was not an exit failure",
        )
    }

    #[test]
    fn failed_report_replaces_prior_pass_and_binds_markdown_digest() -> Result<(), String> {
        let fixture = TestRepo::new()?;
        let mut pass = sample_report("pass");
        write_report(&fixture.path, &pass)?;
        let first: serde_json::Value = serde_json::from_slice(
            &fs::read(fixture.path.join(REPORT_JSON)).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        require(
            first["status"] == "pass",
            "initial report was not published",
        )?;

        pass.status = "incomplete_evidence".to_string();
        pass.failure_kind = Some(PrecommitFailureKind::IncompleteEvidence);
        pass.limitations.push("missing accepted base".to_string());
        write_report(&fixture.path, &pass)?;
        let json = fs::read_to_string(fixture.path.join(REPORT_JSON))
            .map_err(|error| error.to_string())?;
        let second: serde_json::Value =
            serde_json::from_str(&json).map_err(|error| error.to_string())?;
        require(
            second["status"] == "incomplete_evidence",
            "failed run did not replace the prior pass",
        )?;
        let digest = second["report_digest"]
            .as_str()
            .ok_or_else(|| "report digest missing".to_string())?;
        let markdown = fs::read_to_string(fixture.path.join(REPORT_MARKDOWN))
            .map_err(|error| error.to_string())?;
        require(
            markdown.contains(digest),
            "Markdown was not derived from the canonical report",
        )?;
        let temporary_count = fs::read_dir(fixture.path.join("target/ripr/reports"))
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
            .count();
        require(
            temporary_count == 0,
            "report publication left a temporary file",
        )
    }

    #[test]
    fn real_git_fixture_discovers_all_git_change_layers_and_ignored() -> Result<(), String> {
        let fixture = TestRepo::new()?;
        fixture.write("crates/a/src/lib.rs", "pub fn a() {}\n")?;
        fixture.write("crates/b/src/lib.rs", "pub fn b() {}\n")?;
        fixture.write("crates/a/src/rename.rs", "pub fn rename() {}\n")?;
        fixture.write("deleted.txt", "delete me\n")?;
        fixture.write(".gitignore", "ignored.txt\n")?;
        fixture.git(&["add", "."])?;
        fixture.git(&["commit", "-m", "base"])?;
        let base = fixture.git_text(&["rev-parse", "HEAD"])?;
        fixture.write("committed.txt", "committed\n")?;
        fixture.git(&["add", "committed.txt"])?;
        fixture.git(&["commit", "-m", "committed branch change"])?;
        fixture.write("crates/a/src/lib.rs", "pub fn a_staged() {}\n")?;
        fixture.git(&["add", "crates/a/src/lib.rs"])?;
        fixture.write("crates/a/src/lib.rs", "pub fn a_staged_and_unstaged() {}\n")?;
        fixture.write("crates/b/src/lib.rs", "pub fn b_unstaged() {}\n")?;
        fixture.git(&["mv", "crates/a/src/rename.rs", "crates/b/src/renamed.rs"])?;
        fixture.git(&["rm", "deleted.txt"])?;
        fixture.write("untracked.txt", "untracked\n")?;
        fixture.write("ignored.txt", "ignored\n")?;
        let changes = discover_change_set(&fixture.path, &base).map_err(|error| error.message)?;
        require(changes.complete, "fixture was incomplete")?;
        let paths = changes.paths();
        require(
            paths.iter().any(|path| path == "crates/a/src/lib.rs"),
            "rename old path absent",
        )?;
        require(
            paths.iter().any(|path| path == "crates/b/src/renamed.rs"),
            "rename new path absent",
        )?;
        require(
            paths.iter().any(|path| path == "untracked.txt"),
            "untracked path absent",
        )?;
        require(
            paths.iter().any(|path| path == "committed.txt"),
            "committed branch-only path absent",
        )?;
        require(
            paths.iter().any(|path| path == "deleted.txt"),
            "deleted path absent",
        )?;
        require(
            !paths.iter().any(|path| path == "ignored.txt"),
            "ignored path included",
        )?;
        require(
            paths.iter().any(|path| path == "crates/b/src/lib.rs"),
            "unstaged path absent",
        )?;
        let rename = changes
            .changes
            .iter()
            .find(|change| change.kind == "renamed")
            .ok_or_else(|| "rename record absent".to_string())?;
        require(
            rename.old_path.as_deref() == Some("crates/a/src/rename.rs")
                && rename.path == "crates/b/src/renamed.rs",
            "cross-package rename identity was not retained",
        )?;
        let staged_and_unstaged = changes
            .changes
            .iter()
            .find(|change| change.path == "crates/a/src/lib.rs")
            .ok_or_else(|| "layered path record absent".to_string())?;
        require(
            staged_and_unstaged.origins == vec!["staged".to_string(), "unstaged".to_string()],
            "duplicate staged and unstaged path was not merged deterministically",
        )
    }

    struct TestRepo {
        path: PathBuf,
    }

    impl TestRepo {
        fn new() -> Result<Self, String> {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "ripr-precommit-fixture-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&path).map_err(|error| format!("create fixture: {error}"))?;
            let fixture = Self { path };
            fixture.git(&["init", "-q", "--initial-branch=main"])?;
            fixture.git(&["config", "user.email", "fixture@example.invalid"])?;
            fixture.git(&["config", "user.name", "fixture"])?;
            Ok(fixture)
        }

        fn write(&self, relative: &str, contents: &str) -> Result<(), String> {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("create fixture parent: {error}"))?;
            }
            fs::write(path, contents).map_err(|error| format!("write fixture: {error}"))
        }

        fn git(&self, args: &[&str]) -> Result<(), String> {
            let mut owned = vec!["-C".to_string(), self.path.display().to_string()];
            owned.extend(args.iter().map(|arg| (*arg).to_string()));
            run_process_status("git", &owned)
                .map_err(|error| format!("git fixture failed: {}", error.message))
        }

        fn git_text(&self, args: &[&str]) -> Result<String, String> {
            let mut owned = vec!["-C".to_string(), self.path.display().to_string()];
            owned.extend(args.iter().map(|arg| (*arg).to_string()));
            let bytes = capture_process_output("git", &owned)
                .map_err(|error| format!("git fixture failed: {}", error.message))?;
            Ok(String::from_utf8_lossy(&bytes).trim().to_string())
        }
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            match fs::remove_dir_all(&self.path) {
                Ok(()) | Err(_) => {}
            }
        }
    }
}
