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
const SCHEMA_VERSION: &str = "0.2";
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
            other => {
                return Err(format!(
                    "unknown eval-sweep argument: {other}\nusage: cargo xtask eval-sweep [--manifest <path>] [--clone] [--checkout-root <dir>] [--repo <id>] [--timeout-secs <secs>] [--json-only] | cargo xtask eval-sweep check [--manifest <path>] [--runs <path>] | cargo xtask eval-sweep refresh --manifest <path> --ripr-bin <path> --out <dir> --allow-network | cargo xtask eval-sweep report (--candidate <receipt.json> [--dispositions <path>] [--accept] | --check-currentness)"
                ));
            }
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
fn classify(timed_out: bool, failed_exit: bool, parsed: Option<&Value>) -> Outcome {
    if timed_out {
        return Outcome::TimedOut;
    }
    // SPEC-0086 §Run algorithm step 3: classification uses exit code AND
    // JSON. A failure exit is a crash even when stdout carries parseable
    // JSON — without this leg the crash rate under-counts exactly the
    // failure family the Tier A sweep exists to measure (#3258).
    if failed_exit {
        return Outcome::Crash;
    }
    match parsed {
        Some(value) if findings_have_parse_failure(value) => Outcome::ParseFailure,
        Some(_) => Outcome::Ok,
        None => Outcome::Crash,
    }
}

/// Shared with the refresh route (#3566), which classifies current statuses
/// from the same producer facts.
pub(crate) fn findings_have_parse_failure(value: &Value) -> bool {
    let Some(findings) = value.get("findings").and_then(Value::as_array) else {
        return false;
    };
    findings.iter().any(|finding| {
        // The real `ripr check --json` finding key is `classification`
        // (crates/ripr/src/output/json/report.rs), not the legacy `class`.
        let class_unknown = finding
            .get("classification")
            .and_then(Value::as_str)
            .is_some_and(|class| class == "static_unknown");
        let unsupported_limit = finding
            .get("static_limit_kind")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind == "unsupported_syntax");
        class_unknown || unsupported_limit
    })
}

/// Shared with the refresh route (#3566): the same gap-identity extraction
/// feeds the candidate receipt's repeat-run comparison.
pub(crate) fn gap_ids(value: &Value) -> BTreeSet<String> {
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
// Classification + alignment distribution (descriptive; Tier-A robustness only)
// ---------------------------------------------------------------------------

/// The 7-way exposure-class distribution over a run's findings. Descriptive
/// only: it counts emitted facts and never affects `gate_status`. Shared with
/// the refresh route (#3566) so both surfaces count identically.
#[derive(Clone, Copy, Default)]
pub(crate) struct ClassificationCounts {
    exposed: usize,
    weakly_exposed: usize,
    reachable_unrevealed: usize,
    no_static_path: usize,
    infection_unknown: usize,
    propagation_unknown: usize,
    static_unknown: usize,
}

impl ClassificationCounts {
    pub(crate) fn add(&mut self, other: &ClassificationCounts) {
        self.exposed += other.exposed;
        self.weakly_exposed += other.weakly_exposed;
        self.reachable_unrevealed += other.reachable_unrevealed;
        self.no_static_path += other.no_static_path;
        self.infection_unknown += other.infection_unknown;
        self.propagation_unknown += other.propagation_unknown;
        self.static_unknown += other.static_unknown;
    }

    pub(crate) fn to_json(self) -> Value {
        json!({
            "exposed": self.exposed,
            "weakly_exposed": self.weakly_exposed,
            "reachable_unrevealed": self.reachable_unrevealed,
            "no_static_path": self.no_static_path,
            "infection_unknown": self.infection_unknown,
            "propagation_unknown": self.propagation_unknown,
            "static_unknown": self.static_unknown,
        })
    }
}

/// The Python sink-alignment distribution (RIPR-SPEC-0028 fields) plus
/// repair-packet presence counts. `absent` (field not emitted) is kept distinct
/// from `unknown` (the emitted enum value) so the distribution is not silently
/// overcounted. Descriptive only; never affects `gate_status`. Shared with the
/// refresh route (#3566).
#[derive(Clone, Copy, Default)]
pub(crate) struct AlignmentCounts {
    pub(crate) direct: usize,
    pub(crate) alias: usize,
    pub(crate) changed_sink_token: usize,
    pub(crate) orthogonal: usize,
    pub(crate) unknown: usize,
    pub(crate) absent: usize,
    pub(crate) repair_placement_present: usize,
    pub(crate) verify_command_present: usize,
    pub(crate) python_repair_card_present: usize,
}

impl AlignmentCounts {
    pub(crate) fn add(&mut self, other: &AlignmentCounts) {
        self.direct += other.direct;
        self.alias += other.alias;
        self.changed_sink_token += other.changed_sink_token;
        self.orthogonal += other.orthogonal;
        self.unknown += other.unknown;
        self.absent += other.absent;
        self.repair_placement_present += other.repair_placement_present;
        self.verify_command_present += other.verify_command_present;
        self.python_repair_card_present += other.python_repair_card_present;
    }

    pub(crate) fn to_json(self) -> Value {
        json!({
            "direct": self.direct,
            "alias": self.alias,
            "changed_sink_token": self.changed_sink_token,
            "orthogonal": self.orthogonal,
            "unknown": self.unknown,
            "absent": self.absent,
            "repair_placement_present": self.repair_placement_present,
            "verify_command_present": self.verify_command_present,
            "python_repair_card_present": self.python_repair_card_present,
        })
    }
}

/// Tally the exposure-class and sink-alignment distributions over a run's
/// findings, reusing the already-parsed check JSON (no extra invocation). Reads
/// the `"classification"` key (the real `ripr check --json` field; see
/// `crates/ripr/src/output/json/report.rs`), not the legacy `"class"`.
/// Unrecognized/missing values are dropped silently, consistent with Tier-A
/// robustness counting (no panic).
/// Shared with the refresh route (#3566): identical distribution counting.
pub(crate) fn count_distributions(value: &Value) -> (ClassificationCounts, AlignmentCounts) {
    let mut class = ClassificationCounts::default();
    let mut align = AlignmentCounts::default();
    let Some(findings) = value.get("findings").and_then(Value::as_array) else {
        return (class, align);
    };
    for finding in findings {
        match finding.get("classification").and_then(Value::as_str) {
            Some("exposed") => class.exposed += 1,
            Some("weakly_exposed") => class.weakly_exposed += 1,
            Some("reachable_unrevealed") => class.reachable_unrevealed += 1,
            Some("no_static_path") => class.no_static_path += 1,
            Some("infection_unknown") => class.infection_unknown += 1,
            Some("propagation_unknown") => class.propagation_unknown += 1,
            Some("static_unknown") => class.static_unknown += 1,
            _ => {}
        }
        match finding.get("oracle_alignment").and_then(Value::as_str) {
            Some("direct") => align.direct += 1,
            Some("alias") => align.alias += 1,
            Some("changed_sink_token") => align.changed_sink_token += 1,
            Some("orthogonal") => align.orthogonal += 1,
            Some("unknown") => align.unknown += 1,
            Some(_) => align.unknown += 1,
            None => align.absent += 1,
        }
        if finding.get("repair_placement").is_some() {
            align.repair_placement_present += 1;
        }
        if finding
            .get("repair_placement")
            .and_then(|placement| placement.get("verify_command"))
            .and_then(Value::as_str)
            .is_some_and(|command| !command.is_empty())
        {
            align.verify_command_present += 1;
        }
        if finding.get("python_repair_card").is_some() {
            align.python_repair_card_present += 1;
        }
    }
    (class, align)
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
    classification_counts: ClassificationCounts,
    alignment_counts: AlignmentCounts,
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
            classification_counts: ClassificationCounts::default(),
            alignment_counts: AlignmentCounts::default(),
        }
    }
}

fn run_repo(entry: &RepoEntry, manifest: &Manifest, args: &SweepArgs, binary: &Path) -> RepoRun {
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

    let first = run_check(binary, &checkout, &diff, args);
    let second_gap_ids = if first.outcome.counts_as_run() {
        run_check(binary, &checkout, &diff, args).gap_ids
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
        // Distribution is from run-1 only; the re-run exists solely for gap-ID
        // stability and is not counted.
        classification_counts: first.classification_counts,
        alignment_counts: first.alignment_counts,
    }
}

struct CheckRun {
    outcome: Outcome,
    gap_ids: BTreeSet<String>,
    runtime_ms: u128,
    stderr: String,
    classification_counts: ClassificationCounts,
    alignment_counts: AlignmentCounts,
}

fn run_check(binary: &Path, checkout: &Path, diff: &str, args: &SweepArgs) -> CheckRun {
    let root = checkout.to_string_lossy().to_string();
    let program = binary.to_string_lossy().to_string();
    // Invoke the pre-built `ripr` binary directly (not `cargo run`) so the
    // recorded runtime measures analysis time, not cargo's per-invocation overhead.
    let check_args: Vec<String> = vec![
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
        &program,
        &check_args,
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
                    classification_counts: ClassificationCounts::default(),
                    alignment_counts: AlignmentCounts::default(),
                };
            }
            // #3258: the exit-code leg of the SPEC-0086 run algorithm. A
            // missing status without a timeout is treated as a failure exit —
            // the conservative direction for a crash-rate metric.
            let failed_exit = out.status.is_none_or(|status| !status.success());
            let parsed = serde_json::from_str::<Value>(&out.stdout).ok();
            let outcome = classify(false, failed_exit, parsed.as_ref());
            let gaps = parsed.as_ref().map(gap_ids).unwrap_or_default();
            let (classification_counts, alignment_counts) =
                parsed.as_ref().map(count_distributions).unwrap_or_default();
            CheckRun {
                outcome,
                gap_ids: gaps,
                runtime_ms,
                stderr: out.stderr,
                classification_counts,
                alignment_counts,
            }
        }
        Err(err) => CheckRun {
            outcome: Outcome::Crash,
            gap_ids: BTreeSet::new(),
            runtime_ms: 0,
            stderr: err,
            classification_counts: ClassificationCounts::default(),
            alignment_counts: AlignmentCounts::default(),
        },
    }
}

fn ripr_binary_path() -> PathBuf {
    PathBuf::from("target")
        .join("debug")
        .join(format!("ripr{}", std::env::consts::EXE_SUFFIX))
}

fn build_ripr() -> Result<(), String> {
    run::run("cargo", &["build", "-p", "ripr", "--quiet"])
        .map(|_| ())
        .map_err(|err| format!("eval-sweep failed to build ripr before the sweep: {err}"))
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
    classification_counts: ClassificationCounts,
    alignment_counts: AlignmentCounts,
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

    // Aggregate the descriptive distributions over the analyzed (counts_as_run)
    // set only. Plain `usize +=` with no division, so an empty run set yields
    // all-zero aggregates with no guard needed. These never affect the gate.
    let mut classification_counts = ClassificationCounts::default();
    let mut alignment_counts = AlignmentCounts::default();
    for run in &run_set {
        classification_counts.add(&run.classification_counts);
        alignment_counts.add(&run.alignment_counts);
    }

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
        classification_counts,
        alignment_counts,
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
                "classification_counts": run.classification_counts.to_json(),
                "alignment_counts": run.alignment_counts.to_json(),
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
            "classification_counts": metrics.classification_counts.to_json(),
            "alignment_counts": metrics.alignment_counts.to_json(),
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

    let class = &metrics.classification_counts;
    let align = &metrics.alignment_counts;
    out.push_str("## Distribution\n\n");
    out.push_str("Descriptive only — these counts never affect the gate.\n\n");
    out.push_str(&format!(
        "- classification: {} exposed, {} weakly_exposed, {} reachable_unrevealed, {} no_static_path, {} infection_unknown, {} propagation_unknown, {} static_unknown\n",
        class.exposed,
        class.weakly_exposed,
        class.reachable_unrevealed,
        class.no_static_path,
        class.infection_unknown,
        class.propagation_unknown,
        class.static_unknown,
    ));
    out.push_str(&format!(
        "- oracle alignment: {} direct, {} alias, {} changed_sink_token, {} orthogonal, {} unknown, {} absent\n",
        align.direct,
        align.alias,
        align.changed_sink_token,
        align.orthogonal,
        align.unknown,
        align.absent,
    ));
    out.push_str(&format!(
        "- packet completeness: {} repair_placement, {} verify_command, {} python_repair_card\n\n",
        align.repair_placement_present,
        align.verify_command_present,
        align.python_repair_card_present,
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
    // `eval-sweep check` routes to the typed accepted-manifest/receipt
    // validator (RIPR-SPEC-0086, #3565); `eval-sweep refresh` routes to the
    // managed candidate refresh (#3566); `eval-sweep report` routes to the
    // accepted-receipt publisher and currentness gate (#3567); anything else
    // is the live sweep.
    if args.first().map(String::as_str) == Some("check") {
        return super::eval_sweep_check::run_check(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("refresh") {
        return super::eval_sweep_refresh::run_refresh(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("report") {
        return super::eval_sweep_report::run_report(&args[1..]);
    }
    let parsed = parse_args(args)?;
    let manifest = load_manifest(&parsed.manifest)?;

    // Build ripr once and invoke the binary directly per repo, so recorded
    // runtime measures analysis time rather than per-repo `cargo run` overhead.
    // Only build when at least one repo will actually be analyzed.
    let binary = ripr_binary_path();
    let will_run = parsed.clone
        || manifest.repos.iter().any(|entry| {
            PathBuf::from(&parsed.checkout_root)
                .join(&entry.id)
                .exists()
        });
    if will_run {
        build_ripr()?;
    }

    let mut runs = Vec::new();
    for entry in &manifest.repos {
        if let Some(only) = &parsed.only_repo
            && &entry.id != only
        {
            continue;
        }
        runs.push(run_repo(entry, &manifest, &parsed, &binary));
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

    /// The public usage string must match the routed surface: an unknown
    /// argument names every routed subcommand (`check`, `refresh`, `report`),
    /// not just the bare sweep.
    #[test]
    fn unknown_argument_usage_names_every_routed_subcommand() {
        let error = match parse_args(&["--bogus".to_string()]) {
            Ok(_) => "an unknown argument must fail with usage".to_string(),
            Err(error) => error,
        };
        for surface in [
            "eval-sweep check",
            "eval-sweep refresh",
            "eval-sweep report",
        ] {
            assert!(
                error.contains(surface),
                "usage must name the routed `{surface}` subcommand: {error}"
            );
        }
    }

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
        assert!(classify(true, false, None) == Outcome::TimedOut);
        assert!(classify(false, false, None) == Outcome::Crash);
        let ok = json!({ "findings": [{ "canonical_gap": { "id": "gap:python:a" } }] });
        assert!(classify(false, false, Some(&ok)) == Outcome::Ok);
        // Real output emits `classification`; this is the key the parser must read.
        let pf = json!({ "findings": [{ "classification": "static_unknown" }] });
        assert!(classify(false, false, Some(&pf)) == Outcome::ParseFailure);
        let pf2 = json!({ "findings": [{ "static_limit_kind": "unsupported_syntax" }] });
        assert!(classify(false, false, Some(&pf2)) == Outcome::ParseFailure);
        // The legacy `class` key must NOT be read — a finding carrying only the
        // old key is not a parse failure under the real schema.
        let legacy = json!({ "findings": [{ "class": "static_unknown" }] });
        assert!(classify(false, false, Some(&legacy)) == Outcome::Ok);
    }

    /// #3258: SPEC-0086's run algorithm classifies from exit code AND JSON.
    /// A nonzero exit after parseable, well-formed JSON is a crash — the
    /// JSON leg must not mask the process-failure leg.
    #[test]
    fn classifier_treats_failure_exit_as_crash_even_with_valid_json() {
        let ok = json!({ "findings": [{ "canonical_gap": { "id": "gap:python:a" } }] });
        assert!(classify(false, true, Some(&ok)) == Outcome::Crash);
        assert!(classify(false, true, None) == Outcome::Crash);
        // Timeout stays the first leg regardless of exit status.
        assert!(classify(true, true, Some(&ok)) == Outcome::TimedOut);
    }

    /// #3258 boundary regression: `run_check` must forward the captured
    /// process exit status into classification — a wiring regression that
    /// hardcodes the failed-exit leg again would undercount `crash_rate`
    /// while every pure `classify` test stays green. The probe binary is a
    /// tiny host script that prints one well-formed JSON object and exits 1,
    /// driven through the real capture path.
    #[test]
    fn run_check_classifies_failure_exit_with_valid_json_as_crash() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-evalsweep-exit-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|err| format!("create dir: {err}"))?;

        #[cfg(unix)]
        let (script, script_text) = (
            dir.join("fake-ripr"),
            "#!/bin/sh\nprintf '{\"findings\":[]}'\nexit 1\n".to_string(),
        );
        #[cfg(windows)]
        let (script, script_text) = (
            dir.join("fake-ripr.cmd"),
            "@echo {\"findings\":[]}\r\nexit /b 1\r\n".to_string(),
        );
        std::fs::write(&script, script_text).map_err(|err| format!("write script: {err}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .map_err(|err| format!("chmod script: {err}"))?;
        }

        let mut args = SweepArgs::defaults();
        args.timeout = Duration::from_secs(30);
        let run = run_check(&script, &dir, "unused.patch", &args);
        let _ = std::fs::remove_dir_all(&dir);

        if run.outcome != Outcome::Crash {
            return Err(format!(
                "nonzero exit with parseable JSON must classify as crash, got {} (outcome), stderr: {}",
                outcome_name(run.outcome),
                run.stderr
            ));
        }
        Ok(())
    }

    fn outcome_name(outcome: Outcome) -> &'static str {
        match outcome {
            Outcome::Ok => "ok",
            Outcome::ParseFailure => "parse_failure",
            Outcome::TimedOut => "timed_out",
            Outcome::Crash => "crash",
            Outcome::CloneFailed => "clone_failed",
            Outcome::SkippedMissingCheckout => "skipped_missing_checkout",
        }
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
            classification_counts: ClassificationCounts::default(),
            alignment_counts: AlignmentCounts::default(),
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

    /// SPEC-0086 Required Evidence: a golden of the rendered report from a
    /// fixed in-memory run vector (#3261 review). Byte-exact, not
    /// substring-pinned — any rendering change is a report-contract change
    /// and must re-bless this golden deliberately. The vector mixes one
    /// stable `ok` run with one unstable `parse_failure` run so the golden
    /// pins both outcome rows and the review-gate summary arithmetic.
    const EXPECTED_REPORT_JSON: &str = r#"{
  "kind": "python_eval_sweep_report",
  "repos": [
    {
      "alignment_counts": {
        "absent": 0,
        "alias": 0,
        "changed_sink_token": 0,
        "direct": 0,
        "orthogonal": 0,
        "python_repair_card_present": 0,
        "repair_placement_present": 0,
        "unknown": 0,
        "verify_command_present": 0
      },
      "classification_counts": {
        "exposed": 0,
        "infection_unknown": 0,
        "no_static_path": 0,
        "propagation_unknown": 0,
        "reachable_unrevealed": 0,
        "static_unknown": 0,
        "weakly_exposed": 0
      },
      "gap_ids": [],
      "gap_ids_stable": true,
      "id": "r",
      "outcome": "ok",
      "runtime_ms": 96,
      "sha": "s",
      "shape": "pytest_library",
      "stderr_excerpt": "",
      "unstable_gap_ids": []
    },
    {
      "alignment_counts": {
        "absent": 0,
        "alias": 0,
        "changed_sink_token": 0,
        "direct": 0,
        "orthogonal": 0,
        "python_repair_card_present": 0,
        "repair_placement_present": 0,
        "unknown": 0,
        "verify_command_present": 0
      },
      "classification_counts": {
        "exposed": 0,
        "infection_unknown": 0,
        "no_static_path": 0,
        "propagation_unknown": 0,
        "reachable_unrevealed": 0,
        "static_unknown": 0,
        "weakly_exposed": 0
      },
      "gap_ids": [],
      "gap_ids_stable": false,
      "id": "r",
      "outcome": "parse_failure",
      "runtime_ms": 250,
      "sha": "s",
      "shape": "pytest_library",
      "stderr_excerpt": "",
      "unstable_gap_ids": []
    }
  ],
  "schema_version": "0.2",
  "spec": "RIPR-SPEC-0086",
  "summary": {
    "alignment_counts": {
      "absent": 0,
      "alias": 0,
      "changed_sink_token": 0,
      "direct": 0,
      "orthogonal": 0,
      "python_repair_card_present": 0,
      "repair_placement_present": 0,
      "unknown": 0,
      "verify_command_present": 0
    },
    "classification_counts": {
      "exposed": 0,
      "infection_unknown": 0,
      "no_static_path": 0,
      "propagation_unknown": 0,
      "reachable_unrevealed": 0,
      "static_unknown": 0,
      "weakly_exposed": 0
    },
    "crash_count": 0,
    "crash_rate": 0.0,
    "gap_id_stability_rate": 0.5,
    "gap_id_stable_count": 1,
    "gap_id_unstable_count": 1,
    "gate_reason": "0 crash(es) and 1 unstable gap-ID set(s) over 2 repo(s); investigate before promotion",
    "gate_status": "review",
    "parse_failure_count": 1,
    "parse_failure_rate": 0.5,
    "repos_clone_failed": 0,
    "repos_run": 2,
    "repos_skipped": 0,
    "repos_total": 2,
    "runtime_ms_max": 250,
    "runtime_ms_median": 250,
    "runtime_ms_min": 96,
    "runtime_ms_total": 346,
    "timed_out_count": 0
  },
  "tier": "A"
}"#;

    const EXPECTED_REPORT_MARKDOWN: &str = r#"# RIPR Python Tier A Eval Sweep

Gate: **review**

> 0 crash(es) and 1 unstable gap-ID set(s) over 2 repo(s); investigate before promotion

## Summary

- repos: 2 total, 2 run, 0 skipped, 0 clone-failed
- crash rate: 0.000 (0 crash)
- parse-failure rate: 0.500 (1 repos)
- timed out: 0
- gap-ID stability: 0.500 (1/2 stable)
- runtime ms (min/median/max): 96/250/250

## Distribution

Descriptive only — these counts never affect the gate.

- classification: 0 exposed, 0 weakly_exposed, 0 reachable_unrevealed, 0 no_static_path, 0 infection_unknown, 0 propagation_unknown, 0 static_unknown
- oracle alignment: 0 direct, 0 alias, 0 changed_sink_token, 0 orthogonal, 0 unknown, 0 absent
- packet completeness: 0 repair_placement, 0 verify_command, 0 python_repair_card

## Repos

| id | shape | outcome | runtime_ms | gap_ids | stable |
| --- | --- | --- | ---: | ---: | --- |
| r | pytest_library | ok | 96 | 0 | yes |
| r | pytest_library | parse_failure | 250 | 0 | NO |

"#;

    #[test]
    fn rendered_report_matches_golden_from_fixed_run_vector() -> Result<(), String> {
        let runs = vec![
            run_with(Outcome::Ok, 96, true),
            run_with(Outcome::ParseFailure, 250, false),
        ];
        let metrics = compute_metrics(&runs);
        let json = render_json(&metrics, &runs)?;
        if json != EXPECTED_REPORT_JSON {
            return Err(format!(
                "rendered JSON drifted from the SPEC-0086 golden:\n{json}"
            ));
        }
        let markdown = render_markdown(&metrics, &runs);
        if markdown != EXPECTED_REPORT_MARKDOWN {
            return Err(format!(
                "rendered Markdown drifted from the SPEC-0086 golden:\n{markdown}"
            ));
        }
        Ok(())
    }

    #[test]
    fn count_distributions_tallies_classification() {
        let value = json!({ "findings": [
            { "classification": "exposed" },
            { "classification": "weakly_exposed" },
            { "classification": "weakly_exposed" },
            { "classification": "no_static_path" },
            { "classification": "static_unknown" },
        ]});
        let (class, _align) = count_distributions(&value);
        assert_eq!(class.exposed, 1);
        assert_eq!(class.weakly_exposed, 2);
        assert_eq!(class.no_static_path, 1);
        assert_eq!(class.static_unknown, 1);
        assert_eq!(class.reachable_unrevealed, 0);
    }

    #[test]
    fn count_distributions_uses_classification_key_not_class() {
        // Real `ripr check --json` emits "classification" (report.rs). The
        // legacy "class" key must NOT be counted, or the distribution silently
        // reads zero against real output.
        let real = json!({ "findings": [{ "classification": "exposed" }] });
        let legacy = json!({ "findings": [{ "class": "exposed" }] });
        assert_eq!(count_distributions(&real).0.exposed, 1);
        assert_eq!(count_distributions(&legacy).0.exposed, 0);
    }

    #[test]
    fn count_distributions_oracle_alignment_buckets() {
        let value = json!({ "findings": [
            { "classification": "exposed", "oracle_alignment": "direct" },
            { "classification": "exposed", "oracle_alignment": "alias" },
            { "classification": "exposed", "oracle_alignment": "changed_sink_token" },
            { "classification": "weakly_exposed", "oracle_alignment": "orthogonal" },
            { "classification": "weakly_exposed", "oracle_alignment": "unknown" },
            { "classification": "no_static_path" },
        ]});
        let (_class, align) = count_distributions(&value);
        assert_eq!(align.direct, 1);
        assert_eq!(align.alias, 1);
        assert_eq!(align.changed_sink_token, 1);
        assert_eq!(align.orthogonal, 1);
        assert_eq!(align.unknown, 1);
        // The finding with no oracle_alignment field is `absent`, not `unknown`.
        assert_eq!(align.absent, 1);
    }

    #[test]
    fn count_distributions_counts_packet_completeness_presence() {
        let value = json!({ "findings": [
            { "classification": "weakly_exposed",
              "repair_placement": { "verify_command": "pytest tests/x.py" },
              "python_repair_card": { "card_version": "v1" } },
            { "classification": "weakly_exposed",
              "repair_placement": { "verify_command": "" } },
            { "classification": "no_static_path" },
        ]});
        let (_class, align) = count_distributions(&value);
        assert_eq!(align.repair_placement_present, 2);
        // The empty verify_command is not counted as present.
        assert_eq!(align.verify_command_present, 1);
        assert_eq!(align.python_repair_card_present, 1);
    }

    #[test]
    fn count_distributions_empty_findings_is_all_zero() {
        let none = json!({ "summary": {} });
        let (class, align) = count_distributions(&none);
        assert_eq!(class.exposed, 0);
        assert_eq!(align.absent, 0);
    }

    fn run_with_counts(class: ClassificationCounts, align: AlignmentCounts) -> RepoRun {
        RepoRun {
            id: "r".to_string(),
            sha: "s".to_string(),
            shape: "pytest_library".to_string(),
            outcome: Outcome::Ok,
            runtime_ms: 50,
            gap_ids: BTreeSet::new(),
            gap_ids_stable: true,
            unstable_gap_ids: Vec::new(),
            stderr_excerpt: String::new(),
            classification_counts: class,
            alignment_counts: align,
        }
    }

    #[test]
    fn report_includes_distribution_and_gate_is_unaffected() {
        let class = ClassificationCounts {
            exposed: 2,
            weakly_exposed: 1,
            ..Default::default()
        };
        let align = AlignmentCounts {
            direct: 2,
            orthogonal: 1,
            ..Default::default()
        };
        let runs = vec![run_with_counts(class, align)];
        let metrics = compute_metrics(&runs);
        // Aggregated into the metrics.
        assert_eq!(metrics.classification_counts.exposed, 2);
        assert_eq!(metrics.alignment_counts.direct, 2);
        // Distributions never change the gate: one clean Ok run stays `pass`.
        assert_eq!(metrics.gate_status, "pass");
        let json = render_json(&metrics, &runs);
        assert!(json.is_ok());
        if let Ok(text) = &json {
            assert!(text.contains("\"classification_counts\""));
            assert!(text.contains("\"alignment_counts\""));
            assert!(text.contains("\"direct\": 2"));
        }
        let md = render_markdown(&metrics, &runs);
        assert!(md.contains("## Distribution"));
        assert!(md.contains("2 exposed"));
        assert!(md.contains("2 direct"));
    }

    #[test]
    fn distribution_does_not_rescue_not_run_gate() {
        // Even if a skipped repo somehow carried counts, an all-skipped sweep
        // stays not_run — distributions are informational only.
        let runs = vec![run_with(Outcome::SkippedMissingCheckout, 0, true)];
        let metrics = compute_metrics(&runs);
        assert_eq!(metrics.gate_status, "not_run");
    }
}
