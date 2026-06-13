//! Tier A external-repo eval sweep — RIPR-SPEC-0086.
//!
//! Report-only `cargo xtask eval-sweep`. Runs `ripr check` over a pinned
//! manifest of real external Python repos and records only machine-checkable
//! robustness facts: crash rate, parse-failure rate, runtime, and gap-ID
//! stability across a re-run. It does not judge actionability (that is Tier B).
//!
//! Cloning external repos is opt-in (`--clone`) and off the default CI path; all
//! subprocess work routes through the allowlisted `crate::run` helpers.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

use crate::run;

const DEFAULT_MANIFEST: &str = "fixtures/python-eval-sweep/manifest.json";
const DEFAULT_CHECKOUT_ROOT: &str = "target/ripr/eval-sweep/checkouts";
const DEFAULT_TIMEOUT_SECS: u64 = 120;
const REPORT_JSON: &str = "eval-sweep.json";
const REPORT_MD: &str = "eval-sweep.md";
const SCHEMA_VERSION: &str = "0.1";
const STDERR_EXCERPT_LIMIT: usize = 280;

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct SweepArgs {
    manifest: String,
    clone: bool,
    checkout_root: String,
    only_repo: Option<String>,
    timeout: Duration,
    json_only: bool,
}

impl SweepArgs {
    fn defaults() -> Self {
        Self {
            manifest: DEFAULT_MANIFEST.to_string(),
            clone: false,
            checkout_root: DEFAULT_CHECKOUT_ROOT.to_string(),
            only_repo: None,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            json_only: false,
        }
    }
}

fn parse_args(args: &[String]) -> Result<SweepArgs, String> {
    let mut parsed = SweepArgs::defaults();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => parsed.manifest = take_value(args, &mut index, "--manifest")?,
            "--clone" => parsed.clone = true,
            "--checkout-root" => {
                parsed.checkout_root = take_value(args, &mut index, "--checkout-root")?
            }
            "--repo" => parsed.only_repo = Some(take_value(args, &mut index, "--repo")?),
            "--timeout-secs" => {
                let raw = take_value(args, &mut index, "--timeout-secs")?;
                let secs = raw.parse::<u64>().map_err(|err| {
                    format!("eval-sweep --timeout-secs expects an integer, got `{raw}`: {err}")
                })?;
                parsed.timeout = Duration::from_secs(secs);
            }
            "--json-only" => parsed.json_only = true,
            other => return Err(format!("unknown eval-sweep argument: {other}")),
        }
        index += 1;
    }
    Ok(parsed)
}

fn take_value(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("eval-sweep {flag} requires a value"))
}

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

struct RepoEntry {
    id: String,
    url: String,
    sha: String,
    shape: String,
    synthetic_diff: Option<String>,
}

struct Manifest {
    synthetic_diff: Option<String>,
    repos: Vec<RepoEntry>,
}

fn load_manifest(path: &str) -> Result<Manifest, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read manifest {path}: {err}"))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse manifest {path}: {err}"))?;
    parse_manifest(&value)
}

fn parse_manifest(value: &Value) -> Result<Manifest, String> {
    let top_diff = value
        .get("synthetic_diff")
        .and_then(Value::as_str)
        .map(str::to_string);
    let repos_val = value
        .get("repos")
        .and_then(Value::as_array)
        .ok_or_else(|| "manifest must contain a `repos` array".to_string())?;
    if repos_val.is_empty() {
        return Err("manifest `repos` must not be empty".to_string());
    }
    let mut repos = Vec::new();
    let mut seen = BTreeSet::new();
    for repo in repos_val {
        let id = string_field(repo, "id")?;
        if !seen.insert(id.clone()) {
            return Err(format!("manifest has duplicate repo id `{id}`"));
        }
        let url = string_field(repo, "url")?;
        if !url.starts_with("https://") {
            return Err(format!("repo `{id}` url must be https, got `{url}`"));
        }
        let sha = string_field(repo, "sha")?;
        let shape = string_field(repo, "shape")?;
        let synthetic_diff = repo
            .get("synthetic_diff")
            .and_then(Value::as_str)
            .map(str::to_string);
        if synthetic_diff.is_none() && top_diff.is_none() {
            return Err(format!(
                "repo `{id}` has no synthetic_diff and the manifest has no top-level synthetic_diff"
            ));
        }
        repos.push(RepoEntry {
            id,
            url,
            sha,
            shape,
            synthetic_diff,
        });
    }
    Ok(Manifest {
        synthetic_diff: top_diff,
        repos,
    })
}

fn string_field(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("manifest repo entry missing non-empty string field `{key}`"))
}

// ---------------------------------------------------------------------------
// Outcome classification (pure)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Ok,
    ParseFailure,
    TimedOut,
    Crash,
    CloneFailed,
    SkippedMissingCheckout,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::Ok => "ok",
            Outcome::ParseFailure => "parse_failure",
            Outcome::TimedOut => "timed_out",
            Outcome::Crash => "crash",
            Outcome::CloneFailed => "clone_failed",
            Outcome::SkippedMissingCheckout => "skipped_missing_checkout",
        }
    }

    fn counts_as_run(self) -> bool {
        !matches!(self, Outcome::CloneFailed | Outcome::SkippedMissingCheckout)
    }
}

/// Classify the result of one `ripr check` invocation. `parsed` is `None` when
/// stdout was not well-formed JSON (treated as a crash).
fn classify(timed_out: bool, parsed: Option<&Value>) -> Outcome {
    if timed_out {
        return Outcome::TimedOut;
    }
    match parsed {
        Some(value) if findings_have_parse_failure(value) => Outcome::ParseFailure,
        Some(_) => Outcome::Ok,
        None => Outcome::Crash,
    }
}

fn findings_have_parse_failure(value: &Value) -> bool {
    let Some(findings) = value.get("findings").and_then(Value::as_array) else {
        return false;
    };
    findings.iter().any(|finding| {
        let class_unknown = finding
            .get("class")
            .and_then(Value::as_str)
            .is_some_and(|class| class == "static_unknown");
        let unsupported_limit = finding
            .get("static_limit_kind")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind == "unsupported_syntax");
        class_unknown || unsupported_limit
    })
}

fn gap_ids(value: &Value) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let Some(findings) = value.get("findings").and_then(Value::as_array) else {
        return set;
    };
    for finding in findings {
        if let Some(id) = finding
            .get("canonical_gap_id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            set.insert(id.to_string());
        }
        if let Some(id) = finding
            .get("canonical_gap")
            .and_then(|gap| gap.get("id"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            set.insert(id.to_string());
        }
    }
    set
}

// ---------------------------------------------------------------------------
// Per-repo run (orchestration; shells out)
// ---------------------------------------------------------------------------

struct RepoRun {
    id: String,
    sha: String,
    shape: String,
    outcome: Outcome,
    runtime_ms: u128,
    gap_ids: BTreeSet<String>,
    gap_ids_stable: bool,
    unstable_gap_ids: Vec<String>,
    stderr_excerpt: String,
}

impl RepoRun {
    fn terminal(entry: &RepoEntry, outcome: Outcome, stderr: String) -> Self {
        Self {
            id: entry.id.clone(),
            sha: entry.sha.clone(),
            shape: entry.shape.clone(),
            outcome,
            runtime_ms: 0,
            gap_ids: BTreeSet::new(),
            gap_ids_stable: true,
            unstable_gap_ids: Vec::new(),
            stderr_excerpt: excerpt(&stderr),
        }
    }
}

fn run_repo(entry: &RepoEntry, manifest: &Manifest, args: &SweepArgs) -> RepoRun {
    let checkout = PathBuf::from(&args.checkout_root).join(&entry.id);
    if args.clone {
        if let Err(err) = clone_repo(entry, &checkout) {
            return RepoRun::terminal(entry, Outcome::CloneFailed, err);
        }
    } else if !checkout.exists() {
        return RepoRun::terminal(entry, Outcome::SkippedMissingCheckout, String::new());
    }

    let diff = entry
        .synthetic_diff
        .as_deref()
        .or(manifest.synthetic_diff.as_deref())
        .unwrap_or_default()
        .to_string();

    let first = run_check(&checkout, &diff, args);
    let second_gap_ids = if first.outcome.counts_as_run() {
        run_check(&checkout, &diff, args).gap_ids
    } else {
        first.gap_ids.clone()
    };

    let stable = first.gap_ids == second_gap_ids;
    let unstable = first
        .gap_ids
        .symmetric_difference(&second_gap_ids)
        .cloned()
        .collect();

    RepoRun {
        id: entry.id.clone(),
        sha: entry.sha.clone(),
        shape: entry.shape.clone(),
        outcome: first.outcome,
        runtime_ms: first.runtime_ms,
        gap_ids: first.gap_ids,
        gap_ids_stable: stable,
        unstable_gap_ids: unstable,
        stderr_excerpt: excerpt(&first.stderr),
    }
}

struct CheckRun {
    outcome: Outcome,
    gap_ids: BTreeSet<String>,
    runtime_ms: u128,
    stderr: String,
}

fn run_check(checkout: &Path, diff: &str, args: &SweepArgs) -> CheckRun {
    let root = checkout.to_string_lossy().to_string();
    let cargo_args: Vec<String> = vec![
        "run".to_string(),
        "-p".to_string(),
        "ripr".to_string(),
        "--quiet".to_string(),
        "--".to_string(),
        "check".to_string(),
        "--root".to_string(),
        root,
        "--diff".to_string(),
        diff.to_string(),
        "--mode".to_string(),
        "fast".to_string(),
        "--json".to_string(),
    ];
    match run::capture_output_with_timeout(
        "cargo",
        &cargo_args,
        &[],
        args.timeout,
        "eval-sweep ripr check",
    ) {
        Ok(out) => {
            let runtime_ms = out.duration.as_millis();
            if out.timed_out {
                return CheckRun {
                    outcome: Outcome::TimedOut,
                    gap_ids: BTreeSet::new(),
                    runtime_ms,
                    stderr: out.stderr,
                };
            }
            let parsed = serde_json::from_str::<Value>(&out.stdout).ok();
            let outcome = classify(false, parsed.as_ref());
            let gaps = parsed.as_ref().map(gap_ids).unwrap_or_default();
            CheckRun {
                outcome,
                gap_ids: gaps,
                runtime_ms,
                stderr: out.stderr,
            }
        }
        Err(err) => CheckRun {
            outcome: Outcome::Crash,
            gap_ids: BTreeSet::new(),
            runtime_ms: 0,
            stderr: err,
        },
    }
}

fn clone_repo(entry: &RepoEntry, checkout: &Path) -> Result<(), String> {
    if checkout.exists() {
        return Ok(());
    }
    if let Some(parent) = checkout.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create checkout root for `{}`: {err}", entry.id))?;
    }
    let dir = checkout.to_string_lossy().to_string();
    run::run_with_envs(
        "git",
        &["clone", "--filter=blob:none", &entry.url, &dir],
        &[],
    )
    .map_err(|err| format!("clone of `{}` failed: {err}", entry.id))?;
    run::run_with_envs("git", &["-C", &dir, "checkout", &entry.sha], &[])
        .map_err(|err| format!("checkout of `{}`@{} failed: {err}", entry.id, entry.sha))?;
    Ok(())
}

fn excerpt(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= STDERR_EXCERPT_LIMIT {
        trimmed.to_string()
    } else {
        format!("{}…", &trimmed[..STDERR_EXCERPT_LIMIT])
    }
}

// ---------------------------------------------------------------------------
// Metrics (pure)
// ---------------------------------------------------------------------------

struct Metrics {
    repos_total: usize,
    repos_run: usize,
    repos_skipped: usize,
    repos_clone_failed: usize,
    crash_count: usize,
    parse_failure_count: usize,
    timed_out_count: usize,
    runtime_ms_min: u128,
    runtime_ms_median: u128,
    runtime_ms_max: u128,
    runtime_ms_total: u128,
    gap_id_stable_count: usize,
    gap_id_unstable_count: usize,
    crash_rate: f64,
    parse_failure_rate: f64,
    gap_id_stability_rate: f64,
    gate_status: &'static str,
    gate_reason: String,
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn compute_metrics(runs: &[RepoRun]) -> Metrics {
    let repos_total = runs.len();
    let run_set: Vec<&RepoRun> = runs.iter().filter(|r| r.outcome.counts_as_run()).collect();
    let repos_run = run_set.len();
    let repos_skipped = runs
        .iter()
        .filter(|r| r.outcome == Outcome::SkippedMissingCheckout)
        .count();
    let repos_clone_failed = runs
        .iter()
        .filter(|r| r.outcome == Outcome::CloneFailed)
        .count();
    let crash_count = run_set
        .iter()
        .filter(|r| r.outcome == Outcome::Crash)
        .count();
    let parse_failure_count = run_set
        .iter()
        .filter(|r| r.outcome == Outcome::ParseFailure)
        .count();
    let timed_out_count = run_set
        .iter()
        .filter(|r| r.outcome == Outcome::TimedOut)
        .count();

    let mut runtimes: Vec<u128> = run_set.iter().map(|r| r.runtime_ms).collect();
    runtimes.sort_unstable();
    let runtime_ms_total: u128 = runtimes.iter().sum();
    let (runtime_ms_min, runtime_ms_median, runtime_ms_max) = if runtimes.is_empty() {
        (0, 0, 0)
    } else {
        (
            runtimes[0],
            runtimes[runtimes.len() / 2],
            runtimes[runtimes.len() - 1],
        )
    };

    let gap_id_stable_count = run_set.iter().filter(|r| r.gap_ids_stable).count();
    let gap_id_unstable_count = repos_run.saturating_sub(gap_id_stable_count);

    let crash_rate = ratio(crash_count, repos_run);
    let parse_failure_rate = ratio(parse_failure_count, repos_run);
    let gap_id_stability_rate = if repos_run == 0 {
        1.0
    } else {
        gap_id_stable_count as f64 / repos_run as f64
    };

    // A pass/fail gate is only meaningful once at least one repo was analyzed.
    // Zero analyzed repos is `not_run`, never a vacuous `pass`.
    let (gate_status, gate_reason) = if repos_run == 0 {
        (
            "not_run",
            format!(
                "no repos analyzed ({repos_total} total, {repos_skipped} skipped, {repos_clone_failed} clone-failed); pass/review requires at least one analyzed repo (use --clone or pre-place checkouts)"
            ),
        )
    } else if crash_count == 0 && gap_id_unstable_count == 0 {
        (
            "pass",
            format!(
                "{repos_run} repo(s) analyzed; no crashes; canonical gap IDs stable across the re-run"
            ),
        )
    } else {
        (
            "review",
            format!(
                "{crash_count} crash(es) and {gap_id_unstable_count} unstable gap-ID set(s) over {repos_run} repo(s); investigate before promotion"
            ),
        )
    };

    Metrics {
        repos_total,
        repos_run,
        repos_skipped,
        repos_clone_failed,
        crash_count,
        parse_failure_count,
        timed_out_count,
        runtime_ms_min,
        runtime_ms_median,
        runtime_ms_max,
        runtime_ms_total,
        gap_id_stable_count,
        gap_id_unstable_count,
        crash_rate,
        parse_failure_rate,
        gap_id_stability_rate,
        gate_status,
        gate_reason,
    }
}

// ---------------------------------------------------------------------------
// Rendering (pure)
// ---------------------------------------------------------------------------

fn render_json(metrics: &Metrics, runs: &[RepoRun]) -> Result<String, String> {
    let repos: Vec<Value> = runs
        .iter()
        .map(|run| {
            json!({
                "id": run.id,
                "sha": run.sha,
                "shape": run.shape,
                "outcome": run.outcome.as_str(),
                "runtime_ms": run.runtime_ms,
                "gap_ids": run.gap_ids.iter().cloned().collect::<Vec<_>>(),
                "gap_ids_stable": run.gap_ids_stable,
                "unstable_gap_ids": run.unstable_gap_ids,
                "stderr_excerpt": run.stderr_excerpt,
            })
        })
        .collect();

    let document = json!({
        "schema_version": SCHEMA_VERSION,
        "kind": "python_eval_sweep_report",
        "spec": "RIPR-SPEC-0086",
        "tier": "A",
        "summary": {
            "repos_total": metrics.repos_total,
            "repos_run": metrics.repos_run,
            "repos_skipped": metrics.repos_skipped,
            "repos_clone_failed": metrics.repos_clone_failed,
            "crash_count": metrics.crash_count,
            "crash_rate": metrics.crash_rate,
            "parse_failure_count": metrics.parse_failure_count,
            "parse_failure_rate": metrics.parse_failure_rate,
            "timed_out_count": metrics.timed_out_count,
            "runtime_ms_min": metrics.runtime_ms_min,
            "runtime_ms_median": metrics.runtime_ms_median,
            "runtime_ms_max": metrics.runtime_ms_max,
            "runtime_ms_total": metrics.runtime_ms_total,
            "gap_id_stable_count": metrics.gap_id_stable_count,
            "gap_id_unstable_count": metrics.gap_id_unstable_count,
            "gap_id_stability_rate": metrics.gap_id_stability_rate,
            "gate_status": metrics.gate_status,
            "gate_reason": metrics.gate_reason,
        },
        "repos": repos,
    });

    serde_json::to_string_pretty(&document)
        .map_err(|err| format!("failed to render eval-sweep JSON: {err}"))
}

fn render_markdown(metrics: &Metrics, runs: &[RepoRun]) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Python Tier A Eval Sweep\n\n");
    out.push_str(&format!("Gate: **{}**\n\n", metrics.gate_status));
    out.push_str(&format!("> {}\n\n", metrics.gate_reason));
    out.push_str("## Summary\n\n");
    out.push_str(&format!(
        "- repos: {} total, {} run, {} skipped, {} clone-failed\n",
        metrics.repos_total, metrics.repos_run, metrics.repos_skipped, metrics.repos_clone_failed
    ));
    out.push_str(&format!(
        "- crash rate: {:.3} ({} crash)\n",
        metrics.crash_rate, metrics.crash_count
    ));
    out.push_str(&format!(
        "- parse-failure rate: {:.3} ({} repos)\n",
        metrics.parse_failure_rate, metrics.parse_failure_count
    ));
    out.push_str(&format!("- timed out: {}\n", metrics.timed_out_count));
    out.push_str(&format!(
        "- gap-ID stability: {:.3} ({}/{} stable)\n",
        metrics.gap_id_stability_rate, metrics.gap_id_stable_count, metrics.repos_run
    ));
    out.push_str(&format!(
        "- runtime ms (min/median/max): {}/{}/{}\n\n",
        metrics.runtime_ms_min, metrics.runtime_ms_median, metrics.runtime_ms_max
    ));

    out.push_str("## Repos\n\n");
    out.push_str("| id | shape | outcome | runtime_ms | gap_ids | stable |\n");
    out.push_str("| --- | --- | --- | ---: | ---: | --- |\n");
    for run in runs {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            run.id,
            run.shape,
            run.outcome.as_str(),
            run.runtime_ms,
            run.gap_ids.len(),
            if run.gap_ids_stable { "yes" } else { "NO" },
        ));
    }
    out.push('\n');
    out
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub(crate) fn eval_sweep(args: &[String]) -> Result<(), String> {
    let parsed = parse_args(args)?;
    let manifest = load_manifest(&parsed.manifest)?;

    let mut runs = Vec::new();
    for entry in &manifest.repos {
        if let Some(only) = &parsed.only_repo
            && &entry.id != only
        {
            continue;
        }
        runs.push(run_repo(entry, &manifest, &parsed));
    }

    let metrics = compute_metrics(&runs);
    let document = render_json(&metrics, &runs)?;
    crate::write_report(REPORT_JSON, &format!("{document}\n"))?;
    if !parsed.json_only {
        crate::write_report(REPORT_MD, &render_markdown(&metrics, &runs))?;
    }
    println!(
        "eval-sweep: {} repos, {} run, gate={}",
        metrics.repos_total, metrics.repos_run, metrics.gate_status
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests (no unwrap/expect/panic; assert macros only)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn good_manifest() -> Value {
        json!({
            "synthetic_diff": "fixtures/python-eval-sweep/synthetic-diff.diff",
            "repos": [
                { "id": "a", "url": "https://example.com/a", "sha": "deadbeef", "license": "MIT", "shape": "pytest_library" }
            ]
        })
    }

    #[test]
    fn manifest_load_accepts_valid() {
        let parsed = parse_manifest(&good_manifest());
        assert!(parsed.is_ok());
        if let Ok(manifest) = parsed {
            assert_eq!(manifest.repos.len(), 1);
        }
    }

    #[test]
    fn manifest_load_rejects_invalid() {
        let empty = json!({ "repos": [] });
        assert!(parse_manifest(&empty).is_err());

        let dup = json!({
            "synthetic_diff": "d.diff",
            "repos": [
                { "id": "a", "url": "https://x/a", "sha": "1", "license": "MIT", "shape": "s" },
                { "id": "a", "url": "https://x/a2", "sha": "2", "license": "MIT", "shape": "s" }
            ]
        });
        assert!(parse_manifest(&dup).is_err());

        let non_https = json!({
            "synthetic_diff": "d.diff",
            "repos": [{ "id": "a", "url": "http://x/a", "sha": "1", "license": "MIT", "shape": "s" }]
        });
        assert!(parse_manifest(&non_https).is_err());

        let no_diff = json!({
            "repos": [{ "id": "a", "url": "https://x/a", "sha": "1", "license": "MIT", "shape": "s" }]
        });
        assert!(parse_manifest(&no_diff).is_err());
    }

    #[test]
    fn classifier_maps_outcomes() {
        assert!(classify(true, None) == Outcome::TimedOut);
        assert!(classify(false, None) == Outcome::Crash);
        let ok = json!({ "findings": [{ "canonical_gap": { "id": "gap:python:a" } }] });
        assert!(classify(false, Some(&ok)) == Outcome::Ok);
        let pf = json!({ "findings": [{ "class": "static_unknown" }] });
        assert!(classify(false, Some(&pf)) == Outcome::ParseFailure);
        let pf2 = json!({ "findings": [{ "static_limit_kind": "unsupported_syntax" }] });
        assert!(classify(false, Some(&pf2)) == Outcome::ParseFailure);
    }

    #[test]
    fn gap_ids_collects_both_shapes() {
        let value = json!({
            "findings": [
                { "canonical_gap": { "id": "gap:python:a" } },
                { "canonical_gap_id": "gap:python:b" },
                { "canonical_gap_id": "" }
            ]
        });
        let ids = gap_ids(&value);
        assert_eq!(ids.len(), 2);
        assert!(ids.contains("gap:python:a"));
        assert!(ids.contains("gap:python:b"));
    }

    #[test]
    fn gap_id_instability_detected() {
        let a: BTreeSet<String> = ["gap:1".to_string(), "gap:2".to_string()]
            .into_iter()
            .collect();
        let b: BTreeSet<String> = ["gap:1".to_string()].into_iter().collect();
        assert!(a != b);
        let unstable: Vec<String> = a.symmetric_difference(&b).cloned().collect();
        assert_eq!(unstable, vec!["gap:2".to_string()]);
    }

    fn run_with(outcome: Outcome, runtime_ms: u128, stable: bool) -> RepoRun {
        RepoRun {
            id: "r".to_string(),
            sha: "s".to_string(),
            shape: "pytest_library".to_string(),
            outcome,
            runtime_ms,
            gap_ids: BTreeSet::new(),
            gap_ids_stable: stable,
            unstable_gap_ids: Vec::new(),
            stderr_excerpt: String::new(),
        }
    }

    #[test]
    fn metrics_guard_empty_run_set() {
        let metrics = compute_metrics(&[]);
        assert_eq!(metrics.repos_run, 0);
        assert!((metrics.crash_rate - 0.0).abs() < f64::EPSILON);
        assert!((metrics.gap_id_stability_rate - 1.0).abs() < f64::EPSILON);
        // Zero analyzed repos must never read as a vacuous pass.
        assert_eq!(metrics.gate_status, "not_run");
    }

    #[test]
    fn metrics_all_skipped_is_not_run() {
        let runs = vec![
            run_with(Outcome::SkippedMissingCheckout, 0, true),
            run_with(Outcome::SkippedMissingCheckout, 0, true),
        ];
        let metrics = compute_metrics(&runs);
        assert_eq!(metrics.repos_total, 2);
        assert_eq!(metrics.repos_run, 0);
        assert_eq!(metrics.gate_status, "not_run");
    }

    #[test]
    fn metrics_gate_review_on_crash() {
        let runs = vec![
            run_with(Outcome::Ok, 100, true),
            run_with(Outcome::Crash, 0, true),
        ];
        let metrics = compute_metrics(&runs);
        assert_eq!(metrics.repos_run, 2);
        assert_eq!(metrics.crash_count, 1);
        assert_eq!(metrics.gate_status, "review");
    }

    #[test]
    fn metrics_skipped_excluded_from_run() {
        let runs = vec![
            run_with(Outcome::Ok, 50, true),
            run_with(Outcome::SkippedMissingCheckout, 0, true),
        ];
        let metrics = compute_metrics(&runs);
        assert_eq!(metrics.repos_total, 2);
        assert_eq!(metrics.repos_run, 1);
        assert_eq!(metrics.repos_skipped, 1);
        assert_eq!(metrics.gate_status, "pass");
    }

    #[test]
    fn report_render_is_deterministic() {
        let runs = vec![run_with(Outcome::Ok, 96, true)];
        let metrics = compute_metrics(&runs);
        let a = render_json(&metrics, &runs);
        let b = render_json(&metrics, &runs);
        assert!(a.is_ok());
        assert!(a == b);
        let md = render_markdown(&metrics, &runs);
        assert!(md.contains("Tier A Eval Sweep"));
        assert!(md.contains("Gate: **pass**"));
    }
}
