//! Arg-parsing and dispatch for `ripr check`.
//!
//! This is the CLI adapter layer only. Analysis, evaluation, and rendering
//! semantics live in `crate::app`, `crate::analysis`, and `crate::output`.
//! This module owns argv parsing, output destination selection, and exit
//! mapping for the check command family.

use crate::analysis;
use crate::app::{self, CheckInput, OutputFormat};
use crate::cli::commands_context::ensure_command_root;
use crate::cli::help;
use crate::cli::parse::{
    disclose_attached_terminal_stdin_read, expect_value, parse_format, parse_mode,
};
use crate::cli::suggest::unknown_argument;
use crate::config::{CheckInputExplicit, RiprConfig, apply_to_check_input, load_for_root};
use crate::output;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

fn repo_scope_diff_bound_warning(
    format: OutputFormat,
    base_explicitly_provided: bool,
    diff_file: Option<&Path>,
) -> Option<String> {
    if !format.is_repo_scope() || (!base_explicitly_provided && diff_file.is_none()) {
        return None;
    }
    Some(format!(
        "ripr: format {} is repo-scoped; --base/--diff does not bound it.\n\
Use --format json for diff-scoped findings, or --format repo-exposure-summary-json for a bounded repo summary.",
        format.primary_cli_name()
    ))
}

/// A repo-scoped format does not read the diff, but `--base` is still
/// recorded as snapshot provenance (`base_revision`) and `--diff` still
/// names an input. Both must exist, exactly as on the diff-scoped path,
/// so a typo exits 2 instead of producing an artifact that records a ref
/// or file that is not there (#4445).
fn validate_repo_scope_diff_inputs(
    input: &CheckInput,
    base_explicitly_provided: bool,
) -> Result<(), String> {
    if let Some(diff) = input.diff_file.as_deref() {
        if diff != Path::new("-") && !diff.is_file() {
            return Err(format!(
                "check: --diff {} is not a readable file",
                diff.display()
            ));
        }
        return Ok(());
    }
    if !base_explicitly_provided {
        return Ok(());
    }
    let Some(base) = input.base.as_deref() else {
        return Ok(());
    };
    let commit = format!("{base}^{{commit}}");
    let output = crate::git::run_git_output_with_deadline(
        &input.root,
        &["rev-parse", "--verify", "--quiet", commit.as_str()],
        input.git_timeout,
    )?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "check: base revision {base:?} does not resolve to a commit in {}",
            input.root.display()
        ))
    }
}

pub(super) fn resolve_workspace_root(start: &Path) -> Result<Option<PathBuf>, String> {
    let start = std::fs::canonicalize(start).map_err(|error| {
        format!(
            "resolve implicit workspace root from {} failed: {error}",
            start.display()
        )
    })?;

    for ancestor in start.ancestors() {
        if manifest_declares_workspace(&ancestor.join("Cargo.toml")) {
            return Ok(Some(ancestor.to_path_buf()));
        }
        // A git top level bounds the walk: a workspace in an enclosing
        // repository never claims a nested, independent repository.
        if ancestor.join(".git").exists() {
            break;
        }
    }
    Ok(None)
}

fn manifest_declares_workspace(manifest: &Path) -> bool {
    std::fs::read_to_string(manifest)
        .ok()
        .and_then(|contents| toml::from_str::<toml::Value>(&contents).ok())
        .is_some_and(|document| document.get("workspace").is_some_and(toml::Value::is_table))
}

/// Why an implicit run moved away from the current directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ImplicitRootReason {
    Workspace,
    Package,
    /// #4553: JavaScript and Python workspace declarations.
    PnpmWorkspace,
    PackageJsonWorkspaces,
    UvWorkspace,
    GitTopLevel,
}

impl ImplicitRootReason {
    fn disclosure(self) -> &'static str {
        match self {
            Self::Workspace => "Cargo.toml contains [workspace]",
            Self::Package => "nearest Cargo.toml",
            Self::PnpmWorkspace => "pnpm-workspace.yaml",
            Self::PackageJsonWorkspaces => "package.json declares workspaces",
            Self::UvWorkspace => "pyproject.toml contains [tool.uv.workspace]",
            Self::GitTopLevel => "git top level; no Cargo.toml found",
        }
    }
}

/// Resolve the root an implicit run analyzes from `start`.
///
/// A `[workspace]` manifest anywhere above wins. Otherwise the nearest
/// ancestor holding a `Cargo.toml`, a JavaScript or Python workspace
/// declaration (`pnpm-workspace.yaml`, a `package.json` with `workspaces`,
/// a `pyproject.toml` with `[tool.uv.workspace]`), or a `.git` entry is the
/// root. So a run from `repo/src` of a single-crate repo analyzes the crate
/// instead of silently scoping the diff to `src/` and reporting a clean,
/// complete result (#4610), and a run from one package of a pnpm/npm/yarn/bun
/// or uv monorepo analyzes the workspace, where sibling-package tests are
/// visible (#4553). The walk stops at the git top level so a stray manifest
/// outside the repository is never adopted.
pub(super) fn resolve_project_root(
    start: &Path,
) -> Result<Option<(PathBuf, ImplicitRootReason)>, String> {
    if let Some(root) = resolve_workspace_root(start)? {
        return Ok(Some((root, ImplicitRootReason::Workspace)));
    }
    let start = std::fs::canonicalize(start).map_err(|error| {
        format!(
            "resolve implicit project root from {} failed: {error}",
            start.display()
        )
    })?;
    for ancestor in start.ancestors() {
        if ancestor.join("Cargo.toml").is_file() {
            return Ok(Some((ancestor.to_path_buf(), ImplicitRootReason::Package)));
        }
        if let Some(reason) = non_cargo_workspace_marker(ancestor) {
            return Ok(Some((ancestor.to_path_buf(), reason)));
        }
        if ancestor.join(".git").exists() {
            return Ok(Some((
                ancestor.to_path_buf(),
                ImplicitRootReason::GitTopLevel,
            )));
        }
    }
    Ok(None)
}

fn non_cargo_workspace_marker(dir: &Path) -> Option<ImplicitRootReason> {
    if dir.join("pnpm-workspace.yaml").is_file() {
        return Some(ImplicitRootReason::PnpmWorkspace);
    }
    if std::fs::read_to_string(dir.join("package.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .is_some_and(|manifest| manifest.get("workspaces").is_some())
    {
        return Some(ImplicitRootReason::PackageJsonWorkspaces);
    }
    if std::fs::read_to_string(dir.join("pyproject.toml"))
        .ok()
        .and_then(|text| toml::from_str::<toml::Value>(&text).ok())
        .is_some_and(|document| {
            document
                .get("tool")
                .and_then(|tool| tool.get("uv"))
                .and_then(|uv| uv.get("workspace"))
                .is_some_and(toml::Value::is_table)
        })
    {
        return Some(ImplicitRootReason::UvWorkspace);
    }
    None
}

fn resolve_implicit_workspace_root(input: &mut CheckInput) -> Result<(), String> {
    let Some((root, reason)) = resolve_project_root(Path::new("."))? else {
        return Ok(());
    };
    let current = std::fs::canonicalize(".")
        .map_err(|error| format!("resolve current directory failed: {error}"))?;
    if root == current {
        return Ok(());
    }

    eprintln!(
        "ripr: resolved workspace root to {} ({})",
        root.display(),
        reason.disclosure()
    );
    input.root = root;
    Ok(())
}

fn parse_git_timeout(value: &str) -> Result<Option<std::time::Duration>, String> {
    parse_git_timeout_from("--git-timeout", value)
}

/// Parse a git timeout in seconds from `source` (the flag or the env var),
/// naming that source in every refusal so a typo never runs with a silently
/// different deadline (#4374).
fn parse_git_timeout_from(
    source: &str,
    value: &str,
) -> Result<Option<std::time::Duration>, String> {
    let secs: u64 = value.parse().map_err(|_parse_err| {
        format!("{source} requires a non-negative integer (seconds); got {value:?}")
    })?;
    let timeout = std::time::Duration::from_secs(secs);
    if std::time::Instant::now().checked_add(timeout).is_none() {
        return Err(format!(
            "{source} is too large for the platform deadline; got {secs} seconds"
        ));
    }
    Ok((secs > 0).then_some(timeout))
}

fn git_timeout_from_env(
    explicit: bool,
    env_value: Result<String, std::env::VarError>,
) -> Result<Option<Option<std::time::Duration>>, String> {
    if explicit {
        return Ok(None);
    }
    match env_value {
        Ok(value) => parse_git_timeout_from("RIPR_GIT_TIMEOUT", &value).map(Some),
        Err(std::env::VarError::NotPresent) => Ok(None),
        // Present but unreadable is still a misconfiguration (#4374).
        Err(std::env::VarError::NotUnicode(_)) => {
            Err("RIPR_GIT_TIMEOUT must be valid UTF-8".to_string())
        }
    }
}

pub(in crate::cli) fn check(args: &[String]) -> Result<(), String> {
    let mut input = CheckInput {
        git_timeout: Some(app::default_cli_git_timeout()),
        ..CheckInput::default()
    };
    let mut explicit = CheckInputExplicit::default();
    let mut gap_ledger: Option<PathBuf> = None;
    // RIPR-SPEC-0083: track whether the user provided any analysis scope.
    // Starts false; set true when --diff, --base, or --worktree is parsed from argv.
    // --mode is a SPEED TIER on the diff path, NOT a scope provider — a bare
    // `ripr check --mode fast` analyzes nothing and must still show the no-scope
    // disclosure. When still false at analysis time, the output discloses that
    // nothing was analyzed, preventing an empty result from being read as clean.
    let mut scope_explicitly_provided = false;
    // RIPR-SPEC-0084: track whether --base was explicitly given by the user.
    // When false, the CLI resolves the repo's real default branch before
    // running analysis. An explicit bad --base keeps its error; only the
    // default path triggers auto-resolution.
    let mut base_explicitly_provided = false;
    let mut worktree_explicitly_provided = false;
    let mut root_explicitly_provided = false;
    // RIPR-SPEC-0140: explicit artifact sink for the explain/context reuse
    // pair. No implicit cache: the user names the artifact path.
    let mut write_artifact: Option<PathBuf> = None;
    let mut git_timeout_explicitly_provided = false;
    let mut candidate_tree: Option<String> = None;
    let mut candidate_base: Option<String> = None;
    let mut quiet = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                input.root = PathBuf::from(expect_value(args, i, "--root")?);
                root_explicitly_provided = true;
            }
            "--base" => {
                i += 1;
                input.base = Some(expect_value(args, i, "--base")?.to_string());
                scope_explicitly_provided = true;
                base_explicitly_provided = true;
            }
            "--diff" => {
                i += 1;
                input.diff_file = Some(PathBuf::from(expect_value(args, i, "--diff")?));
                scope_explicitly_provided = true;
            }
            // #3237/#3278: immutable Git candidate input. The subject
            // owns both identities; --diff/--base conflicts are
            // rejected at binding (app::analysis_subject).
            "--candidate-tree" => {
                i += 1;
                candidate_tree = Some(expect_value(args, i, "--candidate-tree")?.to_string());
                scope_explicitly_provided = true;
            }
            "--candidate-base" => {
                i += 1;
                candidate_base = Some(expect_value(args, i, "--candidate-base")?.to_string());
            }
            "--worktree" => {
                scope_explicitly_provided = true;
                worktree_explicitly_provided = true;
            }
            "--mode" => {
                i += 1;
                input.mode = parse_mode(expect_value(args, i, "--mode")?)?;
                explicit.mode = true;
                // NOTE: do NOT set scope_explicitly_provided here.
                // --mode is a speed tier on the diff path, not a scope provider.
                // `ripr check --mode fast` with no --diff/--base analyzes nothing
                // and must still trigger the no-scope disclosure (RIPR-SPEC-0083).
                // #2644: the `fast` no-op notice is NOT emitted here — argv
                // position is not the effective mode. It fires once after the
                // config merge below.
            }
            "--json" => input.format = OutputFormat::Json,
            "--format" => {
                i += 1;
                input.format = parse_format(expect_value(args, i, "--format")?)?;
            }
            "--gap-ledger" => {
                i += 1;
                gap_ledger = Some(PathBuf::from(expect_value(args, i, "--gap-ledger")?));
            }
            "--no-unchanged-tests" => {
                input.include_unchanged_tests = false;
                explicit.include_unchanged_tests = true;
            }
            "--perl-facts" => {
                i += 1;
                input.perl_facts_path = Some(PathBuf::from(expect_value(args, i, "--perl-facts")?));
            }
            "--suppression-policy" => {
                i += 1;
                input.suppression_policy = Some(PathBuf::from(expect_value(
                    args,
                    i,
                    "--suppression-policy",
                )?));
            }
            "--write-artifact" => {
                i += 1;
                write_artifact = Some(PathBuf::from(expect_value(args, i, "--write-artifact")?));
            }
            "--git-timeout" => {
                i += 1;
                let value = expect_value(args, i, "--git-timeout")?;
                input.git_timeout = parse_git_timeout(value)?;
                git_timeout_explicitly_provided = true;
            }
            "--quiet" => {
                quiet = true;
            }
            "--help" | "-h" => {
                help::print_check_help();
                return Ok(());
            }
            other => return Err(unknown_argument("check", other)),
        }
        i += 1;
    }
    if root_explicitly_provided {
        // An explicit --root that is not a directory reached the diff loader
        // and surfaced git's spawn failure, complete with the full argv:
        // `failed to run git diff: failed to run git -C /no/such/dir ["-c",
        // "core.quotePath=true", "diff", ...]: No such file or directory`.
        // Validate it through the same authority `rerun`, `agent`, and
        // `swarm` already use, so the user is told which path is wrong rather
        // than being handed the invocation that failed on it. Only an explicit
        // root is checked: the implicit path below legitimately walks up from
        // the current directory.
        ensure_command_root(&input.root, "check")?;
    } else {
        resolve_implicit_workspace_root(&mut input)?;
    }
    // RIPR-SPEC-0084: when no --base was explicitly given AND no --diff file
    // was provided, resolve the repo's real default branch instead of
    // hardcoding origin/main. Setting base to None here triggers
    // `load_diff` → `resolve_default_base`, which tries (in order):
    // symbolic-ref origin/HEAD → origin/main → origin/master → main → master.
    // When --diff is given, input.base is kept as-is (it appears in output for
    // informational purposes but is not used for the diff itself). When --base
    // is explicitly given, base_explicitly_provided is true and we preserve it.
    if !base_explicitly_provided && input.diff_file.is_none() {
        input.base = None;
    }
    // #2613: RIPR_GIT_TIMEOUT env var is a fallback when --git-timeout was
    // not passed on the command line. Seconds; 0 disables the deadline.
    if let Some(timeout) = git_timeout_from_env(
        git_timeout_explicitly_provided,
        std::env::var("RIPR_GIT_TIMEOUT"),
    )? {
        input.git_timeout = timeout;
    }
    if worktree_explicitly_provided && input.diff_file.is_some() {
        return Err("check --worktree cannot be combined with --diff".to_string());
    }
    // #1441: --suppression-policy applies to the findings-based check
    // surfaces only. SARIF keeps its existing `.ripr/suppressions.toml`
    // finding_id channel, and badge/repo formats have their own suppression
    // projections — silently ignoring the flag there would misreport policy
    // application, so fail closed with a named limitation instead.
    if input.suppression_policy.is_some()
        && !matches!(
            input.format,
            OutputFormat::Human
                | OutputFormat::HumanFull
                | OutputFormat::Json
                | OutputFormat::Github
        )
    {
        return Err(
            "--suppression-policy applies to the findings-based check formats (human, human-full, json, github); \
             it is not yet supported for SARIF, badge, or repo formats"
                .to_string(),
        );
    }
    // #3237/#3278: bind the immutable subject before the analysis
    // call. Construction runs #3276's typed validation (malformed OIDs,
    // oversized treeishes) and the binding layer rejects --diff/--base
    // combinations with named errors, so an invalid subject never
    // reaches analysis.
    if let Some(tree) = candidate_tree.as_deref() {
        let base = match candidate_base.as_deref() {
            Some(explicit) => crate::domain::GitCandidateBase::Treeish(
                crate::domain::GitTreeish::new(explicit).map_err(|error| error.to_string())?,
            ),
            None => crate::domain::GitCandidateBase::EmptyTree,
        };
        let candidate =
            crate::domain::GitObjectId::parse(tree).map_err(|error| error.to_string())?;
        input.git_candidate = Some(crate::domain::GitCandidateSubject::new(
            input.root.clone(),
            base,
            candidate,
        ));
    }
    // #3278 review: --candidate-base without --candidate-tree is a
    // dangling input the run would silently ignore; fail closed.
    if candidate_base.is_some() && candidate_tree.is_none() {
        return Err(
            "--candidate-base requires --candidate-tree: name the candidate tree the base applies to"
                .to_string(),
        );
    }
    // #4252: a bound subject configures itself from its candidate tree
    // (#3279 R4 below), so the worktree ripr.toml is never read for it.
    // Loading it here only to feed the argv gates let a worktree file the
    // subject must ignore still decide the run: an unparseable file, or a
    // `languages.enabled` entry this binary lacks (`python` in a Rust-only
    // build), aborted it with exit 2, and a worktree `[analysis] mode`
    // carried into a subject whose tree sets none. The gates below read
    // only argv-derived state, so the pure default serves them.
    let config = if candidate_tree.is_some() {
        RiprConfig::default()
    } else {
        load_for_root(&input.root)?
    };
    apply_to_check_input(&mut input, &config, explicit);
    let format = input.format;
    // #3278 review M1: repo-scope formats, repo exposure, and the gap
    // ledger analyze the LIVE repository by definition. A bound
    // immutable subject is an exact-tree contract; silently dropping it
    // would render live-repo output under a candidate-tree invocation —
    // the exact "analyzed T vs analyzed HEAD by fallback" confusion
    // this surface exists to eliminate. Fail closed with a named error.
    let subject_bound_with_live_repo_path = candidate_tree.is_some()
        && (gap_ledger.is_some()
            || matches!(format, OutputFormat::RepoExposureJson)
            || format.is_repo_scope());
    if subject_bound_with_live_repo_path {
        return Err(format!(
            "--candidate-tree cannot be combined with the live-repository path (--format {}{}): the subject binds exact trees, not the live repo",
            format.primary_cli_name(),
            if gap_ledger.is_some() {
                " with --gap-ledger"
            } else {
                ""
            }
        ));
    }
    // RIPR-SPEC-0140: --write-artifact records a diff-scoped findings run.
    // Repo-scoped and gap-ledger paths produce no such finding set, so both
    // fail closed with a named limitation rather than silently skipping the
    // requested artifact. --worktree runs record the base-to-worktree diff
    // source, which is re-resolvable at reuse time.
    if let Some(path) = write_artifact.as_ref() {
        // #3278 review B1: the artifact records a diff source and its
        // reuse verifier re-resolves it; a tree-to-tree subject diff is
        // not re-resolvable from the recorded shape, so the artifact
        // would misdescribe the analyzed run and pass verification
        // vacuously. Fail closed until a subject-aware diff source
        // identity exists (R4 scope).
        if candidate_tree.is_some() {
            return Err(format!(
                "--write-artifact {} cannot be combined with --candidate-tree: the artifact records a diff source; the subject's tree-to-tree diff is not re-resolvable yet",
                path.display()
            ));
        }
        if gap_ledger.is_some() {
            return Err(format!(
                "--write-artifact {} cannot be combined with --gap-ledger: the artifact records a findings-based check run",
                path.display()
            ));
        }
        if matches!(format, OutputFormat::RepoExposureJson) || format.is_repo_scope() {
            return Err(format!(
                "--write-artifact {} records a diff-scoped findings run; --format {} is repo-scoped and produces no such artifact",
                path.display(),
                format.primary_cli_name()
            ));
        }
        // Managed producer mode generates the Perl fact packet inside
        // `run_check`, after the CLI-level input was captured — the
        // generated packet would not be part of the recorded identity, and
        // resolving it at reuse time would re-run the producer and defeat
        // reuse, so fail closed with a named limitation. An explicit
        // --perl-facts packet is already recorded (path + content hash) and
        // remains supported.
        if input.perl_facts_path.is_none()
            && config
                .perl()
                .producer()
                .is_some_and(app::is_managed_perl_producer)
        {
            return Err(format!(
                "--write-artifact {} does not support [perl] producer packet generation (named limitation: the generated packet is not part of the recorded identity); pass --perl-facts <path> explicitly to make the packet part of the recorded identity",
                path.display()
            ));
        }
    }
    // #3279 R4: a bound subject configures itself — the candidate
    // tree's own ripr.toml (or the pure default when the tree carries
    // none) replaces the worktree file, so a dirty worktree config
    // cannot change a subject run. Applied AFTER the argv conflict
    // gates so an input conflict never depends on a Git object read
    // (the R3 conflict tests pin the error text, not the ordering;
    // #3279 review m1). The worktree `config` value above fed only
    // gates that reject subject combinations outright.
    let mut config = config;
    if let Some(subject) = input.git_candidate.as_ref() {
        config = crate::config::config_for_candidate(subject, &config)?;
        apply_to_check_input(&mut input, &config, explicit);
    }
    // #2644: `fast` is currently behaviorally identical to `draft`. The notice
    // fires on the EFFECTIVE mode after `apply_to_check_input`, not on argv
    // position, so a repo `ripr.toml` with `mode = "fast"` is disclosed too,
    // `--mode fast --mode deep` stays silent, and a repeated `--mode fast`
    // discloses once. stderr only: stdout and every machine format are
    // unchanged. This sits before the gap-ledger and repo-exposure-json early
    // returns below so repo-scoped formats keep the notice.
    if input.mode == app::Mode::Fast {
        eprintln!(
            "ripr: mode fast is currently identical to mode draft; there is no behavioral difference. \
             Use draft or deep (--mode on the command line, or [analysis] mode in ripr.toml)."
        );
    }
    // #2901: OraclePolicy (snapshot_strength, mock_expectation_strength,
    // broad_error_strength) is consumed only by the Rust adapter. Python,
    // Perl, and TypeScript silently ignore it. Warn when a non-Rust language
    // is enabled and the policy is non-default, so the config-identity hash
    // change (which invalidates caches for all languages) is not mistaken for
    // a behavioral change in the preview adapters.
    let oracle_customized = config.oracles != crate::config::OraclePolicy::default();
    let has_preview_language = config
        .languages
        .enabled
        .iter()
        .any(|lang| !matches!(lang, crate::domain::LanguageId::Rust));
    if oracle_customized && has_preview_language {
        eprintln!(
            "ripr: [oracles] policy applies to Rust only; TypeScript/Python/Perl adapters use \
             hardcoded oracle strengths. The config-identity hash changes for all languages but \
             analysis behavior changes only for Rust."
        );
    }
    if let Some(warning) =
        repo_scope_diff_bound_warning(format, base_explicitly_provided, input.diff_file.as_deref())
    {
        eprintln!("{warning}");
    }
    if format.is_repo_scope() {
        validate_repo_scope_diff_inputs(&input, base_explicitly_provided)?;
    }
    if let Some(gap_ledger) = gap_ledger.as_ref() {
        write_stdout_chunked(&render_check_gap_ledger_badge(
            gap_ledger, &format, &config,
        )?)?;
        return Ok(());
    }
    // #4945: the sink is wired BEFORE every repo-format path so the
    // longest-running surfaces project the same producer stages the
    // diff-scoped path promises (`ripr progress: <stage> [<scope>]`,
    // throttled heartbeats included). Both the audit-path disclosure and the
    // stage lines are stderr-only and cannot change machine stdout.
    let progress = (!quiet).then(|| {
        crate::cli::progress::CliProgressSink::for_stderr(
            std::io::stderr().is_terminal(),
            crate::cli::progress::ProgressPolicy::STANDARD,
        )
    });
    let progress_sink = progress
        .as_ref()
        .map(|sink| sink as &dyn crate::app::AnalysisProgressSink);
    // #4945: invocation-time cost disclosure for the full-repo audit-path
    // formats. One line, before the run begins, naming the expected cost
    // class (cold full-corpus walk; the warm-rerun clause is honest per
    // format — see `repo_audit_path_disclosure`) — like the repo-scope
    // --base/--diff warning above, this is an advisory notice, so it is not
    // part of the --quiet-suppressed progress stream.
    if let Some(disclosure) = format.repo_audit_path_disclosure() {
        eprintln!("{disclosure}");
    }
    if matches!(format, OutputFormat::RepoExposureJson) {
        // #4945: this early return renders straight from the inventory, so it
        // brackets the walk itself with repo-scope stage boundaries instead of
        // relying on `check_with_progress` (which this path never reaches).
        app::repo_inventory_with_progress(
            progress_sink,
            || analysis::inventory_classified_seams_report_at_with_config(&input.root, &config),
            |report| {
                let ts_guidance = output::render::detect_ts_full_repo_guidance_pub(
                    &input.root,
                    &report.classified,
                );
                let python_guidance = output::render::detect_python_repo_exposure_guidance_pub(
                    &input.root,
                    &report.classified,
                );
                let generated_skip =
                    output::repo_exposure::GeneratedRustSkip::from_paths(report.skipped_generated);
                let artifact_context =
                    crate::agent::artifact::RepoExposureArtifactContext::for_repo_exposure(
                        input.root.clone(),
                        input.mode.as_str().to_string(),
                        input.base.clone(),
                        &config,
                    )?;
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                output::repo_exposure::write_repo_exposure_json_with_context(
                    &report.classified,
                    report.limit_info.as_ref(),
                    ts_guidance.as_ref(),
                    python_guidance.as_ref(),
                    generated_skip.as_ref(),
                    &artifact_context,
                    &mut handle,
                )?;
                Ok(())
            },
        )?;
        // Producer `completed` was held until the streaming stdout write
        // finished; a failed write already dropped the sink, which projected
        // `failed` instead.
        if let Some(sink) = &progress {
            sink.commit_success();
        }
        return Ok(());
    }
    // Capture diff_file before input is moved into the analysis call; the
    // RIPR-SPEC-0112 disclosure gate after the analysis needs it.
    let input_diff_file_is_some = input.diff_file.is_some();
    let limited_check_input = input.clone();
    // #4319: `--diff -` reads the diff from stdin. On an attached terminal
    // that blocks until EOF with no visible sign of why, so the cli adapter
    // discloses the read before dispatching; the analysis loader itself
    // stays silent for library callers. Only the diff-scoped pipeline path
    // consumes the stdin read — repo-scoped and seam-inventory formats
    // ignore `--diff` entirely (see the zero-findings warning below), so
    // they must not claim to be reading it.
    if !format.is_repo_scope() && !format.is_repo_seam_inventory() {
        disclose_attached_terminal_stdin_read(input.diff_file.as_deref());
    }
    let progress_scope = if format.is_repo_scope() {
        app::AnalysisProgressScope::Repo
    } else if worktree_explicitly_provided {
        app::AnalysisProgressScope::Worktree
    } else {
        app::AnalysisProgressScope::Diff
    };
    let output_result = if format.is_repo_seam_inventory() {
        // Repo seam-driven formats do not consume legacy repo `Findings`,
        // so skip `run_repo_analysis` and let `render_check` drive the
        // seam walker directly from `output.root`. The synthesized
        // `CheckOutput` carries only the fields these renderers read.
        Ok(app::repo_seam_inventory_input(input))
    } else {
        app::check_with_progress(input, &config, progress_scope, progress_sink)
    };
    let mut output = match output_result {
        Ok(output) => output,
        Err(err) => {
            if matches!(format, OutputFormat::Json)
                && let Some(rendered) = output::limited_check::render_diff_scope_limited_check_json(
                    &limited_check_input,
                    &err,
                )?
            {
                write_stdout_chunked(&rendered)?;
            }
            return Err(err);
        }
    };
    // RIPR-SPEC-0140: persist the full-fidelity finding set plus the input
    // identity for a later `explain --from` / `context --from`. A failed
    // write fails the command: the user explicitly requested the artifact.
    if let Some(path) = write_artifact.as_deref() {
        app::check_artifact::write_check_artifact(
            path,
            &limited_check_input,
            &config,
            &output.findings,
            worktree_explicitly_provided,
        )?;
    }
    // RIPR-SPEC-0083: disclose when no scope was provided and the result is empty.
    // #4012: gate on what was actually analyzed, not on what was typed. A
    // resolved default branch that analyzed changed files is a real
    // analyzed-empty result (ordinary no_behavioral_candidates), not
    // missing scope — so the producer outcome's changed_file_count is the
    // discriminator. Repo-scope formats analyze the whole repo by
    // definition, so the diff-scope disclosure never applies to them.
    let analyzed_changed_files = output
        .analysis_outcome
        .as_ref()
        .is_some_and(|outcome| outcome.counts.changed_file_count > 0);
    if !scope_explicitly_provided
        && output.findings.is_empty()
        && !analyzed_changed_files
        && !format.is_repo_scope()
    {
        output.no_scope_provided = true;
    }
    // #2425: when --diff was explicitly provided but produced zero findings
    // on a diff-scoped format, disclose on stderr why the result is empty.
    // This does NOT change the exit code or the JSON contract — stderr
    // advisory only. Repo-scoped formats (repo-exposure-json, etc.)
    // intentionally ignore --diff for their analysis scope, so the hedge is
    // gated to diff-scoped formats only. #4376(a)/#4395(c): the hedge runs
    // before stdout is rendered, so it must name the typed cause when one
    // exists instead of guessing at diff validity.
    if input_diff_file_is_some
        && output.findings.is_empty()
        && !format.is_repo_scope()
        && let Some(hedge) = zero_findings_diff_hedge(output.analysis_outcome.as_ref())
    {
        eprintln!("{hedge}");
    }
    // #2642: surface expired suppression entries as a stderr warning so they
    // are visible even in --json mode (the human output already shows them as
    // "policy warning:" lines). Expired suppressions are not applied, but a
    // stale suppressions.toml indicates the policy needs maintenance.
    if let Some(suppression) = &output.suppression
        && !suppression.warnings.is_empty()
    {
        eprintln!(
            "ripr: {} suppression policy warning(s). Run `ripr check` (human format) for details, \
             or review the suppression policy file for expired or unmatched entries.",
            suppression.warnings.len()
        );
    }
    // RIPR-SPEC-0112: the analysis sets `unanalyzed_working_tree` when a
    // committed-history diff (an explicit --base or the resolved default base;
    // both run `git diff <base>...HEAD`) read the HEAD content of tracked
    // source or test files that have uncommitted edits. Those edits were NOT
    // analyzed, so a zero-finding result must not read as a clean pass. It
    // stays off for --diff, --worktree, --candidate-tree and repo-scope
    // formats, none of which is a committed-history diff of the live tree.
    let committed_history_diff = !worktree_explicitly_provided
        && !input_diff_file_is_some
        && candidate_tree.is_none()
        && !format.is_repo_scope();
    if !committed_history_diff {
        output.unanalyzed_working_tree = false;
    }
    let navigation = if worktree_explicitly_provided && write_artifact.is_none() {
        None
    } else {
        Some(app::finding_navigation(
            &limited_check_input,
            write_artifact.as_deref(),
            explicit.mode,
        ))
    };
    // #4945: repo seam-driven formats run their walks inside the render arms,
    // so the sink threads through rendering to bracket those walks with
    // repo-scope stage boundaries; diff-scoped arms ignore it.
    write_stdout_chunked(&app::render_check_with_config_and_navigation_and_progress(
        &output,
        &format,
        &config,
        navigation.as_ref(),
        progress_sink,
    )?)?;
    if let Some(sink) = &progress {
        sink.commit_success();
    }
    Ok(())
}

/// The stderr hedge for an explicit `--diff` run that produced zero findings
/// (#2425, #2491). The first stderr line a user reads must be the true cause
/// of the empty result (#4376(a), #4395(c)):
///
/// - when the producer outcome records a language adapter that was disabled
///   by config or unavailable in this binary, name that typed cause;
/// - when the diff parsed to at least one changed file, the diff was valid
///   and the analysis outcome on stdout already explains the empty result,
///   so no diff-validity guess is printed;
/// - only when nothing parsed (or no outcome exists) print the generic
///   "may not be a valid unified diff" hint.
fn zero_findings_diff_hedge(
    outcome: Option<&crate::analysis_outcome::AnalysisOutcome>,
) -> Option<String> {
    use crate::analysis_outcome::AnalysisLimitationKind;
    let generic = "ripr: --diff produced zero findings. If the diff file is not a valid unified diff, this result is empty because nothing was parsed — not because all behavior is covered.";
    let Some(outcome) = outcome else {
        return Some(generic.to_string());
    };
    let causes = outcome
        .limitations
        .iter()
        .filter(|limitation| limitation.kind == AnalysisLimitationKind::LanguageAdapterUnavailable)
        .map(|limitation| {
            limitation
                .bounded_detail
                .clone()
                .unwrap_or_else(|| limitation.recovery.detail.trim_end_matches('.').to_string())
        })
        .collect::<Vec<_>>();
    if !causes.is_empty() {
        return Some(format!(
            "ripr: --diff produced zero findings because changed files were not analyzed: {}. \
             This empty result is not a clean pass; see the analysis outcome for the recovery.",
            causes.join("; ")
        ));
    }
    if outcome.counts.changed_file_count > 0 {
        return None;
    }
    Some(generic.to_string())
}

/// Write `text` to stdout in bounded chunks.
///
/// A single large write to a Windows console or pipe can fail with
/// `os error 87` ("the parameter is incorrect"); chunking keeps every
/// underlying write small enough to avoid that limit. Write errors are
/// returned as `Err` rather than panicking, so a failed write surfaces as
/// a normal CLI error instead of aborting the process.
fn write_stdout_chunked(text: &str) -> Result<(), String> {
    use std::io::Write;
    const CHUNK: usize = 16 * 1024;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    for chunk in text.as_bytes().chunks(CHUNK) {
        handle
            .write_all(chunk)
            .map_err(|err| format!("write to stdout failed: {err}"))?;
    }
    handle
        .flush()
        .map_err(|err| format!("flush stdout failed: {err}"))?;
    Ok(())
}

fn render_check_gap_ledger_badge(
    gap_ledger: &Path,
    format: &OutputFormat,
    config: &RiprConfig,
) -> Result<String, String> {
    let (kind, shields) = match format {
        OutputFormat::RepoBadgeJson => (output::badge::BadgeKind::Ripr, false),
        OutputFormat::RepoBadgeShields => (output::badge::BadgeKind::Ripr, true),
        OutputFormat::RepoBadgePlusJson => (output::badge::BadgeKind::RiprPlus, false),
        OutputFormat::RepoBadgePlusShields => (output::badge::BadgeKind::RiprPlus, true),
        _ => {
            return Err(
                "check --gap-ledger is only supported with repo-badge-* formats".to_string(),
            );
        }
    };
    let text = crate::bounded_input::read_to_string(gap_ledger)
        .map_err(|err| format!("failed to read gap ledger {}: {err}", gap_ledger.display()))?;
    let policy = output::badge::BadgePolicy {
        suppressions_path: config.suppressions().display_path(),
        ..output::badge::BadgePolicy::default()
    };
    let mut summary = output::badge::repo_gap_ledger_badge_summary_from_json(&text, kind, policy)?;
    output::badge::attach_public_projection(&mut summary, &gap_ledger.display().to_string());
    if shields {
        Ok(output::badge::render_shields_json(&summary))
    } else {
        Ok(output::badge::render_native_json(&summary))
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod candidate_tree_tests {
    use super::super::super::commands::check;

    fn repo_with_candidate(name: &str) -> Result<(std::path::PathBuf, String, String), String> {
        let root = std::env::temp_dir().join(format!("ripr-3278-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
        let run = |args: &[&str]| -> Result<String, String> {
            let out = crate::git::run_git_output_with_deadline(
                &root,
                args,
                Some(std::time::Duration::from_secs(30)),
            )
            .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(format!(
                    "git {} failed: {}",
                    args[0],
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        run(&["init", "--initial-branch=main"])?;
        run(&["config", "user.email", "r@e.in"])?;
        run(&["config", "user.name", "r"])?;
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn one() -> u8 { 1 }
",
        )
        .map_err(|e| e.to_string())?;
        run(&["add", "."])?;
        run(&["commit", "-qm", "base"])?;
        let base = run(&["rev-parse", "HEAD"])?;
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn one() -> u8 { 2 }
",
        )
        .map_err(|e| e.to_string())?;
        run(&["add", "."])?;
        run(&["commit", "-qm", "candidate"])?;
        let candidate = run(&["rev-parse", "HEAD"])?;
        Ok((root, base, candidate))
    }

    struct Guard(std::path::PathBuf);
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn root_args(root: &std::path::Path, extra: &[&str]) -> Vec<String> {
        let mut args = vec!["--root".to_string(), root.to_string_lossy().to_string()];
        args.extend(extra.iter().map(|a| a.to_string()));
        args
    }

    // #3278: a valid subject runs end to end through the real CLI
    // parser, and the rendered JSON carries the exact resolved identity
    // (candidate_tree equals the supplied commit's tree — argv is never
    // the source).
    #[test]
    fn candidate_tree_cli_runs_the_subject() -> Result<(), String> {
        let (root, base, candidate) = repo_with_candidate("cli-run")?;
        let _guard = Guard(root.clone());
        check(&root_args(
            &root,
            &[
                "--candidate-tree",
                &candidate,
                "--candidate-base",
                &base,
                "--format",
                "json",
            ],
        ))?;
        Ok(())
    }

    // #3278 review B1/M1: the artifact, repo-scope, and gap-ledger paths
    // must fail closed on a bound subject instead of silently dropping
    // it; --candidate-base without a tree is a named dangling input.
    #[test]
    fn candidate_tree_rejects_live_repo_and_artifact_paths() -> Result<(), String> {
        let (root, _base, candidate) = repo_with_candidate("cli-paths")?;
        let _guard = Guard(root.clone());
        let cases: [Vec<&str>; 4] = [
            vec!["--candidate-tree", &candidate, "--write-artifact", "a.json"],
            vec![
                "--candidate-tree",
                &candidate,
                "--format",
                "repo-badge-json",
            ],
            vec![
                "--candidate-tree",
                &candidate,
                "--format",
                "repo-exposure-json",
            ],
            vec!["--candidate-base", "main"],
        ];
        let expected = [
            "cannot be combined with --candidate-tree",
            "cannot be combined with the live-repository path",
            "cannot be combined with the live-repository path",
            "--candidate-base requires --candidate-tree",
        ];
        for (extra, want) in cases.iter().zip(expected.iter()) {
            let error = check(&root_args(&root, extra)).err().unwrap_or_default();
            assert!(error.contains(want), "expected `{want}` in: {error}");
        }
        Ok(())
    }

    // #3278: the conflicting-input controls fail before analysis.
    #[test]
    fn candidate_tree_rejects_conflicting_inputs() -> Result<(), String> {
        let (root, _base, candidate) = repo_with_candidate("cli-conflict")?;
        let _guard = Guard(root.clone());
        let cases: [Vec<&str>; 3] = [
            vec!["--candidate-tree", &candidate, "--diff", "x.diff"],
            vec!["--candidate-tree", &candidate, "--base", "main"],
            vec!["--candidate-tree", "deadbeef"],
        ];
        for extra in cases {
            let error = check(&root_args(&root, &extra)).err().unwrap_or_default();
            assert!(
                error.contains("git candidate subject"),
                "must fail closed naming the subject: {error}"
            );
        }
        Ok(())
    }

    // #3278: the app layer projects the exact resolved identity —
    // subject_kind, both trees, and a sha256 diff identity — into the
    // check JSON the CLI renders.
    #[test]
    fn candidate_tree_json_carries_exact_subject_identity() -> Result<(), String> {
        let (root, base, candidate) = repo_with_candidate("cli-json")?;
        let _guard = Guard(root.clone());
        let subject = crate::domain::GitCandidateSubject::new(
            &root,
            crate::domain::GitCandidateBase::Treeish(
                crate::domain::GitTreeish::new(&base).map_err(|e| e.to_string())?,
            ),
            crate::domain::GitObjectId::parse(&candidate).map_err(|e| e.to_string())?,
        );
        let input = crate::app::CheckInput {
            root: root.clone(),
            base: None,
            diff_file: None,
            git_candidate: Some(subject),
            ..crate::app::CheckInput::default()
        };
        let output = crate::app::check_workspace(input)?;
        let outcome = output
            .analysis_outcome
            .as_ref()
            .ok_or("candidate run must carry an analysis outcome")?;
        let identity = outcome
            .identity
            .git_candidate_subject
            .as_ref()
            .ok_or("git_candidate_subject missing from identity")?;
        assert_eq!(identity.subject_kind, "tree_to_tree");
        assert!(!identity.base_tree.is_empty());
        assert!(!identity.candidate_tree.is_empty());
        assert!(identity.diff_identity.starts_with("sha256:"));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{
        args, copy_sample_workspace_to_temp, repo_root, unique_command_test_dir,
        unique_repo_relative_test_dir,
    };
    use super::*;

    /// Run the real diff pipeline over the sample workspace's valid Rust diff
    /// with the given effective language set, returning the producer outcome
    /// the zero-findings hedge consumes.
    fn sample_diff_outcome(
        label: &str,
        enabled: Vec<crate::domain::LanguageId>,
    ) -> Result<(usize, crate::analysis_outcome::AnalysisOutcome), String> {
        let root = copy_sample_workspace_to_temp(label)?;
        let mut config = RiprConfig::default();
        config.languages.enabled = enabled;
        let input = CheckInput {
            root: root.clone(),
            diff_file: Some(root.join("example.diff")),
            ..CheckInput::default()
        };
        let result = app::check_workspace_with_config(input, &config);
        if let Ok(()) = std::fs::remove_dir_all(&root) {}
        let output = result?;
        let outcome = output
            .analysis_outcome
            .ok_or_else(|| "diff pipeline must project an analysis outcome".to_string())?;
        Ok((output.findings.len(), outcome))
    }

    #[test]
    fn zero_findings_hedge_names_config_excluded_rust_instead_of_diff_validity()
    -> Result<(), String> {
        // #4376(a): `[languages] enabled = ["typescript"]` over a valid Rust
        // diff. The hedge must name the configuration cause, never the
        // "may not be a valid unified diff" guess.
        use crate::domain::LanguageId;
        for (label, enabled) in [
            ("hedge-rust-excluded-ts", vec![LanguageId::TypeScript]),
            ("hedge-rust-excluded-empty", Vec::new()),
        ] {
            let (findings, outcome) = sample_diff_outcome(label, enabled)?;
            assert_eq!(
                findings, 0,
                "fixture precondition: {label} must analyze nothing"
            );
            assert!(
                outcome.counts.changed_file_count > 0,
                "fixture precondition: the valid sample diff must parse"
            );
            assert_eq!(
                outcome.kind,
                crate::analysis_outcome::AnalysisOutcomeKind::PartialWithLimitations,
                "an excluded Rust adapter must not claim a complete analysis ({label})"
            );
            let hedge = zero_findings_diff_hedge(Some(&outcome))
                .ok_or_else(|| format!("{label}: a typed exclusion must be named on stderr"))?;
            assert!(
                hedge.contains("rust is not in the effective [languages].enabled set"),
                "{label}: {hedge}"
            );
            assert!(
                !hedge.contains("not a valid unified diff"),
                "{label}: {hedge}"
            );
        }
        Ok(())
    }

    #[test]
    fn zero_findings_hedge_names_unavailable_adapter_before_diff_validity() -> Result<(), String> {
        // #4395(c): a valid diff whose only changed file needs an adapter
        // that is not available (Perl in a default build) must lead with
        // the availability cause.
        use crate::analysis_outcome::{
            AnalysisIdentity, AnalysisLimitation, AnalysisLimitationKind, AnalysisOutcome,
            AnalysisOutcomeCounts, AnalysisOutcomeKind, AnalysisRecovery, AnalysisRecoveryKind,
            AnalysisStage,
        };
        let outcome = AnalysisOutcome::new(
            AnalysisOutcomeKind::PartialWithLimitations,
            AnalysisIdentity::default(),
            AnalysisOutcomeCounts {
                changed_file_count: 1,
                changed_line_count: 1,
                ..AnalysisOutcomeCounts::default()
            },
            vec![
                AnalysisLimitation::new(
                    AnalysisLimitationKind::LanguageAdapterUnavailable,
                    AnalysisStage::LanguageAdapter,
                    AnalysisRecovery::new(
                        AnalysisRecoveryKind::EnableLanguage,
                        "Use a ripr binary built with Cargo feature `lang-perl`.",
                    )?,
                )
                .with_detail(
                    "perl changed 1 file(s), but the preview adapter was not enabled or available",
                )?,
            ],
        )?;
        let hedge = zero_findings_diff_hedge(Some(&outcome))
            .ok_or_else(|| "an unavailable adapter must be named on stderr".to_string())?;
        assert!(hedge.contains("perl changed 1 file(s)"), "{hedge}");
        assert!(!hedge.contains("not a valid unified diff"), "{hedge}");
        Ok(())
    }

    #[test]
    fn zero_findings_hedge_is_silent_for_a_parsed_diff_and_generic_when_nothing_parsed()
    -> Result<(), String> {
        // A parsed Rust diff with Rust enabled: the diff was valid, so no
        // diff-validity guess is printed (the outcome on stdout explains).
        let (_, outcome) =
            sample_diff_outcome("hedge-rust-enabled", vec![crate::domain::LanguageId::Rust])?;
        assert!(outcome.counts.changed_file_count > 0);
        assert!(
            zero_findings_diff_hedge(Some(&outcome)).is_none(),
            "a parsed diff must not be blamed for diff validity"
        );
        // Nothing parsed (or no outcome): the generic hint remains.
        let empty = crate::analysis_outcome::AnalysisOutcome::new(
            crate::analysis_outcome::AnalysisOutcomeKind::NoScope,
            crate::analysis_outcome::AnalysisIdentity::default(),
            crate::analysis_outcome::AnalysisOutcomeCounts::default(),
            Vec::new(),
        )?;
        for outcome in [Some(&empty), None] {
            let hedge = zero_findings_diff_hedge(outcome)
                .ok_or_else(|| "an unparsed diff must keep the generic hint".to_string())?;
            assert!(hedge.contains("not a valid unified diff"), "{hedge}");
        }
        Ok(())
    }

    #[test]
    fn repo_scope_format_with_base_emits_scope_warning() -> Result<(), String> {
        let warning = repo_scope_diff_bound_warning(OutputFormat::RepoExposureJson, true, None)
            .ok_or_else(|| "repo-scoped format plus --base should warn".to_string())?;

        assert!(warning.contains("format repo-exposure-json is repo-scoped"));
        assert!(warning.contains("--base/--diff does not bound it"));
        assert!(warning.contains("--format json"));
        assert!(warning.contains("--format repo-exposure-summary-json"));
        Ok(())
    }

    #[test]
    fn repo_scope_format_with_diff_emits_scope_warning() -> Result<(), String> {
        let warning = repo_scope_diff_bound_warning(
            OutputFormat::RepoSarif,
            false,
            Some(Path::new("changes.diff")),
        )
        .ok_or_else(|| "repo-scoped format plus --diff should warn".to_string())?;

        assert!(warning.contains("format repo-sarif is repo-scoped"));
        assert!(warning.contains("--base/--diff does not bound it"));
        Ok(())
    }

    #[test]
    fn diff_json_with_base_does_not_emit_repo_scope_warning() {
        let warning = repo_scope_diff_bound_warning(OutputFormat::Json, true, None);

        assert!(warning.is_none());
    }

    #[test]
    fn implicit_workspace_root_walks_up_to_a_workspace_manifest() -> Result<(), String> {
        let root = unique_repo_relative_test_dir("workspace-root-walk");
        let nested = root.join("crates/member/src");
        std::fs::create_dir_all(&nested).map_err(|error| error.to_string())?;
        std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n")
            .map_err(|error| error.to_string())?;

        let resolved = resolve_workspace_root(&nested)?
            .ok_or_else(|| "expected a workspace root from the nested path".to_string())?;
        let expected = std::fs::canonicalize(&root).map_err(|error| error.to_string())?;
        let mismatch = (resolved != expected).then(|| {
            format!(
                "resolved workspace root {} differs from expected {}",
                resolved.display(),
                expected.display()
            )
        });
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        if let Some(error) = mismatch {
            return Err(error);
        }
        Ok(())
    }

    #[test]
    fn workspace_root_walk_ignores_a_package_only_manifest() -> Result<(), String> {
        let workspace = std::fs::canonicalize(repo_root()).map_err(|error| error.to_string())?;
        let candidate = unique_command_test_dir("workspace-root-package");
        let parent = workspace
            .parent()
            .ok_or_else(|| "workspace root has no parent".to_string())?;
        let name = candidate
            .file_name()
            .ok_or_else(|| "temporary fixture has no file name".to_string())?;
        let root = parent.join(name);
        let nested = root.join("src");
        std::fs::create_dir_all(&nested).map_err(|error| error.to_string())?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"package-only\"\nversion = \"0.1.0\"\n",
        )
        .map_err(|error| error.to_string())?;

        let resolved = resolve_workspace_root(&nested)?;
        let found_workspace = resolved.is_some();
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        if found_workspace {
            return Err("package-only manifest must not be treated as a workspace".to_string());
        }
        Ok(())
    }

    fn outside_workspace_fixture(label: &str) -> Result<PathBuf, String> {
        let workspace = std::fs::canonicalize(repo_root()).map_err(|error| error.to_string())?;
        let candidate = unique_command_test_dir(label);
        let parent = workspace
            .parent()
            .ok_or_else(|| "workspace root has no parent".to_string())?;
        let name = candidate
            .file_name()
            .ok_or_else(|| "temporary fixture has no file name".to_string())?;
        Ok(parent.join(name))
    }

    #[test]
    fn project_root_walk_reaches_a_js_or_python_workspace_from_a_package() -> Result<(), String> {
        let cases: [(&str, &str, &str, ImplicitRootReason); 3] = [
            (
                "pnpm-workspace.yaml",
                "packages:\n  - 'packages/*'\n",
                "packages/utils/src",
                ImplicitRootReason::PnpmWorkspace,
            ),
            (
                "package.json",
                r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#,
                "packages/utils/src",
                ImplicitRootReason::PackageJsonWorkspaces,
            ),
            (
                "pyproject.toml",
                "[tool.uv.workspace]\nmembers = [\"packages/*\"]\n",
                "packages/core/src/core",
                ImplicitRootReason::UvWorkspace,
            ),
        ];
        for (index, (manifest, contents, nested, marker)) in cases.into_iter().enumerate() {
            let root = outside_workspace_fixture(&format!("non-cargo-walk-{index}"))?;
            let repo = root.join("repo");
            let nested = repo.join(nested);
            std::fs::create_dir_all(&nested).map_err(|error| error.to_string())?;
            std::fs::create_dir_all(repo.join(".git")).map_err(|error| error.to_string())?;
            std::fs::write(repo.join(manifest), contents).map_err(|error| error.to_string())?;
            // A package-level manifest between the start and the workspace
            // root must not stop the walk.
            let package = nested
                .ancestors()
                .find(|dir| {
                    dir.parent()
                        .is_some_and(|parent| parent.ends_with("packages"))
                })
                .map(Path::to_path_buf)
                .ok_or_else(|| "fixture has no package directory".to_string())?;
            std::fs::write(package.join("package.json"), r#"{"name":"member"}"#)
                .map_err(|error| error.to_string())?;
            std::fs::write(
                package.join("pyproject.toml"),
                "[project]\nname = \"member\"\n",
            )
            .map_err(|error| error.to_string())?;

            let resolved = resolve_project_root(&nested);
            let expected = std::fs::canonicalize(&repo).map_err(|error| error.to_string());
            std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
            assert_eq!(resolved?, Some((expected?, marker)), "{manifest}");
        }
        Ok(())
    }

    #[test]
    fn project_root_walk_reaches_a_package_manifest_from_its_source_dir() -> Result<(), String> {
        let root = outside_workspace_fixture("project-root-package")?;
        let nested = root.join("src/inner");
        std::fs::create_dir_all(&nested).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(root.join(".git")).map_err(|error| error.to_string())?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"package-only\"\nversion = \"0.1.0\"\n",
        )
        .map_err(|error| error.to_string())?;

        let resolved = resolve_project_root(&nested);
        let expected = std::fs::canonicalize(&root).map_err(|error| error.to_string());
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        assert_eq!(
            resolved?,
            Some((expected?, ImplicitRootReason::Package)),
            "a package-only crate run from src/ must analyze the crate root"
        );
        Ok(())
    }

    #[test]
    fn project_root_walk_ignores_markers_above_the_git_top_level() -> Result<(), String> {
        let root = outside_workspace_fixture("non-cargo-walk-bounded")?;
        let repo = root.join("repo");
        let nested = repo.join("packages/utils");
        std::fs::create_dir_all(&nested).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(repo.join(".git")).map_err(|error| error.to_string())?;
        // A workspace marker above the work tree belongs to something else.
        std::fs::write(root.join("pnpm-workspace.yaml"), "packages: []\n")
            .map_err(|error| error.to_string())?;
        // A package.json without a workspaces field is not a marker.
        std::fs::write(repo.join("package.json"), r#"{"name":"root"}"#)
            .map_err(|error| error.to_string())?;

        let resolved = resolve_project_root(&nested);
        let expected = std::fs::canonicalize(&repo).map_err(|error| error.to_string());
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        assert_eq!(
            resolved?,
            Some((expected?, ImplicitRootReason::GitTopLevel))
        );
        Ok(())
    }

    #[test]
    fn project_root_walk_stops_at_the_git_top_level() -> Result<(), String> {
        let outer = outside_workspace_fixture("project-root-git-stop")?;
        let repo = outer.join("repo");
        let nested = repo.join("web/src");
        std::fs::create_dir_all(&nested).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(repo.join(".git")).map_err(|error| error.to_string())?;
        // A manifest outside the repository must never be adopted.
        std::fs::write(
            outer.join("Cargo.toml"),
            "[package]\nname = \"outside\"\nversion = \"0.1.0\"\n",
        )
        .map_err(|error| error.to_string())?;

        let resolved = resolve_project_root(&nested);
        let expected = std::fs::canonicalize(&repo).map_err(|error| error.to_string());
        std::fs::remove_dir_all(&outer).map_err(|error| error.to_string())?;
        assert_eq!(
            resolved?,
            Some((expected?, ImplicitRootReason::GitTopLevel)),
            "the walk must stop at the git top level"
        );
        Ok(())
    }

    #[test]
    fn enclosing_workspace_does_not_claim_a_nested_repository() -> Result<(), String> {
        let outer = outside_workspace_fixture("project-root-nested-repo")?;
        let inner = outer.join("vendor/tool");
        let nested = inner.join("src");
        std::fs::create_dir_all(&nested).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(inner.join(".git")).map_err(|error| error.to_string())?;
        std::fs::write(outer.join("Cargo.toml"), "[workspace]\nmembers = []\n")
            .map_err(|error| error.to_string())?;
        std::fs::write(
            inner.join("Cargo.toml"),
            "[package]\nname = \"tool\"\nversion = \"0.1.0\"\n",
        )
        .map_err(|error| error.to_string())?;

        let resolved = resolve_project_root(&nested);
        let expected = std::fs::canonicalize(&inner).map_err(|error| error.to_string());
        std::fs::remove_dir_all(&outer).map_err(|error| error.to_string())?;
        assert_eq!(
            resolved?,
            Some((expected?, ImplicitRootReason::Package)),
            "an enclosing workspace must not cross the nested repository's git boundary"
        );
        Ok(())
    }

    #[test]
    fn git_timeout_cli_values_are_parsed_before_dispatch() -> Result<(), String> {
        assert_eq!(check(&args(&["--git-timeout", "0", "--help"])), Ok(()));
        assert_eq!(check(&args(&["--git-timeout", "12", "--help"])), Ok(()));

        let error = check(&args(&["--git-timeout", "not-a-number"]))
            .err()
            .ok_or("invalid git timeout should fail closed")?;
        assert!(error.contains("--git-timeout requires a non-negative integer"));
        Ok(())
    }

    #[test]
    fn quiet_is_accepted_without_running_analysis() {
        assert_eq!(check(&args(&["--quiet", "--help"])), Ok(()));
    }

    #[test]
    fn git_timeout_environment_is_a_fallback_and_zero_disables() -> Result<(), String> {
        assert_eq!(
            git_timeout_from_env(false, Ok("12".to_string())),
            Ok(Some(Some(std::time::Duration::from_secs(12))))
        );
        assert_eq!(
            git_timeout_from_env(false, Ok("0".to_string())),
            Ok(Some(None))
        );
        assert_eq!(
            git_timeout_from_env(false, Err(std::env::VarError::NotPresent)),
            Ok(None)
        );
        assert_eq!(git_timeout_from_env(true, Ok("12".to_string())), Ok(None));
        // An explicit --git-timeout wins, so a bad env value is not read.
        assert_eq!(
            git_timeout_from_env(true, Ok("invalid".to_string())),
            Ok(None)
        );
        let error = git_timeout_from_env(false, Ok("18446744073709551615".to_string()))
            .err()
            .ok_or("an overflowing timeout should fail closed")?;
        assert!(error.contains("RIPR_GIT_TIMEOUT is too large"), "{error}");
        // #4374: non-numeric and beyond-u64 values fail closed naming the
        // variable instead of silently keeping the default deadline.
        for value in ["invalid", "99999999999999999999", "-1", ""] {
            let error = git_timeout_from_env(false, Ok(value.to_string()))
                .err()
                .ok_or(format!("RIPR_GIT_TIMEOUT={value:?} should fail closed"))?;
            assert_eq!(
                error,
                format!(
                    "RIPR_GIT_TIMEOUT requires a non-negative integer (seconds); got {value:?}"
                )
            );
        }
        assert_eq!(
            git_timeout_from_env(
                false,
                Err(std::env::VarError::NotUnicode(std::ffi::OsString::from(
                    "x"
                )))
            ),
            Err("RIPR_GIT_TIMEOUT must be valid UTF-8".to_string())
        );
        Ok(())
    }

    #[test]
    fn check_requires_values_for_value_flags() {
        assert_eq!(
            check(&args(&["--diff"])),
            Err("missing value for --diff".to_string())
        );
        assert_eq!(
            check(&args(&["--mode"])),
            Err("missing value for --mode".to_string())
        );
    }

    #[test]
    fn check_repo_exposure_json_streams_output() -> Result<(), String> {
        let root = copy_sample_workspace_to_temp("repo-exposure-json")?;
        let root_arg = root.to_string_lossy().into_owned();
        assert_eq!(
            check(&[
                "--root".to_string(),
                root_arg,
                "--format".to_string(),
                "repo-exposure-json".to_string()
            ]),
            Ok(())
        );
        std::fs::remove_dir_all(root)
            .map_err(|err| format!("failed to remove temp sample workspace: {err}"))?;
        Ok(())
    }

    #[test]
    fn check_json_returns_limited_artifact_error_for_oversized_diff() -> Result<(), String> {
        let root = unique_repo_relative_test_dir("oversized-diff");
        let diff = root.join("oversized.diff");
        std::fs::create_dir_all(&root)
            .map_err(|err| format!("failed to create oversized diff root: {err}"))?;
        std::fs::write(&diff, oversized_rust_diff(2001))
            .map_err(|err| format!("failed to write oversized diff: {err}"))?;
        let root_arg = root.to_string_lossy().into_owned();
        let diff_arg = diff.to_string_lossy().into_owned();

        let result = check(&[
            "--root".to_string(),
            root_arg,
            "--diff".to_string(),
            diff_arg,
            "--json".to_string(),
        ]);

        let cleanup = std::fs::remove_dir_all(&root)
            .map_err(|err| format!("failed to remove oversized diff root: {err}"));
        assert!(
            matches!(result, Err(ref message) if message.contains("diff_scope_oversized")),
            "expected diff_scope_oversized error, got {result:?}"
        );
        cleanup
    }

    fn oversized_rust_diff(changed_lines: usize) -> String {
        let mut diff = format!(
            "diff --git a/src/lib.rs b/src/lib.rs\n\
             index 0000000..1111111 100644\n\
             --- a/src/lib.rs\n\
             +++ b/src/lib.rs\n\
             @@ -0,0 +1,{changed_lines} @@\n",
        );
        for index in 0..changed_lines {
            diff.push_str(&format!(
                "+pub fn generated_{index}() -> usize {{ {index} }}\n"
            ));
        }
        diff
    }

    #[test]
    fn check_rejects_unknown_argument() {
        assert_eq!(
            check(&args(&["--wat"])),
            Err("unknown check argument \"--wat\". Run `ripr check --help`.".to_string())
        );
    }

    #[test]
    fn check_rejects_diff_file_plus_worktree_mode() -> Result<(), String> {
        let result = check(&args(&["--diff", "change.patch", "--worktree"]));
        match result {
            Err(message) if message == "check --worktree cannot be combined with --diff" => Ok(()),
            other => Err(format!(
                "expected --diff plus --worktree rejection, got {other:?}"
            )),
        }
    }

    #[test]
    fn check_requires_values_for_all_value_flags() {
        assert_eq!(
            check(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            check(&args(&["--base"])),
            Err("missing value for --base".to_string())
        );
        assert_eq!(
            check(&args(&["--format"])),
            Err("missing value for --format".to_string())
        );
    }
}
