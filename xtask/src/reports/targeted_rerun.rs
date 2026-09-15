//! `cargo xtask targeted-rerun-benchmark` — a bounded, receipt-backed
//! cold/full versus targeted-rerun benchmark for SPEC-0123.

use crate::run::{capture_output_with_timeout, run, run_output, run_output_owned};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: &str = "ripr-targeted-rerun-benchmark-v1";
const DEFAULT_SAMPLES: usize = 3;
const DEFAULT_TIMEOUT_MS: u64 = 300_000;
const WARM_P50_TARGET_MS: u128 = 30_000;
const SPEEDUP_TARGET: u128 = 5;
const CACHE_ENV: &str = "RIPR_CACHE_DIR";

pub(crate) fn targeted_rerun_benchmark(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    let root = fs::canonicalize(&options.root).map_err(|err| {
        format!(
            "targeted-rerun-benchmark root {}: {err}",
            options.root.display()
        )
    })?;
    let binary = crate::ripr_debug_binary();
    run("cargo", &["build", "-p", "ripr"])?;

    let cache_dir = benchmark_cache_dir();
    let _cache_guard = CacheDirGuard::new(cache_dir.clone());
    clear_cache(&cache_dir)?;
    let envs = [(CACHE_ENV, cache_dir.to_string_lossy().into_owned())];
    let timeout = Duration::from_millis(options.timeout_ms);

    let cold_full = run_series(&binary, &root, &envs, timeout, options.samples, true)?;
    clear_cache(&cache_dir)?;
    let cold_targeted = run_targeted_series(&TargetedSeriesConfig {
        binary: &binary,
        root: &root,
        options: &options,
        envs: &envs,
        timeout,
        samples: options.samples,
        clear_each: true,
        check_parity: false,
    })?;
    let warm_targeted = run_targeted_series(&TargetedSeriesConfig {
        binary: &binary,
        root: &root,
        options: &options,
        envs: &envs,
        timeout,
        samples: options.samples,
        clear_each: false,
        check_parity: false,
    })?;

    // A deliberate cache reset is the registered invalidation case. It proves
    // that a targeted rerun does not silently report a hit after its fact
    // store has been invalidated.
    clear_cache(&cache_dir)?;
    let invalidation = run_targeted_series(&TargetedSeriesConfig {
        binary: &binary,
        root: &root,
        options: &options,
        envs: &envs,
        timeout,
        samples: 1,
        clear_each: false,
        check_parity: false,
    })?;
    let parity = run_targeted_series(&TargetedSeriesConfig {
        binary: &binary,
        root: &root,
        options: &options,
        envs: &envs,
        timeout,
        samples: 1,
        clear_each: false,
        check_parity: true,
    })?;

    let report = build_report(
        &root,
        &options,
        &cold_full,
        &cold_targeted,
        &warm_targeted,
        &invalidation,
        &parity,
    );
    let json_text = serde_json::to_string_pretty(&report)
        .map_err(|err| format!("serialize targeted rerun benchmark: {err}"))?;
    crate::write_report("targeted-rerun-benchmark.json", &format!("{json_text}\n"))?;
    crate::write_report("targeted-rerun-benchmark.md", &benchmark_markdown(&report))?;
    println!("Wrote target/ripr/reports/targeted-rerun-benchmark.json");
    println!("Wrote target/ripr/reports/targeted-rerun-benchmark.md");
    Ok(())
}

const USAGE: &str = "usage: cargo xtask targeted-rerun-benchmark --root <path> --changed-test <path> [--samples <n>] [--timeout-ms <n>]";

#[derive(Clone, Debug, Eq, PartialEq)]
struct Options {
    root: PathBuf,
    changed_test: String,
    samples: usize,
    timeout_ms: u64,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut root = None;
    let mut changed_test = None;
    let mut samples = DEFAULT_SAMPLES;
    let mut timeout_ms = DEFAULT_TIMEOUT_MS;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                root = Some(PathBuf::from(required_arg(args, index, "--root")?));
            }
            "--changed-test" => {
                index += 1;
                changed_test = Some(required_arg(args, index, "--changed-test")?.to_string());
            }
            "--samples" => {
                index += 1;
                samples = required_arg(args, index, "--samples")?
                    .parse()
                    .map_err(|err| {
                        format!("targeted-rerun-benchmark --samples must be positive: {err}")
                    })?;
                if samples == 0 {
                    return Err("targeted-rerun-benchmark --samples must be positive".to_string());
                }
            }
            "--timeout-ms" => {
                index += 1;
                timeout_ms = required_arg(args, index, "--timeout-ms")?
                    .parse()
                    .map_err(|err| {
                        format!("targeted-rerun-benchmark --timeout-ms must be positive: {err}")
                    })?;
                if timeout_ms == 0 {
                    return Err(
                        "targeted-rerun-benchmark --timeout-ms must be positive".to_string()
                    );
                }
            }
            other => {
                return Err(format!(
                    "unknown targeted-rerun-benchmark argument `{other}`; {USAGE}"
                ));
            }
        }
        index += 1;
    }
    Ok(Options {
        root: root.ok_or_else(|| format!("missing --root; {USAGE}"))?,
        changed_test: changed_test.ok_or_else(|| format!("missing --changed-test; {USAGE}"))?,
        samples,
        timeout_ms,
    })
}

fn required_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    let Some(value) = args.get(index) else {
        return Err(format!("missing value for {flag}; {USAGE}"));
    };
    if value.trim().is_empty() {
        return Err(format!("{flag} requires a non-empty value; {USAGE}"));
    }
    Ok(value)
}

#[derive(Clone, Debug, Default)]
struct Series {
    samples: Vec<Sample>,
}

#[derive(Clone, Debug)]
struct Sample {
    status: &'static str,
    duration_ms: u128,
    cache_reuse_state: Option<String>,
    selected_seam_count: Option<usize>,
    parity_state: Option<String>,
    detail: Option<String>,
}

fn run_series(
    binary: &Path,
    root: &Path,
    envs: &[(&str, String)],
    timeout: Duration,
    samples: usize,
    clear_each: bool,
) -> Result<Series, String> {
    let mut series = Series::default();
    for _ in 0..samples {
        if clear_each {
            clear_env_cache(envs)?;
        }
        let args = [
            "check".to_string(),
            "--root".to_string(),
            root.display().to_string(),
            "--format".to_string(),
            "repo-exposure-json".to_string(),
        ];
        series
            .samples
            .push(run_sample(binary, &args, envs, timeout, false)?);
    }
    Ok(series)
}

struct TargetedSeriesConfig<'a> {
    binary: &'a Path,
    root: &'a Path,
    options: &'a Options,
    envs: &'a [(&'a str, String)],
    timeout: Duration,
    samples: usize,
    clear_each: bool,
    check_parity: bool,
}

fn run_targeted_series(config: &TargetedSeriesConfig<'_>) -> Result<Series, String> {
    let mut series = Series::default();
    for _ in 0..config.samples {
        if config.clear_each {
            clear_env_cache(config.envs)?;
        }
        let mut args = vec![
            "rerun".to_string(),
            "--root".to_string(),
            config.root.display().to_string(),
            "--changed-test".to_string(),
            config.options.changed_test.clone(),
            "--json".to_string(),
        ];
        if config.check_parity {
            args.push("--check-parity".to_string());
        }
        series.samples.push(run_sample(
            config.binary,
            &args,
            config.envs,
            config.timeout,
            true,
        )?);
    }
    Ok(series)
}

fn run_sample(
    binary: &Path,
    args: &[String],
    envs: &[(&str, String)],
    timeout: Duration,
    parse_targeted: bool,
) -> Result<Sample, String> {
    let envs = envs
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect::<Vec<_>>();
    let binary_text = binary.display().to_string();
    let output = capture_output_with_timeout(
        &binary_text,
        args,
        &envs,
        timeout,
        "targeted-rerun benchmark",
    )?;
    let mut status = if output.timed_out {
        "timeout"
    } else if output.status.is_some_and(|status| status.success()) {
        "pass"
    } else {
        "fail"
    };
    let (cache_reuse_state, selected_seam_count, parity_state, parse_detail) =
        if parse_targeted && status == "pass" {
            match serde_json::from_str::<Value>(&output.stdout) {
                Ok(value) => (
                    value
                        .pointer("/cache/reuse_state")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    value
                        .pointer("/seams")
                        .and_then(Value::as_array)
                        .map(Vec::len),
                    value
                        .pointer("/parity/state")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    None,
                ),
                Err(err) => {
                    status = "invalid_receipt";
                    (
                        None,
                        None,
                        None,
                        Some(format!("targeted JSON parse failed: {err}")),
                    )
                }
            }
        } else {
            (None, None, None, None)
        };
    let detail =
        parse_detail.or_else(|| (status != "pass").then(|| summarize_output(&output.stderr)));
    Ok(Sample {
        status,
        duration_ms: output.duration.as_millis(),
        cache_reuse_state,
        selected_seam_count,
        parity_state,
        detail,
    })
}

fn build_report(
    root: &Path,
    options: &Options,
    cold_full: &Series,
    cold_targeted: &Series,
    warm_targeted: &Series,
    invalidation: &Series,
    parity: &Series,
) -> Value {
    let cold_full_p50 = percentile(&cold_full.samples, 50);
    let warm_targeted_p50 = percentile(&warm_targeted.samples, 50);
    let speedup = if warm_targeted_p50 == 0 {
        None
    } else {
        Some(cold_full_p50 as f64 / warm_targeted_p50 as f64)
    };
    let parity_state = parity
        .samples
        .first()
        .and_then(|sample| sample.parity_state.as_deref())
        .unwrap_or("unavailable");
    let threshold_pass = warm_targeted_p50 <= WARM_P50_TARGET_MS
        && cold_full_p50 >= warm_targeted_p50.saturating_mul(SPEEDUP_TARGET)
        && parity_state == "matched"
        && all_pass(cold_full)
        && all_pass(cold_targeted)
        && all_pass(warm_targeted)
        && all_pass(invalidation)
        && all_pass(parity);
    json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "report": "targeted-rerun-benchmark",
        "status": if threshold_pass { "pass" } else { "inconclusive" },
        "root": normalize_path(root),
        "changed_test": options.changed_test,
        "revision": git_revision(),
        "runner_class": runner_class(),
        "analyzer_version": analyzer_version(&crate::ripr_debug_binary()),
        "cache_policy": "isolated RIPR_CACHE_DIR; cache reset between cold and invalidation samples",
        "samples": options.samples,
        "timeout_ms": options.timeout_ms,
        "commands": {
            "cold_full": "ripr check --root <root> --format repo-exposure-json",
            "cold_targeted": "ripr rerun --root <root> --changed-test <test> --json",
            "warm_targeted": "ripr rerun --root <root> --changed-test <test> --json",
            "invalidation": "reset isolated file-fact cache, then rerun the selected test",
            "parity": "ripr rerun --root <root> --changed-test <test> --check-parity --json"
        },
        "cold_full": series_json(cold_full),
        "cold_targeted": series_json(cold_targeted),
        "warm_targeted": series_json(warm_targeted),
        "invalidation": {
            "case": "explicit_file_fact_cache_reset",
            "series": series_json(invalidation)
        },
        "parity": {
            "state": parity_state,
            "selected_seam_count": parity.samples.first().and_then(|sample| sample.selected_seam_count),
            "cache_reuse_state": parity.samples.first().and_then(|sample| sample.cache_reuse_state.clone())
        },
        "comparison": {
            "cold_full_p50_ms": cold_full_p50,
            "cold_full_p95_ms": percentile(&cold_full.samples, 95),
            "warm_targeted_p50_ms": warm_targeted_p50,
            "warm_targeted_p95_ms": percentile(&warm_targeted.samples, 95),
            "cold_to_warm_speedup": speedup,
            "warm_p50_target_ms": WARM_P50_TARGET_MS,
            "minimum_speedup": SPEEDUP_TARGET,
            "thresholds_met": threshold_pass
        },
        "claim_boundary": "Named-repository static benchmark only; this does not claim runtime mutation behavior, correctness, coverage adequacy, universal latency, or complete input invalidation attribution."
    })
}

fn series_json(series: &Series) -> Value {
    json!({
        "p50_ms": percentile(&series.samples, 50),
        "p95_ms": percentile(&series.samples, 95),
        "samples": series.samples.iter().map(|sample| json!({
            "status": sample.status,
            "duration_ms": sample.duration_ms,
            "cache_reuse_state": sample.cache_reuse_state,
            "selected_seam_count": sample.selected_seam_count,
            "parity_state": sample.parity_state,
            "detail": sample.detail,
        })).collect::<Vec<_>>()
    })
}

fn benchmark_markdown(report: &Value) -> String {
    let status = report["status"].as_str().unwrap_or("unknown");
    let comparison = &report["comparison"];
    format!(
        "# Targeted Rerun Benchmark\n\nStatus: `{status}`\n\nRoot: `{}`\nChanged test: `{}`\nRevision: `{}`\nRunner: `{}`\nAnalyzer: `{}`\n\n| Measure | Value |\n| --- | ---: |\n| Cold full p50 | {} ms |\n| Cold full p95 | {} ms |\n| Warm targeted p50 | {} ms |\n| Warm targeted p95 | {} ms |\n| Cold-to-warm speedup | {}x |\n\nParity: `{}`\n\nClaim boundary: {}\n",
        report["root"].as_str().unwrap_or("unknown"),
        report["changed_test"].as_str().unwrap_or("unknown"),
        report["revision"].as_str().unwrap_or("unavailable"),
        report["runner_class"].as_str().unwrap_or("unknown"),
        report["analyzer_version"].as_str().unwrap_or("unknown"),
        comparison["cold_full_p50_ms"],
        comparison["cold_full_p95_ms"],
        comparison["warm_targeted_p50_ms"],
        comparison["warm_targeted_p95_ms"],
        comparison["cold_to_warm_speedup"],
        report["parity"]["state"].as_str().unwrap_or("unavailable"),
        report["claim_boundary"].as_str().unwrap_or("unknown"),
    )
}

fn all_pass(series: &Series) -> bool {
    !series.samples.is_empty() && series.samples.iter().all(|sample| sample.status == "pass")
}

fn percentile(samples: &[Sample], percentile: usize) -> u128 {
    let mut values = samples
        .iter()
        .map(|sample| sample.duration_ms)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    let rank = ((values.len() * percentile).saturating_add(99) / 100).saturating_sub(1);
    values[rank.min(values.len() - 1)]
}

fn clear_cache(cache_dir: &Path) -> Result<(), String> {
    if cache_dir.exists() {
        fs::remove_dir_all(cache_dir)
            .map_err(|err| format!("remove benchmark cache {}: {err}", cache_dir.display()))?;
    }
    fs::create_dir_all(cache_dir)
        .map_err(|err| format!("create benchmark cache {}: {err}", cache_dir.display()))
}

struct CacheDirGuard {
    path: PathBuf,
}

impl CacheDirGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for CacheDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn clear_env_cache(envs: &[(&str, String)]) -> Result<(), String> {
    let cache_dir = envs
        .iter()
        .find(|(name, _)| *name == CACHE_ENV)
        .map(|(_, value)| PathBuf::from(value))
        .ok_or_else(|| format!("benchmark environment is missing {CACHE_ENV}"))?;
    clear_cache(&cache_dir)
}

fn benchmark_cache_dir() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    PathBuf::from("target/ripr/reports").join(format!("targeted-rerun-benchmark-cache-{stamp}"))
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("//?/")
        .to_string()
}

fn git_revision() -> String {
    run_output("git", &["rev-parse", "HEAD"])
        .ok()
        .map(|output| output.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unavailable".to_string())
}

fn runner_class() -> String {
    std::env::var("RUNNER_NAME")
        .or_else(|_| std::env::var("GITHUB_RUNNER_NAME"))
        .unwrap_or_else(|_| format!("local-{}-{}", std::env::consts::OS, std::env::consts::ARCH))
}

fn analyzer_version(binary: &Path) -> String {
    run_output_owned(&binary.display().to_string(), &["--version".to_string()])
        .ok()
        .map(|output| output.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unavailable".to_string())
}

fn summarize_output(output: &str) -> String {
    const LIMIT: usize = 500;
    let trimmed = output.trim();
    if trimmed.len() <= LIMIT {
        trimmed.to_string()
    } else {
        format!("{}…", trimmed.chars().take(LIMIT).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_options_requires_root_and_changed_test() -> Result<(), String> {
        let err = match parse_options(&[]) {
            Ok(_) => return Err("missing benchmark inputs unexpectedly succeeded".to_string()),
            Err(err) => err,
        };
        assert!(err.contains("missing --root"));
        let options = parse_options(&[
            "--root".to_string(),
            "fixtures/boundary_gap/input".to_string(),
            "--changed-test".to_string(),
            "tests/pricing.rs".to_string(),
            "--samples".to_string(),
            "2".to_string(),
        ])?;
        assert_eq!(options.samples, 2);
        assert_eq!(options.timeout_ms, DEFAULT_TIMEOUT_MS);
        Ok(())
    }

    #[test]
    fn percentile_uses_sorted_nearest_rank() {
        let samples = [
            Sample {
                status: "pass",
                duration_ms: 30,
                cache_reuse_state: None,
                selected_seam_count: None,
                parity_state: None,
                detail: None,
            },
            Sample {
                status: "pass",
                duration_ms: 10,
                cache_reuse_state: None,
                selected_seam_count: None,
                parity_state: None,
                detail: None,
            },
            Sample {
                status: "pass",
                duration_ms: 20,
                cache_reuse_state: None,
                selected_seam_count: None,
                parity_state: None,
                detail: None,
            },
        ];
        let series = Series {
            samples: samples.to_vec(),
        };
        assert_eq!(percentile(&series.samples, 50), 20);
        assert_eq!(percentile(&series.samples, 95), 30);
    }

    #[test]
    fn all_pass_rejects_empty_or_failed_series() {
        assert!(!all_pass(&Series::default()));
        let mut series = Series {
            samples: vec![Sample {
                status: "pass",
                duration_ms: 1,
                cache_reuse_state: None,
                selected_seam_count: None,
                parity_state: Some("matched".to_string()),
                detail: None,
            }],
        };
        assert!(all_pass(&series));
        series.samples[0].status = "fail";
        assert!(!all_pass(&series));
    }
}
