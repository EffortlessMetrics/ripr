//! Public proof page (`cargo xtask public-proof`).
//!
//! Renders `docs/PUBLIC_PROOF.md`, the page a skeptical developer reads before
//! adopting ripr, from committed receipts under `metrics/public-proof/`.
//! Nothing on the page is typed by hand: every number, trend and shortfall
//! line is computed from a receipt, and a receipt that is missing a field the
//! page needs is a hard error, not a silent omission.
//!
//! `--check` fails when the committed page differs from what the receipts
//! render, or when a receipt has drifted from the canonical in-repo source it
//! was copied from. `--refresh-receipts` re-copies those canonical sources
//! before rendering. The xtask unit test that the required Rust gate runs
//! checks only the page against its receipts, so a PR that moves a canonical
//! source does not fail required CI; source drift is advisory.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;

const PAGE: &str = "docs/PUBLIC_PROOF.md";
const RECEIPTS: &str = "metrics/public-proof";
const CORPUS_MANIFEST: &str = "benchmarks/rust_corpus/manifest.json";
const CORPUS_RECEIPT: &str = "metrics/public-proof/corpus-manifest.json";

/// Receipts that are verbatim copies of a canonical in-repo output. The
/// canonical file must equal the receipt byte for byte, but only
/// `public-proof --check` compares them. The required unit test checks the page
/// against its receipts alone, so a PR that moves a source (a corpus update, say)
/// does not fail required CI; the page lags until someone refreshes it.
const CANONICAL_SOURCES: [(&str, &str); 3] = [
    ("dx-scoreboard.json", "metrics/dx-scoreboard/baseline.json"),
    (
        "verdict-corpus.json",
        "fixtures/rust-verdict-corpus/expected/report.json",
    ),
    ("corpus-manifest.json", CORPUS_MANIFEST),
];

/// Lane receipts the nightly scoreboard does not ingest. The page reads these
/// in place when a board would otherwise render as "not measured".
const CORPUS_FULL_BASELINE: &str = "metrics/dx-scoreboard/corpus-full-baseline.json";
const CORPUS_FAST_BASELINE: &str = "metrics/dx-scoreboard/corpus-fast-baseline.json";
const RANKING_BASELINE: &str = "metrics/dx-scoreboard/pilot-ranking-baseline.json";
const LANE_BASELINE_PATHS: [&str; 3] =
    [CORPUS_FULL_BASELINE, CORPUS_FAST_BASELINE, RANKING_BASELINE];

const NULL: &Value = &Value::Null;

/// Changed page lines shown when a receipt has drifted from its source.
const PREVIEW_LINES: usize = 12;

struct Options {
    check: bool,
    refresh_receipts: bool,
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let options = parse_options(args)?;
    let root = Path::new(".");
    if options.refresh_receipts {
        refresh_receipts(root)?;
    }
    let rendered = render(root)?;
    let page = root.join(PAGE);
    if options.check {
        check_receipts(root)?;
        check_page(root, &rendered)?;
        println!("{PAGE} matches its receipts, and the receipts match their sources.");
        return Ok(());
    }
    fs::write(&page, rendered).map_err(|err| format!("failed to write {PAGE}: {err}"))?;
    println!("wrote {PAGE}");
    Ok(())
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        check: false,
        refresh_receipts: false,
    };
    for arg in args {
        match arg.as_str() {
            "--check" => options.check = true,
            "--refresh-receipts" => options.refresh_receipts = true,
            other => {
                return Err(format!(
                    "unknown public-proof argument `{other}`; usage: cargo xtask public-proof [--check] [--refresh-receipts]"
                ));
            }
        }
    }
    if options.check && options.refresh_receipts {
        return Err(
            "`--check` only reads; run `--refresh-receipts` without it to update the receipts"
                .to_string(),
        );
    }
    Ok(options)
}

/// Fails with the next step when a receipt has drifted from its canonical source.
fn check_receipts(root: &Path) -> Result<(), String> {
    let drift = receipt_drift(root)?;
    if drift.is_empty() {
        return Ok(());
    }
    let preview = match refresh_preview(root) {
        Ok(lines) if lines.is_empty() => {
            "refreshing would not change the page text, only the receipts".to_string()
        }
        Ok(lines) => format!(
            "refreshing would change the page like this ({}):\n{}",
            if lines.len() > PREVIEW_LINES {
                format!("first {PREVIEW_LINES} changed lines")
            } else {
                "all changed lines".to_string()
            },
            lines
                .iter()
                .take(PREVIEW_LINES)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        ),
        // The drift is already the finding; a receipt the page cannot render
        // from is reported by the render step itself.
        Err(err) => format!("could not preview the refreshed page: {err}"),
    };
    Err(format!(
        "{}\n{preview}\nrun `cargo xtask public-proof --refresh-receipts`, then `cargo xtask public-proof`, and commit the result",
        drift.join("\n")
    ))
}

/// Changed page lines, `- old` then `+ new`, were the receipts refreshed from
/// their canonical sources. It renders in a scratch copy and writes nothing in
/// the repository, so whoever bumps a scoreboard sees which published numbers
/// move before committing.
fn refresh_preview(root: &Path) -> Result<Vec<String>, String> {
    // Unique per call: tests and callers in one process must not share a scratch.
    static SCRATCH_ID: AtomicUsize = AtomicUsize::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "ripr-public-proof-preview-{}-{}",
        std::process::id(),
        SCRATCH_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let receipts = scratch.join(RECEIPTS);
    fs::create_dir_all(&receipts)
        .map_err(|err| format!("failed to create {}: {err}", receipts.display()))?;
    let result = (|| {
        let entries = fs::read_dir(root.join(RECEIPTS))
            .map_err(|err| format!("failed to read {RECEIPTS}: {err}"))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("failed to read {RECEIPTS}: {err}"))?;
            fs::copy(entry.path(), receipts.join(entry.file_name()))
                .map_err(|err| format!("failed to copy {}: {err}", entry.path().display()))?;
        }
        for (receipt, source) in CANONICAL_SOURCES {
            fs::write(receipts.join(receipt), read_bytes(&root.join(source))?)
                .map_err(|err| format!("failed to write the {receipt} preview: {err}"))?;
        }
        copy_lane_baselines(root, &scratch)?;
        let refreshed = render(&scratch)?;
        let committed = fs::read_to_string(root.join(PAGE))
            .map_err(|err| format!("failed to read {PAGE}: {err}"))?;
        Ok(changed_lines(&committed, &refreshed))
    })();
    let _ = fs::remove_dir_all(&scratch);
    result
}

/// An ordered line diff, `- old` and `+ new`, in page order. It is a longest
/// common subsequence diff, so a moved or duplicated line is reported; blank
/// lines are not shown.
fn changed_lines(old: &str, new: &str) -> Vec<String> {
    let a_all: Vec<&str> = old.lines().collect();
    let b_all: Vec<&str> = new.lines().collect();
    // Drop the common head and tail first: a page edit is local, so the
    // quadratic table below only spans the lines in between.
    let head = a_all.iter().zip(&b_all).take_while(|(x, y)| x == y).count();
    let tail = a_all[head..]
        .iter()
        .rev()
        .zip(b_all[head..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let a = &a_all[head..a_all.len() - tail];
    let b = &b_all[head..b_all.len() - tail];
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            i += 1;
            j += 1;
        } else if j < b.len() && (i == a.len() || lcs[i][j + 1] > lcs[i + 1][j]) {
            if !b[j].is_empty() {
                out.push(format!("+ {}", b[j]));
            }
            j += 1;
        } else {
            if !a[i].is_empty() {
                out.push(format!("- {}", a[i]));
            }
            i += 1;
        }
    }
    out
}

/// Fails with the next step when the page no longer matches its receipts.
fn check_page(root: &Path, rendered: &str) -> Result<(), String> {
    let committed = fs::read_to_string(root.join(PAGE))
        .map_err(|err| format!("failed to read {PAGE}: {err}; run `cargo xtask public-proof`"))?;
    if committed != rendered {
        return Err(format!(
            "{PAGE} does not match its receipts; run `cargo xtask public-proof` and commit the result"
        ));
    }
    Ok(())
}

/// One line per receipt whose canonical source exists and differs.
fn receipt_drift(root: &Path) -> Result<Vec<String>, String> {
    let mut drift = Vec::new();
    for (receipt, source) in CANONICAL_SOURCES {
        let source_path = root.join(source);
        let canonical = read_bytes(&source_path)?;
        let copy = read_bytes(&root.join(RECEIPTS).join(receipt))?;
        if canonical != copy {
            drift.push(format!(
                "{RECEIPTS}/{receipt} differs from its canonical source {source}"
            ));
        }
    }
    Ok(drift)
}

fn copy_lane_baselines(from: &Path, to: &Path) -> Result<(), String> {
    for rel in LANE_BASELINE_PATHS {
        let source = from.join(rel);
        if !source.exists() {
            continue;
        }
        let target = to.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }
        fs::copy(&source, &target)
            .map_err(|err| format!("failed to copy {rel} into the preview: {err}"))?;
    }
    Ok(())
}

fn refresh_receipts(root: &Path) -> Result<(), String> {
    for (receipt, source) in CANONICAL_SOURCES {
        let source_path = root.join(source);
        let bytes = read_bytes(&source_path)?;
        let target: PathBuf = root.join(RECEIPTS).join(receipt);
        fs::write(&target, bytes)
            .map_err(|err| format!("failed to write {}: {err}", target.display()))?;
        println!("refreshed {RECEIPTS}/{receipt} from {source}");
    }
    Ok(())
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|err| format!("failed to read {}: {err}", path.display()))
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = read_bytes(path)?;
    serde_json::from_slice(&bytes).map_err(|err| format!("{} is not JSON: {err}", path.display()))
}

struct LaneReceipt {
    path: String,
    value: Value,
}

struct Receipts {
    dx: Value,
    verdicts: Value,
    mutation: Value,
    first_previous: Value,
    first_current: Value,
    agent: Value,
    install: Value,
    corpus: Value,
    corpus_lane: Option<LaneReceipt>,
    ranking_lane: Option<LaneReceipt>,
}

fn load(root: &Path) -> Result<Receipts, String> {
    let receipt = |name: &str| read_json(&root.join(RECEIPTS).join(name));
    Ok(Receipts {
        dx: receipt("dx-scoreboard.json")?,
        verdicts: receipt("verdict-corpus.json")?,
        mutation: receipt("mutation-spot-check.json")?,
        first_previous: receipt("first-run-previous.json")?,
        first_current: receipt("first-run-current.json")?,
        agent: receipt("agent-as-user.json")?,
        install: receipt("install.json")?,
        corpus: receipt("corpus-manifest.json")?,
        corpus_lane: load_lane(root, CORPUS_FULL_BASELINE, Some(CORPUS_FAST_BASELINE))?,
        ranking_lane: load_lane(root, RANKING_BASELINE, None)?,
    })
}

fn load_lane(
    root: &Path,
    preferred: &str,
    fallback: Option<&str>,
) -> Result<Option<LaneReceipt>, String> {
    for path in [Some(preferred), fallback].into_iter().flatten() {
        let full = root.join(path);
        if full.exists() {
            return Ok(Some(LaneReceipt {
                path: path.to_string(),
                value: read_json(&full)?,
            }));
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// JSON access. A missing field the page depends on is an error naming it.

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.get(key).unwrap_or(NULL)
}

fn items<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    field(value, key)
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn req_arr<'a>(value: &'a Value, key: &str, ctx: &str) -> Result<&'a [Value], String> {
    field(value, key)
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{ctx}: missing array `{key}`"))
}

fn req_str(value: &Value, key: &str, ctx: &str) -> Result<String, String> {
    field(value, key)
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{ctx}: missing string `{key}`"))
}

fn req_f64(value: &Value, key: &str, ctx: &str) -> Result<f64, String> {
    field(value, key)
        .as_f64()
        .ok_or_else(|| format!("{ctx}: missing number `{key}`"))
}

fn req_u64(value: &Value, key: &str, ctx: &str) -> Result<u64, String> {
    field(value, key)
        .as_u64()
        .ok_or_else(|| format!("{ctx}: missing count `{key}`"))
}

/// A count in a sparse outcome map. The producer omits outcomes that never
/// occurred, so an absent key is zero; an absent or non-object map is an error.
fn sparse_count(counts: &Value, key: &str, ctx: &str) -> Result<u64, String> {
    let map = counts
        .as_object()
        .ok_or_else(|| format!("{ctx}: missing outcome counts"))?;
    match map.get(key) {
        None => Ok(0),
        Some(value) => value
            .as_u64()
            .ok_or_else(|| format!("{ctx}: outcome `{key}` is not a count")),
    }
}

/// A string the page may legitimately leave empty (an optional label), never a
/// value the page prints as evidence.
fn text(value: &Value, key: &str) -> String {
    field(value, key).as_str().unwrap_or("").to_string()
}

fn item_text(value: &Value) -> String {
    if let Some(s) = value.as_str() {
        return s.to_string();
    }
    if let Some(s) = value.get("code").and_then(Value::as_str) {
        return s.to_string();
    }
    value.to_string()
}

/// A rate recorded as `{numerator, denominator, rate}` with `rate` a decimal string.
struct Rate {
    numerator: u64,
    denominator: u64,
    rate: f64,
}

fn req_rate(value: &Value, key: &str, ctx: &str) -> Result<Rate, String> {
    let node = field(value, key);
    let numerator = field(node, "numerator")
        .as_u64()
        .ok_or_else(|| format!("{ctx}: `{key}` has no numerator"))?;
    let denominator = field(node, "denominator")
        .as_u64()
        .ok_or_else(|| format!("{ctx}: `{key}` has no denominator"))?;
    let rate = field(node, "rate")
        .as_str()
        .and_then(|raw| raw.parse::<f64>().ok())
        .ok_or_else(|| format!("{ctx}: `{key}` has no rate"))?;
    Ok(Rate {
        numerator,
        denominator,
        rate,
    })
}

// ---------------------------------------------------------------------------
// Formatting

fn num(value: f64) -> String {
    if (value - value.round()).abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

fn percent(ratio: f64) -> String {
    format!("{:.1}%", ratio * 100.0)
}

fn fmt_value(value: f64, unit: &str) -> String {
    match unit {
        "ms" => format!("{:.1} s", value / 1000.0),
        "MB" => format!("{value:.0} MB"),
        "ratio" => percent(value),
        "s" => format!("{} s", num(value)),
        "" => num(value),
        "flag" => if value > 0.5 { "yes" } else { "no" }.to_string(),
        // `1 findings` reads as a typo; the units are plural nouns.
        other if (value - 1.0).abs() < f64::EPSILON && other.ends_with('s') => {
            format!("1 {}", other.trim_end_matches('s'))
        }
        other => format!("{} {other}", num(value)),
    }
}

fn fmt_delta(delta: f64, unit: &str) -> String {
    let sign = if delta > 0.0 {
        "+"
    } else if delta < 0.0 {
        "-"
    } else {
        ""
    };
    format!("{sign}{}", fmt_value(delta.abs(), unit))
}

fn cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn short(sha: &str, len: usize) -> String {
    sha.chars().take(len).collect()
}

/// `ripr 0.11.0 (a7a089e1c51d...)` becomes `ripr 0.11.0 (a7a089e)`.
fn short_version(version: &str) -> String {
    match version.split_once('(') {
        Some((head, rest)) => {
            let sha = rest.trim_end_matches(')');
            format!("{}({})", head, short(sha, 7))
        }
        None => version.to_string(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Meets,
    Below,
    NotMeasured,
    /// The producer recorded an instrument failure for this metric.
    Failed,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Self::Meets => "meets the bar",
            Self::Below => "below the bar",
            Self::NotMeasured => "not measured",
            Self::Failed => "instrument failed",
        }
    }
}

struct Page(String);

impl Page {
    fn line(&mut self, line: impl AsRef<str>) {
        self.0.push_str(line.as_ref());
        self.0.push('\n');
    }

    fn blank(&mut self) {
        self.0.push('\n');
    }

    fn table(&mut self, head: &[&str], rows: &[Vec<String>]) {
        self.line(format!("| {} |", head.join(" | ")));
        self.line(format!("| {} |", vec!["---"; head.len()].join(" | ")));
        for row in rows {
            let cells: Vec<String> = row.iter().map(|c| cell(c)).collect();
            self.line(format!("| {} |", cells.join(" | ")));
        }
        self.blank();
    }
}

// ---------------------------------------------------------------------------
// Bars: one row per scoreboard metric, whatever receipt it comes from.

struct Bar {
    id: String,
    board: String,
    title: String,
    value: Option<f64>,
    unit: String,
    target: f64,
    lower_is_better: bool,
    status: Status,
    trend: String,
    basis: String,
}

fn board_name(id: &str) -> &str {
    match id {
        "speed" => "Speed and memory",
        "ci" => "CI adoption",
        "trust" => "Trust",
        "paste" => "Pasted commands",
        "first_run" => "First run",
        other => other,
    }
}

fn meets(value: f64, target: f64, lower_is_better: bool) -> bool {
    if lower_is_better {
        value <= target
    } else {
        value >= target
    }
}

fn is_unknown_verdict(verdict: &str) -> bool {
    verdict.ends_with("_unknown")
}

fn steps_of(case: &Value) -> &[Value] {
    items(case, "steps")
}

fn all_steps(receipt: &Value) -> Vec<&Value> {
    items(receipt, "cases").iter().flat_map(steps_of).collect()
}

fn friction_of(step: &Value) -> &[Value] {
    items(step, "friction")
}

/// First-run bars computed from a raw `first_run.v1` receipt, so they stay
/// comparable across the two receipts the page shows.
fn first_run_value(id: &str, receipt: &Value) -> Option<f64> {
    let steps = all_steps(receipt);
    let count = |n: usize| -> Option<f64> { u32::try_from(n).ok().map(f64::from) };
    match id {
        "first_run.unknown_verdicts" => count(
            items(receipt, "cases")
                .iter()
                .filter(|case| is_unknown_verdict(&text(case, "verdict")))
                .count(),
        ),
        "first_run.friction_events" => count(steps.iter().map(|s| friction_of(s).len()).sum()),
        "first_run.failed_steps" => count(
            steps
                .iter()
                .filter(|s| field(s, "exit").as_i64() != Some(0))
                .count(),
        ),
        "first_run.over_budget_steps" => count(
            steps
                .iter()
                .filter(|s| {
                    friction_of(s)
                        .iter()
                        .any(|f| f.as_str().is_some_and(|t| t.contains("budget")))
                })
                .count(),
        ),
        // A step with no numeric duration makes the walk's total unknowable, so the
        // metric is not measured rather than summed over the steps that were timed.
        "first_run.walk_secs" => items(receipt, "cases")
            .iter()
            .map(|case| {
                let steps = steps_of(case);
                if steps.is_empty() {
                    return None;
                }
                steps
                    .iter()
                    .map(|s| field(s, "secs").as_f64())
                    .sum::<Option<f64>>()
            })
            .collect::<Option<Vec<f64>>>()?
            .into_iter()
            .reduce(f64::max),
        _ => None,
    }
}

struct Derived {
    value: Option<f64>,
    trend: String,
    basis: String,
}

fn derived(id: &str, r: &Receipts) -> Result<Option<Derived>, String> {
    let first_run = |basis: &str| -> Derived {
        let value = first_run_value(id, &r.first_current);
        let previous = first_run_value(id, &r.first_previous);
        let before_label = short_version(&text(&r.first_previous, "ripr"));
        let trend = match (value, previous) {
            (Some(now), Some(before)) if (now - before).abs() < 1e-9 => {
                format!("unchanged from {before_label} ({})", fmt_value(before, ""))
            }
            (Some(_), Some(before)) => {
                format!("was {} in {before_label}", fmt_value(before, ""))
            }
            _ => "no earlier receipt".to_string(),
        };
        Derived {
            value,
            trend,
            basis: basis.to_string(),
        }
    };
    let first_receipt = |value: f64, basis: &str| -> Option<Derived> {
        Some(Derived {
            value: Some(value),
            trend: format!("first receipt ({basis})"),
            basis: basis.to_string(),
        })
    };
    Ok(match id {
        "trust.false_verdict_rate" => {
            let rate = origin_rate(&r.verdicts, "upstream", "false_verdict_rate")?;
            let all = req_rate(&r.verdicts, "false_verdict_rate", "verdict-corpus")?;
            first_receipt(
                rate.rate,
                &format!(
                    "verdict corpus, upstream cases only; all {} cases: {}",
                    all.denominator,
                    percent(all.rate)
                ),
            )
        }
        "trust.discriminator_claim_agreement" => first_receipt(
            req_f64(
                field(
                    field(&r.mutation, "scored_families"),
                    "claims_discriminator",
                ),
                "agreement_rate",
                "mutation-spot-check",
            )?,
            &scored_basis(&r.mutation, "claims_discriminator", "strongly_gripped")?,
        ),
        "trust.gap_claim_agreement" => first_receipt(
            req_f64(
                field(
                    field(&r.mutation, "scored_families"),
                    "claims_no_discriminator",
                ),
                "agreement_rate",
                "mutation-spot-check",
            )?,
            &scored_basis(&r.mutation, "claims_no_discriminator", "ungripped")?,
        ),
        "trust.mutation_join_coverage" => {
            let (precise, mutants) = mutation_join_totals(&r.mutation)?;
            first_receipt(
                if mutants == 0.0 {
                    0.0
                } else {
                    precise / mutants
                },
                "mutation spot check",
            )
        }
        "first_run.unknown_verdicts"
        | "first_run.friction_events"
        | "first_run.failed_steps"
        | "first_run.over_budget_steps"
        | "first_run.walk_secs" => Some(first_run("first-run receipts")),
        _ => None,
    })
}

/// Seam-precise joins and total mutants across the spot-check repositories.
fn mutation_join_totals(mutation: &Value) -> Result<(f64, f64), String> {
    let mut precise = 0.0;
    let mut mutants = 0.0;
    for repo in req_arr(mutation, "repos", "mutation-spot-check")? {
        let ctx = "mutation-spot-check repo";
        precise += req_f64(field(repo, "pairings"), "seam_precise", ctx)?;
        mutants += req_f64(field(repo, "calibration_metrics"), "mutants_total", ctx)?;
    }
    Ok((precise, mutants))
}

/// Basis text for an agreement bar: how many seam-precise mutants of the
/// grip class the rate actually scored, so an excluded outcome is visible.
fn scored_basis(mutation: &Value, family: &str, grip_class: &str) -> Result<String, String> {
    let scored = req_f64(
        field(field(mutation, "scored_families"), family),
        "mutants_scored",
        "mutation-spot-check family",
    )?;
    let class = field(
        field(
            field(mutation, "outcomes_by_pairing_and_grip_class"),
            "seam_precise",
        ),
        grip_class,
    );
    let Some(counts) = class.as_object() else {
        return Err(format!(
            "mutation-spot-check is missing seam_precise outcomes for {grip_class}"
        ));
    };
    let total: f64 = counts.values().filter_map(Value::as_f64).sum();
    Ok(format!(
        "mutation spot check, {} of {} seam-precise `{grip_class}` mutants scored; {} unscored",
        num(scored),
        num(total),
        num(total - scored)
    ))
}

/// Mutants that enter an agreement rate, summed over the scored families.
fn mutation_scored_total(mutation: &Value) -> Result<f64, String> {
    let families = field(mutation, "scored_families");
    let Some(map) = families.as_object() else {
        return Err("mutation-spot-check is missing scored_families".to_string());
    };
    let mut scored = 0.0;
    for family in map.values() {
        scored += req_f64(family, "mutants_scored", "mutation-spot-check family")?;
    }
    Ok(scored)
}

/// Boards the nightly scoreboard receipt defines but does not ingest.
/// An unmeasured row here is omitted unless a lane baseline supplies a number.
fn foreign_scoreboard_board(board: &str) -> bool {
    matches!(board, "agent" | "corpus" | "ranking")
}

fn metric_lacks_number(metric: &Value) -> bool {
    field(metric, "value").as_f64().is_none() && text(metric, "status") != "failed"
}

fn lane_measurement<'a>(id: &str, r: &'a Receipts) -> Option<(&'a Value, &'a LaneReceipt)> {
    for lane in r.corpus_lane.iter().chain(r.ranking_lane.iter()) {
        if let Some(metric) = items(&lane.value, "metrics")
            .iter()
            .find(|metric| text(metric, "id") == id)
            && (field(metric, "value").as_f64().is_some() || text(metric, "status") == "failed")
        {
            return Some((metric, lane));
        }
    }
    None
}

fn runner_class_differs(reason: &str) -> bool {
    reason.contains("runner class differs")
}

fn quoted_after<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = text.split_once(prefix)?.1;
    let class = rest.split_once('`')?.0;
    if class.is_empty() { None } else { Some(class) }
}

fn comparable_trend(delta: f64, before: f64, unit: &str, baseline_rev: &str) -> String {
    if delta == 0.0 {
        format!(
            "unchanged since {baseline_rev} ({})",
            fmt_value(before, unit)
        )
    } else if unit == "flag" {
        format!(
            "changed since {baseline_rev} (was {})",
            fmt_value(before, unit)
        )
    } else {
        format!(
            "{} since {baseline_rev} (was {})",
            fmt_delta(delta, unit),
            fmt_value(before, unit)
        )
    }
}

fn cross_class_trend(before: Option<f64>, unit: &str, reason: &str) -> String {
    let class = quoted_after(reason, "baseline `")
        .map(|class| format!(" `{class}`"))
        .unwrap_or_default();
    match before {
        Some(value) => format!(
            "earlier receipt on another runner class{class} (was {})",
            fmt_value(value, unit)
        ),
        None => format!("earlier receipt on another runner class{class}"),
    }
}

/// Trend for a scoreboard or lane metric. A same-class baseline is compared.
/// An earlier receipt on another runner class is named, not treated as absent.
fn scoreboard_trend(metric: &Value, baseline_rev: &str) -> String {
    let Some(base) = metric.get("baseline") else {
        return "no earlier measurement".to_string();
    };
    let comparable = base.get("comparable").and_then(Value::as_bool) == Some(true);
    let before = base.get("value").and_then(Value::as_f64);
    let delta = base.get("delta").and_then(Value::as_f64);
    let unit = text(metric, "unit");
    let reason = text(base, "reason");
    if comparable {
        return match (before, delta) {
            (Some(before), Some(delta)) => comparable_trend(delta, before, &unit, baseline_rev),
            _ => "no earlier measurement".to_string(),
        };
    }
    if runner_class_differs(&reason)
        && (before.is_some() || field(metric, "value").as_f64().is_some())
    {
        return cross_class_trend(before, &unit, &reason);
    }
    "no earlier measurement".to_string()
}

fn bars(r: &Receipts) -> Result<Vec<Bar>, String> {
    let baseline_rev = short(
        &req_str(
            field(&r.dx, "gate"),
            "baseline_revision",
            "dx-scoreboard gate",
        )?,
        7,
    );
    let mut out = Vec::new();
    for metric in req_arr(&r.dx, "metrics", "dx-scoreboard")? {
        let id = req_str(metric, "id", "dx-scoreboard metric")?;
        let mut unit = text(metric, "unit");
        let mut target = req_f64(metric, "target", &id)?;
        let mut lower_is_better = text(metric, "direction") != "higher_is_better";
        let mut value = field(metric, "value").as_f64();
        let mut trend = scoreboard_trend(metric, &baseline_rev);
        let mut basis = text(metric, "source");
        let mut is_derived = false;
        let mut status_src = metric;
        if let Some(derived) = derived(&id, r)? {
            is_derived = true;
            value = derived.value;
            trend = derived.trend;
            basis = derived.basis;
        } else if metric_lacks_number(metric) {
            if let Some((lane_metric, lane)) = lane_measurement(&id, r) {
                status_src = lane_metric;
                unit = text(lane_metric, "unit");
                target = req_f64(lane_metric, "target", &id)?;
                lower_is_better = text(lane_metric, "direction") != "higher_is_better";
                value = field(lane_metric, "value").as_f64();
                let lane_rev = short(&text(field(&lane.value, "gate"), "baseline_revision"), 7);
                trend = scoreboard_trend(lane_metric, &lane_rev);
                basis = lane.path.clone();
            } else if foreign_scoreboard_board(&text(metric, "board")) {
                continue;
            }
        }
        let mut status = match value {
            Some(v) if meets(v, target, lower_is_better) => Status::Meets,
            Some(_) => Status::Below,
            None => Status::NotMeasured,
        };
        // The producer's own status outranks a number recomputed from its worst
        // successful sample: a failed instrument or an incomplete measurement
        // can leave a value that happens to meet the target.
        if !is_derived {
            match text(status_src, "status").as_str() {
                "failed" => status = Status::Failed,
                "below_target" if status == Status::Meets => status = Status::Below,
                _ => {}
            }
        }
        if status == Status::NotMeasured || status == Status::Failed {
            let reason = unmeasured_reason(status_src);
            // A derived metric has no producer reason; keep what it was derived from
            // and say why the number is missing.
            basis = if is_derived && status == Status::NotMeasured {
                format!("{basis}: an untimed or empty step list leaves no total")
            } else {
                reason
            };
            if status == Status::Failed {
                basis = format!("instrument failed: {basis}");
            }
        }
        let title = if id == "trust.mutation_join_coverage" {
            let scored = mutation_scored_total(&r.mutation)?;
            let (_, mutants) = mutation_join_totals(&r.mutation)?;
            format!(
                "Real mutants that join a ripr seam precisely (join coverage; {} of {} enter an agreement rate)",
                num(scored),
                num(mutants)
            )
        } else if id == "trust.false_verdict_rate" {
            "Wrong verdicts on hand-checked changes from real repositories".to_string()
        } else {
            text(status_src, "title")
        };
        out.push(Bar {
            board: board_name(&text(status_src, "board")).to_string(),
            id,
            title,
            value,
            unit,
            target,
            lower_is_better,
            status,
            trend,
            basis,
        });
    }
    Ok(out)
}

fn bar_row(bar: &Bar) -> Vec<String> {
    let value = bar
        .value
        .map_or_else(|| "not measured".to_string(), |v| fmt_value(v, &bar.unit));
    let value = if bar.status == Status::Below {
        format!("**{value}**")
    } else {
        value
    };
    vec![
        bar.board.clone(),
        if bar.title.is_empty() {
            bar.id.clone()
        } else {
            bar.title.clone()
        },
        value,
        format!(
            "{} {}",
            if bar.lower_is_better { "<=" } else { ">=" },
            fmt_value(bar.target, &bar.unit)
        ),
        bar.status.label().to_string(),
        bar.trend.clone(),
    ]
}

// ---------------------------------------------------------------------------
// Rendering

fn render(root: &Path) -> Result<String, String> {
    let r = load(root)?;
    let bars = bars(&r)?;
    let mut page = Page(String::new());
    header(&mut page, &r)?;
    scoreboard(&mut page, &bars);
    shortfalls(&mut page, &r, &bars)?;
    mutation_section(&mut page, &r.mutation)?;
    verdict_section(&mut page, &r.verdicts)?;
    speed_section(&mut page, &r.dx)?;
    first_run_section(&mut page, &r.first_previous, &r.first_current, &r.install)?;
    agent_section(&mut page, &r.agent)?;
    corpus_section(&mut page, &r)?;
    boundaries(&mut page, &r)?;
    reproduce(&mut page);
    Ok(page.0)
}

fn header(page: &mut Page, r: &Receipts) -> Result<(), String> {
    page.line("# Public proof");
    page.blank();
    page.line("<!-- Generated by `cargo xtask public-proof`. Do not edit by hand. -->");
    page.blank();
    page.line("This page is for a developer deciding whether to trust ripr. Every number comes from a committed receipt and sits next to the bar we set for it. Each receipt names what it was measured on where it records that, and the sections say where it does not. Where ripr misses a bar, the page says so before it says anything else.");
    page.blank();
    page.line("The page is generated. A unit test that CI requires fails when the page no longer matches its receipts. `cargo xtask public-proof --check` also fails when a receipt has drifted from the in-repo output it was copied from; that comparison is advisory and is not part of required CI, so the page can lag the corpus until someone refreshes it.");
    page.blank();
    page.line("## Receipts");
    page.blank();
    let mutation_repos = req_arr(&r.mutation, "repos", "mutation-spot-check")?;
    let mutation_tools: Vec<String> = {
        let mut seen: Vec<String> = Vec::new();
        for repo in mutation_repos {
            let version = req_str(repo, "cargo_mutants_version", "mutation-spot-check repo")?;
            if !seen.contains(&version) {
                seen.push(version);
            }
        }
        seen
    };
    let mut rows = vec![
        vec![
            "`metrics/public-proof/dx-scoreboard.json`".to_string(),
            "Speed, memory, CI adoption, pasted-command safety, self-contradictions".to_string(),
            short_version(&req_str(&r.dx, "analyzer_version", "dx-scoreboard")?),
            format!(
                "runner `{}`",
                req_str(&r.dx, "runner_class", "dx-scoreboard")?
            ),
        ],
        r.corpus_lane
            .as_ref()
            .map(|lane| {
                vec![
                    format!("`{}`", lane.path),
                    "Corpus lane, used when the scoreboard receipt did not ingest corpus"
                        .to_string(),
                    lane_revision(&lane.value),
                    runner_detail(&lane.value),
                ]
            })
            .unwrap_or_default(),
        r.ranking_lane
            .as_ref()
            .map(|lane| {
                vec![
                    format!("`{}`", lane.path),
                    "Pilot ranking lane, used when the scoreboard receipt did not ingest ranking"
                        .to_string(),
                    lane_revision(&lane.value),
                    runner_detail(&lane.value),
                ]
            })
            .unwrap_or_default(),
        vec![
            "`metrics/public-proof/verdict-corpus.json`".to_string(),
            "Hand-labeled verdict corpus".to_string(),
            format!(
                "corpus {}",
                req_str(&r.verdicts, "corpus_version", "verdict-corpus")?
            ),
            req_str(&r.verdicts, "spec", "verdict-corpus")?,
        ],
        vec![
            "`metrics/public-proof/mutation-spot-check.json`".to_string(),
            "Agreement with real mutation runs".to_string(),
            format!("{} repositories at pinned revisions", mutation_repos.len()),
            format!("cargo-mutants {}", mutation_tools.join(", ")),
        ],
        vec![
            "`metrics/public-proof/first-run-previous.json`".to_string(),
            "New-developer walk, earlier release".to_string(),
            short_version(&req_str(&r.first_previous, "ripr", "first-run-previous")?),
            format!(
                "{} crates",
                req_arr(&r.first_previous, "cases", "first-run-previous")?.len()
            ),
        ],
        vec![
            "`metrics/public-proof/first-run-current.json`".to_string(),
            "New-developer walk, later build".to_string(),
            short_version(&req_str(&r.first_current, "ripr", "first-run-current")?),
            format!(
                "{} crates",
                req_arr(&r.first_current, "cases", "first-run-current")?.len()
            ),
        ],
        vec![
            "`metrics/public-proof/agent-as-user.json`".to_string(),
            "An agent using only ripr's help to close a real test gap".to_string(),
            req_str(&r.agent, "source", "agent-as-user")?,
            req_str(&r.agent, "evidence", "agent-as-user")?,
        ],
        vec![
            "`metrics/public-proof/install.json`".to_string(),
            "Time to install a prebuilt release".to_string(),
            req_str(&r.install, "source", "install")?,
            "one cloud container, not hosted CI".to_string(),
        ],
        vec![
            format!("`{CORPUS_RECEIPT}`"),
            "Pinned reference corpus of real repositories".to_string(),
            format!(
                "corpus {}",
                req_str(&r.corpus, "corpus_version", CORPUS_MANIFEST)?
            ),
            "copy of the pinned manifest".to_string(),
        ],
    ];
    rows.retain(|row| !row.is_empty());
    page.table(&["Receipt", "Measures", "Revision", "Detail"], &rows);
    Ok(())
}

fn scoreboard(page: &mut Page, bars: &[Bar]) {
    let total = bars.len();
    let met = bars.iter().filter(|b| b.status == Status::Meets).count();
    let below = bars.iter().filter(|b| b.status == Status::Below).count();
    let unmeasured = bars
        .iter()
        .filter(|b| b.status == Status::NotMeasured)
        .count();
    let failed = bars.iter().filter(|b| b.status == Status::Failed).count();
    let failed_note = if failed == 0 {
        String::new()
    } else {
        format!(", has {failed} whose instrument failed")
    };
    page.line("## Scoreboard");
    page.blank();
    page.line(scoreboard_summary(
        total,
        met,
        below,
        unmeasured,
        failed_note,
        bars,
    ));
    page.blank();
    let rows: Vec<Vec<String>> = bars.iter().map(bar_row).collect();
    page.table(&["Board", "Bar", "Now", "Target", "Status", "Trend"], &rows);
    let unmeasured_rows: Vec<&Bar> = bars
        .iter()
        .filter(|b| matches!(b.status, Status::NotMeasured | Status::Failed))
        .collect();
    if !unmeasured_rows.is_empty() {
        page.line(if failed == 0 {
            "Not measured, and why:"
        } else {
            "Not measured or failed, and why:"
        });
        page.blank();
        for bar in unmeasured_rows {
            page.line(format!("- `{}`: {}", bar.id, bar.basis));
        }
        page.blank();
    }
}

/// A verdict-corpus rate for one case origin ("upstream" or "authored").
/// Authored cases were written to fill empty cells, so only the upstream
/// rates describe tests in real repositories.
fn origin_rate(verdicts: &Value, origin: &str, key: &str) -> Result<Rate, String> {
    let ctx = format!("verdict-corpus by_origin {origin}");
    let by_origin = field(verdicts, "by_origin");
    if by_origin.get(origin).is_none() {
        return Err(format!("verdict-corpus is missing by_origin.{origin}"));
    }
    req_rate(field(by_origin, origin), key, &ctx)
}

fn shortfalls(page: &mut Page, r: &Receipts, bars: &[Bar]) -> Result<(), String> {
    page.line("## Where ripr falls short");
    page.blank();
    page.line("Each line below is computed from the receipts above. Detail sections follow.");
    page.blank();

    let rows = req_arr(&r.verdicts, "rows", "verdict-corpus")?;
    let false_actionable = origin_rate(&r.verdicts, "upstream", "false_actionable_rate")?;
    let authored_false_actionable = origin_rate(&r.verdicts, "authored", "false_actionable_rate")?;
    let false_cases: Vec<String> = rows
        .iter()
        .filter(|row| {
            text(row, "outcome") == "false_actionable" && text(row, "origin") == "upstream"
        })
        .map(|row| format!("`{}`", text(row, "case_id")))
        .collect();
    page.line(format!(
        "- **Wrong gaps.** On changes from real repositories ripr reported a gap on {} of {} whose tests caught every listed mutant ({}): {}. On the authored cases, which were written to fill empty corpus cells, it did so on {} of {} ({}).",
        false_actionable.numerator,
        false_actionable.denominator,
        percent(false_actionable.rate),
        false_cases.join(", "),
        authored_false_actionable.numerator,
        authored_false_actionable.denominator,
        percent(authored_false_actionable.rate)
    ));

    let abstention = origin_rate(&r.verdicts, "upstream", "abstention_rate")?;
    let authored_abstention = origin_rate(&r.verdicts, "authored", "abstention_rate")?;
    page.line(format!(
        "- **Mostly unsure.** On real-repository changes it abstained on {} of {} cases ({}); on the authored cases, {} of {} ({}). Abstaining is the safe failure, but each abstention is a change ripr gave the developer no help on.",
        abstention.numerator,
        abstention.denominator,
        percent(abstention.rate),
        authored_abstention.numerator,
        authored_abstention.denominator,
        percent(authored_abstention.rate)
    ));

    let families = field(&r.mutation, "scored_families");
    let no_disc = field(families, "claims_no_discriminator");
    let counts = field(no_disc, "counts");
    let agree = sparse_count(
        counts,
        "agree",
        "mutation-spot-check claims_no_discriminator",
    )?;
    let false_gap = sparse_count(
        counts,
        "false_gap",
        "mutation-spot-check claims_no_discriminator",
    )?;
    let false_gap_repos: Vec<String> = {
        let mut seen: Vec<String> = Vec::new();
        for example in items(field(&r.mutation, "disagreement_examples"), "false_gap") {
            let repo = text(example, "repo");
            if !seen.contains(&repo) {
                seen.push(repo);
            }
        }
        seen
    };
    page.line(format!(
        "- **Real mutants disagree with \"no test would notice\".** Of {} mutants on seams ripr called ungripped, real mutation testing caught {} that ripr said nothing would catch; ripr agreed on {}. The {} recorded examples are all in: {}.",
        agree + false_gap,
        false_gap,
        agree,
        items(field(&r.mutation, "disagreement_examples"), "false_gap").len(),
        if false_gap_repos.is_empty() {
            "none recorded".to_string()
        } else {
            false_gap_repos.join(", ")
        }
    ));

    let (precise, mutants) = mutation_join_totals(&r.mutation)?;
    let scored = mutation_scored_total(&r.mutation)?;
    page.line(format!(
        "- **Thin ground truth.** Only {} of {} mutants ({}) enter an agreement rate. {} join a ripr seam precisely, and {} of those still do not enter a rate.",
        num(scored),
        num(mutants),
        percent(if mutants == 0.0 { 0.0 } else { scored / mutants }),
        num(precise),
        num(precise - scored)
    ));

    for bar in bars.iter().filter(|b| {
        matches!(b.status, Status::Below | Status::Failed) && b.board == "Speed and memory"
    }) {
        let worst = worst_sample(&r.dx, &bar.id).unwrap_or_else(|| {
            bar.value
                .map_or_else(|| "not measured".to_string(), |v| fmt_value(v, &bar.unit))
        });
        // A failed metric with no failed sample (the producer recorded only a
        // reason) must not name a measured repository as the failed one.
        let (lead, worst) = if bar.status == Status::Failed {
            let named = worst.contains("(instrument failed)");
            let detail = if named {
                worst
            } else {
                bar.basis
                    .trim_start_matches("instrument failed: ")
                    .to_string()
            };
            ("Instrument failed", detail)
        } else {
            ("Worst repository", worst)
        };
        page.line(format!(
            "- **{}.** {lead}: {worst}; the bar is {} {}.",
            bar.title,
            if bar.lower_is_better {
                "at most"
            } else {
                "at least"
            },
            fmt_value(bar.target, &bar.unit),
        ));
    }

    let unknown_now = bars
        .iter()
        .find(|b| b.id == "first_run.unknown_verdicts")
        .and_then(|b| b.value);
    let cases_now = items(&r.first_current, "cases").len();
    if let Some(unknown) = unknown_now {
        let unknown_before = first_run_value("first_run.unknown_verdicts", &r.first_previous)
            .ok_or("first-run-previous: no unknown-verdict count")?;
        page.line(format!(
            "- **First-run verdicts are unresolved.** {} of {} first-run crates ended in an `*_unknown` verdict on {}; {} of {} did on {}.",
            num(unknown),
            cases_now,
            short_version(&text(&r.first_current, "ripr")),
            num(unknown_before),
            items(&r.first_previous, "cases").len(),
            short_version(&text(&r.first_previous, "ripr"))
        ));
    }

    let dx_repos = items(&r.dx, "repos");
    let manifest = items(&r.corpus, "repos");
    let on_manifest = dx_repos
        .iter()
        .filter(|repo| {
            manifest.iter().any(|entry| {
                text(entry, "id") == text(repo, "id") && text(entry, "sha") == text(repo, "sha")
            })
        })
        .count();
    page.line(format!(
        "- **Narrow coverage.** The speed scoreboard measured {} repositories and the first-run walk {cases_now} crates. The corpus manifest pins {} repositories; {on_manifest} of the scoreboard's repositories appear in it at the same revision; the others were measured at a different revision or are not in the manifest.",
        dx_repos.len(),
        manifest.len(),
    ));
    page.blank();
    Ok(())
}

/// Why a metric has no usable number: its own reason, else the details of its
/// failed samples (a failed instrument records the diagnostic only there).
fn unmeasured_reason(metric: &Value) -> String {
    let reason = text(metric, "reason");
    if !reason.trim().is_empty() {
        return reason;
    }
    let samples = items(metric, "samples");
    let mut details: Vec<String> = samples
        .iter()
        .filter(|s| text(s, "status") == "failed")
        .map(|s| text(s, "detail"))
        .filter(|d| !d.trim().is_empty())
        .collect();
    if details.is_empty() {
        details = samples
            .iter()
            .map(|s| text(s, "detail"))
            .filter(|d| !d.trim().is_empty())
            .collect();
    }
    let mut seen = Vec::new();
    details.retain(|d| {
        let fresh = !seen.contains(d);
        seen.push(d.clone());
        fresh
    });
    if details.is_empty() {
        "the receipt records no reason".to_string()
    } else {
        details.join("; ")
    }
}

/// The repositories whose instrument failed, then the worst measured sample, as
/// `repo (value)`. A failed sample has no number, so it is named by status
/// rather than dropped behind the worst repository that did measure.
fn worst_sample(dx: &Value, id: &str) -> Option<String> {
    let metric = items(dx, "metrics")
        .iter()
        .find(|m| field(m, "id").as_str() == Some(id))?;
    let unit = text(metric, "unit");
    let lower = text(metric, "direction") != "higher_is_better";
    let samples = items(metric, "samples");
    let failed: Vec<String> = samples
        .iter()
        .filter(|s| text(s, "status") == "failed")
        .map(|s| {
            let repo = text(s, "repo");
            if repo.is_empty() {
                "a repository".to_string()
            } else {
                repo
            }
        })
        .collect();
    let worst = samples
        .iter()
        .filter(|s| text(s, "status") != "failed")
        .filter_map(|s| Some((text(s, "repo"), field(s, "value").as_f64()?)))
        .reduce(|a, b| {
            let worse = if lower { b.1 > a.1 } else { b.1 < a.1 };
            if worse { b } else { a }
        })
        .map(|(repo, value)| format!("{repo} at {}", fmt_value(value, &unit)));
    match (failed.is_empty(), worst) {
        (true, worst) => worst,
        (false, None) => Some(format!("{} (instrument failed)", failed.join(", "))),
        (false, Some(worst)) => Some(format!(
            "{} (instrument failed); worst measured is {worst}",
            failed.join(", ")
        )),
    }
}

fn family_label(key: &str) -> String {
    match key {
        "claims_discriminator" => "ripr found a discriminator (`strongly_gripped`)".to_string(),
        "claims_no_discriminator" => "ripr found no discriminator (`ungripped`)".to_string(),
        other => other.to_string(),
    }
}

fn mutation_section(page: &mut Page, mutation: &Value) -> Result<(), String> {
    let ctx = "mutation-spot-check";
    page.line("## Mutation agreement");
    page.blank();
    page.line(format!(
        "{} Status of this receipt: `{}`.",
        req_str(mutation, "claim_boundary", ctx)?,
        req_str(mutation, "status", ctx)?
    ));
    page.blank();
    match field(mutation, "analyzer_version").as_str() {
        Some(version) => page.line(format!("Static classifications were produced by {version}.")),
        None => page.line("This receipt does not record which ripr build produced the static classifications, only the checkout revisions and the cargo-mutants version. The agreement rates below cannot be tied to a specific analyzer revision, and they may not describe the current build."),
    }
    page.blank();
    let families = field(mutation, "scored_families");
    let mut family_rows: Vec<Vec<String>> = Vec::new();
    for (key, family) in families.as_object().into_iter().flatten() {
        let fctx = format!("{ctx} family `{key}`");
        let counts = field(family, "counts");
        family_rows.push(vec![
            family_label(key),
            req_u64(family, "seams_scored", &fctx)?.to_string(),
            req_u64(family, "mutants_scored", &fctx)?.to_string(),
            sparse_count(counts, "agree", &fctx)?.to_string(),
            sparse_count(counts, "overclaim", &fctx)?.to_string(),
            sparse_count(counts, "false_gap", &fctx)?.to_string(),
            percent(req_f64(family, "agreement_rate", &fctx)?),
        ]);
    }
    if family_rows.is_empty() {
        return Err(format!("{ctx}: `scored_families` is missing or empty"));
    }
    page.table(
        &[
            "Verdict class",
            "Seams",
            "Mutants",
            "Agree",
            "Overclaim",
            "False gap",
            "Agreement",
        ],
        &family_rows,
    );
    page.line("An overclaim is a mutant ripr said a test would catch that no test caught. A false gap is a mutant ripr said nothing would catch that the tests did catch.");
    page.blank();

    let mut repo_rows: Vec<Vec<String>> = Vec::new();
    for repo in req_arr(mutation, "repos", ctx)? {
        let rctx = format!("{ctx} repo");
        let metrics = field(repo, "calibration_metrics");
        repo_rows.push(vec![
            req_str(repo, "name", &rctx)?,
            format!("`{}`", short(&req_str(repo, "revision", &rctx)?, 12)),
            req_str(repo, "cargo_mutants_version", &rctx)?,
            req_u64(metrics, "mutants_total", &rctx)?.to_string(),
            req_u64(metrics, "matched_total", &rctx)?.to_string(),
            req_u64(metrics, "ambiguous_file_line_total", &rctx)?.to_string(),
            req_u64(metrics, "unmatched_mutants_total", &rctx)?.to_string(),
            req_u64(field(repo, "pairings"), "seam_precise", &rctx)?.to_string(),
        ]);
    }
    page.table(
        &[
            "Repository",
            "Revision",
            "cargo-mutants",
            "Mutants",
            "Joined",
            "Ambiguous",
            "Unmatched",
            "Seam-precise",
        ],
        &repo_rows,
    );

    let grip = field(
        field(mutation, "outcomes_by_pairing_and_grip_class"),
        "seam_precise",
    );
    let mut grip_rows: Vec<Vec<String>> = Vec::new();
    for (class, outcomes) in grip.as_object().into_iter().flatten() {
        let mut summary: Vec<String> = Vec::new();
        for (name, count) in outcomes.as_object().into_iter().flatten() {
            let n = count.as_u64().ok_or_else(|| {
                format!("{ctx}: grip class `{class}` outcome `{name}` is not a count")
            })?;
            summary.push(format!("{name} {n}"));
        }
        grip_rows.push(vec![format!("`{class}`"), summary.join(", ")]);
    }
    if grip_rows.is_empty() {
        return Err(format!(
            "{ctx}: `outcomes_by_pairing_and_grip_class.seam_precise` is missing or empty"
        ));
    }
    page.line("Real mutation outcomes by ripr grip class, seam-precise joins only:");
    page.blank();
    page.table(&["Grip class", "Mutation outcomes"], &grip_rows);

    let examples = items(field(mutation, "disagreement_examples"), "false_gap");
    if !examples.is_empty() {
        page.line("False-gap examples, as recorded:");
        page.blank();
        for example in examples {
            let ectx = format!("{ctx} false-gap example");
            let mutant = req_str(example, "mutant", &ectx)?;
            page.line(format!(
                "- {} `{}:{}` {} `{}` is `{}`; mutant `{}` was {}.",
                req_str(example, "repo", &ectx)?,
                req_str(example, "file", &ectx)?,
                req_u64(example, "line", &ectx)?,
                req_str(example, "seam_kind", &ectx)?,
                req_str(example, "expression", &ectx)?,
                req_str(example, "grip_class", &ectx)?,
                mutant
                    .split_once(": ")
                    .map_or_else(|| mutant.clone(), |(_, rest)| rest.to_string()),
                req_str(example, "runtime_outcome", &ectx)?,
            ));
        }
        page.blank();
    }
    Ok(())
}

fn verdict_section(page: &mut Page, verdicts: &Value) -> Result<(), String> {
    let ctx = "verdict-corpus";
    page.line("## Verdict corpus");
    page.blank();
    page.line(format!(
        "{} hand-labeled changes (corpus {}, {}). Each has a ground-truth label from real mutants and an ideal verdict; ripr's observed verdict is compared against it. A false actionable verdict is a reported gap on a change whose tests caught every listed mutant. That is the failure that costs a developer's trust, so it is tracked on its own.",
        req_f64(verdicts, "cases_total", ctx)?,
        text(verdicts, "corpus_version"),
        text(verdicts, "spec")
    ));
    page.blank();
    match field(verdicts, "analyzer_version").as_str() {
        Some(version) => page.line(format!("Verdicts were produced by {version}.")),
        None => page.line("This receipt does not record which ripr build produced the observed verdicts, only the corpus version. The rates below cannot be tied to a specific analyzer revision, and they may not describe the current build."),
    }
    page.blank();
    let labels = [
        ("false_verdict_rate", "False verdicts (all cases)"),
        (
            "false_actionable_rate",
            "False actionable (of discriminated)",
        ),
        (
            "false_exposed_rate",
            "False exposed (of not fully discriminated)",
        ),
        (
            "false_silent_rate",
            "False silent (of not fully discriminated)",
        ),
        ("ideal_rate", "Ideal verdict"),
        (
            "abstention_rate",
            "Abstained (limited or silent where acceptable)",
        ),
        ("contradiction_rate", "Findings with a contradiction"),
    ];
    page.line("Only the upstream cases come from real repositories. The authored cases were written to fill verdict and probe-family cells the upstream cases leave empty, so their rates are not real-world rates and are shown apart.");
    page.blank();
    let cell = |rate: Rate| {
        format!(
            "{}/{} ({})",
            rate.numerator,
            rate.denominator,
            percent(rate.rate)
        )
    };
    let mut rate_rows = Vec::new();
    for (key, label) in labels {
        let split = |origin: &str| {
            if field(field(verdicts, "by_origin"), origin)
                .get(key)
                .is_some()
            {
                origin_rate(verdicts, origin, key).map(cell)
            } else {
                Ok("not split by origin".to_string())
            }
        };
        rate_rows.push(vec![
            label.to_string(),
            cell(req_rate(verdicts, key, ctx)?),
            split("upstream")?,
            split("authored")?,
        ]);
    }
    page.table(
        &[
            "Rate",
            "All cases",
            "Upstream (real repositories)",
            "Authored",
        ],
        &rate_rows,
    );
    let case_rows: Vec<Vec<String>> = req_arr(verdicts, "rows", ctx)?
        .iter()
        .map(|row| {
            let contradictions: Vec<String> =
                items(row, "contradictions").iter().map(item_text).collect();
            let classes: Vec<String> = items(row, "observed_classifications")
                .iter()
                .map(item_text)
                .collect();
            vec![
                format!("`{}`", text(row, "case_id")),
                text(row, "origin"),
                text(row, "truth"),
                text(row, "ideal_verdict"),
                text(row, "observed_verdict"),
                classes.join(", "),
                text(row, "outcome"),
                if contradictions.is_empty() {
                    "none".to_string()
                } else {
                    contradictions.join(", ")
                },
            ]
        })
        .collect();
    page.table(
        &[
            "Case",
            "Origin",
            "Truth",
            "Ideal",
            "Observed",
            "Static classes",
            "Outcome",
            "Contradictions",
        ],
        &case_rows,
    );
    let non_claims = items(verdicts, "non_claims");
    if !non_claims.is_empty() {
        page.line("What the corpus does not claim:");
        page.blank();
        for claim in non_claims {
            page.line(format!("- {}", item_text(claim)));
        }
        page.blank();
    }
    Ok(())
}

fn speed_section(page: &mut Page, dx: &Value) -> Result<(), String> {
    let ctx = "dx-scoreboard";
    page.line("## Speed and memory");
    page.blank();
    let ids = [
        "speed.cold_pilot_ms",
        "speed.cold_pilot_peak_rss_mb",
        "speed.warm_check_ms",
        "speed.warm_check_peak_rss_mb",
    ];
    let metrics = req_arr(dx, "metrics", ctx)?;
    let find = |id: &str| metrics.iter().find(|m| field(m, "id").as_str() == Some(id));
    let mut head = vec!["Repository".to_string()];
    let mut target_row = vec!["**Bar**".to_string()];
    for id in ids {
        let metric = find(id).ok_or_else(|| format!("{ctx}: metric `{id}` is missing"))?;
        head.push(text(metric, "title"));
        target_row.push(fmt_value(
            req_f64(metric, "target", id)?,
            &text(metric, "unit"),
        ));
    }
    let mut rows = vec![target_row];
    for repo in req_arr(dx, "repos", ctx)? {
        let mut row = vec![format!("{} ({})", text(repo, "id"), text(repo, "note"))];
        for id in ids {
            let entry = items(repo, "metrics")
                .iter()
                .find(|m| field(m, "metric").as_str() == Some(id));
            row.push(match entry {
                Some(m) => {
                    let value = field(m, "value").as_f64().map_or_else(
                        || "not measured".to_string(),
                        |v| fmt_value(v, &text(m, "unit")),
                    );
                    if text(m, "status") == "below_target" {
                        format!("**{value}**")
                    } else {
                        value
                    }
                }
                None => "not measured".to_string(),
            });
        }
        rows.push(row);
    }
    let head_refs: Vec<&str> = head.iter().map(String::as_str).collect();
    page.line(format!(
        "Measured on the pinned corpus repositories at the revisions in the receipt, with a release build, on runner class `{}`. Bold values miss the bar. Cold is a fresh cache; warm is the second `ripr check` against the last commit. Peak memory is the process's high-water mark sampled every 10 ms on Linux, a lower bound.",
        text(dx, "runner_class")
    ));
    page.blank();
    page.table(&head_refs, &rows);
    Ok(())
}

fn step_range(receipt: &Value, step: &str) -> String {
    let secs: Vec<f64> = items(receipt, "cases")
        .iter()
        .flat_map(steps_of)
        .filter(|s| field(s, "step").as_str() == Some(step))
        .filter_map(|s| field(s, "secs").as_f64())
        .collect();
    let min = secs.iter().copied().reduce(f64::min);
    let max = secs.iter().copied().reduce(f64::max);
    match (min, max) {
        (Some(lo), Some(hi)) if (hi - lo).abs() < 1e-9 => format!("{} s", num(lo)),
        (Some(lo), Some(hi)) => format!("{}-{} s", num(lo), num(hi)),
        _ => "not run".to_string(),
    }
}

fn friction_summary(receipt: &Value) -> Vec<String> {
    let mut seen: Vec<(String, usize)> = Vec::new();
    for step in all_steps(receipt) {
        for flag in friction_of(step) {
            let flag = item_text(flag);
            match seen.iter_mut().find(|(text, _)| *text == flag) {
                Some((_, count)) => *count += 1,
                None => seen.push((flag, 1)),
            }
        }
    }
    seen.into_iter()
        .map(|(flag, count)| {
            if count > 1 {
                format!("{flag} (x{count})")
            } else {
                flag
            }
        })
        .collect()
}

fn first_run_section(
    page: &mut Page,
    previous: &Value,
    current: &Value,
    install: &Value,
) -> Result<(), String> {
    let before = short_version(&req_str(previous, "ripr", "first-run-previous")?);
    let after = short_version(&req_str(current, "ripr", "first-run-current")?);
    page.line("## First run");
    page.blank();
    page.line(
        "A scripted new developer runs `doctor`, `check`, `pilot`, the follow-up command `check` prints, and `init --ci github` against crates ripr was not tuned on, with one committed boundary edit each. The walk records timings and friction and does not judge verdict accuracy. Timings come from one Linux container. A standalone install timing is shown at the end of this section; it is not the time for the generated CI workflow to get ripr, which stays unmeasured."
    );
    page.blank();
    let mut verdict_rows = Vec::new();
    for case in req_arr(current, "cases", "first-run-current")? {
        let name = req_str(case, "case", "first-run-current case")?;
        let earlier = req_arr(previous, "cases", "first-run-previous")?
            .iter()
            .find(|c| text(c, "case") == name);
        let (earlier_verdict, earlier_lines) = match earlier {
            Some(c) => (
                req_str(c, "verdict", "first-run-previous case")?,
                req_u64(c, "workflow_lines", "first-run-previous case")?.to_string(),
            ),
            None => ("not run".to_string(), "not run".to_string()),
        };
        verdict_rows.push(vec![
            name,
            earlier_verdict,
            req_str(case, "verdict", "first-run-current case")?,
            earlier_lines,
            req_u64(case, "workflow_lines", "first-run-current case")?.to_string(),
        ]);
    }
    page.table(
        &[
            "Crate",
            &format!("Verdict, {before}"),
            &format!("Verdict, {after}"),
            &format!("Workflow lines, {before}"),
            &format!("Workflow lines, {after}"),
        ],
        &verdict_rows,
    );
    let mut step_names: Vec<String> = Vec::new();
    for receipt in [previous, current] {
        for step in all_steps(receipt) {
            let name = text(step, "step");
            if !step_names.contains(&name) {
                step_names.push(name);
            }
        }
    }
    let step_rows: Vec<Vec<String>> = step_names
        .iter()
        .map(|name| {
            vec![
                format!("`{name}`"),
                step_range(previous, name),
                step_range(current, name),
            ]
        })
        .collect();
    page.line("Seconds per step, range across crates:");
    page.blank();
    page.table(&["Step", &before, &after], &step_rows);
    for (label, receipt) in [(&before, previous), (&after, current)] {
        let flags = friction_summary(receipt);
        page.line(format!("Friction flagged on {label}:"));
        page.blank();
        if flags.is_empty() {
            page.line("- none");
        }
        for flag in flags {
            page.line(format!("- {flag}"));
        }
        page.blank();
    }
    page.line("Install, from its own receipt:");
    page.blank();
    page.line(format!("> {}", req_str(install, "evidence", "install")?));
    page.blank();
    Ok(())
}

fn agent_section(page: &mut Page, agent: &Value) -> Result<(), String> {
    page.line("## An agent using only ripr's help");
    page.blank();
    page.line(format!(
        "One agent per crate was given a real test gap and only ripr's own help output, and was scored against a mutation answer key. This is one run per crate, not a rate. Evidence: {}.",
        req_str(agent, "evidence", "agent-as-user")?
    ));
    page.blank();
    let metrics = req_arr(agent, "metrics", "agent-as-user")?;
    let value = |id: &str, repo: &str| -> String {
        metrics
            .iter()
            .find(|m| text(m, "id") == id && text(m, "repo") == repo)
            .and_then(|m| field(m, "value").as_f64())
            .map_or_else(|| "n/a".to_string(), num)
    };
    let mut repos: Vec<String> = Vec::new();
    for metric in metrics {
        let repo = text(metric, "repo");
        if !repo.is_empty() && !repos.contains(&repo) {
            repos.push(repo);
        }
    }
    let rows: Vec<Vec<String>> = repos
        .iter()
        .map(|repo| {
            vec![
                repo.clone(),
                value("agent.answer_key_mutants_caught", repo),
                value("agent.ripr_commands_to_fix", repo),
                value("agent.tool_steps_to_fix", repo),
                value("agent.false_weak_findings_after_fix", repo),
            ]
        })
        .collect();
    page.table(
        &[
            "Crate",
            "Answer-key mutants caught",
            "ripr commands to fix",
            "Tool steps to fix",
            "False weak findings after the fix",
        ],
        &rows,
    );
    let overall = |id: &str| -> String {
        metrics
            .iter()
            .find(|m| text(m, "id") == id && text(m, "repo").is_empty())
            .and_then(|m| field(m, "value").as_f64())
            .map_or_else(|| "n/a".to_string(), num)
    };
    let fix_success = metrics
        .iter()
        .find(|m| text(m, "id") == "agent.fix_success_rate" && text(m, "repo").is_empty())
        .and_then(|m| field(m, "value").as_f64())
        .map_or_else(|| "n/a".to_string(), percent);
    page.line(format!(
        "Across the runs: fix success {}, stale re-check cycles {}, white-box tests written only to satisfy ripr {}.",
        fix_success,
        overall("agent.stale_recheck_cycles"),
        overall("agent.white_box_tests_to_satisfy_ripr"),
    ));
    page.blank();
    Ok(())
}

fn corpus_section(page: &mut Page, r: &Receipts) -> Result<(), String> {
    let ctx = "rust-corpus manifest";
    let repos = req_arr(&r.corpus, "repos", ctx)?;
    let class_count = |class: &str| {
        repos
            .iter()
            .filter(|repo| field(field(repo, "profile"), "class").as_str() == Some(class))
            .count()
    };
    page.line("## The corpus behind the numbers");
    page.blank();
    page.line(format!(
        "{} real repositories are pinned to exact upstream commits (corpus {}): {} well-maintained, {} legacy, {} ordinary. The class is a judgment recorded per repository, not a measurement. The scoreboards were run on the repositories named in their own sections, which are not all in this manifest at these revisions, so a number above describes only the repositories its section names.",
        repos.len(),
        text(&r.corpus, "corpus_version"),
        class_count("good"),
        class_count("legacy"),
        class_count("common"),
    ));
    page.blank();
    let known_gaps = items(&r.corpus, "known_gaps");
    if !known_gaps.is_empty() {
        page.line("Known gaps in the corpus:");
        page.blank();
        for gap in known_gaps {
            page.line(format!("- {}", item_text(gap)));
        }
        page.blank();
    }
    let dx_rows: Vec<Vec<String>> = req_arr(&r.dx, "corpus", "dx-scoreboard")?
        .iter()
        .map(|repo| {
            vec![
                text(repo, "id"),
                format!("`{}`", short(&text(repo, "sha"), 12)),
                text(repo, "url"),
                text(repo, "status"),
            ]
        })
        .collect();
    page.line("Repositories the speed scoreboard ran on:");
    page.blank();
    page.table(
        &["Repository", "Pinned commit", "Source", "Status"],
        &dx_rows,
    );
    Ok(())
}

fn boundaries(page: &mut Page, r: &Receipts) -> Result<(), String> {
    page.line("## What these numbers do not say");
    page.blank();
    page.line(format!(
        "- {}",
        req_str(&r.dx, "claim_boundary", "dx-scoreboard")?
    ));
    page.line(format!(
        "- {}",
        req_str(&r.mutation, "claim_boundary", "mutation-spot-check")?
    ));
    for claim in items(&r.verdicts, "non_claims") {
        page.line(format!("- {}", item_text(claim)));
    }
    page.line("- ripr reports static evidence. It does not run your tests or mutate your code, and no figure here is a runtime mutation result for your repository.");
    page.line("- Agent and first-run figures are single walks, not rates.");
    page.blank();
    Ok(())
}

fn reproduce(page: &mut Page) {
    page.line("## Reproduce and refresh");
    page.blank();
    page.line("```bash");
    page.line(
        "cargo xtask dx-scoreboard --clone            # speed, memory, CI size, pasted commands",
    );
    page.line("cargo xtask first-run                        # the new-developer walk");
    page.line(
        "cargo xtask public-proof --refresh-receipts  # re-copy canonical outputs, then render",
    );
    page.line("cargo xtask public-proof --check             # fail if this page is stale");
    page.line("```");
    page.blank();
    page.line("Receipts live in `metrics/public-proof/`. `dx-scoreboard.json`, `verdict-corpus.json` and `corpus-manifest.json` are verbatim copies of `metrics/dx-scoreboard/baseline.json`, `fixtures/rust-verdict-corpus/expected/report.json` and `benchmarks/rust_corpus/manifest.json`; `--check` fails when a source moves ahead of its copy, and `--refresh-receipts` re-copies them. Corpus and ranking scoreboard rows are read from `metrics/dx-scoreboard/corpus-full-baseline.json` (falling back to `corpus-fast-baseline.json`) and `metrics/dx-scoreboard/pilot-ranking-baseline.json` when the copied scoreboard receipt did not ingest those boards. The mutation, first-run, agent and install receipts have no in-repo source to compare against: they are committed copies of harness output from the revisions named in their sections, and `--check` cannot detect a hand edit to them. The mutation spot-check has no command in this repository yet.");
}

fn lane_revision(receipt: &Value) -> String {
    let rev = text(receipt, "revision");
    if rev.is_empty() {
        "revision not recorded".to_string()
    } else if rev == "unavailable" {
        "revision unavailable".to_string()
    } else {
        short(&rev, 7)
    }
}

fn runner_detail(receipt: &Value) -> String {
    let class = text(receipt, "runner_class");
    if class.is_empty() {
        "runner class not recorded".to_string()
    } else {
        format!("runner `{class}`")
    }
}

fn scoreboard_summary(
    total: usize,
    met: usize,
    below: usize,
    unmeasured: usize,
    failed_note: String,
    bars: &[Bar],
) -> String {
    let cross = bars
        .iter()
        .any(|bar| bar.trend.contains("another runner class"));
    let from_lane = bars
        .iter()
        .any(|bar| bar.basis.starts_with("metrics/dx-scoreboard/"));
    let mut line = format!(
        "{total} bars. ripr meets {met}, is below the bar on {below}, and has not measured {unmeasured}{failed_note}. Bold values miss their bar. A comparable trend shows the baseline revision and prior value it compares."
    );
    if cross {
        line.push_str(" A cross-class trend shows the earlier runner class and, when available, its prior value; it does not compare measurements.");
    }
    if bars.iter().any(measured_first_receipt) {
        line.push_str(" A measured row with no earlier measurement is a first measurement.");
    }
    if from_lane {
        line.push_str(" Corpus and ranking rows come from those lanes' own baselines when the scoreboard receipt did not ingest them.");
    }
    line
}

fn measured_first_receipt(bar: &Bar) -> bool {
    matches!(bar.status, Status::Meets | Status::Below)
        && bar.trend.contains("no earlier measurement")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
    }

    #[test]
    fn committed_page_matches_its_receipts() -> Result<(), String> {
        let root = workspace_root();
        let rendered = render(&root)?;
        check_page(&root, &rendered)
    }

    #[test]
    fn page_names_its_shortfalls_before_its_detail() -> Result<(), String> {
        let rendered = render(&workspace_root())?;
        let shortfalls = rendered
            .find("## Where ripr falls short")
            .ok_or("no shortfalls section")?;
        let mutation = rendered
            .find("## Mutation agreement")
            .ok_or("no mutation section")?;
        assert!(shortfalls < mutation);
        Ok(())
    }

    #[test]
    fn stale_page_is_reported() {
        let result = check_page(&workspace_root(), "# not the page\n");
        assert!(result.is_err_and(|err| err.contains("does not match its receipts")));
    }

    #[test]
    fn unknown_arguments_are_refused() {
        let result = parse_options(&["--nope".to_string()]);
        assert!(result.is_err());
        let both = parse_options(&["--check".to_string(), "--refresh-receipts".to_string()]);
        assert!(both.is_err());
    }

    #[test]
    fn missing_receipt_field_is_an_error_not_a_blank() -> Result<(), String> {
        let mut receipts = load(&workspace_root())?;
        if let Some(object) = receipts
            .verdicts
            .get_mut("by_origin")
            .and_then(|origins| origins.get_mut("upstream"))
            .and_then(Value::as_object_mut)
        {
            object.remove("false_actionable_rate");
        }
        let mut page = Page(String::new());
        let result = shortfalls(&mut page, &receipts, &[]);
        assert!(result.is_err_and(|err| err.contains("false_actionable_rate")));
        Ok(())
    }

    #[test]
    fn missing_mutation_count_is_an_error_not_a_zero() -> Result<(), String> {
        let mut receipts = load(&workspace_root())?;
        let removed = receipts
            .mutation
            .get_mut("repos")
            .and_then(|repos| repos.get_mut(0))
            .and_then(|repo| repo.get_mut("pairings"))
            .and_then(Value::as_object_mut)
            .and_then(|pairings| pairings.remove("seam_precise"));
        assert!(removed.is_some(), "fixture lost seam_precise");
        let result = mutation_join_totals(&receipts.mutation);
        assert!(result.is_err_and(|err| err.contains("seam_precise")));
        let mut page = Page(String::new());
        let section = mutation_section(&mut page, &receipts.mutation);
        assert!(section.is_err_and(|err| err.contains("seam_precise")));
        Ok(())
    }

    #[test]
    fn missing_canonical_source_is_an_error() -> Result<(), String> {
        let dir =
            std::env::temp_dir().join(format!("ripr-public-proof-nosrc-{}", std::process::id()));
        let receipts = dir.join(RECEIPTS);
        fs::create_dir_all(&receipts).map_err(|e| e.to_string())?;
        fs::write(receipts.join("dx-scoreboard.json"), "{}").map_err(|e| e.to_string())?;
        fs::write(receipts.join("verdict-corpus.json"), "{}").map_err(|e| e.to_string())?;
        fs::write(receipts.join("corpus-manifest.json"), "{}").map_err(|e| e.to_string())?;
        // Two of the three sources exist; the corpus manifest does not.
        fs::create_dir_all(dir.join("metrics/dx-scoreboard")).map_err(|e| e.to_string())?;
        fs::write(dir.join("metrics/dx-scoreboard/baseline.json"), "{}")
            .map_err(|e| e.to_string())?;
        fs::create_dir_all(dir.join("fixtures/rust-verdict-corpus/expected"))
            .map_err(|e| e.to_string())?;
        fs::write(
            dir.join("fixtures/rust-verdict-corpus/expected/report.json"),
            "{}",
        )
        .map_err(|e| e.to_string())?;
        let result = receipt_drift(&dir);
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        assert!(result.is_err_and(|err| err.contains("manifest.json")));
        Ok(())
    }

    #[test]
    fn mutation_section_says_when_the_analyzer_build_is_unrecorded() -> Result<(), String> {
        let receipts = load(&workspace_root())?;
        let mut page = Page(String::new());
        mutation_section(&mut page, &receipts.mutation)?;
        assert!(page.0.contains("does not record which ripr build produced"));
        let mut recorded = receipts.mutation.clone();
        if let Some(object) = recorded.as_object_mut() {
            object.insert(
                "analyzer_version".to_string(),
                Value::from("ripr 0.11.0 (abc1234)"),
            );
        }
        let mut page = Page(String::new());
        mutation_section(&mut page, &recorded)?;
        assert!(page.0.contains("produced by ripr 0.11.0 (abc1234)"));
        assert!(!page.0.contains("does not record which ripr build"));
        Ok(())
    }

    #[test]
    fn verdict_section_says_when_the_analyzer_build_is_unrecorded() -> Result<(), String> {
        let receipts = load(&workspace_root())?;
        let mut page = Page(String::new());
        verdict_section(&mut page, &receipts.verdicts)?;
        assert!(
            page.0
                .contains("does not record which ripr build produced the observed verdicts")
        );
        let mut recorded = receipts.verdicts.clone();
        if let Some(object) = recorded.as_object_mut() {
            object.insert(
                "analyzer_version".to_string(),
                Value::from("ripr 0.11.0 (abc1234)"),
            );
        }
        let mut page = Page(String::new());
        verdict_section(&mut page, &recorded)?;
        assert!(page.0.contains("produced by ripr 0.11.0 (abc1234)"));
        assert!(!page.0.contains("does not record which ripr build"));
        Ok(())
    }

    #[test]
    fn thin_ground_truth_counts_scored_mutants_not_joins() -> Result<(), String> {
        let receipts = load(&workspace_root())?;
        let (precise, _) = mutation_join_totals(&receipts.mutation)?;
        let scored = mutation_scored_total(&receipts.mutation)?;
        assert!(
            scored < precise,
            "scored {scored} must be below joined {precise}"
        );
        let mut broken = receipts.mutation.clone();
        if let Some(object) = broken.as_object_mut() {
            object.remove("scored_families");
        }
        assert!(mutation_scored_total(&broken).is_err_and(|e| e.contains("scored_families")));
        Ok(())
    }

    #[test]
    fn agreement_bars_show_their_scored_denominator() -> Result<(), String> {
        let receipts = load(&workspace_root())?;
        let basis = scored_basis(
            &receipts.mutation,
            "claims_discriminator",
            "strongly_gripped",
        )?;
        assert!(basis.contains("scored;"), "{basis}");
        assert!(basis.contains("unscored"), "{basis}");
        assert!(
            scored_basis(&receipts.mutation, "claims_discriminator", "no_such_class")
                .is_err_and(|e| e.contains("no_such_class"))
        );
        let mut page = Page(String::new());
        agent_section(&mut page, &receipts.agent)?;
        assert!(page.0.contains("fix success 100.0%"), "{}", page.0);
        Ok(())
    }

    #[test]
    fn producer_status_outranks_a_recomputed_number() -> Result<(), String> {
        let mut receipts = load(&workspace_root())?;
        let id = "paste.unsafe_commands";
        let status_of = |receipts: &Receipts| -> Result<Status, String> {
            bars(receipts)?
                .into_iter()
                .find(|bar| bar.id == id)
                .map(|bar| bar.status)
                .ok_or_else(|| format!("no bar {id}"))
        };
        assert!(status_of(&receipts)? == Status::Meets);
        let set_status = |receipts: &mut Receipts, status: &str| {
            if let Some(metrics) = receipts.dx.get_mut("metrics").and_then(Value::as_array_mut) {
                for metric in metrics {
                    if metric.get("id").and_then(Value::as_str) == Some(id) {
                        metric["status"] = Value::from(status);
                    }
                }
            }
        };
        set_status(&mut receipts, "failed");
        assert!(status_of(&receipts)? == Status::Failed);
        set_status(&mut receipts, "below_target");
        assert!(status_of(&receipts)? == Status::Below);
        Ok(())
    }

    #[test]
    fn failed_instrument_keeps_its_sample_detail() -> Result<(), String> {
        let mut receipts = load(&workspace_root())?;
        let id = "paste.unsafe_commands";
        if let Some(metrics) = receipts.dx.get_mut("metrics").and_then(Value::as_array_mut) {
            for metric in metrics {
                if metric.get("id").and_then(Value::as_str) == Some(id) {
                    metric["status"] = Value::from("failed");
                    metric["reason"] = Value::from("");
                    metric["samples"] = serde_json::json!([
                        {"repo": null, "status": "failed", "value": null, "detail": "paste corpus did not build"}
                    ]);
                }
            }
        }
        let bar = bars(&receipts)?
            .into_iter()
            .find(|bar| bar.id == id)
            .ok_or_else(|| format!("no bar {id}"))?;
        assert!(bar.status == Status::Failed);
        assert!(
            bar.basis
                .contains("instrument failed: paste corpus did not build"),
            "basis was {:?}",
            bar.basis
        );
        Ok(())
    }

    #[test]
    fn untimed_walk_step_is_not_measured_not_summed() -> Result<(), String> {
        let mut receipts = load(&workspace_root())?;
        let status_of = |receipts: &Receipts| -> Result<Status, String> {
            bars(receipts)?
                .into_iter()
                .find(|bar| bar.id == "first_run.walk_secs")
                .map(|bar| bar.status)
                .ok_or_else(|| "no first_run.walk_secs bar".to_string())
        };
        assert!(status_of(&receipts)? != Status::NotMeasured);
        let mut removed = false;
        if let Some(cases) = receipts
            .first_current
            .get_mut("cases")
            .and_then(Value::as_array_mut)
        {
            for case in cases {
                let steps = case
                    .get_mut("steps")
                    .and_then(Value::as_array_mut)
                    .into_iter()
                    .flatten();
                for step in steps {
                    if !removed
                        && step.get("secs").is_some()
                        && let Some(object) = step.as_object_mut()
                    {
                        object.remove("secs");
                        removed = true;
                    }
                }
            }
        }
        assert!(removed, "fixture has no timed step to remove");
        assert!(status_of(&receipts)? == Status::NotMeasured);
        let bar = bars(&receipts)?
            .into_iter()
            .find(|bar| bar.id == "first_run.walk_secs")
            .ok_or_else(|| "no first_run.walk_secs bar".to_string())?;
        assert!(
            bar.basis.contains("first-run receipts") && bar.basis.contains("untimed or empty"),
            "basis was {:?}",
            bar.basis
        );
        Ok(())
    }

    #[test]
    fn empty_walk_is_not_measured_not_zero_seconds() -> Result<(), String> {
        let mut receipts = load(&workspace_root())?;
        if let Some(cases) = receipts
            .first_current
            .get_mut("cases")
            .and_then(Value::as_array_mut)
        {
            for case in cases {
                case["steps"] = serde_json::json!([]);
            }
        }
        let bar = bars(&receipts)?
            .into_iter()
            .find(|bar| bar.id == "first_run.walk_secs")
            .ok_or_else(|| "no first_run.walk_secs bar".to_string())?;
        assert!(bar.status == Status::NotMeasured);
        Ok(())
    }

    #[test]
    fn verdict_rates_are_split_by_origin() -> Result<(), String> {
        let receipts = load(&workspace_root())?;
        let all = req_rate(&receipts.verdicts, "false_verdict_rate", "verdict-corpus")?;
        let upstream = origin_rate(&receipts.verdicts, "upstream", "false_verdict_rate")?;
        assert!(
            upstream.denominator < all.denominator,
            "upstream must be a subset of all cases"
        );
        let mut page = Page(String::new());
        verdict_section(&mut page, &receipts.verdicts)?;
        assert!(page.0.contains("Upstream (real repositories)"));
        let mut broken = receipts.verdicts.clone();
        if let Some(object) = broken.as_object_mut() {
            object.remove("by_origin");
        }
        assert!(
            origin_rate(&broken, "upstream", "false_verdict_rate")
                .is_err_and(|e| e.contains("by_origin.upstream"))
        );
        Ok(())
    }

    #[test]
    fn formatting_keeps_units_readable() {
        assert_eq!(fmt_value(237_697.110_6, "ms"), "237.7 s");
        assert_eq!(fmt_value(1.0, "findings"), "1 finding");
        assert_eq!(fmt_value(2.0, "commands"), "2 commands");
        assert_eq!(fmt_value(3467.17, "MB"), "3467 MB");
        assert_eq!(fmt_value(0.043, "ratio"), "4.3%");
        assert_eq!(fmt_delta(-1220.0, "lines"), "-1220 lines");
        assert_eq!(
            short_version("ripr 0.11.0 (a7a089e1c51dfa51)"),
            "ripr 0.11.0 (a7a089e)"
        );
    }

    #[test]
    fn bar_status_follows_direction() {
        assert!(meets(0.96, 0.95, false));
        assert!(!meets(0.04, 0.8, false));
        assert!(meets(1.0, 2.0, true));
        assert!(!meets(3.0, 2.0, true));
    }

    #[test]
    fn worst_sample_names_a_failed_instrument() -> Result<(), String> {
        let dx = serde_json::json!({"metrics": [{
            "id": "speed.cold_s", "unit": "s", "direction": "lower_is_better",
            "samples": [
                {"repo": "alpha", "status": "meets_target", "value": 3.0},
                {"repo": "beta", "status": "failed", "value": null},
                {"repo": "gamma", "status": "meets_target", "value": 9.0}
            ]
        }]});
        let both = worst_sample(&dx, "speed.cold_s").ok_or("no sample line")?;
        assert!(both.contains("beta (instrument failed)"), "{both}");
        assert!(both.contains("worst measured is gamma"), "{both}");
        let only_failed = serde_json::json!({"metrics": [{
            "id": "speed.cold_s", "unit": "s", "direction": "lower_is_better",
            "samples": [{"repo": "beta", "status": "failed", "value": null}]
        }]});
        let line = worst_sample(&only_failed, "speed.cold_s").ok_or("no sample line")?;
        assert_eq!(line, "beta (instrument failed)");
        Ok(())
    }

    #[test]
    fn source_drift_previews_the_page_lines_that_would_change() -> Result<(), String> {
        let real = workspace_root();
        let dir = std::env::temp_dir().join(format!(
            "ripr-public-proof-preview-test-{}",
            std::process::id()
        ));
        let receipts = dir.join(RECEIPTS);
        fs::create_dir_all(&receipts).map_err(|e| e.to_string())?;
        fs::create_dir_all(dir.join("docs")).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(real.join(RECEIPTS)).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            fs::copy(entry.path(), receipts.join(entry.file_name())).map_err(|e| e.to_string())?;
        }
        fs::copy(real.join(PAGE), dir.join(PAGE)).map_err(|e| e.to_string())?;
        for (receipt, source) in CANONICAL_SOURCES {
            let target = dir.join(source);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(receipts.join(receipt), &target).map_err(|e| e.to_string())?;
        }
        copy_lane_baselines(&real, &dir)?;
        // In sync: the check passes before any source moves.
        check_receipts(&dir)?;
        // Bump one bar's title in the dx source; a refresh must move that page line.
        let source = dir.join("metrics/dx-scoreboard/baseline.json");
        let mut dx = read_json(&source)?;
        let mut old_title = String::new();
        if let Some(metric) = dx
            .get_mut("metrics")
            .and_then(Value::as_array_mut)
            .and_then(|metrics| metrics.first_mut())
        {
            old_title = text(metric, "title");
            metric["title"] = Value::from("Renamed by the preview test");
        }
        fs::write(&source, serde_json::to_vec(&dx).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let result = check_receipts(&dir);
        let untouched = fs::read(receipts.join("dx-scoreboard.json")).map_err(|e| e.to_string())?;
        let original =
            fs::read(real.join(RECEIPTS).join("dx-scoreboard.json")).map_err(|e| e.to_string())?;
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        let err = result.err().ok_or("drift was not reported")?;
        assert!(err.contains("refreshing would change the page"), "{err}");
        assert!(
            err.contains("+ ") && err.contains("Renamed by the preview test"),
            "{err}"
        );
        assert!(err.contains("- ") && err.contains(&old_title), "{err}");
        assert!(untouched == original, "the preview must not write receipts");
        Ok(())
    }

    #[test]
    fn changed_lines_reports_moved_and_duplicated_lines() {
        assert!(changed_lines("a\nb\nc", "a\nb\nc").is_empty());
        assert_eq!(changed_lines("a\nb", "a\nx\nb"), vec!["+ x"]);
        assert_eq!(changed_lines("a\nb\na", "a\nb"), vec!["- a"]);
        assert!(!changed_lines("a\nb", "b\na").is_empty());
        assert_eq!(
            changed_lines("a\nold\nz", "a\nnew\nz"),
            vec!["- old", "+ new"]
        );
    }

    #[test]
    fn failed_speed_instrument_reaches_the_rendered_shortfalls() -> Result<(), String> {
        let mut receipts = load(&workspace_root())?;
        let id = bars(&receipts)?
            .into_iter()
            .find(|bar| bar.board == "Speed and memory" && bar.id.starts_with("speed."))
            .map(|bar| bar.id)
            .ok_or("no speed bar in the receipts")?;
        if let Some(metrics) = receipts.dx.get_mut("metrics").and_then(Value::as_array_mut) {
            for metric in metrics {
                if metric.get("id").and_then(Value::as_str) == Some(id.as_str()) {
                    metric["status"] = Value::from("failed");
                    metric["samples"] = serde_json::json!([
                        {"repo": "beta", "status": "failed", "value": null, "detail": "clone failed"}
                    ]);
                }
            }
        }
        let all = bars(&receipts)?;
        let mut page = Page(String::new());
        shortfalls(&mut page, &receipts, &all)?;
        assert!(
            page.0
                .contains("Instrument failed: beta (instrument failed)"),
            "{}",
            page.0
        );
        Ok(())
    }

    #[test]
    fn failed_metric_without_failed_sample_uses_its_reason() -> Result<(), String> {
        let mut receipts = load(&workspace_root())?;
        let id = bars(&receipts)?
            .into_iter()
            .find(|bar| bar.board == "Speed and memory" && bar.id.starts_with("speed."))
            .map(|bar| bar.id)
            .ok_or("no speed bar in the receipts")?;
        if let Some(metrics) = receipts.dx.get_mut("metrics").and_then(Value::as_array_mut) {
            for metric in metrics {
                if metric.get("id").and_then(Value::as_str) == Some(id.as_str()) {
                    metric["status"] = Value::from("failed");
                    metric["reason"] = Value::from("pilot harness crashed");
                    metric["samples"] = serde_json::json!([
                        {"repo": "alpha", "status": "meets_target", "value": 3.0}
                    ]);
                }
            }
        }
        let all = bars(&receipts)?;
        let mut page = Page(String::new());
        shortfalls(&mut page, &receipts, &all)?;
        assert!(!page.0.contains("Instrument failed: alpha"), "{}", page.0);
        assert!(page.0.contains("Instrument failed:"), "{}", page.0);
        Ok(())
    }

    #[test]
    fn drifted_receipt_is_reported() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!("ripr-public-proof-{}", std::process::id()));
        let receipts = dir.join(RECEIPTS);
        fs::create_dir_all(&receipts).map_err(|e| e.to_string())?;
        fs::create_dir_all(dir.join("metrics/dx-scoreboard")).map_err(|e| e.to_string())?;
        fs::write(receipts.join("dx-scoreboard.json"), "{\"a\":1}").map_err(|e| e.to_string())?;
        fs::write(dir.join("metrics/dx-scoreboard/baseline.json"), "{\"a\":2}")
            .map_err(|e| e.to_string())?;
        fs::write(receipts.join("verdict-corpus.json"), "{}").map_err(|e| e.to_string())?;
        fs::create_dir_all(dir.join("fixtures/rust-verdict-corpus/expected"))
            .map_err(|e| e.to_string())?;
        fs::write(
            dir.join("fixtures/rust-verdict-corpus/expected/report.json"),
            "{}",
        )
        .map_err(|e| e.to_string())?;
        fs::create_dir_all(dir.join("benchmarks/rust_corpus")).map_err(|e| e.to_string())?;
        fs::write(receipts.join("corpus-manifest.json"), "{\"v\":1}").map_err(|e| e.to_string())?;
        fs::write(dir.join(CORPUS_MANIFEST), "{\"v\":2}").map_err(|e| e.to_string())?;
        let drift = receipt_drift(&dir)?;
        let advisory = check_receipts(&dir);
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        assert!(advisory.is_err_and(|err| err.contains("--refresh-receipts")));
        assert_eq!(drift.len(), 2);
        assert!(drift.iter().any(|line| line.contains("dx-scoreboard.json")));
        assert!(
            drift
                .iter()
                .any(|line| line.contains("corpus-manifest.json"))
        );
        Ok(())
    }

    fn set_metric(receipts: &mut Receipts, id: &str, edit: impl FnOnce(&mut Value)) {
        if let Some(metrics) = receipts.dx.get_mut("metrics").and_then(Value::as_array_mut) {
            for metric in metrics {
                if metric.get("id").and_then(Value::as_str) == Some(id) {
                    edit(metric);
                    return;
                }
            }
        }
    }

    fn bar_named(receipts: &Receipts, id: &str) -> Result<Bar, String> {
        bars(receipts)?
            .into_iter()
            .find(|bar| bar.id == id)
            .ok_or_else(|| format!("no bar {id}"))
    }

    #[test]
    fn cross_runner_baseline_is_disclosed_not_a_first_measurement() -> Result<(), String> {
        let receipts = load(&workspace_root())?;
        let bar = bar_named(&receipts, "speed.cold_pilot_ms")?;
        assert!(
            bar.trend.contains("another runner class"),
            "trend was {}",
            bar.trend
        );
        assert!(
            bar.trend.contains("local-linux-x86_64-4cpu"),
            "trend was {}",
            bar.trend
        );
        assert!(bar.trend.contains("237.7 s"), "trend was {}", bar.trend);
        assert!(
            !bar.trend.contains("no earlier measurement"),
            "trend was {}",
            bar.trend
        );
        let all = bars(&receipts)?;
        let mut page = Page(String::new());
        scoreboard(&mut page, &all);
        assert!(
            page.0
                .contains("A cross-class trend shows the earlier runner class"),
            "{}",
            page.0
        );
        Ok(())
    }

    #[test]
    fn cross_runner_without_a_prior_value_still_names_the_class() {
        let current_only = serde_json::json!({
            "unit": "ms",
            "value": 77565.7,
            "baseline": {
                "comparable": false,
                "reason": "runner class differs (baseline `local-linux-x86_64-4cpu`, current `hosted`)"
            }
        });
        assert_eq!(
            scoreboard_trend(&current_only, "abc1234"),
            "earlier receipt on another runner class `local-linux-x86_64-4cpu`"
        );
        let unparsed = serde_json::json!({
            "unit": "ms",
            "value": 1.0,
            "baseline": {
                "comparable": false,
                "reason": "runner class differs"
            }
        });
        assert_eq!(
            scoreboard_trend(&unparsed, "abc1234"),
            "earlier receipt on another runner class"
        );
        let neither = serde_json::json!({
            "unit": "s",
            "baseline": {
                "comparable": false,
                "reason": "runner class differs (baseline `local-linux-x86_64-4cpu`, current `hosted`)"
            }
        });
        assert_eq!(
            scoreboard_trend(&neither, "abc1234"),
            "no earlier measurement"
        );
    }

    #[test]
    fn absent_baseline_stays_a_first_measurement() {
        let metric = serde_json::json!({
            "unit": "s",
            "baseline": {
                "comparable": false,
                "reason": "metric absent from baseline"
            }
        });
        assert_eq!(
            scoreboard_trend(&metric, "abc1234"),
            "no earlier measurement"
        );
        let comparable = serde_json::json!({
            "unit": "lines",
            "baseline": {
                "comparable": true,
                "value": 1154.0,
                "delta": -767.0
            }
        });
        assert_eq!(
            scoreboard_trend(&comparable, "c6ccf9d"),
            "-767 lines since c6ccf9d (was 1154 lines)"
        );
    }

    #[test]
    fn unowned_boards_use_lane_baselines_or_are_omitted() -> Result<(), String> {
        let receipts = load(&workspace_root())?;
        let all = bars(&receipts)?;
        let corpus = bar_named(&receipts, "corpus.not_analyzed")?;
        assert!(
            corpus.status == Status::Meets,
            "corpus.not_analyzed should meet a zero-unanalyzed bar"
        );
        assert_eq!(corpus.value, Some(0.0));
        assert!(
            corpus.basis.contains("corpus-") && corpus.basis.ends_with("-baseline.json"),
            "basis was {}",
            corpus.basis
        );
        let ranking = bar_named(&receipts, "ranking.pilot_precision_top5")?;
        assert!(ranking.status != Status::NotMeasured);
        assert!(ranking.value.is_some());
        assert!(
            all.iter()
                .all(|bar| !bar.id.starts_with("agent.") || bar.status != Status::NotMeasured),
            "unowned agent rows must not list as not measured"
        );
        assert!(
            all.iter()
                .any(|bar| bar.id == "ci.install_seconds" && bar.status == Status::NotMeasured),
            "a scoreboard-owned gap must stay visible"
        );
        Ok(())
    }

    #[test]
    fn scoreboard_owned_measurement_outranks_a_lane_baseline() -> Result<(), String> {
        let mut receipts = load(&workspace_root())?;
        set_metric(&mut receipts, "corpus.not_analyzed", |metric| {
            metric["value"] = serde_json::json!(5.0);
            metric["status"] = Value::from("below_target");
            metric["reason"] = Value::from("");
        });
        let bar = bar_named(&receipts, "corpus.not_analyzed")?;
        assert_eq!(bar.value, Some(5.0));
        assert!(bar.status == Status::Below);
        assert!(
            !bar.basis.contains("corpus-full-baseline"),
            "basis was {}",
            bar.basis
        );
        Ok(())
    }

    #[test]
    fn missing_lane_baseline_omits_the_unowned_board() -> Result<(), String> {
        let mut receipts = load(&workspace_root())?;
        receipts.corpus_lane = None;
        receipts.ranking_lane = None;
        let all = bars(&receipts)?;
        assert!(
            all.iter()
                .all(|bar| bar.id != "corpus.not_analyzed" && !bar.id.starts_with("ranking.")),
            "unowned boards without a lane receipt must be omitted, not listed as not measured"
        );
        Ok(())
    }

    #[test]
    fn corpus_fast_is_used_when_full_is_absent() -> Result<(), String> {
        let mut receipts = load(&workspace_root())?;
        let Some(full) = receipts.corpus_lane.take() else {
            return Err("workspace is missing a corpus lane receipt".to_string());
        };
        let fast = items(&full.value, "metrics")
            .iter()
            .find(|metric| text(metric, "id") == "corpus.not_analyzed")
            .cloned()
            .ok_or("full corpus receipt missing corpus.not_analyzed")?;
        receipts.corpus_lane = Some(LaneReceipt {
            path: CORPUS_FAST_BASELINE.to_string(),
            value: serde_json::json!({"metrics": [fast]}),
        });
        let bar = bar_named(&receipts, "corpus.not_analyzed")?;
        assert_eq!(bar.basis, CORPUS_FAST_BASELINE);
        assert!(bar.value.is_some());
        Ok(())
    }

    #[test]
    fn lane_revision_uses_the_receipt_revision_not_the_comparison_baseline() {
        let receipt = serde_json::json!({
            "revision": "10e56371911a4e6ef8020fbcd7e5f2692fc9b7f1",
            "gate": {"baseline_revision": "adf4e6303d53a9b0cd5ec7d1c29d71996b02c8db"}
        });
        assert_eq!(lane_revision(&receipt), "10e5637");
        assert_eq!(
            lane_revision(&serde_json::json!({"revision": "unavailable"})),
            "revision unavailable"
        );
        assert_eq!(
            lane_revision(&serde_json::json!({})),
            "revision not recorded"
        );
    }

    #[test]
    fn receipts_table_names_lane_revisions() -> Result<(), String> {
        let receipts = load(&workspace_root())?;
        let mut page = Page(String::new());
        header(&mut page, &receipts)?;
        assert!(
            page.0.contains("10e5637"),
            "corpus lane revision missing: {}",
            page.0
        );
        assert!(
            page.0.contains("4f81060"),
            "ranking lane revision missing: {}",
            page.0
        );
        Ok(())
    }

    #[test]
    fn failed_lane_instrument_stays_visible() -> Result<(), String> {
        let mut receipts = load(&workspace_root())?;
        receipts.corpus_lane = Some(LaneReceipt {
            path: CORPUS_FULL_BASELINE.to_string(),
            value: serde_json::json!({
                "metrics": [{
                    "id": "corpus.check_ms",
                    "board": "corpus",
                    "unit": "ms",
                    "target": 5000.0,
                    "direction": "lower_is_better",
                    "status": "failed",
                    "value": null,
                    "reason": "clone failed",
                    "title": "Diff-scoped check"
                }]
            }),
        });
        let bar = bar_named(&receipts, "corpus.check_ms")?;
        assert!(bar.status == Status::Failed);
        assert!(
            bar.basis.contains("instrument failed: clone failed"),
            "basis was {}",
            bar.basis
        );
        Ok(())
    }

    fn dummy_bar(id: &str, status: Status, trend: &str, value: Option<f64>) -> Bar {
        Bar {
            id: id.to_string(),
            board: "CI adoption".to_string(),
            title: id.to_string(),
            value,
            unit: "s".to_string(),
            target: 30.0,
            lower_is_better: true,
            status,
            trend: trend.to_string(),
            basis: String::new(),
        }
    }

    #[test]
    fn scoreboard_header_does_not_claim_cross_class_trends_compare() {
        let summary = scoreboard_summary(
            1,
            0,
            1,
            0,
            String::new(),
            &[dummy_bar(
                "speed.cold_pilot_ms",
                Status::Below,
                "earlier receipt on another runner class `local` (was 2 s)",
                Some(1.0),
            )],
        );
        assert!(summary.contains("does not compare measurements"));
        assert!(!summary.contains("A trend names the earlier receipt it compares against"));
    }

    #[test]
    fn scoreboard_header_does_not_call_unmeasured_rows_first_measurements() -> Result<(), String> {
        let receipts = load(&workspace_root())?;
        let all = bars(&receipts)?;
        assert!(
            all.iter().any(|bar| {
                bar.id == "ci.install_seconds"
                    && bar.status == Status::NotMeasured
                    && bar.trend.contains("no earlier measurement")
            }),
            "fixture must keep a scoreboard-owned unmeasured row"
        );
        assert!(
            all.iter().any(|bar| {
                matches!(bar.status, Status::Meets | Status::Below)
                    && bar.trend.contains("no earlier measurement")
            }),
            "fixture must keep a measured first receipt"
        );
        let mut page = Page(String::new());
        scoreboard(&mut page, &all);
        assert!(
            page.0
                .contains("A measured row with no earlier measurement is a first measurement"),
            "{}",
            page.0
        );
        assert!(
            !page
                .0
                .contains("Rows with no earlier measurement are first measurements"),
            "{}",
            page.0
        );
        Ok(())
    }

    #[test]
    fn scoreboard_header_omits_first_measurement_when_only_unmeasured_rows_lack_a_prior() {
        let summary = scoreboard_summary(
            1,
            0,
            0,
            1,
            String::new(),
            &[dummy_bar(
                "ci.install_seconds",
                Status::NotMeasured,
                "no earlier measurement",
                None,
            )],
        );
        assert!(
            !summary.contains("first measurement"),
            "unmeasured rows are gaps, not first measurements: {summary}"
        );
    }

    #[test]
    fn load_lane_prefers_full_and_falls_back_to_fast() -> Result<(), String> {
        let dir =
            std::env::temp_dir().join(format!("ripr-public-proof-lane-{}", std::process::id()));
        fs::create_dir_all(dir.join("metrics/dx-scoreboard")).map_err(|e| e.to_string())?;
        fs::write(dir.join(CORPUS_FAST_BASELINE), "{\"via\":\"fast\"}")
            .map_err(|e| e.to_string())?;
        let fallback = load_lane(&dir, CORPUS_FULL_BASELINE, Some(CORPUS_FAST_BASELINE))?
            .ok_or("fast fallback was not used")?;
        assert_eq!(fallback.path, CORPUS_FAST_BASELINE);
        assert_eq!(fallback.value["via"], "fast");
        fs::write(dir.join(CORPUS_FULL_BASELINE), "{\"via\":\"full\"}")
            .map_err(|e| e.to_string())?;
        let preferred = load_lane(&dir, CORPUS_FULL_BASELINE, Some(CORPUS_FAST_BASELINE))?
            .ok_or("full receipt was not preferred")?;
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        assert_eq!(preferred.path, CORPUS_FULL_BASELINE);
        assert_eq!(preferred.value["via"], "full");
        Ok(())
    }

    #[test]
    fn lane_check_time_discloses_its_cross_class_earlier_receipt() -> Result<(), String> {
        let receipts = load(&workspace_root())?;
        let bar = bar_named(&receipts, "corpus.check_ms")?;
        assert!(bar.value.is_some());
        assert!(
            bar.trend.contains("another runner class"),
            "trend was {}",
            bar.trend
        );
        Ok(())
    }
}
