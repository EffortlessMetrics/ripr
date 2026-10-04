//! `cargo xtask seam-inventory-scaling-benchmark` — a bounded, receipt-backed
//! scaling benchmark for the seam-inventory construction cost behind
//! #1887 / #4996 / #4997.
//!
//! The command generates deterministic synthetic Rust workspaces at several
//! file counts under `target/`, then times cold `ripr check` inventory runs
//! (`repo-seams-json` plus `repo-exposure-json`) at each size. The receipt
//! records the per-size cost curve so a cap-during-construction fix can show
//! a slope change on a later run. Like the other benchmark receipts, claims
//! are limited to the recorded revision and runner class.
//!
//! Child runs pin `RIPR_REPO_EXPOSURE_SEAM_LIMIT` to the product default so
//! caller environment cannot silently change the measured quantity, and each
//! sample records the child `run_status` so a capped run cannot pass as an
//! uncapped baseline. Within each size, format samples interleave in
//! alternating order and each sample records its execution order, so gradual
//! runner drift cannot systematically favor one format's selected timing.

use crate::run::{capture_output_with_timeout, run, run_output, run_output_owned};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: &str = "ripr-seam-inventory-scaling-benchmark-v1";
const DEFAULT_SIZES: &str = "200,800,2000";
const DEFAULT_SAMPLES: usize = 2;
const DEFAULT_TIMEOUT_MS: u64 = 300_000;
const MAX_SIZE: usize = 12_000;
const CACHE_ENV: &str = "RIPR_CACHE_DIR";
const SEAM_LIMIT_ENV: &str = "RIPR_REPO_EXPOSURE_SEAM_LIMIT";
/// Product default pinned for every child run. Must match
/// `DEFAULT_REPO_EXPOSURE_SEAM_LIMIT` in
/// `crates/ripr/src/analysis/seam_inventory.rs`.
const PINNED_SEAM_LIMIT: usize = 10_000;
const SEAMS_FORMAT: &str = "repo-seams-json";
const EXPOSURE_FORMAT: &str = "repo-exposure-json";

pub(crate) fn seam_inventory_scaling_benchmark(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    run("cargo", &["build", "-p", "ripr"])?;
    let binary = crate::ripr_debug_binary();

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let scratch =
        PathBuf::from("target/ripr/reports").join(format!("seam-inventory-scaling-{stamp}"));
    fs::create_dir_all(&scratch)
        .map_err(|err| format!("create benchmark scratch {}: {err}", scratch.display()))?;
    let cache_dir = scratch.join("cache");
    let envs = [
        (CACHE_ENV, cache_dir.to_string_lossy().into_owned()),
        (SEAM_LIMIT_ENV, PINNED_SEAM_LIMIT.to_string()),
    ];
    let timeout = Duration::from_millis(options.timeout_ms);

    let sizes_result = collect_sizes(&binary, &scratch, &envs, timeout, &options);
    if options.keep_workspaces {
        let _ = fs::remove_dir_all(&cache_dir);
    } else {
        let _ = fs::remove_dir_all(&scratch);
    }
    let sizes = sizes_result?;

    let report = build_report(&options, &sizes, &binary);
    let json_text = serde_json::to_string_pretty(&report)
        .map_err(|err| format!("serialize seam inventory scaling benchmark: {err}"))?;
    crate::write_report(
        "seam-inventory-scaling-benchmark.json",
        &format!("{json_text}\n"),
    )?;
    crate::write_report(
        "seam-inventory-scaling-benchmark.md",
        &benchmark_markdown(&report),
    )?;
    println!("Wrote target/ripr/reports/seam-inventory-scaling-benchmark.json");
    println!("Wrote target/ripr/reports/seam-inventory-scaling-benchmark.md");
    Ok(())
}

/// Run the per-size sampling loop. The caller owns scratch cleanup so a
/// failed sample cannot leave generated workspaces behind.
fn collect_sizes(
    binary: &Path,
    scratch: &Path,
    envs: &[(&str, String)],
    timeout: Duration,
    options: &Options,
) -> Result<Vec<SizeReport>, String> {
    let mut sizes = Vec::new();
    for files in &options.sizes {
        let workspace = generate_workspace(scratch, *files)?;
        let mut seams = Vec::new();
        let mut exposure = Vec::new();
        let mut order = 0;
        for sample_index in 0..options.samples {
            // Interleave the formats and alternate which runs first so
            // gradual runner drift cannot systematically favor one
            // format's selected timing.
            if sample_index % 2 == 0 {
                seams.push(run_one_sample(
                    binary,
                    &workspace,
                    SEAMS_FORMAT,
                    envs,
                    timeout,
                    parse_seams_count,
                    order,
                )?);
                order += 1;
                exposure.push(run_one_sample(
                    binary,
                    &workspace,
                    EXPOSURE_FORMAT,
                    envs,
                    timeout,
                    parse_exposure_count,
                    order,
                )?);
                order += 1;
            } else {
                exposure.push(run_one_sample(
                    binary,
                    &workspace,
                    EXPOSURE_FORMAT,
                    envs,
                    timeout,
                    parse_exposure_count,
                    order,
                )?);
                order += 1;
                seams.push(run_one_sample(
                    binary,
                    &workspace,
                    SEAMS_FORMAT,
                    envs,
                    timeout,
                    parse_seams_count,
                    order,
                )?);
                order += 1;
            }
        }
        sizes.push(SizeReport {
            files: *files,
            seams,
            exposure,
        });
        if !options.keep_workspaces {
            fs::remove_dir_all(&workspace).map_err(|err| {
                format!("remove generated workspace {}: {err}", workspace.display())
            })?;
        }
    }
    Ok(sizes)
}

const USAGE: &str = "usage: cargo xtask seam-inventory-scaling-benchmark [--sizes <n,n,n>] [--samples <n>] [--timeout-ms <n>] [--keep-workspaces]";

#[derive(Clone, Debug)]
struct Options {
    sizes: Vec<usize>,
    samples: usize,
    timeout_ms: u64,
    keep_workspaces: bool,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut sizes = None;
    let mut samples = DEFAULT_SAMPLES;
    let mut timeout_ms = DEFAULT_TIMEOUT_MS;
    let mut keep_workspaces = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--sizes" => {
                index += 1;
                sizes = Some(parse_sizes(required_arg(args, index, "--sizes")?)?);
            }
            "--samples" => {
                index += 1;
                samples = required_arg(args, index, "--samples")?
                    .parse()
                    .map_err(|err| {
                        format!(
                            "seam-inventory-scaling-benchmark --samples must be positive: {err}"
                        )
                    })?;
                if samples == 0 {
                    return Err(
                        "seam-inventory-scaling-benchmark --samples must be positive".to_string(),
                    );
                }
            }
            "--timeout-ms" => {
                index += 1;
                timeout_ms = required_arg(args, index, "--timeout-ms")?
                    .parse()
                    .map_err(|err| {
                        format!(
                            "seam-inventory-scaling-benchmark --timeout-ms must be positive: {err}"
                        )
                    })?;
                if timeout_ms == 0 {
                    return Err(
                        "seam-inventory-scaling-benchmark --timeout-ms must be positive"
                            .to_string(),
                    );
                }
            }
            "--keep-workspaces" => {
                keep_workspaces = true;
            }
            other => {
                return Err(format!(
                    "unknown seam-inventory-scaling-benchmark argument `{other}`; {USAGE}"
                ));
            }
        }
        index += 1;
    }
    Ok(Options {
        sizes: sizes.unwrap_or_else(|| {
            DEFAULT_SIZES
                .split(',')
                .filter_map(|size| size.parse().ok())
                .collect()
        }),
        samples,
        timeout_ms,
        keep_workspaces,
    })
}

fn parse_sizes(value: &str) -> Result<Vec<usize>, String> {
    let mut sizes = Vec::new();
    for part in value.split(',') {
        let size: usize = part.trim().parse().map_err(|err| {
            format!("seam-inventory-scaling-benchmark --sizes must be a comma list of file counts: {err}")
        })?;
        if size == 0 || size > MAX_SIZE {
            return Err(format!(
                "seam-inventory-scaling-benchmark sizes must be within 1..={MAX_SIZE}, got {size}"
            ));
        }
        sizes.push(size);
    }
    if sizes.is_empty() {
        return Err(format!(
            "seam-inventory-scaling-benchmark needs at least one --sizes entry; {USAGE}"
        ));
    }
    sizes.sort_unstable();
    sizes.dedup();
    Ok(sizes)
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

/// Generate a deterministic synthetic workspace: `files` production modules
/// with three small fns each, no tests directory, fixed template content.
fn generate_workspace(scratch: &Path, files: usize) -> Result<PathBuf, String> {
    let root = scratch.join(format!("ws-{files}"));
    let src = root.join("src");
    fs::create_dir_all(&src)
        .map_err(|err| format!("create generated workspace {}: {err}", root.display()))?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scaleprobe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .map_err(|err| format!("write generated Cargo.toml: {err}"))?;
    let mut lib = String::new();
    for index in 0..files {
        lib.push_str(&format!("mod m{index};\n"));
        let body = format!(
            "pub fn f{index}_a(x: u32) -> u32 {{ if x > {index} {{ x * 2 }} else {{ x + 1 }} }}\n\
             pub fn f{index}_b(s: &str) -> usize {{ s.len() + {index} }}\n\
             pub fn f{index}_c(flag: bool) -> u32 {{ if flag {{ {index} }} else {{ 0 }} }}\n"
        );
        fs::write(src.join(format!("m{index}.rs")), body)
            .map_err(|err| format!("write generated module m{index}: {err}"))?;
    }
    fs::write(src.join("lib.rs"), lib).map_err(|err| format!("write generated lib.rs: {err}"))?;
    Ok(root)
}

#[derive(Clone, Debug)]
struct Sample {
    status: String,
    duration_ms: u128,
    seam_count: Option<u64>,
    stdout_bytes: usize,
    detail: Option<String>,
    run_status: Option<String>,
    order: usize,
}

struct SizeReport {
    files: usize,
    seams: Vec<Sample>,
    exposure: Vec<Sample>,
}

/// One cold child run. `order` is the sample's global execution sequence
/// within its size; the caller interleaves formats so gradual runner drift
/// cannot systematically favor one format's selected timing.
fn run_one_sample(
    binary: &Path,
    root: &Path,
    format: &str,
    envs: &[(&str, String)],
    timeout: Duration,
    parse_count: fn(&Value) -> Option<u64>,
    order: usize,
) -> Result<Sample, String> {
    let owned: Vec<(&str, &str)> = envs
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    clear_env_cache(envs)?;
    let args = [
        "check".to_string(),
        "--root".to_string(),
        root.display().to_string(),
        "--format".to_string(),
        format.to_string(),
    ];
    let output = capture_output_with_timeout(
        &binary.display().to_string(),
        &args,
        &owned,
        timeout,
        "seam inventory scaling benchmark",
    )?;
    let mut status = if output.timed_out {
        "timeout"
    } else if output.status.is_some_and(|status| status.success()) {
        "pass"
    } else {
        "fail"
    };
    let stdout_bytes = output.stdout.len();
    let (seam_count, detail, run_status) = if status == "pass" {
        match serde_json::from_str::<Value>(&output.stdout) {
            Ok(value) => {
                let child_status = value
                    .pointer("/run_status")
                    .and_then(|node| node.as_str())
                    .map(str::to_string);
                let count = parse_count(&value);
                if child_status.as_deref() == Some("seam_limit_applied") {
                    status = "capped";
                    (
                        count,
                        Some(format!(
                            "{format} applied the seam limit; capped timing excluded from slope"
                        )),
                        child_status,
                    )
                } else {
                    match require_nonempty_inventory(format, count) {
                        Ok(count) => (Some(count), None, child_status),
                        Err(detail) => {
                            status = "empty_inventory";
                            (count, Some(detail), child_status)
                        }
                    }
                }
            }
            Err(err) => {
                status = "invalid_receipt";
                (
                    None,
                    Some(format!("{format} JSON parse failed: {err}")),
                    None,
                )
            }
        }
    } else {
        (None, Some(summarize_output(&output.stderr)), None)
    };
    Ok(Sample {
        status: status.to_string(),
        duration_ms: output.duration.as_millis(),
        seam_count,
        stdout_bytes,
        detail,
        run_status,
        order,
    })
}

fn parse_seams_count(value: &Value) -> Option<u64> {
    value
        .pointer("/seams")?
        .as_array()
        .map(|seams| seams.len() as u64)
}

fn parse_exposure_count(value: &Value) -> Option<u64> {
    value.pointer("/metrics/seams_total")?.as_u64()
}

/// Generated workspaces always contain probeable functions, so a zero or
/// missing seam count means discovery or extraction regressed, not a
/// genuinely empty corpus. Reject it instead of recording a passing
/// sample over an empty analysis.
fn require_nonempty_inventory(format: &str, count: Option<u64>) -> Result<u64, String> {
    match count {
        Some(count) if count > 0 => Ok(count),
        _ => Err(format!(
            "{format} reported no seams for a generated workspace with probeable functions"
        )),
    }
}

fn clear_env_cache(envs: &[(&str, String)]) -> Result<(), String> {
    let cache_dir = envs
        .iter()
        .find(|(name, _)| *name == CACHE_ENV)
        .map(|(_, value)| PathBuf::from(value))
        .ok_or_else(|| format!("benchmark environment is missing {CACHE_ENV}"))?;
    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir)
            .map_err(|err| format!("remove benchmark cache {}: {err}", cache_dir.display()))?;
    }
    fs::create_dir_all(&cache_dir)
        .map_err(|err| format!("create benchmark cache {}: {err}", cache_dir.display()))
}

fn percentile(values: &mut [u128], percentile: usize) -> u128 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    let rank = ((values.len() * percentile).saturating_add(99) / 100).saturating_sub(1);
    values[rank.min(values.len() - 1)]
}

fn series_p50(samples: &[Sample]) -> u128 {
    let mut values: Vec<u128> = samples.iter().map(|sample| sample.duration_ms).collect();
    percentile(&mut values, 50)
}

fn series_p95(samples: &[Sample]) -> u128 {
    let mut values: Vec<u128> = samples.iter().map(|sample| sample.duration_ms).collect();
    percentile(&mut values, 95)
}

fn all_pass(samples: &[Sample]) -> bool {
    !samples.is_empty() && samples.iter().all(|sample| sample.status == "pass")
}

fn series_json(samples: &[Sample]) -> Value {
    json!({
        "p50_ms": series_p50(samples),
        "p95_ms": series_p95(samples),
        "samples": samples.iter().map(|sample| json!({
            "status": sample.status,
            "order": sample.order,
            "duration_ms": sample.duration_ms,
            "seam_count": sample.seam_count,
            "run_status": sample.run_status,
            "stdout_bytes": sample.stdout_bytes,
            "detail": sample.detail,
        })).collect::<Vec<_>>()
    })
}

/// Coarse ms-per-file slope between the smallest and largest passing size.
/// A construction-cost fix (cap during build) should bend this slope, not
/// just shift one point.
fn slope_ms_per_file(sizes: &[SizeReport], select: fn(&SizeReport) -> &[Sample]) -> Option<f64> {
    let first = sizes.first()?;
    let last = sizes.last()?;
    if last.files <= first.files || !all_pass(select(first)) || !all_pass(select(last)) {
        return None;
    }
    Some(
        (series_p50(select(last)) as f64 - series_p50(select(first)) as f64)
            / (last.files - first.files) as f64,
    )
}

fn build_report(options: &Options, sizes: &[SizeReport], binary: &Path) -> Value {
    let complete = sizes
        .iter()
        .all(|size| all_pass(&size.seams) && all_pass(&size.exposure));
    json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "report": "seam-inventory-scaling-benchmark",
        "status": if complete { "pass" } else { "inconclusive" },
        "revision": git_revision(),
        "runner_class": runner_class(),
        "analyzer_version": analyzer_version(binary),
        "cache_policy": "isolated RIPR_CACHE_DIR cleared before every sample (cold inventory)",
        "seam_limit": {
            "control": SEAM_LIMIT_ENV,
            "value": PINNED_SEAM_LIMIT,
            "source": "pinned_product_default"
        },
        "generator": "deterministic synthetic workspace: N production modules, 3 small fns each, no tests directory",
        "sizes": options.sizes,
        "samples": options.samples,
        "timeout_ms": options.timeout_ms,
        "commands": {
            "seams": "ripr check --root <ws> --format repo-seams-json",
            "exposure": "ripr check --root <ws> --format repo-exposure-json"
        },
        "sizes_report": sizes.iter().map(|size| json!({
            "files": size.files,
            "seams": series_json(&size.seams),
            "exposure": series_json(&size.exposure),
        })).collect::<Vec<_>>(),
        "comparison": {
            "seams_slope_ms_per_file": slope_ms_per_file(sizes, |size| &size.seams),
            "exposure_slope_ms_per_file": slope_ms_per_file(sizes, |size| &size.exposure),
            "slope_samples_per_size": options.samples,
            "sample_order": "interleaved_alternating",
        },
        "claim_boundary": "Static cold-inventory wall time on synthetic workspaces for the recorded revision and runner class only; not peak memory, not a gate, not universal latency. Compare runs only on the same runner class."
    })
}

fn benchmark_markdown(report: &Value) -> String {
    let mut rows = String::new();
    if let Some(sizes) = report["sizes_report"].as_array() {
        for size in sizes {
            rows.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                size["files"],
                size["seams"]["p50_ms"],
                size["seams"]["p95_ms"],
                size["exposure"]["p50_ms"],
                size["exposure"]["p95_ms"],
            ));
        }
    }
    format!(
        "# Seam Inventory Scaling Benchmark\n\nStatus: `{}`\n\nRevision: `{}`\nRunner: `{}`\nAnalyzer: `{}`\n\n| Files | Seams p50 (ms) | Seams p95 (ms) | Exposure p50 (ms) | Exposure p95 (ms) |\n| ---: | ---: | ---: | ---: | ---: |\n{}\nSeams slope: {} ms/file; exposure slope: {} ms/file (p50 of {} samples per size, endpoint sizes).\n\nClaim boundary: {}\n",
        report["status"].as_str().unwrap_or("unknown"),
        report["revision"].as_str().unwrap_or("unavailable"),
        report["runner_class"].as_str().unwrap_or("unknown"),
        report["analyzer_version"].as_str().unwrap_or("unknown"),
        rows,
        report["comparison"]["seams_slope_ms_per_file"],
        report["comparison"]["exposure_slope_ms_per_file"],
        report["comparison"]["slope_samples_per_size"],
        report["claim_boundary"].as_str().unwrap_or("unknown"),
    )
}

fn summarize_output(stderr: &str) -> String {
    const LIMIT: usize = 500;
    let trimmed = stderr.trim();
    if trimmed.chars().count() <= LIMIT {
        return trimmed.to_string();
    }
    format!("{}…", trimmed.chars().take(LIMIT).collect::<String>())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_options_defaults_to_three_sizes() -> Result<(), String> {
        let options = parse_options(&[])?;
        assert_eq!(options.sizes, vec![200, 800, 2000]);
        assert_eq!(options.samples, DEFAULT_SAMPLES);
        assert!(!options.keep_workspaces);
        Ok(())
    }

    #[test]
    fn parse_sizes_rejects_zero_and_oversize() -> Result<(), String> {
        for bad in ["0", "12001", ""] {
            if parse_sizes(bad).is_ok() {
                return Err(format!("parse_sizes({bad:?}) unexpectedly succeeded"));
            }
        }
        let deduped = parse_sizes("800,200,800")?;
        assert_eq!(deduped, vec![200, 800]);
        Ok(())
    }

    #[test]
    fn empty_series_has_no_slope() {
        assert_eq!(slope_ms_per_file(&[], |size| &size.seams), None);
    }

    fn sample(status: &str, duration_ms: u128) -> Sample {
        Sample {
            status: status.to_string(),
            duration_ms,
            seam_count: Some(100),
            stdout_bytes: 10,
            detail: None,
            run_status: Some("complete".to_string()),
            order: 0,
        }
    }

    #[test]
    fn percentile_ranks_match_sorted_positions() {
        assert_eq!(percentile(&mut [10, 20, 30, 40], 50), 20);
        assert_eq!(percentile(&mut [10, 20, 30, 40], 95), 40);
        assert_eq!(percentile(&mut [7], 50), 7);
        let empty: &mut [u128] = &mut [];
        assert_eq!(percentile(empty, 50), 0);
    }

    #[test]
    fn slope_uses_endpoint_p50_and_rejects_failures() -> Result<(), String> {
        let passing = vec![
            SizeReport {
                files: 100,
                seams: vec![sample("pass", 100), sample("pass", 120)],
                exposure: Vec::new(),
            },
            SizeReport {
                files: 200,
                seams: vec![sample("pass", 300), sample("pass", 320)],
                exposure: Vec::new(),
            },
        ];
        match slope_ms_per_file(&passing, |size| &size.seams) {
            Some(slope) if (slope - 2.0).abs() < f64::EPSILON => {}
            other => return Err(format!("expected slope 2.0, got {other:?}")),
        }
        let failing = vec![
            SizeReport {
                files: 100,
                seams: vec![sample("pass", 100), sample("fail", 120)],
                exposure: Vec::new(),
            },
            SizeReport {
                files: 200,
                seams: vec![sample("pass", 300)],
                exposure: Vec::new(),
            },
        ];
        if slope_ms_per_file(&failing, |size| &size.seams).is_some() {
            return Err("slope must be None when an endpoint series fails".to_string());
        }
        if slope_ms_per_file(&passing[..1], |size| &size.seams).is_some() {
            return Err("slope must be None for a single size".to_string());
        }
        Ok(())
    }

    #[test]
    fn nonempty_inventory_rejects_zero_and_missing() -> Result<(), String> {
        assert_eq!(require_nonempty_inventory("repo-seams-json", Some(12))?, 12);
        for bad in [None, Some(0)] {
            if require_nonempty_inventory("repo-exposure-json", bad).is_ok() {
                return Err(format!(
                    "require_nonempty_inventory({bad:?}) unexpectedly succeeded"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn all_pass_rejects_empty_inventory_status() {
        assert!(!all_pass(&[sample("empty_inventory", 5)]));
        assert!(all_pass(&[sample("pass", 5)]));
    }

    #[test]
    fn all_pass_rejects_capped_status() {
        let mut capped = sample("capped", 5);
        capped.run_status = Some("seam_limit_applied".to_string());
        assert!(!all_pass(&[capped]));
    }

    #[test]
    fn series_json_records_sample_order() -> Result<(), String> {
        let mut first = sample("pass", 100);
        first.order = 0;
        let mut second = sample("pass", 120);
        second.order = 3;
        let document = series_json(&[first, second]);
        let samples = document
            .get("samples")
            .and_then(Value::as_array)
            .ok_or_else(|| "series_json must render samples".to_string())?;
        let orders: Vec<u64> = samples
            .iter()
            .map(|sample| sample.get("order").and_then(Value::as_u64))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| "series_json samples must carry order".to_string())?;
        if orders != vec![0, 3] {
            return Err(format!("expected sample orders [0, 3], got {orders:?}"));
        }
        Ok(())
    }

    fn temp_root(label: &str) -> Result<PathBuf, String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-xtask-scalebench-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&root).map_err(|err| format!("create temp root: {err}"))?;
        Ok(root)
    }

    #[test]
    fn generate_workspace_writes_expected_modules() -> Result<(), String> {
        let root = temp_root("workspace")?;
        let workspace = generate_workspace(&root, 3)?;
        let mut missing = Vec::new();
        for name in [
            "Cargo.toml",
            "src/lib.rs",
            "src/m0.rs",
            "src/m1.rs",
            "src/m2.rs",
        ] {
            if !workspace.join(name).is_file() {
                missing.push(name);
            }
        }
        let lib = fs::read_to_string(workspace.join("src/lib.rs"))
            .map_err(|err| format!("read generated lib.rs: {err}"))?;
        let module = fs::read_to_string(workspace.join("src/m1.rs"))
            .map_err(|err| format!("read generated m1.rs: {err}"))?;
        fs::remove_dir_all(&root)
            .map_err(|err| format!("remove temp root {}: {err}", root.display()))?;
        if !missing.is_empty() {
            return Err(format!("generated workspace is missing {missing:?}"));
        }
        if !lib.contains("mod m0;") || !module.contains("pub fn f1_a") {
            return Err("generated modules do not match the fixed template".to_string());
        }
        Ok(())
    }
}
