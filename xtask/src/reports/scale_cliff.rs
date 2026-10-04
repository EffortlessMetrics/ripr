//! `cargo xtask scale-cliff-benchmark` — a bounded, receipt-backed probe for
//! where `ripr check` and `ripr pilot` stop being usable as repository size
//! grows.
//!
//! Targets are either deterministic generated single-crate workspaces (with a
//! git history so `check --base` has a real one-file diff) or one existing
//! repository supplied with `--repo` and `--base`. Every run is cold, bounded
//! by `--timeout-ms`, and classified as `pass`, `timeout`,
//! `refused_oversized` (the diff-index file cap), or `fail`, so a cliff shows
//! up as a status change instead of a missing row. A `fail` sample is a
//! measurement error (`failed`), not `cliff_observed`. A refused or timed-out
//! sample names the next flag to try. With `--repo`, the receipt records the
//! target repository's HEAD and the resolved `--base`, not the xtask checkout.
//!
//! The child is the release binary: debug-build timings are not
//! representative of what users run. Peak RSS comes from GNU `time -v` when it
//! is installed (Linux runners); elsewhere the sample records
//! `peak_rss_kib: null` rather than guessing.
//!
//! Claims are limited to the recorded revision, runner class and corpus.

use crate::run::{capture_output_in_dir, capture_output_with_timeout, run};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: &str = "ripr-scale-cliff-benchmark-v2";
const DEFAULT_SIZES: &str = "250,1000,4000";
const DEFAULT_TIMEOUT_MS: u64 = 600_000;
const MAX_SIZE: usize = 100_000;
const DEFAULT_INDEX_CAP: &str = "1000000";
const INDEX_CAP_ENV: &str = "RIPR_MAX_DIFF_INDEX_FILES";
const GNU_TIME: &str = "/usr/bin/time";
const RSS_LINE: &str = "Maximum resident set size (kbytes):";

const USAGE: &str = "usage: cargo xtask scale-cliff-benchmark [--sizes <n,n,n>] [--repo <path> --base <rev>] [--mode <draft|deep|instant>] [--commands <check,pilot>] [--index-cap <n|product>] [--timeout-ms <n>] [--keep-workspaces]";

#[derive(Clone, Debug)]
struct Options {
    sizes: Vec<usize>,
    repo: Option<(PathBuf, String)>,
    mode: String,
    commands: Vec<Command>,
    index_cap: Option<String>,
    timeout_ms: u64,
    keep_workspaces: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Check,
    Pilot,
}

impl Command {
    fn name(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Pilot => "pilot",
        }
    }
}

struct Target {
    label: String,
    files: Option<usize>,
    root: PathBuf,
    base: String,
}

#[derive(Clone, Debug)]
struct Sample {
    command: &'static str,
    status: String,
    duration_ms: u128,
    peak_rss_kib: Option<u64>,
    findings: Option<u64>,
    detail: Option<String>,
}

struct TargetReport {
    label: String,
    files: Option<usize>,
    samples: Vec<Sample>,
}

pub(crate) fn scale_cliff_benchmark(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    run("cargo", &["build", "--release", "-p", "ripr"])?;
    let binary = release_binary();

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let scratch = PathBuf::from("target/ripr/reports").join(format!("scale-cliff-{stamp}"));
    fs::create_dir_all(&scratch)
        .map_err(|err| format!("create benchmark scratch {}: {err}", scratch.display()))?;

    let result = collect_targets(&binary, &scratch, &options);
    if !options.keep_workspaces {
        let _ = fs::remove_dir_all(&scratch);
    }
    let (targets, identity) = result?;

    let report = build_report(&options, &targets, &binary, &identity);
    let json_text = serde_json::to_string_pretty(&report)
        .map_err(|err| format!("serialize scale cliff benchmark: {err}"))?;
    crate::write_report("scale-cliff-benchmark.json", &format!("{json_text}\n"))?;
    crate::write_report("scale-cliff-benchmark.md", &benchmark_markdown(&report))?;
    println!("Wrote target/ripr/reports/scale-cliff-benchmark.json");
    println!("Wrote target/ripr/reports/scale-cliff-benchmark.md");
    Ok(())
}

fn release_binary() -> PathBuf {
    let name = format!("ripr{}", std::env::consts::EXE_SUFFIX);
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target"))
        .join("release")
        .join(name)
}

fn collect_targets(
    binary: &Path,
    scratch: &Path,
    options: &Options,
) -> Result<(Vec<TargetReport>, MeasuredIdentity), String> {
    let mut identity = measured_identity(options);
    let mut targets = Vec::new();
    if let Some((repo, base)) = &options.repo {
        let (target, captured) = bind_repo_target(repo, base);
        identity = captured;
        targets.push(target);
    } else {
        for files in &options.sizes {
            let root = generate_workspace(scratch, *files)?;
            targets.push(Target {
                label: format!("synthetic-{files}"),
                files: Some(*files),
                root,
                base: "HEAD~1".to_string(),
            });
        }
    }
    let mut reports = Vec::new();
    for target in targets {
        let mut samples = Vec::new();
        for command in &options.commands {
            // A spawn or capture error is a measurement outcome, not a reason
            // to discard the samples already collected.
            samples.push(
                run_sample(binary, scratch, &target, *command, options).unwrap_or_else(|err| {
                    Sample {
                        command: command.name(),
                        status: "fail".to_string(),
                        duration_ms: 0,
                        peak_rss_kib: None,
                        findings: None,
                        detail: Some(err),
                    }
                }),
            );
        }
        reports.push(TargetReport {
            label: target.label,
            files: target.files,
            samples,
        });
    }
    if let Some((repo, _)) = &options.repo
        && identity_drifted(repo, &identity)
        && let Some(report) = reports.last_mut()
    {
        report.samples.push(head_drift_sample(
            options.commands.first().copied().unwrap_or(Command::Check),
        ));
    }
    Ok((reports, identity))
}

/// One source file per module, three small functions and two inline tests
/// each, committed twice so `HEAD~1..HEAD` is a one-file behavior change.
fn generate_workspace(scratch: &Path, files: usize) -> Result<PathBuf, String> {
    let root = scratch.join(format!("ws-{files}"));
    let src = root.join("src");
    fs::create_dir_all(&src)
        .map_err(|err| format!("create generated workspace {}: {err}", root.display()))?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scalecliff\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .map_err(|err| format!("write generated Cargo.toml: {err}"))?;
    let mut lib = String::new();
    for index in 0..files {
        lib.push_str(&format!("mod m{index};\n"));
        fs::write(src.join(format!("m{index}.rs")), module_source(index, 2))
            .map_err(|err| format!("write generated module m{index}: {err}"))?;
    }
    fs::write(src.join("lib.rs"), lib).map_err(|err| format!("write generated lib.rs: {err}"))?;
    git(&root, &["init", "-q", "-b", "main"])?;
    git(&root, &["add", "-A"])?;
    git(&root, &["commit", "-q", "-m", "base"])?;
    fs::write(src.join("m0.rs"), module_source(0, 3))
        .map_err(|err| format!("write changed module m0: {err}"))?;
    git(&root, &["commit", "-q", "-am", "change"])?;
    Ok(root)
}

fn module_source(index: usize, factor: u32) -> String {
    format!(
        "pub fn f{index}_a(x: u32) -> u32 {{ if x > {index} {{ x * {factor} }} else {{ x + 1 }} }}\n\
         pub fn f{index}_b(s: &str) -> usize {{ s.len() + {index} }}\n\
         pub fn f{index}_c(flag: bool) -> u32 {{ if flag {{ {index} }} else {{ 0 }} }}\n\
         #[cfg(test)]\nmod tests {{\n    use super::*;\n    #[test]\n    fn a_{index}() {{ assert_eq!(f{index}_a({index} + 1), {factor} * ({index} + 1)); }}\n    #[test]\n    fn b_{index}() {{ assert_eq!(f{index}_b(\"ab\"), {index} + 2); }}\n}}\n"
    )
}

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
    let mut full: Vec<String> = vec![
        "-c".to_string(),
        "user.name=ripr-bench".to_string(),
        "-c".to_string(),
        "user.email=bench@example.invalid".to_string(),
    ];
    full.extend(args.iter().map(|arg| arg.to_string()));
    let output = capture_output_in_dir("git", &full, root, "scale cliff benchmark git")?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git {} failed in {}: {}",
            args.join(" "),
            root.display(),
            output.stderr.trim()
        ))
    }
}

fn run_sample(
    binary: &Path,
    scratch: &Path,
    target: &Target,
    command: Command,
    options: &Options,
) -> Result<Sample, String> {
    let root = target.root.display().to_string();
    let mut child_args: Vec<String> = match command {
        Command::Check => vec![
            "check".to_string(),
            "--root".to_string(),
            root,
            "--base".to_string(),
            target.base.clone(),
            "--mode".to_string(),
            options.mode.clone(),
            "--format".to_string(),
            "json".to_string(),
        ],
        Command::Pilot => {
            let out = scratch.join(format!("pilot-{}", target.label.replace(['/', '\\'], "_")));
            vec![
                "pilot".to_string(),
                "--root".to_string(),
                root,
                "--out".to_string(),
                out.display().to_string(),
                "--quiet".to_string(),
            ]
        }
    };
    let (program, wrapped) = if cfg!(target_os = "linux") && Path::new(GNU_TIME).exists() {
        let mut wrapped = vec!["-v".to_string(), binary.display().to_string()];
        wrapped.append(&mut child_args);
        (GNU_TIME.to_string(), wrapped)
    } else {
        (binary.display().to_string(), child_args)
    };
    let mut envs: Vec<(&str, &str)> = Vec::new();
    if let Some(cap) = &options.index_cap {
        envs.push((INDEX_CAP_ENV, cap.as_str()));
    }
    let output = capture_output_with_timeout(
        &program,
        &wrapped,
        &envs,
        Duration::from_millis(options.timeout_ms),
        "scale cliff benchmark",
    )?;
    let peak_rss_kib = parse_peak_rss_kib(&output.stderr);
    let succeeded = output.status.is_some_and(|status| status.success());
    let (status, findings, detail) = if output.timed_out {
        ("timeout", None, None)
    } else if succeeded {
        (
            "pass",
            match command {
                Command::Check => parse_findings(&output.stdout),
                Command::Pilot => None,
            },
            None,
        )
    } else if output.stderr.contains("diff_scope_oversized") {
        (
            "refused_oversized",
            None,
            Some(summarize_error(&output.stderr)),
        )
    } else {
        ("fail", None, Some(summarize_error(&output.stderr)))
    };
    Ok(Sample {
        command: command.name(),
        status: status.to_string(),
        duration_ms: output.duration.as_millis(),
        peak_rss_kib,
        findings,
        detail,
    })
}

fn parse_peak_rss_kib(stderr: &str) -> Option<u64> {
    stderr
        .lines()
        .find_map(|line| line.trim().strip_prefix(RSS_LINE))
        .and_then(|rest| rest.trim().parse().ok())
}

fn parse_findings(stdout: &str) -> Option<u64> {
    serde_json::from_str::<Value>(stdout)
        .ok()?
        .pointer("/summary/findings")?
        .as_u64()
}

/// The refusal or failure line, not the whole stderr: progress heartbeats and
/// GNU time's resource block would bury it.
fn summarize_error(stderr: &str) -> String {
    const LIMIT: usize = 400;
    let line = stderr
        .lines()
        .rev()
        .find(|line| line.starts_with("ripr:") && !line.contains("progress"))
        .unwrap_or("")
        .trim();
    if line.chars().count() <= LIMIT {
        return line.to_string();
    }
    format!("{}…", line.chars().take(LIMIT).collect::<String>())
}

/// Log-log slope of wall time against file count between the smallest and
/// largest passing synthetic sizes: ~1 is linear, ~2 quadratic.
fn scaling_exponent(targets: &[TargetReport], command: &str) -> Option<f64> {
    let points: Vec<(f64, f64)> = targets
        .iter()
        .filter_map(|target| {
            let files = target.files? as f64;
            let sample = target
                .samples
                .iter()
                .find(|sample| sample.command == command && sample.status == "pass")?;
            let ms = sample.duration_ms as f64;
            (ms > 0.0).then_some((files, ms))
        })
        .collect();
    let first = points.first()?;
    let last = points.last()?;
    if last.0 <= first.0 {
        return None;
    }
    Some((last.1.ln() - first.1.ln()) / (last.0.ln() - first.0.ln()))
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut sizes = None;
    let mut repo: Option<PathBuf> = None;
    let mut base: Option<String> = None;
    let mut mode = "draft".to_string();
    let mut commands = vec![Command::Check, Command::Pilot];
    let mut index_cap = Some(DEFAULT_INDEX_CAP.to_string());
    let mut timeout_ms = DEFAULT_TIMEOUT_MS;
    let mut keep_workspaces = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--sizes" => {
                index += 1;
                sizes = Some(parse_sizes(required_arg(args, index, "--sizes")?)?);
            }
            "--repo" => {
                index += 1;
                repo = Some(PathBuf::from(required_arg(args, index, "--repo")?));
            }
            "--base" => {
                index += 1;
                base = Some(required_arg(args, index, "--base")?.to_string());
            }
            "--mode" => {
                index += 1;
                let value = required_arg(args, index, "--mode")?;
                if !["instant", "draft", "deep"].contains(&value) {
                    return Err(format!("--mode must be instant, draft or deep\n{USAGE}"));
                }
                mode = value.to_string();
            }
            "--commands" => {
                index += 1;
                commands = parse_commands(required_arg(args, index, "--commands")?)?;
            }
            "--index-cap" => {
                index += 1;
                let value = required_arg(args, index, "--index-cap")?;
                index_cap = if value == "product" {
                    None
                } else {
                    value.parse::<u64>().map_err(|err| {
                        format!("--index-cap must be a number or `product` ({err})\n{USAGE}")
                    })?;
                    Some(value.to_string())
                };
            }
            "--timeout-ms" => {
                index += 1;
                timeout_ms = required_arg(args, index, "--timeout-ms")?
                    .parse::<u64>()
                    .map_err(|err| {
                        format!("--timeout-ms must be a positive integer ({err})\n{USAGE}")
                    })?;
                if timeout_ms == 0 {
                    return Err(format!("--timeout-ms must be positive\n{USAGE}"));
                }
            }
            "--keep-workspaces" => keep_workspaces = true,
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
        index += 1;
    }
    let repo = match (repo, base) {
        (Some(repo), Some(base)) => Some((repo, base)),
        (None, None) => None,
        _ => return Err(format!("--repo and --base must be given together\n{USAGE}")),
    };
    let sizes = match sizes {
        Some(sizes) => sizes,
        None => parse_sizes(DEFAULT_SIZES)?,
    };
    Ok(Options {
        sizes,
        repo,
        mode,
        commands,
        index_cap,
        timeout_ms,
        keep_workspaces,
    })
}

fn required_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{flag} requires a value\n{USAGE}"))
}

fn parse_sizes(value: &str) -> Result<Vec<usize>, String> {
    let mut sizes = Vec::new();
    for part in value.split(',') {
        let size: usize = part
            .trim()
            .parse()
            .map_err(|err| format!("invalid size `{part}` ({err})\n{USAGE}"))?;
        if size == 0 || size > MAX_SIZE {
            return Err(format!("size {size} must be between 1 and {MAX_SIZE}"));
        }
        sizes.push(size);
    }
    sizes.sort_unstable();
    sizes.dedup();
    Ok(sizes)
}

fn parse_commands(value: &str) -> Result<Vec<Command>, String> {
    let mut commands = Vec::new();
    for part in value.split(',') {
        let command = match part.trim() {
            "check" => Command::Check,
            "pilot" => Command::Pilot,
            other => return Err(format!("unknown command `{other}`\n{USAGE}")),
        };
        if !commands.contains(&command) {
            commands.push(command);
        }
    }
    Ok(commands)
}

fn build_report(
    options: &Options,
    targets: &[TargetReport],
    binary: &Path,
    identity: &MeasuredIdentity,
) -> Value {
    let revision = identity.revision.as_str();
    let base_revision = identity.base_revision.as_deref();
    json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "report": "scale-cliff-benchmark",
        "status": report_status(targets),
        "revision": revision,
        "base_revision": base_revision,
        "runner_class": runner_class(),
        "analyzer_version": analyzer_version(binary),
        "binary": "release",
        "mode": options.mode,
        "index_cap": options.index_cap.as_deref().unwrap_or("product"),
        "timeout_ms": options.timeout_ms,
        "corpus": match &options.repo {
            Some((repo, base)) => json!({
                "kind": "repo",
                "path": repo.display().to_string(),
                "base": base,
                "revision": revision,
                "base_revision": base_revision,
            }),
            None => json!({"kind": "synthetic", "sizes": options.sizes, "generator": "N modules, 3 fns and 2 inline tests each; one-file change in m0"}),
        },
        "targets": targets.iter().map(|target| json!({
            "label": target.label,
            "files": target.files,
            "samples": target.samples.iter().map(|sample| json!({
                "command": sample.command,
                "status": sample.status,
                "duration_ms": sample.duration_ms,
                "peak_rss_kib": sample.peak_rss_kib,
                "findings": sample.findings,
                "detail": sample.detail,
                "hint": next_step_hint(&sample.status),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "scaling_exponent": {
            "check": scaling_exponent(targets, "check"),
            "pilot": scaling_exponent(targets, "pilot"),
        },
        "claim_boundary": "One cold run per target and command on the recorded revision, runner class and corpus. With --repo, revision and base_revision are captured from the target repository before sampling and the resolved base SHA is what check receives; a HEAD move during the run is failed, not a cliff. A fail sample is status failed, not a size cliff; timeout and refused_oversized name the next flag. A synthetic corpus has a trivial call graph, so it bounds file-count cost only; call-graph-driven cost (name-colliding Rust code) needs a real repository via --repo. Peak RSS is null where GNU time is unavailable. Not a gate and not universal latency."
    })
}

/// Top-level status is `failed` when any sample is a plain failure. A size
/// cliff (`timeout` / `refused_oversized`) is `cliff_observed` only when every
/// non-pass sample is one of those cliffs. Unknown sample statuses fail closed.
fn report_status(targets: &[TargetReport]) -> &'static str {
    let mut saw_fail = false;
    let mut saw_cliff = false;
    for sample in targets.iter().flat_map(|target| &target.samples) {
        match sample.status.as_str() {
            "pass" => {}
            "timeout" | "refused_oversized" => saw_cliff = true,
            _ => saw_fail = true,
        }
    }
    if saw_fail {
        "failed"
    } else if saw_cliff {
        "cliff_observed"
    } else {
        "pass"
    }
}

fn next_step_hint(status: &str) -> Option<&'static str> {
    match status {
        "timeout" => Some("raise `--timeout-ms`"),
        "refused_oversized" => Some("rerun with `--index-cap <n>`"),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MeasuredIdentity {
    revision: String,
    base_revision: Option<String>,
}

fn measured_identity(options: &Options) -> MeasuredIdentity {
    match &options.repo {
        Some((path, base)) => bind_repo_target(path, base).1,
        None => MeasuredIdentity {
            revision: git_revision(),
            base_revision: None,
        },
    }
}

/// Resolve the target HEAD and `--base` before any sample so a moving ref
/// cannot rename the receipt after the fact. `check` receives the captured
/// base SHA, not `HEAD~1`.
fn bind_repo_target(repo: &Path, base: &str) -> (Target, MeasuredIdentity) {
    let identity = MeasuredIdentity {
        revision: git_revision_in(repo, "HEAD"),
        base_revision: Some(git_revision_in(repo, base)),
    };
    let check_base = match identity.base_revision.as_deref() {
        Some("unavailable") | None => base.to_string(),
        Some(sha) => sha.to_string(),
    };
    (
        Target {
            label: repo.display().to_string(),
            files: None,
            root: repo.to_path_buf(),
            base: check_base,
        },
        identity,
    )
}

fn identity_drifted(repo: &Path, captured: &MeasuredIdentity) -> bool {
    git_revision_in(repo, "HEAD") != captured.revision
}

fn head_drift_sample(command: Command) -> Sample {
    Sample {
        command: command.name(),
        status: "fail".to_string(),
        duration_ms: 0,
        peak_rss_kib: None,
        findings: None,
        detail: Some(
            "target HEAD moved during the run; recorded revision is the pre-sample capture"
                .to_string(),
        ),
    }
}

fn benchmark_markdown(report: &Value) -> String {
    let mut rows = String::new();
    if let Some(targets) = report["targets"].as_array() {
        for target in targets {
            let Some(samples) = target["samples"].as_array() else {
                continue;
            };
            for sample in samples {
                let rss = sample["peak_rss_kib"]
                    .as_u64()
                    .map_or_else(|| "n/a".to_string(), |kib| format!("{} MiB", kib / 1024));
                let hint = sample["hint"].as_str().unwrap_or("");
                rows.push_str(&format!(
                    "| {} | {} | {} | {} ms | {} | {} |\n",
                    target["label"].as_str().unwrap_or("?"),
                    sample["command"].as_str().unwrap_or("?"),
                    sample["status"].as_str().unwrap_or("?"),
                    sample["duration_ms"],
                    rss,
                    hint,
                ));
            }
        }
    }
    let base_line = report["base_revision"]
        .as_str()
        .map(|base| format!("Base: `{base}`\n"))
        .unwrap_or_default();
    format!(
        "# Scale Cliff Benchmark\n\nStatus: `{}`\n\nRevision: `{}`\n{}Runner: `{}`\nAnalyzer: `{}`\nMode: `{}`; index cap: `{}`; timeout: {} ms\n\n| Target | Command | Status | Wall | Peak RSS | Next |\n| --- | --- | --- | ---: | ---: | --- |\n{}\nScaling exponent (log-log, smallest to largest passing size): check {}, pilot {}.\n\nClaim boundary: {}\n",
        report["status"].as_str().unwrap_or("unknown"),
        report["revision"].as_str().unwrap_or("unavailable"),
        base_line,
        report["runner_class"].as_str().unwrap_or("unknown"),
        report["analyzer_version"].as_str().unwrap_or("unknown"),
        report["mode"].as_str().unwrap_or("unknown"),
        report["index_cap"].as_str().unwrap_or("unknown"),
        report["timeout_ms"],
        rows,
        report["scaling_exponent"]["check"],
        report["scaling_exponent"]["pilot"],
        report["claim_boundary"].as_str().unwrap_or("unknown"),
    )
}

fn git_revision() -> String {
    std::env::current_dir()
        .map(|cwd| git_revision_in(&cwd, "HEAD"))
        .unwrap_or_else(|_| "unavailable".to_string())
}

fn git_revision_in(root: &Path, rev: &str) -> String {
    let spec = format!("{rev}^{{commit}}");
    let args = ["rev-parse".to_string(), "--verify".to_string(), spec];
    capture_output_in_dir("git", &args, root, "scale cliff revision")
        .ok()
        .filter(|output| output.status.success())
        .map(|output| output.stdout.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unavailable".to_string())
}

fn runner_class() -> String {
    std::env::var("RUNNER_NAME")
        .or_else(|_| std::env::var("GITHUB_RUNNER_NAME"))
        .unwrap_or_else(|_| format!("local-{}-{}", std::env::consts::OS, std::env::consts::ARCH))
}

fn analyzer_version(binary: &Path) -> String {
    crate::run::run_output_owned(&binary.display().to_string(), &["--version".to_string()])
        .ok()
        .map(|output| output.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unavailable".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rejected<T>(result: Result<T, String>) -> bool {
        result.is_err()
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn defaults_run_both_commands_with_a_raised_index_cap() -> Result<(), String> {
        let options = parse_options(&[])?;
        assert_eq!(options.sizes, vec![250, 1000, 4000]);
        assert_eq!(options.commands, vec![Command::Check, Command::Pilot]);
        assert_eq!(options.index_cap.as_deref(), Some(DEFAULT_INDEX_CAP));
        assert_eq!(options.mode, "draft");
        Ok(())
    }

    #[test]
    fn repo_and_base_must_come_together() {
        assert!(rejected(parse_options(&args(&["--repo", "x"]))));
        assert!(rejected(parse_options(&args(&["--base", "HEAD~1"]))));
    }

    #[test]
    fn product_index_cap_is_not_overridden() -> Result<(), String> {
        let options = parse_options(&args(&["--index-cap", "product"]))?;
        assert_eq!(options.index_cap, None);
        Ok(())
    }

    #[test]
    fn rejects_zero_oversize_and_unknown_inputs() {
        assert!(rejected(parse_sizes("0")));
        assert!(rejected(parse_sizes("100001")));
        assert!(rejected(parse_commands("check,deep")));
        assert!(rejected(parse_options(&args(&["--mode", "fast"]))));
        assert!(rejected(parse_options(&args(&["--timeout-ms", "0"]))));
        assert!(rejected(parse_options(&args(&["--nope"]))));
    }

    #[test]
    fn peak_rss_is_read_from_gnu_time_output() {
        let stderr = "ripr progress: completed\n\tCommand being timed: \"ripr\"\n\tMaximum resident set size (kbytes): 123456\n";
        assert_eq!(parse_peak_rss_kib(stderr), Some(123_456));
        assert_eq!(parse_peak_rss_kib("no resource block"), None);
    }

    #[test]
    fn error_summary_names_the_refusal_not_the_heartbeats() {
        let stderr = "ripr progress: analyzing still active after 2s\nripr: diff_scope_oversized: 36646 indexed Rust files exceed the limit\n\tCommand exited with non-zero status 2\n";
        assert!(summarize_error(stderr).starts_with("ripr: diff_scope_oversized"));
    }

    fn passing(files: usize, duration_ms: u128) -> TargetReport {
        TargetReport {
            label: format!("synthetic-{files}"),
            files: Some(files),
            samples: vec![Sample {
                command: "check",
                status: "pass".to_string(),
                duration_ms,
                peak_rss_kib: None,
                findings: Some(1),
                detail: None,
            }],
        }
    }

    #[test]
    fn exponent_is_one_for_linear_and_two_for_quadratic() -> Result<(), String> {
        let linear = scaling_exponent(&[passing(100, 1000), passing(400, 4000)], "check")
            .ok_or("linear exponent missing")?;
        assert!((linear - 1.0).abs() < 1e-9);
        let quadratic = scaling_exponent(&[passing(100, 1000), passing(400, 16000)], "check")
            .ok_or("quadratic exponent missing")?;
        assert!((quadratic - 2.0).abs() < 1e-9);
        Ok(())
    }

    #[test]
    fn exponent_ignores_a_target_that_did_not_pass() {
        let slow = with_status(passing(400, 4000), "timeout");
        assert_eq!(scaling_exponent(&[passing(100, 1000), slow], "check"), None);
    }

    fn draft_options() -> Options {
        Options {
            sizes: vec![100, 400],
            repo: None,
            mode: "draft".to_string(),
            commands: vec![Command::Check],
            index_cap: None,
            timeout_ms: 1000,
            keep_workspaces: false,
        }
    }

    fn with_status(mut target: TargetReport, status: &str) -> TargetReport {
        target.samples[0].status = status.to_string();
        target
    }

    fn report(options: &Options, targets: &[TargetReport]) -> Value {
        build_report(
            options,
            targets,
            Path::new("ripr"),
            &measured_identity(options),
        )
    }

    #[test]
    fn report_marks_a_non_pass_target_as_a_cliff() {
        let slow = with_status(passing(400, 4000), "timeout");
        let report = report(&draft_options(), &[passing(100, 1000), slow]);
        assert_eq!(report["status"], "cliff_observed");
        assert_eq!(report["index_cap"], "product");
    }

    #[test]
    fn all_passing_samples_are_pass_and_synthetic_identity_is_the_checkout() {
        let report = report(&draft_options(), &[passing(100, 1000), passing(400, 4000)]);
        assert_eq!(report["status"], "pass");
        assert_eq!(report["revision"], git_revision());
        assert!(report["base_revision"].is_null());
        assert_eq!(report["schema_version"], SCHEMA_VERSION);
    }

    #[test]
    fn a_failed_sample_is_failed_not_a_cliff() {
        let failed = with_status(passing(100, 10), "fail");
        let report = report(&draft_options(), &[failed]);
        assert_eq!(report["status"], "failed");
    }

    #[test]
    fn a_failed_sample_beats_a_timeout_for_top_level_status() {
        let failed = with_status(passing(100, 10), "fail");
        let timed_out = with_status(passing(400, 4000), "timeout");
        let report = report(&draft_options(), &[failed, timed_out]);
        assert_eq!(report["status"], "failed");
    }

    #[test]
    fn refused_oversized_is_a_cliff_not_a_failure() {
        let refused = with_status(passing(400, 4000), "refused_oversized");
        let report = report(&draft_options(), &[refused]);
        assert_eq!(report["status"], "cliff_observed");
    }

    #[test]
    fn timeout_and_refused_samples_name_the_next_flag() {
        let timed_out = with_status(passing(100, 10), "timeout");
        let refused = with_status(passing(400, 20), "refused_oversized");
        let report = report(&draft_options(), &[timed_out, refused]);
        assert_eq!(
            report["targets"][0]["samples"][0]["hint"],
            "raise `--timeout-ms`"
        );
        assert_eq!(
            report["targets"][1]["samples"][0]["hint"],
            "rerun with `--index-cap <n>`"
        );
        assert!(report["targets"][0]["samples"][0]["detail"].is_null());
    }

    #[test]
    fn pass_and_fail_samples_carry_no_next_flag() {
        let failed = with_status(passing(400, 20), "fail");
        let report = report(&draft_options(), &[passing(100, 10), failed]);
        assert!(report["targets"][0]["samples"][0]["hint"].is_null());
        assert!(report["targets"][1]["samples"][0]["hint"].is_null());
        assert_eq!(report["status"], "failed");
    }

    struct TempRepo {
        root: PathBuf,
    }

    impl TempRepo {
        fn create(label: &str) -> Result<Self, String> {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default();
            let root = std::env::temp_dir().join(format!("ripr-scale-cliff-{label}-{stamp}"));
            fs::create_dir_all(&root).map_err(|err| format!("create fixture repo: {err}"))?;
            Ok(Self { root })
        }

        fn commit(&self, contents: &str, message: &str) -> Result<String, String> {
            fs::write(self.root.join("note"), contents)
                .map_err(|err| format!("write fixture: {err}"))?;
            if !self.root.join(".git").exists() {
                git(&self.root, &["init", "-q", "-b", "main"])?;
                git(&self.root, &["add", "-A"])?;
            }
            git(&self.root, &["commit", "-q", "-am", message])?;
            self.rev("HEAD")
        }

        fn rev(&self, spec: &str) -> Result<String, String> {
            let output = capture_output_in_dir(
                "git",
                &["rev-parse".into(), spec.into()],
                &self.root,
                "fixture rev-parse",
            )?;
            if !output.status.success() {
                return Err(format!(
                    "fixture rev-parse {spec} failed: {}",
                    output.stderr
                ));
            }
            Ok(output.stdout.trim().to_string())
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn repo_receipt_records_target_head_and_resolved_base() -> Result<(), String> {
        let repo = TempRepo::create("rev")?;
        let base = repo.commit("one", "base")?;
        let head = repo.commit("two", "change")?;
        assert_ne!(
            head,
            git_revision(),
            "fixture HEAD must differ from the xtask checkout"
        );
        assert_ne!(head, base);

        let mut options = draft_options();
        options.repo = Some((repo.root.clone(), "HEAD~1".to_string()));
        let report = report(&options, &[passing(100, 10)]);
        assert_eq!(report["revision"], head);
        assert_eq!(report["base_revision"], base);
        assert_eq!(report["corpus"]["revision"], head);
        assert_eq!(report["corpus"]["base_revision"], base);
        assert_eq!(report["corpus"]["base"], "HEAD~1");
        Ok(())
    }

    #[test]
    fn missing_repo_base_records_unavailable_not_the_xtask_checkout() -> Result<(), String> {
        let repo = TempRepo::create("missing-base")?;
        let head = repo.commit("one", "base")?;
        let mut options = draft_options();
        options.repo = Some((repo.root.clone(), "no-such-rev".to_string()));
        let report = report(&options, &[with_status(passing(100, 10), "fail")]);
        assert_eq!(report["revision"], head);
        assert_eq!(report["base_revision"], "unavailable");
        assert_eq!(report["status"], "failed");
        assert_ne!(report["revision"], git_revision());
        Ok(())
    }

    #[test]
    fn bind_repo_target_passes_the_captured_base_sha_not_the_moving_ref() -> Result<(), String> {
        let repo = TempRepo::create("bind-base")?;
        let base = repo.commit("one", "base")?;
        let head = repo.commit("two", "change")?;
        let (target, identity) = bind_repo_target(&repo.root, "HEAD~1");
        assert_eq!(target.base, base);
        assert_eq!(identity.revision, head);
        assert_eq!(identity.base_revision.as_deref(), Some(base.as_str()));
        repo.commit("three", "later")?;
        assert_ne!(repo.rev("HEAD~1")?, base);
        assert_eq!(target.base, base);
        assert!(identity_drifted(&repo.root, &identity));
        Ok(())
    }

    #[test]
    fn report_keeps_the_pre_sample_identity_after_head_moves() -> Result<(), String> {
        let repo = TempRepo::create("captured-id")?;
        let base = repo.commit("one", "base")?;
        let head = repo.commit("two", "change")?;
        let mut options = draft_options();
        options.repo = Some((repo.root.clone(), "HEAD~1".to_string()));
        let identity = measured_identity(&options);
        repo.commit("three", "later")?;
        let report = build_report(&options, &[passing(100, 10)], Path::new("ripr"), &identity);
        assert_eq!(report["revision"], head);
        assert_eq!(report["base_revision"], base);
        assert_ne!(report["revision"], git_revision_in(&repo.root, "HEAD"));
        Ok(())
    }

    #[test]
    fn head_drift_is_a_plain_failure_not_a_cliff() {
        let sample = head_drift_sample(Command::Check);
        assert_eq!(sample.status, "fail");
        assert!(
            sample
                .detail
                .as_deref()
                .is_some_and(|detail| { detail.contains("target HEAD moved during the run") })
        );
        let report = report(
            &draft_options(),
            &[TargetReport {
                label: "repo".to_string(),
                files: None,
                samples: vec![sample],
            }],
        );
        assert_eq!(report["status"], "failed");
    }

    #[test]
    fn markdown_names_failed_status_target_base_and_next_flag() {
        let failed = with_status(passing(100, 10), "fail");
        let timed_out = with_status(passing(400, 20), "timeout");
        let mut options = draft_options();
        options.repo = Some((PathBuf::from("/tmp/not-a-real-repo"), "HEAD~1".to_string()));
        let report = report(&options, &[failed, timed_out]);
        let markdown = benchmark_markdown(&report);
        assert!(markdown.contains("Status: `failed`"));
        assert!(markdown.contains("Base: `unavailable`"));
        assert!(markdown.contains("| Next |"));
        assert!(markdown.contains("raise `--timeout-ms`"));
        assert!(!markdown.contains("cliff_observed"));
    }

    #[test]
    fn generated_module_embeds_the_factor_so_the_change_commit_differs() {
        assert_ne!(module_source(0, 2), module_source(0, 3));
        assert!(module_source(7, 3).contains("x * 3"));
    }
}
