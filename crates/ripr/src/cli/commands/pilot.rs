use crate::analysis;
use crate::app::{self, CheckInput, Mode, OutputFormat};
use crate::cli::commands_numeric::{parse_positive_u64, parse_positive_usize};
use crate::cli::commands_options::PilotOptions;
use crate::cli::help;
use crate::cli::parse::{expect_value, parse_mode};
use crate::cli::progress::{CliProgressSink, ProgressPolicy};
use crate::cli::suggest::unknown_argument;
use crate::config::{CheckInputExplicit, RiprConfig, apply_to_check_input, load_for_root};
use crate::output;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

const DEFAULT_PILOT_TIMEOUT_MS: u64 = 30_000;

/// Budget for the auto-retry when the default timeout fires (#2424).
/// A cold fact cache on a multi-crate workspace can need ~155s; the
/// default 30s is too low for first-run. The retry budget is set high
/// enough to cover a cold cache on the ripr-swarm repo itself.
const PILOT_RETRY_TIMEOUT_MS: u64 = 240_000;

/// Write pilot's `repo-exposure.json`.
///
/// When pilot saw the same seam population `ripr check --format
/// repo-exposure-json` would, the snapshot carries the same producer-owned
/// `artifact` identity, so it can be the `--before` of `ripr agent verify`
/// that the first-PR workflow and the evidence records name (#3906). When the
/// pilot seam budget truncated the population, the snapshot is written
/// without that identity: a stamped partial snapshot would pass verify's
/// comparability check against a full after snapshot and compare two
/// different populations. Verify then refuses it rather than misreporting.
fn write_pilot_repo_exposure_json(
    path: &Path,
    input: &CheckInput,
    config: &RiprConfig,
    classified: &[analysis::ClassifiedSeam],
    limit_info: Option<&analysis::SeamLimitInfo>,
    generated_skip: Option<&output::repo_exposure::GeneratedRustSkip>,
    pilot_budget_truncated: bool,
) -> Result<(), String> {
    let ts_guidance = output::render::detect_ts_full_repo_guidance_pub(&input.root, classified);
    let python_guidance =
        output::render::detect_python_repo_exposure_guidance_pub(&input.root, classified);
    let write_failed = |err: String| format!("write {} failed: {err}", path.display());
    if pilot_budget_truncated {
        return write_pilot_file(
            path,
            output::repo_exposure::render_repo_exposure_json_with_generated_skip(
                classified,
                limit_info,
                ts_guidance.as_ref(),
                python_guidance.as_ref(),
                generated_skip,
            ),
        );
    }
    // Base `None`: pilot's printed after-snapshot command passes no `--base`
    // or `--diff`, so both snapshots intentionally carry no base under
    // RIPR-SPEC-0084. Keep this explicit rather than coupling artifact
    // identity to the `CheckInput` default.
    let context = crate::agent::artifact::RepoExposureArtifactContext::for_repo_exposure(
        input.root.clone(),
        input.mode.as_str().to_string(),
        None,
        config,
    )?;
    let mut render_error = None;
    output::file_write::write_with(path, |file| {
        let mut writer = std::io::BufWriter::new(file);
        if let Err(err) = output::repo_exposure::write_repo_exposure_json_with_context(
            classified,
            limit_info,
            ts_guidance.as_ref(),
            python_guidance.as_ref(),
            generated_skip,
            &context,
            &mut writer,
        ) {
            render_error = Some(err);
            return Err(std::io::Error::other("repo exposure rendering failed"));
        }
        std::io::Write::flush(&mut writer)
    })
    .map_err(|err| write_failed(render_error.take().unwrap_or_else(|| err.to_string())))
}

pub(in crate::cli) fn pilot(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_pilot_help();
        return Ok(());
    }

    let options = parse_pilot_options(args)?;
    if !options.root.is_dir() {
        return Err(format!(
            "pilot root {} is not a directory",
            options.root.display()
        ));
    }

    let config = load_for_root(&options.root)?;
    // Refuse an invalid RIPR_PILOT_SEAM_BUDGET (#4529) before the analysis
    // it would bound, not after it.
    analysis::pilot_seam_budget()?;
    let mut input = CheckInput {
        root: options.root.clone(),
        mode: options.mode.clone(),
        ..CheckInput::default()
    };
    apply_to_check_input(&mut input, &config, options.explicit);

    let artifacts = pilot_artifacts(&options.out_dir);
    output::file_write::create_output_dir(&options.out_dir, "--out")?;

    // #5019: pilot's repo inventory is the same multi-minute walk `ripr
    // check` projects progress for, so route it through the shared
    // app-layer progress-bearing entry point instead of calling the
    // analyzer directly.
    //
    // When the default deadline fires and the user did not pass an explicit
    // --timeout-ms (#2424), the deadline extends on the same run instead of
    // cancelling it and starting over. A cold fact cache on a multi-crate
    // workspace can need minutes, and a cancelled first attempt leaves
    // nothing the restart can reuse, so restarting redid the first 30s of
    // work (serde cold: 75s with the restart, 44s without, identical
    // output).
    let extension_ms = pilot_deadline_extension_ms(&options);
    let progress = pilot_progress_sink(options.quiet);
    let analysis_result = run_pilot_analysis_with_timeout(
        options.timeout_ms,
        extension_ms,
        || {
            eprintln!(
                "ripr: pilot still running after {}ms; extending the deadline by {}ms (a cold cache needs more time)...",
                DEFAULT_PILOT_TIMEOUT_MS, PILOT_RETRY_TIMEOUT_MS
            );
        },
        {
            let root = input.root.clone();
            let cfg = config.clone();
            let sink = progress.as_ref().map(Arc::clone);
            move || run_pilot_inventory(&root, &cfg, sink.as_ref())
        },
    )?;
    // The timeout hint scales from the budget actually spent, so an
    // extended run does not suggest a smaller --timeout-ms than it used.
    let spent_timeout_ms = options.timeout_ms.saturating_add(extension_ms.unwrap_or(0));
    let PilotAnalysisResult::Complete(report) = analysis_result else {
        let context = output::pilot::PilotSummaryContext {
            root: &input.root,
            mode: &input.mode,
            config_path: config.source_path(),
            max_seams: options.max_seams,
            timeout_ms: spent_timeout_ms,
            artifacts: &artifacts,
            python_first_use: None,
            language_routes: None,
        };
        write_pilot_file(
            &artifacts.pilot_summary_json,
            output::pilot::render_pilot_timeout_summary_json(context),
        )?;
        write_pilot_file(
            &artifacts.pilot_summary_md,
            output::pilot::render_pilot_timeout_summary_md(context),
        )?;
        print!("{}", output::pilot::render_pilot_timeout_terminal(context));
        return Ok(());
    };

    // Apply the pilot artifact seam budget.  The inventory may already have
    // been capped by the repo-exposure seam limit; we then further cap the
    // classified slice for the two pilot artifacts so they stay under a
    // manageable size.  `limit_info` carries whichever cap fired (pilot
    // budget wins when both fire; inventory limit is the outer bound).
    let mut classified = report.classified;
    let inventory_limit_info = report.limit_info;
    let generated_skip = output::repo_exposure::GeneratedRustSkip::from_paths(
        report.skipped_generated,
        report.naming_only_skips,
    );
    let pilot_budget_info = analysis::apply_pilot_seam_budget(&mut classified)?;
    let pilot_budget_truncated = pilot_budget_info.is_some();
    let limit_info = pilot_budget_info.or(inventory_limit_info);
    let (causal_projection, causal_projection_warning) =
        crate::app::causal_projection::CausalDeltaArtifact::load_optional(&input.root);
    if let Some(warning) = causal_projection_warning {
        eprintln!("ripr pilot: {warning}");
    }

    let python_first_use = collect_pilot_python_first_use(&input, &config);
    // #3906: pilot ranks Rust seams only. Name the languages it did not rank
    // so an empty ranking is never read as a clean result for them.
    let language_routes = output::pilot::PilotLanguageRoutes::from_discovered(
        &input.root,
        !classified.is_empty(),
        config.languages().enabled(),
        &analysis::workspace_preview_language_files(&input.root),
    )
    .with_unanalyzed(
        analysis::workspace_unanalyzed_source_languages(&input.root),
        !analysis::workspace_rust_files(&input.root).is_empty(),
    );
    let context = output::pilot::PilotSummaryContext {
        root: &input.root,
        mode: &input.mode,
        config_path: config.source_path(),
        max_seams: options.max_seams,
        timeout_ms: options.timeout_ms,
        artifacts: &artifacts,
        python_first_use: python_first_use.as_ref(),
        language_routes: Some(&language_routes),
    };

    let ts_guidance = output::render::detect_ts_full_repo_guidance_pub(&input.root, &classified);
    let python_guidance =
        output::render::detect_python_repo_exposure_guidance_pub(&input.root, &classified);
    write_pilot_repo_exposure_json(
        &artifacts.repo_exposure_json,
        &input,
        &config,
        &classified,
        limit_info.as_ref(),
        generated_skip.as_ref(),
        pilot_budget_truncated,
    )?;
    write_pilot_file(
        &artifacts.repo_exposure_md,
        output::repo_exposure::render_repo_exposure_md_with_generated_skip(
            &classified,
            limit_info.as_ref(),
            ts_guidance.as_ref(),
            python_guidance.as_ref(),
            generated_skip.as_ref(),
        ),
    )?;
    write_pilot_file(
        &artifacts.agent_seam_packets_json,
        output::agent_seam_packets::render_agent_seam_packets_json_with_causal(
            &classified,
            limit_info.as_ref(),
            causal_projection.as_ref(),
        ),
    )?;

    write_pilot_file(
        &artifacts.pilot_summary_json,
        output::pilot::render_pilot_summary_json(&classified, context),
    )?;
    write_pilot_file(
        &artifacts.pilot_summary_md,
        output::pilot::render_pilot_summary_md(&classified, context),
    )?;

    print!(
        "{}",
        output::pilot::render_pilot_terminal(&classified, context)
    );
    // #5019: producer `completed` was held until every artifact write and
    // the terminal render succeeded; commit it now. An earlier failure
    // already dropped the sink, which projected `failed` instead.
    if let Some(sink) = progress.as_ref() {
        sink.commit_success();
    }
    Ok(())
}

/// Build the stderr progress sink for one pilot analysis attempt (#5019).
///
/// `--quiet` opts out of the progress stream per the #2608 contract; the
/// one-shot timeout notice and command errors still reach stderr. The
/// sink is terminal-once, so a timed-out attempt (or its #2424 retry)
/// needs a fresh one from `pilot_progress_sink`.
fn pilot_progress_sink(quiet: bool) -> Option<Arc<CliProgressSink>> {
    if quiet {
        return None;
    }
    Some(Arc::new(CliProgressSink::for_stderr(
        std::io::stderr().is_terminal(),
        ProgressPolicy::STANDARD,
    )))
}

/// Run the pilot repo inventory through the shared app-layer
/// progress-bearing entry point (#5019): the same producer-owned
/// `loading_input`/`analyzing`/`building_output`/`completed` boundaries
/// `ripr check` projects, at repo scope, so a cold multi-minute first
/// run shows bounded stage lines and throttled heartbeats instead of
/// minutes of silence. Stage identity stays producer-owned; pilot adds
/// no stage vocabulary of its own.
fn run_pilot_inventory(
    root: &Path,
    config: &RiprConfig,
    sink: Option<&Arc<CliProgressSink>>,
) -> Result<analysis::ClassifiedSeamsReport, String> {
    app::repo_inventory_with_progress(
        sink.map(|sink| &**sink as &dyn crate::app::AnalysisProgressSink),
        || analysis::inventory_classified_seams_report_at_with_config(root, config),
        Ok,
    )
}

fn collect_pilot_python_first_use(
    input: &CheckInput,
    config: &RiprConfig,
) -> Option<output::pilot::PilotPythonFirstUse> {
    if !config
        .languages()
        .enabled()
        .contains(&crate::domain::LanguageId::Python)
    {
        return None;
    }

    let mut check_input = input.clone();
    check_input.format = OutputFormat::Json;
    Some(
        match app::check_workspace_with_config(check_input, config) {
            Ok(output) => output::pilot::PilotPythonFirstUse::from_check_output(&output),
            Err(error) => output::pilot::PilotPythonFirstUse::analysis_unavailable(error),
        },
    )
}

fn parse_pilot_options(args: &[String]) -> Result<PilotOptions, String> {
    let mut options = PilotOptions {
        root: PathBuf::from("."),
        out_dir: PathBuf::from("target/ripr/pilot"),
        mode: Mode::Draft,
        explicit: CheckInputExplicit::default(),
        max_seams: 5,
        timeout_ms: DEFAULT_PILOT_TIMEOUT_MS,
        timeout_explicit: false,
        quiet: false,
    };
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                options.root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--out" => {
                i += 1;
                options.out_dir = PathBuf::from(expect_value(args, i, "--out")?);
            }
            "--mode" => {
                i += 1;
                options.mode = parse_mode(expect_value(args, i, "--mode")?)?;
                options.explicit.mode = true;
            }
            "--max-seams" => {
                i += 1;
                options.max_seams =
                    parse_positive_usize(expect_value(args, i, "--max-seams")?, "--max-seams")?;
            }
            "--timeout-ms" => {
                i += 1;
                options.timeout_ms =
                    parse_positive_u64(expect_value(args, i, "--timeout-ms")?, "--timeout-ms")?;
                options.timeout_explicit = true;
            }
            "--quiet" => {
                options.quiet = true;
            }
            other => return Err(unknown_argument("pilot", other)),
        }
        i += 1;
    }
    Ok(options)
}

/// The deadline extension applies only to the default budget. A typed
/// `--timeout-ms`, even one equal to the default, is a hard limit.
fn pilot_deadline_extension_ms(options: &PilotOptions) -> Option<u64> {
    (!options.timeout_explicit).then_some(PILOT_RETRY_TIMEOUT_MS)
}

enum PilotAnalysisResult {
    Complete(analysis::ClassifiedSeamsReport),
    TimedOut,
}

/// Run the pilot analysis on a worker thread under a deadline.
///
/// When `extension_ms` is set and the first deadline passes, `on_extend` runs
/// once and the same analysis keeps going for `extension_ms` more; work done
/// before the first deadline is never thrown away.
fn run_pilot_analysis_with_timeout<F>(
    timeout_ms: u64,
    extension_ms: Option<u64>,
    on_extend: impl FnOnce(),
    runner: F,
) -> Result<PilotAnalysisResult, String>
where
    F: FnOnce() -> Result<analysis::ClassifiedSeamsReport, String> + Send + 'static,
{
    let cancellation_token = crate::analysis::cancellation::AnalysisCancellationToken::new();
    let worker_token = cancellation_token.clone();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = crate::analysis::cancellation::with_token(&worker_token, runner);
        let _ignored = tx.send(result);
    });

    let mut outcome = rx.recv_timeout(Duration::from_millis(timeout_ms));
    if let (Err(mpsc::RecvTimeoutError::Timeout), Some(extension_ms)) = (&outcome, extension_ms) {
        on_extend();
        outcome = rx.recv_timeout(Duration::from_millis(extension_ms));
    }
    match outcome {
        Ok(result) => result.map(PilotAnalysisResult::Complete),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            cancellation_token
                .cancel(crate::analysis::cancellation::AnalysisAbortKind::DeadlineExceeded);
            // #5019: the detached worker owns its progress run's terminal
            // (`cancelled`), and the caller exits soon after this return.
            // Give the worker a bounded window to reach a cancellation
            // checkpoint and finish, so the terminal is not lost to process
            // exit. A worker stuck past the window cannot be forced (it is
            // a detached thread): a known, bounded limitation.
            let _ignored = rx.recv_timeout(Duration::from_secs(5));
            Ok(PilotAnalysisResult::TimedOut)
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("pilot analysis stopped before producing a result".to_string())
        }
    }
}

fn pilot_artifacts(out_dir: &Path) -> output::pilot::PilotArtifacts {
    output::pilot::PilotArtifacts {
        repo_exposure_json: out_dir.join("repo-exposure.json"),
        repo_exposure_md: out_dir.join("repo-exposure.md"),
        agent_seam_packets_json: out_dir.join("agent-seam-packets.json"),
        pilot_summary_json: out_dir.join("pilot-summary.json"),
        pilot_summary_md: out_dir.join("pilot-summary.md"),
    }
}

/// Pilot artifacts default under the analyzed repository, which may commit a
/// symlink at an artifact path; never write through it.
fn write_pilot_file(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), String> {
    output::file_write::write(path, contents.as_ref())
        .map_err(|err| format!("write output {} failed: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn pilot_requires_values_for_value_flags() {
        assert_eq!(
            pilot(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            pilot(&args(&["--out"])),
            Err("missing value for --out".to_string())
        );
        assert_eq!(
            pilot(&args(&["--mode"])),
            Err("missing value for --mode".to_string())
        );
        assert_eq!(
            pilot(&args(&["--max-seams"])),
            Err("missing value for --max-seams".to_string())
        );
        assert_eq!(
            pilot(&args(&["--timeout-ms"])),
            Err("missing value for --timeout-ms".to_string())
        );
    }

    #[test]
    fn pilot_rejects_unknown_arguments() {
        assert_eq!(
            pilot(&args(&["--wat"])),
            Err("unknown pilot argument \"--wat\". Run `ripr pilot --help`.".to_string())
        );
    }

    #[test]
    fn pilot_rejects_non_positive_max_seams() {
        assert_eq!(
            parse_pilot_options(&args(&["--max-seams", "0"])),
            Err("--max-seams requires a positive integer; got \"0\"".to_string())
        );
    }

    #[test]
    fn pilot_rejects_non_positive_timeout() {
        assert_eq!(
            parse_pilot_options(&args(&["--timeout-ms", "0"])),
            Err("--timeout-ms requires a positive integer; got \"0\"".to_string())
        );
    }

    #[test]
    fn pilot_parses_root_out_mode_max_seams_quiet_and_timeout() {
        let options = parse_pilot_options(&args(&[
            "--root",
            "repo",
            "--out",
            "target/pilot",
            "--mode",
            "ready",
            "--max-seams",
            "3",
            "--timeout-ms",
            "120000",
            "--quiet",
        ]));

        assert_eq!(
            options,
            Ok(PilotOptions {
                root: PathBuf::from("repo"),
                out_dir: PathBuf::from("target/pilot"),
                mode: Mode::Ready,
                explicit: CheckInputExplicit {
                    mode: true,
                    include_unchanged_tests: false,
                },
                max_seams: 3,
                timeout_ms: 120_000,
                timeout_explicit: true,
                quiet: true,
            })
        );
    }

    #[test]
    fn explicit_default_timeout_is_a_hard_limit() -> Result<(), String> {
        // Only an omitted --timeout-ms earns the cold-run extension; typing
        // the default value must not turn a 30s limit into 270s.
        let omitted = parse_pilot_options(&args(&["--quiet"]))?;
        assert_eq!(
            pilot_deadline_extension_ms(&omitted),
            Some(PILOT_RETRY_TIMEOUT_MS)
        );
        let explicit = parse_pilot_options(&args(&["--timeout-ms", "30000"]))?;
        assert_eq!(explicit.timeout_ms, DEFAULT_PILOT_TIMEOUT_MS);
        assert_eq!(pilot_deadline_extension_ms(&explicit), None);
        Ok(())
    }

    #[test]
    fn pilot_analysis_timeout_cancels_worker() {
        let (cancelled_tx, cancelled_rx) = mpsc::channel();
        let result = run_pilot_analysis_with_timeout(
            1,
            None,
            || {},
            move || {
                loop {
                    if crate::analysis::cancellation::checkpoint().is_err() {
                        let _ignored = cancelled_tx.send(());
                        return Err("analysis cancelled".to_string());
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            },
        );

        assert!(matches!(result, Ok(PilotAnalysisResult::TimedOut)));
        assert_eq!(cancelled_rx.recv_timeout(Duration::from_secs(1)), Ok(()));
    }

    #[test]
    fn pilot_deadline_extension_keeps_the_running_analysis() {
        // #2424: when the default deadline passes, the same analysis keeps
        // running under the extension. A restart would invoke the runner a
        // second time and redo the work done before the first deadline.
        let runs = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let worker_runs = std::sync::Arc::clone(&runs);
        let extended = std::cell::Cell::new(0);
        let result = run_pilot_analysis_with_timeout(
            20,
            Some(5_000),
            || extended.set(extended.get() + 1),
            move || {
                worker_runs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(200));
                crate::analysis::cancellation::checkpoint()?;
                Ok(analysis::ClassifiedSeamsReport {
                    classified: Vec::new(),
                    limit_info: None,
                    skipped_generated: Vec::new(),
                    naming_only_skips: Vec::new(),
                })
            },
        );

        assert!(
            matches!(result, Ok(PilotAnalysisResult::Complete(_))),
            "the extended run must complete instead of timing out"
        );
        assert_eq!(extended.get(), 1, "the extension notice prints once");
        assert_eq!(
            runs.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the analysis must run once, not restart"
        );
    }

    #[test]
    fn pilot_deadline_without_extension_still_cancels() {
        // Negative control for the extension: an explicit --timeout-ms
        // passes no extension, so the first deadline cancels the run and
        // the extension callback never fires.
        let extended = std::cell::Cell::new(false);
        let result = run_pilot_analysis_with_timeout(
            20,
            None,
            || extended.set(true),
            || {
                std::thread::sleep(Duration::from_millis(200));
                crate::analysis::cancellation::checkpoint()?;
                Err("analysis was not cancelled".to_string())
            },
        );

        assert!(matches!(result, Ok(PilotAnalysisResult::TimedOut)));
        assert!(!extended.get());
    }

    #[derive(Clone)]
    struct ProgressBuffer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl ProgressBuffer {
        fn new() -> Self {
            Self(std::sync::Arc::new(std::sync::Mutex::new(Vec::new())))
        }

        fn text(&self) -> String {
            match self.0.lock() {
                Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                Err(poisoned) => String::from_utf8_lossy(&poisoned.into_inner()).into_owned(),
            }
        }
    }

    impl std::io::Write for ProgressBuffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            match self.0.lock() {
                Ok(mut bytes) => bytes.extend_from_slice(buf),
                Err(poisoned) => poisoned.into_inner().extend_from_slice(buf),
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn pilot_progress_run_emits_stage_heartbeat_and_cancelled_on_timeout() {
        // #5019: a long-running pilot analysis (a deliberately blocked
        // inventory stands in for a cold-cache multi-minute repo walk) must
        // project the same repo-scope stage lines and throttled heartbeat
        // evidence `ripr check` emits, and the deadline must close the run
        // as `cancelled`, never `completed`.
        let buffer = ProgressBuffer::new();
        let sink = std::sync::Arc::new(CliProgressSink::with_writer(
            Box::new(buffer.clone()),
            false,
            ProgressPolicy::STANDARD,
        ));
        let (done_tx, done_rx) = mpsc::channel();
        let result = run_pilot_analysis_with_timeout(
            3_000,
            None,
            || {},
            move || {
                let result = app::repo_inventory_with_progress(
                    Some(&*sink),
                    || -> Result<analysis::ClassifiedSeamsReport, String> {
                        loop {
                            if crate::analysis::cancellation::checkpoint().is_err() {
                                return Err("analysis cancelled".to_string());
                            }
                            std::thread::sleep(Duration::from_millis(10));
                        }
                    },
                    Ok,
                );
                let _ignored = done_tx.send(());
                result
            },
        );

        assert!(matches!(result, Ok(PilotAnalysisResult::TimedOut)));
        // The bounded handoff joins the cancelled worker before returning,
        // so the run's terminal must already be projected; no waiting on
        // the worker is needed to observe it.
        let text = buffer.text();
        assert!(
            text.contains("ripr progress: cancelled [repo]"),
            "deadline must close the run as cancelled: {text}"
        );
        assert_eq!(done_rx.recv_timeout(Duration::from_secs(5)), Ok(()));
        let text = buffer.text();
        assert!(
            text.contains("ripr progress: loading_input [repo]"),
            "missing repo-scope loading_input: {text}"
        );
        assert!(
            text.contains("ripr progress: analyzing [repo]"),
            "missing repo-scope analyzing: {text}"
        );
        assert!(
            text.contains("still active after 2s"),
            "a stage held past first_heartbeat must heartbeat: {text}"
        );
        assert!(
            !text.contains("completed"),
            "a timed-out run must never project completed: {text}"
        );
        assert!(
            !text.contains("[diff]"),
            "pilot must project repo scope, not diff scope: {text}"
        );
    }

    #[test]
    fn pilot_progress_stream_absent_when_quiet_or_sink_removed() {
        // #5019 removal experiment per #2608's closure rule: disabling the
        // producer-side progress stream (--quiet, or no sink at all) removes
        // every stage and heartbeat line while the timeout machinery still
        // behaves identically.
        assert!(
            pilot_progress_sink(true).is_none(),
            "--quiet must suppress the pilot progress sink"
        );
        assert!(pilot_progress_sink(false).is_some());

        let result = run_pilot_analysis_with_timeout(
            50,
            None,
            || {},
            || {
                app::repo_inventory_with_progress(
                    None,
                    || -> Result<analysis::ClassifiedSeamsReport, String> {
                        loop {
                            if crate::analysis::cancellation::checkpoint().is_err() {
                                return Err("analysis cancelled".to_string());
                            }
                            std::thread::sleep(Duration::from_millis(5));
                        }
                    },
                    Ok,
                )
            },
        );
        assert!(
            matches!(result, Ok(PilotAnalysisResult::TimedOut)),
            "removing the sink must not change the timeout behavior"
        );
    }

    #[cfg(unix)]
    #[test]
    fn unwritable_out_dir_names_out_not_out_dir() -> Result<(), String> {
        use crate::testing::unwritable_output::OutputDirFixture;

        let env = OutputDirFixture::unwritable("pilot-ro", "pilot")?;
        let root = OutputDirFixture::path_arg(&env.root)?;
        let out = OutputDirFixture::path_arg(&env.target)?;
        let error = match pilot(&args(&["--root", root, "--out", out])) {
            Err(error) => error,
            Ok(()) => return Err("unwritable --out must fail before analysis".to_string()),
        };
        assert!(error.contains(&format!("create {out} failed:")), "{error}");
        assert!(
            error.contains("write elsewhere with --out PATH"),
            "pilot must name --out PATH, got {error}"
        );
        assert!(
            !error.contains("--out-dir"),
            "pilot must not name first-pr's flag, got {error}"
        );
        Ok(())
    }

    #[test]
    fn occupying_file_out_dir_does_not_name_the_relocate_flag() -> Result<(), String> {
        use crate::testing::unwritable_output::OutputDirFixture;

        let env = OutputDirFixture::occupying_file("pilot-file", "pilot")?;
        let root = OutputDirFixture::path_arg(&env.root)?;
        let out = OutputDirFixture::path_arg(&env.target)?;
        let error = match pilot(&args(&["--root", root, "--out", out])) {
            Err(error) => error,
            Ok(()) => return Err("file occupying --out must fail".to_string()),
        };
        assert!(error.contains(&format!("create {out} failed:")), "{error}");
        assert!(
            !error.contains("write elsewhere"),
            "a file occupying --out is not a not-writable tree: {error}"
        );
        Ok(())
    }
}
