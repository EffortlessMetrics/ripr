//! Arg-parsing and dispatch for `ripr review-comments`.
//!
//! This is the CLI adapter layer only. Review-comment classification,
//! placement, rendering, identity, receipt DTOs, and analysis live in
//! `crate::output::review_comments*`, `crate::analysis`, and `crate::app`.
//! This module owns the current CLI transaction: argv/options, input
//! preparation, phase receipts, deadline checks, and output-path handling.

use crate::analysis;
use crate::app::CheckInput;
use crate::app::agent_brief::{
    AgentBriefChangedScope, AgentBriefPolicy, AgentBriefResolvedWorkingSet,
    select_agent_brief_seams,
};
use crate::cli::commands_agent_support::{
    agent_brief_lines_from_diff, agent_brief_owner_attribution_for_lines,
};
use crate::cli::commands_numeric::parse_positive_u64;
use crate::cli::commands_options::ReviewCommentsOptions;
use crate::cli::help;
use crate::cli::parse::expect_value;
use crate::cli::suggest::unknown_argument;
use crate::config::{CheckInputExplicit, apply_to_check_input, load_for_root};
use crate::output;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::write_text_file;

const DEFAULT_REVIEW_COMMENTS_TIMEOUT_MS: u64 = 120_000;

fn record_review_comments_error(
    receipt: &mut crate::output::review_comments_receipt::ReviewCommentsRunReceipt,
    receipt_path: &Path,
    phase: &str,
    error: String,
) -> String {
    receipt.failed(phase, &error);
    match receipt.write_atomic(receipt_path) {
        Ok(()) => error,
        Err(receipt_error) => {
            format!("{error}; failed to persist terminal receipt: {receipt_error}")
        }
    }
}

fn enforce_review_comments_deadline(
    receipt: &mut crate::output::review_comments_receipt::ReviewCommentsRunReceipt,
    receipt_path: &Path,
    started: Instant,
    now: Instant,
    timeout_ms: u64,
    phase: &str,
) -> Result<(), String> {
    if now.saturating_duration_since(started) < Duration::from_millis(timeout_ms) {
        return Ok(());
    }
    Err(record_review_comments_timeout(receipt, receipt_path, phase))
}

fn record_review_comments_timeout(
    receipt: &mut output::review_comments_receipt::ReviewCommentsRunReceipt,
    receipt_path: &Path,
    phase: &str,
) -> String {
    receipt.limited_timeout(phase);
    let error = format!("review-comments timed out during {phase}");
    match receipt.write_atomic(receipt_path) {
        Ok(()) => error,
        Err(receipt_error) => {
            format!("{error}; failed to persist terminal receipt: {receipt_error}")
        }
    }
}

fn load_review_comments_analysis_outcome(
    path: Option<&Path>,
    root: &Path,
    base: &str,
    diff_text: &str,
) -> Result<Option<crate::analysis_outcome::AnalysisOutcome>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    let text = crate::bounded_input::read_to_string(path).map_err(|error| {
        format!(
            "review-comments --check-output {} is invalid: read failed: {error}",
            path.display()
        )
    })?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
        format!(
            "review-comments --check-output {} is invalid: JSON parse failed: {error}",
            path.display()
        )
    })?;
    let _producer_schema_version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        .filter(|version| !version.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "review-comments --check-output {} is invalid: missing producer schema_version",
                path.display()
            )
        })?;
    if value.get("tool").and_then(serde_json::Value::as_str) != Some("ripr") {
        return Err(format!(
            "review-comments --check-output {} is invalid: producer tool must be ripr",
            path.display()
        ));
    }
    for field in ["mode", "root", "base"] {
        if value
            .get(field)
            .and_then(serde_json::Value::as_str)
            .is_none()
        {
            return Err(format!(
                "review-comments --check-output {} is invalid: producer envelope is missing string field {field}",
                path.display()
            ));
        }
    }
    if value.get("root").and_then(serde_json::Value::as_str)
        != Some(output::outcome::display_path(root).as_str())
    {
        return Err(format!(
            "review-comments --check-output {} is invalid: producer root does not match requested root",
            path.display()
        ));
    }
    if value.get("base").and_then(serde_json::Value::as_str) != Some(base) {
        return Err(format!(
            "review-comments --check-output {} is invalid: producer base does not match requested base",
            path.display()
        ));
    }
    if !value
        .get("summary")
        .is_some_and(serde_json::Value::is_object)
        || !value
            .get("findings")
            .is_some_and(serde_json::Value::is_array)
    {
        return Err(format!(
            "review-comments --check-output {} is invalid: producer envelope requires summary and findings",
            path.display()
        ));
    }
    let Some(envelope) = value.get("analysis_outcome") else {
        return Err(format!(
            "review-comments --check-output {} is invalid: missing analysis_outcome",
            path.display()
        ));
    };
    if envelope.is_null() {
        return Err(format!(
            "review-comments --check-output {} is invalid: analysis_outcome is null",
            path.display()
        ));
    }
    let declared_complete = envelope
        .get("analysis_complete")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| {
            format!(
                "review-comments --check-output {} is invalid: analysis_complete is missing or not boolean",
                path.display()
            )
        })?;
    let outcome = envelope.get("outcome").cloned().ok_or_else(|| {
        format!(
            "review-comments --check-output {} is invalid: analysis_outcome.outcome is missing",
            path.display()
        )
    })?;
    let outcome: crate::analysis_outcome::AnalysisOutcome =
        serde_json::from_value(outcome).map_err(|error| {
            format!(
                "review-comments --check-output {} is invalid: typed outcome failed validation: {error}",
                path.display()
            )
        })?;
    let expected_input_identity = format!(
        "sha256:{}",
        Sha256::digest(diff_text.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    if outcome.identity.input_identity.as_deref() != Some(expected_input_identity.as_str()) {
        return Err(format!(
            "review-comments --check-output {} is invalid: producer input identity does not match the requested diff",
            path.display()
        ));
    }
    if outcome
        .identity
        .base_revision
        .as_deref()
        .is_some_and(|revision| revision != base)
    {
        return Err(format!(
            "review-comments --check-output {} is invalid: typed outcome base revision does not match requested base",
            path.display()
        ));
    }
    if declared_complete != outcome.kind.is_complete() {
        return Err(format!(
            "review-comments --check-output {} is invalid: analysis_complete does not match typed outcome kind",
            path.display()
        ));
    }
    Ok(Some(outcome))
}

pub(in crate::cli) fn review_comments(args: &[String]) -> Result<(), String> {
    review_comments_with_diff_loader(args, load_review_comments_diff)
}

fn review_comments_with_diff_loader(
    args: &[String],
    load_diff: impl Fn(&Path, &str, &str) -> Result<String, String>,
) -> Result<(), String> {
    review_comments_with_diff_loader_at(args, load_diff, Instant::now)
}

fn review_comments_with_diff_loader_at(
    args: &[String],
    load_diff: impl Fn(&Path, &str, &str) -> Result<String, String>,
    now: impl Fn() -> Instant + Send + Sync + 'static,
) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_review_comments_help();
        return Ok(());
    }

    let options = parse_review_comments_options(args)?;
    if !options.root.is_dir() {
        return Err(format!(
            "review-comments root {} is not a directory",
            options.root.display()
        ));
    }

    let config = load_for_root(&options.root)?;
    let mut input = CheckInput {
        root: options.root.clone(),
        ..CheckInput::default()
    };
    apply_to_check_input(&mut input, &config, CheckInputExplicit::default());
    let receipt_path =
        output::review_comments_receipt::ReviewCommentsRunReceipt::path_for_output(&options.out);
    let markdown_path = review_comments_markdown_path(&options.out);
    let artifacts = vec![
        output::outcome::display_path(&options.out),
        output::outcome::display_path(&markdown_path),
    ];
    let now: analysis::cancellation::AnalysisClock = std::sync::Arc::new(now);
    let started = now();
    let cancellation = analysis::cancellation::AnalysisCancellationToken::with_budget(
        started,
        Duration::from_millis(options.timeout_ms),
        std::sync::Arc::clone(&now),
    );
    // #4363 review: the receipt's revision probes must respect this run's
    // `--timeout-ms` budget, not a fixed one-minute ceiling per probe — the
    // run's own deadline is only enforced after construction. The constructor
    // runs both probes itself and re-derives the remaining budget between
    // them on its own clock, so no pre-construction subtraction is needed
    // here: a fresh observation would break the run's single clock-observation
    // contract in the source-failure flow
    // (`review_comments_source_error_wins_over_later_clock_expiry`), and
    // `started` is observed immediately before admission, making the run's
    // full budget the remaining budget at this point.
    let revision_budget = Some(Duration::from_millis(options.timeout_ms));
    let mut receipt = output::review_comments_receipt::ReviewCommentsRunReceipt::new(
        &input.root,
        &options.base,
        &options.head,
        options.timeout_ms,
        &artifacts,
        revision_budget,
    );
    receipt.write_atomic(&receipt_path)?;
    receipt.phase("input_validation", "configuration");
    receipt.write_atomic(&receipt_path)?;

    if let Some(gap_ledger) = &options.gap_ledger {
        if options.check_output.is_some() {
            return Err(
                "review-comments accepts at most one of --gap-ledger or --check-output".to_string(),
            );
        }
        let gap_ledger_text = crate::bounded_input::read_to_string(gap_ledger).map_err(|err| {
            record_review_comments_error(
                &mut receipt,
                &receipt_path,
                "configuration",
                format!(
                    "review-comments --gap-ledger {} is invalid: read failed: {err}",
                    output::pr_inline_comment_publish_plan::display_path(gap_ledger)
                ),
            )
        })?;
        let records = output::gap_decision_ledger::parse_gap_records_json(&gap_ledger_text)
            .map_err(|err| {
                record_review_comments_error(
                    &mut receipt,
                    &receipt_path,
                    "configuration",
                    format!(
                        "review-comments --gap-ledger {} is invalid: {err}",
                        output::pr_inline_comment_publish_plan::display_path(gap_ledger)
                    ),
                )
            })?;
        enforce_review_comments_deadline(
            &mut receipt,
            &receipt_path,
            started,
            now(),
            options.timeout_ms,
            "configuration",
        )?;
        receipt.phase("configuration", "static_rendering");
        receipt.write_atomic(&receipt_path)?;
        let gap_ledger_path = output::pr_inline_comment_publish_plan::display_path(gap_ledger);
        let rendered_json = output::review_comments::render_gap_record_review_comments_json(
            &input.root,
            &options.base,
            &options.head,
            &input.mode,
            &gap_ledger_path,
            &records,
        )
        .map_err(|error| {
            record_review_comments_error(&mut receipt, &receipt_path, "static_rendering", error)
        })?;
        let rendered_md = output::review_comments::render_gap_record_review_comments_markdown(
            &input.root,
            &options.base,
            &options.head,
            &input.mode,
            &gap_ledger_path,
            &records,
        );
        enforce_review_comments_deadline(
            &mut receipt,
            &receipt_path,
            started,
            now(),
            options.timeout_ms,
            "static_rendering",
        )?;
        receipt.phase("static_rendering", "artifact_io");
        receipt.write_atomic(&receipt_path)?;
        let rendered_json =
            output::review_comments_receipt::attach_to_json(&rendered_json, &receipt)?;
        write_text_file(&options.out, &rendered_json).map_err(|error| {
            record_review_comments_error(&mut receipt, &receipt_path, "artifact_io", error)
        })?;
        write_text_file(&markdown_path, &rendered_md).map_err(|error| {
            record_review_comments_error(&mut receipt, &receipt_path, "artifact_io", error)
        })?;
        enforce_review_comments_deadline(
            &mut receipt,
            &receipt_path,
            started,
            now(),
            options.timeout_ms,
            "artifact_io",
        )?;
        receipt.complete(&artifacts);
        let rendered_json =
            output::review_comments_receipt::attach_to_json(&rendered_json, &receipt)?;
        write_text_file(&options.out, &rendered_json)?;
        receipt.write_atomic(&receipt_path)?;
        println!("Wrote {}", options.out.display());
        println!("Wrote {}", markdown_path.display());
        return Ok(());
    }

    receipt.phase("configuration", "diff_discovery");
    receipt.write_atomic(&receipt_path)?;
    let diff_text = analysis::cancellation::with_token(&cancellation, || {
        load_diff(&input.root, &options.base, &options.head)
    })
    .map_err(|error| {
        if crate::git::is_git_invocation_timeout(&error)
            || (analysis::cancellation::is_cancellation_error(&error)
                && cancellation.abort_kind()
                    == Some(analysis::cancellation::AnalysisAbortKind::DeadlineExceeded))
        {
            record_review_comments_timeout(&mut receipt, &receipt_path, "diff_discovery")
        } else {
            record_review_comments_error(&mut receipt, &receipt_path, "diff_discovery", error)
        }
    })?;
    if analysis::working_tree_has_tracked_changes(&input.root) {
        eprintln!(
            "ripr: warning: working tree has uncommitted tracked changes; \
             the committed diff at {}...{} does not include them.",
            options.base, options.head
        );
    }
    enforce_review_comments_deadline(
        &mut receipt,
        &receipt_path,
        started,
        now(),
        options.timeout_ms,
        "diff_discovery",
    )?;
    receipt.phase("diff_discovery", "language_facts");
    receipt.write_atomic(&receipt_path)?;
    let changed_lines = agent_brief_lines_from_diff(&input.root, &diff_text);
    let (changed_owners, enclosing_owners) =
        agent_brief_owner_attribution_for_lines(&input.root, &changed_lines);
    enforce_review_comments_deadline(
        &mut receipt,
        &receipt_path,
        started,
        now(),
        options.timeout_ms,
        "language_facts",
    )?;
    receipt.phase("language_facts", "canonical_analysis");
    receipt.write_atomic(&receipt_path)?;
    let working_set = AgentBriefResolvedWorkingSet::base(options.base.clone(), changed_lines)
        .with_changed_owners(changed_owners)
        .with_enclosing_owners(enclosing_owners);
    let changed_owner_names = working_set
        .changed_owners
        .iter()
        .map(|owner| owner.owner.clone())
        .collect::<Vec<_>>();
    // Review slots fill from changed lines and changed owners first, so
    // the rest of the scope is only evaluated when those fall short.
    let changed_scope = AgentBriefChangedScope::new(&working_set);
    let policy = AgentBriefPolicy::from_config(&config);
    let first_stage = |seam: &analysis::RepoSeam| changed_scope.contains(seam);
    let first_stage_sufficient = |classified: &[analysis::ClassifiedSeam]| {
        changed_scope.fills_selection(
            classified,
            output::review_comments::DEFAULT_REVIEW_MAX_SUMMARY_ITEMS,
            policy,
        )
    };
    let stages = analysis::DiffScopeEvidenceStages {
        first: &first_stage,
        sufficient: &first_stage_sufficient,
    };
    let scoped_inventory = analysis::cancellation::with_token(&cancellation, || {
        analysis::inventory_diff_scoped_classified_seams_staged_at_with_config(
            &input.root,
            &config,
            &working_set.files,
            &changed_owner_names,
            &stages,
        )
    })
    .map_err(|error| {
        if analysis::cancellation::is_cancellation_error(&error)
            && cancellation.abort_kind()
                == Some(analysis::cancellation::AnalysisAbortKind::DeadlineExceeded)
        {
            record_review_comments_timeout(&mut receipt, &receipt_path, "canonical_analysis")
        } else {
            record_review_comments_error(&mut receipt, &receipt_path, "canonical_analysis", error)
        }
    })?;
    enforce_review_comments_deadline(
        &mut receipt,
        &receipt_path,
        started,
        now(),
        options.timeout_ms,
        "canonical_analysis",
    )?;
    receipt.phase("canonical_analysis", "route_construction");
    receipt.write_atomic(&receipt_path)?;
    let mut selection = select_agent_brief_seams(
        &scoped_inventory.classified,
        &working_set,
        output::review_comments::DEFAULT_REVIEW_MAX_SUMMARY_ITEMS,
        policy,
    );
    if scoped_inventory.unevaluated_seams > 0 {
        // The cap count covers only evaluated seams, so it is a floor.
        for warning in &mut selection.warnings {
            if warning.ends_with("omitted by the brief cap") {
                *warning = format!("at least {warning}");
            }
        }
        let skipped = scoped_inventory.unevaluated_seams;
        let (noun, verb) = if skipped == 1 {
            ("seam", "was")
        } else {
            ("seams", "were")
        };
        selection.warnings.push(format!(
            "{skipped} scoped {noun} outside changed lines and changed owner functions {verb} \
             not evaluated: seams on changed lines and in changed owners already filled all {} \
             review slots",
            output::review_comments::DEFAULT_REVIEW_MAX_SUMMARY_ITEMS
        ));
    }
    enforce_review_comments_deadline(
        &mut receipt,
        &receipt_path,
        started,
        now(),
        options.timeout_ms,
        "route_construction",
    )?;
    receipt.phase("route_construction", "static_rendering");
    receipt.write_atomic(&receipt_path)?;
    let analysis_scope = output::review_comments::ReviewCommentsAnalysisScope::limited_diff_scope(
        &working_set,
        &scoped_inventory,
    );
    let analysis_outcome = load_review_comments_analysis_outcome(
        options.check_output.as_deref(),
        &input.root,
        &options.base,
        &diff_text,
    )
    .map_err(|error| {
        record_review_comments_error(&mut receipt, &receipt_path, "static_rendering", error)
    })?;
    let render_context = output::review_comments::ReviewCommentsRenderContext {
        root: &input.root,
        base: &options.base,
        head: &options.head,
        mode: &input.mode,
        config: &config,
    };
    let rendered_json = output::review_comments::render_review_comments_json_with_scope(
        &render_context,
        &working_set,
        &selection,
        &analysis_scope,
        analysis_outcome.as_ref(),
    )
    .map_err(|error| {
        record_review_comments_error(&mut receipt, &receipt_path, "static_rendering", error)
    })?;
    let rendered_md = output::review_comments::render_review_comments_markdown_with_scope(
        &render_context,
        &working_set,
        &selection,
        &analysis_scope,
        analysis_outcome.as_ref(),
    );
    enforce_review_comments_deadline(
        &mut receipt,
        &receipt_path,
        started,
        now(),
        options.timeout_ms,
        "static_rendering",
    )?;
    receipt.phase("static_rendering", "artifact_io");
    receipt.write_atomic(&receipt_path)?;
    let rendered_json = output::review_comments_receipt::attach_to_json(&rendered_json, &receipt)?;
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&markdown_path, &rendered_md)?;
    enforce_review_comments_deadline(
        &mut receipt,
        &receipt_path,
        started,
        now(),
        options.timeout_ms,
        "artifact_io",
    )?;
    receipt.complete(&artifacts);
    let rendered_json = output::review_comments_receipt::attach_to_json(&rendered_json, &receipt)?;
    write_text_file(&options.out, &rendered_json)?;
    receipt.write_atomic(&receipt_path)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", markdown_path.display());
    Ok(())
}

fn parse_review_comments_options(args: &[String]) -> Result<ReviewCommentsOptions, String> {
    let mut root = PathBuf::from(".");
    let mut base: Option<String> = None;
    let mut head: Option<String> = None;
    let mut gap_ledger = None;
    let mut check_output = None;
    let mut out = PathBuf::from("target/ripr/review/comments.json");
    let mut timeout_ms = DEFAULT_REVIEW_COMMENTS_TIMEOUT_MS;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--base" => {
                i += 1;
                let value = expect_value(args, i, "--base")?;
                if value.trim().is_empty() {
                    return Err("review-comments --base requires a non-empty revision".to_string());
                }
                base = Some(value.to_string());
            }
            "--head" => {
                i += 1;
                let value = expect_value(args, i, "--head")?;
                if value.trim().is_empty() {
                    return Err("review-comments --head requires a non-empty revision".to_string());
                }
                head = Some(value.to_string());
            }
            "--gap-ledger" => {
                i += 1;
                let value = expect_value(args, i, "--gap-ledger")?;
                if value.trim().is_empty() {
                    return Err(
                        "review-comments --gap-ledger requires a non-empty path".to_string()
                    );
                }
                gap_ledger = Some(PathBuf::from(value));
            }
            "--check-output" => {
                i += 1;
                let value = expect_value(args, i, "--check-output")?;
                if value.trim().is_empty() {
                    return Err(
                        "review-comments --check-output requires a non-empty path".to_string()
                    );
                }
                check_output = Some(PathBuf::from(value));
            }
            "--out" => {
                i += 1;
                let value = expect_value(args, i, "--out")?;
                if value.trim().is_empty() {
                    return Err("review-comments --out requires a non-empty path".to_string());
                }
                out = PathBuf::from(value);
            }
            "--timeout-ms" => {
                i += 1;
                timeout_ms =
                    parse_positive_u64(expect_value(args, i, "--timeout-ms")?, "--timeout-ms")?;
            }
            other => return Err(unknown_argument("review-comments", other)),
        }
        i += 1;
    }

    Ok(ReviewCommentsOptions {
        root,
        base: base.ok_or_else(|| "review-comments requires --base <sha>".to_string())?,
        head: head.ok_or_else(|| "review-comments requires --head <sha>".to_string())?,
        gap_ledger,
        check_output,
        out,
        timeout_ms,
    })
}

/// Review-comments diff through the shared range authority (#4538): the base
/// and head are verified like `ripr check` verifies its base, and the range
/// uses the pinned diff presentation, so ambient `color.diff` or
/// `diff.submodule` config cannot empty or widen the changed-line set.
fn load_review_comments_diff(root: &Path, base: &str, head: &str) -> Result<String, String> {
    let base = analysis::resolve_effective_base(
        root,
        Some(base),
        analysis::cancellation::remaining_budget(),
    )?;
    analysis::load_diff_range_with_deadline(
        root,
        &base,
        head,
        analysis::cancellation::remaining_budget(),
    )
}

fn review_comments_markdown_path(json_path: &Path) -> PathBuf {
    let mut path = json_path.to_path_buf();
    path.set_extension("md");
    path
}

#[cfg(test)]
mod tests {
    use super::super::tests::{args, unique_command_test_dir};
    use super::*;
    use sha2::{Digest, Sha256};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    struct OwnedDeadlineFixture(PathBuf);
    impl Drop for OwnedDeadlineFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn review_comments_parses_required_revisions_and_out() {
        assert_eq!(
            parse_review_comments_options(&args(&[
                "--root",
                "repo",
                "--base",
                "origin/main",
                "--head",
                "HEAD",
                "--out",
                "target/ripr/review/comments.json",
            ])),
            Ok(ReviewCommentsOptions {
                root: PathBuf::from("repo"),
                base: "origin/main".to_string(),
                head: "HEAD".to_string(),
                gap_ledger: None,
                check_output: None,
                out: PathBuf::from("target/ripr/review/comments.json"),
                timeout_ms: DEFAULT_REVIEW_COMMENTS_TIMEOUT_MS,
            })
        );
        assert_eq!(
            parse_review_comments_options(&args(&[
                "--base",
                "origin/main",
                "--head",
                "HEAD",
                "--gap-ledger",
                "target/ripr/reports/gap-decision-ledger.json",
            ])),
            Ok(ReviewCommentsOptions {
                root: PathBuf::from("."),
                base: "origin/main".to_string(),
                head: "HEAD".to_string(),
                gap_ledger: Some(PathBuf::from(
                    "target/ripr/reports/gap-decision-ledger.json"
                )),
                check_output: None,
                out: PathBuf::from("target/ripr/review/comments.json"),
                timeout_ms: DEFAULT_REVIEW_COMMENTS_TIMEOUT_MS,
            })
        );
    }

    #[test]
    fn review_comments_requires_base_and_head() {
        assert_eq!(
            parse_review_comments_options(&args(&["--head", "HEAD"])),
            Err("review-comments requires --base <sha>".to_string())
        );
        assert_eq!(
            parse_review_comments_options(&args(&["--base", "main"])),
            Err("review-comments requires --head <sha>".to_string())
        );
        assert_eq!(
            parse_review_comments_options(&args(&["--base"])),
            Err("missing value for --base".to_string())
        );
    }

    #[test]
    fn review_comments_rejects_empty_values_and_unknown_args() {
        assert_eq!(
            parse_review_comments_options(&args(&["--base", "", "--head", "HEAD"])),
            Err("review-comments --base requires a non-empty revision".to_string())
        );
        assert_eq!(
            parse_review_comments_options(&args(&["--base", "main", "--head", ""])),
            Err("review-comments --head requires a non-empty revision".to_string())
        );
        assert_eq!(
            parse_review_comments_options(&args(&[
                "--base", "main", "--head", "HEAD", "--out", "",
            ])),
            Err("review-comments --out requires a non-empty path".to_string())
        );
        assert_eq!(
            parse_review_comments_options(&args(&[
                "--base",
                "main",
                "--head",
                "HEAD",
                "--gap-ledger",
                "",
            ])),
            Err("review-comments --gap-ledger requires a non-empty path".to_string())
        );
        assert_eq!(
            parse_review_comments_options(&args(&[
                "--base",
                "main",
                "--head",
                "HEAD",
                "--check-output",
                "",
            ])),
            Err("review-comments --check-output requires a non-empty path".to_string())
        );
        assert_eq!(
            parse_review_comments_options(&args(&["--base", "main", "--head", "HEAD", "--bad"])),
            Err(
                "unknown review-comments argument \"--bad\". Run `ripr review-comments --help`."
                    .to_string()
            )
        );
    }

    #[test]
    fn review_comments_loads_typed_outcome_from_check_artifact() -> Result<(), String> {
        let root = unique_command_test_dir("review-comments-check-output");
        std::fs::create_dir_all(&root).map_err(|err| format!("create temp root: {err}"))?;
        let path = root.join("check.json");
        let diff_text = "fixture diff";
        let input_identity = format!(
            "sha256:{}",
            Sha256::digest(diff_text.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        let mut artifact = serde_json::json!({
            "schema_version": "0.2",
            "tool": "ripr",
            "mode": "draft",
            "root": ".",
            "base": "main",
            "summary": {},
            "findings": [],
            "analysis_outcome": {
                "analysis_complete": false,
                "outcome": {
                    "schema_version": "0.1",
                    "kind": "partial_with_limitations",
                    "identity": {
                        "repository_identity": null,
                        "root_identity": null,
                        "config_identity": null,
                        "base_revision": "main",
                        "input_identity": input_identity,
                        "snapshot_identity": null,
                        "git_candidate_subject": null
                    },
                    "counts": {
                        "changed_file_count": 0,
                        "changed_line_count": 0,
                        "candidate_line_count": 0,
                        "probe_count": 0,
                        "finding_count": 0
                    },
                    "limitations": [{
                        "kind": "producer_timeout",
                        "producer_stage": "analysis_pipeline",
                        "path": null,
                        "affected_items": null,
                        "bounded_detail": null,
                        "recovery": {
                            "kind": "retry",
                            "detail": "rerun the producer"
                        }
                    }],
                    "claim_boundary": crate::analysis_outcome::ANALYSIS_OUTCOME_CLAIM_BOUNDARY
                }
            }
        });
        std::fs::write(
            &path,
            serde_json::to_vec(&artifact).map_err(|err| err.to_string())?,
        )
        .map_err(|err| format!("write check artifact: {err}"))?;
        let outcome =
            load_review_comments_analysis_outcome(Some(&path), Path::new("."), "main", diff_text)?
                .ok_or_else(|| "expected typed outcome".to_string())?;
        assert_eq!(outcome.kind.as_str(), "partial_with_limitations");
        assert_eq!(outcome.limitations[0].recovery.kind.as_str(), "retry");
        artifact["analysis_outcome"]["analysis_complete"] = serde_json::json!(true);
        std::fs::write(
            &path,
            serde_json::to_vec(&artifact).map_err(|err| err.to_string())?,
        )
        .map_err(|err| format!("rewrite mismatched check artifact: {err}"))?;
        let mismatch = match load_review_comments_analysis_outcome(
            Some(&path),
            Path::new("."),
            "main",
            diff_text,
        ) {
            Ok(_) => return Err("mismatched completeness must fail closed".to_string()),
            Err(error) => error,
        };
        assert!(mismatch.contains("does not match typed outcome kind"));
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn review_comments_markdown_path_replaces_json_extension() {
        assert_eq!(
            review_comments_markdown_path(Path::new("target/ripr/review/comments.json")),
            PathBuf::from("target/ripr/review/comments.md")
        );
    }

    #[test]
    fn review_comments_rejects_missing_root_before_loading_diff() -> Result<(), String> {
        let root = unique_command_test_dir("review-comments-missing-root");
        let root_arg = root.display().to_string();
        let result = review_comments_with_diff_loader(
            &args(&["--root", &root_arg, "--base", "main", "--head", "HEAD"]),
            |_root, _base, _head| Ok(String::new()),
        );

        let err = match result {
            Ok(_) => return Err("missing root should be rejected".to_string()),
            Err(err) => err,
        };
        assert!(err.contains("is not a directory"));
        Ok(())
    }

    #[test]
    fn review_comments_diff_uses_the_pinned_range_authority() -> Result<(), String> {
        // #4538: the production review-comments loader must go through the
        // shared range authority. The raw control proves the fixture
        // discriminates: ambient `color.diff=always` colors a plain
        // `git diff`, and the old private loader parsed that as zero changed
        // lines. The same loader must name unresolvable revisions in ripr's
        // own voice instead of git's `ambiguous argument` advice.
        use crate::testing::fixture_git::{fixture_git_ok, remove_fixture_tree};
        let root = unique_command_test_dir("review-comments-pinned-diff");
        std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
        let lib = root.join("src/lib.rs");
        fixture_git_ok(&root, &["init", "-q", "--initial-branch=main"])?;
        for (key, value) in [
            ("user.name", "Review Comments"),
            ("user.email", "review-comments@example.com"),
            ("commit.gpgsign", "false"),
            ("color.diff", "always"),
        ] {
            fixture_git_ok(&root, &["config", "--local", key, value])?;
        }
        std::fs::write(&lib, "pub fn f(x: i32) -> bool { x > 1 }\n")
            .map_err(|err| format!("write base lib: {err}"))?;
        fixture_git_ok(&root, &["add", "."])?;
        fixture_git_ok(&root, &["commit", "-q", "-m", "base"])?;
        std::fs::write(&lib, "pub fn f(x: i32) -> bool { x >= 1 }\n")
            .map_err(|err| format!("write head lib: {err}"))?;
        fixture_git_ok(&root, &["commit", "-q", "-am", "head"])?;

        let raw = crate::git::run_git_output_with_deadline(
            &root,
            &["diff", "--unified=0", "--no-ext-diff", "HEAD~1...HEAD"],
            None,
        )?;
        let raw = String::from_utf8_lossy(&raw.stdout).into_owned();
        assert!(
            raw.contains('\u{1b}'),
            "color.diff=always control did not color the raw diff, so the fixture does not discriminate:\n{raw}"
        );
        assert!(analysis::parse_unified_diff(&raw).is_empty());

        let diff = load_review_comments_diff(&root, "HEAD~1", "HEAD")?;
        assert!(
            !diff.contains('\u{1b}'),
            "pinned diff kept ANSI color:\n{diff}"
        );
        let changed = analysis::parse_unified_diff(&diff);
        assert_eq!(changed.len(), 1, "expected one changed file: {diff}");

        let Err(err) = load_review_comments_diff(&root, "no-such-base", "HEAD") else {
            return Err("an unresolvable base must fail".to_string());
        };
        assert!(
            err.contains("the base `no-such-base` does not resolve to a commit")
                && !err.contains("ambiguous argument"),
            "base failure must be named by ripr, got: {err}"
        );
        let Err(err) = load_review_comments_diff(&root, "HEAD~1", "no-such-head") else {
            return Err("an unresolvable head must fail".to_string());
        };
        assert!(
            err.contains("the head `no-such-head` does not resolve to a commit")
                && err.contains("--head <ref>")
                && !err.contains("ambiguous argument"),
            "head failure must be named by ripr, got: {err}"
        );
        remove_fixture_tree(&root)
    }

    #[test]
    fn review_comments_returns_diff_loader_errors() -> Result<(), String> {
        let root = unique_command_test_dir("review-comments-diff-error");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let root_arg = root.display().to_string();
        let out = root.join("comments.json");
        let out_arg = out.display().to_string();
        let result = review_comments_with_diff_loader(
            &args(&[
                "--root", &root_arg, "--base", "main", "--head", "HEAD", "--out", &out_arg,
            ]),
            |_root, _base, _head| Err("synthetic diff failure".to_string()),
        );

        assert_eq!(result, Err("synthetic diff failure".to_string()));
        let receipt_path = out.with_file_name("run-receipt.json");
        let receipt_json = std::fs::read_to_string(&receipt_path)
            .map_err(|err| format!("read failed receipt: {err}"))?;
        let receipt: serde_json::Value = serde_json::from_str(&receipt_json)
            .map_err(|err| format!("parse failed receipt: {err}"))?;
        assert_eq!(receipt["status"], "failed");
        assert_eq!(receipt["active_phase"], "diff_discovery");
        assert_eq!(receipt["limitations"][0]["category"], "analysis_failed");
        assert_eq!(
            receipt["limitations"][0]["repair_route"],
            "synthetic diff failure"
        );
        assert_eq!(receipt["non_claims"][1], "no complete route inventory");
        assert_eq!(receipt["non_claims"][2], "no all-clear");
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn review_comments_writes_json_and_markdown_from_loaded_diff() -> Result<(), String> {
        let root = unique_command_test_dir("review-comments");
        std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"review_comments_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml: {err}"))?;
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn discounted_total(amount: i32) -> i32 {\n    if amount > 10 { amount - 1 } else { amount }\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn above_threshold_gets_discount() {\n        assert_eq!(discounted_total(11), 10);\n    }\n}\n",
        )
        .map_err(|err| format!("write src/lib.rs: {err}"))?;

        let out = root.join("target/ripr/review/comments.json");
        let root_arg = root.display().to_string();
        let out_arg = out.display().to_string();
        review_comments_with_diff_loader(
            &args(&[
                "--root", &root_arg, "--base", "HEAD~1", "--head", "HEAD", "--out", &out_arg,
            ]),
            |diff_root, base, head| {
                assert_eq!(diff_root, root.as_path());
                assert_eq!(base, "HEAD~1");
                assert_eq!(head, "HEAD");
                Ok("diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -2 +2 @@\n-    if amount >= 10 { amount - 1 } else { amount }\n+    if amount > 10 { amount - 1 } else { amount }\n".to_string())
            },
        )?;

        let rendered_json = std::fs::read_to_string(&out)
            .map_err(|err| format!("read review comments JSON: {err}"))?;
        let rendered_md = std::fs::read_to_string(out.with_extension("md"))
            .map_err(|err| format!("read review comments Markdown: {err}"))?;
        assert!(rendered_json.contains("\"schema_version\": \"0.1\""));
        assert!(rendered_json.contains("\"status\": \"advisory\""));
        assert!(rendered_json.contains("\"base\": \"HEAD~1\""));
        assert!(rendered_json.contains("\"head\": \"HEAD\""));
        let value: serde_json::Value = serde_json::from_str(&rendered_json)
            .map_err(|err| format!("parse review comments JSON: {err}"))?;
        assert_eq!(value["analysis_scope"]["run_status"], "limited_diff_scope");
        assert_eq!(
            value["analysis_scope"]["limitation"],
            "review_comments_diff_scope_only"
        );
        assert!(rendered_md.contains("# RIPR PR Guidance"));
        assert!(rendered_md.contains("run status: `limited_diff_scope`"));
        assert!(rendered_md.contains("Advisory static evidence only"));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn review_comments_scopes_diff_fast_path_to_changed_files_and_immediate_callers()
    -> Result<(), String> {
        let root = unique_command_test_dir("review-comments-diff-scope");
        std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"review_comments_scope_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml: {err}"))?;
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn discounted_total(amount: i32) -> i32 {\n    if amount > 10 { amount - 1 } else { amount }\n}\n",
        )
        .map_err(|err| format!("write src/lib.rs: {err}"))?;
        std::fs::write(
            root.join("src/wrapper.rs"),
            "pub fn quote(amount: i32) -> i32 {\n    if discounted_total(amount) > 0 { discounted_total(amount) } else { 0 }\n}\n",
        )
        .map_err(|err| format!("write src/wrapper.rs: {err}"))?;
        std::fs::write(
            root.join("src/unrelated.rs"),
            "pub fn unrelated(value: i32) -> i32 {\n    if value > 0 { value } else { 0 }\n}\n",
        )
        .map_err(|err| format!("write src/unrelated.rs: {err}"))?;

        let out = root.join("target/ripr/review/comments.json");
        review_comments_with_diff_loader(
            &args(&[
                "--root",
                &root.display().to_string(),
                "--base",
                "HEAD~1",
                "--head",
                "HEAD",
                "--out",
                &out.display().to_string(),
            ]),
            |_diff_root, _base, _head| {
                Ok("diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -2 +2 @@\n-    if amount >= 10 { amount - 1 } else { amount }\n+    if amount > 10 { amount - 1 } else { amount }\n".to_string())
            },
        )?;

        let rendered_json = std::fs::read_to_string(&out)
            .map_err(|err| format!("read review comments JSON: {err}"))?;
        let value: serde_json::Value = serde_json::from_str(&rendered_json)
            .map_err(|err| format!("parse review comments JSON: {err}"))?;
        let scope = &value["analysis_scope"];
        assert_eq!(scope["scope"], "diff_scoped_changed_files");
        assert_eq!(scope["run_status"], "limited_diff_scope");
        assert_eq!(
            scope["basis"],
            "changed_production_files_plus_immediate_callers"
        );
        assert_eq!(scope["total_production_files"], 3);
        assert_eq!(scope["production_files_considered"], 2);
        assert_eq!(
            scope["changed_production_files"],
            serde_json::json!(["src/lib.rs"])
        );
        assert_eq!(
            scope["immediate_caller_files"],
            serde_json::json!(["src/wrapper.rs"])
        );
        assert_eq!(
            scope["scoped_production_files"],
            serde_json::json!(["src/lib.rs", "src/wrapper.rs"])
        );
        assert!(
            !rendered_json.contains("src/unrelated.rs"),
            "unrelated production files must stay out of the scoped review report"
        );

        let rendered_md = std::fs::read_to_string(out.with_extension("md"))
            .map_err(|err| format!("read review comments Markdown: {err}"))?;
        assert!(rendered_md.contains("analysis scope: `diff_scoped_changed_files`"));
        assert!(rendered_md.contains("scoped production files: 2/3"));
        assert!(rendered_md.contains("review_comments_diff_scope_only"));
        assert!(
            !rendered_json.contains("not evaluated") && scope.get("unevaluated_seams").is_none(),
            "a scope the changed lines cannot fill is evaluated in full"
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn review_comments_skips_evidence_outside_changed_lines_once_they_fill_the_review_slots()
    -> Result<(), String> {
        let root = unique_command_test_dir("review-comments-staged-scope");
        std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"review_comments_staged_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml: {err}"))?;
        // Twelve changed one-line predicates fill the ten review slots;
        // the unchanged thirteenth function is never evaluated.
        let changed = (1..=12)
            .map(|n| format!("pub fn changed_{n}(value: i32) -> i32 {{ if value > {n} {{ 1 }} else {{ 0 }} }}\n"))
            .collect::<String>();
        std::fs::write(
            root.join("src/lib.rs"),
            format!("{changed}pub fn untouched(value: i32) -> i32 {{ if value > 99 {{ 1 }} else {{ 0 }} }}\n"),
        )
        .map_err(|err| format!("write src/lib.rs: {err}"))?;
        let removed = (1..=12)
            .map(|n| format!("-pub fn changed_{n}(value: i32) -> i32 {{ if value >= {n} {{ 1 }} else {{ 0 }} }}\n"))
            .collect::<String>();
        let added = changed
            .lines()
            .map(|line| format!("+{line}\n"))
            .collect::<String>();
        let diff = format!(
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,12 +1,12 @@\n{removed}{added}"
        );

        let out = root.join("target/ripr/review/comments.json");
        review_comments_with_diff_loader(
            &args(&[
                "--root",
                &root.display().to_string(),
                "--base",
                "HEAD~1",
                "--head",
                "HEAD",
                "--out",
                &out.display().to_string(),
            ]),
            move |_diff_root, _base, _head| Ok(diff.clone()),
        )?;

        let rendered_json = std::fs::read_to_string(&out)
            .map_err(|err| format!("read review comments JSON: {err}"))?;
        let value: serde_json::Value = serde_json::from_str(&rendered_json)
            .map_err(|err| format!("parse review comments JSON: {err}"))?;
        let returned = value["comments"].as_array().map_or(0, Vec::len)
            + value["summary_only"].as_array().map_or(0, Vec::len);
        assert_eq!(
            returned,
            output::review_comments::DEFAULT_REVIEW_MAX_SUMMARY_ITEMS
        );
        assert!(
            !rendered_json.contains("untouched"),
            "the unchanged function's seam must not be evaluated or rendered"
        );
        assert_eq!(value["analysis_scope"]["unevaluated_seams"], 1);
        let messages = value["warnings"]
            .as_array()
            .ok_or("warnings must be an array")?
            .iter()
            .filter_map(|warning| warning["message"].as_str())
            .collect::<Vec<_>>();
        assert!(
            messages.contains(
                &"1 scoped seam outside changed lines and changed owner functions was not \
                  evaluated: seams on changed lines and in changed owners already filled all 10 \
                  review slots"
            ),
            "missing staged-scope warning in {messages:?}"
        );
        assert!(
            messages
                .iter()
                .filter(|message| message.contains("omitted by the brief cap"))
                .all(|message| message.starts_with("at least ")),
            "a cap count over evaluated seams only is a floor: {messages:?}"
        );
        let rendered_md = std::fs::read_to_string(out.with_extension("md"))
            .map_err(|err| format!("read review comments Markdown: {err}"))?;
        assert!(rendered_md.contains("- scoped seams not evaluated: 1"));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn review_comments_gap_ledger_writes_repair_cards_without_loading_diff() -> Result<(), String> {
        let root = unique_command_test_dir("review-comments-gap-ledger");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let gap_ledger = root.join("gap-ledger.json");
        let out = root.join("target/ripr/review/comments.json");
        std::fs::write(
            &gap_ledger,
            r#"{"records":[{"gap_id":"gap:pr:pricing","source_currentness":"candidate_current","seam_id":"seam:pricing:threshold-boundary","kind":"MissingBoundaryAssertion","language":"rust","language_status":"stable","scope":"pr_local","evidence_class":"predicate_boundary","gap_state":"actionable","policy_state":"new","repairability":"repairable","anchor":{"file":"src/pricing.rs","line":42,"dedupe_fingerprint":"gap:pricing"},"repair_route":{"route_kind":"AddBoundaryAssertion","target_file":"tests/pricing.rs","assertion_shape":"assert_eq!(discount(100, 100), 90)","changed_behavior":"amount == threshold"},"verification_commands":["cargo xtask fixtures boundary_gap"],"projection_eligibility":{"pr_comment":{"eligible":true,"reason":"stable_anchor_and_repair_route"}}}]}"#,
        )
        .map_err(|err| format!("write gap ledger: {err}"))?;

        review_comments_with_diff_loader(
            &args(&[
                "--root",
                &root.display().to_string(),
                "--base",
                "main",
                "--head",
                "HEAD",
                "--gap-ledger",
                &gap_ledger.display().to_string(),
                "--out",
                &out.display().to_string(),
            ]),
            |_root, _base, _head| Err("gap-ledger path should not load git diff".to_string()),
        )?;

        let rendered_json = std::fs::read_to_string(&out)
            .map_err(|err| format!("read gap-ledger review comments JSON: {err}"))?;
        let rendered_md = std::fs::read_to_string(out.with_extension("md"))
            .map_err(|err| format!("read gap-ledger review comments Markdown: {err}"))?;
        assert!(rendered_json.contains(r#""source": "gap_decision_ledger""#));
        assert!(rendered_json.contains(r#""repair_card""#));
        let value: serde_json::Value = serde_json::from_str(&rendered_json)
            .map_err(|err| format!("parse gap-ledger review comments JSON: {err}"))?;
        assert_eq!(value["analysis_scope"]["scope"], "gap_ledger_artifact");
        assert_eq!(value["analysis_scope"]["run_status"], "artifact_scope");
        assert_eq!(
            value["analysis_scope"]["basis"],
            "supplied_gap_decision_ledger"
        );
        assert_eq!(
            value["analysis_scope"]["changed_files"],
            serde_json::json!(["src/pricing.rs"])
        );
        assert!(rendered_md.contains("ripr first-action"));
        assert!(rendered_md.contains("analysis scope: `gap_ledger_artifact`"));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn review_comments_gap_ledger_reports_read_and_parse_errors() -> Result<(), String> {
        let root = unique_command_test_dir("review-comments-gap-ledger-errors");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let missing_ledger = root.join("missing-gap-ledger.json");
        let out = root.join("target/ripr/review/comments.json");

        let read_err = match review_comments_with_diff_loader(
            &args(&[
                "--root",
                &root.display().to_string(),
                "--base",
                "main",
                "--head",
                "HEAD",
                "--gap-ledger",
                &missing_ledger.display().to_string(),
                "--out",
                &out.display().to_string(),
            ]),
            |_root, _base, _head| Err("gap-ledger path should not load git diff".to_string()),
        ) {
            Ok(()) => return Err("missing gap ledger should fail before diff loading".to_string()),
            Err(err) => err,
        };
        assert!(read_err.contains("review-comments --gap-ledger"));
        assert!(read_err.contains("read failed"));

        // Pin the read-failure receipt before the next run overwrites it.
        let receipt_path = out.with_file_name("run-receipt.json");
        let read_receipt: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&receipt_path)
                .map_err(|err| format!("read read-failure receipt: {err}"))?,
        )
        .map_err(|err| format!("parse read-failure receipt: {err}"))?;
        assert_eq!(read_receipt["status"], "failed");
        assert_eq!(read_receipt["active_phase"], "configuration");

        let malformed_ledger = root.join("malformed-gap-ledger.json");
        std::fs::write(&malformed_ledger, "{not json")
            .map_err(|err| format!("write malformed gap ledger: {err}"))?;
        let parse_err = match review_comments_with_diff_loader(
            &args(&[
                "--root",
                &root.display().to_string(),
                "--base",
                "main",
                "--head",
                "HEAD",
                "--gap-ledger",
                &malformed_ledger.display().to_string(),
                "--out",
                &out.display().to_string(),
            ]),
            |_root, _base, _head| Err("gap-ledger path should not load git diff".to_string()),
        ) {
            Ok(()) => {
                return Err("malformed gap ledger should fail before diff loading".to_string());
            }
            Err(err) => err,
        };
        assert!(parse_err.contains("review-comments --gap-ledger"));
        assert!(parse_err.contains("invalid"));

        // Both failure routes must publish a terminal failed receipt so a
        // reader of the run receipt never mistakes the run for in-progress.
        let receipt_path = out.with_file_name("run-receipt.json");
        let receipt_json = std::fs::read_to_string(&receipt_path)
            .map_err(|err| format!("read failed receipt: {err}"))?;
        let receipt: serde_json::Value = serde_json::from_str(&receipt_json)
            .map_err(|err| format!("parse failed receipt: {err}"))?;
        assert_eq!(receipt["status"], "failed");
        assert_eq!(receipt["active_phase"], "configuration");
        assert_eq!(receipt["limitations"][0]["category"], "analysis_failed");
        assert!(
            receipt["limitations"][0]["repair_route"]
                .as_str()
                .is_some_and(|route| route.contains("--gap-ledger")),
            "receipt must carry the gap-ledger failure route: {receipt_json}"
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn review_comments_canonical_deadline_cancellation_records_timeout() -> Result<(), String> {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        // The owned CLI clock expires after canonical admission, so the
        // installed token must interrupt real inventory work before its
        // ordinary posthoc phase clock is consulted.
        let fixture_path = unique_command_test_dir("review-canonical-cancellation");
        std::fs::create_dir(&fixture_path)
            .map_err(|error| format!("claim canonical fixture root: {error}"))?;
        let fixture = OwnedDeadlineFixture(fixture_path);
        let root = &fixture.0;
        std::fs::create_dir_all(root.join("src"))
            .map_err(|error| format!("create canonical fixture: {error}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"review_canonical_cancellation\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n",
        )
        .map_err(|error| format!("write canonical fixture manifest: {error}"))?;
        std::fs::write(root.join("src/lib.rs"), "pub fn value() -> i32 { 1 }\n")
            .map_err(|error| format!("write canonical fixture source: {error}"))?;

        let out = root.join("target/ripr/review/comments.json");
        let calls = Arc::new(AtomicUsize::new(0));
        let clock_calls = Arc::clone(&calls);
        let start = Instant::now();
        let result = review_comments_with_diff_loader_at(
            &args(&[
                "--root",
                &root.display().to_string(),
                "--base",
                "BASE",
                "--head",
                "HEAD",
                "--timeout-ms",
                "1000",
                "--out",
                &out.display().to_string(),
            ]),
            |_root, _base, _head| {
                Ok("diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-pub fn value() -> i32 { 0 }\n+pub fn value() -> i32 { 1 }\n".to_string())
            },
            move || {
                let call = clock_calls.fetch_add(1, Ordering::SeqCst);
                // start, diff, language facts, canonical admission remain
                // unexpired; the next interior observation expires.
                if call >= 4 {
                    start + Duration::from_secs(1)
                } else {
                    start
                }
            },
        );
        if calls.load(Ordering::SeqCst) != 5 {
            return Err(
                "canonical work did not stop at its first expired interior checkpoint".to_string(),
            );
        }

        let receipt_path = out.with_file_name("run-receipt.json");
        let receipt: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&receipt_path)
                .map_err(|error| format!("read canonical cancellation receipt: {error}"))?,
        )
        .map_err(|error| format!("parse canonical cancellation receipt: {error}"))?;
        if receipt
            .get("active_phase")
            .and_then(serde_json::Value::as_str)
            != Some("canonical_analysis")
        {
            return Err(format!(
                "cancellation did not reach canonical inventory: {receipt}"
            ));
        }
        if receipt.get("status").and_then(serde_json::Value::as_str) != Some("limited_timeout") {
            return Err(format!(
                "deadline cancellation must be a typed timeout: {receipt}"
            ));
        }
        if result != Err("review-comments timed out during canonical_analysis".to_string()) {
            return Err(format!("unexpected canonical timeout result: {result:?}"));
        }
        if out.exists() || out.with_extension("md").exists() {
            return Err("cancelled inventory published review artifacts".to_string());
        }
        Ok(())
    }

    #[test]
    fn review_comments_source_error_wins_over_later_clock_expiry() -> Result<(), String> {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let path = unique_command_test_dir("review-error-before-expiry");
        std::fs::create_dir(&path).map_err(|error| format!("claim error fixture: {error}"))?;
        let fixture = OwnedDeadlineFixture(path);
        let out = fixture.0.join("comments.json");
        let calls = Arc::new(AtomicUsize::new(0));
        let owned_calls = Arc::clone(&calls);
        let started = Instant::now();
        let result = review_comments_with_diff_loader_at(
            &args(&[
                "--root",
                &fixture.0.display().to_string(),
                "--base",
                "BASE",
                "--head",
                "HEAD",
                "--timeout-ms",
                "1000",
                "--out",
                &out.display().to_string(),
            ]),
            |_, _, _| Err("source failure before the next deadline observation".to_string()),
            move || {
                if owned_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    started
                } else {
                    started + Duration::from_secs(1)
                }
            },
        );
        if result != Err("source failure before the next deadline observation".to_string())
            || calls.load(Ordering::SeqCst) != 1
        {
            return Err(
                "ordinary source failure was replaced by a later deadline observation".to_string(),
            );
        }
        let receipt: serde_json::Value = serde_json::from_slice(
            &std::fs::read(out.with_file_name("run-receipt.json"))
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        if receipt.get("status").and_then(serde_json::Value::as_str) != Some("failed")
            || receipt
                .get("active_phase")
                .and_then(serde_json::Value::as_str)
                != Some("diff_discovery")
            || out.exists()
            || out.with_extension("md").exists()
        {
            return Err(format!(
                "ordinary failure changed its receipt/output contract: {receipt}"
            ));
        }
        Ok(())
    }

    #[test]
    fn review_comments_diff_route_records_timeout_at_injected_deadline() -> Result<(), String> {
        let root = unique_command_test_dir("review-comments-clock-diff");
        std::fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create fixture source: {err}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"review_comments_clock_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write fixture manifest: {err}"))?;
        std::fs::write(root.join("src/lib.rs"), "pub fn value() -> i32 { 1 }\n")
            .map_err(|err| format!("write fixture source: {err}"))?;

        let out = root.join("target/ripr/review/comments.json");
        let start = Instant::now();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let result = review_comments_with_diff_loader_at(
            &args(&[
                "--root",
                &root.display().to_string(),
                "--base",
                "BASE",
                "--head",
                "HEAD",
                "--timeout-ms",
                "1000",
                "--out",
                &out.display().to_string(),
            ]),
            |_root, _base, _head| Ok(String::new()),
            move || {
                let call = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if call == 0 {
                    start
                } else {
                    start + Duration::from_secs(1)
                }
            },
        );
        if result != Err("review-comments timed out during diff_discovery".to_string()) {
            return Err(format!(
                "diff route must expose the injected timeout: {result:?}"
            ));
        }

        let receipt_path = out.with_file_name("run-receipt.json");
        let receipt: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&receipt_path)
                .map_err(|err| format!("read timeout receipt: {err}"))?,
        )
        .map_err(|err| format!("parse timeout receipt: {err}"))?;
        if receipt["status"] != "limited_timeout" || receipt["active_phase"] != "diff_discovery" {
            return Err(format!("unexpected diff timeout receipt: {receipt}"));
        }
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove fixture: {err}"))?;
        Ok(())
    }

    #[test]
    fn review_comments_gap_ledger_records_timeout_at_injected_deadline() -> Result<(), String> {
        let root = unique_command_test_dir("review-comments-clock-gap-ledger");
        std::fs::create_dir_all(&root).map_err(|err| format!("create fixture: {err}"))?;
        let gap_ledger = root.join("gap-ledger.json");
        let out = root.join("target/ripr/review/comments.json");
        std::fs::write(&gap_ledger, r#"{"records":[]}"#)
            .map_err(|err| format!("write gap ledger: {err}"))?;

        let start = Instant::now();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let result = review_comments_with_diff_loader_at(
            &args(&[
                "--root",
                &root.display().to_string(),
                "--base",
                "BASE",
                "--head",
                "HEAD",
                "--gap-ledger",
                &gap_ledger.display().to_string(),
                "--timeout-ms",
                "1000",
                "--out",
                &out.display().to_string(),
            ]),
            |_root, _base, _head| Err("diff loader must not run".to_string()),
            move || {
                let call = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if call == 0 {
                    start
                } else {
                    start + Duration::from_secs(1)
                }
            },
        );
        if result != Err("review-comments timed out during configuration".to_string()) {
            return Err(format!(
                "gap-ledger route must expose the injected timeout: {result:?}"
            ));
        }

        let receipt_path = out.with_file_name("run-receipt.json");
        let receipt: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&receipt_path)
                .map_err(|err| format!("read timeout receipt: {err}"))?,
        )
        .map_err(|err| format!("parse timeout receipt: {err}"))?;
        if receipt["status"] != "limited_timeout" || receipt["active_phase"] != "configuration" {
            return Err(format!("unexpected gap-ledger timeout receipt: {receipt}"));
        }
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove fixture: {err}"))?;
        Ok(())
    }
}
