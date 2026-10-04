//! `cargo xtask mutation-spot-check` — ground-truth spot check of static grip
//! verdicts against real cargo-mutants outcomes on real repositories.
//!
//! For each supplied checkout the command writes a `repo-exposure-json`
//! snapshot, obtains a cargo-mutants `mutants.out` directory (supplied, or run
//! with `--run-mutants`), and joins the two through the product
//! `ripr calibrate cargo-mutants` path so the join stays owned by one
//! implementation. The harness then scores only the joins it can trust:
//!
//! - `seam_precise`: an operator mutant (cargo-mutants `BinaryOperator` or
//!   `UnaryOperator`) whose original operator token appears in the expression
//!   of a `predicate_boundary` or `return_value` seam on the same line. The
//!   mutant changes the seam's own behavior, so its outcome tests the verdict.
//! - `function_body`: a `FnValue` mutant that replaces the whole function
//!   body. It is caught or missed by the function's tests as a whole, not by a
//!   discriminator for the joined seam; reported, never scored.
//! - `same_line_other`: any other unambiguous file/line join (a call-presence
//!   seam joined to an arithmetic mutant on the same line, for example);
//!   reported, never scored.
//!
//! Scored verdict families: `strongly_gripped` claims a discriminator exists
//! (a caught mutant agrees, a missed one is an overclaim); `ungripped` and
//! `reachable_unrevealed` claim none does (a missed mutant agrees, a caught one
//! is a false gap). `weakly_gripped` and the unknown classes do not make a
//! claim a single operator mutant can settle, so their outcomes are counted
//! but not scored. Claims are limited to the recorded checkout revisions, the
//! cargo-mutants version that produced the outcomes, and this join rule.
//!
//! The report also scores `ripr pilot`'s top recommendations against the same
//! outcomes (see [`pilot`]), because a wrong top recommendation is the error a
//! developer meets first.

mod pilot;

use crate::run::{
    capture_bytes_in_dir_with_timeout, capture_output_with_timeout,
    capture_stdout_to_file_with_timeout,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const SCHEMA_VERSION: &str = "ripr-mutation-spot-check-v1";
const DEFAULT_JOBS: usize = 2;
const DEFAULT_MUTANT_TIMEOUT_SECS: u64 = 60;
const DEFAULT_EXAMPLES: usize = 10;
const EXPOSURE_TIMEOUT: Duration = Duration::from_mins(15);
const CALIBRATE_TIMEOUT: Duration = Duration::from_mins(5);
const MUTANTS_TIMEOUT: Duration = Duration::from_hours(4);
const SCRATCH: &str = "target/ripr/reports/mutation-spot-check";
/// cargo-mutants options the harness sets itself, that would mutate a tree
/// other than the analyzed checkout, or that stop cargo-mutants from writing
/// outcomes. `--mutants-arg` is for selecting mutants.
const HARNESS_OWNED_MUTANTS_ARGS: &[&str] = &[
    "--dir",
    "--output",
    "--jobs",
    "--timeout",
    // The harness always passes --timeout: cargo-mutants rejects the
    // multiplier alongside it and ignores the minimum, so neither can apply.
    "--timeout-multiplier",
    "--minimum-test-timeout",
    "--manifest-path",
    "--shuffle",
    "--no-shuffle",
    "--in-place",
    "--list",
    "--list-files",
    "--check",
    "--json",
    "--completions",
    "--emit-schema",
    "--version",
];
/// Short forms of `--dir`, `--output`, `--jobs` and `--timeout`.
const HARNESS_OWNED_SHORT_FLAGS: &[char] = &['d', 'o', 'j', 't'];
/// cargo-mutants 27 short flags that take no value (`--caught`, `--unviable`,
/// `--help`), so a bundled argument such as `-vj8` continues past them.
const VALUELESS_SHORT_FLAGS: &[char] = &['v', 'V', 'h'];
const USAGE: &str = "usage: cargo xtask mutation-spot-check --repo <name>=<checkout> [--repo ...] [--mutants-out <name>=<mutants.out dir>] [--run-mutants] [--mutants-arg <name>=<arg>] [--jobs <n>] [--mutant-timeout-secs <n>] [--examples <n>] [--ripr <binary>]";

fn harness_owned_mutants_arg(arg: &str) -> bool {
    if let Some(bundle) = arg.strip_prefix('-').filter(|rest| !rest.starts_with('-')) {
        // clap reads `-vj8` as `-v -j 8`: scan valueless flags until the
        // first flag that takes a value, whose remainder is that value.
        for flag in bundle.chars() {
            if HARNESS_OWNED_SHORT_FLAGS.contains(&flag) {
                return true;
            }
            if !VALUELESS_SHORT_FLAGS.contains(&flag) {
                return false;
            }
        }
        return false;
    }
    HARNESS_OWNED_MUTANTS_ARGS.iter().any(|owned| {
        arg == *owned
            || arg
                .strip_prefix(owned)
                .is_some_and(|rest| rest.starts_with('='))
    })
}

pub(crate) fn mutation_spot_check(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    let binary = match &options.ripr {
        Some(path) => path.clone(),
        None => {
            crate::run::run("cargo", &["build", "-p", "ripr"])?;
            crate::ripr_debug_binary()
        }
    };
    for (name, checkout) in &options.repos {
        reject_checkout_inside_workspace(name, checkout)?;
    }
    let scratch = PathBuf::from(SCRATCH);
    fs::create_dir_all(&scratch)
        .map_err(|err| format!("create spot-check scratch {}: {err}", scratch.display()))?;

    let mut repos = Vec::new();
    for (name, checkout) in &options.repos {
        repos.push(spot_check_repo(
            &binary, &scratch, name, checkout, &options,
        )?);
    }
    let report = build_report(&repos, options.examples);
    let json_text = serde_json::to_string_pretty(&report)
        .map_err(|err| format!("serialize mutation spot check: {err}"))?;
    crate::write_report("mutation-spot-check.json", &format!("{json_text}\n"))?;
    crate::write_report("mutation-spot-check.md", &spot_check_markdown(&report))?;
    println!("Wrote target/ripr/reports/mutation-spot-check.json");
    println!("Wrote target/ripr/reports/mutation-spot-check.md");
    Ok(())
}

#[derive(Clone, Debug)]
struct Options {
    repos: Vec<(String, PathBuf)>,
    mutants_out: BTreeMap<String, PathBuf>,
    mutants_args: BTreeMap<String, Vec<String>>,
    run_mutants: bool,
    jobs: usize,
    mutant_timeout_secs: u64,
    examples: usize,
    ripr: Option<PathBuf>,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        repos: Vec::new(),
        mutants_out: BTreeMap::new(),
        mutants_args: BTreeMap::new(),
        run_mutants: false,
        jobs: DEFAULT_JOBS,
        mutant_timeout_secs: DEFAULT_MUTANT_TIMEOUT_SECS,
        examples: DEFAULT_EXAMPLES,
        ripr: None,
    };
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        match flag {
            "--repo" | "--mutants-out" => {
                index += 1;
                let (name, path) = parse_named_path(required_arg(args, index, flag)?, flag)?;
                if flag == "--repo" {
                    if options.repos.iter().any(|(existing, _)| *existing == name) {
                        return Err(format!("duplicate --repo name `{name}`"));
                    }
                    options.repos.push((name, path));
                } else if options.mutants_out.insert(name.clone(), path).is_some() {
                    return Err(format!("duplicate --mutants-out name `{name}`"));
                }
            }
            "--mutants-arg" => {
                index += 1;
                let value = required_arg(args, index, flag)?;
                let Some((name, arg)) = value
                    .split_once('=')
                    .filter(|(name, arg)| !name.trim().is_empty() && !arg.trim().is_empty())
                else {
                    return Err(format!("--mutants-arg expects <name>=<arg>, got `{value}`"));
                };
                if harness_owned_mutants_arg(arg) {
                    return Err(format!(
                        "--mutants-arg `{arg}` would override how the harness runs cargo-mutants; use the spot-check options (--jobs, --mutant-timeout-secs) or select mutants with --file, --re, --exclude, --package or --workspace"
                    ));
                }
                options
                    .mutants_args
                    .entry(name.trim().to_string())
                    .or_default()
                    .push(arg.to_string());
            }
            "--run-mutants" => options.run_mutants = true,
            "--jobs" => {
                index += 1;
                options.jobs = parse_positive(required_arg(args, index, flag)?, flag)? as usize;
            }
            "--mutant-timeout-secs" => {
                index += 1;
                options.mutant_timeout_secs =
                    parse_positive(required_arg(args, index, flag)?, flag)?;
            }
            "--examples" => {
                index += 1;
                options.examples = parse_positive(required_arg(args, index, flag)?, flag)? as usize;
            }
            "--ripr" => {
                index += 1;
                options.ripr = Some(PathBuf::from(required_arg(args, index, flag)?));
            }
            other => {
                return Err(format!(
                    "unknown mutation-spot-check argument `{other}`; {USAGE}"
                ));
            }
        }
        index += 1;
    }
    if options.repos.is_empty() {
        return Err(format!(
            "mutation-spot-check needs at least one --repo; {USAGE}"
        ));
    }
    for name in options.mutants_out.keys() {
        if !options.repos.iter().any(|(repo, _)| repo == name) {
            return Err(format!("--mutants-out `{name}` names no --repo"));
        }
    }
    for name in options.mutants_args.keys() {
        if !options.repos.iter().any(|(repo, _)| repo == name) {
            return Err(format!("--mutants-arg `{name}` names no --repo"));
        }
        if options.mutants_out.contains_key(name) || !options.run_mutants {
            return Err(format!(
                "--mutants-arg `{name}` only applies when --run-mutants produces that repo's outcomes"
            ));
        }
    }
    Ok(options)
}

fn parse_named_path(value: &str, flag: &str) -> Result<(String, PathBuf), String> {
    match value.split_once('=') {
        Some((name, path)) if is_repo_name(name.trim()) && !path.trim().is_empty() => {
            Ok((name.trim().to_string(), PathBuf::from(path.trim())))
        }
        _ => Err(format!(
            "{flag} expects <name>=<path> with a name of letters, digits, `.`, `_` or `-` (it names files under {SCRATCH}), got `{value}`"
        )),
    }
}

/// Repo names become scratch file names, so they may not carry separators.
fn is_repo_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
}

fn parse_positive(value: &str, flag: &str) -> Result<u64, String> {
    match value.parse::<u64>() {
        Ok(parsed) if parsed > 0 => Ok(parsed),
        _ => Err(format!("{flag} must be a positive integer, got `{value}`")),
    }
}

fn required_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    match args.get(index) {
        Some(value) if !value.trim().is_empty() => Ok(value),
        _ => Err(format!("missing value for {flag}; {USAGE}")),
    }
}

struct RepoRun {
    name: String,
    revision: String,
    diffs_checked: usize,
    exposure_run_status: Option<String>,
    cargo_mutants_args: Vec<String>,
    cargo_mutants_version: Option<String>,
    metrics: Value,
    pairs: Vec<Pair>,
    /// Judged pilot recommendations, or why pilot produced none. A pilot
    /// failure does not discard the repo's validated mutation outcomes.
    pilot: Result<Vec<Value>, String>,
}

fn spot_check_repo(
    binary: &Path,
    scratch: &Path,
    name: &str,
    checkout: &Path,
    options: &Options,
) -> Result<RepoRun, String> {
    let revision = run_text(
        "git",
        &[
            "-C".to_string(),
            path_arg(checkout),
            "rev-parse".to_string(),
            "HEAD".to_string(),
        ],
        Duration::from_secs(30),
        "git rev-parse for spot-check checkout",
    )?
    .trim()
    .to_string();

    let exposure_path = scratch.join(format!("{name}.repo-exposure.json"));
    let exposure = capture_stdout_to_file_with_timeout(
        &path_arg(binary),
        &[
            "check".to_string(),
            "--root".to_string(),
            path_arg(checkout),
            "--format".to_string(),
            "repo-exposure-json".to_string(),
        ],
        &[],
        &exposure_path,
        EXPOSURE_TIMEOUT,
        "ripr repo-exposure-json for spot check",
    )?;
    if exposure.timed_out || !exposure.status.is_some_and(|status| status.success()) {
        return Err(format!(
            "ripr repo-exposure-json failed for `{name}`: {}",
            exposure.stderr.trim()
        ));
    }
    let exposure_json = read_json(&exposure_path)?;

    let mutants_dir = match options.mutants_out.get(name) {
        Some(dir) => dir.clone(),
        None if options.run_mutants => run_cargo_mutants(scratch, name, checkout, options)?,
        None => {
            return Err(format!(
                "no cargo-mutants outcomes for `{name}`: pass --mutants-out {name}=<dir> or --run-mutants"
            ));
        }
    };
    let read_mutants_out = |file: &str| {
        read_json(&mutants_dir.join(file)).map_err(|err| {
            format!(
                "{err}\nexpected a cargo-mutants mutants.out directory for `{name}` containing outcomes.json and mutants.json"
            )
        })
    };
    let outcomes = read_mutants_out("outcomes.json")?;
    let mutant_records = read_mutants_out("mutants.json")?;
    let diffs_checked = require_mutants_match_checkout(name, checkout, &revision, &mutant_records)?;
    let pilot_top = pilot::pilot_top_seams(binary, scratch, name, checkout);

    let calibration = run_text(
        &path_arg(binary),
        &[
            "calibrate".to_string(),
            "cargo-mutants".to_string(),
            "--mutants-json".to_string(),
            path_arg(&mutants_dir),
            "--repo-exposure-json".to_string(),
            path_arg(&exposure_path),
            "--format".to_string(),
            "json".to_string(),
        ],
        CALIBRATE_TIMEOUT,
        "ripr calibrate cargo-mutants for spot check",
    )?;
    let calibration: Value = serde_json::from_str(&calibration)
        .map_err(|err| format!("parse calibration JSON for `{name}`: {err}"))?;

    Ok(RepoRun {
        name: name.to_string(),
        revision,
        diffs_checked,
        exposure_run_status: exposure_json
            .get("run_status")
            .and_then(Value::as_str)
            .map(str::to_string),
        cargo_mutants_args: if options.mutants_out.contains_key(name) {
            Vec::new()
        } else {
            options.mutants_args.get(name).cloned().unwrap_or_default()
        },
        cargo_mutants_version: outcomes
            .get("cargo_mutants_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        metrics: calibration.get("metrics").cloned().unwrap_or(Value::Null),
        pairs: classify_matches(&calibration, &exposure_json, &mutant_records),
        pilot: pilot_top.map(|top| {
            pilot::judge_recommendations(
                &top,
                &mutant_records,
                &outcomes,
                &seam_expressions(&exposure_json),
                &|file, line| {
                    let index = usize::try_from(line).ok()?.checked_sub(1)?;
                    std::fs::read_to_string(checkout.join(file))
                        .ok()?
                        .lines()
                        .nth(index)
                        .map(str::to_string)
                },
            )
        }),
    })
}

/// cargo-mutants records no source revision, so a supplied `mutants.out` from
/// another commit would join stale outcomes to this checkout's seams. Every
/// mutant diff carries the original lines it replaced; they must still match
/// the checkout, or nothing from this directory is scored.
fn require_mutants_match_checkout(
    name: &str,
    checkout: &Path,
    revision: &str,
    mutant_records: &Value,
) -> Result<usize, String> {
    let records = match mutant_records.as_array() {
        Some(records) if !records.is_empty() => records,
        _ => {
            return Err(format!(
                "mutants.json for `{name}` is not a non-empty array of mutants, so nothing can be checked against the checkout at {revision}. Pass the mutants.out directory of a cargo-mutants 27 or later run that found mutants."
            ));
        }
    };
    check_mutant_diffs(records, |file| {
        fs::read_to_string(checkout.join(file))
            .ok()
            .map(|text| source_lines(&text))
    })
    .map_err(|problem| {
        format!("mutants.out for `{name}` does not match the checkout at {revision}: {problem}")
    })
}

/// Checks every mutant record's diff against the source `read_source`
/// returns, and returns how many were checked. A record without a diff
/// (cargo-mutants before 27 omits it) cannot be checked, so it refuses the
/// directory rather than scoring unverified outcomes.
fn check_mutant_diffs(
    records: &[Value],
    mut read_source: impl FnMut(&str) -> Option<Vec<String>>,
) -> Result<usize, String> {
    let mut sources: BTreeMap<String, Option<Vec<String>>> = BTreeMap::new();
    let mut stale = Vec::new();
    let mut missing_files = BTreeSet::new();
    for record in records {
        let (Some(file), Some(diff)) = (
            record.get("file").and_then(Value::as_str),
            record.get("diff").and_then(Value::as_str),
        ) else {
            return Err(
                "mutants.json has a mutant without `file` or `diff`, so its source revision cannot be checked. Re-run with cargo-mutants 27 or later, which records each mutant's diff.".to_string(),
            );
        };
        let lines = sources
            .entry(file.to_string())
            .or_insert_with(|| read_source(file));
        match lines {
            None => {
                missing_files.insert(file.to_string());
            }
            Some(lines) => {
                if let Some(line) = first_stale_line(diff, lines) {
                    stale.push(format!("{file}:{line}"));
                }
            }
        }
    }
    if let Some(file) = missing_files.first() {
        return Err(format!(
            "{} mutated file(s) are missing from the checkout, first `{file}`. Pass the checkout root cargo-mutants ran in.",
            missing_files.len()
        ));
    }
    if let Some(first) = stale.first() {
        return Err(format!(
            "{} of {} mutant diffs no longer apply (first at {first}). Check out the revision cargo-mutants ran on, or re-run cargo mutants on this one.",
            stale.len(),
            records.len()
        ));
    }
    Ok(records.len())
}

/// Splits source the way cargo-mutants' diff library counts lines: `\n`,
/// `\r\n`, and a bare `\r` each end a line, so hunk line numbers agree.
fn source_lines(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::to_string)
        .collect()
}

/// Returns the first source line where a mutant diff's context or removed
/// lines differ from `source`, or `None` when the whole diff still applies.
fn first_stale_line(diff: &str, source: &[String]) -> Option<usize> {
    let mut cursor: Option<usize> = None;
    for line in diff.lines() {
        if let Some(header) = line.strip_prefix("@@ -") {
            let start = header
                .split([',', ' '])
                .next()
                .and_then(|value| value.parse::<usize>().ok());
            match start {
                Some(start) => cursor = Some(start),
                None => return Some(0),
            }
            continue;
        }
        let Some(current) = cursor else {
            continue;
        };
        let original = match line.chars().next() {
            Some(' ') | Some('-') => line.get(1..).unwrap_or_default(),
            None => "",
            Some(_) => continue,
        };
        if source.get(current.wrapping_sub(1)).map(String::as_str) != Some(original) {
            return Some(current);
        }
        cursor = Some(current + 1);
    }
    None
}

/// A checkout under this workspace inherits its Cargo workspace, so neither
/// the baseline build nor ripr's root resolution would see the crate alone.
fn reject_checkout_inside_workspace(name: &str, checkout: &Path) -> Result<(), String> {
    let checkout = fs::canonicalize(checkout).map_err(|err| {
        format!(
            "--repo {name}: cannot resolve {}: {err}",
            checkout.display()
        )
    })?;
    let workspace = std::env::current_dir()
        .and_then(fs::canonicalize)
        .map_err(|err| format!("resolve current directory: {err}"))?;
    if checkout.starts_with(&workspace) {
        return Err(format!(
            "--repo {name}: {} is inside this workspace; clone spot-check repos outside it",
            checkout.display()
        ));
    }
    Ok(())
}

fn run_cargo_mutants(
    scratch: &Path,
    name: &str,
    checkout: &Path,
    options: &Options,
) -> Result<PathBuf, String> {
    let output_root = scratch.join(format!("{name}-mutants"));
    let _ = fs::remove_dir_all(&output_root);
    fs::create_dir_all(&output_root)
        .map_err(|err| format!("create {}: {err}", output_root.display()))?;
    // The child runs from the checkout, so hand it absolute paths.
    let output_root = fs::canonicalize(&output_root)
        .map_err(|err| format!("resolve {}: {err}", output_root.display()))?;
    let checkout = &fs::canonicalize(checkout)
        .map_err(|err| format!("resolve checkout {}: {err}", checkout.display()))?;
    // The repository's `.cargo/config.toml` forces TMPDIR into this
    // workspace's `target/`. cargo-mutants copies the crate under TMPDIR, and
    // a copy inside this workspace fails its baseline build with "believes
    // it's in a workspace when it's not". Put the copies beside the checkout.
    let temp_root = checkout
        .parent()
        .unwrap_or(checkout)
        .join(format!(".ripr-spot-check-tmp-{name}"));
    fs::create_dir_all(&temp_root)
        .map_err(|err| format!("create {}: {err}", temp_root.display()))?;
    let temp_dir = path_arg(&temp_root);
    // Run from the checkout with the toolchain selectors that `cargo xtask`
    // exports removed, so rustup picks the subject's own toolchain (its
    // rust-toolchain file, else the default) instead of this repository's pin.
    let output = capture_bytes_in_dir_with_timeout(
        Path::new("cargo"),
        &[
            "mutants".to_string(),
            "--dir".to_string(),
            path_arg(checkout),
            "--output".to_string(),
            path_arg(&output_root),
            "--jobs".to_string(),
            options.jobs.to_string(),
            "--timeout".to_string(),
            options.mutant_timeout_secs.to_string(),
            "--no-shuffle".to_string(),
        ]
        .into_iter()
        .chain(
            options
                .mutants_args
                .get(name)
                .into_iter()
                .flatten()
                .cloned(),
        )
        .collect::<Vec<_>>(),
        checkout,
        &[
            ("TMPDIR", &temp_dir),
            ("TMP", &temp_dir),
            ("TEMP", &temp_dir),
        ],
        &["RUSTUP_TOOLCHAIN", "CARGO"],
        MUTANTS_TIMEOUT,
        "cargo mutants for spot check",
    );
    let _ = fs::remove_dir_all(&temp_root);
    let output = output?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    // cargo-mutants exits 2 when mutants were missed and 3 when some timed
    // out; both are results. Any other non-zero exit (1 usage, 4 baseline
    // failure) means there is no outcome set to score.
    let code = output.status.and_then(|status| status.code());
    if output.timed_out || !matches!(code, Some(0 | 2 | 3)) {
        return Err(format!(
            "cargo mutants failed for `{name}` (exit {code:?}); check that cargo-mutants is installed and that `cargo test` passes in {}; see {}/mutants.out/log/baseline.log\n{}",
            checkout.display(),
            output_root.display(),
            stderr.trim()
        ));
    }
    let mutants_out = output_root.join("mutants.out");
    if !mutants_out.join("outcomes.json").exists() {
        return Err(format!(
            "cargo mutants found no mutants for `{name}`, so there is nothing to score; for a multi-crate workspace, run `cargo mutants --workspace` yourself and pass its output with --mutants-out {name}=<dir>\n{}",
            stderr.trim()
        ));
    }
    Ok(mutants_out)
}

fn run_text(
    program: &str,
    args: &[String],
    timeout: Duration,
    context: &str,
) -> Result<String, String> {
    let output = capture_output_with_timeout(program, args, &[], timeout, context)?;
    if output.timed_out || !output.status.is_some_and(|status| status.success()) {
        return Err(format!("{context} failed: {}", output.stderr.trim()));
    }
    Ok(output.stdout)
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

fn path_arg(path: &Path) -> String {
    path.display().to_string()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Pair {
    pairing: &'static str,
    grip_class: String,
    outcome: String,
    seam_id: String,
    seam_kind: String,
    file: String,
    line: u64,
    expression: String,
    mutant: String,
}

/// Seam id to source expression, from the repo exposure JSON.
fn seam_expressions(exposure: &Value) -> BTreeMap<&str, &str> {
    exposure
        .get("seams")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|seam| {
            Some((
                seam.get("seam_id")?.as_str()?,
                seam.get("expression").and_then(Value::as_str).unwrap_or(""),
            ))
        })
        .collect()
}

/// Classify every unambiguous calibration match. Ambiguous and unmatched
/// runtime records stay in the calibration metrics and are never scored.
fn classify_matches(calibration: &Value, exposure: &Value, mutants: &Value) -> Vec<Pair> {
    let genres: BTreeMap<&str, &str> = mutants
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|mutant| {
            Some((
                mutant.get("name")?.as_str()?,
                mutant.get("genre")?.as_str()?,
            ))
        })
        .collect();
    let expressions = seam_expressions(exposure);
    let text = |value: &Value, pointer: &str| {
        value
            .pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    calibration
        .get("matches")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|record| {
            let mutant = text(record, "/runtime/mutant_id");
            let seam_id = text(record, "/static/seam_id");
            let seam_kind = text(record, "/static/seam_kind");
            let expression = expressions
                .get(seam_id.as_str())
                .copied()
                .unwrap_or("")
                .to_string();
            let genre = genres.get(mutant.as_str()).copied().unwrap_or("");
            Pair {
                pairing: pairing_for(genre, &mutant, &seam_kind, &expression),
                grip_class: text(record, "/static/seam_grip_class"),
                outcome: text(record, "/runtime/runtime_outcome"),
                seam_id,
                seam_kind,
                file: text(record, "/static/file"),
                line: record
                    .pointer("/static/line")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                expression,
                mutant,
            }
        })
        .collect()
}

fn pairing_for(genre: &str, mutant: &str, seam_kind: &str, expression: &str) -> &'static str {
    if genre == "FnValue" {
        return "function_body";
    }
    let operator_genre = matches!(genre, "BinaryOperator" | "UnaryOperator");
    let behavior_seam = matches!(seam_kind, "predicate_boundary" | "return_value");
    match original_operator(mutant) {
        Some(operator)
            if operator_genre && behavior_seam && contains_operator_token(expression, operator) =>
        {
            "seam_precise"
        }
        _ => "same_line_other",
    }
}

/// True when `operator` occurs in `expression` with no adjacent operator
/// character, so `>` does not match inside `>=`, `->`, `=>`, or `>>`, and `!`
/// does not match inside `!=`. A generic bracket such as `Vec<u8>` can still
/// match `<`; the line-level join already narrows candidates to one seam.
fn contains_operator_token(expression: &str, operator: &str) -> bool {
    const OPERATOR_CHARS: &str = "!%&*+-/<=>^|";
    expression.match_indices(operator).any(|(start, _)| {
        let before = expression[..start].chars().next_back();
        let after = expression[start + operator.len()..].chars().next();
        !before.is_some_and(|ch| OPERATOR_CHARS.contains(ch))
            && !after.is_some_and(|ch| OPERATOR_CHARS.contains(ch))
    })
}

/// The operator a cargo-mutants operator mutant replaces or deletes, from its
/// name: `src/a.rs:3:9: replace > with >= in f` → `>`,
/// `src/a.rs:3:9: delete ! in f` → `!`.
fn original_operator(mutant: &str) -> Option<&str> {
    let (_, description) = mutant.split_once(": ")?;
    let rest = description
        .strip_prefix("replace ")
        .or_else(|| description.strip_prefix("delete "))?;
    let operator = rest.split_whitespace().next()?;
    (!operator.is_empty() && !operator.chars().any(char::is_alphanumeric)).then_some(operator)
}

fn verdict_family(grip_class: &str) -> &'static str {
    match grip_class {
        "strongly_gripped" => "claims_discriminator",
        "ungripped" | "reachable_unrevealed" => "claims_no_discriminator",
        "weakly_gripped" => "weak",
        "intentional" | "suppressed" => "excluded",
        _ => "unknown",
    }
}

/// `agree`, `overclaim` (claimed a discriminator, mutant missed), `false_gap`
/// (claimed none, mutant caught), or `unscored`.
fn agreement(family: &str, outcome: &str) -> &'static str {
    match (family, outcome) {
        ("claims_discriminator", "caught") | ("claims_no_discriminator", "missed") => "agree",
        ("claims_discriminator", "missed") => "overclaim",
        ("claims_no_discriminator", "caught") => "false_gap",
        _ => "unscored",
    }
}

fn build_report(repos: &[RepoRun], examples: usize) -> Value {
    let mut by_pairing: BTreeMap<&str, BTreeMap<String, BTreeMap<String, usize>>> = BTreeMap::new();
    let mut scored: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    let mut seams_scored: BTreeMap<&str, BTreeSet<(String, String)>> = BTreeMap::new();
    let mut disagreements: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
    let mut example_seams = BTreeSet::new();
    for repo in repos {
        for pair in &repo.pairs {
            *by_pairing
                .entry(pair.pairing)
                .or_default()
                .entry(pair.grip_class.clone())
                .or_default()
                .entry(pair.outcome.clone())
                .or_default() += 1;
            if pair.pairing != "seam_precise" {
                continue;
            }
            let family = verdict_family(&pair.grip_class);
            let verdict = agreement(family, &pair.outcome);
            if verdict == "unscored" {
                continue;
            }
            *scored
                .entry(family)
                .or_default()
                .entry(verdict)
                .or_default() += 1;
            seams_scored
                .entry(family)
                .or_default()
                .insert((repo.name.clone(), pair.seam_id.clone()));
            let list = disagreements.entry(verdict).or_default();
            if verdict != "agree"
                && list.len() < examples
                && example_seams.insert((repo.name.clone(), pair.seam_id.clone()))
            {
                list.push(json!({
                    "repo": repo.name,
                    "revision": repo.revision,
                    "file": pair.file,
                    "line": pair.line,
                    "seam_kind": pair.seam_kind,
                    "seam_id": pair.seam_id,
                    "expression": pair.expression,
                    "grip_class": pair.grip_class,
                    "mutant": pair.mutant,
                    "runtime_outcome": pair.outcome,
                }));
            }
        }
    }
    disagreements.remove("agree");
    let families = scored
        .iter()
        .map(|(family, counts)| {
            let agree = counts.get("agree").copied().unwrap_or(0);
            let total: usize = counts.values().sum();
            (
                family.to_string(),
                json!({
                    "mutants_scored": total,
                    "seams_scored": seams_scored.get(family).map_or(0, BTreeSet::len),
                    "counts": counts,
                    "agreement_rate": rate(agree, total),
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    json!({
        "schema_version": SCHEMA_VERSION,
        "status": "advisory",
        "claim_boundary": "Agreement is scored only on seam_precise joins (operator mutants whose original operator appears in a predicate_boundary or return_value seam expression on the same line). Claims are limited to the recorded checkout revisions, cargo-mutants versions, and this join rule; this is not a suite adequacy measure.",
        "repos": repos.iter().map(|repo| json!({
            "name": repo.name,
            "revision": repo.revision,
            "mutant_diffs_checked_against_revision": repo.diffs_checked,
            "exposure_run_status": repo.exposure_run_status,
            "cargo_mutants_args": repo.cargo_mutants_args,
            "cargo_mutants_version": repo.cargo_mutants_version,
            "calibration_metrics": repo.metrics,
            "pairings": pairing_counts(&repo.pairs),
        })).collect::<Vec<_>>(),
        "outcomes_by_pairing_and_grip_class": by_pairing,
        "scored_families": families,
        "disagreement_examples": disagreements,
        "pilot_top_recommendations": pilot::summarize(
            &repos
                .iter()
                .map(|repo| (repo.name.clone(), repo.pilot.clone()))
                .collect::<Vec<_>>(),
        ),
    })
}

fn pairing_counts(pairs: &[Pair]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for pair in pairs {
        *counts.entry(pair.pairing).or_default() += 1;
    }
    counts
}

fn rate(numerator: usize, denominator: usize) -> Value {
    if denominator == 0 {
        Value::Null
    } else {
        json!(((numerator as f64 / denominator as f64) * 1000.0).round() / 1000.0)
    }
}

fn spot_check_markdown(report: &Value) -> String {
    let mut out = String::from("# Mutation spot check\n\n");
    if let Some(boundary) = report.get("claim_boundary").and_then(Value::as_str) {
        out.push_str(&format!("{boundary}\n\n"));
    }
    out.push_str("## Repositories\n\n| Repo | Revision | cargo-mutants | Mutants | Unambiguous joins | Ambiguous | Unmatched | Seam-precise |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for repo in report
        .get("repos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let metric = |key: &str| {
            repo.pointer(&format!("/calibration_metrics/{key}"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        };
        out.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} | {} | {} |\n",
            repo.get("name").and_then(Value::as_str).unwrap_or(""),
            repo.get("revision")
                .and_then(Value::as_str)
                .unwrap_or("")
                .get(..12)
                .unwrap_or(""),
            repo.get("cargo_mutants_version")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            metric("mutants_total"),
            metric("matched_total"),
            metric("ambiguous_file_line_total"),
            metric("unmatched_mutants_total"),
            repo.pointer("/pairings/seam_precise")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        ));
    }
    for repo in report
        .get("repos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let sampled = repo
            .get("cargo_mutants_args")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|arg| format!("`{arg}`"))
            .collect::<Vec<_>>();
        if !sampled.is_empty() {
            out.push_str(&format!(
                "\n{} ran cargo-mutants with {}.\n",
                repo.get("name").and_then(Value::as_str).unwrap_or(""),
                sampled.join(" ")
            ));
        }
    }
    out.push_str("\n## Scored verdicts (seam-precise joins)\n\n| Verdict family | Seams | Mutants | Agree | Overclaim | False gap | Agreement |\n| --- | --- | --- | --- | --- | --- | --- |\n");
    if let Some(families) = report.get("scored_families").and_then(Value::as_object) {
        for (family, row) in families {
            let count = |key: &str| {
                row.pointer(&format!("/counts/{key}"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
            };
            out.push_str(&format!(
                "| {family} | {} | {} | {} | {} | {} | {} |\n",
                row.get("seams_scored").and_then(Value::as_u64).unwrap_or(0),
                row.get("mutants_scored")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                count("agree"),
                count("overclaim"),
                count("false_gap"),
                row.get("agreement_rate")
                    .and_then(Value::as_f64)
                    .map_or_else(|| "n/a".to_string(), |rate| format!("{:.1}%", rate * 100.0)),
            ));
        }
    }
    out.push_str("\n## Outcomes by pairing and grip class\n\n| Pairing | Grip class | Outcomes |\n| --- | --- | --- |\n");
    if let Some(pairings) = report
        .get("outcomes_by_pairing_and_grip_class")
        .and_then(Value::as_object)
    {
        for (pairing, classes) in pairings {
            for (class, outcomes) in classes.as_object().into_iter().flatten() {
                let cells = outcomes
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(outcome, count)| format!("{outcome} {count}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("| {pairing} | {class} | {cells} |\n"));
            }
        }
    }
    out.push_str("\n## Disagreement examples\n\n");
    let mut any = false;
    for (verdict, rows) in report
        .get("disagreement_examples")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        for row in rows.as_array().into_iter().flatten() {
            any = true;
            let field = |key: &str| row.get(key).and_then(Value::as_str).unwrap_or("");
            out.push_str(&format!(
                "- **{verdict}** {} `{}:{}` {} `{}` is `{}`; mutant `{}` was {}.\n",
                field("repo"),
                field("file"),
                row.get("line").and_then(Value::as_u64).unwrap_or(0),
                field("seam_kind"),
                field("expression"),
                field("grip_class"),
                field("mutant")
                    .split_once(": ")
                    .map_or(field("mutant"), |(_, rest)| rest),
                field("runtime_outcome"),
            ));
        }
    }
    if !any {
        out.push_str("None in the scored joins.\n");
    }
    out.push_str(&pilot::markdown(report));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutant_diffs_must_match_the_checkout_lines_they_replaced() -> Result<(), String> {
        let diff = "--- src/a.rs\n+++ replace > with < in f\n@@ -2,3 +2,3 @@\n fn f(a: u8) -> bool {\n-    a > 1\n+    a < 1\n }\n";
        let source = |body: &str| -> Vec<String> {
            ["// header", "fn f(a: u8) -> bool {", body, "}"]
                .iter()
                .map(|line| line.to_string())
                .collect()
        };
        assert_eq!(first_stale_line(diff, &source("    a > 1")), None);
        assert_eq!(first_stale_line(diff, &source("    a >= 1")), Some(3));
        assert_eq!(first_stale_line(diff, &source("    a > 1")[..2]), Some(3));

        assert_eq!(
            source_lines("a\r\nb\rc\nd"),
            ["a", "b", "c", "d"].map(String::from)
        );

        let records = [json!({"file": "src/a.rs", "diff": diff})];
        assert_eq!(
            check_mutant_diffs(&records, |_| Some(source("    a > 1"))),
            Ok(1)
        );
        let Err(stale) = check_mutant_diffs(&records, |_| Some(source("    a >= 1"))) else {
            return Err("a changed source line must refuse the directory".to_string());
        };
        assert!(stale.contains("1 of 1 mutant diffs no longer apply (first at src/a.rs:3)"));
        let Err(missing) = check_mutant_diffs(&records, |_| None) else {
            return Err("a missing source file must refuse the directory".to_string());
        };
        assert!(missing.contains("missing from the checkout, first `src/a.rs`"));
        // cargo-mutants 26.x writes mutants.json without diffs.
        let Err(no_diff) = check_mutant_diffs(&[json!({"file": "src/a.rs"})], |_| {
            Some(source("    a > 1"))
        }) else {
            return Err("a mutant without a diff must refuse the directory".to_string());
        };
        assert!(no_diff.contains("cargo-mutants 27 or later"));
        Ok(())
    }

    #[test]
    fn repo_names_cannot_escape_the_scratch_directory() -> Result<(), String> {
        let (name, _) = parse_named_path("semver=../corpus/semver", "--repo")?;
        assert_eq!(name, "semver");
        for bad in ["../x=/tmp/x", "a/b=/tmp/x", "..=/tmp/x", "=/tmp/x"] {
            let Err(err) = parse_named_path(bad, "--repo") else {
                return Err(format!("`{bad}` should be refused"));
            };
            assert!(err.contains("letters, digits"), "{err}");
        }
        Ok(())
    }

    #[test]
    fn mutants_args_pass_through_only_to_a_run_this_harness_starts() -> Result<(), String> {
        let args = |extra: &[&str]| -> Vec<String> {
            ["--repo", "hex=/tmp/hex"]
                .iter()
                .chain(extra)
                .map(|arg| arg.to_string())
                .collect()
        };
        let options = parse_options(&args(&[
            "--run-mutants",
            "--mutants-arg",
            "hex=--file=src/lib.rs",
            "--mutants-arg",
            "hex=--re=decode",
            "--mutants-arg",
            "hex=--jobserver=false",
            "--mutants-arg",
            "hex=-Dx.diff",
            "--mutants-arg",
            "hex=-Fdecode",
            "--mutants-arg",
            "hex=-vfsrc/output.rs",
            "--mutants-arg",
            "hex=-V",
        ]))?;
        assert_eq!(
            options.mutants_args.get("hex"),
            Some(&vec![
                "--file=src/lib.rs".to_string(),
                "--re=decode".to_string(),
                "--jobserver=false".to_string(),
                "-Dx.diff".to_string(),
                "-Fdecode".to_string(),
                "-vfsrc/output.rs".to_string(),
                "-V".to_string()
            ])
        );
        for (extra, expected) in [
            (
                vec!["--mutants-arg", "hex=--re=x"],
                "only applies when --run-mutants",
            ),
            (
                vec!["--run-mutants", "--mutants-arg", "other=--re=x"],
                "names no --repo",
            ),
            (
                vec!["--run-mutants", "--mutants-arg", "hex="],
                "expects <name>=<arg>",
            ),
            (
                vec!["--run-mutants", "--mutants-arg", "hex= "],
                "expects <name>=<arg>",
            ),
            (
                vec!["--run-mutants", "--mutants-arg", "hex=--in-place"],
                "would override how the harness runs",
            ),
            (
                vec!["--run-mutants", "--mutants-arg", "hex=--output=/tmp/x"],
                "would override how the harness runs",
            ),
            (
                vec!["--run-mutants", "--mutants-arg", "hex=-j8"],
                "would override how the harness runs",
            ),
            (
                vec!["--run-mutants", "--mutants-arg", "hex=-t5"],
                "would override how the harness runs",
            ),
            (
                vec![
                    "--run-mutants",
                    "--mutants-arg",
                    "hex=--timeout-multiplier=2",
                ],
                "would override how the harness runs",
            ),
            (
                vec![
                    "--run-mutants",
                    "--mutants-arg",
                    "hex=--minimum-test-timeout=5",
                ],
                "would override how the harness runs",
            ),
            (
                vec!["--run-mutants", "--mutants-arg", "hex=-dfoo"],
                "would override how the harness runs",
            ),
            (
                vec!["--run-mutants", "--mutants-arg", "hex=-o=/tmp/x"],
                "would override how the harness runs",
            ),
            (
                vec!["--run-mutants", "--mutants-arg", "hex=-vj8"],
                "would override how the harness runs",
            ),
            (
                vec!["--run-mutants", "--mutants-arg", "hex=-Vt5"],
                "would override how the harness runs",
            ),
            (
                vec!["--run-mutants", "--mutants-arg", "hex=--list-files"],
                "would override how the harness runs",
            ),
        ] {
            let Err(err) = parse_options(&args(&extra)) else {
                return Err(format!("{extra:?} should be refused"));
            };
            assert!(err.contains(expected), "{err}");
        }
        Ok(())
    }

    #[test]
    fn original_operator_reads_replace_and_delete_names() {
        assert_eq!(
            original_operator("src/a.rs:3:9: replace > with >= in f"),
            Some(">")
        );
        assert_eq!(original_operator("src/a.rs:3:9: delete ! in f"), Some("!"));
        assert_eq!(
            original_operator("src/a.rs:3:9: replace f -> bool with true"),
            None
        );
        assert_eq!(
            original_operator("src/a.rs:3:9: delete match arm Some(x) in f"),
            None
        );
    }

    #[test]
    fn pairing_scores_only_operator_mutants_inside_behavior_seams() {
        let boundary = "src/a.rs:3:9: replace > with >= in f";
        assert_eq!(
            pairing_for("BinaryOperator", boundary, "predicate_boundary", "i > 0"),
            "seam_precise"
        );
        // Same line, but the seam's claim is about a call, not the operator.
        assert_eq!(
            pairing_for("BinaryOperator", boundary, "call_presence", "f(i > 0)"),
            "same_line_other"
        );
        // `>` appears only inside the seam's `>=`, so it is not the seam's operator.
        assert_eq!(
            pairing_for("BinaryOperator", boundary, "predicate_boundary", "x >= y"),
            "same_line_other"
        );
        assert_eq!(
            pairing_for(
                "UnaryOperator",
                "src/a.rs:3:9: delete ! in f",
                "predicate_boundary",
                "a != b"
            ),
            "same_line_other"
        );
        assert_eq!(
            pairing_for(
                "BinaryOperator",
                "src/a.rs:3:9: replace >= with < in f",
                "predicate_boundary",
                "x >= y"
            ),
            "seam_precise"
        );
        // Same line and kind, but the operator is outside the seam expression.
        assert_eq!(
            pairing_for("BinaryOperator", boundary, "predicate_boundary", "x == y"),
            "same_line_other"
        );
        assert_eq!(
            pairing_for(
                "FnValue",
                "src/a.rs:3:9: replace f -> bool with true",
                "return_value",
                "x > 0"
            ),
            "function_body"
        );
    }

    #[test]
    fn agreement_separates_overclaims_from_false_gaps() {
        assert_eq!(
            agreement(verdict_family("strongly_gripped"), "caught"),
            "agree"
        );
        assert_eq!(
            agreement(verdict_family("strongly_gripped"), "missed"),
            "overclaim"
        );
        assert_eq!(agreement(verdict_family("ungripped"), "missed"), "agree");
        assert_eq!(
            agreement(verdict_family("ungripped"), "caught"),
            "false_gap"
        );
        assert_eq!(
            agreement(verdict_family("weakly_gripped"), "missed"),
            "unscored"
        );
        assert_eq!(
            agreement(verdict_family("activation_unknown"), "caught"),
            "unscored"
        );
        assert_eq!(
            agreement(verdict_family("strongly_gripped"), "timeout"),
            "unscored"
        );
    }

    #[test]
    fn report_scores_precise_joins_and_keeps_examples_per_seam() {
        let calibration = json!({"matches": [
            {"static": {"seam_id": "s1", "seam_kind": "predicate_boundary", "seam_grip_class": "ungripped", "file": "src/a.rs", "line": 3},
             "runtime": {"mutant_id": "src/a.rs:3:9: replace > with < in f", "runtime_outcome": "caught"}},
            {"static": {"seam_id": "s1", "seam_kind": "predicate_boundary", "seam_grip_class": "ungripped", "file": "src/a.rs", "line": 3},
             "runtime": {"mutant_id": "src/a.rs:3:9: replace > with >= in f", "runtime_outcome": "missed"}},
            {"static": {"seam_id": "s2", "seam_kind": "return_value", "seam_grip_class": "strongly_gripped", "file": "src/a.rs", "line": 7},
             "runtime": {"mutant_id": "src/a.rs:7:5: replace + with - in g", "runtime_outcome": "caught"}},
            {"static": {"seam_id": "s3", "seam_kind": "call_presence", "seam_grip_class": "strongly_gripped", "file": "src/a.rs", "line": 9},
             "runtime": {"mutant_id": "src/a.rs:9:5: replace + with - in h", "runtime_outcome": "missed"}}
        ]});
        let exposure = json!({"seams": [
            {"seam_id": "s1", "expression": "i > 0"},
            {"seam_id": "s2", "expression": "a + b"},
            {"seam_id": "s3", "expression": "h(a + b)"}
        ]});
        let mutants = json!([
            {"name": "src/a.rs:3:9: replace > with < in f", "genre": "BinaryOperator"},
            {"name": "src/a.rs:3:9: replace > with >= in f", "genre": "BinaryOperator"},
            {"name": "src/a.rs:7:5: replace + with - in g", "genre": "BinaryOperator"},
            {"name": "src/a.rs:9:5: replace + with - in h", "genre": "BinaryOperator"}
        ]);
        let repo = RepoRun {
            name: "demo".to_string(),
            revision: "abc".to_string(),
            diffs_checked: 4,
            exposure_run_status: None,
            cargo_mutants_args: Vec::new(),
            cargo_mutants_version: Some("27.1.0".to_string()),
            metrics: Value::Null,
            pairs: classify_matches(&calibration, &exposure, &mutants),
            pilot: Ok(vec![
                json!({"verdict": "refuted", "tier": "line", "grip_class": "weakly_gripped"}),
            ]),
        };
        let report = build_report(&[repo], 5);

        let gap = &report["scored_families"]["claims_no_discriminator"];
        assert_eq!(gap["mutants_scored"], 2);
        assert_eq!(gap["seams_scored"], 1);
        assert_eq!(gap["counts"]["false_gap"], 1);
        assert_eq!(gap["agreement_rate"], 0.5);
        // The call-presence miss is a same-line join, so it cannot count as
        // an overclaim against the strongly gripped verdict.
        let clean = &report["scored_families"]["claims_discriminator"];
        assert_eq!(clean["mutants_scored"], 1);
        assert_eq!(clean["agreement_rate"], 1.0);
        assert_eq!(
            report["outcomes_by_pairing_and_grip_class"]["same_line_other"]["strongly_gripped"]["missed"],
            1
        );
        assert_eq!(
            report["disagreement_examples"]["false_gap"][0]["seam_id"],
            "s1"
        );
        assert!(report["disagreement_examples"].get("overclaim").is_none());
        let markdown = spot_check_markdown(&report);
        assert!(markdown.contains("**false_gap** demo `src/a.rs:3`"));
        assert!(!markdown.contains("ran cargo-mutants with"));

        let mut report = report;
        report["repos"][0]["cargo_mutants_args"] = json!(["--re=decode", "--workspace"]);
        assert!(
            spot_check_markdown(&report)
                .contains("demo ran cargo-mutants with `--re=decode` `--workspace`.")
        );
        assert_eq!(report["pilot_top_recommendations"]["counts"]["refuted"], 1);
        assert!(spot_check_markdown(&report).contains("## Pilot top recommendations"));
    }
}
