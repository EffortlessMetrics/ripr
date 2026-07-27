//! Goldens + fixture-runner + drift-semantics cluster: the fixture runner
//! (`fixtures` gate, per-fixture `ripr` invocation, golden comparison), the
//! `goldens check` / `goldens bless` machinery, the `golden-drift` report, and
//! the drift-semantics classification that separates semantic drift from
//! formatting-only drift.
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items are `pub(crate)` and re-exported from `main.rs` so
//! existing call sites (`dispatch.rs`, `dogfood.rs`, `fixture_contracts`,
//! `evidence_promotion`, and `tests.rs`) compile unchanged.

use crate::no_panic::contains_word;
use crate::run::{run, run_output_owned};
use crate::{
    collect_pr_changes, forbidden_static_terms, has_markdown_heading, json_escape, markdown_cell,
    normalize_path, read_text_lossy, ripr_debug_binary, write_json_string_array, write_report,
};
use rayon::prelude::*;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn fixtures_impl(name: Option<&String>) -> Result<(), String> {
    // Build once per invocation (#2110): every scenario then invokes the
    // fresh worktree binary directly instead of paying `cargo run` + link
    // check per scenario.
    run("cargo", &["build", "-p", "ripr"])?;
    let fixture_dirs = fixture_dirs()?;
    let selected = match name {
        Some(value) => vec![fixture_dir_for_name(value)?],
        None => fixture_dirs,
    };
    // Parallelize fixture execution via rayon (#2415). Each fixture is
    // independent (disjoint input/expected dirs), so parallel runs produce
    // identical output.
    let results: Vec<Result<(Vec<String>, Option<FixtureRun>), String>> = selected
        .par_iter()
        .map(|path| {
            if !path.exists() {
                return Ok((
                    vec![format!("fixture does not exist: {}", normalize_path(path))],
                    None,
                ));
            }
            if !path.is_dir() {
                return Ok((
                    vec![format!(
                        "fixture is not a directory: {}",
                        normalize_path(path)
                    )],
                    None,
                ));
            }
            let contract_violations = fixture_contract_violations(path)?;
            if !contract_violations.is_empty() {
                return Ok((contract_violations, None));
            }
            match run_fixture(path) {
                Ok(run) => Ok((run.comparison_violations(), Some(run))),
                Err(err) => Err(err),
            }
        })
        .collect();

    let mut violations = Vec::new();
    let mut runs = Vec::new();
    for result in results {
        match result {
            Ok((vios, maybe_run)) => {
                violations.extend(vios);
                if let Some(run) = maybe_run {
                    runs.push(run);
                }
            }
            Err(err) => violations.push(err),
        }
    }

    let body = fixture_report_body(name.map(String::as_str), &selected, &runs, &violations);
    write_report("fixtures.md", &body)?;

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "fixture command failed; see target/ripr/reports/fixtures.md\n{}",
            violations.join("\n")
        ))
    }
}

pub(crate) fn goldens_impl(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("check") => goldens_check(),
        Some("bless") => {
            let Some(name) = args.get(1) else {
                return Err(
                    "goldens bless requires a fixture name\nusage: cargo xtask goldens bless <name> --reason <reason>"
                        .to_string(),
                );
            };
            let reason = parse_reason(&args[2..])?;
            goldens_bless(name, &reason)
        }
        Some(other) => Err(format!(
            "unknown goldens command `{other}`\nusage: cargo xtask goldens check\n       cargo xtask goldens bless <name> --reason <reason>"
        )),
        None => Err(
            "missing goldens command\nusage: cargo xtask goldens check\n       cargo xtask goldens bless <name> --reason <reason>"
                .to_string(),
        ),
    }
}

pub(crate) fn goldens_check() -> Result<(), String> {
    let run_set = collect_golden_runs()?;
    let entries = write_golden_drift_reports(&run_set.runs, &run_set.violations)?;
    let body = goldens_check_report_body(&run_set.fixtures, &run_set.runs, &run_set.violations);
    write_report("goldens.md", &body)?;
    if run_set.violations.is_empty() {
        Ok(())
    } else {
        Err(goldens_check_failure_message(&entries, &run_set.violations))
    }
}

/// Build an actionable `goldens check` failure: each drifted fixture gets its
/// semantic drift type (the discriminator from [`golden_drift_type`]) and a
/// blessing-state note, non-drift violations (contract/run errors) are listed
/// verbatim, and the message ends with the reproduce + next-action commands. The
/// gate emits its own repair card instead of "failed; see report".
pub(crate) fn goldens_check_failure_message(
    entries: &[GoldenDriftEntry],
    violations: &[String],
) -> String {
    let mut message = String::from(
        "goldens check failed; semantic drift detail in target/ripr/reports/golden-drift.md\n",
    );
    for entry in entries {
        let blessing = if entry.blessing_reason_present {
            "blessing CHANGELOG present"
        } else {
            "no blessing CHANGELOG — re-bless if the flip is intended"
        };
        message.push_str(&format!(
            "  drift: fixture `{}` [{}]: {} ({})\n",
            entry.fixture,
            entry.surface,
            golden_drift_type(&entry.semantics),
            blessing,
        ));
    }
    // Violations that are not per-fixture output drift (contract or run errors)
    // are not represented in `entries`; surface them verbatim so nothing is lost.
    for violation in violations
        .iter()
        .filter(|violation| !violation.contains("drift for fixture"))
    {
        message.push_str(&format!("  error: {violation}\n"));
    }
    message.push_str("reproduce: cargo xtask goldens check\n");
    message.push_str(
        "next: inspect target/ripr/reports/golden-drift.md; if a flip is intended, run `cargo xtask goldens bless <fixture> --reason <reason>`",
    );
    message
}

pub(crate) fn golden_drift_impl() -> Result<(), String> {
    let run_set = collect_golden_runs()?;
    write_golden_drift_reports(&run_set.runs, &run_set.violations)?;
    if run_set
        .violations
        .iter()
        .any(|violation| !violation.contains("drift for fixture"))
    {
        Err(format!(
            "golden drift report had fixture errors; see target/ripr/reports/golden-drift.md\n{}",
            run_set.violations.join("\n")
        ))
    } else {
        Ok(())
    }
}

fn collect_golden_runs() -> Result<GoldenRunSet, String> {
    let fixture_dirs = fixture_dirs()?;
    // Parallelize fixture execution via rayon (#2415). Each fixture is independent
    // (disjoint input/expected dirs), so parallel runs produce identical output.
    // The sequential results are collected in input order for deterministic reports.
    let results: Vec<Result<(Vec<String>, Option<FixtureRun>), String>> = fixture_dirs
        .par_iter()
        .map(|fixture| {
            let contract_violations = fixture_contract_violations(fixture)?;
            if !contract_violations.is_empty() {
                return Ok((contract_violations, None));
            }
            match run_fixture(fixture) {
                Ok(run) => Ok((run.comparison_violations(), Some(run))),
                Err(err) => Err(err),
            }
        })
        .collect();

    let mut violations = Vec::new();
    let mut runs = Vec::new();
    for result in results {
        match result {
            Ok((vios, maybe_run)) => {
                violations.extend(vios);
                if let Some(run) = maybe_run {
                    runs.push(run);
                }
            }
            Err(err) => violations.push(err),
        }
    }
    Ok(GoldenRunSet {
        fixtures: fixture_dirs,
        runs,
        violations,
    })
}

fn goldens_bless(name: &str, reason: &str) -> Result<(), String> {
    let fixture = fixture_dir_for_name(name)?;
    if !fixture.exists() {
        return Err(format!(
            "fixture does not exist: {}",
            normalize_path(&fixture)
        ));
    }
    let run = run_fixture_outputs(&fixture)?;

    // Integrity guards (#2410): validate the run output BEFORE copying it
    // over the expected files. A regression that produces empty/garbage
    // output must not be silently blessed as the new golden.
    validate_bless_output(&run, &fixture)?;

    let expected = fixture.join("expected");
    fs::create_dir_all(&expected)
        .map_err(|err| format!("failed to create {}: {err}", normalize_path(&expected)))?;
    fs::copy(&run.check_json, expected.join("check.json")).map_err(|err| {
        format!(
            "failed to update {} from {}: {err}",
            normalize_path(&expected.join("check.json")),
            normalize_path(&run.check_json)
        )
    })?;
    fs::copy(&run.human_txt, expected.join("human.txt")).map_err(|err| {
        format!(
            "failed to update {} from {}: {err}",
            normalize_path(&expected.join("human.txt")),
            normalize_path(&run.human_txt)
        )
    })?;
    let expected_human_full = expected.join("human-full.txt");
    let updated_human_full = expected_human_full.exists();
    if updated_human_full {
        fs::copy(&run.human_full_txt, &expected_human_full).map_err(|err| {
            format!(
                "failed to update {} from {}: {err}",
                normalize_path(&expected_human_full),
                normalize_path(&run.human_full_txt)
            )
        })?;
    }
    let changelog = expected.join("CHANGELOG.md");
    let mut entry = format!(
        "\n## Pending\n\nReason:\n{reason}\n\nCommand:\n`cargo xtask goldens bless {name} --reason \"...\"`\n\nUpdated:\n- `expected/check.json`\n- `expected/human.txt`\n"
    );
    if updated_human_full {
        entry.push_str("- `expected/human-full.txt`\n");
    }
    let mut text = if changelog.exists() {
        read_text_lossy(&changelog)?
    } else {
        "# Golden Output Changes\n".to_string()
    };
    text.push_str(&entry);
    fs::write(&changelog, text)
        .map_err(|err| format!("failed to write {}: {err}", normalize_path(&changelog)))?;
    let mut actual_outputs = format!(
        "- `{}`\n- `{}`\n",
        normalize_path(&run.check_json),
        normalize_path(&run.human_txt)
    );
    let mut updated_outputs = format!(
        "- `{}`\n- `{}`\n",
        normalize_path(&expected.join("check.json")),
        normalize_path(&expected.join("human.txt"))
    );
    if updated_human_full {
        actual_outputs.push_str(&format!("- `{}`\n", normalize_path(&run.human_full_txt)));
        updated_outputs.push_str(&format!("- `{}`\n", normalize_path(&expected_human_full)));
    }
    updated_outputs.push_str(&format!("- `{}`\n", normalize_path(&changelog)));
    let body = format!(
        "# ripr goldens bless report\n\nStatus: pass\n\nFixture:\n- `{}`\n\nReason:\n```text\n{reason}\n```\n\nActual outputs:\n{actual_outputs}\nUpdated:\n{updated_outputs}",
        normalize_path(&fixture)
    );
    write_report("goldens-bless.md", &body)
}

pub(crate) fn fixture_dirs() -> Result<Vec<PathBuf>, String> {
    let fixtures_dir = Path::new("fixtures");
    if !fixtures_dir.exists() {
        return Ok(Vec::new());
    }
    let mut fixtures = Vec::new();
    for entry in
        fs::read_dir(fixtures_dir).map_err(|err| format!("failed to read fixtures: {err}"))?
    {
        let entry = entry.map_err(|err| format!("failed to read fixtures: {err}"))?;
        let path = entry.path();
        if path.is_dir() && !is_manifest_only_fixture_dir(&path) {
            fixtures.push(path);
        }
    }
    fixtures.sort();
    Ok(fixtures)
}

pub(crate) fn is_manifest_only_fixture_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                "active-goal-authority-audit"
                    | "actionable-gap-outcomes-corpus"
                    | "bun-ub-cross-language-dogfood"
                    | "cross-language-oracle-graph-corpus"
                    | "editor_gap_cockpit"
                    | "editor_first_run_usability"
                    | "editor_first_pr_bridge"
                    | "editor_adoption_assurance"
                    | "editor_actionable_gap_queue"
                    | "evidence-promotion-honesty-corpus"
                    | "evidence-quality-benchmark"
                    | "first_successful_pr"
                    | "finding-alignment-dogfood"
                    | "gap-decision-ledger"
                    | "perl_lsp_facts_exporter"
                    | "perl-real-repo-evals"
                    | "python"
                    | "python-eval-sweep"
                    | "python-judged-pr-panel"
                    | "python-real-repo-evals"
                    | "real-repair-attempts"
                    | "surface-projection-alignment"
                    | "swarm-plan-packet-corpus"
                    | "typescript-bun-ub-calibration"
                    | "typescript-preview-false-actionable-audit"
                    | "typescript-preview-repair-loop"
                    | "user-surface-projection-alignment"
            )
        })
}

fn fixture_dir_for_name(name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err(format!("invalid fixture name `{name}`"));
    }
    Ok(Path::new("fixtures").join(name))
}

pub(crate) fn fixture_contract_violations(path: &Path) -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    if is_manifest_only_fixture_dir(path) {
        return Ok(violations);
    }
    let normalized = normalize_path(path);
    let spec = path.join("SPEC.md");
    let diff = path.join("diff.patch");
    let expected_check = path.join("expected/check.json");

    if !spec.exists() {
        violations.push(format!("{normalized} is missing SPEC.md"));
        return Ok(violations);
    }
    if !diff.exists() {
        violations.push(format!("{normalized} is missing diff.patch"));
    }
    if !expected_check.exists() {
        violations.push(format!("{normalized} is missing expected/check.json"));
    }

    let text = read_text_lossy(&spec)?;
    if !text
        .lines()
        .any(|line| line.starts_with("Spec: RIPR-SPEC-"))
    {
        violations.push(format!(
            "{} is missing `Spec: RIPR-SPEC-NNNN`",
            normalize_path(&spec)
        ));
    }
    for heading in ["## Given", "## When", "## Then", "## Must Not"] {
        if !has_markdown_heading(&text, heading) {
            violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
        }
    }
    Ok(violations)
}

#[derive(Debug)]
struct FixtureRun {
    name: String,
    actual_dir: PathBuf,
    check_json: PathBuf,
    human_txt: PathBuf,
    human_full_txt: PathBuf,
    comparisons: Vec<GoldenComparison>,
}

impl FixtureRun {
    fn comparison_violations(&self) -> Vec<String> {
        self.comparisons
            .iter()
            .filter(|comparison| !comparison.matches)
            .map(|comparison| {
                let difference_hint = comparison
                    .first_difference
                    .as_ref()
                    .map(|hint| format!("\n  diff:    {hint}"))
                    .unwrap_or_default();
                format!(
                    "{} drift for fixture `{}`\n  expected: {}\n  actual:   {}{}",
                    comparison.surface,
                    self.name,
                    normalize_path(&comparison.expected),
                    normalize_path(&comparison.actual),
                    difference_hint
                )
            })
            .collect()
    }
}

#[derive(Debug)]
struct GoldenComparison {
    surface: &'static str,
    expected: PathBuf,
    actual: PathBuf,
    matches: bool,
    first_difference: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct GoldenDriftSemantics {
    pub(crate) added_finding_ids: Vec<String>,
    pub(crate) removed_finding_ids: Vec<String>,
    pub(crate) changed_exposure_classes: Vec<String>,
    pub(crate) changed_probe_families: Vec<String>,
    pub(crate) changed_oracle_kinds: Vec<String>,
    pub(crate) changed_oracle_strengths: Vec<String>,
    pub(crate) changed_stop_reasons: Vec<String>,
    pub(crate) changed_recommendations: Vec<String>,
    pub(crate) static_language_terms: Vec<String>,
    pub(crate) changed_line_count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct GoldenDriftEntry {
    pub(crate) fixture: String,
    pub(crate) surface: String,
    pub(crate) expected: String,
    pub(crate) actual: String,
    pub(crate) blessing_reason_required: bool,
    pub(crate) blessing_reason_present: bool,
    pub(crate) semantics: GoldenDriftSemantics,
}

#[derive(Debug)]
struct GoldenRunSet {
    fixtures: Vec<PathBuf>,
    runs: Vec<FixtureRun>,
    violations: Vec<String>,
}

fn run_fixture(path: &Path) -> Result<FixtureRun, String> {
    let run = run_fixture_outputs(path)?;
    let expected = path.join("expected");
    let comparisons = fixture_golden_comparisons(
        &expected,
        &run.check_json,
        &run.human_txt,
        &run.human_full_txt,
    )?;
    Ok(FixtureRun { comparisons, ..run })
}

/// Validate the run output before blessing it as the new golden (#2410).
/// Refuses to copy files that are malformed, empty, or show a suspicious
/// finding-count collapse — the classic regression signature.
fn validate_bless_output(run: &FixtureRun, fixture: &Path) -> Result<(), String> {
    // 1. check.json must be valid JSON with expected top-level fields.
    let json_text = read_text_lossy(&run.check_json)?;
    let parsed: serde_json::Value = serde_json::from_str(&json_text).map_err(|err| {
        format!(
            "goldens bless: check.json is not valid JSON for {}: {err}",
            run.name
        )
    })?;
    if parsed.get("findings").is_none() {
        return Err(format!(
            "goldens bless: check.json for {} has no 'findings' field — output may be malformed",
            run.name
        ));
    }

    // 2. Zero-finding guard: if the new output has 0 findings but the
    //    previous expected had >0, this is likely a regression. Compare
    //    against the existing expected/check.json before overwriting.
    let new_count = parsed["findings"].as_array().map(|a| a.len()).unwrap_or(0);
    if new_count == 0 {
        let expected_path = fixture.join("expected/check.json");
        if expected_path.exists()
            && let Ok(old_text) = read_text_lossy(&expected_path)
            && let Ok(old_json) = serde_json::from_str::<serde_json::Value>(&old_text)
        {
            let old_count = old_json["findings"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0);
            if old_count > 0 {
                return Err(format!(
                    "goldens bless: check.json for {} dropped from {old_count} to 0 findings — \
                     this likely indicates a regression, not a golden update. \
                     If intentional, inspect the diff manually before re-blessing.",
                    run.name
                ));
            }
        }
    }

    // 3. human.txt must be non-empty.
    let human_text = read_text_lossy(&run.human_txt)?;
    if human_text.trim().is_empty() {
        return Err(format!(
            "goldens bless: human.txt for {} is empty — output may be malformed",
            run.name
        ));
    }

    Ok(())
}

fn run_fixture_outputs(path: &Path) -> Result<FixtureRun, String> {
    let name = fixture_name(path)?;
    let diff = path.join("diff.patch");
    let input = path.join("input");
    if !diff.exists() {
        return Err(format!("{} is missing diff.patch", normalize_path(path)));
    }
    if !input.exists() {
        return Err(format!(
            "{} is missing input/ fixture workspace",
            normalize_path(path)
        ));
    }

    let actual_dir = Path::new("target")
        .join("ripr")
        .join("fixtures")
        .join(&name);
    fs::create_dir_all(&actual_dir).map_err(|err| {
        format!(
            "failed to create actual fixture output directory {}: {err}",
            normalize_path(&actual_dir)
        )
    })?;

    let check_json = actual_dir.join("check.json");
    let human_txt = actual_dir.join("human.txt");
    let human_full_txt = actual_dir.join("human-full.txt");
    let root = normalize_path(&input);
    let diff_file = normalize_path(&diff);

    let json = normalize_fixture_json_output(&run_fixture_check(
        &root,
        &diff_file,
        FixtureCheckFormat::Json,
    )?);
    fs::write(&check_json, json).map_err(|err| {
        format!(
            "failed to write actual fixture output {}: {err}",
            normalize_path(&check_json)
        )
    })?;

    let human = normalize_fixture_human_output(&run_fixture_check(
        &root,
        &diff_file,
        FixtureCheckFormat::Human,
    )?);
    fs::write(&human_txt, human).map_err(|err| {
        format!(
            "failed to write actual fixture output {}: {err}",
            normalize_path(&human_txt)
        )
    })?;

    let human_full = normalize_fixture_human_output(&run_fixture_check(
        &root,
        &diff_file,
        FixtureCheckFormat::HumanFull,
    )?);
    fs::write(&human_full_txt, human_full).map_err(|err| {
        format!(
            "failed to write actual fixture output {}: {err}",
            normalize_path(&human_full_txt)
        )
    })?;

    Ok(FixtureRun {
        name,
        actual_dir,
        check_json,
        human_txt,
        human_full_txt,
        comparisons: Vec::new(),
    })
}

#[derive(Clone, Copy)]
pub(crate) enum FixtureCheckFormat {
    Json,
    Human,
    HumanFull,
}

/// The worktree-absolute debug binary, built once per fixtures/dogfood
/// run (#2110): each scenario previously paid a `cargo run` spawn plus
/// link check; the absolute path keeps a long `../` from resolving to a
/// stale binary in the enclosing checkout.
pub(crate) fn ripr_fixture_binary() -> Result<String, String> {
    // ripr_debug_binary() honors CARGO_TARGET_DIR (#2176 review): the
    // routed CI jobs set it, so `cargo build` writes there.
    let binary = ripr_debug_binary();
    if !binary.exists() {
        run("cargo", &["build", "-p", "ripr"])?;
    }
    std::path::absolute(&binary)
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|err| format!("resolve {} failed: {err}", binary.display()))
}

pub(crate) fn run_fixture_check(
    root: &str,
    diff_file: &str,
    format: FixtureCheckFormat,
) -> Result<String, String> {
    let binary = ripr_fixture_binary()?;
    let mut args = vec![
        "check".to_string(),
        "--root".to_string(),
        root.to_string(),
        "--diff".to_string(),
        diff_file.to_string(),
        "--mode".to_string(),
        "fast".to_string(),
    ];
    match format {
        FixtureCheckFormat::Json => args.push("--json".to_string()),
        FixtureCheckFormat::Human => {}
        FixtureCheckFormat::HumanFull => {
            args.push("--format".to_string());
            args.push("human-full".to_string());
        }
    }
    run_output_owned(&binary, &args)
}

fn fixture_golden_comparisons(
    expected: &Path,
    check_json: &Path,
    human_txt: &Path,
    human_full_txt: &Path,
) -> Result<Vec<GoldenComparison>, String> {
    let mut comparisons = Vec::new();
    comparisons.push(compare_golden(
        "check.json",
        &expected.join("check.json"),
        check_json,
    )?);

    let expected_human = expected.join("human.txt");
    if expected_human.exists() {
        comparisons.push(compare_golden("human.txt", &expected_human, human_txt)?);
    }
    let expected_human_full = expected.join("human-full.txt");
    if expected_human_full.exists() {
        comparisons.push(compare_golden(
            "human-full.txt",
            &expected_human_full,
            human_full_txt,
        )?);
    }
    Ok(comparisons)
}

fn compare_golden(
    surface: &'static str,
    expected: &Path,
    actual: &Path,
) -> Result<GoldenComparison, String> {
    let expected_text = read_text_lossy(expected)?;
    let actual_text = read_text_lossy(actual)?;
    let normalized_expected = normalize_golden_text(&expected_text);
    let normalized_actual = normalize_golden_text(&actual_text);
    Ok(GoldenComparison {
        surface,
        expected: expected.to_path_buf(),
        actual: actual.to_path_buf(),
        matches: normalized_expected == normalized_actual,
        first_difference: first_line_difference(&normalized_expected, &normalized_actual),
    })
}

pub(crate) fn normalize_golden_text(value: &str) -> String {
    let normalized = value.replace("\r\n", "\n");
    normalized
        .strip_suffix('\n')
        .unwrap_or(&normalized)
        .to_string()
}

pub(crate) fn first_line_difference(expected: &str, actual: &str) -> Option<String> {
    let expected_lines: Vec<&str> = expected.split('\n').collect();
    let actual_lines: Vec<&str> = actual.split('\n').collect();
    let max_len = expected_lines.len().max(actual_lines.len());

    for index in 0..max_len {
        let expected_line = expected_lines.get(index).copied().unwrap_or("<missing>");
        let actual_line = actual_lines.get(index).copied().unwrap_or("<missing>");
        if expected_line != actual_line {
            return Some(format!(
                "line {} expected `{}` vs actual `{}`",
                index + 1,
                snapshot_line_preview(expected_line),
                snapshot_line_preview(actual_line)
            ));
        }
    }

    None
}

fn snapshot_line_preview(line: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 120;

    if line == "<missing>" {
        return line.to_string();
    }

    let escaped = line.escape_debug().to_string().replace('`', "\\`");
    let mut preview: String = escaped.chars().take(MAX_PREVIEW_CHARS).collect();
    if escaped.chars().count() > MAX_PREVIEW_CHARS {
        preview.push('…');
    }
    preview
}

pub(crate) fn normalize_fixture_json_output(value: &str) -> String {
    // #2337: match the human normalizer's approach — replace ALL backslashes
    // with forward slashes, not just the escaped double-backslash form. On
    // Windows, ripr check --json can emit single-backslash paths inside
    // free-text string values (error messages, code excerpts). The previous
    // normalizer only handled the `\\` (JSON-escaped) form, missing these.
    // First replace `\\` (the JSON-escaped form), then replace any remaining
    // single `\` — matching the human normalizer at line 737.
    value.replace("\\\\", "/").replace('\\', "/")
}

pub(crate) fn normalize_fixture_human_output(value: &str) -> String {
    let normalized = value.replace('\\', "/");
    let trimmed = normalized.trim_end_matches(['\r', '\n']);
    let mut output = trimmed.to_string();
    output.push('\n');
    output
}

fn fixture_name(path: &Path) -> Result<String, String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("invalid fixture path {}", normalize_path(path)))
}

fn fixture_report_body(
    name: Option<&str>,
    selected: &[PathBuf],
    runs: &[FixtureRun],
    violations: &[String],
) -> String {
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let mut body = format!("# ripr fixtures report\n\nStatus: {status}\n\n");
    match name {
        Some(value) => body.push_str(&format!("Requested fixture: `{value}`\n\n")),
        None => body.push_str("Requested fixture: all fixtures\n\n"),
    }
    body.push_str("## Fixtures\n\n");
    if selected.is_empty() {
        body.push_str("No fixture directories found.\n\n");
    } else {
        for path in selected {
            body.push_str(&format!("- `{}`\n", normalize_path(path)));
        }
        body.push('\n');
    }
    write_fixture_runs_section(&mut body, runs);
    write_violations_section(&mut body, violations);
    body
}

fn goldens_check_report_body(
    fixtures: &[PathBuf],
    runs: &[FixtureRun],
    violations: &[String],
) -> String {
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let mut body = format!("# ripr goldens report\n\nStatus: {status}\n\n");
    body.push_str("## Fixtures\n\n");
    if fixtures.is_empty() {
        body.push_str("No fixture directories found.\n\n");
    } else {
        for fixture in fixtures {
            body.push_str(&format!("- `{}`\n", normalize_path(fixture)));
        }
        body.push('\n');
    }
    write_fixture_runs_section(&mut body, runs);
    write_violations_section(&mut body, violations);
    body
}

fn write_golden_drift_reports(
    runs: &[FixtureRun],
    violations: &[String],
) -> Result<Vec<GoldenDriftEntry>, String> {
    let changed_paths = collect_changed_paths_set().unwrap_or_default();
    let entries = golden_drift_entries(runs, &changed_paths)?;
    let markdown = golden_drift_markdown(&entries, violations);
    let json = golden_drift_json(&entries, violations);
    write_report("golden-drift.md", &markdown)?;
    write_report("golden-drift.json", &json)?;
    Ok(entries)
}

/// Summarize a single golden drift as a semantic category, so a `goldens check`
/// failure says *what kind* of drift happened (classification flip, added/removed
/// finding, oracle change, banned static-language term, or formatting-only) rather
/// than only pointing at a report file. Reuses the already-computed
/// [`GoldenDriftSemantics`]; this is the discriminator a reviewer needs to decide
/// whether a flip is intended (re-bless) or a regression.
pub(crate) fn golden_drift_type(semantics: &GoldenDriftSemantics) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !semantics.changed_exposure_classes.is_empty() {
        parts.push(format!(
            "classification_changed [{}]",
            semantics.changed_exposure_classes.join("; ")
        ));
    }
    if !semantics.added_finding_ids.is_empty() {
        parts.push(format!(
            "finding_added x{}",
            semantics.added_finding_ids.len()
        ));
    }
    if !semantics.removed_finding_ids.is_empty() {
        parts.push(format!(
            "finding_removed x{}",
            semantics.removed_finding_ids.len()
        ));
    }
    if !semantics.changed_oracle_strengths.is_empty() {
        parts.push("oracle_strength_changed".to_string());
    }
    if !semantics.changed_oracle_kinds.is_empty() {
        parts.push("oracle_kind_changed".to_string());
    }
    if !semantics.changed_probe_families.is_empty() {
        parts.push("probe_family_changed".to_string());
    }
    if !semantics.changed_stop_reasons.is_empty() {
        parts.push("stop_reason_changed".to_string());
    }
    if !semantics.changed_recommendations.is_empty() {
        parts.push("recommendation_changed".to_string());
    }
    if !semantics.static_language_terms.is_empty() {
        parts.push(format!(
            "banned_static_language [{}]",
            semantics.static_language_terms.join(", ")
        ));
    }
    if parts.is_empty() {
        parts.push(format!(
            "formatting_only ({} line(s))",
            semantics.changed_line_count
        ));
    }
    parts.join(", ")
}

fn golden_drift_entries(
    runs: &[FixtureRun],
    changed_paths: &BTreeSet<String>,
) -> Result<Vec<GoldenDriftEntry>, String> {
    let mut entries = Vec::new();
    for run in runs {
        let changelog = Path::new("fixtures")
            .join(&run.name)
            .join("expected")
            .join("CHANGELOG.md");
        let blessing_reason_present = changed_paths.contains(&normalize_path(&changelog));
        for comparison in &run.comparisons {
            if comparison.matches {
                continue;
            }
            let expected = read_text_lossy(&comparison.expected)?;
            let actual = read_text_lossy(&comparison.actual)?;
            entries.push(GoldenDriftEntry {
                fixture: run.name.clone(),
                surface: comparison.surface.to_string(),
                expected: normalize_path(&comparison.expected),
                actual: normalize_path(&comparison.actual),
                blessing_reason_required: true,
                blessing_reason_present,
                semantics: golden_drift_semantics(comparison.surface, &expected, &actual),
            });
        }
    }
    Ok(entries)
}

pub(crate) fn golden_drift_semantics(
    surface: &str,
    expected: &str,
    actual: &str,
) -> GoldenDriftSemantics {
    let changed_line_count = changed_line_count(expected, actual);
    let static_language_terms = static_language_terms(expected, actual);
    if surface == "check.json" {
        let expected_ids = json_string_values_for_key(expected, "id")
            .into_iter()
            .filter(|value| value.starts_with("probe:"))
            .collect::<BTreeSet<_>>();
        let actual_ids = json_string_values_for_key(actual, "id")
            .into_iter()
            .filter(|value| value.starts_with("probe:"))
            .collect::<BTreeSet<_>>();
        let expected_classes = json_string_values_for_key(expected, "classification");
        let actual_classes = json_string_values_for_key(actual, "classification");
        let expected_families = json_string_values_for_key(expected, "family");
        let actual_families = json_string_values_for_key(actual, "family");
        let expected_oracles = json_string_values_for_key(expected, "oracle_strength");
        let actual_oracles = json_string_values_for_key(actual, "oracle_strength");
        let expected_oracle_kinds = json_string_values_for_key(expected, "oracle_kind");
        let actual_oracle_kinds = json_string_values_for_key(actual, "oracle_kind");
        let expected_stop_reasons = json_stop_reason_values(expected);
        let actual_stop_reasons = json_stop_reason_values(actual);
        let expected_recommendations =
            json_string_values_for_key(expected, "recommended_next_step");
        let actual_recommendations = json_string_values_for_key(actual, "recommended_next_step");

        GoldenDriftSemantics {
            added_finding_ids: set_difference(&actual_ids, &expected_ids),
            removed_finding_ids: set_difference(&expected_ids, &actual_ids),
            changed_exposure_classes: set_change_summary(&expected_classes, &actual_classes),
            changed_probe_families: set_change_summary(&expected_families, &actual_families),
            changed_oracle_kinds: set_change_summary(&expected_oracle_kinds, &actual_oracle_kinds),
            changed_oracle_strengths: set_change_summary(&expected_oracles, &actual_oracles),
            changed_stop_reasons: set_change_summary(&expected_stop_reasons, &actual_stop_reasons),
            changed_recommendations: set_change_summary(
                &expected_recommendations,
                &actual_recommendations,
            ),
            static_language_terms,
            changed_line_count,
        }
    } else {
        let expected_recommendations = human_recommended_next_steps(expected);
        let actual_recommendations = human_recommended_next_steps(actual);
        let expected_stop_reasons = human_stop_reason_lines(expected);
        let actual_stop_reasons = human_stop_reason_lines(actual);
        GoldenDriftSemantics {
            changed_stop_reasons: set_change_summary(&expected_stop_reasons, &actual_stop_reasons),
            changed_recommendations: set_change_summary(
                &expected_recommendations,
                &actual_recommendations,
            ),
            static_language_terms,
            changed_line_count,
            ..GoldenDriftSemantics::default()
        }
    }
}

fn golden_drift_markdown(entries: &[GoldenDriftEntry], violations: &[String]) -> String {
    let status = if violations
        .iter()
        .any(|violation| !violation.contains("drift for fixture"))
    {
        "fail"
    } else if entries.is_empty() {
        "pass"
    } else {
        "warn"
    };
    let mut body = format!("# ripr golden drift report\n\nStatus: {status}\n\n");
    body.push_str("This report summarizes expected-output drift for reviewer inspection. It never blesses goldens.\n\n");
    body.push_str("## Summary\n\n");
    body.push_str(&format!("- drift entries: {}\n", entries.len()));
    body.push_str(&format!(
        "- fixture errors: {}\n",
        fixture_error_count(violations)
    ));
    body.push_str("\n## Drift\n\n");
    if entries.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for entry in entries {
            body.push_str(&format!("### `{}` `{}`\n\n", entry.fixture, entry.surface));
            body.push_str(&format!("- expected: `{}`\n", entry.expected));
            body.push_str(&format!("- actual: `{}`\n", entry.actual));
            body.push_str(&format!(
                "- changed lines: {}\n",
                entry.semantics.changed_line_count
            ));
            write_optional_list(
                &mut body,
                "added finding IDs",
                &entry.semantics.added_finding_ids,
            );
            write_optional_list(
                &mut body,
                "removed finding IDs",
                &entry.semantics.removed_finding_ids,
            );
            write_optional_list(
                &mut body,
                "changed exposure classes",
                &entry.semantics.changed_exposure_classes,
            );
            write_optional_list(
                &mut body,
                "changed probe families",
                &entry.semantics.changed_probe_families,
            );
            write_optional_list(
                &mut body,
                "changed oracle strengths",
                &entry.semantics.changed_oracle_strengths,
            );
            write_optional_list(
                &mut body,
                "changed oracle kinds",
                &entry.semantics.changed_oracle_kinds,
            );
            write_optional_list(
                &mut body,
                "changed stop reasons",
                &entry.semantics.changed_stop_reasons,
            );
            write_optional_list(
                &mut body,
                "changed recommended next steps",
                &entry.semantics.changed_recommendations,
            );
            if entry.semantics.static_language_terms.is_empty() {
                body.push_str("- static-language boundary: pass\n");
            } else {
                body.push_str("- static-language boundary: fail\n");
                write_optional_list(
                    &mut body,
                    "static-language terms",
                    &entry.semantics.static_language_terms,
                );
            }
            body.push_str(&format!(
                "- blessing reason required: {}\n",
                yes_no(entry.blessing_reason_required)
            ));
            body.push_str(&format!(
                "- blessing reason present in PR: {}\n\n",
                yes_no(entry.blessing_reason_present)
            ));
        }
    }
    if !violations.is_empty() {
        body.push_str("## Fixture / Golden Violations\n\n");
        write_violations_section(&mut body, violations);
    }
    body
}

fn golden_drift_json(entries: &[GoldenDriftEntry], violations: &[String]) -> String {
    let status = if violations
        .iter()
        .any(|violation| !violation.contains("drift for fixture"))
    {
        "fail"
    } else if entries.is_empty() {
        "pass"
    } else {
        "warn"
    };
    let mut body = String::from("{\n");
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str(&format!("  \"status\": \"{}\",\n", json_escape(status)));
    body.push_str(&format!("  \"drift_count\": {},\n", entries.len()));
    body.push_str("  \"entries\": [\n");
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"fixture\": \"{}\",\n",
            json_escape(&entry.fixture)
        ));
        body.push_str(&format!(
            "      \"surface\": \"{}\",\n",
            json_escape(&entry.surface)
        ));
        body.push_str(&format!(
            "      \"expected\": \"{}\",\n",
            json_escape(&entry.expected)
        ));
        body.push_str(&format!(
            "      \"actual\": \"{}\",\n",
            json_escape(&entry.actual)
        ));
        body.push_str(&format!(
            "      \"changed_line_count\": {},\n",
            entry.semantics.changed_line_count
        ));
        body.push_str(&format!(
            "      \"blessing_reason_required\": {},\n",
            entry.blessing_reason_required
        ));
        body.push_str(&format!(
            "      \"blessing_reason_present\": {},\n",
            entry.blessing_reason_present
        ));
        write_json_field_array(
            &mut body,
            "added_finding_ids",
            &entry.semantics.added_finding_ids,
            true,
        );
        write_json_field_array(
            &mut body,
            "removed_finding_ids",
            &entry.semantics.removed_finding_ids,
            true,
        );
        write_json_field_array(
            &mut body,
            "changed_exposure_classes",
            &entry.semantics.changed_exposure_classes,
            true,
        );
        write_json_field_array(
            &mut body,
            "changed_probe_families",
            &entry.semantics.changed_probe_families,
            true,
        );
        write_json_field_array(
            &mut body,
            "changed_oracle_strengths",
            &entry.semantics.changed_oracle_strengths,
            true,
        );
        write_json_field_array(
            &mut body,
            "changed_oracle_kinds",
            &entry.semantics.changed_oracle_kinds,
            true,
        );
        write_json_field_array(
            &mut body,
            "changed_stop_reasons",
            &entry.semantics.changed_stop_reasons,
            true,
        );
        write_json_field_array(
            &mut body,
            "changed_recommendations",
            &entry.semantics.changed_recommendations,
            true,
        );
        write_json_field_array(
            &mut body,
            "static_language_terms",
            &entry.semantics.static_language_terms,
            false,
        );
        body.push_str("\n    }");
    }
    body.push_str("\n  ],\n");
    body.push_str("  \"violations\": [");
    write_json_string_array(&mut body, violations);
    body.push_str("]\n");
    body.push_str("}\n");
    body
}

fn write_fixture_runs_section(body: &mut String, runs: &[FixtureRun]) {
    body.push_str("## Actual Outputs\n\n");
    if runs.is_empty() {
        body.push_str("No fixture outputs generated.\n\n");
        return;
    }
    for run in runs {
        body.push_str(&format!(
            "- `{}` -> `{}`\n",
            run.name,
            normalize_path(&run.actual_dir)
        ));
        body.push_str(&format!("  - `{}`\n", normalize_path(&run.check_json)));
        body.push_str(&format!("  - `{}`\n", normalize_path(&run.human_txt)));
    }
    body.push('\n');

    body.push_str("## Golden Comparisons\n\n");
    for run in runs {
        if run.comparisons.is_empty() {
            body.push_str(&format!(
                "- `{}`: no expected outputs compared.\n",
                run.name
            ));
            continue;
        }
        for comparison in &run.comparisons {
            let status = if comparison.matches { "pass" } else { "fail" };
            body.push_str(&format!(
                "- `{}` `{}`: {status}\n  - expected: `{}`\n  - actual: `{}`\n",
                run.name,
                comparison.surface,
                normalize_path(&comparison.expected),
                normalize_path(&comparison.actual)
            ));
        }
    }
    body.push('\n');
}

fn write_optional_list(body: &mut String, label: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    body.push_str(&format!("- {label}:\n"));
    for value in values {
        body.push_str(&format!("  - `{}`\n", markdown_cell(value)));
    }
}

fn write_json_field_array(body: &mut String, key: &str, values: &[String], trailing_comma: bool) {
    body.push_str(&format!("      \"{}\": [", json_escape(key)));
    write_json_string_array(body, values);
    body.push(']');
    if trailing_comma {
        body.push(',');
    }
    body.push('\n');
}

pub(crate) fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn fixture_error_count(violations: &[String]) -> usize {
    violations
        .iter()
        .filter(|violation| !violation.contains("drift for fixture"))
        .count()
}

fn changed_line_count(expected: &str, actual: &str) -> usize {
    let expected_lines = normalize_golden_text(expected)
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let actual_lines = normalize_golden_text(actual)
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let max_len = expected_lines.len().max(actual_lines.len());
    let mut changed = 0usize;
    for index in 0..max_len {
        if expected_lines.get(index) != actual_lines.get(index) {
            changed += 1;
        }
    }
    changed
}

fn static_language_terms(expected: &str, actual: &str) -> Vec<String> {
    let combined = format!("{expected}\n{actual}").to_ascii_lowercase();
    forbidden_static_terms()
        .into_iter()
        .filter(|term| contains_word(&combined, term))
        .collect()
}

fn set_difference(left: &BTreeSet<String>, right: &BTreeSet<String>) -> Vec<String> {
    left.difference(right).cloned().collect()
}

fn set_change_summary(expected: &BTreeSet<String>, actual: &BTreeSet<String>) -> Vec<String> {
    if expected == actual {
        Vec::new()
    } else {
        vec![format!(
            "expected [{}] -> actual [{}]",
            expected.iter().cloned().collect::<Vec<_>>().join(", "),
            actual.iter().cloned().collect::<Vec<_>>().join(", ")
        )]
    }
}

fn json_stop_reason_values(text: &str) -> BTreeSet<String> {
    let mut values = json_string_values_for_key(text, "stop_reason");
    values.extend(json_string_values_for_key(text, "stop_reasons"));
    values
}

pub(crate) fn json_string_values_for_key(text: &str, key: &str) -> BTreeSet<String> {
    let needle = format!("\"{key}\"");
    let mut values = BTreeSet::new();
    let mut multiline = String::new();
    let mut collecting = false;
    for line in text.lines() {
        if collecting {
            multiline.push(' ');
            multiline.push_str(line.trim());
            if line.contains(']') {
                values.extend(json_strings_in_fragment(&multiline));
                multiline.clear();
                collecting = false;
            }
            continue;
        }
        let Some((_, rest)) = line.split_once(&needle) else {
            continue;
        };
        let Some((_, value)) = rest.split_once(':') else {
            continue;
        };
        let trimmed = value.trim();
        if trimmed.starts_with('[') && !trimmed.contains(']') {
            multiline.push_str(trimmed);
            collecting = true;
        } else {
            values.extend(json_strings_in_fragment(trimmed));
        }
    }
    values
}

fn json_strings_in_fragment(value: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut chars = value.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch != '"' {
            continue;
        }
        let mut current = String::new();
        let mut escaped = false;
        for (_, next) in chars.by_ref() {
            if escaped {
                current.push(match next {
                    '"' => '"',
                    '\\' => '\\',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    other => other,
                });
                escaped = false;
                continue;
            }
            if next == '\\' {
                escaped = true;
                continue;
            }
            if next == '"' {
                strings.push(current);
                break;
            }
            current.push(next);
        }
    }
    strings
}

fn human_recommended_next_steps(text: &str) -> BTreeSet<String> {
    human_section_lines(text, "Recommended next step:")
}

fn human_stop_reason_lines(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("stop reason") || lower.contains("stop:")
        })
        .map(str::to_string)
        .collect()
}

fn human_section_lines(text: &str, heading: &str) -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    let mut capture = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if capture {
            if trimmed.is_empty() {
                capture = false;
            } else {
                values.insert(trimmed.to_string());
            }
            continue;
        }
        if trimmed == heading {
            capture = true;
        }
    }
    values
}

fn collect_changed_paths_set() -> Result<BTreeSet<String>, String> {
    Ok(collect_pr_changes()?
        .into_iter()
        .map(|change| change.path)
        .collect())
}

fn write_violations_section(body: &mut String, violations: &[String]) {
    body.push_str("## Violations\n\n");
    if violations.is_empty() {
        body.push_str("None detected.\n");
    } else {
        for violation in violations {
            body.push_str("```text\n");
            body.push_str(violation);
            body.push_str("\n```\n\n");
        }
    }
}

pub(crate) fn parse_reason(args: &[String]) -> Result<String, String> {
    let mut index = 0;
    while index < args.len() {
        let value = &args[index];
        if let Some(reason) = value.strip_prefix("--reason=") {
            return non_empty_reason(reason);
        }
        if value == "--reason" {
            let Some(reason) = args.get(index + 1) else {
                return Err("--reason requires a value".to_string());
            };
            return non_empty_reason(reason);
        }
        index += 1;
    }
    Err("goldens bless requires --reason <reason>".to_string())
}

fn non_empty_reason(value: &str) -> Result<String, String> {
    let reason = value.trim();
    if reason.is_empty() {
        Err("--reason must not be empty".to_string())
    } else {
        Ok(reason.to_string())
    }
}

pub(crate) use self::fixtures_impl as fixtures;
pub(crate) use self::golden_drift_impl as golden_drift;
pub(crate) use self::goldens_impl as goldens;

/// Route `fixtures` subcommands: `new <name>` scaffolds, everything else runs (#2445).
pub(crate) fn fixtures_with_args(args: &[String]) -> Result<(), String> {
    if let Some(sub) = args.first().map(String::as_str)
        && sub == "new"
    {
        let name = args
            .get(1)
            .ok_or_else(|| "fixtures new requires a <name> argument".to_string())?;
        return fixtures_new(name);
    }
    // Fall through to the existing runner (pass the first arg as the optional fixture name).
    fixtures_impl(args.first())
}

/// Scaffold a new fixture directory with a seeded golden (#2445).
fn fixtures_new(name: &str) -> Result<(), String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        return Err(format!(
            "fixture name must be a simple directory name (no path separators): {name:?}"
        ));
    }
    let fixture_dir = Path::new("fixtures").join(name);
    if fixture_dir.exists() {
        return Err(format!("fixture already exists: {}", fixture_dir.display()));
    }

    // Create the directory tree.
    let input_dir = fixture_dir.join("input");
    let src_dir = input_dir.join("src");
    let expected_dir = fixture_dir.join("expected");
    std::fs::create_dir_all(&src_dir).map_err(|err| format!("create {src_dir:?}: {err}"))?;
    std::fs::create_dir_all(&expected_dir)
        .map_err(|err| format!("create {expected_dir:?}: {err}"))?;

    // Write SPEC.md with BDD structure.
    let spec = format!(
        "# Fixture: {name}\n\n\
         Spec: RIPR-SPEC-NNNN\n\n\
         ## Given\n\n\
         (Describe the input code and test state.)\n\n\
         ## When\n\n\
         (Describe the diff that changes behavior.)\n\n\
         ## Then\n\n\
         (Describe the expected ripr findings.)\n\n\
         ## Must Not\n\n\
         (Describe what ripr must NOT report.)\n"
    );
    std::fs::write(fixture_dir.join("SPEC.md"), spec)
        .map_err(|err| format!("write SPEC.md: {err}"))?;

    // Write a minimal input workspace.
    let cargo_toml =
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n");
    std::fs::write(input_dir.join("Cargo.toml"), cargo_toml)
        .map_err(|err| format!("write Cargo.toml: {err}"))?;

    let lib_rs = "// Edit this file to set up the input source code.\n\
                  pub fn example() -> i32 { 42 }\n";
    std::fs::write(src_dir.join("lib.rs"), lib_rs).map_err(|err| format!("write lib.rs: {err}"))?;

    // Write a minimal diff.patch.
    let diff = "--- Edit this diff to describe the behavior change.\n\
                diff --git a/src/lib.rs b/src/lib.rs\n\
                --- a/src/lib.rs\n+++ b/src/lib.rs\n\
                @@ -1,1 +1,1 @@\n\
                -pub fn example() -> i32 { 42 }\n\
                +pub fn example() -> i32 { 84 }\n";
    std::fs::write(fixture_dir.join("diff.patch"), diff)
        .map_err(|err| format!("write diff.patch: {err}"))?;

    // Seed expected/check.json by running ripr check once.
    let run = run_fixture_outputs(&fixture_dir);
    match run {
        Ok(run) => {
            // Copy the actual output to expected.
            std::fs::copy(&run.check_json, expected_dir.join("check.json"))
                .map_err(|err| format!("seed check.json: {err}"))?;
            if let Ok(human) = std::fs::read_to_string(&run.human_txt) {
                std::fs::write(expected_dir.join("human.txt"), human)
                    .map_err(|err| format!("seed human.txt: {err}"))?;
            }
        }
        Err(err) => {
            // Seeding failed — the fixture still has SPEC.md, input/, and diff.patch.
            // The contributor can debug and run `cargo xtask goldens bless <name>` later.
            eprintln!(
                "warning: could not seed golden (the fixture is created but expected/ may be empty): {err}"
            );
        }
    }

    println!("Created fixture: {}", fixture_dir.display());
    println!();
    println!("Next steps:");
    println!(
        "  1. Edit {}/input/src/lib.rs to set up the source code.",
        name
    );
    println!(
        "  2. Edit {}/diff.patch to describe the behavior change.",
        name
    );
    println!(
        "  3. Edit {}/SPEC.md to fill in the Given/When/Then/Must Not.",
        name
    );
    println!(
        "  4. Run: cargo xtask goldens bless {} --reason \"initial scaffold\"",
        name
    );
    println!("  5. Run: cargo xtask fixtures {} to verify.", name);

    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_json_handles_double_backslash() {
        let input = r#"{"path":"crates\\ripr\\src\\lib.rs"}"#;
        let out = normalize_fixture_json_output(input);
        assert!(
            !out.contains('\\'),
            "double backslash must be normalized: {out}"
        );
    }

    #[test]
    fn normalize_json_handles_single_backslash() {
        // #2337: single backslash in free-text fields (code excerpts, error
        // messages) must also be normalized.
        let input = r#"{"expression":"a\b\c"}"#;
        let out = normalize_fixture_json_output(input);
        assert!(
            !out.contains('\\'),
            "single backslash must be normalized: {out}"
        );
    }

    #[test]
    fn normalize_json_preserves_no_backslash_content() {
        let input = r#"{"path":"src/lib.rs","count":42}"#;
        let out = normalize_fixture_json_output(input);
        assert_eq!(out, input);
    }
}
