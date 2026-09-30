use crate::cli::commands_options::{InitCi, InitOptions};
use crate::cli::help;
use crate::cli::parse::expect_value;
use crate::cli::suggest::unknown_argument;
use crate::config::{CONFIG_FILE_NAME, generated_init_config};
use crate::output;
use std::path::{Path, PathBuf};

use super::init_workflow::generated_github_actions_workflow;

pub(in crate::cli) fn init(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_init_help();
        return Ok(());
    }
    let options = parse_init_options(args)?;
    // #2572: `--dry-run` and the real run resolve the SAME plan, so a preview
    // can never disagree with the run it previews. Previously `--dry-run`
    // returned before every precondition check and printed file bodies
    // unconditionally, so it reported success for two runs that actually fail:
    // an existing `ripr.toml` without `--force`, and a root that is not a
    // directory.
    let plan = init_plan(&options)?;
    if let Some(warning) = unanalyzed_root_warning(&options.root) {
        eprintln!("{warning}");
    }
    if options.dry_run {
        print_init_dry_run(&plan);
        return Ok(());
    }
    apply_init_plan(&plan)
}

/// Warn before configuring ripr for a repository it cannot analyze: a Go or
/// Java repository got a workflow and "run `ripr check`" with no hint that
/// every change would be reported as not analyzed.
fn unanalyzed_root_warning(root: &Path) -> Option<String> {
    if !crate::analysis::workspace_rust_files(root).is_empty()
        || !crate::analysis::workspace_preview_language_files(root).is_empty()
    {
        return None;
    }
    let unanalyzed = crate::analysis::workspace_unanalyzed_source_languages(root);
    if unanalyzed.is_empty() {
        return None;
    }
    let found = unanalyzed
        .iter()
        .map(|(language, count)| format!("{language} ({count} file(s))"))
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "ripr: warning: the root `{}` has {found} source and no Rust, TypeScript/JavaScript or Python source. ripr does not analyze these languages, so `ripr check` and this configuration will report their changes as not analyzed.",
        output::path::human_path(root)
    ))
}

/// What `ripr init` would do to one file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InitAction {
    /// The path does not exist; it would be created.
    Create,
    /// The path exists and `--force` was given; it would be replaced.
    Overwrite,
    /// The config exists and `--ci` has work to do, so the config is left as
    /// the user wrote it, with or without `--force`.
    LeaveUnchanged,
}

impl InitAction {
    /// Fixed-width so a multi-target plan lines up in a terminal.
    fn label(self) -> &'static str {
        match self {
            Self::Create => "create        ",
            Self::Overwrite => "overwrite     ",
            Self::LeaveUnchanged => "leave existing",
        }
    }
}

/// One file in the resolved plan, with the body that would be written.
struct InitTarget {
    path: PathBuf,
    action: InitAction,
    body: String,
}

/// Resolve what the run would do, or fail with the reason it cannot run.
///
/// This is the single authority for `ripr init` preconditions: both
/// `--dry-run` and the real run go through it, so the two always agree on
/// whether the run is possible and on which files it touches.
fn init_plan(options: &InitOptions) -> Result<Vec<InitTarget>, String> {
    if !options.root.is_dir() {
        return Err(format!(
            "init root {} is not a directory",
            options.root.display()
        ));
    }
    let config_path = options.root.join(CONFIG_FILE_NAME);
    let workflow_path = options
        .ci
        .as_ref()
        .map(|ci| init_ci_workflow_path(&options.root, ci));

    // #2576 review: the plan must reject a parent the real run cannot create,
    // not just a target that already exists. If `<root>/.github` is a regular
    // file, nothing exists at `.github/workflows/ripr.yml`, so the target reads
    // as `create` while `create_dir_all` will fail — and because the config is
    // written first, the run would half-initialize the repo before failing.
    // Checking every target up front also means a doomed run writes nothing.
    for path in std::iter::once(&config_path).chain(workflow_path.as_ref()) {
        ensure_creatable_parent(path)?;
    }

    if path_is_occupied(&config_path)? && !options.force && options.ci.is_none() {
        return Err(format!(
            "{} already exists; rerun `ripr init --force` to overwrite it",
            config_path.display()
        ));
    }
    if let Some(path) = workflow_path.as_ref().filter(|_| !options.force)
        && path_is_occupied(path)?
    {
        return Err(format!(
            "{} already exists; rerun `ripr init --ci github --force` to overwrite it",
            path.display()
        ));
    }

    // With `--ci`, `--force` only lets the workflow be replaced. Refreshing a
    // workflow after an upgrade (`ripr init --ci github --force`, which
    // `ripr doctor` recommends) must not reset a customized `ripr.toml`;
    // `ripr init --force` without `--ci` still resets the config.
    let config_action = if path_is_occupied(&config_path)? {
        if options.force && options.ci.is_none() {
            InitAction::Overwrite
        } else {
            InitAction::LeaveUnchanged
        }
    } else {
        InitAction::Create
    };
    let mut targets = vec![InitTarget {
        path: config_path,
        action: config_action,
        body: generated_init_config().to_string(),
    }];
    if let Some(path) = workflow_path {
        let action = if path_is_occupied(&path)? {
            InitAction::Overwrite
        } else {
            InitAction::Create
        };
        targets.push(InitTarget {
            path,
            action,
            body: generated_github_actions_workflow(),
        });
    }
    Ok(targets)
}

/// Is anything at all sitting at `path`?
///
/// This deliberately does not use `Path::exists()`, which follows symlinks and
/// so reports `false` for a dangling symlink. The write path opens with
/// `create_new`, which fails when *any* entry occupies the path — including a
/// dangling symlink — so planning has to ask the same question the write asks.
/// `symlink_metadata` also surfaces permission errors instead of silently
/// reading as "absent, will create".
fn path_is_occupied(path: &Path) -> Result<bool, String> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(format!("cannot inspect {}: {err}", path.display())),
    }
}

/// Fail when a target's parent directory cannot be created.
///
/// `create_dir_all` fails if an existing ancestor is not a directory, so the
/// nearest existing ancestor decides whether the write is possible at all.
/// Ancestors are followed through symlinks, matching what `create_dir_all`
/// itself does.
fn ensure_creatable_parent(path: &Path) -> Result<(), String> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    for ancestor in parent.ancestors() {
        match std::fs::metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() => return Ok(()),
            Ok(_) => {
                return Err(format!(
                    "cannot write {}: {} exists and is not a directory",
                    path.display(),
                    ancestor.display()
                ));
            }
            // `NotFound` means this level would simply be created. `NotADirectory`
            // means a *shallower* ancestor is the real culprit, so keep walking
            // up until the offending entry itself is found and can be named.
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                continue;
            }
            Err(err) => {
                return Err(format!(
                    "cannot write {}: inspecting {} failed: {err}",
                    path.display(),
                    ancestor.display()
                ));
            }
        }
    }
    Ok(())
}

fn apply_init_plan(plan: &[InitTarget]) -> Result<(), String> {
    let mut wrote_any = false;
    for target in plan {
        match target.action {
            InitAction::LeaveUnchanged => {
                println!(
                    "Left existing {} unchanged",
                    output::path::human_path(&target.path)
                );
            }
            InitAction::Create => {
                write_init_target(target)?;
                wrote_any = true;
            }
            InitAction::Overwrite => {
                println!(
                    "Overwrote existing {}",
                    output::path::human_path(&target.path)
                );
                write_init_target(target)?;
                wrote_any = true;
            }
        }
    }
    if wrote_any {
        println!();
        println!(
            "Next: run `ripr doctor` to verify your setup, then `ripr check` to analyze your branch against its default branch."
        );
    }
    Ok(())
}

fn write_init_target(target: &InitTarget) -> Result<(), String> {
    if let Some(parent) = target
        .path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    if target.action == InitAction::Overwrite {
        // --force must not follow a pre-placed symlink either (#2101):
        // remove_file unlinks the entry itself (it never follows a
        // symlink to its target), and the create_new write below then
        // fails closed if anything reappears at the path.
        match std::fs::remove_file(&target.path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(format!(
                    "remove existing {} for --force failed: {err}",
                    target.path.display()
                ));
            }
        }
    }
    // Use create_new to prevent a symlink-following write race where a
    // symlink is placed at the path between the exists() check in
    // init_plan and the write. create_new fails if the path already
    // exists, including symlinks. (#1948)
    //
    // Write through the create_new handle: reopening the path after
    // creation would leave a swap window where a planted symlink is
    // followed (#2101 review, CWE-367).
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target.path)
        .map_err(|err| format!("write {} failed: {err}", target.path.display()))?;
    use std::io::Write;
    file.write_all(target.body.as_bytes())
        .map_err(|err| format!("write {} failed: {err}", target.path.display()))?;
    println!("Wrote {}", output::path::human_path(&target.path));
    Ok(())
}

pub(super) fn parse_init_options(args: &[String]) -> Result<InitOptions, String> {
    let mut options = InitOptions {
        root: PathBuf::from("."),
        dry_run: false,
        force: false,
        ci: None,
    };
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                options.root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--ci" => {
                i += 1;
                options.ci = Some(parse_init_ci(expect_value(args, i, "--ci")?)?);
            }
            "--dry-run" => options.dry_run = true,
            "--force" => options.force = true,
            other => return Err(unknown_argument("init", other)),
        }
        i += 1;
    }
    Ok(options)
}

fn parse_init_ci(value: &str) -> Result<InitCi, String> {
    match value {
        "github" => Ok(InitCi::Github),
        _ => Err(format!("unknown init --ci provider {value:?}")),
    }
}

/// Render the resolved plan, then the body of each file that would be written.
///
/// The plan block goes first so the reader learns which paths are involved and
/// what would happen to each before scrolling through a multi-hundred-line
/// generated workflow. Previously this printed bodies only, so `--dry-run`
/// never named its targets or said that nothing had been written.
fn print_init_dry_run(plan: &[InitTarget]) {
    print!("{}", render_init_dry_run(plan, cfg!(windows)));
}

/// The dry-run text, pure over the host separator rule so the Windows
/// rendering is testable anywhere (#4378): a forward-slash `--root` argv
/// prefix and the joined `\ripr.toml` suffix render as one slash path.
fn render_init_dry_run(plan: &[InitTarget], windows: bool) -> String {
    let path = |target: &InitTarget| {
        output::path::human_path_text(&target.path.to_string_lossy(), windows)
    };
    let mut text = String::from("ripr init plan (dry run — nothing was written)\n");
    for target in plan {
        text.push_str(&format!("  {} {}\n", target.action.label(), path(target)));
    }
    for target in plan {
        if target.action == InitAction::LeaveUnchanged {
            continue;
        }
        text.push_str(&format!("\n# {}\n", path(target)));
        text.push_str(&target.body);
    }
    text.push_str("\nRerun without --dry-run to apply.\n");
    text
}

fn init_ci_workflow_path(root: &Path, ci: &InitCi) -> PathBuf {
    match ci {
        InitCi::Github => root.join(".github/workflows/ripr.yml"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::agent_review_summary::NO_RECEIPT_BEFORE_REPAIR;
    use crate::cli::commands_options::InitCi;
    use crate::output::first_pr::{
        MANUAL_RECEIPT_LABEL, MANUAL_VERIFY_LABEL, RECEIPT_AFTER_VERIFY_LABEL,
        REPAIR_AFTER_PHASE_LABEL, REPAIR_AFTER_PHASE_STEP, VERIFY_AFTER_EDIT_LABEL,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    /// #4378: on Windows `ripr init --dry-run --root <drive>:/Temp/demo`
    /// printed `<drive>:/Temp/demo\ripr.toml`, two separator styles in one
    /// path. The preview uses one convention per host and never rewrites a
    /// Unix filename's backslash.
    #[test]
    fn dry_run_preview_uses_one_separator_convention_per_host() {
        // `check-local-context` forbids drive-letter path literals, so the
        // Windows shape is assembled from parts.
        let root = format!("{}/Temp/demo", "F:");
        let plan = vec![InitTarget {
            path: PathBuf::from(format!(r"{root}\ripr.toml")),
            action: InitAction::Create,
            body: "[analysis]\n".to_string(),
        }];
        let windows = render_init_dry_run(&plan, true);
        assert!(
            windows.contains(&format!("  create         {root}/ripr.toml\n"))
                && windows.contains(&format!("# {root}/ripr.toml\n")),
            "{windows}"
        );
        assert!(!windows.contains('\\'), "{windows}");
        let unix = render_init_dry_run(&plan, false);
        assert!(unix.contains(&format!(r"# {root}\ripr.toml")), "{unix}");
        assert!(
            unix.ends_with("\nRerun without --dry-run to apply.\n"),
            "{unix}"
        );
    }

    /// #4391: the steps use bash-only syntax, so the job pins `shell: bash`
    /// instead of inheriting a runner default (PowerShell on Windows).
    #[test]
    fn generated_workflow_pins_bash_for_every_run_step() {
        let workflow = generated_github_actions_workflow();
        let defaults_at = workflow
            .find("\ndefaults:\n  run:\n    shell: bash\n")
            .unwrap_or(usize::MAX);
        let jobs_at = workflow.find("\njobs:\n").unwrap_or(usize::MAX);
        assert!(
            defaults_at < jobs_at && jobs_at != usize::MAX,
            "the workflow must pin bash for every job:\n{workflow}"
        );
        assert!(
            workflow.contains("gate_args=("),
            "bash-only syntax the pin protects"
        );
    }

    /// The workflow carries the shared proof-path labels (#3906) inside
    /// single-quoted shell strings, so none may hold a single quote, and
    /// every placeholder must be substituted.
    #[test]
    fn generated_workflow_substitutes_shared_labels_into_single_quoted_strings() {
        for text in [
            REPAIR_AFTER_PHASE_LABEL,
            REPAIR_AFTER_PHASE_STEP,
            MANUAL_VERIFY_LABEL,
            MANUAL_RECEIPT_LABEL,
            VERIFY_AFTER_EDIT_LABEL,
            RECEIPT_AFTER_VERIFY_LABEL,
            NO_RECEIPT_BEFORE_REPAIR,
        ] {
            assert!(!text.contains('\''), "{text}");
        }
        let workflow = generated_github_actions_workflow();
        assert!(!workflow.contains("@RIPR_"), "unsubstituted placeholder");
        assert!(workflow.contains(&format!("='{MANUAL_VERIFY_LABEL}'")));
        assert!(workflow.contains(&format!("echo '- Receipt: {NO_RECEIPT_BEFORE_REPAIR}'")));
    }

    /// #4726: the gate reads PR labels from `$GITHUB_EVENT_PATH`, so a run
    /// only sees a waiver label that exists when its event fires. Without
    /// `labeled`/`unlabeled` triggers, adding `ripr-waive` never re-evaluates
    /// an `acknowledgeable` gate and removing it leaves a stale green.
    #[test]
    fn generated_workflow_reruns_when_pull_request_labels_change() -> Result<(), String> {
        let workflow = generated_github_actions_workflow();
        assert!(
            workflow.contains("\"$GITHUB_EVENT_PATH\" > target/ci/labels.json"),
            "labels are no longer read from the event payload; revisit #4726"
        );
        let on_block: Vec<&str> = workflow
            .lines()
            .skip_while(|line| *line != "on:")
            .skip(1)
            .take_while(|line| line.starts_with(' ') || line.is_empty())
            .collect();
        let pull_request = on_block
            .iter()
            .position(|line| *line == "  pull_request:")
            .ok_or_else(|| format!("no pull_request trigger in {on_block:?}"))?;
        let types = on_block
            .iter()
            .skip(pull_request + 1)
            .take_while(|line| line.starts_with("    "))
            .find_map(|line| line.trim().strip_prefix("types:"))
            .ok_or_else(|| format!("pull_request trigger has no types: {on_block:?}"))?;
        let types: Vec<&str> = types
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(str::trim)
            .collect();
        assert_eq!(
            types,
            ["opened", "synchronize", "reopened", "labeled", "unlabeled"]
        );
        Ok(())
    }

    /// W5: an unpinned `cargo install ripr` installs whatever release is
    /// latest, so CI could run an older ripr that lacks the commands this
    /// workflow calls, or change behavior silently on a later release. The
    /// install pins the generating binary's own version and keeps `--locked`.
    #[test]
    fn generated_workflow_pins_the_generating_ripr_version() {
        let workflow = generated_github_actions_workflow();
        let pinned = format!(
            "run: cargo install ripr --version {} --locked",
            env!("CARGO_PKG_VERSION")
        );
        assert!(workflow.contains(&pinned), "missing {pinned}");
        let installs: Vec<&str> = workflow
            .lines()
            .filter(|line| line.contains("run: cargo install"))
            .collect();
        assert_eq!(installs.len(), 1, "{installs:?}");
    }

    /// The `cli_smoke` tests drive `ripr init` as a subprocess, so they prove
    /// end-to-end behavior but leave the planning logic uninstrumented. These
    /// in-process tests exercise `init_plan` and its precondition helpers
    /// directly, which is also where the interesting branches live.
    fn temp_root(name: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root =
            std::env::temp_dir().join(format!("ripr-init-{name}-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&root).map_err(|err| format!("create temp root failed: {err}"))?;
        Ok(root)
    }

    #[test]
    fn unanalyzed_root_warning_names_go_only_repositories() -> Result<(), String> {
        let root = temp_root("go-only")?;
        std::fs::write(root.join("main.go"), "package main\n")
            .map_err(|err| format!("write main.go: {err}"))?;
        let warning = unanalyzed_root_warning(&root).unwrap_or_default();
        assert!(warning.contains("Go (1 file(s))"), "{warning}");
        assert!(
            warning.contains("report their changes as not analyzed"),
            "{warning}"
        );
        // Negative control: Rust source beside the Go file is analyzable.
        std::fs::create_dir_all(root.join("src")).map_err(|err| format!("mkdir: {err}"))?;
        std::fs::write(root.join("src/lib.rs"), "pub fn f() {}\n")
            .map_err(|err| format!("write lib.rs: {err}"))?;
        assert_eq!(unanalyzed_root_warning(&root), None);
        std::fs::remove_dir_all(&root).map_err(|err| format!("cleanup: {err}"))?;
        Ok(())
    }

    fn options(root: &Path) -> InitOptions {
        InitOptions {
            root: root.to_path_buf(),
            dry_run: false,
            force: false,
            ci: None,
        }
    }

    fn write(path: &Path, text: &str) -> Result<(), String> {
        std::fs::write(path, text).map_err(|err| format!("write {} failed: {err}", path.display()))
    }

    #[test]
    fn plan_creates_the_config_in_a_clean_root() -> Result<(), String> {
        let root = temp_root("clean")?;
        let plan = init_plan(&options(&root))?;

        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].action, InitAction::Create);
        assert_eq!(plan[0].path, root.join(CONFIG_FILE_NAME));
        assert!(plan[0].body.contains("[analysis]"));

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn plan_adds_the_workflow_target_for_ci_github() -> Result<(), String> {
        let root = temp_root("ci")?;
        let mut opts = options(&root);
        opts.ci = Some(InitCi::Github);
        let plan = init_plan(&opts)?;

        assert_eq!(plan.len(), 2);
        assert_eq!(plan[1].action, InitAction::Create);
        assert_eq!(plan[1].path, root.join(".github/workflows/ripr.yml"));
        assert!(plan[1].body.contains("name: RIPR"));

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn plan_rejects_an_existing_config_without_force() -> Result<(), String> {
        let root = temp_root("exists")?;
        write(&root.join(CONFIG_FILE_NAME), "[analysis]\n")?;

        match init_plan(&options(&root)) {
            Ok(_) => return Err("an existing config without --force must block".to_string()),
            Err(message) => {
                assert!(message.contains("already exists"), "{message}");
                assert!(message.contains("--force"), "{message}");
            }
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn plan_overwrites_an_existing_config_with_force() -> Result<(), String> {
        let root = temp_root("force")?;
        write(&root.join(CONFIG_FILE_NAME), "[analysis]\n")?;
        let mut opts = options(&root);
        opts.force = true;

        let plan = init_plan(&opts)?;
        assert_eq!(plan[0].action, InitAction::Overwrite);

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// An existing config only blocks when there is nothing else to do; with
    /// `--ci` the run still has a workflow to write.
    #[test]
    fn plan_leaves_an_existing_config_alone_when_ci_still_has_work() -> Result<(), String> {
        let root = temp_root("leave")?;
        write(&root.join(CONFIG_FILE_NAME), "[analysis]\n")?;
        let mut opts = options(&root);
        opts.ci = Some(InitCi::Github);

        let plan = init_plan(&opts)?;
        assert_eq!(plan[0].action, InitAction::LeaveUnchanged);
        assert_eq!(plan[1].action, InitAction::Create);

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// Upgrade path: refreshing an existing workflow with `--force` replaces
    /// the workflow and keeps the repository's own `ripr.toml`.
    #[test]
    fn plan_ci_force_refreshes_the_workflow_and_keeps_the_config() -> Result<(), String> {
        let root = temp_root("ci-force")?;
        write(
            &root.join(CONFIG_FILE_NAME),
            "[lsp]\nseam_diagnostics = false\n",
        )?;
        let workflow = root.join(".github/workflows/ripr.yml");
        if let Some(parent) = workflow.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
        }
        write(&workflow, "run: cargo install ripr --locked\n")?;
        let mut opts = options(&root);
        opts.ci = Some(InitCi::Github);
        opts.force = true;

        let plan = init_plan(&opts)?;
        assert_eq!(plan[0].action, InitAction::LeaveUnchanged);
        assert_eq!(plan[1].action, InitAction::Overwrite);
        apply_init_plan(&plan)?;
        let config = std::fs::read_to_string(root.join(CONFIG_FILE_NAME))
            .map_err(|err| format!("read config failed: {err}"))?;
        assert_eq!(config, "[lsp]\nseam_diagnostics = false\n");
        let written = std::fs::read_to_string(&workflow)
            .map_err(|err| format!("read workflow failed: {err}"))?;
        assert!(written.contains("--version"), "{written}");

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn plan_rejects_an_existing_workflow_without_force() -> Result<(), String> {
        let root = temp_root("wf-exists")?;
        let workflow = root.join(".github/workflows/ripr.yml");
        if let Some(parent) = workflow.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
        }
        write(&workflow, "name: existing\n")?;
        let mut opts = options(&root);
        opts.ci = Some(InitCi::Github);

        match init_plan(&opts) {
            Ok(_) => return Err("an existing workflow without --force must block".to_string()),
            Err(message) => assert!(message.contains("already exists"), "{message}"),
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn plan_rejects_a_root_that_is_not_a_directory() -> Result<(), String> {
        let root = temp_root("bad-root")?;
        let file_root = root.join("a-file");
        write(&file_root, "not a directory\n")?;

        match init_plan(&options(&file_root)) {
            Ok(_) => return Err("a non-directory root must block".to_string()),
            Err(message) => assert!(message.contains("is not a directory"), "{message}"),
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// #2576 review: the plan must reject a parent `create_dir_all` cannot
    /// make, and must name the entry that actually blocks it.
    #[test]
    fn plan_rejects_a_workflow_parent_that_is_a_file() -> Result<(), String> {
        let root = temp_root("parent-file")?;
        write(&root.join(".github"), "not a directory\n")?;
        let mut opts = options(&root);
        opts.ci = Some(InitCi::Github);

        match init_plan(&opts) {
            Ok(_) => return Err("an uncreatable parent must block".to_string()),
            Err(message) => {
                assert!(
                    message.contains("exists and is not a directory"),
                    "{message}"
                );
                assert!(
                    message.contains(&root.join(".github").display().to_string()),
                    "message should name the blocking entry: {message}"
                );
            }
        }

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn creatable_parent_accepts_directories_that_do_not_exist_yet() -> Result<(), String> {
        let root = temp_root("deep")?;
        ensure_creatable_parent(&root.join("a/b/c/file.yml"))?;

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn occupied_reports_absent_and_present_paths() -> Result<(), String> {
        let root = temp_root("occupied")?;
        assert!(!path_is_occupied(&root.join("missing"))?);

        let file = root.join("present");
        write(&file, "x")?;
        assert!(path_is_occupied(&file)?);

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// `Path::exists()` follows symlinks and reports `false` here, but
    /// `create_new` still refuses the path, so planning must call it occupied.
    #[cfg(unix)]
    #[test]
    fn occupied_reports_a_dangling_symlink_as_present() -> Result<(), String> {
        let root = temp_root("dangling")?;
        let link = root.join("link");
        std::os::unix::fs::symlink(root.join("nowhere"), &link)
            .map_err(|err| format!("symlink failed: {err}"))?;

        assert!(!link.exists(), "precondition: exists() misses this case");
        assert!(path_is_occupied(&link)?);

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn action_labels_are_padded_to_a_common_width() {
        let width = InitAction::Create.label().len();
        assert_eq!(InitAction::Overwrite.label().len(), width);
        assert_eq!(InitAction::LeaveUnchanged.label().len(), width);
        assert!(InitAction::Create.label().starts_with("create"));
        assert!(InitAction::Overwrite.label().starts_with("overwrite"));
        assert!(
            InitAction::LeaveUnchanged
                .label()
                .starts_with("leave existing")
        );
    }
}
