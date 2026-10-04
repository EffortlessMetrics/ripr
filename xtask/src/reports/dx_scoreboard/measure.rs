//! Instruments for the `measured` scoreboard metrics.
//!
//! Every child runs through `crate::run::capture_output_measured`, the
//! shared owned-subprocess path, with an explicit deadline. Corpus runs use
//! a fresh `RIPR_CACHE_DIR` per repository so "cold" means no ripr cache,
//! and the warm check reuses that same cache.

use super::{Config, CorpusEntry, Options, RunContext, Sample, SampleOutcome};
use crate::run::{MeasuredOutput, capture_output_measured};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CACHE_ENV: &str = "RIPR_CACHE_DIR";
const GIT_TIMEOUT: Duration = Duration::from_mins(10);
const SHORT_TIMEOUT: Duration = Duration::from_mins(2);
/// Directory name of the paste-safety fixture root. It holds a space, a
/// command substitution that would create `PWNED` if a printed command left
/// it unquoted, an apostrophe and double quotes.
const HOSTILE_DIR: &str = "dx paste $(touch PWNED) it's \"q\"";
const CANARY: &str = "PWNED";
/// Subcommands whose printed invocations the paste board replays.
const COMMAND_WORDS: [&str; 17] = [
    "check",
    "explain",
    "context",
    "pilot",
    "agent",
    "first-pr",
    "outcome",
    "reports",
    "init",
    "doctor",
    "receipt",
    "pr-summary",
    "pr-evidence",
    "review-comments",
    "cache",
    "gate",
    "status",
];

pub(super) fn measure(
    config: &Config,
    options: &Options,
    samples: &mut Vec<Sample>,
) -> Result<RunContext, String> {
    let boards = &options.boards;
    let wants = |board: &str| boards.iter().any(|b| b == board);
    let needs_binary = ["speed", "ci", "trust", "paste"]
        .iter()
        .any(|board| wants(board));
    let mut context = RunContext {
        revision: git_revision(),
        runner_class: runner_class(),
        ..RunContext::default()
    };
    if !needs_binary {
        return Ok(context);
    }
    let binary = match &options.ripr_bin {
        Some(path) => path.clone(),
        None => {
            crate::run::run(
                "cargo",
                &["build", "--release", "-p", "ripr", "--bin", "ripr"],
            )?;
            release_binary()
        }
    };
    let binary = fs::canonicalize(&binary)
        .map_err(|err| format!("ripr binary {}: {err}", binary.display()))?;
    // Record the binary relative to the checkout so a committed report does
    // not carry a host-specific absolute path.
    context.binary = std::env::current_dir()
        .ok()
        .and_then(|cwd| binary.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| binary.clone())
        .display()
        .to_string();
    context.analyzer_version = analyzer_version(&binary);

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let scratch = std::env::current_dir()
        .map_err(|err| format!("current dir: {err}"))?
        .join("target/ripr/dx-scoreboard")
        .join(format!("run-{stamp}"));
    fs::create_dir_all(&scratch)
        .map_err(|err| format!("create scoreboard scratch {}: {err}", scratch.display()))?;
    let result = measure_in(config, options, &binary, &scratch, samples, &mut context);
    let _ = fs::remove_dir_all(&scratch);
    result.map(|()| context)
}

fn measure_in(
    config: &Config,
    options: &Options,
    binary: &Path,
    scratch: &Path,
    samples: &mut Vec<Sample>,
    context: &mut RunContext,
) -> Result<(), String> {
    let wants = |board: &str| options.boards.iter().any(|b| b == board);
    let timeout = Duration::from_millis(options.timeout_ms);
    if wants("speed") || wants("trust") {
        for entry in selected_corpus(config, options)? {
            let corpus = measure_corpus_entry(entry, options, binary, scratch, timeout, samples);
            context.corpus.push(corpus);
        }
    }
    if wants("ci") {
        samples.extend(measure_ci(binary, scratch));
    }
    if wants("trust") {
        samples.push(measure_bad_input(binary, scratch));
        samples.push(measure_hostile_repos(binary));
    }
    if wants("paste") {
        samples.extend(measure_paste(binary, scratch));
    }
    Ok(())
}

fn selected_corpus<'a>(
    config: &'a Config,
    options: &Options,
) -> Result<Vec<&'a CorpusEntry>, String> {
    for repo in &options.repos {
        if !config.corpus.iter().any(|entry| &entry.id == repo) {
            return Err(format!("--repo `{repo}` is not in the scoreboard corpus"));
        }
    }
    Ok(config
        .corpus
        .iter()
        .filter(|entry| {
            if options.repos.is_empty() {
                options.include_heavy || !entry.heavy
            } else {
                options.repos.contains(&entry.id)
            }
        })
        .collect())
}

// ------------------------------------------------------------- corpus ----

fn measure_corpus_entry(
    entry: &CorpusEntry,
    options: &Options,
    binary: &Path,
    scratch: &Path,
    timeout: Duration,
    samples: &mut Vec<Sample>,
) -> Value {
    let repo = Some(entry.id.clone());
    let sample = |metric: &str, outcome: SampleOutcome, detail: String| Sample {
        metric: metric.to_string(),
        repo: repo.clone(),
        outcome,
        detail,
    };
    let corpus_metrics = [
        "speed.cold_pilot_ms",
        "speed.cold_pilot_peak_rss_mb",
        "speed.warm_check_ms",
        "speed.warm_check_peak_rss_mb",
        "trust.self_contradictions",
    ];
    let checkout = match prepare_checkout(entry, options) {
        Ok(path) => path,
        Err(reason) => {
            // With --clone the run promised a checkout, so losing it is a
            // broken instrument the gate must see; without it, the corpus
            // was simply not fetched.
            let (outcome, status) = if options.clone {
                (SampleOutcome::Failed, "failed")
            } else {
                (SampleOutcome::NotMeasured, "not_measured")
            };
            for metric in corpus_metrics {
                samples.push(sample(metric, outcome.clone(), reason.clone()));
            }
            return json!({"id": entry.id, "sha": entry.sha, "status": status, "reason": reason});
        }
    };

    let cache = scratch.join(format!("cache-{}", entry.id));
    let out = scratch.join(format!("pilot-{}", entry.id));
    if let Some(reason) = linked_target(&checkout) {
        for metric in corpus_metrics {
            samples.push(sample(metric, SampleOutcome::Failed, reason.clone()));
        }
        return json!({
            "id": entry.id,
            "sha": entry.sha,
            "url": entry.url,
            "status": "failed",
            "reason": reason,
        });
    }
    let _ = fs::remove_dir_all(checkout.join("target/ripr"));
    let cache_text = cache.display().to_string();
    let envs = [(CACHE_ENV, cache_text.as_str())];
    let root = checkout.display().to_string();

    let pilot_args = vec![
        "pilot".to_string(),
        "--root".to_string(),
        root.clone(),
        "--out".to_string(),
        out.display().to_string(),
        "--mode".to_string(),
        "ready".to_string(),
        "--max-seams".to_string(),
        "5".to_string(),
        "--quiet".to_string(),
    ];
    let mut contradictions: Option<(usize, Vec<String>)> = None;
    // Both documents must be scanned; one alone would undercount.
    let mut missing_sources: Vec<&str> = Vec::new();
    match capture_output_measured(
        &binary.display().to_string(),
        &pilot_args,
        None,
        &envs,
        timeout,
        "dx-scoreboard cold pilot",
    ) {
        Ok(measured) => {
            let summary = read_json_file(&out.join("pilot-summary.json"));
            let status = summary
                .as_ref()
                .and_then(|summary| summary["status"].as_str().map(str::to_string))
                .unwrap_or_else(|| "missing".to_string());
            let complete = exited_zero(&measured) && status == "complete";
            let ms = duration_ms(&measured);
            let detail = format!(
                "exit {}, pilot status `{status}`{}",
                exit_label(&measured),
                if measured.output.timed_out {
                    ", harness deadline reached"
                } else {
                    ""
                }
            );
            samples.push(sample(
                "speed.cold_pilot_ms",
                if complete {
                    SampleOutcome::Value(ms)
                } else {
                    SampleOutcome::Incomplete(ms)
                },
                detail.clone(),
            ));
            samples.push(rss_sample(
                &sample,
                "speed.cold_pilot_peak_rss_mb",
                &measured,
                complete,
            ));
            if let Some(exposure) = read_json_file(&out.join("repo-exposure.json")) {
                let found = repo_exposure_contradictions(&exposure);
                contradictions = Some(merge_contradictions(contradictions, found));
            } else {
                missing_sources.push("pilot repo-exposure.json");
            }
        }
        Err(err) => {
            missing_sources.push("pilot repo-exposure.json");
            samples.push(sample(
                "speed.cold_pilot_ms",
                SampleOutcome::Failed,
                err.clone(),
            ));
            samples.push(sample(
                "speed.cold_pilot_peak_rss_mb",
                SampleOutcome::Failed,
                err,
            ));
        }
    }

    let check_args = vec![
        "check".to_string(),
        "--root".to_string(),
        root.clone(),
        "--base".to_string(),
        entry
            .base_sha
            .clone()
            .unwrap_or_else(|| "HEAD~1".to_string()),
        "--format".to_string(),
        "json".to_string(),
    ];
    // The first run warms the cache for this exact diff; the second is the
    // edit-check loop a developer repeats.
    let warmup = capture_output_measured(
        &binary.display().to_string(),
        &check_args,
        None,
        &envs,
        timeout,
        "dx-scoreboard warm-up check",
    );
    let measured = warmup.and_then(|_| {
        capture_output_measured(
            &binary.display().to_string(),
            &check_args,
            None,
            &envs,
            timeout,
            "dx-scoreboard warm check",
        )
    });
    match measured {
        Ok(measured) => {
            let ms = duration_ms(&measured);
            let parsed = serde_json::from_str::<Value>(&measured.output.stdout).ok();
            let ok = exited_zero(&measured) && parsed.is_some();
            let findings = parsed
                .as_ref()
                .and_then(|json| json["summary"]["findings"].as_u64())
                .unwrap_or(0);
            samples.push(sample(
                "speed.warm_check_ms",
                if ok {
                    SampleOutcome::Value(ms)
                } else {
                    SampleOutcome::Incomplete(ms)
                },
                format!(
                    "exit {}, {findings} finding(s) on HEAD~1..HEAD",
                    exit_label(&measured)
                ),
            ));
            samples.push(rss_sample(
                &sample,
                "speed.warm_check_peak_rss_mb",
                &measured,
                ok,
            ));
            if let Some(json) = parsed {
                contradictions = Some(merge_contradictions(
                    contradictions,
                    check_contradictions(&json),
                ));
            } else {
                missing_sources.push("warm check JSON");
            }
        }
        Err(err) => {
            missing_sources.push("warm check JSON");
            samples.push(sample(
                "speed.warm_check_ms",
                SampleOutcome::Failed,
                err.clone(),
            ));
            samples.push(sample(
                "speed.warm_check_peak_rss_mb",
                SampleOutcome::Failed,
                err,
            ));
        }
    }

    let (outcome, detail) = contradiction_outcome(contradictions, &missing_sources);
    samples.push(sample("trust.self_contradictions", outcome, detail));
    json!({"id": entry.id, "sha": entry.sha, "url": entry.url, "status": "measured"})
}

/// Why earlier ripr state in `checkout` must not be cleared, if it must not.
/// A checkout that ships `target` or `target/ripr` as a symlink would point
/// the delete outside the checkout.
pub(crate) fn linked_target(checkout: &Path) -> Option<String> {
    let is_link =
        |path: &Path| fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink());
    (is_link(&checkout.join("target")) || is_link(&checkout.join("target/ripr"))).then(|| {
        "checkout has a symlinked target/ or target/ripr; refusing to clear it \
         (remove the link from the checkout or pin a SHA without it)"
            .to_string()
    })
}

/// The contradiction sample. Both documents must be scanned; a count from
/// one alone would undercount, so it is incomplete rather than a value.
pub(crate) fn contradiction_outcome(
    contradictions: Option<(usize, Vec<String>)>,
    missing_sources: &[&str],
) -> (SampleOutcome, String) {
    match contradictions {
        Some((count, _)) if !missing_sources.is_empty() => (
            SampleOutcome::Incomplete(count as f64),
            format!("not scanned: {}", missing_sources.join(", ")),
        ),
        Some((count, examples)) => (
            SampleOutcome::Value(count as f64),
            if examples.is_empty() {
                "pilot repo-exposure and warm check JSON scanned".to_string()
            } else {
                format!("e.g. {}", examples.join("; "))
            },
        ),
        None => (
            SampleOutcome::NotMeasured,
            "no pilot or check JSON to scan".to_string(),
        ),
    }
}

fn prepare_checkout(entry: &CorpusEntry, options: &Options) -> Result<PathBuf, String> {
    let dir = options.corpus_dir.join(&entry.id);
    if !dir.join(".git").exists() {
        if !options.clone {
            return Err(format!(
                "no checkout at {}; rerun with --clone to fetch the pinned corpus",
                dir.display()
            ));
        }
        fs::create_dir_all(&options.corpus_dir)
            .map_err(|err| format!("create {}: {err}", options.corpus_dir.display()))?;
        git(
            None,
            &[
                "clone",
                "--quiet",
                "--filter=blob:none",
                &entry.url,
                &dir.display().to_string(),
            ],
        )?;
    }
    let dir = fs::canonicalize(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    let head = git(Some(&dir), &["rev-parse", "HEAD"])?;
    if head.trim() != entry.sha {
        let checkout = git(Some(&dir), &["checkout", "--quiet", "--detach", &entry.sha]);
        if checkout.is_err() {
            if !options.clone {
                return Err(format!(
                    "{} lacks pin {}; rerun with --clone to fetch it",
                    dir.display(),
                    entry.sha
                ));
            }
            git(Some(&dir), &["fetch", "--quiet", "origin", &entry.sha])?;
            git(Some(&dir), &["checkout", "--quiet", "--detach", &entry.sha])?;
        }
    }
    let dirty = git(
        Some(&dir),
        &["status", "--porcelain", "--untracked-files=no"],
    )?;
    if !dirty.trim().is_empty() {
        return Err(format!(
            "{} has local changes; the pinned corpus must be clean",
            dir.display()
        ));
    }
    Ok(dir)
}

fn git(cwd: Option<&Path>, args: &[&str]) -> Result<String, String> {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    let measured = capture_output_measured("git", &owned, cwd, &[], GIT_TIMEOUT, "git")?;
    if exited_zero(&measured) {
        Ok(measured.output.stdout)
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            measured.output.stderr.trim()
        ))
    }
}

/// Self-contradiction rules over one pilot `repo-exposure.json`:
/// `R1` a seam says reach is `no` while it lists related tests.
pub(crate) fn repo_exposure_contradictions(exposure: &Value) -> (usize, Vec<String>) {
    let mut count = 0;
    let mut examples = Vec::new();
    for seam in exposure["seams"].as_array().into_iter().flatten() {
        let related = seam["related_tests_total"].as_u64().unwrap_or(0);
        if seam["evidence"]["reach"].as_str() == Some("no") && related > 0 {
            count += 1;
            if examples.len() < 3 {
                examples.push(format!(
                    "R1 {}:{} reach `no` with {related} related tests",
                    seam["file"].as_str().unwrap_or("?"),
                    seam["line"]
                ));
            }
        }
    }
    (count, examples)
}

/// Self-contradiction rules over one `ripr check --format json` result:
/// `R2` classification `no_static_path` while listing related tests;
/// `R3` evidence says related tests were found while listing none.
pub(crate) fn check_contradictions(check: &Value) -> (usize, Vec<String>) {
    let mut count = 0;
    let mut examples = Vec::new();
    for finding in check["findings"].as_array().into_iter().flatten() {
        let related = finding["related_tests_total"].as_u64().unwrap_or(0);
        let id = finding["id"].as_str().unwrap_or("?");
        let rule = if finding["classification"].as_str() == Some("no_static_path") && related > 0 {
            Some(format!(
                "R2 {id} no_static_path with {related} related tests"
            ))
        } else if related == 0
            && finding["evidence"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .any(|line| line.starts_with("Related tests were found"))
        {
            Some(format!("R3 {id} says related tests were found but lists 0"))
        } else {
            None
        };
        if let Some(rule) = rule {
            count += 1;
            if examples.len() < 3 {
                examples.push(rule);
            }
        }
    }
    (count, examples)
}

fn merge_contradictions(
    current: Option<(usize, Vec<String>)>,
    found: (usize, Vec<String>),
) -> (usize, Vec<String>) {
    let (mut count, mut examples) = current.unwrap_or((0, Vec::new()));
    count += found.0;
    for example in found.1 {
        if examples.len() < 4 {
            examples.push(example);
        }
    }
    (count, examples)
}

// ----------------------------------------------------------------- ci ----

fn measure_ci(binary: &Path, scratch: &Path) -> Vec<Sample> {
    let root = scratch.join("ci-init");
    let result = (|| -> Result<(usize, bool), String> {
        write_tiny_crate(&root)?;
        let args = vec![
            "init".to_string(),
            "--root".to_string(),
            root.display().to_string(),
            "--ci".to_string(),
            "github".to_string(),
        ];
        let measured = capture_output_measured(
            &binary.display().to_string(),
            &args,
            Some(scratch),
            &[],
            SHORT_TIMEOUT,
            "dx-scoreboard init --ci github",
        )?;
        if !exited_zero(&measured) {
            return Err(format!(
                "ripr init --ci github exited {}: {}",
                exit_label(&measured),
                measured.output.stderr.trim()
            ));
        }
        let workflow = root.join(".github/workflows/ripr.yml");
        let text = fs::read_to_string(&workflow)
            .map_err(|err| format!("read {}: {err}", workflow.display()))?;
        Ok((text.lines().count(), builds_ripr_from_source(&text)))
    })();
    match result {
        Ok((lines, from_source)) => vec![
            Sample {
                metric: "ci.workflow_lines".to_string(),
                repo: None,
                outcome: SampleOutcome::Value(lines as f64),
                detail: "lines in .github/workflows/ripr.yml from `ripr init --ci github`"
                    .to_string(),
            },
            Sample {
                metric: "ci.builds_ripr_from_source".to_string(),
                repo: None,
                outcome: SampleOutcome::Value(if from_source { 1.0 } else { 0.0 }),
                detail: if from_source {
                    "workflow compiles ripr with `cargo install` and has no prebuilt download"
                        .to_string()
                } else {
                    "workflow downloads a prebuilt ripr release (`cargo install` only as fallback, if at all)".to_string()
                },
            },
        ],
        Err(err) => ["ci.workflow_lines", "ci.builds_ripr_from_source"]
            .iter()
            .map(|metric| Sample {
                metric: (*metric).to_string(),
                repo: None,
                outcome: SampleOutcome::Failed,
                detail: err.clone(),
            })
            .collect(),
    }
}

/// True when the workflow's only route to ripr is compiling it. A
/// `cargo install` kept as the fallback behind a prebuilt release download is
/// not counted: developers on supported runners never pay for it.
pub(crate) fn builds_ripr_from_source(workflow: &str) -> bool {
    let code = || {
        workflow
            .lines()
            .map(str::trim_start)
            .filter(|line| !line.starts_with('#'))
    };
    let compiles = code().any(|line| line.contains("cargo install ripr"));
    let downloads_prebuilt = code().any(|line| line.contains("/releases/download/"));
    compiles && !downloads_prebuilt
}

// -------------------------------------------------------------- trust ----

/// Commands pointed at a repository that does not exist. Each should refuse
/// with a nonzero exit; an exit of 0 is a false clean a developer or CI
/// script would read as success.
fn bad_input_commands(missing: &str, scratch: &str) -> Vec<Vec<String>> {
    let bad_pilot = format!("{scratch}/bad-pilot");
    let before = format!("{missing}/before.json");
    let after = format!("{missing}/after.json");
    let exposure = format!("{missing}/repo-exposure.json");
    let ledger = format!("{scratch}/bad-ledger.json");
    let rows: [&[&str]; 12] = [
        &["check", "--root", missing],
        &["check", "--root", missing, "--format", "json"],
        &["pilot", "--root", missing, "--out", &bad_pilot],
        &["first-pr", "--root", missing, "--head", "HEAD"],
        &["doctor", "--root", missing],
        &["init", "--root", missing, "--dry-run"],
        &["explain", "--root", missing, "probe:missing"],
        &["agent", "card", "--root", missing],
        &["outcome", "--before", &before, "--after", &after],
        &[
            "reports",
            "gap-ledger",
            "--root",
            missing,
            "--repo-exposure",
            &exposure,
            "--out",
            &ledger,
        ],
        &["pr-summary", "--root", missing],
        &["pr-evidence", "--root", missing],
    ];
    rows.iter()
        .map(|row| row.iter().map(|arg| (*arg).to_string()).collect())
        .collect()
}

fn measure_bad_input(binary: &Path, scratch: &Path) -> Sample {
    let cwd = scratch.join("bad-input-cwd");
    if let Err(err) = fs::create_dir_all(&cwd) {
        return Sample {
            metric: "trust.false_clean_on_bad_input".to_string(),
            repo: None,
            outcome: SampleOutcome::Failed,
            detail: format!("create {}: {err}", cwd.display()),
        };
    }
    let missing = scratch.join("missing-repository").display().to_string();
    let scratch_text = scratch.display().to_string();
    let probes = bad_input_commands(&missing, &scratch_text);
    let shown = bad_input_commands("<missing>", "<scratch>");
    let mut false_clean = Vec::new();
    for (args, shown) in probes.iter().zip(&shown) {
        match capture_output_measured(
            &binary.display().to_string(),
            args,
            Some(&cwd),
            &[],
            SHORT_TIMEOUT,
            "dx-scoreboard bad-input probe",
        )
        .map(|measured| probe_result(&measured))
        {
            Ok(Probe::ExitedZero) => {
                false_clean.push(format!("`ripr {}`", shown.join(" ")));
            }
            Ok(Probe::TimedOut) => {
                return Sample {
                    metric: "trust.false_clean_on_bad_input".to_string(),
                    repo: None,
                    outcome: SampleOutcome::Failed,
                    detail: format!(
                        "`ripr {}` timed out instead of refusing; a bad input must exit \
                         non-zero promptly",
                        shown.join(" ")
                    ),
                };
            }
            Ok(Probe::Refused) => {}
            Err(err) => {
                return Sample {
                    metric: "trust.false_clean_on_bad_input".to_string(),
                    repo: None,
                    outcome: SampleOutcome::Failed,
                    detail: err,
                };
            }
        }
    }
    let detail = if false_clean.is_empty() {
        format!("all {} probes refused", probes.len())
    } else {
        format!(
            "{} of {} exit 0: {}",
            false_clean.len(),
            probes.len(),
            false_clean.join(", ")
        )
    };
    Sample {
        metric: "trust.false_clean_on_bad_input".to_string(),
        repo: None,
        outcome: SampleOutcome::Value(false_clean.len() as f64),
        detail,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Probe {
    Refused,
    ExitedZero,
    /// A probe that hangs never refused anything.
    TimedOut,
}

pub(crate) fn probe_result(measured: &MeasuredOutput) -> Probe {
    if measured.output.timed_out {
        Probe::TimedOut
    } else if exited_zero(measured) {
        Probe::ExitedZero
    } else {
        Probe::Refused
    }
}

/// Run the hostile-repository journeys (`crates/ripr/tests/hostile_repos.rs`)
/// against the binary under measurement (`RIPR_HOSTILE_BIN`) and count the
/// failing ones. A run that does not report a test result is a failed
/// instrument, not zero failures.
fn measure_hostile_repos(binary: &Path) -> Sample {
    let sample = |outcome: SampleOutcome, detail: String| Sample {
        metric: "trust.hostile_repo_failures".to_string(),
        repo: None,
        outcome,
        detail,
    };
    let args: Vec<String> = ["test", "-p", "ripr", "--test", "hostile_repos"]
        .iter()
        .map(|arg| (*arg).to_string())
        .collect();
    let binary_text = binary.display().to_string();
    let measured = match capture_output_measured(
        "cargo",
        &args,
        None,
        &[("RIPR_HOSTILE_BIN", binary_text.as_str())],
        GIT_TIMEOUT,
        "dx-scoreboard hostile repos",
    ) {
        Ok(measured) => measured,
        Err(err) => return sample(SampleOutcome::Failed, err),
    };
    if measured.output.timed_out {
        return sample(
            SampleOutcome::Failed,
            "cargo test --test hostile_repos timed out".to_string(),
        );
    }
    match parse_test_result(&measured.output.stdout) {
        Some((passed, failed)) => sample(
            SampleOutcome::Value(failed as f64),
            format!(
                "{passed} of {} hostile-input journeys pass",
                passed + failed
            ),
        ),
        None => sample(
            SampleOutcome::Failed,
            format!(
                "cargo test --test hostile_repos printed no test result: {}",
                measured.output.stderr.lines().last().unwrap_or("no output")
            ),
        ),
    }
}

/// `(passed, failed)` from libtest's `test result:` line.
pub(crate) fn parse_test_result(stdout: &str) -> Option<(u64, u64)> {
    let line = stdout
        .lines()
        .rev()
        .find(|line| line.starts_with("test result:"))?;
    let count = |label: &str| -> Option<u64> {
        line.split(';')
            .find(|part| part.trim_end().ends_with(label))?
            .split_whitespace()
            .rev()
            .nth(1)?
            .parse()
            .ok()
    };
    Some((count("passed")?, count("failed")?))
}

// -------------------------------------------------------------- paste ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PasteVerdict {
    Bound,
    Unbound,
    Unsafe,
}

fn measure_paste(binary: &Path, scratch: &Path) -> Vec<Sample> {
    let fail = |detail: String| {
        ["paste.unsafe_commands", "paste.unbound_commands"]
            .iter()
            .map(|metric| Sample {
                metric: (*metric).to_string(),
                repo: None,
                outcome: if cfg!(unix) {
                    SampleOutcome::Failed
                } else {
                    SampleOutcome::NotMeasured
                },
                detail: detail.clone(),
            })
            .collect::<Vec<_>>()
    };
    if !cfg!(unix) {
        return fail("the paste replay uses bash and runs on Unix hosts only".to_string());
    }
    match paste_results(binary, scratch) {
        Ok(results) => {
            let unsafe_lines: Vec<&(String, PasteVerdict, String)> = results
                .iter()
                .filter(|(_, verdict, _)| *verdict == PasteVerdict::Unsafe)
                .collect();
            let unbound: Vec<&(String, PasteVerdict, String)> = results
                .iter()
                .filter(|(_, verdict, _)| *verdict == PasteVerdict::Unbound)
                .collect();
            let describe = |rows: &[&(String, PasteVerdict, String)]| {
                rows.iter()
                    .take(4)
                    .map(|(line, _, why)| format!("`{}` ({why})", shorten(line)))
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            vec![
                Sample {
                    metric: "paste.unsafe_commands".to_string(),
                    repo: None,
                    outcome: SampleOutcome::Value(unsafe_lines.len() as f64),
                    detail: format!(
                        "{} of {} printed commands replayed under bash{}{}",
                        unsafe_lines.len(),
                        results.len(),
                        if unsafe_lines.is_empty() { "" } else { ": " },
                        describe(&unsafe_lines)
                    ),
                },
                Sample {
                    metric: "paste.unbound_commands".to_string(),
                    repo: None,
                    outcome: SampleOutcome::Value(unbound.len() as f64),
                    detail: format!(
                        "{} of {} printed commands drop the root{}{}",
                        unbound.len(),
                        results.len(),
                        if unbound.is_empty() { "" } else { ": " },
                        describe(&unbound)
                    ),
                },
            ]
        }
        Err(err) => fail(err),
    }
}

fn paste_results(
    binary: &Path,
    scratch: &Path,
) -> Result<Vec<(String, PasteVerdict, String)>, String> {
    let base = scratch.join("paste");
    let root = base.join(HOSTILE_DIR);
    write_paste_fixture(&root)?;
    let root_text = root.display().to_string();
    let caller = base.join("caller");
    fs::create_dir_all(&caller).map_err(|err| format!("create {}: {err}", caller.display()))?;
    let cache = base.join("cache").display().to_string();
    let envs = [(CACHE_ENV, cache.as_str())];

    let pilot_out = root.join("target/ripr/pilot");
    let runs: Vec<Vec<String>> = vec![
        vec!["check", "--root", &root_text, "--base", "HEAD~1"],
        vec![
            "check",
            "--root",
            &root_text,
            "--base",
            "HEAD~1",
            "--format",
            "human-full",
        ],
        vec![
            "pilot",
            "--root",
            &root_text,
            "--out",
            &pilot_out.display().to_string(),
            "--quiet",
        ],
        vec![
            "first-pr", "--root", &root_text, "--base", "HEAD~1", "--head", "HEAD",
        ],
        vec!["doctor", "--root", &root_text],
    ]
    .into_iter()
    .map(|row| row.into_iter().map(str::to_string).collect())
    .collect();

    let mut lines = BTreeSet::new();
    for args in &runs {
        let measured = capture_output_measured(
            &binary.display().to_string(),
            args,
            Some(&caller),
            &envs,
            SHORT_TIMEOUT,
            "dx-scoreboard paste source",
        )?;
        lines.extend(extract_commands(&measured.output.stdout));
        lines.extend(extract_commands(&measured.output.stderr));
    }
    for artifact in markdown_files(&root.join("target/ripr")) {
        if let Ok(text) = fs::read_to_string(&artifact) {
            lines.extend(extract_commands(&text));
        }
    }
    if lines.is_empty() {
        return Err("no printed ripr commands found to replay".to_string());
    }
    for dir in ["reports", "pilot", "pr", "agent", "review"] {
        let _ = fs::create_dir_all(root.join("target/ripr").join(dir));
    }

    let mut results = Vec::new();
    for (index, line) in lines.into_iter().enumerate() {
        let (verdict, why) = replay(&line, &root, &base, index)?;
        results.push((line, verdict, why));
    }
    Ok(results)
}

/// Replay one printed line under bash with `ripr` replaced by a recorder
/// function, from a caller directory outside the repository.
fn replay(
    line: &str,
    root: &Path,
    base: &Path,
    index: usize,
) -> Result<(PasteVerdict, String), String> {
    let caller = base.join(format!("replay-{index}"));
    fs::create_dir_all(&caller).map_err(|err| format!("create {}: {err}", caller.display()))?;
    let argv_file = base.join(format!("argv-{index}"));
    let cwd_file = base.join(format!("cwd-{index}"));
    let script = base.join(format!("replay-{index}.sh"));
    let body = format!(
        "ripr() {{ printf '%s\\0' \"$@\" > {argv}; pwd -P > {cwd}; }}\n{line}\n",
        argv = single_quote(&argv_file.display().to_string()),
        cwd = single_quote(&cwd_file.display().to_string()),
    );
    fs::write(&script, body).map_err(|err| format!("write {}: {err}", script.display()))?;
    let measured = capture_output_measured(
        "bash",
        &[script.display().to_string()],
        Some(&caller),
        &[],
        SHORT_TIMEOUT,
        "dx-scoreboard paste replay",
    )?;
    let canary = [caller.join(CANARY), root.join(CANARY), base.join(CANARY)]
        .into_iter()
        .find(|path| path.exists());
    if let Some(path) = canary {
        let _ = fs::remove_file(&path);
        return Ok((PasteVerdict::Unsafe, "ran the injected command".to_string()));
    }
    let argv = fs::read(&argv_file).ok();
    let cwd = fs::read_to_string(&cwd_file).ok();
    let root_text = fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .display()
        .to_string();
    Ok(classify_replay(
        exited_zero(&measured),
        argv.as_deref(),
        cwd.as_deref(),
        &root_text,
    ))
}

pub(crate) fn classify_replay(
    exit_ok: bool,
    argv: Option<&[u8]>,
    cwd: Option<&str>,
    root: &str,
) -> (PasteVerdict, String) {
    let Some(argv) = argv else {
        return (
            PasteVerdict::Unsafe,
            if exit_ok {
                "never reached ripr".to_string()
            } else {
                "bash rejected the line".to_string()
            },
        );
    };
    let args: Vec<String> = argv
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect();
    let under_root = |value: &str| {
        value == root
            || value.starts_with(&format!("{root}/"))
            || value.strip_prefix("--root=") == Some(root)
    };
    let cwd = cwd.map(str::trim).unwrap_or_default();
    if args.iter().any(|arg| under_root(arg)) || under_root(cwd) {
        if !exit_ok {
            return (
                PasteVerdict::Unbound,
                "reached ripr with the root intact, but the pasted line then failed".to_string(),
            );
        }
        return (PasteVerdict::Bound, "root intact".to_string());
    }
    // A `--root` whose value is not the root, or a fragment of the hostile
    // directory name, means the shell split or rewrote the path.
    let root_flag_rewritten = args
        .windows(2)
        .any(|pair| pair.first().is_some_and(|flag| flag == "--root"));
    if root_flag_rewritten || args.iter().any(|arg| arg.contains("dx paste")) {
        return (
            PasteVerdict::Unsafe,
            "root path was split or rewritten".to_string(),
        );
    }
    (
        PasteVerdict::Unbound,
        "no root argument and run from the caller directory".to_string(),
    )
}

/// Pull pasteable `ripr …` invocations out of human output or Markdown:
/// backtick spans, fenced bash blocks, and lines (or label suffixes) that
/// start an invocation with an option. PowerShell forms are skipped; this
/// replay checks bash. Prose that merely mentions ripr has no `--` option
/// and is ignored.
pub(crate) fn extract_commands(text: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut fence: Option<String> = None;
    for raw in text.lines() {
        let trimmed = raw.trim();
        if let Some(info) = trimmed.strip_prefix("```") {
            fence = match fence {
                Some(_) => None,
                None => Some(info.trim().to_ascii_lowercase()),
            };
            continue;
        }
        let in_powershell = fence.as_deref().is_some_and(|info| {
            ["powershell", "pwsh", "ps1"]
                .iter()
                .any(|tag| info.contains(tag))
        });
        if in_powershell || raw.contains("PowerShell") {
            continue;
        }
        if fence.is_none() && trimmed.contains('`') {
            for (index, span) in trimmed.split('`').enumerate() {
                if index % 2 == 1 && is_invocation(span) {
                    commands.push(span.trim().to_string());
                }
            }
            continue;
        }
        if let Some(start) = invocation_start(trimmed) {
            let candidate = trimmed[start..].trim();
            if is_invocation(candidate) {
                commands.push(candidate.to_string());
            }
        }
    }
    commands
}

fn invocation_start(line: &str) -> Option<usize> {
    let mut offset = 0;
    while let Some(found) = line[offset..].find("ripr ") {
        let index = offset + found;
        let boundary = index == 0
            || line[..index]
                .chars()
                .last()
                .is_some_and(|c| c.is_whitespace() || c == ':' || c == '$');
        if boundary && is_invocation(&line[index..]) {
            return Some(index);
        }
        offset = index + "ripr ".len();
    }
    None
}

fn is_invocation(candidate: &str) -> bool {
    let candidate = candidate.trim();
    let Some(rest) = candidate.strip_prefix("ripr ") else {
        return false;
    };
    let word = rest.split_whitespace().next().unwrap_or_default();
    COMMAND_WORDS.contains(&word) && rest.contains(" --")
}

fn markdown_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        let Ok(entries) = fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "md") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

fn write_paste_fixture(root: &Path) -> Result<(), String> {
    write_tiny_crate(root)?;
    let tests = root.join("tests");
    fs::create_dir_all(&tests).map_err(|err| format!("create {}: {err}", tests.display()))?;
    fs::write(
        tests.join("discount.rs"),
        "#[test]\nfn discount_runs() {\n    let _ = dxfixture::discount(150);\n}\n",
    )
    .map_err(|err| format!("write fixture test: {err}"))?;
    git(Some(root), &["init", "--quiet"])?;
    for args in [
        ["config", "user.email", "dx-scoreboard@example.invalid"],
        ["config", "user.name", "dx-scoreboard"],
        ["config", "commit.gpgsign", "false"],
    ] {
        git(Some(root), &args)?;
    }
    git(Some(root), &["add", "-A"])?;
    git(Some(root), &["commit", "--quiet", "-m", "one"])?;
    fs::write(
        root.join("src/lib.rs"),
        "pub fn discount(total: u32) -> u32 {\n    if total >= 100 { total - 10 } else { total }\n}\n",
    )
    .map_err(|err| format!("write fixture change: {err}"))?;
    git(Some(root), &["commit", "--quiet", "-am", "two"])?;
    Ok(())
}

fn write_tiny_crate(root: &Path) -> Result<(), String> {
    let src = root.join("src");
    fs::create_dir_all(&src).map_err(|err| format!("create {}: {err}", src.display()))?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dxfixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .map_err(|err| format!("write fixture manifest: {err}"))?;
    fs::write(
        src.join("lib.rs"),
        "pub fn discount(total: u32) -> u32 {\n    if total > 100 { total - 10 } else { total }\n}\n",
    )
    .map_err(|err| format!("write fixture source: {err}"))?;
    Ok(())
}

// ------------------------------------------------------------ helpers ----

pub(crate) fn rss_sample(
    sample: &dyn Fn(&str, SampleOutcome, String) -> Sample,
    metric: &str,
    measured: &MeasuredOutput,
    complete: bool,
) -> Sample {
    match measured.peak_rss_bytes {
        // The peak of a run that stopped early is not the workload's peak.
        Some(bytes) => {
            let mb = bytes as f64 / (1024.0 * 1024.0);
            if complete {
                sample(
                    metric,
                    SampleOutcome::Value(mb),
                    "VmHWM sampled every 10 ms".to_string(),
                )
            } else {
                sample(
                    metric,
                    SampleOutcome::Incomplete(mb),
                    "VmHWM of a run that did not complete".to_string(),
                )
            }
        }
        None => sample(
            metric,
            SampleOutcome::NotMeasured,
            "peak memory sampling needs Linux /proc".to_string(),
        ),
    }
}

fn exited_zero(measured: &MeasuredOutput) -> bool {
    !measured.output.timed_out
        && measured
            .output
            .status
            .is_some_and(|status| status.success())
}

fn exit_label(measured: &MeasuredOutput) -> String {
    if measured.output.timed_out {
        return "timeout".to_string();
    }
    measured
        .output
        .status
        .and_then(|status| status.code())
        .map_or_else(|| "signal".to_string(), |code| code.to_string())
}

fn duration_ms(measured: &MeasuredOutput) -> f64 {
    measured.output.duration.as_secs_f64() * 1000.0
}

fn read_json_file(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn shorten(line: &str) -> String {
    const LIMIT: usize = 120;
    if line.chars().count() <= LIMIT {
        line.to_string()
    } else {
        format!("{}…", line.chars().take(LIMIT).collect::<String>())
    }
}

fn release_binary() -> PathBuf {
    let binary_name = format!("ripr{}", std::env::consts::EXE_SUFFIX);
    std::env::var_os("CARGO_TARGET_DIR")
        .map_or_else(|| PathBuf::from("target"), PathBuf::from)
        .join("release")
        .join(binary_name)
}

fn git_revision() -> String {
    crate::run::run_output("git", &["rev-parse", "HEAD"])
        .ok()
        .map(|output| output.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unavailable".to_string())
}

fn analyzer_version(binary: &Path) -> String {
    capture_output_measured(
        &binary.display().to_string(),
        &["--version".to_string()],
        None,
        &[],
        SHORT_TIMEOUT,
        "ripr --version",
    )
    .ok()
    .map(|measured| measured.output.stdout.trim().to_string())
    .filter(|value| !value.is_empty())
    .unwrap_or_else(|| "unavailable".to_string())
}

/// Runner class keys runner-dependent comparisons. `RIPR_DX_RUNNER_CLASS`
/// overrides it for a dedicated benchmark host.
pub(crate) fn runner_class() -> String {
    if let Ok(class) = std::env::var("RIPR_DX_RUNNER_CLASS")
        && !class.trim().is_empty()
    {
        return class.trim().to_string();
    }
    let cpus = std::thread::available_parallelism().map_or(0, std::num::NonZeroUsize::get);
    let host = if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true") {
        std::env::var("RUNNER_ENVIRONMENT").unwrap_or_else(|_| "github-hosted".to_string())
    } else {
        "local".to_string()
    };
    format!(
        "{host}-{}-{}-{cpus}cpu",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}
