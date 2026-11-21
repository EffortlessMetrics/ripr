//! `cargo xtask ci-budget` — the advisory CI budget and merge-queue hygiene
//! report from the proof-routing operating model (docs/PROOF_ROUTING.md,
//! slice 19).
//!
//! The command reads recent runs of the routed Rust workflow (default
//! "Routed Rust Small") through `gh run list` / `gh run view` and makes CI
//! cost and failure loops visible. The single most important signal it
//! surfaces is the disk-guard tempfail tax (issue #1058): a poisoned
//! self-hosted host whose scratch disk is consumed by another swarm's work
//! dir trips `ci-disk-guard` (exit 75), so routed lanes fail-fast and retry.
//! Separating those runner-scratch infrastructure failures from real product
//! (test/gate/build) failures is what this report exists to do.
//!
//! The report is `advisory-report-only`: it never fails CI, never reruns or
//! mutates any run, and changes no CI behavior. It calls `gh` exactly the way
//! `pr-triage-report` and `gh-pr-status` do — through the centralized
//! `crate::run` command runner, which is the single process-allowlisted
//! subprocess surface. For offline or sandboxed use it also accepts
//! `--input <path>`, a JSON file the caller produces (for example
//! `gh run list --workflow "Routed Rust Small" --json ... > runs.json`).

use crate::run::{run_output_optional, run_output_owned};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const DEFAULT_WORKFLOW: &str = "Routed Rust Small";
const DEFAULT_LIMIT: usize = 40;
const CI_BUDGET_SCHEMA_VERSION: &str = "0.1";
const CI_BUDGET_STATE: &str = "advisory-report-only";
const CI_BUDGET_JSON: &str = "ci-budget.json";
const CI_BUDGET_MD: &str = "ci-budget.md";
const CI_BUDGET_USAGE: &str =
    "usage: cargo xtask ci-budget [--workflow <name>] [--limit <n>] [--input <path>]";

/// The impl lanes a routed run can land on, parsed from job names. The route
/// and result coordinator jobs are tracked separately and are never counted
/// as the routed lane of a run.
const IMPL_LANES: &[&str] = &["cx43", "cpx42", "cx53", "github"];

pub(crate) fn ci_budget(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{CI_BUDGET_USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    let runs =
        match &options.input {
            Some(path) => {
                let text = std::fs::read_to_string(path)
                    .map_err(|err| format!("failed to read ci-budget input {path}: {err}"))?;
                parse_runs_value(&serde_json::from_str(&text).map_err(|err| {
                    format!("failed to parse ci-budget input {path} as JSON: {err}")
                })?)?
            }
            None => collect_runs_live(&options.workflow, options.limit)?,
        };
    let report = build_report(&options.workflow, options.limit, &runs);

    let json_value = report_json(&report);
    let json_text = serde_json::to_string_pretty(&json_value)
        .map_err(|err| format!("serialize ci-budget report: {err}"))?;
    crate::write_report(CI_BUDGET_JSON, &format!("{json_text}\n"))?;
    crate::write_report(CI_BUDGET_MD, &report_markdown(&report))?;
    println!("Wrote target/ripr/reports/{CI_BUDGET_JSON}");
    println!("Wrote target/ripr/reports/{CI_BUDGET_MD}");
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CiBudgetOptions {
    workflow: String,
    limit: usize,
    input: Option<String>,
}

impl Default for CiBudgetOptions {
    fn default() -> Self {
        Self {
            workflow: DEFAULT_WORKFLOW.to_string(),
            limit: DEFAULT_LIMIT,
            input: None,
        }
    }
}

fn parse_options(args: &[String]) -> Result<CiBudgetOptions, String> {
    let mut options = CiBudgetOptions::default();
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--workflow" => {
                index += 1;
                options.workflow = non_empty_arg(args, index, "--workflow")?.to_string();
            }
            "--limit" => {
                index += 1;
                let value = non_empty_arg(args, index, "--limit")?;
                let parsed = value.parse::<usize>().map_err(|err| {
                    format!("ci-budget --limit must be a positive integer: {err}")
                })?;
                if parsed == 0 {
                    return Err("ci-budget --limit must be greater than zero".to_string());
                }
                options.limit = parsed;
            }
            "--input" => {
                index += 1;
                options.input = Some(non_empty_arg(args, index, "--input")?.to_string());
            }
            other => {
                return Err(format!(
                    "unknown ci-budget argument `{other}`; {CI_BUDGET_USAGE}"
                ));
            }
        }
        index += 1;
    }
    Ok(options)
}

fn non_empty_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    let Some(value) = args.get(index) else {
        return Err(format!("missing value for {flag}; {CI_BUDGET_USAGE}"));
    };
    if value.trim().is_empty() {
        return Err(format!("ci-budget {flag} requires a non-empty value"));
    }
    Ok(value)
}

// ---------------------------------------------------------------------------
// Raw run/job model
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RawJob {
    name: String,
    conclusion: String,
    status: String,
    started_at: String,
    completed_at: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RawRun {
    database_id: u64,
    display_title: String,
    conclusion: String,
    status: String,
    event: String,
    head_branch: String,
    created_at: String,
    started_at: String,
    updated_at: String,
    attempt: u64,
    jobs: Vec<RawJob>,
    /// Failed-step log text for failed runs (used to classify the failure and
    /// to surface the GB-free figure when the disk guard tripped). Empty for
    /// non-failed runs or when the log could not be fetched.
    failed_log: String,
}

fn collect_runs_live(workflow: &str, limit: usize) -> Result<Vec<RawRun>, String> {
    let limit_text = limit.to_string();
    let list_args = [
        "run".to_string(),
        "list".to_string(),
        "--workflow".to_string(),
        workflow.to_string(),
        "--limit".to_string(),
        limit_text,
        "--json".to_string(),
        "databaseId,displayTitle,conclusion,status,event,headBranch,createdAt,startedAt,updatedAt,attempt,number".to_string(),
    ];
    let list_json = run_output_owned("gh", &list_args)?;
    let mut runs = parse_runs_value(
        &serde_json::from_str(&list_json)
            .map_err(|err| format!("failed to parse gh run list JSON: {err}"))?,
    )?;
    for run in &mut runs {
        let id = run.database_id.to_string();
        let jobs_json =
            run_output_optional("gh", &["run", "view", &id, "--json", "jobs"]).unwrap_or_default();
        if !jobs_json.trim().is_empty() {
            run.jobs = parse_jobs(&jobs_json);
        }
        if run.conclusion.eq_ignore_ascii_case("failure") {
            run.failed_log = run_output_optional("gh", &["run", "view", &id, "--log-failed"])
                .unwrap_or_default();
        }
    }
    Ok(runs)
}

/// Accept either a `gh run list --json ...` array or an object with a `runs`
/// array (the richer `--input` shape that may embed `jobs` and `failed_log`).
fn parse_runs_value(value: &Value) -> Result<Vec<RawRun>, String> {
    let items: Vec<Value> = if let Some(array) = value.as_array() {
        array.clone()
    } else if let Some(runs) = value.get("runs").and_then(Value::as_array) {
        runs.clone()
    } else {
        return Err(
            "ci-budget input must be a run array or an object with a `runs` array".to_string(),
        );
    };
    let mut runs = Vec::new();
    for item in &items {
        runs.push(raw_run_from_value(item)?);
    }
    runs.sort_by_key(|run| std::cmp::Reverse(run.database_id));
    Ok(runs)
}

fn raw_run_from_value(item: &Value) -> Result<RawRun, String> {
    let database_id = item
        .get("databaseId")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("gh run JSON item is missing numeric `databaseId`: {item}"))?;
    let attempt = item
        .get("attempt")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1);
    let jobs = item
        .get("jobs")
        .and_then(Value::as_array)
        .map(|array| array.iter().map(raw_job_from_value).collect())
        .unwrap_or_default();
    Ok(RawRun {
        database_id,
        display_title: json_str(item, "displayTitle"),
        conclusion: json_str(item, "conclusion"),
        status: json_str(item, "status"),
        event: json_str(item, "event"),
        head_branch: json_str(item, "headBranch"),
        created_at: json_str(item, "createdAt"),
        started_at: json_str(item, "startedAt"),
        updated_at: json_str(item, "updatedAt"),
        attempt,
        jobs,
        failed_log: json_str(item, "failed_log"),
    })
}

fn parse_jobs(text: &str) -> Vec<RawJob> {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    value
        .get("jobs")
        .and_then(Value::as_array)
        .map(|array| array.iter().map(raw_job_from_value).collect())
        .unwrap_or_default()
}

fn raw_job_from_value(value: &Value) -> RawJob {
    RawJob {
        name: json_str(value, "name"),
        conclusion: json_str(value, "conclusion"),
        status: json_str(value, "status"),
        started_at: json_str(value, "startedAt"),
        completed_at: json_str(value, "completedAt"),
    }
}

fn json_str(value: &Value, key: &str) -> String {
    match value.get(key).and_then(Value::as_str) {
        Some(item) => item.to_string(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Classification + aggregation (pure, unit-tested)
// ---------------------------------------------------------------------------

/// The lane a job ran on, derived from its name. Impl lanes are the routed
/// targets; `route`/`result` are the coordinator jobs; `other` is anything
/// else.
fn lane_of(job_name: &str) -> &'static str {
    let lower = job_name.to_lowercase();
    if lower.contains("on cx43") {
        "cx43"
    } else if lower.contains("on cpx42") {
        "cpx42"
    } else if lower.contains("on cx53") {
        "cx53"
    } else if lower.contains("on github") {
        "github"
    } else if lower.contains("route") {
        "route"
    } else if lower.contains("result") {
        "result"
    } else {
        "other"
    }
}

/// Whether a job actually executed (as opposed to being skipped by an `if:`
/// gate or never started). Skipped jobs report a `skipped` conclusion.
fn job_ran(job: &RawJob) -> bool {
    let conclusion = job.conclusion.to_lowercase();
    !matches!(conclusion.as_str(), "" | "skipped" | "neutral")
}

/// The impl lane a whole run routed to: the impl-lane job that actually ran.
/// The routed workflow gates every non-selected impl lane behind an `if:`, so
/// we prefer a job that ran and fall back to the first impl job otherwise.
/// `unknown` when no impl job is present (for example a run whose jobs were
/// not fetched).
fn run_routed_lane(run: &RawRun) -> &'static str {
    let mut fallback = None;
    for job in &run.jobs {
        let lane = lane_of(&job.name);
        if IMPL_LANES.contains(&lane) {
            if job_ran(job) {
                return lane;
            }
            fallback.get_or_insert(lane);
        }
    }
    fallback.unwrap_or("unknown")
}

/// Classify a failed run's failure from its log text. The disk-guard signal
/// (issue #1058) wins over everything: a "disk guard" message or exit 75 is
/// always infrastructure, never a product failure.
fn classify_failure(log: &str) -> &'static str {
    let lower = log.to_lowercase();
    if lower.contains("disk guard")
        || lower.contains("disk-guard")
        || lower.contains("exit 75")
        || lower.contains("exit code 75")
    {
        "infra_disk_guard"
    } else if log.trim().is_empty() {
        "unknown"
    } else if lower.contains("lost communication")
        || lower.contains("received a shutdown signal")
        || lower.contains("the runner has received")
        || lower.contains("runner_capacity_unavailable")
        || lower.contains("no_idle_runner")
    {
        "infra_other"
    } else {
        "product"
    }
}

/// Extract the GB-free figure reported near a disk-guard message, if present.
fn extract_disk_guard_gb_free(log: &str) -> Option<f64> {
    let lines: Vec<&str> = log.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let lower = line.to_lowercase();
        if lower.contains("disk guard")
            || lower.contains("disk-guard")
            || lower.contains("ci-disk-guard")
        {
            let window: String = lines
                .iter()
                .skip(index)
                .take(3)
                .copied()
                .collect::<Vec<_>>()
                .join(" ");
            if let Some(value) = first_gb_value(&window) {
                return Some(value);
            }
        }
    }
    None
}

fn first_gb_value(text: &str) -> Option<f64> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    for (index, token) in tokens.iter().enumerate() {
        let trimmed = token.trim_matches(|c: char| matches!(c, ',' | '(' | ')' | ':'));
        let lower = trimmed.to_lowercase();
        if let Some(number) = lower
            .strip_suffix("gib")
            .or_else(|| lower.strip_suffix("gb"))
            && let Ok(value) = number.parse::<f64>()
        {
            return Some(value);
        }
        if let Ok(value) = trimmed.parse::<f64>()
            && let Some(next) = tokens.get(index + 1)
        {
            let next_lower = next.to_lowercase();
            if next_lower.starts_with("gb") || next_lower.starts_with("gib") || next_lower == "g" {
                return Some(value);
            }
        }
    }
    None
}

/// `created -> started` queue wait or job duration, in seconds, when both
/// timestamps parse and the delta is non-negative.
fn duration_seconds(start: &str, end: &str) -> Option<u64> {
    let start = parse_iso_seconds(start)?;
    let end = parse_iso_seconds(end)?;
    if end < start {
        return None;
    }
    u64::try_from(end - start).ok()
}

fn parse_iso_seconds(timestamp: &str) -> Option<i64> {
    let timestamp = timestamp.trim();
    let date = timestamp.get(0..10)?;
    let time = timestamp.get(11..19)?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i32>().ok()?;
    let month = date_parts.next()?.parse::<u32>().ok()?;
    let day = date_parts.next()?.parse::<u32>().ok()?;
    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<i64>().ok()?;
    let minute = time_parts.next()?.parse::<i64>().ok()?;
    let second = time_parts.next()?.parse::<i64>().ok()?;
    let days = crate::days_from_civil(year, month, day);
    Some(days * 86_400 + hour * 3600 + minute * 60 + second)
}

fn min_median_max(values: &[u64]) -> Option<(u64, u64, u64)> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let last = sorted.len() - 1;
    let middle = sorted.len() / 2;
    let median = if sorted.len() % 2 == 1 {
        sorted[middle]
    } else {
        (sorted[middle - 1] + sorted[middle]) / 2
    };
    Some((sorted[0], median, sorted[last]))
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct LaneCount {
    lane: String,
    total: usize,
    success: usize,
    failure: usize,
    cancelled: usize,
    other: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct DiskGuardEvent {
    run_id: u64,
    title: String,
    lane: String,
    gb_free: Option<f64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DurationStat {
    lane: String,
    count: usize,
    min_seconds: u64,
    median_seconds: u64,
    max_seconds: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct CiBudgetReport {
    workflow: String,
    limit: usize,
    total_runs: usize,
    by_conclusion: Vec<(String, usize)>,
    by_lane: Vec<LaneCount>,
    failure_total: usize,
    infra_disk_guard: usize,
    product_failures: usize,
    infra_other: usize,
    unknown_failures: usize,
    total_attempts: u64,
    runs_with_reruns: usize,
    tempfail_retry_count: usize,
    extra_attempts: u64,
    durations: Vec<DurationStat>,
    disk_guard_events: Vec<DiskGuardEvent>,
    queue_wait: Option<(u64, u64, u64)>,
}

fn build_report(workflow: &str, limit: usize, runs: &[RawRun]) -> CiBudgetReport {
    let mut by_conclusion: BTreeMap<String, usize> = BTreeMap::new();
    let mut lane_counts: BTreeMap<String, LaneCount> = BTreeMap::new();
    let mut failure_classes: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut lane_durations: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    let mut disk_guard_events = Vec::new();
    let mut queue_waits = Vec::new();

    let mut total_attempts = 0u64;
    let mut runs_with_reruns = 0usize;
    let mut tempfail_retry_count = 0usize;
    let mut extra_attempts = 0u64;

    for run in runs {
        let conclusion = if run.conclusion.is_empty() {
            run.status.to_lowercase()
        } else {
            run.conclusion.to_lowercase()
        };
        *by_conclusion.entry(conclusion.clone()).or_insert(0) += 1;

        let lane = run_routed_lane(run).to_string();
        let entry = lane_counts
            .entry(lane.clone())
            .or_insert_with(|| LaneCount {
                lane: lane.clone(),
                ..LaneCount::default()
            });
        entry.total += 1;
        match conclusion.as_str() {
            "success" => entry.success += 1,
            "failure" => entry.failure += 1,
            "cancelled" => entry.cancelled += 1,
            _ => entry.other += 1,
        }

        total_attempts += run.attempt;
        if run.attempt > 1 {
            runs_with_reruns += 1;
            extra_attempts += run.attempt - 1;
            if conclusion == "success" {
                tempfail_retry_count += 1;
            }
        }

        if conclusion == "failure" {
            let class = classify_failure(&run.failed_log);
            *failure_classes.entry(class).or_insert(0) += 1;
            if class == "infra_disk_guard" {
                disk_guard_events.push(DiskGuardEvent {
                    run_id: run.database_id,
                    title: run.display_title.clone(),
                    lane: lane.clone(),
                    gb_free: extract_disk_guard_gb_free(&run.failed_log),
                });
            }
        }

        for job in &run.jobs {
            let job_lane = lane_of(&job.name);
            // Only count lanes that actually ran. The routed workflow gates
            // every non-selected impl lane behind an `if:`, so skipped jobs
            // would otherwise pollute the duration table with zero-length rows.
            if IMPL_LANES.contains(&job_lane)
                && job_ran(job)
                && let Some(seconds) = duration_seconds(&job.started_at, &job.completed_at)
            {
                lane_durations
                    .entry(job_lane.to_string())
                    .or_default()
                    .push(seconds);
            }
        }

        if let Some(seconds) = duration_seconds(&run.created_at, &run.started_at) {
            queue_waits.push(seconds);
        }
    }

    let durations = lane_durations
        .into_iter()
        .filter_map(|(lane, values)| {
            min_median_max(&values).map(|(min, median, max)| DurationStat {
                lane,
                count: values.len(),
                min_seconds: min,
                median_seconds: median,
                max_seconds: max,
            })
        })
        .collect();

    let failure_total = failure_classes.values().sum();
    CiBudgetReport {
        workflow: workflow.to_string(),
        limit,
        total_runs: runs.len(),
        by_conclusion: by_conclusion.into_iter().collect(),
        by_lane: lane_counts.into_values().collect(),
        failure_total,
        infra_disk_guard: failure_classes
            .get("infra_disk_guard")
            .copied()
            .unwrap_or(0),
        product_failures: failure_classes.get("product").copied().unwrap_or(0),
        infra_other: failure_classes.get("infra_other").copied().unwrap_or(0),
        unknown_failures: failure_classes.get("unknown").copied().unwrap_or(0),
        total_attempts,
        runs_with_reruns,
        tempfail_retry_count,
        extra_attempts,
        durations,
        disk_guard_events,
        queue_wait: min_median_max(&queue_waits),
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn report_json(report: &CiBudgetReport) -> Value {
    let by_conclusion: Vec<Value> = report
        .by_conclusion
        .iter()
        .map(|(conclusion, count)| json!({"conclusion": conclusion, "count": count}))
        .collect();
    let by_lane: Vec<Value> = report
        .by_lane
        .iter()
        .map(|lane| {
            json!({
                "lane": lane.lane,
                "total": lane.total,
                "success": lane.success,
                "failure": lane.failure,
                "cancelled": lane.cancelled,
                "other": lane.other,
            })
        })
        .collect();
    let durations: Vec<Value> = report
        .durations
        .iter()
        .map(|stat| {
            json!({
                "lane": stat.lane,
                "count": stat.count,
                "min_seconds": stat.min_seconds,
                "median_seconds": stat.median_seconds,
                "max_seconds": stat.max_seconds,
            })
        })
        .collect();
    let disk_guard_events: Vec<Value> = report
        .disk_guard_events
        .iter()
        .map(|event| {
            json!({
                "run_id": event.run_id,
                "title": event.title,
                "lane": event.lane,
                "gb_free": event.gb_free,
            })
        })
        .collect();
    let queue_wait = match report.queue_wait {
        Some((min, median, max)) => {
            json!({"min_seconds": min, "median_seconds": median, "max_seconds": max})
        }
        None => Value::Null,
    };
    json!({
        "schema_version": CI_BUDGET_SCHEMA_VERSION,
        "report_state": CI_BUDGET_STATE,
        "workflow": report.workflow,
        "limit": report.limit,
        "total_runs": report.total_runs,
        "by_conclusion": by_conclusion,
        "by_lane": by_lane,
        "failure_classification": {
            "failure_total": report.failure_total,
            "infra_disk_guard": report.infra_disk_guard,
            "product": report.product_failures,
            "infra_other": report.infra_other,
            "unknown": report.unknown_failures,
        },
        "reruns": {
            "total_attempts": report.total_attempts,
            "runs_with_reruns": report.runs_with_reruns,
            "tempfail_retry_count": report.tempfail_retry_count,
            "extra_attempts": report.extra_attempts,
        },
        "durations_by_lane": durations,
        "disk_guard_events": disk_guard_events,
        "queue_wait": queue_wait,
    })
}

fn format_duration(seconds: u64) -> String {
    let minutes = seconds / 60;
    let remainder = seconds % 60;
    format!("{minutes}m{remainder:02}s")
}

fn report_markdown(report: &CiBudgetReport) -> String {
    let mut body = String::from("# ripr CI budget report\n\n");
    body.push_str("Status: advisory\n");
    body.push_str("Mode: advisory\n");
    body.push_str(&format!("Report state: {CI_BUDGET_STATE}\n"));
    body.push_str(&format!("Workflow: `{}`\n", report.workflow));
    body.push_str(&format!(
        "Runs scanned: {} (limit {})\n\n",
        report.total_runs, report.limit
    ));
    body.push_str(
        "This report makes CI cost and failure loops visible. It never reruns, cancels, or \
         mutates any run and changes no CI behavior. Its key signal is the disk-guard tempfail \
         tax (issue #1058): runner-scratch infrastructure failures (a tripped `ci-disk-guard`, \
         exit 75) are separated from real product (test/gate/build) failures.\n\n",
    );

    body.push_str("## Runs by conclusion\n\n");
    if report.by_conclusion.is_empty() {
        body.push_str("(no runs)\n\n");
    } else {
        body.push_str("| Conclusion | Runs |\n| --- | --- |\n");
        for (conclusion, count) in &report.by_conclusion {
            let label = if conclusion.is_empty() {
                "(unknown)"
            } else {
                conclusion
            };
            body.push_str(&format!("| `{label}` | {count} |\n"));
        }
        body.push('\n');
    }

    body.push_str("## Runs by routed lane\n\n");
    if report.by_lane.is_empty() {
        body.push_str("(no runs)\n\n");
    } else {
        body.push_str("| Lane | Runs | Success | Failure | Cancelled | Other |\n");
        body.push_str("| --- | --- | --- | --- | --- | --- |\n");
        for lane in &report.by_lane {
            body.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | {} |\n",
                lane.lane, lane.total, lane.success, lane.failure, lane.cancelled, lane.other
            ));
        }
        body.push('\n');
    }

    body.push_str("## Failure classification\n\n");
    body.push_str(&format!("Total failed runs: {}\n\n", report.failure_total));
    body.push_str("| Class | Runs |\n| --- | --- |\n");
    body.push_str(&format!(
        "| `infra_disk_guard` | {} |\n",
        report.infra_disk_guard
    ));
    body.push_str(&format!("| `product` | {} |\n", report.product_failures));
    body.push_str(&format!("| `infra_other` | {} |\n", report.infra_other));
    body.push_str(&format!("| `unknown` | {} |\n\n", report.unknown_failures));
    if report.infra_disk_guard > 0 {
        body.push_str(&format!(
            "Disk-guard tempfail tax: {} of {} failed run(s) were runner-scratch \
             infrastructure failures (issue #1058), not product failures.\n\n",
            report.infra_disk_guard, report.failure_total
        ));
    }

    body.push_str("## Reruns and tempfail-retry tax\n\n");
    body.push_str(&format!(
        "- Total attempts across scanned runs: {}\n",
        report.total_attempts
    ));
    body.push_str(&format!(
        "- Runs that needed a rerun (attempt > 1): {}\n",
        report.runs_with_reruns
    ));
    body.push_str(&format!(
        "- Fail-then-succeeded sequences (tempfail-retry tax estimate): {}\n",
        report.tempfail_retry_count
    ));
    body.push_str(&format!(
        "- Extra attempts beyond the first: {}\n\n",
        report.extra_attempts
    ));

    body.push_str("## Duration by lane\n\n");
    if report.durations.is_empty() {
        body.push_str("(no lane durations available)\n\n");
    } else {
        body.push_str("| Lane | Jobs | Min | Median | Max |\n");
        body.push_str("| --- | --- | --- | --- | --- |\n");
        for stat in &report.durations {
            body.push_str(&format!(
                "| `{}` | {} | {} | {} | {} |\n",
                stat.lane,
                stat.count,
                format_duration(stat.min_seconds),
                format_duration(stat.median_seconds),
                format_duration(stat.max_seconds),
            ));
        }
        body.push('\n');
    }

    body.push_str("## Scratch/cache warnings\n\n");
    if report.disk_guard_events.is_empty() {
        body.push_str("No disk-guard failures observed in the scanned runs.\n\n");
    } else {
        body.push_str("| Run | Lane | GB free | Title |\n| --- | --- | --- | --- |\n");
        for event in &report.disk_guard_events {
            let gb = match event.gb_free {
                Some(value) => format!("{value:.1}"),
                None => "n/a".to_string(),
            };
            body.push_str(&format!(
                "| {} | `{}` | {} | {} |\n",
                event.run_id,
                event.lane,
                gb,
                markdown_cell(&event.title)
            ));
        }
        body.push('\n');
    }

    body.push_str("## Queue/wait observations\n\n");
    match report.queue_wait {
        Some((min, median, max)) => {
            body.push_str("Created-to-started queue wait per run:\n\n");
            body.push_str(&format!("- Min: {}\n", format_duration(min)));
            body.push_str(&format!("- Median: {}\n", format_duration(median)));
            body.push_str(&format!("- Max: {}\n\n", format_duration(max)));
        }
        None => {
            body.push_str("No queue-wait timestamps available from the API.\n\n");
        }
    }

    body.push_str("Reproduce locally:\n\n");
    body.push_str("```\n");
    body.push_str(&format!(
        "cargo xtask ci-budget --workflow \"{}\" --limit {}\n",
        report.workflow, report.limit
    ));
    body.push_str("```\n");
    body
}

fn markdown_cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_list_sample() -> &'static str {
        r#"[
            {"databaseId": 101, "displayTitle": "fix: scratch", "conclusion": "failure", "status": "completed", "event": "pull_request", "headBranch": "fix-a", "createdAt": "2026-06-07T10:00:00Z", "startedAt": "2026-06-07T10:00:30Z", "updatedAt": "2026-06-07T10:05:00Z", "attempt": 1},
            {"databaseId": 102, "displayTitle": "feat: thing", "conclusion": "failure", "status": "completed", "event": "pull_request", "headBranch": "feat-b", "createdAt": "2026-06-07T11:00:00Z", "startedAt": "2026-06-07T11:00:10Z", "updatedAt": "2026-06-07T11:20:00Z", "attempt": 1},
            {"databaseId": 103, "displayTitle": "docs: tweak", "conclusion": "success", "status": "completed", "event": "pull_request", "headBranch": "docs-c", "createdAt": "2026-06-07T12:00:00Z", "startedAt": "2026-06-07T12:00:05Z", "updatedAt": "2026-06-07T12:08:00Z", "attempt": 2}
        ]"#
    }

    fn jobs_cx43(prepare_failed: bool) -> RawJob {
        RawJob {
            name: "Ripr Rust Small on CX43".to_string(),
            conclusion: if prepare_failed {
                "failure".to_string()
            } else {
                "success".to_string()
            },
            status: "completed".to_string(),
            started_at: "2026-06-07T10:01:00Z".to_string(),
            completed_at: "2026-06-07T10:11:00Z".to_string(),
        }
    }

    fn sample_runs() -> Result<Vec<RawRun>, String> {
        let value: Value = serde_json::from_str(run_list_sample())
            .map_err(|err| format!("sample parse: {err}"))?;
        let mut runs = parse_runs_value(&value)?;
        // Newest-first ordering: 103, 102, 101.
        for run in &mut runs {
            match run.database_id {
                101 => {
                    run.jobs = vec![jobs_cx43(true)];
                    run.failed_log =
                        "ci-disk-guard: /mnt/ci-scratch has 12.3 GB free, needs 35 GB (exit 75: disk guard tripped)".to_string();
                }
                102 => {
                    run.jobs = vec![RawJob {
                        name: "Ripr Rust Small on CPX42".to_string(),
                        conclusion: "failure".to_string(),
                        status: "completed".to_string(),
                        started_at: "2026-06-07T11:01:00Z".to_string(),
                        completed_at: "2026-06-07T11:19:00Z".to_string(),
                    }];
                    run.failed_log =
                        "error[E0308]: test assertion failed\nthread 'tests::foo' panicked\ntest result: FAILED. 1 failed".to_string();
                }
                103 => {
                    run.jobs = vec![jobs_cx43(false)];
                }
                _ => {}
            }
        }
        Ok(runs)
    }

    #[test]
    fn lane_is_parsed_from_job_names() {
        assert_eq!(lane_of("Ripr Rust Small on CX43"), "cx43");
        assert_eq!(lane_of("Ripr Rust Small on CPX42"), "cpx42");
        assert_eq!(lane_of("Ripr Rust Small on CX53"), "cx53");
        assert_eq!(lane_of("Ripr Rust Small on GitHub Hosted"), "github");
        assert_eq!(lane_of("Route Ripr Rust Small"), "route");
        assert_eq!(lane_of("Ripr Rust Small Result"), "result");
        assert_eq!(lane_of("something else"), "other");
    }

    #[test]
    fn disk_guard_log_classifies_as_infra() {
        let log = "ci-disk-guard: /mnt/ci-scratch has 12.3 GB free, needs 35 GB (exit 75: disk guard tripped)";
        assert_eq!(classify_failure(log), "infra_disk_guard");
        assert_eq!(extract_disk_guard_gb_free(log), Some(12.3));
    }

    #[test]
    fn test_failure_log_classifies_as_product() {
        let log = "error[E0308]: mismatched types\nthread 'tests::foo' panicked\ntest result: FAILED. 1 failed";
        assert_eq!(classify_failure(log), "product");
        assert_eq!(extract_disk_guard_gb_free(log), None);
    }

    #[test]
    fn empty_log_classifies_as_unknown() {
        assert_eq!(classify_failure(""), "unknown");
    }

    #[test]
    fn aggregation_counts_conclusions_lanes_and_disk_guard() -> Result<(), String> {
        let runs = sample_runs()?;
        let report = build_report("Routed Rust Small", 40, &runs);

        assert_eq!(report.total_runs, 3);
        // Two failures, one success.
        assert_eq!(
            report.by_conclusion,
            vec![("failure".to_string(), 2), ("success".to_string(), 1)]
        );

        // Failure classification: one disk-guard (infra), one product.
        assert_eq!(report.failure_total, 2);
        assert_eq!(report.infra_disk_guard, 1);
        assert_eq!(report.product_failures, 1);
        assert_eq!(report.infra_other, 0);

        // Disk-guard event surfaced with the GB-free figure.
        assert_eq!(report.disk_guard_events.len(), 1);
        let event = report
            .disk_guard_events
            .first()
            .ok_or_else(|| "missing disk-guard event".to_string())?;
        assert_eq!(event.run_id, 101);
        assert_eq!(event.lane, "cx43");
        assert_eq!(event.gb_free, Some(12.3));
        Ok(())
    }

    #[test]
    fn fail_then_success_counts_as_one_tempfail_retry() -> Result<(), String> {
        let runs = sample_runs()?;
        let report = build_report("Routed Rust Small", 40, &runs);
        // Run 103 succeeded on attempt 2 -> exactly one tempfail-retry.
        assert_eq!(report.tempfail_retry_count, 1);
        assert_eq!(report.runs_with_reruns, 1);
        assert_eq!(report.extra_attempts, 1);
        // 1 + 1 + 2 attempts across the three runs.
        assert_eq!(report.total_attempts, 4);
        Ok(())
    }

    #[test]
    fn duration_and_queue_wait_are_aggregated_by_lane() -> Result<(), String> {
        let runs = sample_runs()?;
        let report = build_report("Routed Rust Small", 40, &runs);

        // Two impl lanes saw jobs: cx43 (101 + 103) and cpx42 (102).
        let cx43 = report
            .durations
            .iter()
            .find(|stat| stat.lane == "cx43")
            .ok_or_else(|| "missing cx43 durations".to_string())?;
        assert_eq!(cx43.count, 2);
        assert_eq!(cx43.min_seconds, 600); // 10 minutes each.
        assert_eq!(cx43.max_seconds, 600);

        let cpx42 = report
            .durations
            .iter()
            .find(|stat| stat.lane == "cpx42")
            .ok_or_else(|| "missing cpx42 durations".to_string())?;
        assert_eq!(cpx42.count, 1);
        assert_eq!(cpx42.median_seconds, 1080); // 18 minutes.

        // Queue wait present (created -> started deltas of 30s, 10s, 5s).
        let (min, _median, max) = report
            .queue_wait
            .ok_or_else(|| "missing queue wait".to_string())?;
        assert_eq!(min, 5);
        assert_eq!(max, 30);
        Ok(())
    }

    #[test]
    fn input_object_with_embedded_jobs_parses() -> Result<(), String> {
        let input = r#"{
            "runs": [
                {
                    "databaseId": 9,
                    "displayTitle": "scratch poisoned",
                    "conclusion": "failure",
                    "status": "completed",
                    "attempt": 1,
                    "createdAt": "2026-06-07T09:00:00Z",
                    "startedAt": "2026-06-07T09:00:20Z",
                    "jobs": [
                        {"name": "Ripr Rust Small on CX53", "conclusion": "failure", "status": "completed", "startedAt": "2026-06-07T09:01:00Z", "completedAt": "2026-06-07T09:02:00Z"}
                    ],
                    "failed_log": "ci-disk-guard /mnt/ci-scratch 8 GB free below 50 GB floor: disk guard exit 75"
                }
            ]
        }"#;
        let value: Value =
            serde_json::from_str(input).map_err(|err| format!("input parse: {err}"))?;
        let runs = parse_runs_value(&value)?;
        let report = build_report("Routed Rust Small", 10, &runs);
        assert_eq!(report.infra_disk_guard, 1);
        let event = report
            .disk_guard_events
            .first()
            .ok_or_else(|| "missing disk-guard event".to_string())?;
        assert_eq!(event.lane, "cx53");
        assert_eq!(event.gb_free, Some(8.0));
        Ok(())
    }

    #[test]
    fn markdown_reports_disk_guard_tax_and_is_advisory() -> Result<(), String> {
        let runs = sample_runs()?;
        let report = build_report("Routed Rust Small", 40, &runs);
        let markdown = report_markdown(&report);
        assert!(markdown.contains("Status: advisory"));
        assert!(markdown.contains("Disk-guard tempfail tax"));
        assert!(markdown.contains("issue #1058"));
        assert!(markdown.contains("infra_disk_guard"));
        Ok(())
    }
}
