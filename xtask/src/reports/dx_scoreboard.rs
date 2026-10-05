//! `cargo xtask dx-scoreboard` — developer-experience scoreboards.
//!
//! The scoreboards answer "would a developer happily reach for ripr?" with
//! numbers a developer would feel: time to the first useful result, warm
//! check latency and memory, the size and install cost of the generated CI
//! workflow, false or self-contradicting verdicts, and whether printed
//! commands survive being pasted. Each metric belongs to one board (speed,
//! ci, trust, paste, first_run); the rollup counts how many metrics meet the
//! bar declared in `benchmarks/dx_scoreboard/scoreboards.toml`.
//!
//! Two judgments stay separate:
//!
//! - **target**: the bar a developer would feel. Missing it is a gap to
//!   close, reported as `below_target`, and never fails the command.
//! - **regression**: worse than a committed baseline by more than the
//!   metric's margin. With `--gate` this exits nonzero. Wall-time and memory
//!   metrics compare only against a baseline from the same runner class.
//!
//! Metrics measured elsewhere (the hand-checked verdict corpus, the scripted
//! first-run journey) arrive through `--ingest <file>` in the
//! `ripr-dx-scoreboard-input-v1` shape. A metric with no instrument is
//! `not_measured` with its reason; absence is never reported as a pass.
//! Cloning the real-repository corpus is opt-in (`--clone`).

mod measure;

use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const SCHEMA_VERSION: &str = "ripr-dx-scoreboard-v1";
const CONFIG_SCHEMA_VERSION: &str = "ripr-dx-scoreboards-v1";
pub(crate) const INPUT_SCHEMA_VERSION: &str = "ripr-dx-scoreboard-input-v1";
/// Receipt written by `cargo xtask first-run` (the scripted new-developer
/// walk); converted on ingest so that harness needs no second output.
const FIRST_RUN_SCHEMA_VERSION: &str = "first_run.v1";
/// Receipt written by the mutation spot-check (real cargo-mutants outcomes
/// joined to ripr seams); converted on ingest.
const MUTATION_SPOT_CHECK_SCHEMA_VERSION: &str = "ripr-mutation-spot-check-v1";
/// One JSON object per line from the first-run walk's scoreboard export.
const FIRST_RUN_ROW_SCHEMA_VERSION: &str = "first_run_row.v1";
const RUST_CORPUS_SMOKE_SCHEMA_VERSION: &str = "ripr-rust-corpus-smoke-v1";
/// Receipt written by `cargo xtask pilot-ranking score` (pilot's top picks
/// judged against the checked-in mutation answer key); converted on ingest.
const PILOT_RANKING_SCHEMA_VERSION: &str = super::pilot_ranking::RECEIPT_SCHEMA_VERSION;
/// Row metrics that carry their own per-step `budget`.
const FIRST_RUN_BUDGETED: [&str; 3] = ["secs", "stdout_lines", "workflow_lines"];
const DEFAULT_CONFIG: &str = "benchmarks/dx_scoreboard/scoreboards.toml";
const DEFAULT_CORPUS_DIR: &str = "target/ripr/dx-scoreboard/corpus";
const DEFAULT_TIMEOUT_MS: u64 = 900_000;
const BOARDS: [&str; 8] = [
    "speed",
    "ci",
    "trust",
    "paste",
    "first_run",
    "agent",
    "corpus",
    "ranking",
];

const USAGE: &str = "usage: cargo xtask dx-scoreboard [--config <path>] [--boards <list>] [--repo <id>]... [--include-heavy] [--corpus-dir <dir>] [--clone] [--ripr-bin <path>] [--ingest <file>]... [--baseline <report.json>] [--gate] [--timeout-ms <n>]

Measures the developer-experience scoreboards declared in
benchmarks/dx_scoreboard/scoreboards.toml and writes
target/ripr/reports/dx-scoreboard.{json,md}.

  --boards <list>     comma-separated subset of speed,ci,trust,paste,first_run,agent,corpus,ranking
  --repo <id>         limit corpus measurements to these corpus ids
  --include-heavy     also measure corpus entries marked heavy
  --corpus-dir <dir>  where pinned corpus checkouts live
  --clone             allow cloning or fetching missing corpus pins (network)
  --ripr-bin <path>   measure this binary instead of building release ripr
  --ingest <file>     merge metrics measured by another harness
  --baseline <file>   compare against an earlier dx-scoreboard.json
  --gate              exit nonzero when a metric regresses past its margin
                      or an instrument fails";

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Config {
    schema_version: String,
    /// Optional shared corpus manifest (`benchmarks/rust_corpus/manifest.json`
    /// shape). When set it replaces the inline `[[corpus]]` list: `fast`
    /// tier entries run by default and `full` tier entries run with
    /// `--include-heavy`.
    #[serde(default)]
    corpus_manifest: Option<String>,
    #[serde(default)]
    pub(crate) corpus_version: Option<String>,
    #[serde(default)]
    pub(crate) corpus: Vec<CorpusEntry>,
    #[serde(default)]
    pub(crate) metric: Vec<MetricDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CorpusEntry {
    pub(crate) id: String,
    pub(crate) url: String,
    pub(crate) sha: String,
    /// Base for the warm check; `HEAD~1` when absent.
    #[serde(default)]
    pub(crate) base_sha: Option<String>,
    #[serde(default)]
    pub(crate) note: String,
    #[serde(default)]
    pub(crate) heavy: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MetricDef {
    pub(crate) id: String,
    pub(crate) board: String,
    pub(crate) title: String,
    pub(crate) unit: String,
    pub(crate) direction: String,
    pub(crate) target: f64,
    pub(crate) regression_pct: f64,
    pub(crate) regression_floor: f64,
    pub(crate) runner_dependent: bool,
    pub(crate) source: String,
    #[serde(default)]
    pub(crate) per_repo: bool,
    #[serde(default)]
    pub(crate) pending_reason: Option<String>,
    /// List a change in the sample details against the baseline for review
    /// without failing the gate (categorical evidence such as verdicts).
    #[serde(default)]
    pub(crate) review_on_change: bool,
    /// Allow `regression_pct` of the baseline plus `regression_floor`
    /// instead of the larger of the two (the first-run walk's own
    /// "1.5x plus 0.5 s" rule).
    #[serde(default)]
    pub(crate) regression_additive: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Options {
    config: PathBuf,
    boards: Vec<String>,
    pub(crate) repos: Vec<String>,
    pub(crate) include_heavy: bool,
    pub(crate) corpus_dir: PathBuf,
    pub(crate) clone: bool,
    pub(crate) ripr_bin: Option<PathBuf>,
    ingest: Vec<PathBuf>,
    baseline: Option<PathBuf>,
    gate: bool,
    pub(crate) timeout_ms: u64,
}

/// One observation of a metric, optionally for one corpus repository.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sample {
    pub(crate) metric: String,
    pub(crate) repo: Option<String>,
    pub(crate) outcome: SampleOutcome,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SampleOutcome {
    /// The instrument produced a number.
    Value(f64),
    /// The instrument ran but the product did not reach a useful result
    /// (for example pilot exited nonzero). The elapsed number is kept for
    /// context but the sample can never meet its target.
    Incomplete(f64),
    /// The instrument itself could not run (missing corpus, spawn failure).
    NotMeasured,
    /// The instrument broke in a way that hides the product's behavior.
    Failed,
}

pub(crate) fn dx_scoreboard(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    let config = load_config(&options.config)?;
    let mut samples = Vec::new();
    for path in &options.ingest {
        samples.extend(load_ingest(path, &config)?);
    }
    samples.extend(file_samples(&config, &options.boards)?);
    let context = measure::measure(&config, &options, &mut samples)?;
    let baseline = match &options.baseline {
        Some(path) => Some(check_baseline(read_json(path)?).map_err(|err| {
            format!(
                "{}: {err}; pass a dx-scoreboard.json report as --baseline",
                path.display()
            )
        })?),
        None => None,
    };
    let report = build_report(
        &config,
        &options.boards,
        &samples,
        &context,
        baseline.as_ref(),
        options.gate,
    );
    let json_text = serde_json::to_string_pretty(&report)
        .map_err(|err| format!("serialize dx scoreboard: {err}"))?;
    crate::write_report("dx-scoreboard.json", &format!("{json_text}\n"))?;
    let markdown = render_markdown(&report);
    crate::write_report("dx-scoreboard.md", &markdown)?;
    println!("Wrote target/ripr/reports/dx-scoreboard.json");
    println!("Wrote target/ripr/reports/dx-scoreboard.md");
    publish_to_actions(&json_text, &markdown);
    if options.gate && report["gate"]["status"].as_str() == Some("fail") {
        return Err(gate_failure_message(&report));
    }
    Ok(())
}

/// On GitHub Actions the report also goes to the run summary and, in a
/// collapsed group, to the job log. Artifacts are not reachable from every
/// reader, and a hosted-runner baseline is rebuilt from this JSON.
fn publish_to_actions(json_text: &str, markdown: &str) {
    let Some(summary) = std::env::var_os("GITHUB_STEP_SUMMARY") else {
        return;
    };
    // A summary write failure must not replace the gate decision; the
    // reports are already on disk and uploaded.
    if let Err(err) = append_step_summary(Path::new(&summary), markdown) {
        eprintln!("dx-scoreboard: step summary not written: {err}");
    }
    // Report details carry child stderr; stop-commands keeps any `##[...]`
    // text in it from being read as a workflow command.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let token = format!("dx-scoreboard-{}-{nanos}", std::process::id());
    println!("::group::dx-scoreboard.json");
    println!("::stop-commands::{token}");
    println!("{json_text}");
    println!("::{token}::");
    println!("::endgroup::");
}

pub(crate) fn append_step_summary(path: &Path, markdown: &str) -> Result<(), String> {
    use std::io::Write as _;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| format!("open step summary {}: {err}", path.display()))?;
    file.write_all(markdown.as_bytes())
        .map_err(|err| format!("write step summary {}: {err}", path.display()))
}

/// A baseline must be an earlier dx-scoreboard report; any other JSON would
/// make every metric "absent from baseline" and pass the gate.
pub(crate) fn check_baseline(value: Value) -> Result<Value, String> {
    if value["schema_version"].as_str() != Some(SCHEMA_VERSION) {
        return Err(format!(
            "baseline schema_version must be `{SCHEMA_VERSION}`"
        ));
    }
    if !value["metrics"].is_array() {
        return Err("baseline has no metrics array".to_string());
    }
    Ok(value)
}

pub(crate) fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        config: PathBuf::from(DEFAULT_CONFIG),
        boards: BOARDS.iter().map(|board| (*board).to_string()).collect(),
        repos: Vec::new(),
        include_heavy: false,
        corpus_dir: PathBuf::from(DEFAULT_CORPUS_DIR),
        clone: false,
        ripr_bin: None,
        ingest: Vec::new(),
        baseline: None,
        gate: false,
        timeout_ms: DEFAULT_TIMEOUT_MS,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |flag: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--config" => options.config = PathBuf::from(value("--config")?),
            "--boards" => {
                let list = value("--boards")?;
                let mut boards = Vec::new();
                for board in list.split(',').map(str::trim).filter(|b| !b.is_empty()) {
                    if !BOARDS.contains(&board) {
                        return Err(format!(
                            "unknown board `{board}`; expected one of {}",
                            BOARDS.join(", ")
                        ));
                    }
                    boards.push(board.to_string());
                }
                if boards.is_empty() {
                    return Err("--boards needs at least one board".to_string());
                }
                options.boards = boards;
            }
            "--repo" => options.repos.push(value("--repo")?),
            "--include-heavy" => options.include_heavy = true,
            "--corpus-dir" => options.corpus_dir = PathBuf::from(value("--corpus-dir")?),
            "--clone" => options.clone = true,
            "--ripr-bin" => options.ripr_bin = Some(PathBuf::from(value("--ripr-bin")?)),
            "--ingest" => options.ingest.push(PathBuf::from(value("--ingest")?)),
            "--baseline" => options.baseline = Some(PathBuf::from(value("--baseline")?)),
            "--gate" => options.gate = true,
            "--timeout-ms" => {
                let raw = value("--timeout-ms")?;
                options.timeout_ms =
                    raw.parse::<u64>()
                        .ok()
                        .filter(|ms| *ms > 0)
                        .ok_or_else(|| {
                            format!("--timeout-ms must be a positive integer, got `{raw}`")
                        })?;
            }
            other => return Err(format!("unknown dx-scoreboard argument: {other}\n{USAGE}")),
        }
    }
    Ok(options)
}

pub(crate) fn load_config(path: &Path) -> Result<Config, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("read dx scoreboard config {}: {err}", path.display()))?;
    let mut config = parse_config(&text).map_err(|err| format!("{}: {err}", path.display()))?;
    if let Some(manifest) = config.corpus_manifest.clone() {
        let value = read_json(Path::new(&manifest))?;
        let (version, corpus) =
            corpus_from_manifest(&value).map_err(|err| format!("{manifest}: {err}"))?;
        validate_corpus(&corpus).map_err(|err| format!("{manifest}: {err}"))?;
        config.corpus_version = Some(version);
        config.corpus = corpus;
    }
    Ok(config)
}

/// Read the shared Rust corpus manifest (`ripr_rust_corpus_manifest`).
pub(crate) fn corpus_from_manifest(value: &Value) -> Result<(String, Vec<CorpusEntry>), String> {
    if value["kind"].as_str() != Some("ripr_rust_corpus_manifest") {
        return Err("kind must be `ripr_rust_corpus_manifest`".to_string());
    }
    let version = value["corpus_version"]
        .as_str()
        .ok_or("manifest needs a corpus_version")?
        .to_string();
    let mut corpus = Vec::new();
    for repo in value["repos"]
        .as_array()
        .ok_or("manifest needs a repos array")?
    {
        let field = |key: &str| {
            repo[key]
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("manifest repo is missing `{key}`"))
        };
        corpus.push(CorpusEntry {
            id: field("id")?,
            url: field("url")?,
            sha: field("sha")?,
            base_sha: repo["base_sha"].as_str().map(str::to_string),
            note: repo["why"].as_str().unwrap_or_default().to_string(),
            heavy: repo["tier"].as_str() != Some("fast"),
        });
    }
    Ok((version, corpus))
}

/// Unique ids and full commit pins, for inline entries and manifest entries
/// alike: a branch name or short sha would let a moving upstream change the
/// measurement.
pub(crate) fn validate_corpus(corpus: &[CorpusEntry]) -> Result<(), String> {
    let mut seen = BTreeMap::new();
    let full_sha = |sha: &str| sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit());
    for entry in corpus {
        if seen.insert(entry.id.as_str(), ()).is_some() {
            return Err(format!("duplicate corpus id `{}`", entry.id));
        }
        if !full_sha(&entry.sha)
            || entry
                .base_sha
                .as_deref()
                .is_some_and(|base| !full_sha(base))
        {
            return Err(format!(
                "corpus `{}` must pin a full 40-character commit sha",
                entry.id
            ));
        }
    }
    Ok(())
}

pub(crate) fn parse_config(text: &str) -> Result<Config, String> {
    let config: Config =
        toml::from_str(text).map_err(|err| format!("parse dx scoreboard config: {err}"))?;
    if config.schema_version != CONFIG_SCHEMA_VERSION {
        return Err(format!(
            "schema_version must be `{CONFIG_SCHEMA_VERSION}`, got `{}`",
            config.schema_version
        ));
    }
    validate_corpus(&config.corpus)?;
    let mut seen = BTreeMap::new();
    for metric in &config.metric {
        if seen.insert(format!("metric:{}", metric.id), ()).is_some() {
            return Err(format!("duplicate metric id `{}`", metric.id));
        }
        if !BOARDS.contains(&metric.board.as_str()) {
            return Err(format!(
                "metric `{}` names unknown board `{}`",
                metric.id, metric.board
            ));
        }
        if !metric.id.starts_with(&format!("{}.", metric.board)) {
            return Err(format!(
                "metric `{}` must be prefixed with its board `{}.`",
                metric.id, metric.board
            ));
        }
        if metric.direction != "lower_is_better" && metric.direction != "higher_is_better" {
            return Err(format!(
                "metric `{}` direction must be lower_is_better or higher_is_better",
                metric.id
            ));
        }
        if metric.regression_pct < 0.0 || metric.regression_floor < 0.0 {
            return Err(format!(
                "metric `{}` regression margins must be >= 0",
                metric.id
            ));
        }
        let source_ok = metric.source == "measured"
            || metric.source == "pending"
            || metric.source.starts_with("ingest:")
            || metric.source.starts_with("file:");
        if !source_ok {
            return Err(format!(
                "metric `{}` has unknown source `{}`",
                metric.id, metric.source
            ));
        }
        if metric.source == "pending" && metric.pending_reason.is_none() {
            return Err(format!(
                "pending metric `{}` must say why in pending_reason",
                metric.id
            ));
        }
    }
    Ok(config)
}

/// Read a `ripr-dx-scoreboard-input-v1` file produced by another harness.
/// Every metric id must exist and declare `ingest:<source>` for the file's
/// `source`, so a harness cannot overwrite a number this command measures.
pub(crate) fn load_ingest(path: &Path, config: &Config) -> Result<Vec<Sample>, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let value = parse_ingest_text(&text).map_err(|err| format!("{}: {err}", path.display()))?;
    parse_ingest(&value, config).map_err(|err| format!("{}: {err}", path.display()))
}

/// Accept one JSON document, or JSON Lines whose every row is
/// `first_run_row.v1`; rows are wrapped as `{schema_version, rows}`.
pub(crate) fn parse_ingest_text(text: &str) -> Result<Value, String> {
    let is_row = |value: &Value| value["schema"].as_str() == Some(FIRST_RUN_ROW_SCHEMA_VERSION);
    if let Ok(value) = serde_json::from_str::<Value>(text)
        && !is_row(&value)
    {
        return Ok(value);
    }
    let mut rows = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: Value =
            serde_json::from_str(line).map_err(|err| format!("parse line {}: {err}", index + 1))?;
        if !is_row(&row) {
            return Err(format!(
                "line {} is not a `{FIRST_RUN_ROW_SCHEMA_VERSION}` row",
                index + 1
            ));
        }
        rows.push(row);
    }
    if rows.is_empty() {
        return Err("ingest file is empty".to_string());
    }
    Ok(json!({"schema_version": FIRST_RUN_ROW_SCHEMA_VERSION, "rows": rows}))
}

pub(crate) fn parse_ingest(value: &Value, config: &Config) -> Result<Vec<Sample>, String> {
    if value["schema_version"].as_str() == Some(FIRST_RUN_SCHEMA_VERSION) {
        let converted = first_run_to_input(value)?;
        return parse_ingest(&converted, config);
    }
    if value["schema_version"].as_str() == Some(MUTATION_SPOT_CHECK_SCHEMA_VERSION) {
        let converted = mutation_spot_check_to_input(value)?;
        return parse_ingest(&converted, config);
    }
    if value["schema_version"].as_str() == Some(FIRST_RUN_ROW_SCHEMA_VERSION) {
        let converted = first_run_rows_to_input(value)?;
        return parse_ingest(&converted, config);
    }
    if value["schema_version"].as_str() == Some(RUST_CORPUS_SMOKE_SCHEMA_VERSION) {
        let converted = rust_corpus_smoke_to_input(value)?;
        return parse_ingest(&converted, config);
    }
    if value["schema_version"].as_str() == Some(PILOT_RANKING_SCHEMA_VERSION) {
        let converted = pilot_ranking_to_input(value)?;
        return parse_ingest(&converted, config);
    }
    if value["schema_version"].as_str() != Some(INPUT_SCHEMA_VERSION) {
        return Err(format!(
            "schema_version must be one of `{INPUT_SCHEMA_VERSION}`, `{FIRST_RUN_SCHEMA_VERSION}`, `{MUTATION_SPOT_CHECK_SCHEMA_VERSION}`, `{RUST_CORPUS_SMOKE_SCHEMA_VERSION}`, `{PILOT_RANKING_SCHEMA_VERSION}`, or `{FIRST_RUN_ROW_SCHEMA_VERSION}` JSON Lines"
        ));
    }
    let source = value["source"]
        .as_str()
        .filter(|source| !source.is_empty())
        .ok_or("ingest file needs a non-empty `source`")?;
    let evidence_default = value["evidence"].as_str().unwrap_or_default();
    let rows = value["metrics"]
        .as_array()
        .ok_or("ingest file needs a `metrics` array")?;
    let mut samples = Vec::new();
    for row in rows {
        let id = row["id"]
            .as_str()
            .ok_or("ingest metric row needs an `id`")?;
        let def = config
            .metric
            .iter()
            .find(|metric| metric.id == id)
            .ok_or_else(|| {
                format!(
                    "ingest names unknown metric `{id}`; declare it in \
                     benchmarks/dx_scoreboard/scoreboards.toml or drop it from the receipt"
                )
            })?;
        if def.source != format!("ingest:{source}") {
            return Err(format!(
                "metric `{id}` is sourced from `{}`, not `ingest:{source}`",
                def.source
            ));
        }
        let number = row["value"]
            .as_f64()
            .filter(|number| number.is_finite())
            .ok_or_else(|| format!("ingest metric `{id}` needs a finite numeric `value`"))?;
        // Every scoreboard metric is a duration, count or rate; a negative
        // one is a broken receipt and must not read as a fast pass.
        if number < 0.0 {
            return Err(format!(
                "ingest metric `{id}` has negative value {number}; fix the harness that wrote it"
            ));
        }
        let completed = match &row["completed"] {
            Value::Null => true,
            Value::Bool(done) => *done,
            _ => {
                return Err(format!(
                    "ingest metric `{id}` `completed` must be true or false"
                ));
            }
        };
        let detail = row["evidence"]
            .as_str()
            .unwrap_or(evidence_default)
            .to_string();
        samples.push(Sample {
            metric: id.to_string(),
            repo: row["repo"].as_str().map(str::to_string),
            outcome: if completed {
                SampleOutcome::Value(number)
            } else {
                SampleOutcome::Incomplete(number)
            },
            detail: format!("ingested from {source}: {detail}"),
        });
    }
    Ok(samples)
}

/// Convert a `first_run.v1` receipt into scoreboard input rows:
///
/// - time to first useful result per case: the install step (when the walk
///   timed one) plus every step through the first `check` that exited 0.
///   A receipt without an install step leaves this metric not measured,
///   because install dominates the journey and must not silently drop out.
/// - friction events: every friction string in setup and cases.
/// - unknown verdicts: cases whose verdict is a `*_unknown` class.
pub(crate) fn first_run_to_input(value: &Value) -> Result<Value, String> {
    let setup = value["setup"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    // An empty array is the install-failed walk; a missing or non-array value
    // is a malformed receipt and must not pass as one.
    let cases = value["cases"]
        .as_array()
        .map(Vec::as_slice)
        .ok_or("first_run.v1 receipt needs a cases array")?;
    let install_steps: Vec<&Value> = setup
        .iter()
        .filter(|step| {
            step["step"]
                .as_str()
                .is_some_and(|name| name.contains("install"))
        })
        .collect();
    let install_secs: Option<f64> = install_steps
        .iter()
        .filter_map(|step| step["secs"].as_f64())
        .reduce(|a, b| a + b);
    // A walk whose install failed writes no cases but keeps the timed install
    // step; that receipt still carries the install sample.
    if cases.is_empty() && install_steps.is_empty() {
        return Err("first_run.v1 receipt needs a non-empty cases array".to_string());
    }
    // A partial sum would let a slow install look fast, so one untimed
    // install step leaves the row incomplete.
    let install_timed = install_steps.iter().all(|step| step["secs"].is_number());
    let install_failed = install_steps
        .iter()
        .find(|step| step["exit"].as_i64().is_some_and(|code| code != 0));
    let ripr = value["ripr"].as_str().unwrap_or("unknown ripr");
    let friction_in = |steps: &[Value]| -> usize {
        steps
            .iter()
            .map(|step| step["friction"].as_array().map_or(0, Vec::len))
            .sum()
    };
    let mut friction = friction_in(setup);
    let mut unknown = 0_usize;
    let mut rows = Vec::new();
    for case in cases {
        let name = case["case"].as_str().unwrap_or("case");
        let steps = case["steps"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        friction += friction_in(steps);
        if case["verdict"]
            .as_str()
            .is_some_and(|v| v.ends_with("_unknown"))
        {
            unknown += 1;
        }
        let Some(install) = install_secs else {
            continue;
        };
        let mut elapsed = install;
        let mut reached = false;
        for step in steps {
            elapsed += step["secs"].as_f64().unwrap_or(0.0);
            if step["step"].as_str() == Some("check") && step["exit"].as_i64() == Some(0) {
                reached = true;
                break;
            }
        }
        rows.push(json!({
            "id": "first_run.time_to_first_useful_result_s",
            "repo": name,
            "value": elapsed,
            "completed": reached,
        }));
    }
    // An install step with no duration at all is an incomplete sample, not an
    // absent one, so a previously timed install cannot silently drop out.
    if !install_steps.is_empty() {
        rows.push(json!({
            "id": "first_run.install_seconds",
            "value": install_secs.unwrap_or(0.0),
            "completed": install_timed && install_secs.is_some() && install_failed.is_none(),
        }));
    }
    if cases.is_empty() {
        // Install-only walk: case-dependent metrics stay absent.
        return Ok(json!({
            "schema_version": INPUT_SCHEMA_VERSION,
            "source": "first-run",
            "evidence": format!("first_run.v1 receipt for {ripr}, install only (no cases ran)"),
            "metrics": rows,
        }));
    }
    rows.push(json!({"id": "first_run.friction_events", "value": friction}));
    rows.push(json!({"id": "first_run.unknown_verdicts", "value": unknown}));
    Ok(json!({
        "schema_version": INPUT_SCHEMA_VERSION,
        "source": "first-run",
        "evidence": format!(
            "first_run.v1 receipt for {ripr}, {} case(s){}",
            cases.len(),
            if install_secs.is_some() { "" } else { "; no install step timed" }
        ),
        "metrics": rows,
    }))
}

/// Convert `first_run_row.v1` rows (keyed by case, step and metric) into
/// scoreboard rows. The walk's own gates map onto the scoreboard gate:
///
/// - failed steps: `exit` rows that are not 0, and steps with rows but no
///   `exit` row (a walk cut off mid-step);
/// - over-budget steps: `secs`, `stdout_lines` and `workflow_lines` rows over
///   the row's `budget`;
/// - walk seconds per case (setup excluded), compared per case at 50% plus
///   0.5 s, the walk's 1.5x-plus-0.5 s rule applied to each case's total
///   rather than to every step;
/// - friction events: the sum of `friction_count`, gated on any rise;
/// - unknown verdicts: `verdict` rows of an `*_unknown` class. The verdict
///   list is the sample detail, so a change is listed for review and does not
///   fail the gate;
/// - install seconds: the timed install steps in `_setup`, gated against the
///   baseline so a slower compile or download is a regression on its own;
/// - time to first useful result per case: install plus every step through
///   the first `check` that exited 0, only when the walk timed an install.
///
/// Rows fail closed: a row without case, step and metric, or a numeric metric
/// whose value is not a number, rejects the whole file rather than counting
/// as zero. A whole case missing from a truncated file is not detectable here.
pub(crate) fn first_run_rows_to_input(value: &Value) -> Result<Value, String> {
    let rows = value["rows"]
        .as_array()
        .ok_or("first_run_row.v1 input needs rows")?;
    let ripr = rows
        .first()
        .and_then(|row| row["ripr"].as_str())
        .unwrap_or("unknown ripr");

    // Step occurrences in walk order, built from `secs` and `exit` rows only:
    // summary rows (a verdict, the workflow size) may come out of order. A
    // second `secs` or `exit` for the same case and step starts a new
    // occurrence, so a failed check followed by a passing one stays separate.
    struct Step {
        case: String,
        step: String,
        secs: Option<f64>,
        exit: Option<f64>,
    }
    let mut steps: Vec<Step> = Vec::new();
    let mut over = Vec::new();
    let mut friction = 0.0;
    let mut verdicts = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let line = index + 1;
        let text = |key: &str| {
            row[key]
                .as_str()
                .filter(|v| !v.is_empty())
                .ok_or_else(|| format!("row {line}: needs a non-empty `{key}`"))
        };
        let (case, step, metric) = (text("case")?, text("step")?, text("metric")?);
        let numeric = matches!(
            metric,
            "exit" | "secs" | "stdout_lines" | "friction_count" | "workflow_lines"
        );
        let number = row["value"].as_f64().filter(|v| v.is_finite());
        if numeric && number.is_none() {
            return Err(format!(
                "row {line} ({case}/{step} {metric}): value must be a number"
            ));
        }
        if !row["budget"].is_null() && !row["budget"].is_number() {
            return Err(format!(
                "row {line} ({case}/{step} {metric}): budget must be a number or null"
            ));
        }
        let value = number.unwrap_or(0.0);
        // A negative duration or count would shorten the journey it is
        // summed into and read as a pass.
        if numeric && metric != "exit" && value < 0.0 {
            return Err(format!(
                "row {line} ({case}/{step} {metric}): value {value} is negative; fix the harness that wrote it"
            ));
        }
        if metric == "secs" || metric == "exit" {
            // Pair with the earliest run of this step still missing this
            // metric, so rows for repeated runs may arrive in any order.
            let open = steps.iter().position(|s| {
                s.case == case
                    && s.step == step
                    && if metric == "secs" {
                        s.secs.is_none()
                    } else {
                        s.exit.is_none()
                    }
            });
            let index = open.unwrap_or_else(|| {
                steps.push(Step {
                    case: case.to_string(),
                    step: step.to_string(),
                    secs: None,
                    exit: None,
                });
                steps.len() - 1
            });
            if let Some(current) = steps.get_mut(index) {
                if metric == "secs" {
                    current.secs = Some(value);
                } else {
                    current.exit = Some(value);
                }
            }
        }
        match metric {
            "friction_count" => friction += value,
            "secs" | "exit" => {}
            "verdict" => {
                let class = row["value"].as_str().ok_or_else(|| {
                    format!("row {line} ({case}/{step} verdict): value must be a class name")
                })?;
                verdicts.push(format!("{case}={class}"));
            }
            _ => {}
        }
        if FIRST_RUN_BUDGETED.contains(&metric)
            && let Some(budget) = row["budget"].as_f64()
            && value > budget
        {
            over.push(format!("{case}/{step} {metric} {value} > {budget}"));
        }
    }
    let cases: Vec<String> =
        steps
            .iter()
            .filter(|s| s.case != "_setup")
            .fold(Vec::new(), |mut cases, s| {
                if !cases.contains(&s.case) {
                    cases.push(s.case.clone());
                }
                cases
            });
    let failed: Vec<String> = steps
        .iter()
        .filter_map(|s| match s.exit {
            Some(code) => (code != 0.0).then(|| format!("{}/{} exit {code}", s.case, s.step)),
            None => Some(format!("{}/{} no exit recorded", s.case, s.step)),
        })
        .collect();
    let install: Option<f64> = steps
        .iter()
        .filter(|s| s.case == "_setup" && s.step.contains("install"))
        .map(|s| s.secs.unwrap_or(0.0))
        .reduce(|a, b| a + b);
    let install_timed = steps
        .iter()
        .filter(|s| s.case == "_setup" && s.step.contains("install"))
        .all(|s| s.secs.is_some());
    let install_ok = steps
        .iter()
        .filter(|s| s.case == "_setup" && s.step.contains("install"))
        .all(|s| s.exit == Some(0.0));
    // A walk whose install failed writes no case rows; its install timing and
    // failed step are still worth ingesting.
    if cases.is_empty() && install.is_none() {
        return Err("first_run_row.v1 input has no case rows outside _setup".to_string());
    }
    let list = |items: &[String]| {
        if items.is_empty() {
            "none".to_string()
        } else {
            items.join(", ")
        }
    };
    let unknown = verdicts.iter().filter(|v| v.ends_with("_unknown")).count();
    let mut out = vec![
        json!({"id": "first_run.failed_steps", "value": failed.len(), "evidence": list(&failed)}),
        json!({"id": "first_run.over_budget_steps", "value": over.len(), "evidence": list(&over)}),
    ];
    // An install-only walk ran no cases, so a zero here would read as "no
    // friction, no unknown verdicts" rather than "not measured".
    if !cases.is_empty() {
        out.push(json!({"id": "first_run.friction_events", "value": friction}));
        out.push(json!({"id": "first_run.unknown_verdicts", "value": unknown, "evidence": format!("verdicts: {}", list(&verdicts))}));
    }
    if let Some(install) = install {
        // The install alone, so a slower compile is visible even when the
        // walk after it stays fast.
        out.push(json!({
            "id": "first_run.install_seconds",
            "value": install,
            "completed": install_timed && install_ok,
        }));
    }
    for case in &cases {
        let mine: Vec<&Step> = steps.iter().filter(|s| &s.case == case).collect();
        // A step without a `secs` row has no duration; summing it as zero
        // would make the walk look faster, so the sample stays incomplete.
        let untimed: Vec<&str> = mine
            .iter()
            .filter(|s| s.secs.is_none())
            .map(|s| s.step.as_str())
            .collect();
        let walk: f64 = mine.iter().filter_map(|s| s.secs).sum();
        let mut row = json!({"id": "first_run.walk_secs", "repo": case, "value": walk});
        if !untimed.is_empty() {
            row["completed"] = json!(false);
            row["evidence"] = json!(format!("no secs row for: {}", untimed.join(", ")));
        }
        out.push(row);
        if let Some(install) = install {
            let mut elapsed = install;
            let mut reached = false;
            let mut timed = install_timed;
            for step in &mine {
                timed &= step.secs.is_some();
                elapsed += step.secs.unwrap_or(0.0);
                if step.step == "check" && step.exit == Some(0.0) {
                    reached = true;
                    break;
                }
            }
            out.push(json!({
                "id": "first_run.time_to_first_useful_result_s",
                "repo": case,
                "value": elapsed,
                "completed": reached && timed,
            }));
        }
    }
    Ok(json!({
        "schema_version": INPUT_SCHEMA_VERSION,
        "source": "first-run",
        "evidence": format!(
            "first_run_row.v1 rows for {ripr}, {} case(s){}",
            cases.len(),
            if install.is_some() { "" } else { "; no install step timed" }
        ),
        "metrics": out,
    }))
}

/// Convert a `ripr-rust-corpus-smoke-v1` receipt (`cargo xtask rust-corpus
/// smoke`) into per-repository scoreboard rows:
///
/// - `corpus.not_analyzed`: 0 when the diff-scoped check reached `analyzed`,
///   1 when it failed closed, timed out or broke. A repository that flips from
///   0 to 1 against the baseline is a per-repository regression.
/// - `corpus.check_ms`: wall time of an analyzed run. A run that did not
///   analyze has no comparable time, so its sample is incomplete.
///
/// A repository the smoke could not run (`not_fetched`, `spawn_failed`) is an
/// instrument gap, not a verdict: every row for it is incomplete, which the
/// gate treats as lost completion against a baseline that analyzed it.
pub(crate) fn rust_corpus_smoke_to_input(value: &Value) -> Result<Value, String> {
    let repos = value["repos"]
        .as_array()
        .filter(|repos| !repos.is_empty())
        .ok_or("rust-corpus smoke receipt needs a non-empty repos array")?;
    let corpus_version = value["corpus_version"].as_str().unwrap_or("unknown");
    let tier = value["tier"].as_str().unwrap_or("unknown");
    let mut rows = Vec::new();
    for (index, repo) in repos.iter().enumerate() {
        let id = repo["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| format!("rust-corpus smoke repo {} needs an id", index + 1))?;
        let status = repo["status"]
            .as_str()
            .ok_or_else(|| format!("rust-corpus smoke repo `{id}` needs a status"))?;
        let duration = repo["duration_ms"]
            .as_f64()
            .filter(|ms| ms.is_finite() && *ms >= 0.0);
        if status == "analyzed" && duration.is_none() {
            return Err(format!(
                "rust-corpus smoke repo `{id}` is analyzed but has no valid duration_ms; rerun `cargo xtask rust-corpus smoke`"
            ));
        }
        let ms = duration.unwrap_or(0.0);
        let row = |metric: &str, number: f64, completed: bool, evidence: String| {
            json!({
                "id": metric,
                "repo": id,
                "value": number,
                "completed": completed,
                "evidence": evidence,
            })
        };
        if matches!(status, "not_fetched" | "spawn_failed") {
            let reason = repo["reason"].as_str().unwrap_or(status);
            let evidence = format!("{status}: {reason}");
            rows.push(row("corpus.not_analyzed", 1.0, false, evidence.clone()));
            rows.push(row("corpus.check_ms", 0.0, false, evidence));
            continue;
        }
        let analyzed = status == "analyzed";
        rows.push(row(
            "corpus.not_analyzed",
            if analyzed { 0.0 } else { 1.0 },
            true,
            status.to_string(),
        ));
        // Only an analyzed run's time is comparable; a fail-closed run's time
        // stays in the evidence so it cannot regress (or mask) the aggregate.
        rows.push(row(
            "corpus.check_ms",
            if analyzed { ms } else { 0.0 },
            analyzed,
            if analyzed {
                format!("`ripr check --base <pinned base>` in {ms:.0} ms")
            } else {
                format!("{status} after {ms:.0} ms; no analyzed run to time")
            },
        ));
    }
    Ok(json!({
        "schema_version": INPUT_SCHEMA_VERSION,
        "source": "rust-corpus-smoke",
        "evidence": format!(
            "ripr-rust-corpus-smoke-v1 receipt, corpus {corpus_version}, tier {tier}, {} repositories",
            repos.len()
        ),
        "metrics": rows,
    }))
}

/// Convert a `ripr-pilot-ranking-v1` receipt (`cargo xtask pilot-ranking
/// score`) into pooled `ranking` rows: precision of pilot's top 5 and top 10,
/// the share of top-10 picks a label could judge, the share of top-10 picks
/// whose function no higher pick named, and the number of top-10 picks.
///
/// Scored share sits beside precision because precision can rise by making
/// picks unjudgeable, and the pick count because it can rise by ranking
/// fewer seams. A receipt that lost or skipped a repository measured a
/// different population than its baseline, so every row is incomplete (lost
/// completion to the gate) rather than a rate compared as like for like.
pub(crate) fn pilot_ranking_to_input(value: &Value) -> Result<Value, String> {
    let corpus = value["corpus_version"].as_str().unwrap_or("unknown");
    let unavailable = value["unavailable_repos"]
        .as_u64()
        .ok_or("pilot-ranking receipt needs unavailable_repos as a non-negative integer")?;
    let total = value["repos_total"]
        .as_u64()
        .filter(|total| *total > 0 && unavailable <= *total)
        .ok_or(
            "pilot-ranking receipt needs a positive repos_total no smaller than unavailable_repos",
        )?;
    let complete = unavailable == 0 && value["status"].as_str() == Some("complete");
    let mut rows = Vec::new();
    for (metric, cut, field) in [
        ("ranking.pilot_precision_top5", "top5", "precision"),
        ("ranking.pilot_precision_top10", "top10", "precision"),
        ("ranking.pilot_scored_share_top10", "top10", "scored_share"),
        (
            "ranking.pilot_distinct_function_share_top10",
            "top10",
            "distinct_function_share",
        ),
        ("ranking.pilot_picks_top10", "top10", "picks"),
        ("ranking.pilot_confirmed_top5", "top5", "confirmed"),
        ("ranking.pilot_refuted_top5", "top5", "refuted"),
        ("ranking.pilot_confirmed_top10", "top10", "confirmed"),
        ("ranking.pilot_refuted_top10", "top10", "refuted"),
    ] {
        let entry = &value["pooled"][cut];
        let count = |key: &str| {
            entry[key]
                .as_u64()
                .ok_or_else(|| format!("pilot-ranking pooled.{cut} needs {key}"))
        };
        let (picks, confirmed, refuted) = (count("picks")?, count("confirmed")?, count("refuted")?);
        let distinct = count("distinct_functions")?;
        // How much of this cut's precision rests on the coarse line and owner
        // tiers. The split must account for exactly the cut's judged picks.
        let tiers = match entry.get("by_tier") {
            None => String::new(),
            Some(by_tier) => {
                let mut judged = 0;
                let mut parts = Vec::new();
                for tier in ["seam", "line", "owner"] {
                    let tally = |verdict: &str| by_tier[tier][verdict].as_u64().unwrap_or(0);
                    let (tier_confirmed, tier_judged) =
                        (tally("confirmed"), tally("confirmed") + tally("refuted"));
                    judged += tier_judged;
                    parts.push(format!("{tier} {tier_confirmed}/{tier_judged}"));
                }
                if judged != confirmed + refuted {
                    return Err(format!(
                        "pilot-ranking pooled.{cut}.by_tier judges {judged} picks, not the cut's {}",
                        confirmed + refuted
                    ));
                }
                format!(" (by tier: {})", parts.join(", "))
            }
        };
        if confirmed + refuted > picks || distinct > picks {
            return Err(format!(
                "pilot-ranking pooled.{cut} counts more confirmed, refuted or distinct picks than its {picks} picks"
            ));
        }
        // The rate is derived from the validated counts, so a receipt whose
        // stated ratio disagrees with its own counts cannot pass the gate.
        let ratio = |numerator: u64, denominator: u64| {
            (denominator > 0).then(|| numerator as f64 / denominator as f64)
        };
        let counted = matches!(field, "picks" | "confirmed" | "refuted");
        let rate = match field {
            "picks" => Some(picks as f64),
            "confirmed" => Some(confirmed as f64),
            "refuted" => Some(refuted as f64),
            "precision" => ratio(confirmed, confirmed + refuted),
            "scored_share" => ratio(confirmed + refuted, picks),
            _ => ratio(distinct, picks),
        };
        if !counted {
            let stated = entry[field].as_f64();
            let agrees = match (stated, rate) {
                (None, None) => entry[field].is_null(),
                (Some(stated), Some(rate)) => (stated - rate).abs() < 1e-9,
                _ => false,
            };
            if !agrees {
                return Err(format!(
                    "pilot-ranking pooled.{cut}.{field} is {}, but its counts give {}",
                    entry[field],
                    rate.map_or_else(|| "null".to_string(), |rate| rate.to_string())
                ));
            }
        }
        let mut evidence = if field == "distinct_function_share" {
            format!("{distinct} distinct functions in {picks} picks")
        } else {
            format!("{confirmed} confirmed, {refuted} refuted of {picks} picks{tiers}")
        };
        evidence.push_str(&format!(
            " over {} of {total} repositories, corpus {corpus}",
            total - unavailable
        ));
        if !complete {
            evidence.push_str(&format!(
                "; {unavailable} repositories unavailable or not selected, so this run is not comparable"
            ));
        }
        // The corpus identity is the row's repository, so a baseline taken
        // on another answer key shares no repository with this run and is
        // reported uncompared instead of compared as the same population.
        rows.push(json!({
            "id": metric,
            "repo": format!("pilot-ranking corpus {corpus}"),
            "value": rate.unwrap_or(0.0),
            "completed": complete && rate.is_some(),
            "evidence": evidence,
        }));
    }
    Ok(json!({
        "schema_version": INPUT_SCHEMA_VERSION,
        "source": "pilot-ranking",
        "evidence": format!("ripr-pilot-ranking-v1 receipt, corpus {corpus}, {total} repositories"),
        "metrics": rows,
    }))
}

/// The repositories a pilot precision covers, so a row measured over a
/// different population than its baseline says so in its evidence.
fn pilot_repos(pilot: &Value) -> String {
    let names = pilot["repos"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|repo| repo["name"].as_str())
        .collect::<Vec<_>>();
    if names.is_empty() {
        "unrecorded repositories".to_string()
    } else {
        names.join(", ")
    }
}

/// Scored pilot recommendations per judge tier, so the scoreboard row shows
/// how much of its precision rests on the coarse `line` and `owner` tiers.
fn pilot_tier_split(pilot: &Value) -> String {
    ["seam", "line", "owner"]
        .iter()
        .map(|tier| {
            let count = |verdict: &str| {
                pilot
                    .pointer(&format!("/by_tier/{tier}/{verdict}"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
            };
            format!(
                "{tier} {}/{}",
                count("confirmed"),
                count("confirmed") + count("refuted")
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Convert a `ripr-mutation-spot-check-v1` receipt into scoreboard rows:
///
/// - discriminator claim agreement: when ripr says a test discriminates the
///   seam, the share of seam-precise mutants a real run caught;
/// - gap claim agreement: when ripr says no test discriminates, the share of
///   seam-precise mutants a real run missed (the rest are false gaps);
/// - join coverage: seam-precise joins over all mutants, because agreement
///   rates only speak for the mutants that could be joined to a seam;
/// - pilot top-recommendation precision: of `ripr pilot`'s top seams that a
///   real mutant scores, the share where a mutant was missed (receipts written
///   before the pilot section existed simply omit the row).
pub(crate) fn mutation_spot_check_to_input(value: &Value) -> Result<Value, String> {
    let families = value["scored_families"]
        .as_object()
        .ok_or("mutation spot-check receipt needs a scored_families object")?;
    let mut rows = Vec::new();
    for (family, metric) in [
        (
            "claims_discriminator",
            "trust.discriminator_claim_agreement",
        ),
        ("claims_no_discriminator", "trust.gap_claim_agreement"),
    ] {
        let entry = families
            .get(family)
            .ok_or_else(|| format!("mutation spot-check needs scored family `{family}`"))?;
        let scored = entry["mutants_scored"]
            .as_u64()
            .ok_or_else(|| format!("mutation spot-check `{family}` needs mutants_scored"))?;
        if scored == 0 {
            continue;
        }
        let rate = entry["agreement_rate"]
            .as_f64()
            .filter(|rate| (0.0..=1.0).contains(rate))
            .ok_or_else(|| {
                format!("mutation spot-check `{family}` needs agreement_rate between 0 and 1")
            })?;
        rows.push(json!({
            "id": metric,
            "value": rate,
            "evidence": format!("{scored} seam-precise mutants scored"),
        }));
    }
    let repos = value["repos"]
        .as_array()
        .filter(|repos| !repos.is_empty())
        .ok_or("mutation spot-check receipt needs a non-empty repos array")?;
    let (mut joined, mut mutants) = (0_u64, 0_u64);
    for (index, repo) in repos.iter().enumerate() {
        let precise = repo["pairings"]["seam_precise"].as_u64();
        let total = repo["calibration_metrics"]["mutants_total"].as_u64();
        let (Some(precise), Some(total)) = (precise, total) else {
            return Err(format!(
                "mutation spot-check repo {} needs pairings.seam_precise and calibration_metrics.mutants_total",
                index + 1
            ));
        };
        if precise > total {
            return Err(format!(
                "mutation spot-check repo {} joins {precise} of only {total} mutants",
                index + 1
            ));
        }
        joined += precise;
        mutants += total;
    }
    // Older receipts carry no pilot section; a present one must say how many
    // recommendations it scored, so a malformed receipt fails instead of
    // reading as not measured.
    let pilot = &value["pilot_top_recommendations"];
    let scored = if pilot.is_null() {
        0
    } else {
        pilot["scored"].as_u64().ok_or(
            "mutation spot-check pilot_top_recommendations needs scored as a non-negative integer",
        )?
    };
    // A run that lost a repository's pilot ranking measured a different
    // population than the baseline, so it publishes no pilot row rather
    // than a precision the regression gate would compare as like for like.
    let unavailable = match &pilot["unavailable_repos"] {
        Value::Null => 0,
        count => count.as_u64().ok_or(
            "mutation spot-check pilot_top_recommendations needs unavailable_repos as a non-negative integer",
        )?,
    };
    if scored > 0 && unavailable == 0 {
        let precision = pilot["precision"]
            .as_f64()
            .filter(|rate| (0.0..=1.0).contains(rate))
            .ok_or(
                "mutation spot-check pilot_top_recommendations needs precision between 0 and 1",
            )?;
        rows.push(json!({
            "id": "trust.pilot_top_recommendation_precision",
            "value": precision,
            "evidence": format!(
                "{scored} pilot recommendations scored ({}) over {}",
                pilot_tier_split(pilot),
                pilot_repos(pilot)
            ),
        }));
    }
    if mutants > 0 {
        rows.push(json!({
            "id": "trust.mutation_join_coverage",
            "value": joined as f64 / mutants as f64,
            "evidence": format!("{joined} of {mutants} mutants joined seam-precise"),
        }));
    }
    // Rates pool every repository, and a repository run with extra
    // cargo-mutants arguments is not a default run, so every row says how
    // many were: ingest publishes a row's own evidence, not the top-level one.
    let with_args = repos
        .iter()
        .filter(|repo| {
            repo["cargo_mutants_args"]
                .as_array()
                .is_some_and(|args| !args.is_empty())
        })
        .count();
    let caveat = if with_args > 0 {
        format!(
            " ({with_args} of {} repositories ran with cargo-mutants arguments; their rates reflect those arguments, not a default run)",
            repos.len()
        )
    } else {
        String::new()
    };
    for row in &mut rows {
        if let Some(Value::String(evidence)) = row.get_mut("evidence") {
            evidence.push_str(&caveat);
        }
    }
    let evidence = format!(
        "ripr-mutation-spot-check-v1 receipt, {} repositories{caveat}",
        repos.len()
    );
    Ok(json!({
        "schema_version": INPUT_SCHEMA_VERSION,
        "source": "mutation-spot-check",
        "evidence": evidence,
        "metrics": rows,
    }))
}

/// Metrics read from committed receipts (`file:<path>#<json.path>`). The
/// referenced object uses the judged-panel `{numerator, denominator}` shape;
/// a zero denominator is `not_measured`, not a perfect rate.
fn file_samples(config: &Config, boards: &[String]) -> Result<Vec<Sample>, String> {
    let mut samples = Vec::new();
    for metric in &config.metric {
        if !boards.contains(&metric.board) {
            continue;
        }
        let Some(reference) = metric.source.strip_prefix("file:") else {
            continue;
        };
        let (path, pointer) = reference.split_once('#').unwrap_or((reference, ""));
        let sample = match fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(json) => ratio_sample(&metric.id, path, pointer, &json),
                Err(err) => Sample {
                    metric: metric.id.clone(),
                    repo: None,
                    outcome: SampleOutcome::Failed,
                    detail: format!("{path} is not JSON: {err}"),
                },
            },
            Err(err) => Sample {
                metric: metric.id.clone(),
                repo: None,
                outcome: SampleOutcome::NotMeasured,
                detail: match &metric.pending_reason {
                    Some(reason) => format!("{path} unreadable: {err}; {reason}"),
                    None => format!("{path} unreadable: {err}"),
                },
            },
        };
        samples.push(sample);
    }
    Ok(samples)
}

pub(crate) fn ratio_sample(metric: &str, path: &str, pointer: &str, json: &Value) -> Sample {
    let mut node = json;
    for part in pointer.split('.').filter(|part| !part.is_empty()) {
        node = &node[part];
    }
    let numerator = node["numerator"].as_f64();
    let denominator = node["denominator"].as_f64();
    let (outcome, detail) = match (numerator, denominator) {
        (Some(n), Some(d)) if d > 0.0 => (
            SampleOutcome::Value(n / d),
            format!("{path} {pointer}: {n}/{d}"),
        ),
        (Some(_), Some(_)) => (
            SampleOutcome::NotMeasured,
            format!("{path} {pointer}: denominator is 0, no eligible cases yet"),
        ),
        _ => (
            SampleOutcome::Failed,
            format!("{path} {pointer}: missing numerator/denominator"),
        ),
    };
    Sample {
        metric: metric.to_string(),
        repo: None,
        outcome,
        detail,
    }
}

/// Host facts the report records next to its numbers.
#[derive(Debug, Clone, Default)]
pub(crate) struct RunContext {
    pub(crate) revision: String,
    pub(crate) analyzer_version: String,
    pub(crate) runner_class: String,
    pub(crate) binary: String,
    pub(crate) corpus: Vec<Value>,
}

pub(crate) fn build_report(
    config: &Config,
    boards: &[String],
    samples: &[Sample],
    context: &RunContext,
    baseline: Option<&Value>,
    gate: bool,
) -> Value {
    let mut metrics = Vec::new();
    for def in config
        .metric
        .iter()
        .filter(|metric| boards.contains(&metric.board))
    {
        let mine: Vec<&Sample> = samples.iter().filter(|s| s.metric == def.id).collect();
        let mut row = metric_row(def, &mine);
        row["baseline"] = compare_with_baseline(def, &row, context, baseline);
        if def.review_on_change {
            // Categorical evidence is judged by a person, so a worse count
            // is listed with the change and never trips the gate.
            if row["baseline"]["regressed"].as_bool() == Some(true) {
                row["baseline"]["regressed"] = json!(false);
                row["baseline"]["worse"] = json!(true);
            }
            if let Some(change) = review_change(&row, baseline) {
                row["baseline"]["review"] = change;
            }
        }
        metrics.push(row);
    }

    let mut board_rows = Vec::new();
    for board in boards {
        let rows: Vec<&Value> = metrics
            .iter()
            .filter(|row| row["board"].as_str() == Some(board.as_str()))
            .collect();
        let counts = status_counts(&rows);
        board_rows.push(json!({
            "id": board,
            "status": board_status(&counts),
            "counts": counts,
            "metrics": rows.iter().map(|row| row["id"].clone()).collect::<Vec<_>>(),
        }));
    }
    let all: Vec<&Value> = metrics.iter().collect();
    let rollup = status_counts(&all);
    let repos = repo_view(config, &metrics);

    let regressions: Vec<Value> = metrics
        .iter()
        .filter(|row| row["baseline"]["regressed"].as_bool() == Some(true))
        .map(|row| {
            json!({
                "metric": row["id"],
                "baseline": row["baseline"]["value"],
                // The compared value: the baseline plus the delta over the
                // repositories both runs measured, not the overall worst,
                // which may come from a repository new to this run.
                "current": match (
                    row["baseline"]["value"].as_f64(),
                    row["baseline"]["delta"].as_f64(),
                ) {
                    (Some(base), Some(delta)) => json!(round(base + delta)),
                    _ => row["value"].clone(),
                },
                "allowed_worsening": row["baseline"]["allowed_worsening"],
                "reason": row["baseline"]["reason"],
                "regressed_repos": row["baseline"]["regressed_repos"],
            })
        })
        .collect();
    // Metrics the baseline measured that this run could not compare, such as
    // ingested metrics without a receipt: listed so the gate's reach is
    // visible instead of silently passing them.
    let uncompared: Vec<Value> = metrics
        .iter()
        .filter(|_| baseline.is_some())
        .filter(|row| {
            row["baseline"]["comparable"] == false
                && row["baseline"]["review"].is_null()
                && (!row["baseline"]["value"].is_null() || !row["value"].is_null())
        })
        .map(|row| json!({"metric": row["id"], "reason": row["baseline"]["reason"]}))
        .chain(metrics.iter().filter_map(|row| {
            let missing: Vec<&str> = row["baseline"]["missing_repos"]
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .collect();
            (!missing.is_empty()).then(|| {
                json!({
                    "metric": row["id"],
                    "reason": format!(
                        "baseline repositories not measured this run: {}",
                        missing.join(", ")
                    ),
                })
            })
        }))
        .collect();
    let review: Vec<Value> = metrics
        .iter()
        .filter(|row| !row["baseline"]["review"].is_null())
        .map(|row| json!({"metric": row["id"], "change": row["baseline"]["review"]}))
        .collect();
    let failed: Vec<Value> = metrics
        .iter()
        .filter(|row| row["status"].as_str() == Some("failed"))
        .map(|row| row["id"].clone())
        .collect();
    let gate_status = if !gate {
        "not_run"
    } else if baseline.is_none() {
        if failed.is_empty() {
            "no_baseline"
        } else {
            "fail"
        }
    } else if regressions.is_empty() && failed.is_empty() {
        "pass"
    } else {
        "fail"
    };

    json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "report": "dx-scoreboard",
        "revision": context.revision,
        "analyzer_version": context.analyzer_version,
        "runner_class": context.runner_class,
        "binary": context.binary,
        "corpus_version": config.corpus_version,
        "corpus": context.corpus,
        "rollup": rollup,
        "boards": board_rows,
        "repos": repos,
        "metrics": metrics,
        "gate": {
            "enabled": gate,
            "status": gate_status,
            "baseline_revision": baseline.map(|b| b["revision"].clone()).unwrap_or(Value::Null),
            "baseline_runner_class": baseline.map(|b| b["runner_class"].clone()).unwrap_or(Value::Null),
            "regressions": regressions,
            "failed_instruments": failed,
            "review": review,
            "uncompared": uncompared,
        },
        "claim_boundary": "Numbers hold for the recorded revision, binary, runner class and pinned corpus only. Targets are proposed bars, not product guarantees. Wall-time and memory metrics are compared only against a baseline from the same runner class; peak memory is sampled from /proc every 10 ms on Linux and is a lower bound. Static verdict metrics do not claim runtime mutation outcomes.",
    })
}

/// For a `review_on_change` metric, the sample details and values now and in
/// the baseline when either differs. Listed for a person to judge; never a
/// failure.
fn review_change(row: &Value, baseline: Option<&Value>) -> Option<Value> {
    let base_row = baseline?["metrics"]
        .as_array()?
        .iter()
        .find(|r| r["id"] == row["id"])?;
    let details = |r: &Value| -> Vec<String> {
        r["samples"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|s| s["detail"].as_str().unwrap_or("").to_string())
            .collect()
    };
    // Values too: a count can move while the evidence text stays the same.
    let values = |r: &Value| -> Vec<Value> {
        r["samples"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|s| s["value"].clone())
            .collect()
    };
    let (before, after) = (details(base_row), details(row));
    let (before_values, after_values) = (values(base_row), values(row));
    (before != after || before_values != after_values).then(|| {
        json!({
            "baseline": before,
            "current": after,
            "baseline_values": before_values,
            "current_values": after_values,
        })
    })
}

/// Per-repository view: every per-repo sample for one corpus entry, so a
/// target repository we intend to integrate with reads as its own board.
fn repo_view(config: &Config, metrics: &[Value]) -> Vec<Value> {
    let mut repos = Vec::new();
    for entry in &config.corpus {
        let mut rows = Vec::new();
        for metric in metrics {
            for sample in metric["samples"].as_array().into_iter().flatten() {
                if sample["repo"].as_str() == Some(entry.id.as_str()) {
                    rows.push(json!({
                        "metric": metric["id"],
                        "title": metric["title"],
                        "unit": metric["unit"],
                        "target": metric["target"],
                        "value": sample["value"],
                        "status": sample["status"],
                    }));
                }
            }
        }
        let refs: Vec<&Value> = rows.iter().collect();
        let counts = status_counts(&refs);
        repos.push(json!({
            "id": entry.id,
            "sha": entry.sha,
            "note": entry.note,
            "status": if rows.is_empty() { "not_measured" } else { board_status(&counts) },
            "counts": counts,
            "metrics": rows,
        }));
    }
    repos
}

fn metric_row(def: &MetricDef, samples: &[&Sample]) -> Value {
    let base = json!({
        "id": def.id,
        "board": def.board,
        "title": def.title,
        "unit": def.unit,
        "direction": def.direction,
        "target": def.target,
        "source": def.source,
        "runner_dependent": def.runner_dependent,
        "per_repo": def.per_repo,
    });
    let mut row = base;
    let per_repo: Vec<Value> = samples
        .iter()
        .map(|sample| {
            let (value, status) = sample_value_status(def, &sample.outcome);
            json!({
                "repo": sample.repo,
                "value": value,
                "status": status,
                "detail": sample.detail,
            })
        })
        .collect();

    if def.source == "pending" && samples.is_empty() {
        row["status"] = json!("not_measured");
        row["value"] = Value::Null;
        row["reason"] = json!(def.pending_reason.clone().unwrap_or_default());
        row["samples"] = json!(per_repo);
        return row;
    }
    if samples.is_empty() {
        row["status"] = json!("not_measured");
        row["value"] = Value::Null;
        row["reason"] = json!(if def.source.starts_with("ingest:") {
            format!("no --ingest file supplied for `{}`", def.source)
        } else {
            "no sample was taken on this run".to_string()
        });
        row["samples"] = json!(per_repo);
        return row;
    }

    // A failed instrument dominates; otherwise the metric's value is the
    // worst measured sample, because a developer feels the slowest repo.
    let any_failed = samples
        .iter()
        .any(|s| matches!(s.outcome, SampleOutcome::Failed));
    let any_incomplete = samples
        .iter()
        .any(|s| matches!(s.outcome, SampleOutcome::Incomplete(_)));
    let measured: Vec<f64> = samples
        .iter()
        .filter_map(|s| match s.outcome {
            SampleOutcome::Value(v) | SampleOutcome::Incomplete(v) => Some(v),
            SampleOutcome::NotMeasured | SampleOutcome::Failed => None,
        })
        .collect();
    let unmeasured = samples
        .iter()
        .filter(|s| matches!(s.outcome, SampleOutcome::NotMeasured))
        .count();
    let worst = measured
        .iter()
        .copied()
        .reduce(|a, b| if is_worse(def, b, a) { b } else { a });

    let status = if any_failed {
        "failed"
    } else if let Some(value) = worst {
        if any_incomplete || !meets_target(def, value) {
            "below_target"
        } else {
            "meets_target"
        }
    } else {
        "not_measured"
    };
    row["status"] = json!(status);
    row["value"] = worst.map_or(Value::Null, |v| json!(round(v)));
    row["partial"] = json!(unmeasured > 0 && worst.is_some());
    if status == "not_measured" {
        row["reason"] = json!(
            samples
                .iter()
                .map(|s| s.detail.clone())
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    row["samples"] = json!(per_repo);
    row
}

fn sample_value_status(def: &MetricDef, outcome: &SampleOutcome) -> (Value, &'static str) {
    match outcome {
        SampleOutcome::Value(v) => (
            json!(round(*v)),
            if meets_target(def, *v) {
                "meets_target"
            } else {
                "below_target"
            },
        ),
        SampleOutcome::Incomplete(v) => (json!(round(*v)), "incomplete"),
        SampleOutcome::NotMeasured => (Value::Null, "not_measured"),
        SampleOutcome::Failed => (Value::Null, "failed"),
    }
}

pub(crate) fn meets_target(def: &MetricDef, value: f64) -> bool {
    if def.direction == "higher_is_better" {
        value >= def.target
    } else {
        value <= def.target
    }
}

fn is_worse(def: &MetricDef, candidate: f64, current: f64) -> bool {
    if def.direction == "higher_is_better" {
        candidate < current
    } else {
        candidate > current
    }
}

fn round(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

/// Allowed worsening against one baseline value: the larger of
/// `regression_pct` of the baseline and `regression_floor` (so noise on small
/// numbers cannot trip the gate and a large relative slide still does), or
/// their sum for `regression_additive` metrics.
fn allowed_worsening(def: &MetricDef, base: f64) -> f64 {
    let relative = base.abs() * def.regression_pct / 100.0;
    if def.regression_additive {
        relative + def.regression_floor
    } else {
        relative.max(def.regression_floor)
    }
}

fn worsening(def: &MetricDef, base: f64, current: f64) -> f64 {
    if def.direction == "higher_is_better" {
        base - current
    } else {
        current - base
    }
}

/// Repositories whose own sample worsened past the margin against the same
/// repository's baseline sample. The metric value is the worst sample, so
/// without this a regression on any repository but the worst would pass.
fn repo_regressions(def: &MetricDef, row: &Value, base_row: &Value) -> Vec<Value> {
    let base = completed_repo_values(base_row);
    completed_repo_values(row)
        .into_iter()
        .filter_map(|(repo, current)| {
            let (_, before) = base.iter().find(|(id, _)| *id == repo)?;
            let allowed = allowed_worsening(def, *before);
            (worsening(def, *before, current) > allowed).then(|| {
                json!({
                    "repo": repo,
                    "baseline": round(*before),
                    "current": round(current),
                    "allowed_worsening": round(allowed),
                })
            })
        })
        .collect()
}

/// Per-repository values of the samples that completed. An incomplete
/// sample's value is a placeholder (a fail-closed corpus repository records 0
/// ms), so comparing it would read a repository that starts completing again
/// as a regression. Losing completion is judged by `repo_completion_losses`.
fn completed_repo_values(r: &Value) -> Vec<(String, f64)> {
    r["samples"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["status"] != "incomplete")
        .filter_map(|s| Some((s["repo"].as_str()?.to_string(), s["value"].as_f64()?)))
        .collect()
}

/// Repositories that completed in this run but not in the baseline. They have
/// no comparable baseline value, so they are listed rather than dropped.
fn recovered_repos(row: &Value, base_row: &Value) -> Vec<String> {
    let completed_base = completed_repo_values(base_row);
    let base_repos: Vec<&str> = base_row["samples"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s["repo"].as_str())
        .collect();
    completed_repo_values(row)
        .into_iter()
        .map(|(repo, _)| repo)
        .filter(|repo| {
            base_repos.contains(&repo.as_str()) && !completed_base.iter().any(|(id, _)| id == repo)
        })
        .collect()
}

/// Repositories that did not complete in this run although their baseline
/// sample did not stop short either: a repository that completed, or one with
/// no completed baseline sample (new to the corpus, or unmeasured then).
/// Checked per repository because another repository may already have been
/// incomplete in the baseline.
fn repo_completion_losses(row: &Value, base_row: &Value) -> Vec<Value> {
    let statuses = |r: &Value| -> Vec<(String, String)> {
        r["samples"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| {
                Some((
                    s["repo"].as_str()?.to_string(),
                    s["status"].as_str()?.to_string(),
                ))
            })
            .collect()
    };
    let base = statuses(base_row);
    statuses(row)
        .into_iter()
        .filter(|(_, status)| status == "incomplete")
        .filter_map(|(repo, _)| {
            let before = base
                .iter()
                .find(|(id, _)| *id == repo)
                .map(|(_, b)| b.as_str());
            let reason = match before {
                Some("incomplete") => return None,
                Some("meets_target" | "below_target") => "did not complete where the baseline did",
                Some(_) => "did not complete; the baseline has no completed sample for it",
                None => "new in this run and did not complete; no baseline sample",
            };
            Some(json!({"repo": repo, "reason": reason}))
        })
        .collect()
}

/// Worst baseline and current values over common repositories, and the
/// repositories new in this run.
type CommonWorst = (Option<(f64, f64)>, Vec<String>);

/// Worst baseline and current values over the repositories both reports
/// measured to completion, plus the repositories only this run measured. A repository new to
/// the corpus is not a regression, so it must not move the compared worst.
/// `None` when the samples carry no repositories.
fn common_repo_worst(def: &MetricDef, row: &Value, base_row: &Value) -> Option<CommonWorst> {
    let values = |r: &Value| -> Vec<(String, f64)> {
        r["samples"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| Some((s["repo"].as_str()?.to_string(), s["value"].as_f64()?)))
            .collect()
    };
    let (current, base) = (values(row), values(base_row));
    if current.is_empty() && base.is_empty() {
        return None;
    }
    let worst = |items: &mut dyn Iterator<Item = f64>| {
        items.reduce(|a, b| if is_worse(def, b, a) { b } else { a })
    };
    let completed_base = completed_repo_values(base_row);
    let common: Vec<(f64, f64)> = completed_repo_values(row)
        .iter()
        .filter_map(|(repo, now)| {
            completed_base
                .iter()
                .find(|(id, _)| id == repo)
                .map(|(_, before)| (*before, *now))
        })
        .collect();
    let pair =
        worst(&mut common.iter().map(|(b, _)| *b)).zip(worst(&mut common.iter().map(|(_, c)| *c)));
    let new_repos = current
        .iter()
        .filter(|(repo, _)| !base.iter().any(|(id, _)| id == repo))
        .map(|(repo, _)| repo.clone())
        .collect();
    Some((pair, new_repos))
}

/// Compare one metric with the same metric in an earlier report, as the
/// worst value and per repository.
pub(crate) fn compare_with_baseline(
    def: &MetricDef,
    row: &Value,
    context: &RunContext,
    baseline: Option<&Value>,
) -> Value {
    let Some(baseline) = baseline else {
        return json!({"comparable": false, "reason": "no baseline supplied"});
    };
    let base_row = baseline["metrics"].as_array().and_then(|rows| {
        rows.iter()
            .find(|r| r["id"].as_str() == Some(def.id.as_str()))
    });
    let Some(base_row) = base_row else {
        return json!({"comparable": false, "reason": "metric absent from baseline"});
    };
    // Samples with a repository are judged per repository below.
    let incomplete = |r: &Value| {
        r["samples"].as_array().is_some_and(|samples| {
            samples
                .iter()
                .any(|s| s["status"] == "incomplete" && s["repo"].is_null())
        })
    };
    // A run that stops completing is broken on every runner class, and its
    // elapsed time can look faster than the baseline, so check it first, per
    // repository as well as for the metric.
    let lost = repo_completion_losses(row, base_row);
    if !lost.is_empty() || (incomplete(row) && !incomplete(base_row)) {
        // Repositories that completed but got worse would otherwise be hidden
        // behind the lost completion; list them too where values compare.
        let mut lost = lost;
        if !def.runner_dependent
            || baseline["runner_class"].as_str() == Some(context.runner_class.as_str())
        {
            for regression in repo_regressions(def, row, base_row) {
                if !lost.iter().any(|entry| entry["repo"] == regression["repo"]) {
                    lost.push(regression);
                }
            }
        }
        return json!({
            "comparable": true,
            "value": base_row["value"],
            "delta": Value::Null,
            "allowed_worsening": 0.0,
            "regressed": true,
            "reason": "a repository stopped completing or regressed; see each repository below",
            "regressed_repos": lost,
            "recovered_repos": recovered_repos(row, base_row),
        });
    }
    if def.runner_dependent
        && baseline["runner_class"].as_str() != Some(context.runner_class.as_str())
    {
        return json!({
            "comparable": false,
            "value": base_row["value"],
            "reason": format!(
                "runner class differs (baseline `{}`, current `{}`)",
                baseline["runner_class"].as_str().unwrap_or("unknown"),
                context.runner_class
            ),
        });
    }
    let (values, new_repos) = match common_repo_worst(def, row, base_row) {
        Some((pair, new_repos)) => (pair, new_repos),
        None => (
            base_row["value"].as_f64().zip(row["value"].as_f64()),
            Vec::new(),
        ),
    };
    let recovered = recovered_repos(row, base_row);
    let Some((base, current)) = values else {
        return json!({
            "comparable": false,
            "value": base_row["value"],
            "reason": if new_repos.is_empty() && recovered.is_empty() {
                "baseline or current value is missing"
            } else {
                "no repository completed in both reports"
            },
            "new_repos": new_repos,
            "recovered_repos": recovered,
        });
    };
    let allowed = allowed_worsening(def, base);
    let by_repo = repo_regressions(def, row, base_row);
    let missing = missing_repos(row, base_row);
    json!({
        "comparable": true,
        "value": round(base),
        "delta": round(current - base),
        "allowed_worsening": round(allowed),
        "regressed": worsening(def, base, current) > allowed || !by_repo.is_empty(),
        "regressed_repos": by_repo,
        "new_repos": new_repos,
        "missing_repos": missing,
        "recovered_repos": recovered,
    })
}

/// Repositories the baseline measured that this run has no value for. Their
/// absence cannot regress the aggregate, so they are listed instead of being
/// silently passed.
fn missing_repos(row: &Value, base_row: &Value) -> Vec<String> {
    let measured = |r: &Value| -> Vec<String> {
        r["samples"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|s| !s["value"].is_null())
            .filter_map(|s| s["repo"].as_str().map(str::to_string))
            .collect()
    };
    let now = measured(row);
    measured(base_row)
        .into_iter()
        .filter(|repo| !now.contains(repo))
        .collect()
}

fn status_counts(rows: &[&Value]) -> Value {
    let mut counts = BTreeMap::from([
        ("meets_target", 0_u64),
        ("below_target", 0),
        ("not_measured", 0),
        ("failed", 0),
        ("regressed", 0),
    ]);
    for row in rows {
        if let Some(status) = row["status"].as_str()
            && let Some(slot) = counts.get_mut(status)
        {
            *slot += 1;
        }
        if row["baseline"]["regressed"].as_bool() == Some(true)
            && let Some(slot) = counts.get_mut("regressed")
        {
            *slot += 1;
        }
    }
    json!(counts)
}

fn board_status(counts: &Value) -> &'static str {
    let get = |key: &str| counts[key].as_u64().unwrap_or(0);
    if get("failed") > 0 || get("regressed") > 0 {
        "attention"
    } else if get("below_target") > 0 {
        "gaps"
    } else if get("meets_target") == 0 {
        "not_measured"
    } else if get("not_measured") > 0 {
        "partial"
    } else {
        "meets_target"
    }
}

fn gate_failure_message(report: &Value) -> String {
    let mut lines = vec!["dx-scoreboard gate failed:".to_string()];
    for regression in report["gate"]["regressions"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let metric = regression["metric"].as_str().unwrap_or("?");
        if let Some(reason) = regression["reason"].as_str() {
            lines.push(format!("  regressed {metric}: {reason}"));
        } else {
            lines.push(format!(
                "  regressed {metric}: baseline {} -> current {} (allowed worsening {})",
                regression["baseline"], regression["current"], regression["allowed_worsening"],
            ));
        }
        for repo in regression["regressed_repos"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let Some(reason) = repo["reason"].as_str() {
                lines.push(format!(
                    "    {}: {reason}",
                    repo["repo"].as_str().unwrap_or("?")
                ));
                continue;
            }
            lines.push(format!(
                "    {}: baseline {} -> current {} (allowed worsening {})",
                repo["repo"].as_str().unwrap_or("?"),
                repo["baseline"],
                repo["current"],
                repo["allowed_worsening"],
            ));
        }
    }
    for metric in report["gate"]["failed_instruments"]
        .as_array()
        .into_iter()
        .flatten()
    {
        lines.push(format!(
            "  instrument failed: {}",
            metric.as_str().unwrap_or("?")
        ));
    }
    lines.push("see target/ripr/reports/dx-scoreboard.md".to_string());
    lines.join("\n")
}

pub(crate) fn render_markdown(report: &Value) -> String {
    let mut out = String::new();
    out.push_str("# ripr developer-experience scoreboards\n\n");
    let rollup = &report["rollup"];
    out.push_str(&format!(
        "Revision `{}` · {} · runner `{}`\n\n",
        report["revision"].as_str().unwrap_or("unknown"),
        report["analyzer_version"].as_str().unwrap_or("unknown"),
        report["runner_class"].as_str().unwrap_or("unknown"),
    ));
    out.push_str(&format!(
        "**Rollup:** {} meet target, {} below target, {} not measured, {} failed, {} regressed. Gate: `{}`.\n\n",
        rollup["meets_target"],
        rollup["below_target"],
        rollup["not_measured"],
        rollup["failed"],
        rollup["regressed"],
        report["gate"]["status"].as_str().unwrap_or("not_run"),
    ));
    let review = report["gate"]["review"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let uncompared = report["gate"]["uncompared"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if report["gate"]["enabled"] == true && !uncompared.is_empty() {
        out.push_str("**Not compared with the baseline (outside this gate run):**\n\n");
        for item in uncompared {
            out.push_str(&format!(
                "- `{}`: {}\n",
                item["metric"].as_str().unwrap_or("?"),
                item["reason"].as_str().unwrap_or("not compared"),
            ));
        }
        out.push('\n');
    }
    if !review.is_empty() {
        out.push_str("**For review (does not fail the gate):**\n\n");
        for item in review {
            let change = &item["change"];
            // Show the evidence when it changed, else the values that moved.
            let (before, after) = if change["baseline"] == change["current"] {
                (&change["baseline_values"], &change["current_values"])
            } else {
                (&change["baseline"], &change["current"])
            };
            out.push_str(&format!(
                "- `{}` changed: baseline {before} → current {after}\n",
                item["metric"].as_str().unwrap_or("?"),
            ));
        }
        out.push('\n');
    }
    out.push_str("| Board | Status | Meets | Below | Not measured |\n|---|---|---:|---:|---:|\n");
    for board in report["boards"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            board["id"].as_str().unwrap_or("?"),
            board["status"].as_str().unwrap_or("?"),
            board["counts"]["meets_target"],
            board["counts"]["below_target"],
            board["counts"]["not_measured"],
        ));
    }
    let repos: Vec<&Value> = report["repos"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|repo| repo["status"].as_str() != Some("not_measured"))
        .collect();
    if !repos.is_empty() {
        out.push_str("\n## By repository\n\n| Repository | Metric | Value | Target | Status |\n|---|---|---:|---:|---|\n");
        for repo in repos {
            for metric in repo["metrics"].as_array().into_iter().flatten() {
                out.push_str(&format!(
                    "| {} | `{}` | {} | {} | {} |\n",
                    repo["id"].as_str().unwrap_or("?"),
                    metric["metric"].as_str().unwrap_or("?"),
                    display_value(&metric["value"]),
                    display_value(&metric["target"]),
                    metric["status"].as_str().unwrap_or("?"),
                ));
            }
        }
    }
    for board in report["boards"].as_array().into_iter().flatten() {
        let id = board["id"].as_str().unwrap_or("?");
        out.push_str(&format!("\n## {id}\n\n"));
        out.push_str("| Metric | Value | Target | Status | Baseline |\n|---|---:|---:|---|---|\n");
        for metric in report["metrics"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| m["board"].as_str() == Some(id))
        {
            out.push_str(&format!(
                "| {} (`{}`) | {} | {} {} | {} | {} |\n",
                metric["title"].as_str().unwrap_or("?"),
                metric["id"].as_str().unwrap_or("?"),
                display_value(&metric["value"]),
                comparator(metric),
                display_value(&metric["target"]),
                metric["status"].as_str().unwrap_or("?"),
                baseline_cell(&metric["baseline"]),
            ));
        }
        for metric in report["metrics"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| m["board"].as_str() == Some(id))
        {
            let samples = metric["samples"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let reason = metric["reason"].as_str();
            if samples.is_empty() && reason.is_none() {
                continue;
            }
            out.push_str(&format!(
                "\n`{}` ({}):\n",
                metric["id"].as_str().unwrap_or("?"),
                metric["unit"].as_str().unwrap_or("")
            ));
            if let Some(reason) = reason
                && samples.is_empty()
            {
                out.push_str(&format!("- not measured: {reason}\n"));
            }
            for sample in samples {
                out.push_str(&format!(
                    "- {}: {} ({}) {}\n",
                    sample["repo"].as_str().unwrap_or("all"),
                    display_value(&sample["value"]),
                    sample["status"].as_str().unwrap_or("?"),
                    sample["detail"].as_str().unwrap_or(""),
                ));
            }
        }
    }
    out.push_str(&format!(
        "\n## Claim boundary\n\n{}\n",
        report["claim_boundary"].as_str().unwrap_or("")
    ));
    out
}

fn comparator(metric: &Value) -> &'static str {
    if metric["direction"].as_str() == Some("higher_is_better") {
        "≥"
    } else {
        "≤"
    }
}

fn display_value(value: &Value) -> String {
    match value {
        Value::Null => "—".to_string(),
        Value::Number(number) => match number.as_f64() {
            Some(n) if n.abs() >= 10.0 || n.fract().abs() < f64::EPSILON => format!("{n:.0}"),
            Some(n) => format!("{n:.3}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string(),
            None => number.to_string(),
        },
        other => other.to_string(),
    }
}

/// `; <label>: a, b` for the repositories a baseline comparison lists under
/// `key`, or nothing when it lists none.
fn repo_note(baseline: &Value, key: &str, label: &str) -> String {
    let repos: Vec<&str> = baseline[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if repos.is_empty() {
        String::new()
    } else {
        format!("; {label}: {}", repos.join(", "))
    }
}

fn baseline_cell(baseline: &Value) -> String {
    if baseline["comparable"].as_bool() == Some(true) {
        let verdict = if baseline["regressed"].as_bool() == Some(true) {
            "**regressed**"
        } else if baseline["worse"].as_bool() == Some(true) {
            "worse, for review"
        } else {
            "ok"
        };
        let new_note = repo_note(baseline, "new_repos", "not in baseline")
            + &repo_note(baseline, "recovered_repos", "completed again");
        format!(
            "{} (Δ {}) {verdict}{new_note}",
            display_value(&baseline["value"]),
            display_value(&baseline["delta"])
        )
    } else {
        baseline["reason"]
            .as_str()
            .unwrap_or("not compared")
            .to_string()
            + &repo_note(baseline, "recovered_repos", "completed again")
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

#[cfg(test)]
mod tests;
