use crate::config::{CONFIG_FILE_NAME, detect_python_project};
use serde_json::{Value, json};
use std::path::Path;

use super::options::FirstPrOptions;
use super::{
    command_problem, detect_typescript_project, git_args, missing_base_command, resolve_path,
    run_git,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FirstPrPreflight {
    status: &'static str,
    mode: &'static str,
    root: String,
    resolved_root: String,
    base: String,
    head: String,
    next_command: Option<String>,
    checks: Vec<PreflightCheck>,
}

impl FirstPrPreflight {
    pub(super) fn warnings(&self) -> impl Iterator<Item = String> + '_ {
        self.checks
            .iter()
            .filter(|check| {
                check.status != "ok" && check.status != "defaulted" && check.status != "will_create"
            })
            .map(|check| check.message.clone())
    }

    pub(super) fn to_json(&self) -> Value {
        json!({
            "status": self.status,
            "mode": self.mode,
            "root": self.root,
            "resolved_root": self.resolved_root,
            "base": self.base,
            "head": self.head,
            "next_command": self.next_command,
            "checks": self.checks.iter().map(PreflightCheck::to_json).collect::<Vec<_>>()
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreflightCheck {
    id: &'static str,
    label: &'static str,
    status: &'static str,
    message: String,
    path: Option<String>,
    next_command: Option<String>,
}

impl PreflightCheck {
    fn ok(id: &'static str, label: &'static str, message: impl Into<String>) -> Self {
        Self {
            id,
            label,
            status: "ok",
            message: message.into(),
            path: None,
            next_command: None,
        }
    }

    fn defaulted(id: &'static str, label: &'static str, message: impl Into<String>) -> Self {
        Self {
            id,
            label,
            status: "defaulted",
            message: message.into(),
            path: None,
            next_command: None,
        }
    }

    fn needs_attention(
        id: &'static str,
        label: &'static str,
        message: impl Into<String>,
        next_command: Option<String>,
    ) -> Self {
        Self {
            id,
            label,
            status: "needs_attention",
            message: message.into(),
            path: None,
            next_command,
        }
    }

    fn no_action(
        id: &'static str,
        label: &'static str,
        message: impl Into<String>,
        next_command: Option<String>,
    ) -> Self {
        Self {
            id,
            label,
            status: "no_action",
            message: message.into(),
            path: None,
            next_command,
        }
    }

    fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "label": self.label,
            "status": self.status,
            "message": self.message,
            "path": self.path,
            "next_command": self.next_command
        })
    }
}

pub(super) fn first_pr_preflight(root: &Path, options: &FirstPrOptions) -> FirstPrPreflight {
    let mut checks = Vec::new();
    let resolved_root = root.display().to_string();
    checks.push(preflight_root_check(root, options));
    let git_available = matches!(checks.last().map(|check| check.status), Some("ok"))
        && preflight_git_repo_check(root, &mut checks);
    let mut base_ok = false;
    let mut head_ok = false;
    if git_available {
        base_ok = preflight_git_ref_check(
            root,
            &mut checks,
            "git_base",
            "Git base",
            &options.base,
            Some(missing_base_command(options)),
        );
        head_ok = preflight_git_ref_check(
            root,
            &mut checks,
            "git_head",
            "Git head",
            &options.head,
            Some(format!(
                "Check --head `{}` or fetch the branch, then rerun `ripr first-pr --root {} --base {} --head {}`.",
                options.head, options.root, options.base, options.head
            )),
        );
    }
    if git_available && base_ok && head_ok {
        preflight_diff_check(root, options, &mut checks);
    }
    checks.push(preflight_project_check(root));
    checks.push(preflight_config_check(root));
    checks.push(preflight_output_check(root, options));
    checks.push(PreflightCheck::ok(
        "mode",
        "Mode",
        if options.check {
            "Check mode validates the existing start-here packet without rewriting it."
        } else {
            "Write mode composes start-here.json and start-here.md from explicit artifacts."
        },
    ));
    let next_command = checks.iter().find_map(|check| check.next_command.clone());
    let status = if checks
        .iter()
        .any(|check| check.status == "needs_attention" || check.status == "no_action")
    {
        "needs_attention"
    } else {
        "ready"
    };
    FirstPrPreflight {
        status,
        mode: if options.check { "check" } else { "write" },
        root: options.root.clone(),
        resolved_root,
        base: options.base.clone(),
        head: options.head.clone(),
        next_command,
        checks,
    }
}

fn preflight_root_check(root: &Path, options: &FirstPrOptions) -> PreflightCheck {
    if root.is_dir() {
        PreflightCheck::ok(
            "root",
            "Workspace root",
            format!("Workspace root `{}` exists.", options.root),
        )
        .with_path(root.display().to_string())
    } else {
        PreflightCheck::needs_attention(
            "root",
            "Workspace root",
            format!(
                "Workspace root `{}` does not exist or is not a directory.",
                options.root
            ),
            Some("Run from a repository root or pass --root <path>.".to_string()),
        )
        .with_path(root.display().to_string())
    }
}

fn preflight_git_repo_check(root: &Path, checks: &mut Vec<PreflightCheck>) -> bool {
    match run_git(root, &git_args(&["rev-parse", "--is-inside-work-tree"])) {
        Ok(output) if output.success() && output.stdout.trim() == "true" => {
            checks.push(PreflightCheck::ok(
                "git_repo",
                "Git repository",
                "The root is inside a Git worktree.",
            ));
            true
        }
        Ok(output) => {
            checks.push(PreflightCheck::needs_attention(
                "git_repo",
                "Git repository",
                command_problem(
                    "The root is not a Git worktree.",
                    &output,
                    "Run from a Git worktree or pass --root <repo>.",
                ),
                Some("Run from a Git worktree or pass --root <repo>.".to_string()),
            ));
            false
        }
        Err(message) => {
            checks.push(PreflightCheck::needs_attention(
                "git_repo",
                "Git repository",
                format!("Could not run git preflight: {message}."),
                Some(
                    "Install git or run first-pr from an environment where git is available."
                        .to_string(),
                ),
            ));
            false
        }
    }
}

fn preflight_git_ref_check(
    root: &Path,
    checks: &mut Vec<PreflightCheck>,
    id: &'static str,
    label: &'static str,
    rev: &str,
    next_command: Option<String>,
) -> bool {
    let commit = format!("{rev}^{{commit}}");
    match run_git(
        root,
        &[
            "rev-parse".to_string(),
            "--verify".to_string(),
            "--quiet".to_string(),
            commit,
        ],
    ) {
        Ok(output) if output.success() => {
            checks.push(PreflightCheck::ok(
                id,
                label,
                format!("Resolved `{rev}` to a commit."),
            ));
            true
        }
        Ok(output) => {
            checks.push(PreflightCheck::needs_attention(
                id,
                label,
                command_problem(
                    &format!("Could not resolve `{rev}` to a commit."),
                    &output,
                    "Fetch the missing ref or pass a resolvable --base/--head.",
                ),
                next_command,
            ));
            false
        }
        Err(message) => {
            checks.push(PreflightCheck::needs_attention(
                id,
                label,
                format!("Could not run git ref preflight for `{rev}`: {message}."),
                next_command,
            ));
            false
        }
    }
}

fn preflight_diff_check(root: &Path, options: &FirstPrOptions, checks: &mut Vec<PreflightCheck>) {
    let range = format!("{}..{}", options.base, options.head);
    match run_git(
        root,
        &[
            "diff".to_string(),
            "--quiet".to_string(),
            range.clone(),
            "--".to_string(),
        ],
    ) {
        Ok(output) if matches!(output.code, Some(0)) => {
            checks.push(PreflightCheck::no_action(
                "git_diff",
                "Git diff",
                format!("No file diff was found for `{range}`."),
                Some(format!(
                    "Choose a head with changes or rerun after committing PR work: `ripr first-pr --root {} --base {} --head {}`.",
                    options.root, options.base, options.head
                )),
            ));
        }
        Ok(output) if matches!(output.code, Some(1)) => {
            checks.push(PreflightCheck::ok(
                "git_diff",
                "Git diff",
                format!("Found a file diff for `{range}`."),
            ));
        }
        Ok(output) => {
            checks.push(PreflightCheck::needs_attention(
                "git_diff",
                "Git diff",
                command_problem(
                    &format!("Could not inspect diff range `{range}`."),
                    &output,
                    "Check --base and --head, then rerun first-pr.",
                ),
                Some(format!(
                    "Check --base and --head, then rerun `ripr first-pr --root {} --base {} --head {}`.",
                    options.root, options.base, options.head
                )),
            ));
        }
        Err(message) => {
            checks.push(PreflightCheck::needs_attention(
                "git_diff",
                "Git diff",
                format!("Could not run git diff preflight: {message}."),
                Some(
                    "Install git or rerun from an environment where git is available.".to_string(),
                ),
            ));
        }
    }
}

fn preflight_project_check(root: &Path) -> PreflightCheck {
    let manifest = root.join("Cargo.toml");
    if manifest.is_file() {
        PreflightCheck::ok(
            "cargo_workspace",
            "Cargo workspace",
            "Cargo.toml was found at the workspace root.",
        )
        .with_path(manifest.display().to_string())
    } else if detect_python_project(root) {
        PreflightCheck::ok(
            "python_project",
            "Python project",
            "Python project markers were found; first-pr can consume Python preview gap-ledger records.",
        )
        .with_path(root.display().to_string())
    } else if detect_typescript_project(root) {
        PreflightCheck::ok(
            "typescript_project",
            "TypeScript project",
            "TypeScript project markers were found; first-pr can consume TypeScript preview gap-ledger records.",
        )
        .with_path(root.display().to_string())
    } else {
        PreflightCheck::needs_attention(
            "cargo_workspace",
            "Cargo workspace",
            "No Cargo.toml was found at the workspace root.",
            Some(
                "Run from a Rust/Cargo workspace, a Python or TypeScript project root, or pass --root <repo>."
                    .to_string(),
            ),
        )
        .with_path(manifest.display().to_string())
    }
}

fn preflight_config_check(root: &Path) -> PreflightCheck {
    let config = root.join(CONFIG_FILE_NAME);
    if config.is_file() {
        PreflightCheck::ok(
            "ripr_config",
            "RIPR config",
            format!("{CONFIG_FILE_NAME} was found."),
        )
        .with_path(config.display().to_string())
    } else {
        PreflightCheck::defaulted(
            "ripr_config",
            "RIPR config",
            format!("No {CONFIG_FILE_NAME} was found; built-in advisory defaults apply."),
        )
        .with_path(config.display().to_string())
    }
}

fn preflight_output_check(root: &Path, options: &FirstPrOptions) -> PreflightCheck {
    let out_dir = resolve_path(root, &options.out_dir);
    if out_dir.exists() && !out_dir.is_dir() {
        return PreflightCheck::needs_attention(
            "output_dir",
            "Output directory",
            format!(
                "Output path `{}` exists but is not a directory.",
                options.out_dir
            ),
            Some("Choose a directory for --out-dir, then rerun first-pr.".to_string()),
        )
        .with_path(out_dir.display().to_string());
    }
    if out_dir.is_dir() {
        PreflightCheck::ok(
            "output_dir",
            "Output directory",
            format!("Output directory `{}` exists.", options.out_dir),
        )
        .with_path(out_dir.display().to_string())
    } else {
        PreflightCheck {
            id: "output_dir",
            label: "Output directory",
            status: "will_create",
            message: format!(
                "Output directory `{}` will be created if needed.",
                options.out_dir
            ),
            path: Some(out_dir.display().to_string()),
            next_command: None,
        }
    }
}
