//! Arg-parsing and dispatch for `ripr check`.
//!
//! This is the CLI adapter layer only. Analysis, evaluation, and rendering
//! semantics live in `crate::app`, `crate::analysis`, and `crate::output`.
//! This module owns argv parsing, output destination selection, and exit
//! mapping for the check command family.

use crate::analysis;
use crate::app::{self, CheckInput, OutputFormat};
use crate::cli::help;
use crate::cli::parse::{expect_value, parse_format, parse_mode};
use crate::cli::suggest::unknown_argument;
use crate::config::{CheckInputExplicit, RiprConfig, apply_to_check_input, load_for_root};
use crate::output;
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

fn parse_git_timeout(value: &str) -> Result<Option<std::time::Duration>, String> {
    let secs: u64 = value.parse().map_err(|_parse_err| {
        format!("--git-timeout requires a non-negative integer (seconds); got {value:?}")
    })?;
    validate_git_timeout_secs(secs)
}

fn validate_git_timeout_secs(secs: u64) -> Result<Option<std::time::Duration>, String> {
    let timeout = std::time::Duration::from_secs(secs);
    if std::time::Instant::now().checked_add(timeout).is_none() {
        return Err(format!(
            "--git-timeout is too large for the platform deadline; got {secs} seconds"
        ));
    }
    Ok((secs > 0).then_some(timeout))
}

fn git_timeout_from_env(
    explicit: bool,
    env_value: Option<&str>,
) -> Result<Option<Option<std::time::Duration>>, String> {
    if explicit {
        return Ok(None);
    }
    let Some(value) = env_value else {
        return Ok(None);
    };
    let Ok(secs) = value.parse::<u64>() else {
        return Ok(None);
    };
    validate_git_timeout_secs(secs).map(Some)
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
    // RIPR-SPEC-0140: explicit artifact sink for the explain/context reuse
    // pair. No implicit cache: the user names the artifact path.
    let mut write_artifact: Option<PathBuf> = None;
    let mut git_timeout_explicitly_provided = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                input.root = PathBuf::from(expect_value(args, i, "--root")?);
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
            "--help" | "-h" => {
                help::print_check_help();
                return Ok(());
            }
            other => return Err(unknown_argument("check", other)),
        }
        i += 1;
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
        std::env::var("RIPR_GIT_TIMEOUT").ok().as_deref(),
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
    let config = load_for_root(&input.root)?;
    apply_to_check_input(&mut input, &config, explicit);
    let format = input.format;
    // RIPR-SPEC-0140: --write-artifact records a diff-scoped findings run.
    // Repo-scoped and gap-ledger paths produce no such finding set, so both
    // fail closed with a named limitation rather than silently skipping the
    // requested artifact. --worktree runs record the base-to-worktree diff
    // source, which is re-resolvable at reuse time.
    if let Some(path) = write_artifact.as_ref() {
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
    if let Some(warning) =
        repo_scope_diff_bound_warning(format, base_explicitly_provided, input.diff_file.as_deref())
    {
        eprintln!("{warning}");
    }
    if let Some(gap_ledger) = gap_ledger.as_ref() {
        write_stdout_chunked(&render_check_gap_ledger_badge(
            gap_ledger, &format, &config,
        )?)?;
        return Ok(());
    }
    if matches!(format, OutputFormat::RepoExposureJson) {
        let (classified, limit_info) =
            analysis::inventory_classified_seams_at_with_config(&input.root, &config)?;
        let ts_guidance =
            output::render::detect_ts_full_repo_guidance_pub(&input.root, &classified);
        let artifact_context =
            crate::agent::artifact::RepoExposureArtifactContext::for_repo_exposure(
                input.root.clone(),
                input.mode.as_str().to_string(),
                input.base.clone(),
            )?;
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        output::repo_exposure::write_repo_exposure_json_with_context(
            &classified,
            limit_info.as_ref(),
            ts_guidance.as_ref(),
            &artifact_context,
            &mut handle,
        )?;
        return Ok(());
    }
    // Capture root and diff_file before input is moved into the analysis call.
    // These are needed for the RIPR-SPEC-0112 disclosure check after the analysis.
    let input_root = input.root.clone();
    let input_diff_file_is_some = input.diff_file.is_some();
    let limited_check_input = input.clone();
    let output_result = if format.is_repo_seam_inventory() {
        // Repo seam-driven formats do not consume legacy repo `Findings`,
        // so skip `run_repo_analysis` and let `render_check` drive the
        // seam walker directly from `output.root`. The synthesized
        // `CheckOutput` carries only the fields these renderers read.
        Ok(app::repo_seam_inventory_input(input))
    } else if format.is_repo_scope() {
        app::check_workspace_repo_with_config(input, &config)
    } else if worktree_explicitly_provided {
        app::check_workspace_worktree_with_config(input, &config)
    } else {
        app::check_workspace_with_config(input, &config)
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
    // The guidance fires only when scope was NOT explicitly provided — it must
    // NOT fire when --diff/--base/--mode produced a real analyzed-empty result.
    if !scope_explicitly_provided && output.findings.is_empty() {
        output.no_scope_provided = true;
    }
    // #2425: when --diff was explicitly provided but produced zero findings
    // on a diff-scoped format, warn on stderr that the diff may be malformed.
    // A non-diff file (log, source, random text) produces zero parsed files
    // silently, which can be mistaken for a clean bill of health. This does
    // NOT change the exit code or the JSON contract — stderr advisory only.
    // Repo-scoped formats (repo-exposure-json, etc.) intentionally ignore
    // --diff for their analysis scope, so the warning is gated to diff-scoped
    // formats only.
    if input_diff_file_is_some && output.findings.is_empty() && !format.is_repo_scope() {
        eprintln!(
            "ripr: --diff produced zero findings. If the diff file is not a valid unified diff, this result is empty because nothing was parsed — not because all behavior is covered."
        );
    }
    // RIPR-SPEC-0112: disclose when --base was explicitly provided (committed-history
    // diff) AND the working tree has uncommitted changes to tracked source files.
    // Those changes were NOT analyzed. A zero-finding result in this state must NOT
    // be read as a clean pass — the user's uncommitted edits were excluded from the diff.
    // Fires independent of findings.is_empty() (honest whether or not committed diff
    // had findings), but the false-clean risk is highest when findings are empty.
    // Does NOT fire when --diff was used (file-based diff; no live worktree scope).
    if base_explicitly_provided
        && !worktree_explicitly_provided
        && !input_diff_file_is_some
        && analysis::working_tree_has_tracked_changes(&input_root)
    {
        output.unanalyzed_working_tree = true;
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
    write_stdout_chunked(&app::render_check_with_config_and_navigation(
        &output,
        &format,
        &config,
        navigation.as_ref(),
    )?)?;
    Ok(())
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
    let text = std::fs::read_to_string(gap_ledger)
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
mod tests {
    use super::super::tests::{args, copy_sample_workspace_to_temp, unique_repo_relative_test_dir};
    use super::*;

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
    fn git_timeout_environment_is_a_fallback_and_zero_disables() -> Result<(), String> {
        assert_eq!(
            git_timeout_from_env(false, Some("12")),
            Ok(Some(Some(std::time::Duration::from_secs(12))))
        );
        assert_eq!(git_timeout_from_env(false, Some("0")), Ok(Some(None)));
        assert_eq!(git_timeout_from_env(false, Some("invalid")), Ok(None));
        assert_eq!(git_timeout_from_env(false, None), Ok(None));
        assert_eq!(git_timeout_from_env(true, Some("12")), Ok(None));
        let error = git_timeout_from_env(false, Some("18446744073709551615"))
            .err()
            .ok_or("an overflowing timeout should fail closed")?;
        assert!(error.contains("too large"));
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
