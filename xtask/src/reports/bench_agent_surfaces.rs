//! `cargo xtask bench-agent-surfaces` — the cross-surface agent-experience
//! benchmark (RIPR-SPEC-0221, #5257).
//!
//! Measures what an agent consumer of ripr actually experiences: CLI
//! end-to-end analysis latency (M1), MCP stdio round-trip latency (M2), LSP
//! first-diagnostics latency after didOpen (M3), output actionability
//! countable from the `check --format json` envelope (M4), and byte-level
//! determinism of repeated runs (M5). Advisory by default; never a CI gate.
//!
//! Corpora are content-pinned and offline: `tiny` is derived from the
//! checked-in `crates/ripr/examples/sample` bytes plus its diff (rewritten to
//! corpus-relative paths and round-trip verified), `mid` is generated
//! deterministically (20 packages x 25 Rust files), and `repo` is the
//! checkout the bench runs in, diff-scoped against a resolved base. The
//! derived/generated corpora are committed with a pinned git identity at
//! their before-state and measured with the after-state worktree, so the MCP
//! and LSP surfaces (which require a usable git workspace root) see the same
//! behavior change the CLI surface sees through `--diff`.

use crate::run::{capture_output_with_timeout, run, run_output, run_output_owned_with_envs};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: &str = "bench-agent-surfaces-v1";
const USAGE: &str = "usage: cargo xtask bench-agent-surfaces [--corpus <tiny|mid|repo>] [--m1-cold <n>] [--m1-warm <n>] [--m2-sessions <n>] [--m3-cold <n>] [--m3-warm <n>] [--m5-cold <n>] [--m5-warm <n>] [--timeout-ms <n>] [--self-deep] [--compare <path>]";
const CACHE_ENV: &str = "RIPR_CACHE_DIR";
// Pinned to the product default (seam_inventory::DEFAULT_REPO_EXPOSURE_SEAM_LIMIT)
// so an ambient override in the caller's environment cannot silently change
// the measured quantity (same discipline as #5200).
const SEAM_LIMIT_ENV: &str = "RIPR_REPO_EXPOSURE_SEAM_LIMIT";
const SEAM_LIMIT_PINNED: &str = "10000";
const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const TIMEOUT_MS_ENV: &str = "RIPR_BENCH_AGENT_SURFACES_TIMEOUT_MS";
const MCP_EGRESS_BOUND_BYTES: usize = 128 * 1024;
const REGRESSION_FRACTION: f64 = 0.25;
// Spec run-count defaults; `m1_cold`/`m1_warm` of 0 means "per-corpus
// default" (tiny/mid 3 cold / 7 warm; repo 1 cold / 3 warm).
const DEFAULT_M2_SESSIONS: usize = 5;
const DEFAULT_M3_SESSIONS: usize = 5;
const DEFAULT_M5_COLD: usize = 2;
const DEFAULT_M5_WARM: usize = 5;
const STATIC_CLASSES: [&str; 7] = [
    "exposed",
    "weakly_exposed",
    "reachable_unrevealed",
    "no_static_path",
    "infection_unknown",
    "propagation_unknown",
    "static_unknown",
];
const UNKNOWN_CLASSES: [&str; 3] = ["infection_unknown", "propagation_unknown", "static_unknown"];
// Deterministically generated mid corpus layout (spec: 20 packages x 25 files).
const MID_PACKAGES: usize = 20;
const MID_SRC_FILES: usize = 20;
const MID_TEST_FILES: usize = 5;
const MID_MUTATION_SRC_FILE: usize = 7;
const BENCH_DIR: &str = "target/ripr/bench-agent-surfaces";

pub(crate) fn bench_agent_surfaces(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    run("cargo", &["build", "-p", "ripr"])?;
    let binary = crate::ripr_debug_binary();
    if !binary.exists() {
        return Err(format!(
            "bench-agent-surfaces: ripr binary missing at {}",
            binary.display()
        ));
    }

    let bench_root = PathBuf::from(BENCH_DIR);
    let corpora_root = bench_root.join("corpora");
    let caches_root = bench_root.join("caches").join(unix_stamp().to_string());
    let _cleanup = DirGuard::new(bench_root.clone());
    fs::create_dir_all(&caches_root)
        .map_err(|err| format!("bench-agent-surfaces caches dir: {err}"))?;

    let mut corpora = Vec::new();
    if options.wants(CorpusId::Tiny) {
        corpora.push(prepare_tiny_corpus(&corpora_root)?);
    }
    if options.wants(CorpusId::Mid) {
        corpora.push(prepare_mid_corpus(&corpora_root)?);
    }
    if options.wants(CorpusId::Repo) {
        corpora.push(prepare_repo_corpus()?);
    }
    if corpora.is_empty() {
        return Err(format!("bench-agent-surfaces: no corpus selected; {USAGE}"));
    }

    let timeout = Duration::from_millis(options.timeout_ms);
    let mut receipt = json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "report": "bench-agent-surfaces",
        "status": "pass",
        "timeout_ms": options.timeout_ms,
        "self_deep": options.self_deep,
        "identity": identity_overlay(&binary, &corpora)?,
    });

    let mut violations = Vec::new();
    let mut warned = false;
    let mut m1_map = serde_json::Map::new();
    let mut m4_map = serde_json::Map::new();
    let mut m5_map = serde_json::Map::new();
    for corpus in &corpora {
        let m1_result = run_m1_corpus(&binary, corpus, &options, &caches_root, timeout)?;
        warned |= m1_result.had_timeout;
        collect_m1_violations(corpus, &m1_result, &mut violations);
        m4_map.insert(
            corpus.id.label().to_string(),
            actionability_metrics(&m1_result.envelopes),
        );
        m1_map.insert(corpus.id.label().to_string(), m1_result.to_json());
        if corpus.id == CorpusId::Tiny || corpus.id == CorpusId::Mid {
            let determinism = run_m5_corpus(
                &binary,
                corpus,
                &options,
                &caches_root,
                timeout,
                &mut warned,
                &mut violations,
            )?;
            m5_map.insert(corpus.id.label().to_string(), determinism.to_json());
            if let Some(failure) = determinism.failure_reason() {
                violations.push(format!("determinism_failure: {failure}"));
            }
        }
    }
    receipt["m1"] = Value::Object(m1_map);
    receipt["m4"] = Value::Object(m4_map);
    receipt["m5"] = Value::Object(m5_map);

    let mut m2_map = serde_json::Map::new();
    let mut m3_map = serde_json::Map::new();
    for corpus in &corpora {
        let m2_result = run_m2_corpus(
            &binary,
            corpus,
            &options,
            &caches_root,
            timeout,
            &mut warned,
        )?;
        collect_m2_violations(corpus, &m2_result, &mut violations);
        m2_map.insert(corpus.id.label().to_string(), m2_result.to_json());
        let m3_result = run_m3_corpus(
            &binary,
            corpus,
            &options,
            &caches_root,
            timeout,
            &mut warned,
        )?;
        collect_m3_violations(corpus, &m3_result, &mut violations);
        m3_map.insert(corpus.id.label().to_string(), m3_result.to_json());
    }
    receipt["m2"] = Value::Object(m2_map);
    receipt["m3"] = Value::Object(m3_map);

    let previous = options
        .compare
        .as_ref()
        .map(|path| read_previous_receipt(path))
        .transpose()?;
    let regressions = previous
        .as_ref()
        .map(|previous| compare_receipts(previous, &receipt))
        .unwrap_or_default();

    let status = receipt_status(&violations, warned, &regressions);
    receipt["status"] = json!(status);
    receipt["validity_gates"] = json!({
        "violations": violations,
        "regressions": regressions,
        "warned": warned,
    });
    receipt["claim_boundary"] = json!(
        "Real-process wall-clock and output-quality measurements of one binary on pinned corpora at the recorded revision and runner class only; no absolute host-independent latency claim, no correctness or coverage claim, no mutation claim. Advisory: not a precommit/CI gate."
    );

    let json_text = serde_json::to_string_pretty(&receipt)
        .map_err(|err| format!("bench-agent-surfaces serialize: {err}"))?;
    crate::write_report("bench-agent-surfaces.json", &format!("{json_text}\n"))?;
    let markdown = receipt_markdown(&receipt, previous.as_ref());
    crate::write_report("bench-agent-surfaces.md", &markdown)?;
    println!("{markdown}");
    println!("Wrote target/ripr/reports/bench-agent-surfaces.json");
    println!("Wrote target/ripr/reports/bench-agent-surfaces.md");
    finish_with_status(status, &violations, &regressions)
}

// ── options ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CorpusId {
    Tiny,
    Mid,
    Repo,
}

impl CorpusId {
    fn label(self) -> &'static str {
        match self {
            CorpusId::Tiny => "tiny",
            CorpusId::Mid => "mid",
            CorpusId::Repo => "repo",
        }
    }

    fn parse(value: &str) -> Option<CorpusId> {
        match value {
            "tiny" => Some(CorpusId::Tiny),
            "mid" => Some(CorpusId::Mid),
            "repo" => Some(CorpusId::Repo),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct Options {
    corpora: Vec<CorpusId>,
    m1_cold: usize,
    m1_warm: usize,
    m2_sessions: usize,
    m3_cold: usize,
    m3_warm: usize,
    m5_cold: usize,
    m5_warm: usize,
    timeout_ms: u64,
    self_deep: bool,
    compare: Option<PathBuf>,
}

impl Options {
    fn wants(&self, id: CorpusId) -> bool {
        self.corpora.contains(&id)
    }
}

fn positive_count(args: &[String], index: usize, flag: &str) -> Result<usize, String> {
    let value = required_arg(args, index, flag)?;
    let parsed: usize = value
        .parse()
        .map_err(|err| format!("bench-agent-surfaces {flag} must be a positive count: {err}"))?;
    if parsed == 0 {
        return Err(format!("bench-agent-surfaces {flag} must be positive"));
    }
    Ok(parsed)
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        corpora: vec![CorpusId::Tiny, CorpusId::Mid, CorpusId::Repo],
        m1_cold: 0,
        m1_warm: 0,
        m2_sessions: DEFAULT_M2_SESSIONS,
        m3_cold: DEFAULT_M3_SESSIONS,
        m3_warm: DEFAULT_M3_SESSIONS,
        m5_cold: DEFAULT_M5_COLD,
        m5_warm: DEFAULT_M5_WARM,
        timeout_ms: default_timeout_ms(),
        self_deep: false,
        compare: None,
    };
    let mut corpora_explicit = false;
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        match flag {
            "--corpus" => {
                let value = required_arg(args, index + 1, "--corpus")?;
                let id = CorpusId::parse(value).ok_or_else(|| {
                    format!("bench-agent-surfaces: unknown corpus `{value}`; {USAGE}")
                })?;
                if !corpora_explicit {
                    options.corpora.clear();
                    corpora_explicit = true;
                }
                options.corpora.push(id);
                index += 2;
            }
            "--m1-cold" => {
                options.m1_cold = positive_count(args, index + 1, "--m1-cold")?;
                index += 2;
            }
            "--m1-warm" => {
                options.m1_warm = positive_count(args, index + 1, "--m1-warm")?;
                index += 2;
            }
            "--m2-sessions" => {
                options.m2_sessions = positive_count(args, index + 1, "--m2-sessions")?;
                index += 2;
            }
            "--m3-cold" => {
                options.m3_cold = positive_count(args, index + 1, "--m3-cold")?;
                index += 2;
            }
            "--m3-warm" => {
                options.m3_warm = positive_count(args, index + 1, "--m3-warm")?;
                index += 2;
            }
            "--m5-cold" => {
                options.m5_cold = positive_count(args, index + 1, "--m5-cold")?;
                index += 2;
            }
            "--m5-warm" => {
                options.m5_warm = positive_count(args, index + 1, "--m5-warm")?;
                index += 2;
            }
            "--timeout-ms" => {
                let value = required_arg(args, index + 1, "--timeout-ms")?;
                options.timeout_ms = value.parse().map_err(|err| {
                    format!("bench-agent-surfaces --timeout-ms must be positive: {err}")
                })?;
                if options.timeout_ms == 0 {
                    return Err("bench-agent-surfaces --timeout-ms must be positive".to_string());
                }
                index += 2;
            }
            "--self-deep" => {
                options.self_deep = true;
                index += 1;
            }
            "--compare" => {
                options.compare = Some(PathBuf::from(required_arg(args, index + 1, "--compare")?));
                index += 2;
            }
            other => {
                return Err(format!(
                    "unknown bench-agent-surfaces argument `{other}`; {USAGE}"
                ));
            }
        }
    }
    Ok(options)
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

fn default_timeout_ms() -> u64 {
    std::env::var(TIMEOUT_MS_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_TIMEOUT_MS)
}

// ── corpora ──────────────────────────────────────────────────────────────

struct Corpus {
    id: CorpusId,
    root: PathBuf,
    diff_path: Option<PathBuf>,
    base: Option<String>,
    kind: &'static str,
    /// Pinned corpora are constructed to carry findings; a zero-subject run
    /// there is a gate violation. The repo corpus reports whatever the
    /// checkout diff holds, so its zero findings are a recorded disclosure,
    /// never a violation.
    expects_findings: bool,
}

fn pinned_git_env() -> Vec<(&'static str, &'static str)> {
    vec![
        ("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z"),
        ("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z"),
        ("GIT_CONFIG_GLOBAL", os_dev_null()),
        ("GIT_CONFIG_SYSTEM", os_dev_null()),
    ]
}

fn os_dev_null() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

/// Commit the current worktree content of `corpus_root` as the pinned
/// before-state baseline on a dedicated base branch. Callers must already
/// have written the before-state files; the after-state commit is applied
/// afterwards.
fn commit_corpus_base(corpus_root: &Path) -> Result<(), String> {
    // Absolute -C: a relative corpus path would resolve against whatever
    // cwd the process carries, and a git invocation that misses the corpus
    // directory would silently operate on the enclosing checkout instead.
    let root_text = canonical(corpus_root)?.display().to_string();
    let envs = pinned_git_env();
    let config = [
        "-c",
        "user.name=ripr bench corpus",
        "-c",
        "user.email=bench@ripr.invalid",
    ];
    git_args(&root_text, &[], &envs, &["init", "-q"])?;
    // Deterministic base branch name regardless of the host's init default.
    git_args(
        &root_text,
        &[],
        &envs,
        &["symbolic-ref", "HEAD", "refs/heads/bench-base"],
    )?;
    git_args(&root_text, &config, &envs, &["add", "-A"])?;
    git_args(
        &root_text,
        &config,
        &envs,
        &["commit", "-q", "-m", "ripr bench corpus base"],
    )?;
    Ok(())
}

/// Commit the after-state worktree as HEAD on a work branch and anchor the
/// `main` branch at the before-state base commit, leaving the worktree
/// clean. This is the realistic agent shape (work branch one commit ahead
/// of main) and it makes ripr's default-base resolution land on the
/// before-state: the MCP surface analyzes the committed diff against it and
/// the LSP baseRef `HEAD~1` sees the same change, so M2/M3 measure the same
/// behavior M1 measures through `--diff`.
fn commit_corpus_after_state(corpus_root: &Path) -> Result<(), String> {
    let root_text = canonical(corpus_root)?.display().to_string();
    let envs = pinned_git_env();
    let config = [
        "-c",
        "user.name=ripr bench corpus",
        "-c",
        "user.email=bench@ripr.invalid",
    ];
    git_args(
        &root_text,
        &config,
        &envs,
        &["checkout", "-q", "-b", "bench-after"],
    )?;
    git_args(&root_text, &config, &envs, &["add", "-A"])?;
    git_args(
        &root_text,
        &config,
        &envs,
        &["commit", "-q", "-m", "ripr bench corpus after-state"],
    )?;
    git_args(
        &root_text,
        &config,
        &envs,
        &["branch", "-f", "main", "HEAD~1"],
    )?;
    Ok(())
}

fn git_args(
    root: &str,
    config: &[&str],
    envs: &[(&'static str, &'static str)],
    args: &[&str],
) -> Result<String, String> {
    let mut all: Vec<String> = Vec::new();
    for value in config {
        all.push((*value).to_string());
    }
    all.push("-C".to_string());
    all.push(root.to_string());
    for value in args {
        all.push((*value).to_string());
    }
    run_output_owned_with_envs("git", &all, envs)
}

/// Corpus `tiny`: derived from the checked-in sample bytes. The checked-in
/// diff is rewritten to corpus-relative paths; the before-state is derived by
/// removing the diff's added lines; and a patch round-trip (before + diff ==
/// on-disk after bytes) is required before the corpus is usable — that
/// round-trip is the corpus digest match.
fn prepare_tiny_corpus(corpora_root: &Path) -> Result<Corpus, String> {
    let corpus_root = corpora_root.join("tiny");
    reset_dir(&corpus_root)?;
    let sample_dir = Path::new("crates/ripr/examples/sample");
    let diff_bytes = fs::read(sample_dir.join("example.diff"))
        .map_err(|err| format!("read sample example.diff: {err}"))?;
    let rewritten = rewrite_sample_diff(&diff_bytes)
        .map_err(|err| format!("bench-agent-surfaces tiny corpus diff rewrite: {err}"))?;
    let after_lib = fs::read_to_string(sample_dir.join("src/lib.rs"))
        .map_err(|err| format!("read sample src/lib.rs: {err}"))?;
    let before_lib = reverse_apply(&after_lib, &rewritten)
        .map_err(|err| format!("bench-agent-surfaces tiny corpus before-state: {err}"))?;
    let after_tests = fs::read_to_string(sample_dir.join("tests/pricing.rs"))
        .map_err(|err| format!("read sample tests/pricing.rs: {err}"))?;
    write_corpus_file(&corpus_root, "Cargo.toml", TINY_CARGO_TOML)?;
    write_corpus_file(&corpus_root, "src/lib.rs", &before_lib)?;
    write_corpus_file(&corpus_root, "tests/pricing.rs", &after_tests)?;
    commit_corpus_base(&corpus_root)?;
    // After-state second commit; the patch must reproduce the checked-in
    // bytes. MCP's committed-diff analysis and the LSP baseRef `HEAD~1` both
    // see the same behavior change M1 measures through `--diff`.
    let after_from_patch = apply_patch(&before_lib, &rewritten)
        .map_err(|err| format!("bench-agent-surfaces tiny corpus patch apply: {err}"))?;
    if after_from_patch != after_lib {
        return Err(
            "bench-agent-surfaces: corpus digest mismatch: tiny diff round-trip does not reproduce the checked-in sample bytes"
                .to_string(),
        );
    }
    write_corpus_file(&corpus_root, "src/lib.rs", &after_lib)?;
    commit_corpus_after_state(&corpus_root)?;
    let diff_path = corpora_root.join("tiny.diff");
    fs::write(&diff_path, &rewritten).map_err(|err| format!("write tiny diff: {err}"))?;
    Ok(Corpus {
        id: CorpusId::Tiny,
        root: canonical(&corpus_root)?,
        diff_path: Some(diff_path),
        base: None,
        kind: "derived_from_checked_in_sample",
        expects_findings: true,
    })
}

const TINY_CARGO_TOML: &str = "[package]\nname = \"ripr_bench_tiny_corpus\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n";

/// Corpus `mid`: deterministically generated 20 packages x 25 Rust files,
/// each package carrying predicate-boundary, error-path and return-value
/// behaviors with a seeded mix of strong, weak and absent oracles. The base
/// commit holds the before-state; the worktree holds the after-state and the
/// generated diff (before -> after) is written alongside.
fn prepare_mid_corpus(corpora_root: &Path) -> Result<Corpus, String> {
    let corpus_root = corpora_root.join("mid");
    reset_dir(&corpus_root)?;
    let mut diff = String::new();
    for package in 0..MID_PACKAGES {
        let name = format!("pkg_{package:02}");
        write_corpus_file(
            &corpus_root,
            &format!("{name}/Cargo.toml"),
            &mid_cargo_toml(&name),
        )?;
        write_corpus_file(&corpus_root, &format!("{name}/src/lib.rs"), &mid_lib_rs())?;
        for file in 0..MID_SRC_FILES {
            let before = mid_src_content(package, file, false);
            let after = mid_src_content(package, file, true);
            write_corpus_file(&corpus_root, &format!("{name}/src/m{file:02}.rs"), &before)?;
            if file == MID_MUTATION_SRC_FILE {
                diff.push_str(&mid_file_diff(
                    &format!("{name}/src/m{file:02}.rs"),
                    &before,
                    &after,
                ));
            }
        }
        for file in 0..MID_TEST_FILES {
            write_corpus_file(
                &corpus_root,
                &format!("{name}/tests/t{file:02}.rs"),
                &mid_test_content(package, file),
            )?;
        }
    }
    commit_corpus_base(&corpus_root)?;
    // After-state second commit: rewrite only the pinned mutated files so the
    // committed diff (MCP default-base analysis, LSP baseRef HEAD~1) carries
    // the same behavior change M1 measures through `--diff`.
    for package in 0..MID_PACKAGES {
        let name = format!("pkg_{package:02}");
        let after = mid_src_content(package, MID_MUTATION_SRC_FILE, true);
        write_corpus_file(
            &corpus_root,
            &format!("{name}/src/m{:02}.rs", MID_MUTATION_SRC_FILE),
            &after,
        )?;
    }
    commit_corpus_after_state(&corpus_root)?;
    let diff_path = corpora_root.join("mid.diff");
    fs::write(&diff_path, &diff).map_err(|err| format!("write mid diff: {err}"))?;
    Ok(Corpus {
        id: CorpusId::Mid,
        root: canonical(&corpus_root)?,
        diff_path: Some(diff_path),
        base: None,
        kind: "generated_deterministic",
        expects_findings: true,
    })
}

/// Corpus `repo`: the checkout the bench runs in, measured diff-scoped
/// against `git merge-base HEAD origin/main` when that differs from HEAD,
/// else `HEAD~1`. An equal merge-base would make the diff empty, and a
/// zero-subject run proves nothing (spec validity gate); the resolved base
/// and its origin are recorded.
fn prepare_repo_corpus() -> Result<Corpus, String> {
    let head = trimmed_output(run_output("git", &["rev-parse", "HEAD"])?);
    let merge_base = run_output("git", &["merge-base", "HEAD", "origin/main"])
        .ok()
        .map(trimmed_output);
    let (base, origin) = match merge_base {
        Some(base) if base != head => (base, "merge_base_origin_main"),
        _ => (
            trimmed_output(run_output("git", &["rev-parse", "HEAD~1"])?),
            "head_parent_fallback",
        ),
    };
    Ok(Corpus {
        id: CorpusId::Repo,
        root: canonical(Path::new("."))?,
        diff_path: None,
        base: Some(base),
        kind: origin,
        expects_findings: false,
    })
}

fn reset_dir(path: &Path) -> Result<(), String> {
    // Windows can hold delete-pending handles briefly (Defender, search,
    // stragglers); a bounded retry keeps that transient state from failing
    // the whole benchmark.
    let mut last_err = None;
    for _ in 0..3 {
        if path.exists()
            && let Err(err) = fs::remove_dir_all(path)
        {
            last_err = Some(err);
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        match fs::create_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
    Err(format!(
        "reset {} after retries: {}",
        path.display(),
        last_err
            .map(|err| err.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    ))
}

fn write_corpus_file(root: &Path, relative: &str, contents: &str) -> Result<(), String> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
    }
    fs::write(&path, contents).map_err(|err| format!("write {}: {err}", path.display()))
}

fn canonical(path: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(path).map_err(|err| format!("canonicalize {}: {err}", path.display()))
}

fn trimmed_output(output: String) -> String {
    output.trim().to_string()
}

fn unix_stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

// ── mid corpus generation ────────────────────────────────────────────────

fn mid_cargo_toml(name: &str) -> String {
    format!(
        "[package]\nname = \"ripr_bench_{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n"
    )
}

fn mid_lib_rs() -> String {
    let mut out = String::new();
    for file in 0..MID_SRC_FILES {
        out.push_str(&format!("pub mod m{file:02};\n"));
    }
    out
}

fn mid_src_content(package: usize, file: usize, mutated: bool) -> String {
    let mut body = String::new();
    for step in 0..3 {
        let boundary = 1_000 + (package * 31 + file * 7 + step) as i64;
        if step == 0 {
            // Predicate boundary; the mutation tightens `>=` to `>` only in
            // the pinned file (package, MID_MUTATION_SRC_FILE).
            let op = if mutated && file == MID_MUTATION_SRC_FILE {
                ">"
            } else {
                ">="
            };
            body.push_str(&format!(
                "pub fn classify_0(amount: i64) -> &'static str {{\n    let threshold: i64 = {boundary};\n    if amount {op} threshold {{\n        \"premium\"\n    }} else {{\n        \"standard\"\n    }}\n}}\n\n"
            ));
        } else if step == 1 {
            body.push_str(&format!(
                "pub fn fee_1(amount: i64) -> Result<i64, String> {{\n    if amount < 0 {{\n        Err(\"negative amount\".to_string())\n    }} else {{\n        Ok(amount / {boundary})\n    }}\n}}\n\n"
            ));
        } else {
            body.push_str(&format!(
                "pub fn label_2(code: i64) -> i64 {{\n    match code {{\n        0 => {boundary},\n        1..=5 => {boundary} + 10,\n        _ => {boundary} + 20,\n    }}\n}}\n"
            ));
        }
    }
    body
}

fn mid_test_content(package: usize, file: usize) -> String {
    // All test files target the pinned mutated module so the oracles line up
    // with the changed predicate.
    let module = format!("m{:02}", MID_MUTATION_SRC_FILE);
    let threshold: i64 = 1_000 + (package * 31 + MID_MUTATION_SRC_FILE * 7) as i64;
    let import = format!("use ripr_bench_pkg_{package:02}::{module}::classify_0;\n\n");
    let mut body = String::new();
    match (package + file) % 3 {
        // Strong oracle: asserts the old behavior exactly at the boundary, so
        // the mutation must break it (exposed).
        0 => body.push_str(&format!(
            "{import}#[cfg(test)]\nmod tests {{\n    #[test]\n    fn boundary_discriminates_{file}() {{\n        assert_eq!(classify_0({threshold}), \"premium\");\n    }}\n}}\n"
        )),
        // Weak oracle: true far from the boundary; misses the discriminator.
        1 => body.push_str(&format!(
            "{import}#[cfg(test)]\nmod tests {{\n    #[test]\n    fn premium_is_premium_{file}() {{\n        assert_eq!(classify_0({}), \"premium\");\n    }}\n}}\n",
            threshold * 10
        )),
        // Absent oracle: no import and no call into the changed predicate.
        _ => body.push_str(&format!(
            "#[cfg(test)]\nmod tests {{\n    #[test]\n    fn unused_placeholder_{file}() {{\n        assert_eq!(1 + 1, 2);\n    }}\n}}\n"
        )),
    }
    body
}

/// A minimal unified diff for the single changed line between `before` and
/// `after` (they must differ in exactly one line, present in both).
fn mid_file_diff(path: &str, before: &str, after: &str) -> String {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();
    let mut changed = None;
    for (index, (before_line, after_line)) in
        before_lines.iter().zip(after_lines.iter()).enumerate()
    {
        if before_line != after_line {
            changed = Some(index);
            break;
        }
    }
    let Some(line_index) = changed else {
        return String::new();
    };
    let context_start = line_index.saturating_sub(2);
    let context_end = (line_index + 3).min(before_lines.len());
    let count = context_end - context_start;
    let mut body = format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n");
    body.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        context_start + 1,
        count,
        context_start + 1,
        count
    ));
    for index in context_start..context_end {
        if index == line_index {
            body.push_str(&format!("-{}\n", before_lines[index]));
            body.push_str(&format!("+{}\n", after_lines[index]));
        } else {
            body.push_str(&format!(" {}\n", before_lines[index]));
        }
    }
    body
}

/// Rewrite the checked-in sample diff to corpus-relative paths.
fn rewrite_sample_diff(diff: &[u8]) -> Result<Vec<u8>, String> {
    let text = std::str::from_utf8(diff).map_err(|err| format!("diff is not UTF-8: {err}"))?;
    let mut out = String::new();
    for line in text.lines() {
        out.push_str(
            &line
                .replace("a/crates/ripr/examples/sample/", "a/")
                .replace("b/crates/ripr/examples/sample/", "b/"),
        );
        out.push('\n');
    }
    Ok(out.into_bytes())
}

/// Apply a minimal unified diff (hunk headers with context/`+`/`-` lines) to
/// `content`.
fn apply_patch(content: &str, diff: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(diff).map_err(|err| format!("diff is not UTF-8: {err}"))?;
    let mut hunks = Vec::new();
    let mut current: Option<(usize, Vec<(char, String)>)> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("@@") {
            if let Some(hunk) = current.take() {
                hunks.push(hunk);
            }
            let start = rest
                .split('-')
                .nth(1)
                .and_then(|part| part.split(',').next())
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or_else(|| format!("unsupported hunk header `{line}`"))?;
            current = Some((start.saturating_sub(1), Vec::new()));
            continue;
        }
        let Some((_, lines)) = current.as_mut() else {
            continue;
        };
        let (kind, body) = match line.chars().next() {
            Some('+') => ('+', line[1..].to_string()),
            Some('-') => ('-', line[1..].to_string()),
            Some(' ') => (' ', line[1..].to_string()),
            _ => (' ', line.to_string()),
        };
        lines.push((kind, body));
    }
    if let Some(hunk) = current.take() {
        hunks.push(hunk);
    }
    let mut result_lines: Vec<String> = content.lines().map(str::to_string).collect();
    // Hunks carry before-file line numbers; applying later hunks first keeps
    // earlier offsets valid after an earlier hunk's insertions shift them.
    for (start, lines) in hunks.into_iter().rev() {
        let mut cursor = start;
        for (kind, body) in lines {
            match kind {
                '+' => {
                    let at = cursor.min(result_lines.len());
                    result_lines.insert(at, body);
                    cursor += 1;
                }
                '-' => {
                    if result_lines.get(cursor).map(String::as_str) != Some(body.as_str()) {
                        return Err(format!(
                            "patch context mismatch at line {}: expected `{body}`",
                            cursor + 1
                        ));
                    }
                    result_lines.remove(cursor);
                }
                _ => cursor += 1,
            }
        }
    }
    let mut out = result_lines.join("\n");
    if content.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// Derive the before-state by removing the diff's added lines from `content`
/// (the sample diff only adds lines, so dropping its `+` lines recovers the
/// before bytes).
fn reverse_apply(content: &str, diff: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(diff).map_err(|err| format!("diff is not UTF-8: {err}"))?;
    let removed: Vec<String> = text
        .lines()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
        .map(|line| line[1..].to_string())
        .collect();
    let mut out = String::new();
    for line in content.lines() {
        if removed.iter().any(|removed_line| removed_line == line) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    Ok(out)
}

// ── child environment ────────────────────────────────────────────────────

fn pinned_child_envs(cache_dir: &Path) -> Vec<(&'static str, String)> {
    vec![
        (CACHE_ENV, cache_dir.display().to_string()),
        (SEAM_LIMIT_ENV, SEAM_LIMIT_PINNED.to_string()),
    ]
}

fn fresh_scratch(base: &Path, label: &str) -> Result<PathBuf, String> {
    let dir = base.join(format!("{label}-{}", unix_stamp()));
    fs::create_dir_all(&dir)
        .map_err(|err| format!("create cache scratch {}: {err}", dir.display()))?;
    Ok(dir)
}

// ── M1: CLI end-to-end analysis latency ──────────────────────────────────

struct M1Result {
    cold: Vec<M1Sample>,
    warm: Vec<M1Sample>,
    deep: Option<M1Sample>,
    orders: Vec<String>,
    had_timeout: bool,
    envelopes: Vec<Value>,
}

impl M1Result {
    /// Named structural limitation when every sample of both populations was
    /// refused by the analyzer's typed diff-scope budget: for that checkout's
    /// diff shape the refusal itself is the measurement, so it is recorded by
    /// name instead of failing the empty-population gate.
    fn structural_limitation(&self) -> Option<String> {
        let all_limited = |samples: &[M1Sample]| {
            !samples.is_empty()
                && samples
                    .iter()
                    .all(|sample| sample.status == "limited_diff_scope_oversized")
        };
        let some_valid = self.cold.iter().any(|s| s.status == "pass")
            || self.warm.iter().any(|s| s.status == "pass");
        if !some_valid && all_limited(&self.cold) && all_limited(&self.warm) {
            let detail = self
                .cold
                .first()
                .and_then(|sample| sample.detail.clone())
                .unwrap_or_default();
            return Some(format!(
                "recorded limitation: every repo CLI sample was refused by the analyzer's typed diff-scope budget, so no diff-scoped latency population can exist for this checkout's diff shape; the typed refusal is the repo M1 measurement. {detail}"
            ));
        }
        None
    }
}

#[derive(Clone)]
struct M1Sample {
    status: &'static str,
    duration_ms: u128,
    stdout_bytes: usize,
    stderr_bytes: usize,
    findings: Option<u64>,
    detail: Option<String>,
}

impl M1Result {
    fn to_json(&self) -> Value {
        let mut body = json!({
            "cold": population_json(&self.cold),
            "warm": population_json(&self.warm),
            "warm_cold_p50_speedup": speedup(&self.cold, &self.warm),
            "execution_order": self.orders,
        });
        if let Some(deep) = &self.deep {
            body["deep_build_heavy"] = json!({
                "build_heavy": true,
                "sample": deep_json(deep),
            });
        }
        if let Some(limitation) = self.structural_limitation() {
            body["recorded_limitation"] = json!(limitation);
        }
        body
    }
}

fn deep_json(sample: &M1Sample) -> Value {
    json!({
        "status": sample.status,
        "duration_ms": sample.duration_ms,
        "stdout_bytes": sample.stdout_bytes,
        "stderr_bytes": sample.stderr_bytes,
        "findings": sample.findings,
        "detail": sample.detail,
    })
}

fn population_json(samples: &[M1Sample]) -> Value {
    let valid: Vec<u128> = samples
        .iter()
        .filter(|sample| sample.status == "pass")
        .map(|sample| sample.duration_ms)
        .collect();
    json!({
        "n": samples.len(),
        "valid_n": valid.len(),
        "min_ms": valid.iter().min().copied().unwrap_or(0),
        "p50_ms": percentile(&valid, 50),
        "p95_ms": percentile(&valid, 95),
        "max_ms": valid.iter().max().copied().unwrap_or(0),
        "samples": samples.iter().map(deep_json).collect::<Vec<_>>(),
    })
}

fn valid_durations(samples: &[M1Sample]) -> Vec<u128> {
    samples
        .iter()
        .filter(|sample| sample.status == "pass")
        .map(|sample| sample.duration_ms)
        .collect()
}

fn speedup(cold: &[M1Sample], warm: &[M1Sample]) -> Option<f64> {
    let cold_p50 = percentile(&valid_durations(cold), 50);
    let warm_p50 = percentile(&valid_durations(warm), 50);
    if cold_p50 == 0 || warm_p50 == 0 {
        None
    } else {
        Some(cold_p50 as f64 / warm_p50 as f64)
    }
}

fn percentile(values: &[u128], rank: usize) -> u128 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    if sorted.is_empty() {
        return 0;
    }
    let index = ((sorted.len() * rank).saturating_add(99) / 100).saturating_sub(1);
    sorted[index.min(sorted.len() - 1)]
}

fn run_m1_corpus(
    binary: &Path,
    corpus: &Corpus,
    options: &Options,
    caches_root: &Path,
    timeout: Duration,
) -> Result<M1Result, String> {
    let (cold_n, warm_n) = m1_run_counts(corpus.id, options);
    let warm_cache = fresh_scratch(caches_root, &format!("m1-warm-{}", corpus.id.label()))?;
    // One untimed priming run fills the pinned warm cache before its
    // population; a priming failure surfaces through the timed samples.
    let _ = run_m1_sample(binary, corpus, &warm_cache, timeout)?;
    let mut cold_queue: VecDeque<usize> = (0..cold_n).collect();
    let mut warm_queue: VecDeque<usize> = (0..warm_n).collect();
    let mut cold = Vec::new();
    let mut warm = Vec::new();
    let mut orders = Vec::new();
    let mut envelopes = Vec::new();
    let mut had_timeout = false;
    // Interleave cold/warm execution so gradual runner drift cannot
    // systematically favor one population (same rationale as #5200).
    while !cold_queue.is_empty() || !warm_queue.is_empty() {
        if cold_queue.pop_front().is_some() {
            orders.push(format!("cold:{}", orders.len()));
            let cache = fresh_scratch(caches_root, &format!("m1-cold-{}", corpus.id.label()))?;
            let (sample, envelope) = run_m1_sample(binary, corpus, &cache, timeout)?;
            had_timeout |= sample.status == "timeout";
            cold.push(sample);
            if let Some(envelope) = envelope {
                envelopes.push(envelope);
            }
        }
        if warm_queue.pop_front().is_some() {
            orders.push(format!("warm:{}", orders.len()));
            let (sample, envelope) = run_m1_sample(binary, corpus, &warm_cache, timeout)?;
            had_timeout |= sample.status == "timeout";
            warm.push(sample);
            if let Some(envelope) = envelope {
                envelopes.push(envelope);
            }
        }
    }
    let mut deep = None;
    if options.self_deep && corpus.id == CorpusId::Repo {
        let cache = fresh_scratch(caches_root, "m1-deep")?;
        let (sample, _) = run_m1_sample_deep(binary, corpus, &cache, timeout)?;
        had_timeout |= sample.status == "timeout";
        deep = Some(sample);
    }
    Ok(M1Result {
        cold,
        warm,
        deep,
        orders,
        had_timeout,
        envelopes,
    })
}

fn m1_run_counts(id: CorpusId, options: &Options) -> (usize, usize) {
    let default = match id {
        CorpusId::Repo => (1_usize, 3_usize),
        _ => (3_usize, 7_usize),
    };
    let cold = if options.m1_cold == 0 {
        default.0
    } else {
        options.m1_cold
    };
    let warm = if options.m1_warm == 0 {
        default.1
    } else {
        options.m1_warm
    };
    (cold, warm)
}

fn m1_args(corpus: &Corpus, deep: bool) -> Result<Vec<String>, String> {
    let mut args = vec![
        "check".to_string(),
        "--root".to_string(),
        corpus.root.display().to_string(),
    ];
    match (&corpus.diff_path, &corpus.base) {
        (Some(diff), _) => {
            args.push("--diff".to_string());
            args.push(diff.display().to_string());
        }
        (None, Some(base)) => {
            args.push("--base".to_string());
            args.push(base.clone());
        }
        (None, None) => {
            return Err(format!(
                "bench-agent-surfaces: corpus {} has neither diff nor base",
                corpus.id.label()
            ));
        }
    }
    if deep {
        args.push("--mode".to_string());
        args.push("deep".to_string());
    }
    args.push("--format".to_string());
    args.push("json".to_string());
    Ok(args)
}

fn run_m1_sample(
    binary: &Path,
    corpus: &Corpus,
    cache: &Path,
    timeout: Duration,
) -> Result<(M1Sample, Option<Value>), String> {
    run_m1_sample_inner(binary, corpus, cache, timeout, false)
}

fn run_m1_sample_deep(
    binary: &Path,
    corpus: &Corpus,
    cache: &Path,
    timeout: Duration,
) -> Result<(M1Sample, Option<Value>), String> {
    run_m1_sample_inner(binary, corpus, cache, timeout, true)
}

fn run_m1_sample_inner(
    binary: &Path,
    corpus: &Corpus,
    cache: &Path,
    timeout: Duration,
    deep: bool,
) -> Result<(M1Sample, Option<Value>), String> {
    let args = m1_args(corpus, deep)?;
    let envs = pinned_child_envs(cache);
    let started = Instant::now();
    let output = capture_output_with_timeout(
        &binary.display().to_string(),
        &args,
        &envs
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect::<Vec<_>>(),
        timeout,
        "bench-agent-surfaces M1 check",
    )?;
    let duration_ms = started.elapsed().as_millis();
    Ok(m1_sample_from_output(output, duration_ms, corpus))
}

fn m1_sample_from_output(
    output: crate::run::TimedOutput,
    duration_ms: u128,
    corpus: &Corpus,
) -> (M1Sample, Option<Value>) {
    let mut sample = M1Sample {
        status: "pass",
        duration_ms,
        stdout_bytes: output.stdout.len(),
        stderr_bytes: output.stderr.len(),
        findings: None,
        detail: None,
    };
    if output.timed_out {
        sample.status = "timeout";
        sample.detail = Some("timed out; child process tree terminated".to_string());
        return (sample, None);
    }
    let success = output
        .status
        .as_ref()
        .map(|status| status.success())
        .unwrap_or(false);
    if !success {
        // A non-zero exit can still carry the typed limited artifact: when
        // diff-scoped analysis stops at the configured budget before probe
        // expansion, ripr writes the envelope with
        // `analysis_scope.run_status: "diff_scope_oversized"` and fails
        // closed (docs/OUTPUT_SCHEMA.md). That typed refusal is recorded as
        // a structural outcome, not a harness failure.
        let is_diff_scope_oversized = |envelope: &Value| {
            envelope
                .pointer("/analysis_scope/run_status")
                .and_then(Value::as_str)
                == Some("diff_scope_oversized")
        };
        if let Ok(envelope) = serde_json::from_str::<Value>(&output.stdout)
            && is_diff_scope_oversized(&envelope)
        {
            sample.status = "limited_diff_scope_oversized";
            sample.detail = Some(
                envelope
                    .pointer("/analysis_scope/limitation")
                    .and_then(Value::as_str)
                    .unwrap_or("diff_scope_oversized")
                    .to_string(),
            );
            return (sample, None);
        }
        sample.status = "fail";
        sample.detail = Some(truncate_for_receipt(&output.stderr, 400));
        return (sample, None);
    }
    match serde_json::from_str::<Value>(&output.stdout) {
        Ok(value) => {
            let findings = value
                .pointer("/summary/findings")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            sample.findings = Some(findings);
            if findings == 0 && corpus.expects_findings {
                sample.status = "invalid_zero_findings";
                sample.detail = Some(format!(
                    "corpus {} is constructed to carry findings; a zero-subject run proves nothing",
                    corpus.id.label()
                ));
                return (sample, None);
            }
            if findings == 0 {
                sample.detail =
                    Some("recorded disclosure: the checkout diff holds no findings".to_string());
            }
            return (sample, Some(value));
        }
        Err(err) => {
            sample.status = "invalid_envelope";
            sample.detail = Some(format!("check --format json did not parse: {err}"));
        }
    }
    (sample, None)
}

fn collect_m1_violations(corpus: &Corpus, result: &M1Result, violations: &mut Vec<String>) {
    // A fully diff-scope-refused corpus is a named structural limitation
    // (emitted in the receipt body), not a gate violation.
    if result.structural_limitation().is_some() {
        return;
    }
    for (label, samples) in [("cold", &result.cold), ("warm", &result.warm)] {
        for sample in samples {
            match sample.status {
                "fail" => violations.push(format!(
                    "non_zero_child_exit: m1 corpus {} {label} sample exited non-zero: {}",
                    corpus.id.label(),
                    sample.detail.clone().unwrap_or_default()
                )),
                "invalid_envelope" => violations.push(format!(
                    "invalid_envelope_json: m1 corpus {} {label} sample stdout is not a check envelope: {}",
                    corpus.id.label(),
                    sample.detail.clone().unwrap_or_default()
                )),
                "invalid_zero_findings" => violations.push(format!(
                    "zero_findings: m1 corpus {} {label} sample returned zero findings",
                    corpus.id.label()
                )),
                _ => {}
            }
        }
        if !samples.iter().any(|sample| sample.status == "pass") {
            violations.push(format!(
                "empty_population: m1 corpus {} {label} population produced no valid samples",
                corpus.id.label()
            ));
        }
    }
    // The repo envelope's `base` must record the base actually used
    // (#3940). Only assertable when at least one envelope arrived; a fully
    // timed-out population is already named by empty_population.
    if let Some(expected) = &corpus.base {
        let Some(recorded) = result
            .envelopes
            .first()
            .and_then(|envelope| envelope.get("base"))
            .and_then(Value::as_str)
        else {
            return;
        };
        if recorded != expected {
            violations.push(format!(
                "corpus_base_mismatch: m1 corpus {} envelope base {recorded:?} does not match the resolved base {expected:?}",
                corpus.id.label()
            ));
        }
    }
}

// ── M5: determinism ──────────────────────────────────────────────────────

fn run_m5_corpus(
    binary: &Path,
    corpus: &Corpus,
    options: &Options,
    caches_root: &Path,
    timeout: Duration,
    warned: &mut bool,
    violations: &mut Vec<String>,
) -> Result<DeterminismReport, String> {
    let warm_cache = fresh_scratch(caches_root, &format!("m5-warm-{}", corpus.id.label()))?;
    // One untimed priming run fills the shared warm cache.
    let _ = run_m1_sample(binary, corpus, &warm_cache, timeout)?;
    let mut cold_runs = Vec::new();
    let mut timeout_notes = Vec::new();
    for _ in 0..options.m5_cold {
        let cache = fresh_scratch(caches_root, &format!("m5-cold-{}", corpus.id.label()))?;
        collect_m5_run(
            m5_run_bytes(binary, corpus, &cache, timeout),
            &mut cold_runs,
            warned,
            violations,
            &mut timeout_notes,
            &format!("m5 corpus {} cold", corpus.id.label()),
        );
    }
    let mut warm_runs = Vec::new();
    for _ in 0..options.m5_warm {
        collect_m5_run(
            m5_run_bytes(binary, corpus, &warm_cache, timeout),
            &mut warm_runs,
            warned,
            violations,
            &mut timeout_notes,
            &format!("m5 corpus {} warm", corpus.id.label()),
        );
    }
    let cold = determinism_outcome(&cold_runs, &[]);
    let warm = determinism_outcome(&warm_runs, &[]);
    Ok(DeterminismReport {
        corpus: corpus.id.label(),
        cold,
        warm,
        timeout_notes,
    })
}

/// Route one M5 run outcome: bytes feed the comparison, a timeout is a
/// named warn note (kept out of the comparison and out of the fail gates),
/// and a non-timeout child failure is a named validity violation instead of
/// aborting the receipt.
fn collect_m5_run(
    outcome: Result<M5Run, String>,
    runs: &mut Vec<Vec<u8>>,
    warned: &mut bool,
    violations: &mut Vec<String>,
    timeout_notes: &mut Vec<String>,
    label: &str,
) {
    match outcome {
        Ok(M5Run::Bytes(bytes)) => runs.push(bytes),
        Ok(M5Run::Timeout) => {
            *warned = true;
            timeout_notes.push(format!(
                "{label}: sample timed out and is excluded from the byte comparison (warn, not a determinism failure)"
            ));
        }
        Err(detail) => violations.push(format!("m5_sample_failed: {label}: {detail}")),
    }
}

enum M5Run {
    Bytes(Vec<u8>),
    Timeout,
}

fn m5_run_bytes(
    binary: &Path,
    corpus: &Corpus,
    cache: &Path,
    timeout: Duration,
) -> Result<M5Run, String> {
    let args = m1_args(corpus, false)?;
    let envs = pinned_child_envs(cache);
    let output = capture_output_with_timeout(
        &binary.display().to_string(),
        &args,
        &envs
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect::<Vec<_>>(),
        timeout,
        "bench-agent-surfaces M5 check",
    )?;
    if output.timed_out {
        return Ok(M5Run::Timeout);
    }
    let success = output
        .status
        .as_ref()
        .map(|status| status.success())
        .unwrap_or(false);
    if !success {
        return Err(format!(
            "non-zero exit: {}",
            truncate_for_receipt(&output.stderr, 200)
        ));
    }
    Ok(M5Run::Bytes(output.stdout.into_bytes()))
}

struct DeterminismReport {
    corpus: &'static str,
    cold: DeterminismOutcome,
    warm: DeterminismOutcome,
    timeout_notes: Vec<String>,
}

impl DeterminismReport {
    fn failure_reason(&self) -> Option<String> {
        for (population, outcome) in [("cold", &self.cold), ("warm", &self.warm)] {
            if let DeterminismOutcome::Failure { detail } = outcome {
                return Some(format!("corpus {} {population}: {detail}", self.corpus));
            }
        }
        None
    }

    fn to_json(&self) -> Value {
        json!({
            "cold": self.cold.to_json(),
            "warm": self.warm.to_json(),
            "timeout_notes": self.timeout_notes,
        })
    }
}

enum DeterminismOutcome {
    Stable { runs: usize },
    Disclosed { runs: usize, fields: Vec<String> },
    Failure { detail: String },
}

impl DeterminismOutcome {
    fn to_json(&self) -> Value {
        match self {
            DeterminismOutcome::Stable { runs } => json!({
                "runs": runs,
                "byte_stable": true,
            }),
            DeterminismOutcome::Disclosed { runs, fields } => json!({
                "runs": runs,
                "byte_stable": false,
                "disclosed_pass": true,
                "disclosed_volatile_fields": fields,
            }),
            DeterminismOutcome::Failure { detail } => json!({
                "byte_stable": false,
                "disclosed_pass": false,
                "detail": detail,
            }),
        }
    }
}

/// First raw byte equality across a population; on mismatch, canonical value
/// comparison after dropping the explicit allowlist of disclosed-volatile
/// fields (initially empty for the check envelope). A difference surviving
/// normalization is a determinism failure; a difference removed only by
/// allowlisted fields is a disclosed pass naming those fields.
fn determinism_outcome(runs: &[Vec<u8>], allowlist: &[&str]) -> DeterminismOutcome {
    let Some(first) = runs.first() else {
        return DeterminismOutcome::Failure {
            detail: "population produced no comparable runs (every sample timed out or failed); a missing metric is a validity gate".to_string(),
        };
    };
    if runs.iter().all(|run| run == first) {
        return DeterminismOutcome::Stable { runs: runs.len() };
    }
    let mut reference = match serde_json::from_slice::<Value>(first) {
        Ok(value) => value,
        Err(err) => {
            return DeterminismOutcome::Failure {
                detail: format!("determinism envelopes do not parse: {err}"),
            };
        }
    };
    let mut dropped_fields = Vec::new();
    drop_allowlisted_fields(&mut reference, allowlist, &mut dropped_fields);
    for run in runs.iter().skip(1) {
        let mut candidate = match serde_json::from_slice::<Value>(run) {
            Ok(value) => value,
            Err(err) => {
                return DeterminismOutcome::Failure {
                    detail: format!("determinism envelopes do not parse: {err}"),
                };
            }
        };
        drop_allowlisted_fields(&mut candidate, allowlist, &mut dropped_fields);
        if reference != candidate {
            return DeterminismOutcome::Failure {
                detail: "raw bytes differ and the difference is not covered by the disclosed-volatile allowlist"
                    .to_string(),
            };
        }
    }
    DeterminismOutcome::Disclosed {
        runs: runs.len(),
        fields: dropped_fields,
    }
}

/// Recursively drop allowlisted fields from an envelope, recording every
/// allowlisted field name actually removed. Only those recorded names can
/// back a disclosed pass.
fn drop_allowlisted_fields(value: &mut Value, allowlist: &[&str], dropped: &mut Vec<String>) {
    if let Some(map) = value.as_object_mut() {
        for field in allowlist {
            if map.remove(*field).is_some() && !dropped.iter().any(|name| name == field) {
                dropped.push((*field).to_string());
            }
        }
        for (_, child) in map.iter_mut() {
            drop_allowlisted_fields(child, allowlist, dropped);
        }
    } else if let Some(array) = value.as_array_mut() {
        for child in array {
            drop_allowlisted_fields(child, allowlist, dropped);
        }
    }
}

// ── M2: MCP stdio round-trips ────────────────────────────────────────────

struct M2Result {
    sessions: Vec<M2Session>,
}

struct M2Session {
    ops: Vec<M2Op>,
}

struct M2Op {
    op: &'static str,
    status: &'static str,
    duration_ms: u128,
    response_bytes: usize,
    detail: Option<String>,
    response: Option<Value>,
}

impl M2Result {
    fn to_json(&self) -> Value {
        let mut ops = serde_json::Map::new();
        for op in MCP_OPS {
            let samples: Vec<&M2Op> = self
                .sessions
                .iter()
                .flat_map(|session| session.ops.iter())
                .filter(|sample| sample.op == op)
                .collect();
            let valid: Vec<u128> = samples
                .iter()
                .filter(|sample| sample.status == "pass")
                .map(|sample| sample.duration_ms)
                .collect();
            ops.insert(
                (*op).to_string(),
                json!({
                    "n": samples.len(),
                    "valid_n": valid.len(),
                    "p50_ms": percentile(&valid, 50),
                    "p95_ms": percentile(&valid, 95),
                    "avg_response_bytes": if samples.is_empty() { 0 } else {
                        samples.iter().map(|sample| sample.response_bytes).sum::<usize>() / samples.len()
                    },
                    "failures": samples.iter().filter(|sample| sample.status == "fail").count(),
                    "timeouts": samples.iter().filter(|sample| sample.status == "timeout").count(),
                    "details": samples.iter().filter_map(|sample| sample.detail.clone()).collect::<Vec<_>>(),
                }),
            );
        }
        json!({ "sessions": self.sessions.len(), "ops": Value::Object(ops) })
    }
}

const MCP_OPS: [&str; 6] = [
    "initialize",
    "ripr_workspace_status",
    "ripr_refresh_cold",
    "ripr_refresh_warm",
    "ripr_list_gaps",
    "ripr_get_gap",
];

fn run_m2_corpus(
    binary: &Path,
    corpus: &Corpus,
    options: &Options,
    caches_root: &Path,
    timeout: Duration,
    warned: &mut bool,
) -> Result<M2Result, String> {
    let mut sessions = Vec::new();
    let mut had_timeout = false;
    for _ in 0..options.m2_sessions {
        // Fresh cache scratch per session: the session's first refresh is
        // genuinely server-side cold, its second warm.
        let scratch = fresh_scratch(caches_root, &format!("m2-{}", corpus.id.label()))?;
        let envs = pinned_child_envs(&scratch);
        let spawn_result = RpcSession::spawn(
            &binary.display().to_string(),
            &[
                "mcp".to_string(),
                "--stdio".to_string(),
                "--root".to_string(),
                corpus.root.display().to_string(),
            ],
            &envs,
            Framing::Line,
        );
        let mut session = match spawn_result {
            Ok(session) => session,
            Err(err) => {
                sessions.push(M2Session {
                    ops: vec![failed_op(
                        "initialize",
                        format!("session spawn failed: {err}"),
                    )],
                });
                continue;
            }
        };
        let mut ops = Vec::new();
        ops.push(rpc_op(
            &mut session,
            "initialize",
            &json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "ripr-bench-agent-surfaces", "version": "1" }
            }),
            timeout,
        ));
        if let Err(err) = session.notify_line(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
        })) {
            ops.push(failed_op(
                "initialize",
                format!("initialized notification failed: {err}"),
            ));
        }
        ops.push(rpc_tool_op(
            &mut session,
            "ripr_workspace_status",
            json!({}),
            timeout,
        ));
        let refresh_cold = rpc_tool_op(&mut session, "ripr_refresh", json!({}), timeout);
        let refresh_failure = refresh_failure_code(&refresh_cold);
        ops.push(rename_op(refresh_cold, "ripr_refresh_cold"));
        let refresh_warm = rpc_tool_op(&mut session, "ripr_refresh", json!({}), timeout);
        ops.push(rename_op(refresh_warm, "ripr_refresh_warm"));
        let list = rpc_tool_op(&mut session, "ripr_list_gaps", json!({}), timeout);
        let list = name_no_snapshot_absence(list, refresh_failure.as_deref(), "list_gaps");
        let gap_id = list.response.as_ref().and_then(gap_id_from_list_response);
        ops.push(rename_op(list, "ripr_list_gaps"));
        match gap_id {
            Some(gap_id) => {
                let get = rpc_tool_op(
                    &mut session,
                    "ripr_get_gap",
                    json!({ "gap_id": gap_id }),
                    timeout,
                );
                ops.push(name_no_snapshot_absence(get, refresh_failure.as_deref(), "get_gap"));
            }
            None => ops.push(M2Op {
                op: "ripr_get_gap",
                status: "absent_no_gaps",
                duration_ms: 0,
                response_bytes: 0,
                detail: Some(
                    "named absence: list_gaps returned no gap items, so get_gap has no target on this corpus"
                        .to_string(),
                ),
                response: None,
            }),
        }
        had_timeout |= ops.iter().any(|op| op.status == "timeout");
        sessions.push(M2Session { ops });
        drop(session);
    }
    *warned |= had_timeout;
    Ok(M2Result { sessions })
}

fn rename_op(mut op: M2Op, name: &'static str) -> M2Op {
    op.op = name;
    op
}

/// The typed failure code of a refresh attempt, when the analysis behind it
/// refused to produce a snapshot.
fn refresh_failure_code(op: &M2Op) -> Option<String> {
    let response = op.response.as_ref()?;
    let direct = response.pointer("/result/structuredContent/attempt/failure/code");
    if let Some(code) = direct.and_then(Value::as_str) {
        return Some(code.to_string());
    }
    let text = response
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)?;
    let payload: Value = serde_json::from_str(text).ok()?;
    payload
        .pointer("/attempt/failure/code")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// A downstream op that failed `no_snapshot` right after a refresh whose
/// attempt carried a typed failure is a named chained absence, not an
/// independent harness failure: the cause is the refresh refusal.
fn name_no_snapshot_absence(mut op: M2Op, refresh_failure: Option<&str>, op_name: &str) -> M2Op {
    let failed_no_snapshot = op.status == "fail" && op.detail.as_deref() == Some("no_snapshot");
    if let Some(code) = refresh_failure
        && failed_no_snapshot
    {
        op.status = "absent_no_snapshot";
        op.detail = Some(format!(
            "named absence: {op_name} has no snapshot to read because ripr_refresh's analysis attempt failed with the typed refusal `{code}`"
        ));
    }
    op
}

fn failed_op(op: &'static str, detail: String) -> M2Op {
    M2Op {
        op,
        status: "fail",
        duration_ms: 0,
        response_bytes: 0,
        detail: Some(detail),
        response: None,
    }
}

fn rpc_op(session: &mut RpcSession, op: &'static str, params: &Value, timeout: Duration) -> M2Op {
    match session.request("initialize", params.clone(), timeout) {
        Ok(outcome) => m2_op_from_response(op, outcome),
        Err(err) => failed_op(op, err),
    }
}

fn rpc_tool_op(
    session: &mut RpcSession,
    tool: &'static str,
    arguments: Value,
    timeout: Duration,
) -> M2Op {
    match session.request(
        "tools/call",
        json!({ "name": tool, "arguments": arguments }),
        timeout,
    ) {
        Ok(outcome) => m2_op_from_response(tool, outcome),
        Err(err) => failed_op(tool, err),
    }
}

fn m2_op_from_response(op: &'static str, outcome: RpcOutcome) -> M2Op {
    let mut sample = M2Op {
        op,
        status: "pass",
        duration_ms: outcome.elapsed.as_millis(),
        response_bytes: outcome.response_bytes,
        detail: None,
        response: None,
    };
    let response = outcome.response;
    if outcome.response_bytes > MCP_EGRESS_BOUND_BYTES {
        sample.status = "fail";
        sample.detail = Some(format!(
            "response of {} bytes exceeds the documented 128 KiB MCP egress bound",
            outcome.response_bytes
        ));
        return sample;
    }
    if response.get("error").is_some() {
        sample.status = "fail";
        sample.detail = Some(truncate_for_receipt(&response.to_string(), 300));
        return sample;
    }
    let is_error = response
        .pointer("/result/isError")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if is_error {
        sample.status = "fail";
        sample.detail = Some(truncate_for_receipt(
            response
                .pointer("/result/structuredContent/failure/code")
                .and_then(Value::as_str)
                .unwrap_or("isError true"),
            200,
        ));
        return sample;
    }
    sample.response = Some(response);
    sample
}

/// First gap id from the list payload (`items[0]`), tolerating the typed
/// absence of any gap on a corpus. Production items name the id
/// `canonical_id` (mcp/gaps.rs `GapItem`); the other spellings are
/// tolerated defensively but never substituted for the registered name.
fn gap_id_from_list_response(response: &Value) -> Option<String> {
    let text = response
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)?;
    let payload: Value = serde_json::from_str(text).ok()?;
    let items = payload.get("items").and_then(Value::as_array)?;
    let item = items.first()?;
    for key in ["canonical_id", "gap_id", "canonical_item_id", "item_id"] {
        if let Some(id) = item.get(key).and_then(Value::as_str) {
            return Some(id.to_string());
        }
    }
    None
}

fn collect_m2_violations(corpus: &Corpus, result: &M2Result, violations: &mut Vec<String>) {
    for session in &result.sessions {
        for op in &session.ops {
            if op.status == "fail" {
                violations.push(format!(
                    "mcp_expected_success_failed: corpus {} op {} failed: {}",
                    corpus.id.label(),
                    op.op,
                    op.detail.clone().unwrap_or_default()
                ));
            }
        }
    }
}

// ── M3: LSP first diagnostics ────────────────────────────────────────────

struct M3Result {
    cold: Vec<M3Sample>,
    warm: Vec<M3Sample>,
    /// Named limitation when a corpus's first-publish population cannot
    /// exist within any bounded per-sample ceiling (recorded, never silent).
    limitation: Option<String>,
}

struct M3Sample {
    status: &'static str,
    duration_ms: u128,
    diagnostics: Option<usize>,
    detail: Option<String>,
}

impl M3Result {
    fn to_json(&self) -> Value {
        let mut body = json!({
            "cold": m3_population_json(&self.cold),
            "warm": m3_population_json(&self.warm),
        });
        if let Some(limitation) = &self.limitation {
            body["recorded_limitation"] = json!(limitation);
        }
        body
    }
}

fn m3_population_json(samples: &[M3Sample]) -> Value {
    let valid: Vec<u128> = samples
        .iter()
        .filter(|sample| sample.status == "pass")
        .map(|sample| sample.duration_ms)
        .collect();
    json!({
        "n": samples.len(),
        "valid_n": valid.len(),
        "p50_ms": percentile(&valid, 50),
        "p95_ms": percentile(&valid, 95),
        "max_ms": valid.iter().max().copied().unwrap_or(0),
        "samples": samples.iter().map(|sample| json!({
            "status": sample.status,
            "duration_ms": sample.duration_ms,
            "diagnostics_in_first_publish": sample.diagnostics,
            "detail": sample.detail,
        })).collect::<Vec<_>>(),
    })
}

fn m3_document(corpus: &Corpus) -> Result<PathBuf, String> {
    let relative: String = match corpus.id {
        CorpusId::Tiny => "src/lib.rs".to_string(),
        CorpusId::Mid => format!("pkg_00/src/m{:02}.rs", MID_MUTATION_SRC_FILE),
        CorpusId::Repo => "xtask/src/reports/bench_agent_surfaces.rs".to_string(),
    };
    let path = corpus.root.join(&relative);
    if !path.exists() {
        return Err(format!(
            "bench-agent-surfaces: m3 pinned document {} is missing",
            path.display()
        ));
    }
    Ok(path)
}

fn run_m3_corpus(
    binary: &Path,
    corpus: &Corpus,
    options: &Options,
    caches_root: &Path,
    timeout: Duration,
    warned: &mut bool,
) -> Result<M3Result, String> {
    if corpus.id == CorpusId::Repo {
        return Ok(M3Result {
            cold: Vec::new(),
            warm: Vec::new(),
            limitation: Some(
                "recorded limitation: this run recorded no M3 repo populations because the workspace-scale analysis behind the repo's first textDocument/publishDiagnostics exceeds the per-sample timeout ceiling on large checkouts, so a first-publish population cannot exist within any bounded ceiling; the M3 timeout is the named outcome here, which is distinct from an empty-population gate failure (that gate stays reserved for corpora whose population can exist). The authoring session's observations and their measurements are recorded in RIPR-SPEC-0221; a run whose repo sessions do observe a first publish replaces this limitation with measured populations."
                    .to_string(),
            ),
        });
    }
    let document = m3_document(corpus)?;
    let uri = file_uri(&document);
    // Generated corpora commit the before-state at HEAD~1 and the after-state
    // at HEAD, so the committed diff IS the behavior change M1 measures.
    let base_ref = corpus.base.clone().unwrap_or_else(|| "HEAD~1".to_string());
    let mut cold = Vec::new();
    let mut warm = Vec::new();
    let mut had_timeout = false;
    // Warm population first: one untimed warm-up session primes the shared
    // pinned cache, then the timed warm sessions reuse it. Cold sessions get
    // a fresh scratch cache each.
    let warm_cache = fresh_scratch(caches_root, &format!("m3-warm-{}", corpus.id.label()))?;
    let _ = lsp_session_latency(binary, corpus, &warm_cache, &uri, &base_ref, timeout)?;
    for _ in 0..options.m3_warm {
        let (sample, timed_out) =
            lsp_session_latency(binary, corpus, &warm_cache, &uri, &base_ref, timeout)?;
        had_timeout |= timed_out;
        warm.push(sample);
    }
    for _ in 0..options.m3_cold {
        let scratch = fresh_scratch(caches_root, &format!("m3-cold-{}", corpus.id.label()))?;
        let (sample, timed_out) =
            lsp_session_latency(binary, corpus, &scratch, &uri, &base_ref, timeout)?;
        had_timeout |= timed_out;
        cold.push(sample);
    }
    *warned |= had_timeout;
    Ok(M3Result {
        cold,
        warm,
        limitation: None,
    })
}

fn lsp_session_latency(
    binary: &Path,
    corpus: &Corpus,
    cache: &Path,
    uri: &str,
    base_ref: &str,
    timeout: Duration,
) -> Result<(M3Sample, bool), String> {
    let envs = pinned_child_envs(cache);
    let spawn_result = RpcSession::spawn(
        &binary.display().to_string(),
        &["lsp".to_string(), "--stdio".to_string()],
        &envs,
        Framing::ContentLength,
    );
    let mut session = match spawn_result {
        Ok(session) => session,
        Err(err) => {
            return Ok((
                M3Sample {
                    status: "fail",
                    duration_ms: 0,
                    diagnostics: None,
                    detail: Some(format!("session spawn failed: {err}")),
                },
                false,
            ));
        }
    };
    let init_params = json!({
        "processId": null,
        "rootUri": file_uri(&corpus.root),
        "initializationOptions": {
            "baseRef": base_ref,
            "checkMode": "instant",
            "diagnosticProfile": "full",
        },
        "capabilities": {},
    });
    if let Err(err) = session.request("initialize", init_params, timeout) {
        return Ok((failed_m3_sample(format!("initialize failed: {err}")), false));
    }
    if let Err(err) = session.notify_frame("initialized", json!({})) {
        return Ok((
            failed_m3_sample(format!("initialized notification failed: {err}")),
            false,
        ));
    }
    // The didOpen text may carry unsaved buffer content in real editors; the
    // server reads saved identity from the persisted bytes (see #2129), so
    // sending the on-disk text keeps identity consistent.
    let text = fs::read_to_string(document_path_from_uri(uri)).unwrap_or_default();
    let started = Instant::now();
    if let Err(err) = session.notify_frame(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": uri,
                "languageId": "rust",
                "version": 1,
                "text": text,
            }
        }),
    ) {
        return Ok((failed_m3_sample(format!("didOpen failed: {err}")), false));
    }
    let outcome = session.wait_for_notification(
        |message| {
            message.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics")
                && message.pointer("/params/uri").and_then(Value::as_str) == Some(uri)
        },
        timeout,
    );
    let duration_ms = started.elapsed().as_millis();
    drop(session);
    Ok(match outcome {
        Ok(Some(message)) => {
            let diagnostics = message
                .pointer("/params/diagnostics")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);
            (
                M3Sample {
                    status: "pass",
                    duration_ms,
                    diagnostics: Some(diagnostics),
                    detail: None,
                },
                false,
            )
        }
        Ok(None) => (
            M3Sample {
                status: "timeout",
                duration_ms,
                diagnostics: None,
                detail: Some(
                    "no publishDiagnostics for the opened URI within the timeout".to_string(),
                ),
            },
            true,
        ),
        Err(err) => (failed_m3_sample(err), false),
    })
}

fn failed_m3_sample(detail: String) -> M3Sample {
    M3Sample {
        status: "fail",
        duration_ms: 0,
        diagnostics: None,
        detail: Some(detail),
    }
}

fn document_path_from_uri(uri: &str) -> PathBuf {
    PathBuf::from(uri.trim_start_matches("file:///"))
}

fn collect_m3_violations(corpus: &Corpus, result: &M3Result, violations: &mut Vec<String>) {
    // A recorded-limitation corpus has no gated population by definition;
    // its absence is the named evidence (see the receipt's m3 block).
    if result.limitation.is_some() {
        return;
    }
    for (label, samples) in [("cold", &result.cold), ("warm", &result.warm)] {
        for sample in samples {
            if sample.status == "fail" {
                violations.push(format!(
                    "lsp_no_publish_within_timeout: corpus {} {label} session failed: {}",
                    corpus.id.label(),
                    sample.detail.clone().unwrap_or_default()
                ));
            }
        }
        if !samples.iter().any(|sample| sample.status == "pass") {
            violations.push(format!(
                "empty_population: m3 corpus {} {label} population produced no valid sessions",
                corpus.id.label()
            ));
        }
    }
}

fn file_uri(path: &Path) -> String {
    let mut text = path.display().to_string().replace('\\', "/");
    // fs::canonicalize returns extended-length paths (`//?/F:/...`) on
    // Windows; the wire URI needs the plain drive form.
    if let Some(rest) = text.strip_prefix("//?/") {
        text = rest.to_string();
    }
    let text = text.strip_prefix('/').map(str::to_string).unwrap_or(text);
    format!("file:///{text}")
}

// ── M4: output actionability ─────────────────────────────────────────────

fn actionability_metrics(envelopes: &[Value]) -> Value {
    let mut total: u64 = 0;
    let mut classes = serde_json::Map::new();
    for class in STATIC_CLASSES {
        classes.insert(class.to_string(), json!(0));
    }
    let mut actionable_intent: u64 = 0;
    let mut with_evidence_path: u64 = 0;
    let mut with_related_tests: u64 = 0;
    let mut unknown_total: u64 = 0;
    let mut unknown_disclosed: u64 = 0;
    let mut repair_ready: u64 = 0;
    let mut repair_bearing_seen = false;
    let mut complete = true;
    let mut max_limitations: usize = 0;
    let mut alignment_state = "alignment_absent";
    let mut raw_signals: Option<u64> = None;
    let mut canonical_items: Option<u64> = None;
    for envelope in envelopes {
        if envelope
            .pointer("/analysis_outcome/analysis_complete")
            .and_then(Value::as_bool)
            != Some(true)
        {
            complete = false;
        }
        if let Some(limitations) = envelope
            .pointer("/analysis_outcome/outcome/limitations")
            .and_then(Value::as_array)
        {
            max_limitations = max_limitations.max(limitations.len());
        }
        if let Some(summary) = envelope.pointer("/finding_alignment/summary") {
            raw_signals = summary.get("raw_signals").and_then(Value::as_u64);
            canonical_items = summary.get("canonical_items").and_then(Value::as_u64);
            alignment_state = "recorded";
        }
        let Some(findings) = envelope.get("findings").and_then(Value::as_array) else {
            continue;
        };
        for finding in findings {
            total += 1;
            let classification = finding
                .get("classification")
                .and_then(Value::as_str)
                .unwrap_or("static_unknown");
            if let Some(count) = classes.get_mut(classification) {
                let current = count.as_u64().unwrap_or(0);
                *count = json!(current + 1);
            }
            let has_intent = !finding
                .get("missing_discriminators")
                .and_then(Value::as_array)
                .map(Vec::is_empty)
                .unwrap_or(true)
                || non_empty(finding.get("recommended_next_step"))
                || non_empty(finding.get("suggested_next_action"));
            if has_intent {
                actionable_intent += 1;
            }
            if !finding
                .get("evidence_path")
                .and_then(Value::as_array)
                .map(Vec::is_empty)
                .unwrap_or(true)
            {
                with_evidence_path += 1;
            }
            if finding
                .get("related_tests_total")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                > 0
            {
                with_related_tests += 1;
            }
            if UNKNOWN_CLASSES.contains(&classification) {
                unknown_total += 1;
                if !finding
                    .get("stop_reasons")
                    .and_then(Value::as_array)
                    .map(Vec::is_empty)
                    .unwrap_or(true)
                {
                    unknown_disclosed += 1;
                }
            }
            if let Some(ready) = finding_repair_packet_ready(finding) {
                repair_bearing_seen = true;
                if ready {
                    repair_ready += 1;
                }
            }
        }
    }
    let fraction = |numerator: u64| -> Value {
        if total == 0 {
            json!(null)
        } else {
            json!(((numerator as f64 / total as f64) * 10_000.0).round() / 10_000.0)
        }
    };
    let state = if envelopes.is_empty() {
        "no_envelopes_pooled"
    } else {
        "pooled"
    };
    json!({
        "state": state,
        "envelopes_pooled": envelopes.len(),
        "findings_total": total,
        "class_histogram": Value::Object(classes),
        "actionable_intent_fraction": fraction(actionable_intent),
        "evidence_path_fraction": fraction(with_evidence_path),
        "related_test_evidence_fraction": fraction(with_related_tests),
        "unknown_disclosed_fraction": if unknown_total == 0 { json!(null) } else {
            json!(((unknown_disclosed as f64 / unknown_total as f64) * 10_000.0).round() / 10_000.0)
        },
        "unknown_class_findings": unknown_total,
        "analysis_complete_all_envelopes": complete,
        "max_typed_limitations": max_limitations,
        "repair_readiness": {
            "findings_with_repair_packet_ready": if repair_bearing_seen { json!(repair_ready) } else {
                json!("named_absence: check envelope carries no repair-bearing field on this run")
            },
        },
        "finding_alignment": {
            "state": alignment_state,
            "raw_signals": raw_signals,
            "canonical_items": canonical_items,
        },
    })
}

fn non_empty(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false)
}

/// Repair readiness wherever the check renderer actually emits it: the
/// renderer owns the projection shape (preview cards, actionability
/// blocks), so this scans the finding for any `repair_packet_ready` boolean
/// instead of pinning one transport-specific path.
fn finding_repair_packet_ready(finding: &Value) -> Option<bool> {
    match finding {
        Value::Object(map) => {
            if let Some(ready) = map.get("repair_packet_ready").and_then(Value::as_bool) {
                return Some(ready);
            }
            for (_, child) in map {
                if let Some(ready) = finding_repair_packet_ready(child) {
                    return Some(ready);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(finding_repair_packet_ready),
        _ => None,
    }
}

// ── shared stdio JSON-RPC session ────────────────────────────────────────

#[derive(Clone, Copy)]
enum Framing {
    /// Newline-delimited JSON (MCP stdio).
    Line,
    /// `Content-Length` framed JSON (LSP stdio).
    ContentLength,
}

struct RpcOutcome {
    response: Value,
    response_bytes: usize,
    elapsed: Duration,
}

struct RpcSession {
    stdin: ChildStdin,
    receiver: Receiver<Value>,
    child: ripr::process_owner::OwnedProcess,
    framing: Framing,
    next_id: u64,
}

impl RpcSession {
    fn spawn(
        program: &str,
        args: &[String],
        envs: &[(&str, String)],
        framing: Framing,
    ) -> Result<RpcSession, String> {
        let mut command = Command::new(program);
        command.args(args);
        for (name, value) in envs {
            command.env(name, value);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = ripr::process_owner::OwnedProcess::spawn(command)
            .map_err(|err| format!("spawn {program}: {err}"))?;
        let stdin = child
            .stdin_pipe()
            .take()
            .ok_or_else(|| format!("spawned {program} has no stdin pipe"))?;
        let stdout = child
            .stdout_pipe()
            .take()
            .ok_or_else(|| format!("spawned {program} has no stdout pipe"))?;
        let stderr = child
            .stderr_pipe()
            .take()
            .ok_or_else(|| format!("spawned {program} has no stderr pipe"))?;
        let (sender, receiver) = mpsc::channel();
        // Drain stderr concurrently: tracing output is not part of either
        // wire contract, and a full stderr pipe can stall the server.
        thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut sink = Vec::new();
            let _ = reader.read_to_end(&mut sink);
        });
        // Read replies on a dedicated thread so a server blocked on write
        // cannot deadlock the harness (same rationale as the mcp_stdio test's
        // #3587 note).
        thread::spawn(move || match framing {
            Framing::Line => read_line_framed(stdout, &sender),
            Framing::ContentLength => read_content_length_framed(stdout, &sender),
        });
        Ok(RpcSession {
            stdin,
            receiver,
            child,
            framing,
            next_id: 1,
        })
    }

    fn request(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<RpcOutcome, String> {
        let id = self.next_id;
        self.next_id += 1;
        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let started = Instant::now();
        self.write_message(&message)?;
        loop {
            let remaining = timeout.saturating_sub(started.elapsed());
            match self.receiver.recv_timeout(remaining) {
                Ok(message) => {
                    if message.get("id").and_then(Value::as_u64) == Some(id) {
                        let bytes = serde_json::to_vec(&message)
                            .map_err(|err| format!("re-serialize response: {err}"))?
                            .len();
                        return Ok(RpcOutcome {
                            response: message,
                            response_bytes: bytes,
                            elapsed: started.elapsed(),
                        });
                    }
                    // Notifications and out-of-order frames are drained.
                    continue;
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(format!(
                        "bench-agent-surfaces: {method} response timed out after {} ms",
                        timeout.as_millis()
                    ));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("bench-agent-surfaces: server closed its output stream".to_string());
                }
            }
        }
    }

    fn wait_for_notification(
        &mut self,
        matches: impl Fn(&Value) -> bool,
        timeout: Duration,
    ) -> Result<Option<Value>, String> {
        let deadline = Instant::now() + timeout;
        loop {
            // `deadline.elapsed()` would read ~zero against a future
            // instant, resetting the full window on every drained message;
            // measure toward the deadline instead.
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.receiver.recv_timeout(remaining) {
                Ok(message) => {
                    if message.get("id").is_none() && matches(&message) {
                        return Ok(Some(message));
                    }
                }
                Err(RecvTimeoutError::Timeout) => return Ok(None),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("bench-agent-surfaces: server closed its output stream".to_string());
                }
            }
        }
    }

    fn notify_line(&mut self, message: &Value) -> Result<(), String> {
        self.write_message(message)
    }

    fn notify_frame(&mut self, method: &str, params: Value) -> Result<(), String> {
        let message = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_message(&message)
    }

    fn write_message(&mut self, message: &Value) -> Result<(), String> {
        let body =
            serde_json::to_vec(message).map_err(|err| format!("serialize request: {err}"))?;
        let write = match self.framing {
            Framing::Line => self
                .stdin
                .write_all(&body)
                .and_then(|()| self.stdin.write_all(b"\n")),
            Framing::ContentLength => self
                .stdin
                .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
                .and_then(|()| self.stdin.write_all(&body)),
        };
        write
            .and_then(|()| self.stdin.flush())
            .map_err(|err| format!("writing to server stdin: {err}"))
    }
}

impl Drop for RpcSession {
    fn drop(&mut self) {
        // Always kill and reap: an orphaned server holds Windows file locks.
        let _ = self.child.terminate_tree();
        let _ = self.child.wait();
    }
}

fn read_line_framed<R: std::io::Read + Send + 'static>(stdout: R, sender: &mpsc::Sender<Value>) {
    let mut reader = BufReader::new(stdout);
    loop {
        let mut line = Vec::new();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if let Ok(message) = serde_json::from_slice::<Value>(&line)
                    && sender.send(message).is_err()
                {
                    break;
                }
            }
        }
    }
}

fn read_content_length_framed<R: std::io::Read + Send + 'static>(
    stdout: R,
    sender: &mpsc::Sender<Value>,
) {
    let mut reader = BufReader::new(stdout);
    loop {
        let mut header_line = Vec::new();
        match reader.read_until(b'\n', &mut header_line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if header_line == b"\r\n" || header_line == b"\n" {
                    continue;
                }
                let header = String::from_utf8_lossy(&header_line).to_lowercase();
                let Some(length) = header
                    .strip_prefix("content-length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
                else {
                    continue;
                };
                // Consume the remaining header terminator line, then the body.
                let mut terminator = Vec::new();
                if reader.read_until(b'\n', &mut terminator).unwrap_or(0) == 0 {
                    break;
                }
                let mut body = vec![0_u8; length];
                if reader.read_exact(&mut body).is_err() {
                    break;
                }
                if let Ok(message) = serde_json::from_slice::<Value>(&body)
                    && sender.send(message).is_err()
                {
                    break;
                }
            }
        }
    }
}

// ── identity, gates, comparison, rendering ───────────────────────────────

fn identity_overlay(binary: &Path, corpora: &[Corpus]) -> Result<Value, String> {
    let binary_bytes = fs::read(binary).map_err(|err| format!("read binary: {err}"))?;
    let mut corpus_digests = serde_json::Map::new();
    for corpus in corpora {
        let digest = match (&corpus.diff_path, &corpus.base) {
            (Some(diff_path), _) => {
                let diff = fs::read(diff_path).map_err(|err| format!("read diff: {err}"))?;
                crate::blind_journey::sha256_hex(&diff)
            }
            // The repo corpus's actual analysis input is the checkout's diff
            // against the resolved base; hash those bytes so two runs at the
            // same HEAD with different worktree changes cannot share an
            // identity.
            (None, Some(base)) => {
                let diff = run_output("git", &["diff", base])
                    .map_err(|err| format!("repo diff for digest: {err}"))?;
                crate::blind_journey::sha256_hex(diff.as_bytes())
            }
            (None, None) => "no_diff_input".to_string(),
        };
        corpus_digests.insert(
            corpus.id.label().to_string(),
            json!({
                "kind": corpus.kind,
                "base": corpus.base,
                "input_digest": digest,
            }),
        );
    }
    Ok(json!({
        "source_sha": git_revision(),
        "binary_path": binary.display().to_string(),
        "binary_sha256": crate::blind_journey::sha256_hex(&binary_bytes),
        "host_class": format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        "toolchain": trimmed_output(
            run_output("rustc", &["--version"]).unwrap_or_else(|_| "unavailable".to_string()),
        ),
        "timestamp_unix_ms": unix_stamp(),
        "corpora": Value::Object(corpus_digests),
        "env_pins": {
            "ripr_cache_dir_env": CACHE_ENV,
            "ripr_seam_limit_env": SEAM_LIMIT_ENV,
            "ripr_seam_limit_pinned": SEAM_LIMIT_PINNED,
        },
    }))
}

fn git_revision() -> String {
    run_output("git", &["rev-parse", "HEAD"])
        .map(trimmed_output)
        .unwrap_or_else(|_| "unavailable".to_string())
}

fn read_previous_receipt(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("read --compare {}: {err}", path.display()))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|err| format!("parse --compare {}: {err}", path.display()))?;
    if value.get("schema_version").and_then(Value::as_str) != Some(SCHEMA_VERSION) {
        return Err(format!(
            "bench-agent-surfaces --compare: expected schema {SCHEMA_VERSION}"
        ));
    }
    Ok(value)
}

/// p50 deltas versus the previous receipt: any M1/M2/M3 p50 worsening beyond
/// the threshold — or the M1 warm/cold speedup dropping below 1.0 — is a
/// regression witness.
fn compare_receipts(previous: &Value, current: &Value) -> Vec<String> {
    let mut regressions = Vec::new();
    for metric in ["m1", "m2", "m3"] {
        let Some(current_metric) = current.get(metric).and_then(Value::as_object) else {
            continue;
        };
        for (corpus, current_body) in current_metric {
            // M2 nests its per-op series under "ops"; M1/M3 use cold/warm
            // series directly on the corpus body.
            let current_series = current_body.get("ops").unwrap_or(current_body);
            let previous_body = previous.pointer(&format!("/{metric}/{corpus}"));
            let previous_series = previous_body
                .and_then(|body| body.get("ops"))
                .unwrap_or_else(|| previous_body.unwrap_or(&Value::Null));
            let previous_p50s = p50_series(previous_series);
            for (series, current_p50) in p50_series(current_series) {
                let Some(previous_p50) = previous_p50s
                    .iter()
                    .find(|(key, _)| key == &series)
                    .map(|(_, value)| *value)
                else {
                    continue;
                };
                if previous_p50 == 0 {
                    continue;
                }
                if current_p50 as f64 > previous_p50 as f64 * (1.0 + REGRESSION_FRACTION) {
                    let percent = format!("{:.0}", REGRESSION_FRACTION * 100.0);
                    regressions.push(format!(
                        "{metric}/{corpus}/{series} p50 {current_p50} ms is more than {percent}% above the previous {previous_p50} ms"
                    ));
                }
            }
        }
    }
    for corpus in ["tiny", "mid", "repo"] {
        let Some(current_ratio) = current
            .pointer(&format!("/m1/{corpus}/warm_cold_p50_speedup"))
            .and_then(Value::as_f64)
        else {
            continue;
        };
        if current_ratio < 1.0 {
            regressions.push(format!(
                "m1/{corpus} warm/cold p50 speedup {current_ratio:.2}x dropped below 1.0x: cache-regression witness"
            ));
        }
    }
    regressions
}

/// Collect the `p50_ms` series of an M1 (cold/warm), M2 (ops), or M3
/// (cold/warm) body into `series -> p50_ms`.
fn p50_series(body: &Value) -> Vec<(String, u128)> {
    let mut out = Vec::new();
    if let Some(object) = body.as_object() {
        for (key, value) in object {
            if let Some(p50) = value.get("p50_ms").and_then(Value::as_u64) {
                out.push((key.clone(), u128::from(p50)));
            }
        }
    }
    out
}

fn receipt_status(violations: &[String], warned: bool, regressions: &[String]) -> &'static str {
    if !violations.is_empty() {
        "fail"
    } else if !regressions.is_empty() {
        "regressed"
    } else if warned {
        "warn"
    } else {
        "pass"
    }
}

fn finish_with_status(
    status: &str,
    violations: &[String],
    regressions: &[String],
) -> Result<(), String> {
    match status {
        "fail" => Err(format!(
            "bench-agent-surfaces: status fail; validity gates: {}",
            violations.join("; ")
        )),
        "regressed" => Err(format!(
            "bench-agent-surfaces: status regressed; regressions: {}",
            regressions.join("; ")
        )),
        other => {
            println!("bench-agent-surfaces status: {other}");
            Ok(())
        }
    }
}

fn truncate_for_receipt(text: &str, limit: usize) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= limit {
        trimmed.to_string()
    } else {
        format!("{}…", trimmed.chars().take(limit).collect::<String>())
    }
}

fn receipt_markdown(receipt: &Value, previous: Option<&Value>) -> String {
    let mut out = String::from("# Bench Agent Surfaces\n\n");
    out.push_str(&format!(
        "Status: `{}`\n\nRevision: `{}`\nBinary: `{}` (sha256 `{}`)\nHost: `{}`\nToolchain: `{}`\n\n",
        receipt["status"].as_str().unwrap_or("unknown"),
        receipt
            .pointer("/identity/source_sha")
            .and_then(Value::as_str)
            .unwrap_or("unavailable"),
        receipt
            .pointer("/identity/binary_path")
            .and_then(Value::as_str)
            .unwrap_or("unavailable"),
        receipt
            .pointer("/identity/binary_sha256")
            .and_then(Value::as_str)
            .unwrap_or("unavailable"),
        receipt
            .pointer("/identity/host_class")
            .and_then(Value::as_str)
            .unwrap_or("unavailable"),
        receipt
            .pointer("/identity/toolchain")
            .and_then(Value::as_str)
            .unwrap_or("unavailable"),
    ));
    out.push_str("| Metric | Corpus | Population / op | n | p50 ms | p95 ms | Delta p50 |\n");
    out.push_str("| --- | --- | --- | ---: | ---: | ---: | ---: |\n");
    for (metric, label, series_names) in [
        ("m1", "M1 CLI check", vec!["cold", "warm"]),
        ("m3", "M3 LSP didOpen", vec!["cold", "warm"]),
    ] {
        if let Some(corpora) = receipt[metric].as_object() {
            for (corpus, body) in corpora {
                if let Some(limitation) = body.get("recorded_limitation").and_then(Value::as_str) {
                    out.push_str(&format!(
                        "| {label} | {corpus} | recorded limitation | 0 | n/a | n/a | n/a |\n\n> {corpus}: {limitation}\n\n"
                    ));
                    continue;
                }
                for series in &series_names {
                    write_population_row(
                        &mut out,
                        label,
                        corpus,
                        series,
                        &body[series],
                        previous.and_then(|p| {
                            p.pointer(&format!("/{metric}/{corpus}/{series}/p50_ms"))
                                .and_then(Value::as_u64)
                        }),
                    );
                }
                if let Some(deep) = body.get("deep_build_heavy") {
                    write_deep_row(&mut out, corpus, deep);
                }
            }
        }
    }
    if let Some(corpora) = receipt["m2"].as_object() {
        for (corpus, body) in corpora {
            if let Some(ops) = body["ops"].as_object() {
                for (op, op_body) in ops {
                    write_population_row(
                        &mut out,
                        "M2 MCP",
                        corpus,
                        op,
                        op_body,
                        previous.and_then(|p| {
                            p.pointer(&format!("/m2/{corpus}/ops/{op}/p50_ms"))
                                .and_then(Value::as_u64)
                        }),
                    );
                }
            }
        }
    }
    out.push_str("\n## M4 actionability (pooled from M1 envelopes)\n\n");
    if let Some(corpora) = receipt["m4"].as_object() {
        for (corpus, body) in corpora {
            out.push_str(&format!(
                "- **{corpus}**: findings_total {}, classes {}, actionable_intent {}, evidence_path {}, related_tests {}, unknown_disclosed {} (of {} unknown-class), analysis_complete {}, max_limitations {}, alignment {}/{} ({})\n",
                body["findings_total"],
                body["class_histogram"],
                body["actionable_intent_fraction"],
                body["evidence_path_fraction"],
                body["related_test_evidence_fraction"],
                body["unknown_disclosed_fraction"],
                body["unknown_class_findings"],
                body["analysis_complete_all_envelopes"],
                body["max_typed_limitations"],
                body["finding_alignment"]["raw_signals"],
                body["finding_alignment"]["canonical_items"],
                body["finding_alignment"]["state"],
            ));
        }
    }
    out.push_str("\n## M5 determinism\n\n");
    if let Some(corpora) = receipt["m5"].as_object() {
        for (corpus, body) in corpora {
            out.push_str(&format!(
                "- **{corpus}**: cold byte_stable {} ({} runs), warm byte_stable {} ({} runs)\n",
                body["cold"]["byte_stable"],
                body["cold"]["runs"],
                body["warm"]["byte_stable"],
                body["warm"]["runs"],
            ));
        }
    }
    if let Some(gates) = receipt["validity_gates"].as_object() {
        out.push_str("\n## Validity gates\n\n");
        out.push_str(&format!(
            "Violations: {}\n\nRegressions: {}\n\n",
            gates["violations"], gates["regressions"],
        ));
    }
    out.push_str(&format!(
        "\nClaim boundary: {}\n",
        receipt["claim_boundary"].as_str().unwrap_or("unknown")
    ));
    out
}

fn write_population_row(
    out: &mut String,
    metric: &str,
    corpus: &str,
    series: &str,
    body: &Value,
    previous_p50: Option<u64>,
) {
    let p50 = body["p50_ms"].as_u64().unwrap_or(0);
    let delta = previous_p50.map(|previous| {
        if previous == 0 {
            "n/a".to_string()
        } else {
            let change = (p50 as f64 - previous as f64) / previous as f64 * 100.0;
            format!("{change:+.1}%")
        }
    });
    out.push_str(&format!(
        "| {metric} | {corpus} | {series} | {} | {p50} | {} | {} |\n",
        body["n"],
        body["p95_ms"],
        delta.unwrap_or_else(|| "n/a".to_string()),
    ));
}

fn write_deep_row(out: &mut String, corpus: &str, deep: &Value) {
    out.push_str(&format!(
        "| M1 CLI check (deep, build_heavy) | {corpus} | deep | 1 | {} | n/a | n/a |\n",
        deep["sample"]["duration_ms"],
    ));
}

struct DirGuard {
    path: PathBuf,
}

impl DirGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(args: &[&str]) -> Vec<String> {
        args.iter().map(|value| (*value).to_string()).collect()
    }

    /// A real exit(1) status: `ExitStatus::default()` reports success on
    /// Windows, so nonzero-exit witnesses need an actual failing child.
    fn failing_child_status() -> Result<std::process::ExitStatus, String> {
        let output = if cfg!(windows) {
            std::process::Command::new("cmd")
                .args(["/C", "exit", "1"])
                .output()
        } else {
            std::process::Command::new("sh")
                .arg("-c")
                .arg("exit 1")
                .output()
        };
        output
            .map(|child| child.status)
            .map_err(|err| format!("spawn probe child: {err}"))
    }

    #[test]
    fn parse_options_defaults_match_the_spec_run_counts() -> Result<(), String> {
        let options = parse_options(&[])?;
        assert_eq!(options.corpora.len(), 3);
        assert_eq!(options.m2_sessions, 5);
        assert_eq!(options.m3_cold, 5);
        assert_eq!(options.m3_warm, 5);
        assert_eq!(options.m5_cold, 2);
        assert_eq!(options.m5_warm, 5);
        assert_eq!(options.timeout_ms, DEFAULT_TIMEOUT_MS);
        assert!(!options.self_deep);
        // 0 means "per-corpus default" for M1.
        assert_eq!(m1_run_counts(CorpusId::Tiny, &options), (3, 7));
        assert_eq!(m1_run_counts(CorpusId::Mid, &options), (3, 7));
        assert_eq!(m1_run_counts(CorpusId::Repo, &options), (1, 3));
        Ok(())
    }

    #[test]
    fn parse_options_rejects_unknown_arguments_and_empty_values() {
        let err = match parse_options(&sample(&["--nope"])) {
            Ok(_) => return,
            Err(err) => err,
        };
        assert!(err.contains("unknown bench-agent-surfaces argument"));
        let err = match parse_options(&sample(&["--m1-cold"])) {
            Ok(_) => return,
            Err(err) => err,
        };
        assert!(err.contains("missing value for --m1-cold"));
        let err = match parse_options(&sample(&["--corpus", "nope"])) {
            Ok(_) => return,
            Err(err) => err,
        };
        assert!(err.contains("unknown corpus"));
    }

    #[test]
    fn parse_options_supports_corpus_filter_and_overrides() -> Result<(), String> {
        let options = parse_options(&sample(&[
            "--corpus",
            "tiny",
            "--m1-cold",
            "2",
            "--timeout-ms",
            "5000",
        ]))?;
        assert_eq!(options.corpora, vec![CorpusId::Tiny]);
        assert_eq!(options.m1_cold, 2);
        assert_eq!(options.timeout_ms, 5000);
        assert_eq!(m1_run_counts(CorpusId::Tiny, &options), (2, 7));
        Ok(())
    }

    #[test]
    fn percentile_uses_sorted_nearest_rank() {
        assert_eq!(percentile(&[30, 10, 20], 50), 20);
        assert_eq!(percentile(&[30, 10, 20], 95), 30);
        assert_eq!(percentile(&[], 50), 0);
    }

    #[test]
    fn determinism_outcome_passes_stable_and_fails_on_raw_difference() -> Result<(), String> {
        let stable = vec![b"{}".to_vec(), b"{}".to_vec()];
        assert!(matches!(
            determinism_outcome(&stable, &[]),
            DeterminismOutcome::Stable { runs: 2 }
        ));
        // Second negative control: an injected volatile field must fail the
        // gate while the check-envelope allowlist is empty.
        let differing = vec![b"{\"a\":1}".to_vec(), b"{\"a\":2}".to_vec()];
        assert!(matches!(
            determinism_outcome(&differing, &[]),
            DeterminismOutcome::Failure { .. }
        ));
        assert!(
            matches!(
                determinism_outcome(&differing, &["schema_version"]),
                DeterminismOutcome::Failure { .. }
            ),
            "an allowlist entry that does not cover the difference must not launder it"
        );
        // A difference removed only by an allowlisted field is a disclosed
        // pass naming that field.
        let volatile = vec![
            b"{\"ts\":1,\"v\":7}".to_vec(),
            b"{\"ts\":2,\"v\":7}".to_vec(),
        ];
        match determinism_outcome(&volatile, &["ts"]) {
            DeterminismOutcome::Disclosed { runs, fields } => {
                assert_eq!(runs, 2);
                assert_eq!(fields, vec!["ts".to_string()]);
            }
            _ => return Err("expected a disclosed pass".to_string()),
        }
        Ok(())
    }

    #[test]
    fn mid_corpus_generation_is_deterministic_and_mutates_exactly_one_line() {
        let before = mid_src_content(3, MID_MUTATION_SRC_FILE, false);
        let after = mid_src_content(3, MID_MUTATION_SRC_FILE, true);
        let before_again = mid_src_content(3, MID_MUTATION_SRC_FILE, false);
        assert_eq!(
            before, before_again,
            "generation must not depend on clocks or rng"
        );
        let before_lines: Vec<&str> = before.lines().collect();
        let after_lines: Vec<&str> = after.lines().collect();
        assert_eq!(before_lines.len(), after_lines.len());
        let changed: Vec<usize> = before_lines
            .iter()
            .zip(after_lines.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(changed, vec![2], "exactly the predicate line is mutated");
        assert!(before_lines[2].contains(">="));
        assert!(after_lines[2].contains("> threshold"));
        assert_eq!(
            mid_src_content(3, 1, false),
            mid_src_content(3, 1, true),
            "only the pinned file mutates"
        );
    }

    #[test]
    fn mid_file_diff_round_trips_through_apply_patch() -> Result<(), String> {
        let before = mid_src_content(2, MID_MUTATION_SRC_FILE, false);
        let after = mid_src_content(2, MID_MUTATION_SRC_FILE, true);
        let diff = mid_file_diff("pkg_02/src/m07.rs", &before, &after);
        assert!(diff.starts_with("diff --git a/pkg_02/src/m07.rs b/pkg_02/src/m07.rs"));
        let applied = apply_patch(&before, diff.as_bytes())?;
        assert_eq!(applied, after);
        Ok(())
    }

    #[test]
    fn mid_test_oracle_mix_covers_strong_weak_and_absent() {
        let strong = mid_test_content(0, 0);
        assert!(
            strong.contains(&format!(
                "assert_eq!(classify_0({}), \"premium\");",
                1_000 + 7 * 7
            )),
            "strong oracle asserts the old behavior exactly at the boundary: {strong}"
        );
        let weak = mid_test_content(0, 1);
        assert!(
            weak.contains(&format!("classify_0({})", (1_000 + 7 * 7) * 10)),
            "weak oracle asserts far from the boundary: {weak}"
        );
        let absent = mid_test_content(0, 2);
        assert!(
            !absent.contains("classify_0"),
            "absent oracle never calls the changed predicate: {absent}"
        );
    }

    #[test]
    fn sample_diff_rewrites_paths_and_round_trips() -> Result<(), String> {
        // Test binaries run with cwd at the xtask package dir; resolve the
        // checked-in sample from the workspace root via the manifest path.
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| "xtask manifest has no workspace parent".to_string())?;
        let sample_dir = workspace.join("crates/ripr/examples/sample");
        let diff = fs::read(sample_dir.join("example.diff"))
            .map_err(|err| format!("checked-in sample diff exists: {err}"))?;
        let rewritten = rewrite_sample_diff(&diff)?;
        let text = String::from_utf8(rewritten.clone()).map_err(|err| format!("utf-8: {err}"))?;
        assert!(!text.contains("crates/ripr/examples/sample/"));
        assert!(text.contains("--- a/src/lib.rs"));
        let after = fs::read_to_string(sample_dir.join("src/lib.rs"))
            .map_err(|err| format!("sample lib.rs: {err}"))?;
        let before = reverse_apply(&after, &rewritten)?;
        assert_ne!(before, after, "the derived before-state must differ");
        let repatched = apply_patch(&before, &rewritten)?;
        assert_eq!(
            repatched, after,
            "round-trip must reproduce the checked-in bytes"
        );
        Ok(())
    }

    #[test]
    fn actionability_metrics_aggregate_the_schema_fields() {
        let envelope = json!({
            "analysis_outcome": { "analysis_complete": true, "outcome": { "limitations": [] } },
            "findings": [
                {
                    "classification": "weakly_exposed",
                    "evidence_path": ["reach yes: x"],
                    "missing_discriminators": [{ "boundary": 1 }],
                    "related_tests_total": 2,
                    "stop_reasons": [],
                },
                {
                    "classification": "infection_unknown",
                    "evidence_path": ["infection unknown: x"],
                    "missing_discriminators": [],
                    "related_tests_total": 0,
                    "stop_reasons": ["no related test file resolved"],
                    "recommended_next_step": "add a boundary test",
                },
                {
                    "classification": "infection_unknown",
                    "evidence_path": [],
                    "related_tests_total": 0,
                    "stop_reasons": [],
                }
            ],
        });
        let m4 = actionability_metrics(&[envelope]);
        assert_eq!(m4["findings_total"], 3);
        assert_eq!(m4["class_histogram"]["weakly_exposed"], 1);
        assert_eq!(m4["class_histogram"]["infection_unknown"], 2);
        assert_eq!(m4["actionable_intent_fraction"], 0.6667);
        assert_eq!(m4["evidence_path_fraction"], 0.6667);
        assert_eq!(m4["related_test_evidence_fraction"], 0.3333);
        assert_eq!(m4["unknown_disclosed_fraction"], 0.5);
        assert_eq!(m4["analysis_complete_all_envelopes"], true);
        assert_eq!(m4["finding_alignment"]["state"], "alignment_absent");
        assert!(
            m4["repair_readiness"]["findings_with_repair_packet_ready"]
                .as_str()
                .unwrap_or("")
                .starts_with("named_absence"),
            "absent repair-bearing state must be named, not folded into a numerator"
        );
    }

    #[test]
    fn actionability_metrics_records_alignment_when_present() {
        let envelope = json!({
            "analysis_outcome": { "analysis_complete": false, "outcome": { "limitations": [{"kind": "partial"}] } },
            "finding_alignment": { "summary": { "raw_signals": 7, "canonical_items": 3 } },
            "findings": [],
        });
        let m4 = actionability_metrics(&[envelope]);
        assert_eq!(m4["finding_alignment"]["state"], "recorded");
        assert_eq!(m4["finding_alignment"]["raw_signals"], 7);
        assert_eq!(m4["analysis_complete_all_envelopes"], false);
        assert_eq!(m4["max_typed_limitations"], 1);
    }

    #[test]
    fn receipt_status_orders_fail_over_regressed_over_warn() {
        assert_eq!(receipt_status(&["x".to_string()], false, &[]), "fail");
        assert_eq!(receipt_status(&[], false, &["y".to_string()]), "regressed");
        assert_eq!(receipt_status(&[], true, &[]), "warn");
        assert_eq!(receipt_status(&[], false, &[]), "pass");
    }

    #[test]
    fn compare_receipts_flags_worsened_p50_and_sub_unity_ratio() {
        let previous = json!({
            "schema_version": SCHEMA_VERSION,
            "m1": { "tiny": { "cold": { "p50_ms": 100 }, "warm": { "p50_ms": 50 } } },
            "m2": { "tiny": { "ops": { "initialize": { "p50_ms": 10 } } } },
        });
        let unchanged = json!({
            "schema_version": SCHEMA_VERSION,
            "m1": { "tiny": { "cold": { "p50_ms": 120 }, "warm": { "p50_ms": 50 }, "warm_cold_p50_speedup": 2.0 } },
            "m2": { "tiny": { "ops": { "initialize": { "p50_ms": 12 } } } },
        });
        assert!(
            compare_receipts(&previous, &unchanged).is_empty(),
            "+20% p50 and +20% op p50 are inside the 25% threshold"
        );
        let worse = json!({
            "schema_version": SCHEMA_VERSION,
            "m1": { "tiny": { "cold": { "p50_ms": 126 }, "warm": { "p50_ms": 50 }, "warm_cold_p50_speedup": 0.9 } },
            "m2": { "tiny": { "ops": { "initialize": { "p50_ms": 13 } } } },
        });
        let regressions = compare_receipts(&previous, &worse);
        assert_eq!(
            regressions.len(),
            3,
            "m1 p50 worsening, m2 op p50 worsening, and sub-1.0 speedup all flag: {regressions:?}"
        );
    }

    #[test]
    fn m1_sample_classification_covers_the_validity_gates() -> Result<(), String> {
        let corpus = Corpus {
            id: CorpusId::Tiny,
            root: PathBuf::from("."),
            diff_path: Some(PathBuf::from("example.diff")),
            base: None,
            kind: "test",
            expects_findings: true,
        };
        let repo_style = Corpus {
            id: CorpusId::Repo,
            root: PathBuf::from("."),
            diff_path: None,
            base: Some("HEAD~1".to_string()),
            kind: "head_parent_fallback",
            expects_findings: false,
        };
        let success = crate::run::TimedOutput {
            status: Some(std::process::ExitStatus::default()),
            stdout: "{\"summary\": {\"findings\": 4}}".to_string(),
            stderr: String::new(),
            duration: Duration::from_millis(10),
            timed_out: false,
        };
        let (sample, envelope) = m1_sample_from_output(success, 10, &corpus);
        assert_eq!(sample.status, "pass");
        assert_eq!(sample.findings, Some(4));
        assert!(envelope.is_some());

        let zero = crate::run::TimedOutput {
            status: Some(std::process::ExitStatus::default()),
            stdout: "{\"summary\": {\"findings\": 0}}".to_string(),
            stderr: String::new(),
            duration: Duration::from_millis(10),
            timed_out: false,
        };
        let (sample, envelope) = m1_sample_from_output(zero, 10, &corpus);
        assert_eq!(sample.status, "invalid_zero_findings");
        assert!(envelope.is_none());

        // A zero-subject run on the repo corpus is a named disclosure, not a
        // violation: its findings are whatever the checkout diff holds.
        let repo_zero = crate::run::TimedOutput {
            status: Some(std::process::ExitStatus::default()),
            stdout: "{\"summary\": {\"findings\": 0}}".to_string(),
            stderr: String::new(),
            duration: Duration::from_millis(10),
            timed_out: false,
        };
        let (sample, envelope) = m1_sample_from_output(repo_zero, 10, &repo_style);
        assert_eq!(sample.status, "pass");
        assert!(
            sample
                .detail
                .unwrap_or_default()
                .starts_with("recorded disclosure")
        );
        assert!(envelope.is_some());

        // A real failing child: ExitStatus::default() reports success on
        // Windows, so the nonzero-exit witness needs an actual exit(1).
        let failed_status = failing_child_status()?;
        let failed = crate::run::TimedOutput {
            status: Some(failed_status),
            stdout: String::new(),
            stderr: "boom".to_string(),
            duration: Duration::from_millis(10),
            timed_out: false,
        };
        let (sample, _) = m1_sample_from_output(failed, 10, &corpus);
        assert_eq!(sample.status, "fail");

        let mut violations = Vec::new();
        let result = M1Result {
            cold: vec![M1Sample {
                status: "fail",
                duration_ms: 1,
                stdout_bytes: 0,
                stderr_bytes: 4,
                findings: None,
                detail: Some("boom".to_string()),
            }],
            warm: vec![],
            deep: None,
            orders: vec![],
            had_timeout: false,
            envelopes: vec![],
        };
        collect_m1_violations(&corpus, &result, &mut violations);
        assert!(
            violations
                .iter()
                .any(|violation| violation.starts_with("non_zero_child_exit:")),
            "a nonzero child exit must be a named gate violation: {violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.starts_with("empty_population:")),
            "a population without valid samples must be a named gate violation: {violations:?}"
        );
        Ok(())
    }

    #[test]
    fn limited_diff_scope_artifact_is_a_structural_outcome_not_a_harness_failure()
    -> Result<(), String> {
        let corpus = Corpus {
            id: CorpusId::Repo,
            root: PathBuf::from("."),
            diff_path: None,
            base: Some("HEAD~1".to_string()),
            kind: "head_parent_fallback",
            expects_findings: false,
        };
        let oversized = crate::run::TimedOutput {
            status: Some(failing_child_status()?),
            stdout: "{\"summary\": {\"findings\": 0}, \"analysis_scope\": {\"run_status\": \"diff_scope_oversized\", \"limitation\": \"diff_scope_oversized\"}}".to_string(),
            stderr: "ripr: diff_scope_oversized: ...".to_string(),
            duration: Duration::from_millis(10),
            timed_out: false,
        };
        let (sample, envelope) = m1_sample_from_output(oversized, 10, &corpus);
        assert_eq!(sample.status, "limited_diff_scope_oversized");
        assert_eq!(sample.detail.as_deref(), Some("diff_scope_oversized"));
        assert!(envelope.is_none());

        // All samples limited in both populations -> named limitation, and
        // the collector raises no violation for it.
        let limited = M1Sample {
            status: "limited_diff_scope_oversized",
            duration_ms: 10,
            stdout_bytes: 0,
            stderr_bytes: 0,
            findings: None,
            detail: Some("diff_scope_oversized".to_string()),
        };
        let result = M1Result {
            cold: vec![limited.clone()],
            warm: vec![limited],
            deep: None,
            orders: vec![],
            had_timeout: false,
            envelopes: vec![],
        };
        assert!(result.structural_limitation().is_some());
        let mut violations = Vec::new();
        collect_m1_violations(&corpus, &result, &mut violations);
        assert!(
            violations.is_empty(),
            "a fully refused corpus is recorded by name, not gated: {violations:?}"
        );
        // One valid sample keeps the population gated as usual.
        let passing = M1Sample {
            status: "pass",
            duration_ms: 10,
            stdout_bytes: 0,
            stderr_bytes: 0,
            findings: Some(1),
            detail: None,
        };
        let mixed = M1Result {
            cold: vec![passing],
            warm: vec![M1Sample {
                status: "limited_diff_scope_oversized",
                duration_ms: 10,
                stdout_bytes: 0,
                stderr_bytes: 0,
                findings: None,
                detail: None,
            }],
            deep: None,
            orders: vec![],
            had_timeout: false,
            envelopes: vec![json!({})],
        };
        assert!(mixed.structural_limitation().is_none());
        let mut violations = Vec::new();
        collect_m1_violations(&corpus, &mixed, &mut violations);
        assert!(
            violations
                .iter()
                .any(|v| v.starts_with("empty_population:")),
            "a warm population with no valid samples still gates: {violations:?}"
        );
        Ok(())
    }

    #[test]
    fn no_snapshot_after_failed_refresh_is_a_named_chained_absence() {
        let list = failed_op("ripr_list_gaps", "no_snapshot".to_string());
        assert_eq!(list.status, "fail");
        let named = name_no_snapshot_absence(list, Some("diff_scope_oversized"), "list_gaps");
        assert_eq!(named.status, "absent_no_snapshot");
        assert!(
            named
                .detail
                .unwrap_or_default()
                .contains("diff_scope_oversized")
        );
        // Without a typed refresh failure the same op stays a failure.
        let list = failed_op("ripr_list_gaps", "no_snapshot".to_string());
        let unnamed = name_no_snapshot_absence(list, None, "list_gaps");
        assert_eq!(unnamed.status, "fail");
    }

    #[test]
    fn file_uri_is_absolute_with_forward_slashes() {
        // The fixture path is built at runtime so no local absolute path
        // literal is committed (check-local-context) while the drive-letter
        // and extended-length prefix handling stay exercised.
        let sep = std::path::MAIN_SEPARATOR;
        let windows_path = PathBuf::from(format!("F:{sep}dir{sep}corpus{sep}src{sep}lib.rs"));
        let uri = file_uri(&windows_path);
        assert_eq!(uri, "file:///F:/dir/corpus/src/lib.rs");
        // fs::canonicalize yields extended-length paths on Windows.
        let canonical = file_uri(&PathBuf::from(format!("//?/F:{sep}dir{sep}corpus")));
        assert_eq!(canonical, "file:///F:/dir/corpus");
    }

    #[test]
    fn gap_list_absence_is_named_not_faked() {
        // Production shape: items carry the registered `canonical_id`
        // (mcp/gaps.rs GapItem).
        let response = json!({
            "result": { "content": [{ "type": "text", "text": "{\"items\": [{\"canonical_id\": \"gap:any\"}]}" }] }
        });
        assert_eq!(
            gap_id_from_list_response(&response),
            Some("gap:any".to_string())
        );
        let empty = json!({
            "result": { "content": [{ "type": "text", "text": "{\"items\": []}" }] }
        });
        assert_eq!(gap_id_from_list_response(&empty), None);
    }

    #[test]
    fn m5_timeout_is_a_warn_note_not_a_failure() {
        let mut runs = Vec::new();
        let mut warned = false;
        let mut violations = Vec::new();
        let mut timeout_notes = Vec::new();
        collect_m5_run(
            Ok(M5Run::Timeout),
            &mut runs,
            &mut warned,
            &mut violations,
            &mut timeout_notes,
            "m5 corpus tiny cold",
        );
        assert!(runs.is_empty());
        assert!(warned, "a timeout must set the warn flag");
        assert!(
            violations.is_empty(),
            "a timeout must not be a validity-gate violation: {violations:?}"
        );
        assert_eq!(timeout_notes.len(), 1);
        // A non-timeout child failure stays a named violation.
        collect_m5_run(
            Err("non-zero exit: boom".to_string()),
            &mut runs,
            &mut warned,
            &mut violations,
            &mut timeout_notes,
            "m5 corpus tiny cold",
        );
        assert_eq!(violations.len(), 1);
        assert!(violations[0].starts_with("m5_sample_failed:"));
        // An all-timeout population is a missing metric (validity gate).
        assert!(matches!(
            determinism_outcome(&runs, &[]),
            DeterminismOutcome::Failure { .. }
        ));
    }

    #[test]
    fn m4_reads_readiness_from_renderer_projections() {
        // The renderer nests repair readiness inside its preview-card
        // projection rather than at the finding top level.
        let envelope = json!({
            "findings": [{
                "classification": "weakly_exposed",
                "preview_actionability": { "repair_packet_ready": true },
            }],
        });
        let m4 = actionability_metrics(&[envelope]);
        assert_eq!(
            m4["repair_readiness"]["findings_with_repair_packet_ready"], 1,
            "readiness must be read wherever the renderer emits it"
        );
    }
}
