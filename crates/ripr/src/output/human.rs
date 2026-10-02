use crate::app::{CheckOutput, FindingDrillIn, FindingNavigation};
use crate::config::RiprConfig;
use crate::domain::Finding;
use std::collections::BTreeSet;

/// RIPR-SPEC-0112 disclosure. Committed-history diffs (an explicit `--base`
/// or the resolved default base) read every source and test file as committed
/// at `HEAD`, so uncommitted edits and new files count neither in the diff nor
/// as test evidence. `--worktree` (RIPR-SPEC-0116) is the remedy that includes
/// them. Committing works too, but staging alone does not change a `--base`
/// diff.
const UNANALYZED_WORKING_TREE_NOTE: &str = "\nNote: uncommitted source and test changes were not analyzed; \
`ripr check` reads each file as committed at HEAD; add `--worktree` to include staged and \
unstaged edits (for example `ripr check --worktree`).\n";

/// Render the bounded triage report in the default human-readable CLI format.
pub fn render(output: &CheckOutput) -> String {
    render_bounded_with_config(output, &RiprConfig::default())
}

#[cfg(test)]
pub(crate) fn render_with_config(output: &CheckOutput, config: &RiprConfig) -> String {
    render_bounded_with_config(output, config)
}

pub(crate) fn render_bounded_with_config(output: &CheckOutput, config: &RiprConfig) -> String {
    let drill_in = FindingDrillIn::Commands(FindingNavigation::legacy());
    render_bounded_with_config_and_navigation(output, config, Some(&drill_in))
}

/// #4012: the no-scope note must describe what was actually analyzed. When
/// the disclosure fires on an established base (empty `<base>...HEAD`
/// range), name the compared base instead of claiming no scope was
/// provided; only a run with no established base keeps the legacy note.
fn render_no_scope_note(output: &CheckOutput) -> String {
    if let Some(base) = output.base.as_deref() {
        format!(
            "\nNote: `{base}...HEAD` contains no changed files, so there was nothing to analyze. \
             The compared base was `{base}`; an empty result here means no behavior changed against it.\n",
        )
    } else {
        "\nNote: no analysis scope was provided — `ripr check` is diff-first. \
         Run `ripr check --base BASE` with BASE set to an existing ref to analyze your changes, or \
         `ripr check --root . --format repo-exposure-md` for a full-repo scan. \
         An empty result here does NOT mean your changed behavior is covered.\n"
            .to_string()
    }
}

pub(crate) fn render_bounded_with_config_and_navigation(
    output: &CheckOutput,
    config: &RiprConfig,
    drill_in: Option<&FindingDrillIn>,
) -> String {
    let mut out = render_header_summary(output);
    render_analysis_outcome_disclosure(&mut out, output);
    render_suppression_policy_block(&mut out, output);
    render_partial_scope_disclosure(&mut out, output);

    if output.findings.is_empty() {
        out.push_str("No diff-derived static exposure probes found.\n");
        if output.no_scope_provided {
            let triage = triage::select_human_triage(output, config);
            triage::render_human_triage(&mut out, &triage, output, config, drill_in);
        }
        if output.no_scope_provided && !output.unanalyzed_working_tree {
            out.push_str(&render_no_scope_note(output));
        }
        if output.unanalyzed_working_tree {
            out.push_str(UNANALYZED_WORKING_TREE_NOTE);
        }
        render_preview_language_advisories(&mut out, output);
        render_language_runs(&mut out, output);
        return out;
    }

    let triage = triage::select_human_triage(output, config);
    triage::render_human_triage(&mut out, &triage, output, config, drill_in);
    render_all_no_path_disclosure(&mut out, output);
    if output.unanalyzed_working_tree {
        out.push_str(UNANALYZED_WORKING_TREE_NOTE);
    }
    render_preview_language_advisories(&mut out, output);
    render_language_runs(&mut out, output);
    out
}

/// Full human form without the per-finding `Drill in:` block: the drill-in
/// commands need the CLI's root and scope, which only
/// [`render_full_with_config_and_navigation`] receives. Library callers of
/// `ripr::render_check` keep this legacy all-findings form byte-for-byte
/// (`human_full_preserves_legacy_all_findings_output`).
pub(crate) fn render_full_with_config(output: &CheckOutput, config: &RiprConfig) -> String {
    render_full_with_config_and_navigation(output, config, None)
}

/// Full human form. With `drill_in`, every rendered finding carries its own
/// `ripr explain` / `ripr context` drill-in pair (or, for a `--worktree` run
/// without an artifact, the one-line replay route), so rerunning with
/// `--format human-full` as the digest suggests never loses the commands the
/// digest printed (#4379, #4321).
pub(crate) fn render_full_with_config_and_navigation(
    output: &CheckOutput,
    config: &RiprConfig,
    drill_in: Option<&FindingDrillIn>,
) -> String {
    let mut out = render_header_summary(output);

    render_analysis_outcome_disclosure(&mut out, output);
    render_suppression_policy_block(&mut out, output);
    render_partial_scope_disclosure(&mut out, output);

    if output.findings.is_empty() {
        out.push_str("No diff-derived static exposure probes found.\n");
        // RIPR-SPEC-0083: disclose when no analysis scope was provided
        // (#4012: or when the established range is empty — the note then
        // names the compared base instead of claiming no scope).
        // Suppressed while uncommitted working-tree edits are unanalyzed:
        // the working-tree note owns the guidance there (f752562fb).
        if output.no_scope_provided && !output.unanalyzed_working_tree {
            out.push_str(&render_no_scope_note(output));
        }
        // RIPR-SPEC-0112: disclose when a committed-history diff left uncommitted working-tree
        // changes were NOT analyzed. An empty result here does NOT mean those changes
        // are covered — they were excluded from the committed-history diff.
        if output.unanalyzed_working_tree {
            out.push_str(UNANALYZED_WORKING_TREE_NOTE);
        }
        render_preview_language_advisories(&mut out, output);
        render_language_runs(&mut out, output);
        return out;
    }

    let suppressed_ids: BTreeSet<&str> = output
        .suppression
        .iter()
        .flat_map(|outcome| {
            outcome
                .suppressed
                .iter()
                .map(|entry| entry.finding_id.as_str())
        })
        .collect();
    let mut findings_rendered = 0usize;
    for finding in &output.findings {
        if suppressed_ids.contains(finding.id.as_str()) {
            continue;
        }
        findings_rendered += 1;
        out.push_str(&render_finding_with_config(finding, config));
        if let Some(FindingDrillIn::Commands(navigation)) = drill_in {
            out.push_str("Drill in:\n");
            out.push_str(&format!("  {}\n", navigation.explain_command(&finding.id)));
            out.push_str(&format!("  {}\n", navigation.context_command(&finding.id)));
        }
        out.push('\n');
    }
    // #4321: a `--worktree` run without `--write-artifact` has no artifact for
    // drill-in commands to replay; say so and name the route instead of
    // dropping the block silently. The note only points at ids it actually
    // printed — an all-suppressed run states the route without the pointer.
    if let Some(FindingDrillIn::WorktreeReplayNeedsArtifact) = drill_in {
        out.push_str(&FindingDrillIn::worktree_replay_note_full(
            findings_rendered > 0,
        ));
        out.push('\n');
    }
    render_all_no_path_disclosure(&mut out, output);
    // RIPR-SPEC-0112: disclose when a committed-history diff left uncommitted working-tree
    // changes were NOT analyzed. Fires whether or not the committed diff had findings —
    // those uncommitted edits are still unanalyzed regardless.
    if output.unanalyzed_working_tree {
        out.push_str(UNANALYZED_WORKING_TREE_NOTE);
    }
    render_preview_language_advisories(&mut out, output);
    render_language_runs(&mut out, output);
    out
}

fn render_header_summary(output: &CheckOutput) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "ripr static RIPR exposure analysis\nmode: {}\nroot: {}\n\n",
        output.mode.as_str(),
        output.root.display()
    ));
    // #4322: the only per-run denominator line a human sees must match the
    // finding vocabulary. All seven classes render with their canonical
    // `ExposureClass::as_str()` tokens (no `weak`/`unrevealed` abbreviations,
    // no summed unknown bucket — the unknown split is exactly what tells the
    // reader which discriminator machinery is missing), and the shown/total
    // denominator discloses suppression: per-class buckets count unsuppressed
    // findings only, while `summary.findings` stays the total.
    let suppressed = output
        .suppression
        .as_ref()
        .map_or(0, |suppression| suppression.suppressed.len());
    let shown = output.summary.findings.saturating_sub(suppressed);
    out.push_str(&format!(
        "Summary: {} probe(s), {} exposed, {} weakly_exposed, {} reachable_unrevealed, \
         {} no_static_path, {} infection_unknown, {} propagation_unknown, {} static_unknown; \
         {} of {} finding(s) shown\n\n",
        output.summary.probes,
        output.summary.exposed,
        output.summary.weakly_exposed,
        output.summary.reachable_unrevealed,
        output.summary.no_static_path,
        output.summary.infection_unknown,
        output.summary.propagation_unknown,
        output.summary.static_unknown,
        shown,
        output.summary.findings,
    ));
    render_language_file_breakdown(&mut out, output);
    out
}

/// Render the producer-owned completeness contract before any empty-finding
/// message. A limitation is therefore visible even when the parser produced
/// no source findings; zero findings never upgrades an incomplete run.
fn render_analysis_outcome_disclosure(out: &mut String, output: &CheckOutput) {
    let Some(outcome) = &output.analysis_outcome else {
        return;
    };
    out.push_str(&format!(
        "Analysis outcome: {} ({}; {}).\n",
        outcome.kind.plain_label(),
        if outcome.kind.is_complete() {
            "analysis complete"
        } else {
            "analysis incomplete"
        },
        outcome.kind.as_str()
    ));
    if outcome.limitations.is_empty() {
        out.push_str(&format!(
            "  Counts: {} changed file(s), {} changed line(s), {} candidate line(s), {} probe(s), {} finding(s).\n\n",
            outcome.counts.changed_file_count,
            outcome.counts.changed_line_count,
            outcome.counts.candidate_line_count,
            outcome.counts.probe_count,
            outcome.counts.finding_count,
        ));
        return;
    }
    // The "zero findings" hedge only makes sense when there are zero findings;
    // a partial run with findings gets the scope caveat instead. #4952: the
    // EOL-only churn disclosure does not scope the analysis down, so the
    // incomplete-scope hedges stay off when it is the only limitation.
    let scoped_down = outcome.limitations.iter().any(|limitation| {
        limitation.kind != crate::analysis_outcome::AnalysisLimitationKind::EolOnlyChurn
    });
    if scoped_down {
        if output.findings.is_empty() {
            out.push_str(
                "  Zero findings is not a clean result because the analyzed scope is incomplete.\n",
            );
        } else {
            out.push_str(&format!(
                "  The {} finding(s) below cover only the analyzed scope; behavior outside it has no finding.\n",
                output.findings.len()
            ));
        }
    }
    for limitation in &outcome.limitations {
        // Plain words lead; the schema tokens follow in parentheses so the
        // line still greps against JSON and docs (#4323).
        out.push_str(&format!(
            "  Limitation: {} during {} ({} at {})",
            limitation.kind.plain_label(),
            limitation.producer_stage.plain_label(),
            limitation.kind.as_str(),
            limitation.producer_stage.as_str()
        ));
        if let Some(path) = &limitation.path {
            out.push_str(&format!("; file: {path}"));
        }
        if let Some(count) = limitation.affected_items {
            out.push_str(&format!("; affected items: {count}"));
        }
        let recovery = limitation.recovery.kind;
        let recovery_label = if recovery.plain_label() == recovery.as_str() {
            recovery.as_str().to_string()
        } else {
            format!("{} ({})", recovery.plain_label(), recovery.as_str())
        };
        out.push_str(&format!(
            "; recovery: {recovery_label} — {}.\n",
            // The recovery detail is often a full sentence; the line supplies
            // its own terminal period.
            limitation.recovery.detail.trim_end_matches('.')
        ));
    }
    out.push('\n');
}

/// Emit a per-language changed-file breakdown only when a non-Rust language
/// adapter counted at least one file (#2103). Pure-Rust runs emit nothing, so
/// Rust-only output stays byte-identical. `changed_rust_files` itself now
/// carries the Rust adapter's count only; this line shows the full split.
fn render_language_file_breakdown(out: &mut String, output: &CheckOutput) {
    let counts = &output.summary.changed_files_by_language;
    let has_non_rust = counts
        .iter()
        .any(|count| count.language != "rust" && count.files > 0);
    if !has_non_rust {
        return;
    }
    // Zero-count languages add nothing and read as noise ("rust: 0" on a
    // pure-Python change).
    let parts = counts
        .iter()
        .filter(|count| count.files > 0)
        .map(|count| format!("{}: {}", count.language, count.files))
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&format!("Changed file(s) by language: {parts}.\n\n"));
}

/// Emit the `--suppression-policy` application block (#1441): which policy
/// ran, which findings it suppressed (compact one-liners — suppression stays
/// visible, not hidden), and any expired/unmatched policy warnings. Emits
/// nothing when no policy was supplied, so default output is unchanged.
fn render_suppression_policy_block(out: &mut String, output: &CheckOutput) {
    let Some(suppression) = &output.suppression else {
        return;
    };
    out.push_str(&format!(
        "Suppressed by policy ({}): {} finding(s)\n",
        suppression.policy_path,
        suppression.suppressed.len()
    ));
    for entry in &suppression.suppressed {
        if let Some(finding) = output
            .findings
            .iter()
            .find(|finding| finding.id == entry.finding_id)
        {
            out.push_str(&format!(
                "  - {}:{} {} (selector: {})\n",
                finding.probe.location.file.display(),
                finding.probe.location.line,
                finding.class.as_str(),
                entry.selector
            ));
        }
    }
    for warning in &suppression.warnings {
        out.push_str(&format!("  policy warning: {warning}\n"));
    }
    out.push('\n');
}

/// Emit the `limited_partial_scope` run-state disclosure (RIPR-PROP-0019,
/// #1999). The partial result must never be presented as complete: the block
/// names the budget that stopped the run with its effective size, the
/// findings produced before the stop, the exact selected partition, the
/// lower-bound uninspected scope (never a bare "at least 0"), the stop
/// reason, gate ineligibility, and the only continuation route (raising the
/// named budget override).
fn render_partial_scope_disclosure(out: &mut String, output: &CheckOutput) {
    let Some(scope) = &output.partial_scope else {
        return;
    };
    let budget_env = scope.stop_reason.budget_env();
    let budget = scope.stopping_budget();
    let stopped_at = match scope.stop_reason {
        crate::analysis::PartialDiffStopReason::FileBudget => {
            format!("the file budget of {budget} changed file(s) ({budget_env}={budget})")
        }
        crate::analysis::PartialDiffStopReason::LineBudget => {
            format!("the line budget of {budget} changed line(s) ({budget_env}={budget})")
        }
        crate::analysis::PartialDiffStopReason::LineBudgetExceededOnFirstFile => format!(
            "the line budget of {budget} changed line(s) ({budget_env}={budget}); \
             the first selected file alone exceeded it and was analyzed whole"
        ),
    };
    out.push_str(&format!(
        "Partial scope: run state {} — analysis stopped at {stopped_at}; \
         analyzed {} changed file(s) ({} changed line(s)) of the diff; stop reason: {}.\n",
        scope.run_status,
        scope.selected_files.len(),
        scope.selected_changed_lines,
        scope.stop_reason.as_str(),
    ));
    let found = output.findings.len();
    if scope.has_known_uninspected_scope() {
        out.push_str(&format!(
            "  Found {found} finding(s) before stopping. NOT inspected: at least {} changed file(s) \
             and at least {} changed line(s); more findings may exist beyond the budget.\n",
            scope.uninspected_files_lower_bound, scope.uninspected_changed_lines_lower_bound,
        ));
    } else {
        out.push_str(&format!(
            "  Found {found} finding(s) before stopping. Every changed file ripr's language \
             adapters read was selected, but the budget was exceeded, so this result stays \
             partial and is not a complete-scope claim.\n",
        ));
    }
    for file in &scope.selected_files {
        // Paths come from the diff text: a crafted filename with control
        // bytes could forge report lines or emit terminal escape sequences.
        // The raw path stays on the scope record for identities/JSON; only
        // the terminal-facing display is escaped (#2142 review).
        out.push_str(&format!("  selected: {}\n", escape_terminal_display(file)));
    }
    for disclosure in &scope.budget_disclosures {
        out.push_str(&format!("  budget: {disclosure}\n"));
    }
    out.push_str(&format!(
        "  This partial result is not eligible as a gate, baseline, badge, or RIPR Zero input \
         (gate_eligibility: {}).\n",
        crate::analysis::PartialDiffScope::GATE_ELIGIBILITY,
    ));
    // RIPR-PROP-0019 decision 6: raising the explicit overrides is the only
    // continuation route; the budget has no off switch (zero is rejected).
    // The widen wording is shared with JSON, LSP and the analysis outcome.
    out.push_str(&format!(
        "  To widen the analyzed partition, {}. Overrides above the analysis-cost limit are \
         clamped, the budget cannot be switched off, and named partition continuation is not \
         available.\n  partition_identity: {}\n\n",
        scope.widen_instruction(),
        scope.partition_identity,
    ));
}

/// Escape a diff-supplied string for terminal display: control bytes
/// (including ESC, which opens terminal escape sequences) render as
/// `\u{XX}` so a crafted diff path cannot forge report lines or inject
/// terminal control. The raw value is unchanged for identities and JSON
/// (#2142 review).
fn escape_terminal_display(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_control() {
            out.push_str(&format!("\\u{{{:02x}}}", ch as u32));
        } else {
            out.push(ch);
        }
    }
    out
}

/// Emit an advisory note when every finding is no-path or unknown (zero
/// exposed/weakly_exposed/reachable_unrevealed). See RIPR-SPEC-0090.
///
/// Called unconditionally after the findings loop; emits nothing when:
/// - there are zero findings (a different case handled elsewhere), or
/// - at least one finding is exposed/weakly_exposed/reachable_unrevealed
///   (the per-finding output already carries the signal).
///
/// This is a pure ABSENCE-OF-PATH statement, not a coverage or adequacy claim.
fn render_all_no_path_disclosure(out: &mut String, output: &CheckOutput) {
    let s = &output.summary;
    let all_no_path_count =
        s.no_static_path + s.infection_unknown + s.propagation_unknown + s.static_unknown;
    if s.findings == 0 {
        return;
    }
    if s.exposed > 0 || s.weakly_exposed > 0 || s.reachable_unrevealed > 0 {
        return;
    }
    if all_no_path_count != s.findings {
        return;
    }
    // Honesty guard (dogfood: anyhow `Chain::len`): the unknown classes
    // (`static_unknown` / `infection_unknown` / `propagation_unknown`) can carry
    // `reach: yes` — a test DOES reach the change, ripr just could not classify
    // or propagate it. Claiming "no static test path" for the diff then
    // contradicts the finding's own reach evidence. A reaching test IS a static
    // test path, so suppress the all-no-path note whenever any finding reaches.
    if output
        .findings
        .iter()
        .any(|finding| finding.ripr.reach.state == crate::domain::StageState::Yes)
    {
        return;
    }
    let related_tests_total = output
        .findings
        .iter()
        .flat_map(|finding| finding.related_tests.iter())
        .map(|test| {
            (
                test.file.to_string_lossy().into_owned(),
                test.name.clone(),
                test.line,
            )
        })
        .collect::<BTreeSet<_>>()
        .len();
    let scope_summary = if s.changed_rust_files > 0 {
        format!(
            "Scope analyzed: {} changed Rust file(s), {} changed expression(s), and {} statically linked related test(s).",
            s.changed_rust_files, all_no_path_count, related_tests_total
        )
    } else {
        format!(
            "Scope analyzed: {} changed expression(s) and {} statically linked related test(s).",
            all_no_path_count, related_tests_total
        )
    };
    // Language bindings are tested from the other language, so when every
    // finding names the cross-language limitation the same-language repair
    // advice would contradict each finding's own next step. ripr has no
    // evidence that such tests exist yet, so the advice covers adding them.
    let cross_language_count = output
        .findings
        .iter()
        .filter(|finding| {
            finding.static_limit_kind
                == Some(crate::domain::StaticLimitKind::CrossLanguageOracleVisibilityUnresolved)
        })
        .count();
    let repair = if cross_language_count == output.findings.len() {
        "add or check tests in the bindings' other language that observe the changed behavior"
    } else if cross_language_count > 0 {
        "add co-located tests that observe the changed behavior, or, for a language binding, tests in the binding's other language"
    } else {
        "add co-located tests that observe the changed behavior"
    };
    let note = format!(
        "Note: ripr found no static test path for any of the {} changed expression(s) in this diff. {} This is not a coverage assessment. A test may already exercise these changes through macros, helper-call chains, or integration tests that ripr's static model does not yet trace; if none does, {}.",
        all_no_path_count, scope_summary, repair
    );
    out.push('\n');
    out.push_str(&wrap_human_prose(&note, "", "  "));
    out.push('\n');
}

const HUMAN_PROSE_WRAP_COLUMN: usize = 100;

pub(super) fn is_wrappable_advisory_prose(value: &str) -> bool {
    value.starts_with("ripr saw a test reaching public API that may call toward this change")
        || value.starts_with(
            "ripr saw a test reaching a Rust entry point whose path toward this change",
        )
}

pub(super) fn wrap_human_prose(
    value: &str,
    first_prefix: &str,
    continuation_prefix: &str,
) -> String {
    let mut rendered = String::new();
    let mut line_len = first_prefix.chars().count();
    let mut prefix = first_prefix;
    for word in value.split_whitespace() {
        let word_len = word.chars().count();
        let separator_len = usize::from(line_len > prefix.chars().count());
        if line_len + separator_len + word_len > HUMAN_PROSE_WRAP_COLUMN
            && line_len > prefix.chars().count()
        {
            rendered.push('\n');
            rendered.push_str(continuation_prefix);
            prefix = continuation_prefix;
            line_len = prefix.chars().count();
        }
        if line_len > prefix.chars().count() {
            rendered.push(' ');
            line_len += 1;
        }
        rendered.push_str(word);
        line_len += word_len;
    }
    format!("{first_prefix}{rendered}")
}

/// Emit preview-language advisory notes when preview-language files were
/// in the analyzed scope.
///
/// Called unconditionally; emits nothing when `preview_language_advisories`
/// is empty (pure-Rust scope). See RIPR-SPEC-0082.
///
/// Three wordings per the producer-owned completion state:
///
/// - enabled and completed — the preview adapter ran; the empty/partial result
///   is advisory and may be incomplete, not Rust-grade clean.
/// - enabled but failed — the files were routed but not analyzed to adapter
///   completion; the typed `language_runs` status is disclosed.
/// - not enabled — the files were detected but not analyzed because the
///   preview adapter is not enabled in `ripr.toml`; the empty result must not
///   be read as clean. A copy-paste-ready TOML block is appended so enabling
///   the adapter is a single edit.
fn render_preview_language_advisories(out: &mut String, output: &CheckOutput) {
    for advisory in &output.preview_language_advisories {
        let (language, file_language) = advisory_language_names(advisory);
        let file_label = if advisory.file_count == 1 {
            format!("{file_language} file")
        } else {
            format!("{file_language} files")
        };
        if advisory.analyzed(&output.language_runs) {
            // The empty-result caveat only applies when there is no finding.
            let caveat = if output.findings.is_empty() {
                "An empty result here is NOT a clean Rust-grade result."
            } else {
                "Treat its findings as advisory, not Rust-grade."
            };
            out.push_str(&format!(
                "\nNote: {} {} analyzed under preview support — preview evidence is advisory and may be incomplete. {caveat}\n",
                advisory.file_count, file_label,
            ));
        } else if let Some(recovery) = advisory.unavailable_adapter_recovery() {
            // The adapter is not compiled into this binary: a `ripr.toml`
            // edit cannot enable it (config load rejects it), so name the
            // real prerequisites instead of the TOML block.
            out.push_str(&format!(
                "\nNote: this diff contains {} {}. The {} adapter is not compiled into this ripr binary, so these files were not analyzed — this is NOT a clean Rust-grade result. {recovery}.\n",
                advisory.file_count, file_label, language,
            ));
        } else if !advisory.enabled {
            let language_lowercase = advisory.language.to_lowercase();
            out.push_str(&format!(
                "\nNote: this diff contains {} {}. The {} adapter is preview and not enabled, so these files were not analyzed — this is NOT a clean Rust-grade result. Enable it in ripr.toml [languages] to analyze them.\n\nTo enable, add to ripr.toml:\n\n[languages]\nenabled = [\"rust\", \"{language_lowercase}\"]\n",
                advisory.file_count, file_label, language,
            ));
            if advisory.language == "typescript" && advisory.javascript_file_count > 0 {
                out.push_str("\n(\"typescript\" enables the adapter for JavaScript files too.)\n");
            }
            if let Some(prerequisite) = crate::domain::LanguageId::from_wire(&advisory.language)
                .and_then(crate::domain::LanguageId::enable_prerequisite)
            {
                out.push_str(&format!("\n{prerequisite}.\n"));
            }
        } else if let Some(run) = advisory.non_success_run(&output.language_runs) {
            let verb = if advisory.file_count == 1 {
                "was"
            } else {
                "were"
            };
            out.push_str(&format!(
                "\nNote: the {language} preview adapter did not complete successfully ({}), so {} {} {verb} not analyzed — this is NOT a clean Rust-grade result.\n",
                run.status.as_str(), advisory.file_count, file_label,
            ));
        } else {
            out.push_str(&format!(
                "\nNote: the {language} preview adapter was enabled but no {file_label} were routed, so nothing was analyzed — this is NOT a clean Rust-grade result.\n"
            ));
        }
    }
}

/// Render per-language run-status lines for languages that did not complete
/// successfully (non-abort contract, Campaign 31 PR 10, #1403). Silent when
/// every language ran to completion.
fn render_language_runs(out: &mut String, output: &CheckOutput) {
    for run in &output.language_runs {
        let language = language_display_name(&run.language);
        let completion = if run.status == crate::analysis::LanguageRunStatus::Partial {
            "returned a partial result"
        } else {
            "analysis did not complete"
        };
        match &run.reason {
            Some(reason) => out.push_str(&format!(
                "\nNote: {} {} (status: {}). Other languages' findings are still shown above. Reason: {}\n",
                language,
                completion,
                run.status.as_str(),
                reason,
            )),
            None => out.push_str(&format!(
                "\nNote: {} {} (status: {}). Other languages' findings are still shown above.\n",
                language,
                completion,
                run.status.as_str(),
            )),
        }
    }
}

/// Prose names for a preview advisory: the adapter it ran under and the kind
/// of files it counted. The TypeScript adapter also analyzes JavaScript, so a
/// JavaScript-only diff is "JavaScript files" under the "TypeScript/JavaScript"
/// adapter, and a mixed one is "TypeScript/JavaScript files" (#4555).
/// Shared with the GitHub annotation stream so the surfaces cannot drift.
pub(crate) fn advisory_language_names(
    advisory: &crate::analysis::PreviewLanguageAdvisory,
) -> (String, String) {
    let language = language_display_name(&advisory.language);
    if advisory.language != "typescript" || advisory.javascript_file_count == 0 {
        return (language.clone(), language);
    }
    let family = "TypeScript/JavaScript".to_string();
    if advisory.javascript_file_count >= advisory.file_count {
        (family, "JavaScript".to_string())
    } else {
        (family.clone(), family)
    }
}

/// Prose name for a language wire string (`typescript` -> `TypeScript`),
/// owned by [`crate::domain::LanguageId::display_name`].
fn language_display_name(wire: &str) -> String {
    crate::domain::LanguageId::display_name_for_wire(wire)
        .map(str::to_string)
        .unwrap_or_else(|| capitalize_first(wire))
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Render one finding section for the human-readable CLI output.
pub fn render_finding(finding: &Finding) -> String {
    render_finding_with_config(finding, &RiprConfig::default())
}

/// Render one finding with the bounded context follow-up for the selected
/// finding.
pub(crate) fn render_finding_with_context_command(
    finding: &Finding,
    config: &RiprConfig,
    context_command: &str,
) -> String {
    let mut out = render_finding_with_config(finding, config);
    out.push_str(&format!("\nNext: {context_command}\n"));
    out
}

mod evidence_lines;
mod sections;
mod triage;

pub(crate) use sections::render_finding_with_config;

#[cfg(test)]
mod tests {
    use super::{render, render_finding};
    use crate::analysis::PreviewLanguageAdvisory;
    use crate::app::{CheckOutput, Mode};
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, FindingCanonicalGap,
        FlowSinkFact, FlowSinkKind, LanguageFileCount, LanguageId, LanguageStatus,
        MISSING_DISCRIMINATOR_VALUE_PREFIX, MissingDiscriminatorFact, OracleKind, OracleStrength,
        Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation,
        StageEvidence, StageState, Summary, SymbolId, ValueContext, ValueFact,
    };
    use std::path::PathBuf;

    #[test]
    fn render_includes_summary_counts_and_empty_findings_message() {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 8,
                exposed: 1,
                weakly_exposed: 2,
                reachable_unrevealed: 1,
                no_static_path: 1,
                static_unknown: 1,
                infection_unknown: 1,
                propagation_unknown: 1,
                ..Summary::default()
            },
            findings: vec![],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("mode: draft"));
        // #4322: canonical class tokens, unknown classes kept separate, and a
        // shown/total denominator.
        assert!(rendered.contains(
            "Summary: 8 probe(s), 1 exposed, 2 weakly_exposed, 1 reachable_unrevealed, 1 no_static_path, 1 infection_unknown, 1 propagation_unknown, 1 static_unknown; 0 of 0 finding(s) shown"
        ));
        assert!(rendered.contains("No diff-derived static exposure probes found."));
        assert!(!rendered.contains("Next:"));
    }

    /// #4322: the three unknown classes answer *which* discriminator
    /// machinery is missing, so the header must keep them separate instead of
    /// summing them into one bucket.
    #[test]
    fn summary_header_keeps_the_unknown_classes_separate() {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 9,
                findings: 9,
                infection_unknown: 3,
                propagation_unknown: 5,
                static_unknown: 1,
                ..Summary::default()
            },
            findings: vec![],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains(
                "3 infection_unknown, 5 propagation_unknown, 1 static_unknown; 9 of 9 finding(s) shown"
            ),
            "unknown classes must render separately; got:\n{rendered}"
        );
        assert!(
            !rendered.contains(" unknown\n"),
            "the summed unknown bucket must not return; got:\n{rendered}"
        );
    }

    /// #4322: per-class buckets count unsuppressed findings only, so the
    /// header's shown/total denominator must disclose the suppressed
    /// remainder instead of letting a reader sum mismatching counts.
    #[test]
    fn summary_header_shown_total_denominator_reflects_suppression() {
        use crate::output::suppressions::{CheckSuppressionOutcome, SuppressedCheckFinding};
        let mut kept = sample_finding();
        kept.id = "kept-finding".to_string();
        let mut suppressed = sample_finding();
        suppressed.id = "suppressed-finding".to_string();
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 2,
                findings: 2,
                weakly_exposed: 1,
                ..Summary::default()
            },
            findings: vec![kept, suppressed],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: Some(CheckSuppressionOutcome {
                policy_path: "policy/ripr-suppressions.toml".to_string(),
                suppressed: vec![SuppressedCheckFinding {
                    finding_id: "suppressed-finding".to_string(),
                    selector: "src/**".to_string(),
                }],
                warnings: Vec::new(),
            }),
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("; 1 of 2 finding(s) shown"),
            "the denominator must disclose the suppressed remainder; got:\n{rendered}"
        );
    }

    /// #4322: the header tokens are the canonical `ExposureClass` wire
    /// tokens, so grepping the header against finding lines (or the spec
    /// vocabulary) succeeds. The distinctive per-class counts pin both token
    /// and position; adding a class without extending the header fails here.
    #[test]
    fn summary_header_names_every_class_with_its_canonical_token() {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 7,
                findings: 7,
                exposed: 1,
                weakly_exposed: 2,
                reachable_unrevealed: 3,
                no_static_path: 4,
                infection_unknown: 5,
                propagation_unknown: 6,
                static_unknown: 7,
                ..Summary::default()
            },
            findings: vec![],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains(
            "Summary: 7 probe(s), 1 exposed, 2 weakly_exposed, 3 reachable_unrevealed, \
             4 no_static_path, 5 infection_unknown, 6 propagation_unknown, 7 static_unknown; \
             7 of 7 finding(s) shown"
        ));
        // Token-to-taxonomy binding: every canonical token appears in the
        // header, so a renamed or added class cannot silently diverge.
        for class in [
            ExposureClass::Exposed,
            ExposureClass::WeaklyExposed,
            ExposureClass::ReachableUnrevealed,
            ExposureClass::NoStaticPath,
            ExposureClass::InfectionUnknown,
            ExposureClass::PropagationUnknown,
            ExposureClass::StaticUnknown,
        ] {
            assert!(
                rendered.contains(class.as_str()),
                "header must carry the canonical token {}; got:\n{rendered}",
                class.as_str()
            );
        }
    }

    fn partial_outcome_output(findings: Vec<Finding>) -> Result<CheckOutput, String> {
        use crate::analysis_outcome::{
            AnalysisIdentity, AnalysisLimitation, AnalysisLimitationKind, AnalysisOutcome,
            AnalysisOutcomeCounts, AnalysisOutcomeKind, AnalysisRecovery, AnalysisRecoveryKind,
            AnalysisStage,
        };
        let limitation = AnalysisLimitation::new(
            AnalysisLimitationKind::CombinedHunkUnsupported,
            AnalysisStage::DiffParse,
            AnalysisRecovery::new(
                AnalysisRecoveryKind::UseTwoWayDiff,
                "Re-run against a two-way diff of the merge result.",
            )?,
        );
        let outcome = AnalysisOutcome::new(
            AnalysisOutcomeKind::PartialWithLimitations,
            AnalysisIdentity::default(),
            AnalysisOutcomeCounts {
                changed_file_count: 1,
                changed_line_count: 2,
                finding_count: u64::try_from(findings.len()).unwrap_or(u64::MAX),
                ..AnalysisOutcomeCounts::default()
            },
            vec![limitation],
        )?;
        Ok(CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings,
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: Some(outcome),
            partial_scope: None,
        })
    }

    #[test]
    fn partial_outcome_with_findings_does_not_claim_zero_findings() -> Result<(), String> {
        let rendered = render(&partial_outcome_output(vec![sample_finding()])?);
        assert!(rendered.contains("analysis incomplete"));
        assert!(
            !rendered.contains("Zero findings is not a clean result"),
            "{rendered}"
        );
        assert!(rendered.contains(
            "  The 1 finding(s) below cover only the analyzed scope; behavior outside it has no finding.\n"
        ));

        let empty = render(&partial_outcome_output(Vec::new())?);
        assert!(empty.contains("Zero findings is not a clean result"));
        assert!(!empty.contains("finding(s) below cover only"));
        Ok(())
    }

    // #4952: the EOL-only churn disclosure is a churn-shape note, not an
    // incompleteness, so a complete run carrying it renders the limitation
    // without the incomplete-scope hedges.
    fn eol_only_outcome_output(findings: Vec<Finding>) -> Result<CheckOutput, String> {
        use crate::analysis_outcome::{
            AnalysisIdentity, AnalysisLimitation, AnalysisLimitationKind, AnalysisOutcome,
            AnalysisOutcomeCounts, AnalysisOutcomeKind, AnalysisRecovery, AnalysisRecoveryKind,
            AnalysisStage,
        };
        let limitation = AnalysisLimitation::new(
            AnalysisLimitationKind::EolOnlyChurn,
            AnalysisStage::DiffParse,
            AnalysisRecovery::new(
                AnalysisRecoveryKind::Retry,
                "Normalize line endings and re-run the analysis.",
            )?,
        )
        .with_affected_items(1)?
        .with_detail(
            "1 file(s) changed only in line endings; probes treat text as unchanged: src/lib.rs",
        )?;
        let outcome = AnalysisOutcome::new(
            if findings.is_empty() {
                AnalysisOutcomeKind::CompleteNoFindings
            } else {
                AnalysisOutcomeKind::CompleteWithFindings
            },
            AnalysisIdentity::default(),
            AnalysisOutcomeCounts {
                changed_file_count: 1,
                changed_line_count: 2,
                // CompleteNoFindings requires a probe or candidate subject.
                candidate_line_count: if findings.is_empty() { 1 } else { 0 },
                finding_count: u64::try_from(findings.len()).unwrap_or(u64::MAX),
                ..AnalysisOutcomeCounts::default()
            },
            vec![limitation],
        )?;
        Ok(CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings,
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: Some(outcome),
            partial_scope: None,
        })
    }

    #[test]
    fn eol_only_disclosure_renders_without_the_incomplete_scope_hedges() -> Result<(), String> {
        let rendered = render(&eol_only_outcome_output(vec![sample_finding()])?);
        assert!(rendered.contains("analysis complete"), "{rendered}");
        assert!(
            rendered.contains(
                "Limitation: some files changed only in line endings during parsing the diff (eol_only_churn at diff_parse)"
            ),
            "{rendered}"
        );
        assert!(rendered.contains("affected items: 1"), "{rendered}");
        // A disclosure does not scope the analysis down: no incomplete-scope
        // hedge may accompany it.
        assert!(
            !rendered.contains("finding(s) below cover only"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("Zero findings is not a clean result"),
            "{rendered}"
        );
        Ok(())
    }

    #[test]
    fn eol_only_disclosure_with_zero_findings_does_not_claim_incomplete_scope() -> Result<(), String>
    {
        let rendered = render(&eol_only_outcome_output(Vec::new())?);
        assert!(
            !rendered.contains("Zero findings is not a clean result"),
            "{rendered}"
        );
        assert!(rendered.contains("eol_only_churn"), "{rendered}");
        Ok(())
    }

    #[test]
    fn bounded_human_output_suggests_explain_and_context_for_top_finding() {
        let finding = sample_finding();
        let finding_id = finding.id.clone();
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 1,
                findings: 1,
                weakly_exposed: 1,
                ..Summary::default()
            },
            findings: vec![finding],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("Next: drill into the top finding:"));
        assert!(rendered.contains(&format!("  ripr explain {finding_id}\n")));
        assert!(rendered.contains(&format!("  ripr context --at {finding_id}\n")));
    }

    /// A Python preview finding; `carded` controls whether the Python repair
    /// card authority (`python_repair_card`) can build a card for it.
    fn python_preview_finding(line: usize, carded: bool) -> Finding {
        let mut finding = sample_finding();
        finding.id = format!("probe:pricing___init__.py:python_preview:{line}");
        finding.probe.location = SourceLocation::new("pricing/__init__.py", line, 5);
        finding.language = Some(LanguageId::Python);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.confidence = 0.4;
        finding.canonical_gap = Some(FindingCanonicalGap {
            id: format!(
                "gap:python:pricing/__init__.py:owner{line}:predicate_boundary:predicate:x"
            ),
            language: "python".to_string(),
            file: "pricing/__init__.py".to_string(),
            owner: format!("owner{line}"),
            behavior_kind: "predicate_boundary".to_string(),
            probe_kind: "predicate".to_string(),
            normalized_discriminator: "x".to_string(),
        });
        if carded {
            finding.evidence = vec![
                "suggested_test_file: tests/test_pricing.py".to_string(),
                "suggested_test_name: test_small_order_pays_shipping".to_string(),
                "suggested_verify_command: pytest tests/test_pricing.py::test_small_order_pays_shipping".to_string(),
                "suggested_verify_command_confidence: high".to_string(),
            ];
        } else {
            finding.activation.missing_discriminators.clear();
        }
        finding
    }

    /// rc rehearsal (py-pricing): Start here picked the card-less constant
    /// finding at line 5 over the carded literal finding at line 11, so check
    /// said "no ripr command routes this" while pilot had a card. A Python
    /// finding with a repair card now outranks one without; classification
    /// stays the same.
    #[test]
    fn start_here_prefers_a_python_finding_with_a_repair_card() {
        let uncarded = python_preview_finding(5, false);
        let carded = python_preview_finding(11, true);
        assert!(crate::output::python_repair_card::python_repair_card(&uncarded).is_none());
        assert!(crate::output::python_repair_card::python_repair_card(&carded).is_some());
        for findings in [
            vec![uncarded.clone(), carded.clone()],
            vec![carded.clone(), uncarded.clone()],
        ] {
            let output = CheckOutput {
                harness_projections: Vec::new(),
                schema_version: "0.1".to_string(),
                tool: "ripr".to_string(),
                mode: Mode::Draft,
                root: PathBuf::from("repo"),
                base: None,
                summary: Summary {
                    probes: 2,
                    findings: 2,
                    weakly_exposed: 2,
                    ..Summary::default()
                },
                findings,
                preview_language_advisories: Vec::new(),
                language_runs: Vec::new(),
                no_scope_provided: false,
                unanalyzed_working_tree: false,
                suppression: None,
                analysis_outcome: None,
                partial_scope: None,
            };
            let rendered = render(&output);
            assert!(
                rendered.contains("State: preview language, advisory only (preview_limited)"),
                "{rendered}"
            );
            assert!(
                rendered.contains("  File: pricing/__init__.py:11\n"),
                "{rendered}"
            );
            assert!(!rendered.contains("has no repair card"), "{rendered}");
            assert!(
                rendered.contains("apply the next step below to the suggested test"),
                "{rendered}"
            );
        }
    }

    /// #2567: nothing was omitted, so the render must not advertise a hidden
    /// remainder. The format pointers stay under `More:`.
    #[test]
    fn render_replaces_hidden_block_with_more_when_nothing_is_omitted() {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 1,
                findings: 1,
                weakly_exposed: 1,
                ..Summary::default()
            },
            findings: vec![sample_finding()],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("\nMore:\n"));
        assert!(!rendered.contains("Hidden:"));
        assert!(!rendered.contains("lower-priority finding(s) omitted"));
        assert!(rendered.contains("  Full evidence: rerun with --format human-full\n"));
        assert!(rendered.contains("  Machine data: rerun with --format json\n"));
    }

    /// #2567: a real truncated remainder keeps the `Hidden:` heading and the
    /// count line, because that is the section's entire purpose.
    #[test]
    fn render_keeps_hidden_block_when_findings_are_omitted() {
        let mut findings = Vec::new();
        for index in 0..3 {
            let mut finding = sample_finding();
            finding.id = format!("finding-{index}");
            finding.probe.location = SourceLocation::new(format!("src/f{index}.rs"), 1, 1);
            findings.push(finding);
        }
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 3,
                findings: 3,
                weakly_exposed: 3,
                ..Summary::default()
            },
            findings,
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("\nHidden:\n"));
        // #4395(b): unlabeled Rust-only remainder stays the count line with
        // no invented language or preview identity.
        assert!(
            rendered.contains("  2 lower-priority finding(s) omitted from default human output.\n")
        );
        assert!(!rendered.contains("preview"));
        assert!(!rendered.contains("More:"));
    }

    /// #4320: the `Hidden:` block must name what it hides by
    /// `file:line (class)` so a reader can confirm a file they care about was
    /// covered without a rerun.
    #[test]
    fn hidden_block_lists_omitted_findings_by_file_line_and_class() {
        let findings = (0..3)
            .map(|index| {
                let mut finding = sample_finding();
                finding.id = format!("finding-{index}");
                finding.probe.location = SourceLocation::new(format!("src/f{index}.rs"), 1, 1);
                finding
            })
            .collect::<Vec<_>>();

        let rendered = render(&bounded_output_with_findings(findings));

        assert!(
            rendered.contains("    - src/f1.rs:1 (weakly_exposed)\n"),
            "{rendered}"
        );
        assert!(
            rendered.contains("    - src/f2.rs:1 (weakly_exposed)\n"),
            "{rendered}"
        );
    }

    /// #4320: when the #3281 candidate filter hides every finding, the run is
    /// all base-side evidence — the honest framing names that (with the
    /// human-full rerun pointer), not a lower-priority framing, and the
    /// suppressed-by-policy claim must not fire when nothing was suppressed.
    #[test]
    fn hidden_block_all_base_side_run_names_base_side_evidence() {
        let findings = (0..2)
            .map(|index| {
                let mut finding = sample_finding();
                finding.id = format!("base-finding-{index}");
                finding.probe.location = SourceLocation::new(format!("src/base_{index}.rs"), 1, 1);
                finding.source_currentness = crate::domain::SourceCurrentness::BaseDeleted;
                finding
            })
            .collect::<Vec<_>>();

        let rendered = render(&bounded_output_with_findings(findings));

        assert!(rendered.contains("(no_actionable_gap)"), "{rendered}");
        assert!(
            rendered.contains(
                "  Safe next action: all findings are base-side evidence, not candidate edit targets; rerun with --format human-full to inspect the full evidence before treating this run as actionable.\n"
            ),
            "{rendered}"
        );
        assert!(!rendered.contains("suppressed by policy"), "{rendered}");
        assert!(rendered.contains("\nHidden:\n"), "{rendered}");
        assert!(
            rendered.contains(
                "  All 2 finding(s) are base-side evidence, not candidate edit targets — rerun with --format human-full for the full evidence.\n"
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains("    - src/base_0.rs:1 (weakly_exposed)\n"),
            "{rendered}"
        );
        assert!(
            rendered.contains("    - src/base_1.rs:1 (weakly_exposed)\n"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("lower-priority finding(s) omitted"),
            "the all-base-side case must not use the lower-priority framing:\n{rendered}"
        );
    }

    /// #4320 review: `unresolved_subject` is the explicit unknown (#3281), not
    /// base-side evidence. A run where nothing is resolved must not call the
    /// findings base-side — the wording preserves the unknown-currentness
    /// distinction on both the Hidden line and the safe action.
    #[test]
    fn hidden_block_unresolved_subject_run_names_the_unknown_not_base_side() {
        let findings = (0..2)
            .map(|index| {
                let mut finding = sample_finding();
                finding.id = format!("unresolved-finding-{index}");
                finding.probe.location =
                    SourceLocation::new(format!("src/unresolved_{index}.rs"), 1, 1);
                finding.source_currentness = crate::domain::SourceCurrentness::UnresolvedSubject;
                finding
            })
            .collect::<Vec<_>>();

        let rendered = render(&bounded_output_with_findings(findings));

        assert!(rendered.contains("(no_actionable_gap)"), "{rendered}");
        assert!(
            !rendered.contains("base-side evidence"),
            "unknown currentness must not read as base-side:\n{rendered}"
        );
        assert!(
            rendered.contains(
                "  All 2 finding(s) have unresolved subject currentness — not established base-side or candidate edit targets; rerun with --format human-full for the full evidence.\n"
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains(
                "  Safe next action: no finding is resolved to the candidate (subject currentness unresolved); rerun with --format human-full to inspect the full evidence before treating this run as actionable.\n"
            ),
            "{rendered}"
        );
    }

    /// #4320 review: a mixed no-selection run names both dispositions with
    /// their counts instead of collapsing them into one claim.
    #[test]
    fn hidden_block_mixed_currentness_run_names_base_side_and_unresolved_counts() {
        let mut base = sample_finding();
        base.id = "mixed-base".to_string();
        base.probe.location = SourceLocation::new("src/mixed_base.rs".to_string(), 1, 1);
        base.source_currentness = crate::domain::SourceCurrentness::MovedOrRenamed;
        let mut unresolved = sample_finding();
        unresolved.id = "mixed-unresolved".to_string();
        unresolved.probe.location =
            SourceLocation::new("src/mixed_unresolved.rs".to_string(), 1, 1);
        unresolved.source_currentness = crate::domain::SourceCurrentness::UnresolvedSubject;

        let rendered = render(&bounded_output_with_findings(vec![base, unresolved]));

        assert!(
            rendered.contains(
                "  None of the 2 finding(s) is a candidate edit target (1 base-side, 1 unresolved currentness) — rerun with --format human-full for the full evidence.\n"
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains(
                "  Safe next action: no finding is a candidate edit target (1 base-side, 1 unresolved currentness); rerun with --format human-full to inspect the full evidence before treating this run as actionable.\n"
            ),
            "{rendered}"
        );
    }

    /// #4320: the `Hidden:` list is itself a bounded window — beyond the
    /// `HIDDEN_FINDINGS_LISTED` cap it discloses the remainder instead of
    /// printing every identity, keeping the default surface bounded.
    #[test]
    fn hidden_block_list_discloses_remainder_beyond_its_window() {
        let findings = (0..26)
            .map(|index| {
                let mut finding = sample_finding();
                finding.id = format!("finding-{index}");
                finding.probe.location = SourceLocation::new("src/f.rs", index + 1, 1);
                finding
            })
            .collect::<Vec<_>>();

        let rendered = render(&bounded_output_with_findings(findings));

        assert!(
            rendered.contains("25 lower-priority finding(s) omitted"),
            "{rendered}"
        );
        assert_eq!(
            rendered
                .lines()
                .filter(|line| line.starts_with("    - src/f.rs:"))
                .count(),
            20,
            "the list window stays bounded:\n{rendered}"
        );
        assert!(
            rendered.contains(
                "    - … and 5 more omitted finding(s); every identity is in --format json.\n"
            ),
            "{rendered}"
        );
        assert!(
            rendered.lines().count() < 150,
            "the default surface stays bounded with many findings"
        );
    }

    /// #4395(b): mixed-repo remainder must name the omitted preview-language
    /// identity. Ranking still selects the Rust gap; the Hidden line is how
    /// a reader learns the omitted finding was Python preview.
    #[test]
    fn hidden_line_names_omitted_preview_language_identity() {
        let mut rust = sample_finding();
        rust.id = "rust-gap".to_string();
        rust.language = Some(LanguageId::Rust);
        rust.language_status = Some(LanguageStatus::Stable);
        rust.probe.location = SourceLocation::new("src/lib.rs", 4, 1);

        let mut python = sample_finding();
        python.id = "python-preview-gap".to_string();
        python.language = Some(LanguageId::Python);
        python.language_status = Some(LanguageStatus::Preview);
        python.probe.location = SourceLocation::new("src/margin.py", 2, 1);
        python.recommended_next_step = Some("Add a Python preview assertion.".to_string());

        let rendered = render(&bounded_output_with_findings(vec![rust, python]));

        assert!(rendered.contains("State: a test gap to inspect or repair (top_gap)"));
        assert!(rendered.contains("File: src/lib.rs:4"));
        assert!(!rendered.contains("File: src/margin.py:2"));
        assert!(
            rendered.contains(
                "  1 lower-priority finding(s) omitted from default human output (Python preview: 1).\n"
            ),
            "Hidden line must name omitted Python preview identity; got:\n{rendered}"
        );
    }

    /// #4395(b): labeled Rust-only remainder must stay byte-identical to the
    /// count line. Adding identity for every omitted Rust finding would churn
    /// the default human surface without answering the mixed-repo question.
    #[test]
    fn hidden_line_stays_count_only_when_omitted_findings_are_labeled_rust() {
        let mut findings = Vec::new();
        for index in 0..3 {
            let mut finding = sample_finding();
            finding.id = format!("rust-{index}");
            finding.language = Some(LanguageId::Rust);
            finding.language_status = Some(LanguageStatus::Stable);
            finding.probe.location = SourceLocation::new(format!("src/f{index}.rs"), 1, 1);
            findings.push(finding);
        }

        let rendered = render(&bounded_output_with_findings(findings));

        assert!(
            rendered.contains("  2 lower-priority finding(s) omitted from default human output.\n")
        );
        assert!(
            !rendered.contains("Rust:"),
            "Rust-only Hidden line must not grow a language breakdown; got:\n{rendered}"
        );
        assert!(!rendered.contains("preview"));
    }

    /// #4395(b): when every omitted finding is preview, name that language
    /// even if Start here is already `preview_limited`.
    #[test]
    fn hidden_line_names_preview_identity_when_all_omitted_findings_are_preview() {
        let mut findings = Vec::new();
        for index in 0..3 {
            let mut finding = sample_finding();
            finding.id = format!("python-{index}");
            finding.language = Some(LanguageId::Python);
            finding.language_status = Some(LanguageStatus::Preview);
            finding.probe.location = SourceLocation::new(format!("src/f{index}.py"), 1, 1);
            findings.push(finding);
        }

        let rendered = render(&bounded_output_with_findings(findings));

        assert!(rendered.contains("State: preview language, advisory only (preview_limited)"));
        assert!(
            rendered.contains(
                "  2 lower-priority finding(s) omitted from default human output (Python preview: 2).\n"
            ),
            "preview-only remainder must still name Python preview identity; got:\n{rendered}"
        );
    }

    /// #4395(b): preview status without a language id must not invent a
    /// language name. The count line still warns that omitted findings may
    /// include preview-language evidence.
    #[test]
    fn hidden_line_hedges_unlabeled_preview_omitted_findings() {
        let mut selected = sample_finding();
        selected.id = "rust-selected".to_string();
        selected.language = Some(LanguageId::Rust);
        selected.probe.location = SourceLocation::new("src/lib.rs", 1, 1);

        let mut omitted = sample_finding();
        omitted.id = "unlabeled-preview".to_string();
        omitted.language = None;
        omitted.language_status = Some(LanguageStatus::Preview);
        omitted.probe.location = SourceLocation::new("src/unknown.py", 1, 1);

        let rendered = render(&bounded_output_with_findings(vec![selected, omitted]));

        assert!(
            rendered.contains(
                "  1 lower-priority finding(s) omitted from default human output (preview-language: 1).\n"
            ),
            "unlabeled preview remainder must not invent a language; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("Python preview"),
            "unlabeled preview must not be credited as Python; got:\n{rendered}"
        );
    }

    /// #4395(b): two preview languages in the omitted set are counted
    /// separately. Token coincidence on one language name must not hide the
    /// other.
    #[test]
    fn hidden_line_counts_each_omitted_preview_language() {
        let mut rust = sample_finding();
        rust.id = "rust-gap".to_string();
        rust.language = Some(LanguageId::Rust);
        rust.probe.location = SourceLocation::new("src/lib.rs", 1, 1);

        let mut python = sample_finding();
        python.id = "python-gap".to_string();
        python.language = Some(LanguageId::Python);
        python.language_status = Some(LanguageStatus::Preview);
        python.probe.location = SourceLocation::new("src/a.py", 1, 1);

        let mut typescript = sample_finding();
        typescript.id = "ts-gap".to_string();
        typescript.language = Some(LanguageId::TypeScript);
        typescript.language_status = Some(LanguageStatus::Preview);
        typescript.probe.location = SourceLocation::new("src/a.ts", 1, 1);

        let rendered = render(&bounded_output_with_findings(vec![
            rust, python, typescript,
        ]));

        assert!(
            rendered.contains(
                "  2 lower-priority finding(s) omitted from default human output (TypeScript preview: 1, Python preview: 1).\n"
            ),
            "each omitted preview language must appear with its own count; got:\n{rendered}"
        );
    }

    /// #2103: a Rust-only run emits no per-language breakdown line, so
    /// Rust-only human output stays byte-identical.
    #[test]
    fn render_omits_language_breakdown_for_rust_only_run() {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                changed_rust_files: 2,
                changed_files_by_language: vec![LanguageFileCount {
                    language: "rust".to_string(),
                    files: 2,
                }],
                ..Summary::default()
            },
            findings: vec![],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            !rendered.contains("by language"),
            "Rust-only output must not contain a per-language breakdown; got:\n{rendered}"
        );
    }

    /// #2103: when a non-Rust adapter counted files, the breakdown line shows
    /// the per-language split.
    #[test]
    fn render_emits_language_breakdown_when_non_rust_files_counted() {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                changed_rust_files: 1,
                changed_files_by_language: vec![
                    LanguageFileCount {
                        language: "python".to_string(),
                        files: 5,
                    },
                    LanguageFileCount {
                        language: "rust".to_string(),
                        files: 1,
                    },
                ],
                ..Summary::default()
            },
            findings: vec![],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("Changed file(s) by language: python: 5, rust: 1."),
            "expected per-language breakdown line; got:\n{rendered}"
        );

        let mut python_only = output;
        python_only.summary.changed_rust_files = 0;
        python_only.summary.changed_files_by_language[1].files = 0;
        let rendered = render(&python_only);
        assert!(
            rendered.contains("Changed file(s) by language: python: 5.\n"),
            "zero-count languages are omitted; got:\n{rendered}"
        );
    }

    #[test]
    fn bounded_human_output_caps_many_findings_and_reports_omitted_count() {
        let findings = (0..600)
            .map(|idx| {
                let mut finding = sample_finding();
                finding.id = format!("finding-{idx}");
                finding.probe.location.line = idx + 1;
                finding
            })
            .collect::<Vec<_>>();
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 600,
                findings: 600,
                weakly_exposed: 600,
                ..Summary::default()
            },
            findings,
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("Start here:"));
        assert!(rendered.contains("State: a test gap to inspect or repair (top_gap)"));
        assert!(rendered.contains("599 lower-priority finding(s) omitted"));
        assert!(rendered.contains("--format human-full"));
        assert!(rendered.lines().count() < 150);
        assert_eq!(rendered.matches("Static exposure").count(), 1);
    }

    #[test]
    fn bounded_human_output_does_not_select_exposed_over_non_exposed_repair() {
        let mut exposed = sample_finding();
        exposed.id = "exposed-with-route".to_string();
        exposed.class = ExposureClass::Exposed;
        exposed.probe.location = SourceLocation::new("src/exposed.rs", 1, 1);
        exposed.confidence = 0.99;
        exposed.recommended_next_step = Some("Review the already exposed evidence.".to_string());

        let mut actionable = sample_finding();
        actionable.id = "non-exposed-with-route".to_string();
        actionable.class = ExposureClass::ReachableUnrevealed;
        actionable.probe.location = SourceLocation::new("src/actionable.rs", 9, 1);
        actionable.recommended_next_step =
            Some("Add the missing discriminator assertion.".to_string());

        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 2,
                findings: 2,
                exposed: 1,
                reachable_unrevealed: 1,
                ..Summary::default()
            },
            findings: vec![exposed, actionable],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("State: a test gap to inspect or repair (top_gap)"));
        assert!(rendered.contains("File: src/actionable.rs:9"));
        assert!(rendered.contains("Static exposure: unrevealed (reachable_unrevealed, "));
        assert!(!rendered.contains("File: src/exposed.rs:1"));
    }

    #[test]
    fn bounded_human_output_reports_missing_scope_as_start_here_state() {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: true,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("Start here:"));
        assert!(rendered.contains("State: nothing in scope (missing_scope)"));
        assert!(rendered.contains("provide an analysis scope"));
        assert!(rendered.contains("No diff-derived static exposure probes found."));
    }

    #[test]
    fn bounded_human_output_keeps_preview_language_in_preview_limited_state() {
        let mut finding = sample_finding();
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.recommended_next_step = Some("Add a TypeScript preview repair.".to_string());
        finding
            .evidence
            .push("suggested_verify_command: npm test -- pricing".to_string());
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 1,
                findings: 1,
                weakly_exposed: 1,
                ..Summary::default()
            },
            findings: vec![finding],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("State: preview language, advisory only (preview_limited)"));
        assert!(rendered.contains("preview-language evidence is advisory"));
        assert!(!rendered.contains("State: a test gap to inspect or repair (top_gap)"));
    }

    // #2273: an `exposed` finding can carry an observation rationale (not a
    // missing discriminator) in `missing`; the digest label must reflect the
    // discriminator state instead of contradicting the class.
    #[test]
    fn digest_labels_observation_rationale_as_observed_advisory_for_exposed() {
        let mut finding = sample_finding();
        finding.class = ExposureClass::Exposed;
        finding.missing = vec![
            "Related test reaches `applyDiscount` with a `exact_value` oracle; behavior observed."
                .to_string(),
        ];

        let digest = super::sections::render_finding_digest_with_config(
            &finding,
            &crate::config::RiprConfig::default(),
        );

        assert!(
            digest.contains(
                "  Discriminator (observed, advisory): Related test reaches `applyDiscount`"
            ),
            "expected observed-advisory label in digest; got:\n{digest}"
        );
        assert!(
            !digest.contains("Missing discriminator:"),
            "exposed digest must not claim a missing discriminator; got:\n{digest}"
        );
    }

    // #3317 follow-up (RIPR-SPEC-0162): the why-hint must not assert the
    // propagation the class marks unknown, and unknown-class limitation
    // prose renders under the Analyzer limit label — while a real
    // missing discriminator keeps its own label.
    #[test]
    fn propagation_unknown_wording_is_honest() -> Result<(), String> {
        let mut finding = sample_finding();
        finding.class = ExposureClass::PropagationUnknown;
        finding.ripr.propagate = stage(
            StageState::Unknown,
            Confidence::Low,
            "Propagation is not statically obvious from syntax-first analysis",
        );
        finding.missing = vec![
            "No clear propagation path from changed behavior to an observable sink".to_string(),
        ];
        finding.activation.missing_discriminators = Vec::new();
        let digest = super::sections::render_finding_digest_with_config(
            &finding,
            &crate::config::RiprConfig::default(),
        );
        if !digest.contains(
            "Why unknown: the path from the changed behavior to an observable sink is not statically clear",
        ) {
            return Err(format!("honest why-hint missing:
{digest}"));
        }
        if digest.contains("the change propagates but") {
            return Err(format!(
                "the hint must not assert the propagation the class marks unknown:
{digest}"
            ));
        }
        if !digest.contains(
            "  Analyzer limit: No clear propagation path from changed behavior to an observable sink",
        ) {
            return Err(format!("analyzer-limit label missing:
{digest}"));
        }
        // A real missing discriminator keeps the discriminator label even
        // on the unknown classes. #4320: the digest discloses the window —
        // the full Weakness section here carries two entries.
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "end == start".to_string(),
            reason: "no related test call uses end equal to start".to_string(),
            flow_sink: None,
        }];
        finding.missing = vec!["No strong discriminator was detected".to_string()];
        let digest = super::sections::render_finding_digest_with_config(
            &finding,
            &crate::config::RiprConfig::default(),
        );
        if !digest
            .contains("  Missing discriminator (1 of 2): No strong discriminator was detected")
        {
            return Err(format!(
                "a finding with a real missing discriminator keeps its label:
{digest}"
            ));
        }
        Ok(())
    }

    #[test]
    fn digest_keeps_missing_discriminator_label_for_non_exposed_classes() {
        for class in [
            ExposureClass::WeaklyExposed,
            ExposureClass::ReachableUnrevealed,
            ExposureClass::NoStaticPath,
            ExposureClass::StaticUnknown,
        ] {
            let mut finding = sample_finding();
            finding.class = class;
            finding.missing = vec!["missing strong oracle".to_string()];

            let digest = super::sections::render_finding_digest_with_config(
                &finding,
                &crate::config::RiprConfig::default(),
            );

            assert!(
                digest.contains("  Missing discriminator (1 of 2): missing strong oracle"),
                "expected missing-discriminator label for {:?}; got:\n{digest}",
                finding.class
            );
            assert!(
                !digest.contains("Discriminator (observed, advisory)"),
                "non-exposed digest must not use the observed-advisory label; got:\n{digest}"
            );
        }
    }

    /// RIPR-SPEC-0122: the digest line carries the discriminator value alone.
    /// `decision.rs` builds value-shaped entries as
    /// `Missing discriminator value: <value>`, which the renderer used to print
    /// verbatim under its own label, producing
    /// `Missing discriminator: Missing discriminator value: X`.
    #[test]
    fn digest_missing_discriminator_line_does_not_restate_its_label() {
        let mut finding = sample_finding();
        finding.class = ExposureClass::WeaklyExposed;
        finding.missing = vec![format!(
            "{MISSING_DISCRIMINATOR_VALUE_PREFIX}AuthError::RevokedToken"
        )];

        let digest = super::sections::render_finding_digest_with_config(
            &finding,
            &crate::config::RiprConfig::default(),
        );

        assert!(
            digest.contains("  Missing discriminator (1 of 2): AuthError::RevokedToken\n"),
            "digest must print the value alone and disclose the window; got:\n{digest}"
        );
        assert!(
            !digest.contains("Missing discriminator: Missing discriminator"),
            "digest must not restate its own label; got:\n{digest}"
        );
    }

    /// The same field also carries prose entries, which have no value prefix
    /// and must survive the strip untouched.
    #[test]
    fn digest_missing_discriminator_line_keeps_prose_entries_verbatim() {
        let mut finding = sample_finding();
        finding.class = ExposureClass::WeaklyExposed;
        finding.missing = vec!["No strong discriminator was detected".to_string()];

        let digest = super::sections::render_finding_digest_with_config(
            &finding,
            &crate::config::RiprConfig::default(),
        );

        assert!(
            digest.contains(
                "  Missing discriminator (1 of 2): No strong discriminator was detected\n"
            ),
            "prose entries must render unchanged; got:\n{digest}"
        );
    }

    #[test]
    fn digest_surfaces_preview_language_metadata() {
        let mut finding = sample_finding();
        finding.language = Some(LanguageId::Python);
        finding.language_status = Some(LanguageStatus::Preview);

        let digest = super::sections::render_finding_digest_with_config(
            &finding,
            &crate::config::RiprConfig::default(),
        );

        assert!(digest.contains("  Language: python\n"), "digest:\n{digest}");
        assert!(
            digest.contains("  Language status: preview\n"),
            "digest:\n{digest}"
        );
    }

    #[test]
    fn digest_omits_stable_rust_language_metadata() {
        let mut finding = sample_finding();
        finding.language = Some(LanguageId::Rust);
        finding.language_status = Some(LanguageStatus::Stable);

        let digest = super::sections::render_finding_digest_with_config(
            &finding,
            &crate::config::RiprConfig::default(),
        );

        assert!(!digest.contains("  Language:"), "digest:\n{digest}");
    }

    // #2273 / #4216: the preview_limited safe next action distinguishes a
    // complete-but-advisory repair packet (shared validator authority) from a
    // packet the validator kept closed. A closed packet gets a terminal line
    // that quotes the validator's `why_not_actionable` verbatim and names the
    // manual step, never "complete the missing repair-packet fields", which
    // the operator cannot supply.
    #[test]
    fn preview_limited_safe_action_names_terminal_manual_step_for_closed_packet()
    -> Result<(), String> {
        let finding = typescript_preview_finding(false);
        let why = crate::output::preview_actionability::preview_actionability_for(&finding)
            .filter(|actionability| !actionability.repair_packet_ready)
            .ok_or_else(|| "fixture must project a closed packet".to_string())?
            .why_not_actionable;
        let output = single_finding_output(finding);

        let rendered = render(&output);

        assert!(rendered.contains("State: preview language, advisory only (preview_limited)"));
        let expected = format!(
            "  Safe next action: this TypeScript preview finding's repair packet is not ready ({}); `ripr pilot`, `ripr agent repair` and `ripr first-pr` will not route it; add or strengthen a test by hand, then rerun `ripr check`.\n",
            super::sections::one_line(&why)
        );
        assert!(rendered.contains(&expected), "{rendered}");
        assert!(!rendered.contains("complete the missing repair-packet fields"));
        assert!(!rendered.contains("the repair packet is complete but remains advisory"));
        Ok(())
    }

    // #4216 review: an unknown-class closed packet (for example a Bun-bridge
    // cross-language visibility limit: StaticUnknown, no static_limit_kind,
    // missing fields present) is not a known weak test, so the manual step is
    // the Python static-limit check, not a test edit. A long validator reason
    // is bounded; the routing and manual-step parts stay whole.
    #[test]
    fn preview_limited_closed_packet_unknown_class_asks_for_manual_check() {
        for class in [
            ExposureClass::StaticUnknown,
            ExposureClass::InfectionUnknown,
            ExposureClass::PropagationUnknown,
        ] {
            let mut finding = typescript_preview_finding(false);
            finding.class = class.clone();
            finding
                .evidence
                .retain(|line| !line.starts_with("why_not_actionable: "));
            finding.evidence.push(format!(
                "why_not_actionable: Bun bridge visibility limit {}",
                "x".repeat(400)
            ));
            let output = single_finding_output(finding);

            let rendered = render(&output);

            let line = rendered
                .lines()
                .find(|line| line.starts_with("  Safe next action:"))
                .unwrap_or_default();
            assert!(
                line.ends_with(
                    "…); `ripr pilot`, `ripr agent repair` and `ripr first-pr` will not route it; check by hand whether a test observes this change, then rerun `ripr check`."
                ),
                "{class:?}: {line}"
            );
            assert!(!line.contains("add or strengthen a test"), "{line}");
            assert!(
                !line.contains(&"x".repeat(200)),
                "reason must be bounded: {line}"
            );
        }
    }

    // #4216: a long generic preamble must not crowd out the validator's
    // specific cause under the line budget; the cause is what the user acts on.
    #[test]
    fn preview_limited_closed_packet_shows_validator_cause_over_preamble() {
        let mut finding = typescript_preview_finding(false);
        finding
            .evidence
            .retain(|line| !line.starts_with("why_not_actionable: "));
        finding.evidence.push(format!(
            "why_not_actionable: generic preamble {}; validator: boundary constant `DISCOUNT_THRESHOLD` is unresolved",
            "p".repeat(300)
        ));
        let output = single_finding_output(finding);

        let rendered = render(&output);

        let line = rendered
            .lines()
            .find(|line| line.starts_with("  Safe next action:"))
            .unwrap_or_default();
        assert!(
            line.contains("(boundary constant `DISCOUNT_THRESHOLD` is unresolved)"),
            "{line}"
        );
        assert!(!line.contains("generic preamble"), "{line}");

        // The fixed eligibility phrase after `validator: ` is dropped too, so
        // the remedy survives the line budget.
        let mut finding = typescript_preview_finding(false);
        finding
            .evidence
            .retain(|line| !line.starts_with("why_not_actionable: "));
        finding.evidence.push(
            "why_not_actionable: generic preamble; validator: is not agent-packet eligible: observed call input `login('alice')` does not reach the missing discriminator `user.length == 3`; derive an input that hits the boundary"
                .to_string(),
        );
        let rendered = render(&single_finding_output(finding));
        let line = rendered
            .lines()
            .find(|line| line.starts_with("  Safe next action:"))
            .unwrap_or_default();
        assert!(
            line.contains("(observed call input `login('alice')` does not reach"),
            "{line}"
        );
        assert!(
            line.contains("derive an input that hits the boundary"),
            "{line}"
        );
        assert!(!line.contains("is not agent-packet eligible"), "{line}");
    }

    // #4216: an exposed TypeScript preview finding has nothing to repair; its
    // safe action matches the Python exposed wording, not the packet lines.
    #[test]
    fn preview_limited_safe_action_says_no_repair_for_exposed_typescript() {
        let mut finding = typescript_preview_finding(false);
        finding.class = ExposureClass::Exposed;
        let output = single_finding_output(finding);

        let rendered = render(&output);

        assert!(
            rendered.contains("State: preview language, advisory only (preview_limited)"),
            "{rendered}"
        );
        assert!(rendered.contains(
            "  Safe next action: preview-language evidence is advisory; a related test appears to observe this change, so there is no repair to make; verify independently before relying on it.\n"
        ), "{rendered}");
        assert!(!rendered.contains("complete the missing repair-packet fields"));
        assert!(!rendered.contains("repair packet is not ready"));
    }

    #[test]
    fn preview_limited_safe_action_names_complete_but_advisory_packet() {
        let finding = typescript_preview_finding(true);
        let output = single_finding_output(finding);

        let rendered = render(&output);

        assert!(rendered.contains("State: preview language, advisory only (preview_limited)"));
        // The complete packet names its own action, test file, and verify
        // command instead of a bare "verify independently" with no route.
        assert!(
            rendered.contains(
                "  Safe next action: preview-language evidence is advisory; the repair packet is complete: in `tests/discount.test.ts`, add a focused assertion for the missing discriminator `amount == threshold`, shaped like `expect(result).toBe(expected)`; run `jest tests/discount.test.ts`, then rerun `ripr check`.\n"
            ),
            "{rendered}"
        );
        assert!(!rendered.contains("verify independently before acting"));
        assert!(!rendered.contains("complete the missing repair-packet fields before acting"));
    }

    // #2273 (coderabbit thread on #2272): a packet that is blocked with no
    // missing actionability fields AND a structured static-limit kind is held
    // by the named limitation — the safe action must not tell the operator to
    // complete absent fields.
    #[test]
    fn preview_limited_safe_action_names_limitation_block_when_no_fields_missing() {
        let mut finding = typescript_preview_finding(false);
        finding
            .evidence
            .retain(|line| !line.starts_with("missing_actionability_fields: "));
        finding.static_limit_kind = Some(crate::domain::StaticLimitKind::MockedModule);
        let output = single_finding_output(finding);

        let rendered = render(&output);

        assert!(rendered.contains("State: preview language, advisory only (preview_limited)"));
        assert!(rendered.contains(
            "  Safe next action: preview-language evidence is advisory; the repair packet is blocked by the named static limitation, not by missing fields; resolve the limitation and rerun preview evidence before acting.\n"
        ));
        assert!(!rendered.contains("complete the missing repair-packet fields before acting"));
        assert!(!rendered.contains("the repair packet is complete but remains advisory"));
    }

    // Guard against over-crediting the limitation arm: the same blocked
    // packet WITHOUT a structured static-limit kind is not framed as a named
    // limitation; it gets the terminal closed-packet line (e.g. a strong-oracle
    // preview finding whose packet simply lacks projected contract fields).
    #[test]
    fn preview_limited_safe_action_uses_closed_packet_line_without_static_limit_kind() {
        let mut finding = typescript_preview_finding(false);
        finding
            .evidence
            .retain(|line| !line.starts_with("missing_actionability_fields: "));
        let output = single_finding_output(finding);

        let rendered = render(&output);

        assert!(rendered.contains("State: preview language, advisory only (preview_limited)"));
        assert!(
            rendered.contains(
                "  Safe next action: this TypeScript preview finding's repair packet is not ready ("
            ),
            "{rendered}"
        );
        assert!(!rendered.contains("blocked by the named static limitation"));
        assert!(!rendered.contains("complete the missing repair-packet fields"));
    }

    // #4216 row 1: a Python preview finding that no test reaches has no
    // repair card and no test to strengthen; the safe action says so and
    // names the manual step, never "complete the missing fields".
    #[test]
    fn preview_limited_python_no_static_path_names_untested_code() {
        let mut finding = unknown_finding();
        finding.class = ExposureClass::NoStaticPath;
        finding.language = Some(LanguageId::Python);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.probe.location = SourceLocation::new("pricing/__init__.py", 11, 1);
        let output = single_finding_output(finding);

        let rendered = render(&output);

        assert!(
            rendered.contains("State: preview language, advisory only (preview_limited)"),
            "{rendered}"
        );
        assert!(rendered.contains(
            "  Safe next action: this Python preview finding has no repair card (no Python test reaches this code), so `ripr pilot`, `ripr agent repair` and `ripr first-pr` will not route it; add a test that calls it by hand, then rerun `ripr check`.\n"
        ), "{rendered}");
        assert!(!rendered.contains("complete the missing repair-packet fields"));
    }

    fn single_finding_output(finding: Finding) -> CheckOutput {
        CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 1,
                findings: 1,
                weakly_exposed: 1,
                ..Summary::default()
            },
            findings: vec![finding],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        }
    }

    // Build a TypeScript preview finding. With `complete_packet`, the evidence
    // satisfies the shared repair-packet validator so
    // `preview_actionability_for` reports `repair_packet_ready: true`.
    fn typescript_preview_finding(complete_packet: bool) -> Finding {
        let mut finding = unknown_finding();
        finding.class = ExposureClass::WeaklyExposed;
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.owner_kind = Some(crate::domain::OwnerKind::Function);
        finding.probe.location = SourceLocation::new("src/lib.ts", 2, 1);
        finding.evidence = vec![
            "owner: applyDiscount".to_string(),
            "gap_state: advisory".to_string(),
            "actionability_category: incomplete_repair_packet".to_string(),
            "why_not_actionable: TypeScript preview lacks a complete repair packet contract"
                .to_string(),
            "repair_route: project canonical TypeScript repair packet fields later".to_string(),
            "missing_actionability_fields: canonical_gap_id, verify_command".to_string(),
            "missing_graph_legs: verify_command, receipt_command".to_string(),
            "unlock_condition: project complete repair packet fields before public projection"
                .to_string(),
            "evidence_needed_to_promote: canonical gap identity and verify command".to_string(),
            "raw_evidence_ref: leg=rust_seam;file=src/lib.ts;line=2;kind=typescript_preview_probe;source_id=probe:src_lib.ts:2:typescript_preview;owner=applyDiscount;sample=if amount >= threshold".to_string(),
        ];
        if complete_packet {
            finding
                .evidence
                .push("typescript_verify_command: jest tests/discount.test.ts".to_string());
            finding
                .evidence
                .push("typescript_oracle_observed: result".to_string());
            finding
                .evidence
                .push("typescript_oracle_expected: 50".to_string());
            finding
                .activation
                .missing_discriminators
                .push(MissingDiscriminatorFact {
                    value: "amount == threshold".to_string(),
                    reason: "changed TypeScript equality-boundary lacks a concrete discriminator"
                        .to_string(),
                    flow_sink: None,
                });
            finding.related_tests.push(RelatedTest {
                name: "applies discount at threshold".to_string(),
                file: PathBuf::from("tests/discount.test.ts"),
                line: 5,
                oracle_strength: OracleStrength::Weak,
                oracle_kind: OracleKind::ExactValue,
                oracle: Some("expect(result).toBe(50)".to_string()),
                relation_reason: None,
                relation_confidence: None,
            });
        }
        finding
    }

    #[test]
    fn bounded_human_output_prefers_stable_gap_over_preview_with_route() {
        let mut stable = sample_finding();
        stable.id = "stable-gap".to_string();
        stable.probe.location = SourceLocation::new("src/stable.rs", 10, 1);

        let mut preview = sample_finding();
        preview.id = "preview-gap".to_string();
        preview.language = Some(LanguageId::TypeScript);
        preview.language_status = Some(LanguageStatus::Preview);
        preview.probe.location = SourceLocation::new("src/preview.ts", 1, 1);
        preview.recommended_next_step = Some("Add a TypeScript preview repair.".to_string());
        preview
            .evidence
            .push("suggested_verify_command: npm test -- pricing".to_string());

        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 2,
                findings: 2,
                weakly_exposed: 2,
                ..Summary::default()
            },
            findings: vec![preview, stable],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("State: a test gap to inspect or repair (top_gap)"));
        assert!(rendered.contains("File: src/stable.rs:10"));
        assert!(!rendered.contains("File: src/preview.ts:1"));
        assert!(
            rendered.contains(
                "  1 lower-priority finding(s) omitted from default human output (TypeScript preview: 1).\n"
            ),
            "stable-over-preview ranking still omits the preview finding; Hidden must name it; got:\n{rendered}"
        );
    }

    #[test]
    fn human_full_preserves_legacy_all_findings_output() {
        let mut first = sample_finding();
        first.id = "first".to_string();
        first.probe.location.line = 7;
        let mut second = sample_finding();
        second.id = "second".to_string();
        second.probe.location.line = 8;
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 2,
                findings: 2,
                weakly_exposed: 2,
                ..Summary::default()
            },
            findings: vec![first, second],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered =
            super::render_full_with_config(&output, &crate::config::RiprConfig::default());

        assert_eq!(rendered.matches("Changed\n").count(), 2);
        assert_eq!(rendered.matches("Probe\n").count(), 2);
        assert!(!rendered.contains("lower-priority finding(s) omitted"));
        assert!(!rendered.contains("Drill in:"));

        // #4379: the digest sends readers to human-full, so human-full must
        // carry every finding's drill-in pair, not lose them.
        let drill_in =
            crate::app::FindingDrillIn::Commands(crate::app::FindingNavigation::legacy());
        let navigated = super::render_full_with_config_and_navigation(
            &output,
            &crate::config::RiprConfig::default(),
            Some(&drill_in),
        );
        for id in ["first", "second"] {
            assert!(
                navigated.contains(&format!(
                    "Drill in:\n  ripr explain {id}\n  ripr context --at {id}\n"
                )),
                "human-full must carry the drill-in pair for {id}: {navigated}"
            );
        }
    }

    /// #4321: the exhaustive surface prints each finding's id once, so the
    /// digest's `--format human-full` route ends at a nameable finding — the
    /// same token `ripr explain`, `ripr context`, and the JSON `id` carry.
    #[test]
    fn human_full_prints_each_finding_id_exactly_once() {
        let mut first = sample_finding();
        first.id = "probe:src_lib.rs:predicate:c80557eb".to_string();
        first.probe.location.line = 7;
        let mut second = sample_finding();
        second.id = "probe:src_lib.rs:error_path:9af31c02".to_string();
        second.probe.location.line = 8;
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 2,
                findings: 2,
                weakly_exposed: 2,
                ..Summary::default()
            },
            findings: vec![first, second],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered =
            super::render_full_with_config(&output, &crate::config::RiprConfig::default());

        for id in [
            "probe:src_lib.rs:predicate:c80557eb",
            "probe:src_lib.rs:error_path:9af31c02",
        ] {
            assert_eq!(
                rendered.matches(&format!("  id: {id}\n")).count(),
                1,
                "each full-form finding block prints its id exactly once; got:\n{rendered}"
            );
        }
    }

    /// #4321: a `--worktree` run without `--write-artifact` has no artifact
    /// for drill-in commands to replay, so the digest states the artifact
    /// route and names the selected finding instead of dropping the block.
    #[test]
    fn worktree_digest_names_the_artifact_replay_route() {
        let finding = sample_finding();
        let finding_id = finding.id.clone();
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 1,
                findings: 1,
                weakly_exposed: 1,
                ..Summary::default()
            },
            findings: vec![finding],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let drill_in = crate::app::FindingDrillIn::WorktreeReplayNeedsArtifact;
        let rendered = super::render_bounded_with_config_and_navigation(
            &output,
            &crate::config::RiprConfig::default(),
            Some(&drill_in),
        );

        assert!(
            rendered.contains(&format!(
                "Next: this worktree run has no artifact to replay — rerun with --write-artifact, then `ripr explain --from <artifact> {finding_id}` drills into the top finding"
            )),
            "the digest must state the replay route and name the finding; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("Next: drill into the top finding:"),
            "commands replaying a different analysis must not print; got:\n{rendered}"
        );
    }

    /// #4321: the full form for the same run carries the replay route once
    /// instead of per-finding command blocks.
    #[test]
    fn worktree_human_full_names_the_artifact_replay_route_once() {
        let mut first = sample_finding();
        first.id = "first".to_string();
        first.probe.location.line = 7;
        let mut second = sample_finding();
        second.id = "second".to_string();
        second.probe.location.line = 8;
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 2,
                findings: 2,
                weakly_exposed: 2,
                ..Summary::default()
            },
            findings: vec![first, second],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let drill_in = crate::app::FindingDrillIn::WorktreeReplayNeedsArtifact;
        let rendered = super::render_full_with_config_and_navigation(
            &output,
            &crate::config::RiprConfig::default(),
            Some(&drill_in),
        );

        assert!(
            rendered.contains("Finding ids print above."),
            "the full form must state the replay route; got:\n{rendered}"
        );
        assert_eq!(
            rendered.matches("--write-artifact").count(),
            1,
            "the replay route prints once, not per finding; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("Drill in:"),
            "commands replaying a different analysis must not print; got:\n{rendered}"
        );
    }

    /// #4924 review: the full-form replay route must not point at ids it did
    /// not print — when policy suppresses every finding, no `id:` line exists
    /// above the note, so the route prints without the id pointer.
    #[test]
    fn worktree_full_omits_the_id_pointer_when_every_finding_is_suppressed() {
        use crate::output::suppressions::{CheckSuppressionOutcome, SuppressedCheckFinding};
        let finding = sample_finding();
        let finding_id = finding.id.clone();
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 1,
                findings: 1,
                ..Summary::default()
            },
            findings: vec![finding],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: Some(CheckSuppressionOutcome {
                policy_path: "policy/ripr-suppressions.toml".to_string(),
                suppressed: vec![SuppressedCheckFinding {
                    finding_id,
                    selector: "src/**".to_string(),
                }],
                warnings: Vec::new(),
            }),
            analysis_outcome: None,
            partial_scope: None,
        };

        let drill_in = crate::app::FindingDrillIn::WorktreeReplayNeedsArtifact;
        let rendered = super::render_full_with_config_and_navigation(
            &output,
            &crate::config::RiprConfig::default(),
            Some(&drill_in),
        );

        assert!(
            rendered.contains("Next: this worktree run has no artifact to replay"),
            "the replay route must still print under full suppression; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("Finding ids print above."),
            "the note must not claim ids it did not print; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("  id: "),
            "a fully suppressed run prints no finding blocks; got:\n{rendered}"
        );
    }

    #[test]
    fn render_lists_policy_suppressed_findings_compactly_with_warnings() {
        use crate::output::suppressions::{CheckSuppressionOutcome, SuppressedCheckFinding};
        let finding = sample_finding();
        let finding_id = finding.id.clone();
        let location = finding.probe.location.file.display().to_string();
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 1,
                findings: 1,
                ..Summary::default()
            },
            findings: vec![finding],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: Some(CheckSuppressionOutcome {
                policy_path: "policy/ripr-suppressions.toml".to_string(),
                suppressed: vec![SuppressedCheckFinding {
                    finding_id,
                    selector: "src/**".to_string(),
                }],
                warnings: vec![
                    "exposure_gap suppression for `missing/**` did not match any current finding"
                        .to_string(),
                ],
            }),
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("Suppressed by policy (policy/ripr-suppressions.toml): 1 finding(s)")
        );
        assert!(rendered.contains("(selector: src/**)"));
        assert!(rendered.contains("policy warning: exposure_gap suppression for `missing/**`"));
        // The suppressed finding must not also render as a detailed block.
        assert!(!rendered.contains(&format!("WARNING {location}:7")));
    }

    #[test]
    fn bounded_human_output_reports_no_actionable_gap_when_all_findings_suppressed() {
        use crate::output::suppressions::{CheckSuppressionOutcome, SuppressedCheckFinding};
        let finding = sample_finding();
        let finding_id = finding.id.clone();
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 1,
                findings: 1,
                weakly_exposed: 1,
                ..Summary::default()
            },
            findings: vec![finding],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: Some(CheckSuppressionOutcome {
                policy_path: "policy/ripr-suppressions.toml".to_string(),
                suppressed: vec![SuppressedCheckFinding {
                    finding_id,
                    selector: "src/**".to_string(),
                }],
                warnings: Vec::new(),
            }),
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("State: no gap selected for repair (no_actionable_gap)"));
        assert!(rendered.contains("all findings are suppressed by policy"));
        assert!(!rendered.contains("inspect the named static limitation"));
        assert!(!rendered.contains("Next: drill into the top finding:"));
    }

    #[test]
    fn human_output_discloses_limited_partial_scope_run_state() -> Result<(), String> {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: Vec::new(),
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: Some(crate::analysis::PartialDiffScope {
                run_status: crate::analysis::PartialDiffScope::RUN_STATUS.to_string(),
                diff_identity: "sha256:abc".to_string(),
                file_budget: 2,
                line_budget: 100,
                budget_disclosures: vec!["clamped budget disclosure".to_string()],
                selected_files: vec!["src/a.rs".to_string()],
                selected_changed_lines: 60,
                uninspected_files_lower_bound: 3,
                uninspected_changed_lines_lower_bound: 180,
                stop_reason: crate::analysis::PartialDiffStopReason::FileBudget,
                next_file_changed_lines: Some(60),
                partition_identity: "c".repeat(64),
            }),
        };

        for rendered in [
            super::render_bounded_with_config(&output, &crate::config::RiprConfig::default()),
            super::render_full_with_config(&output, &crate::config::RiprConfig::default()),
        ] {
            for needle in [
                "run state limited_partial_scope",
                "stop reason: file_budget",
                "selected: src/a.rs",
                "NOT inspected: at least 3 changed file(s) and at least 180 changed line(s)",
                "not eligible as a gate, baseline, badge, or RIPR Zero input",
                "RIPR_PARTIAL_DIFF_FILE_BUDGET",
                "clamped budget disclosure",
                &format!("partition_identity: {}", "c".repeat(64)),
            ] {
                if !rendered.contains(needle) {
                    return Err(format!(
                        "partial disclosure missing `{needle}` in:\n{rendered}"
                    ));
                }
            }
        }

        // Full-scope runs carry no partial disclosure.
        let full = CheckOutput {
            analysis_outcome: None,
            partial_scope: None,
            ..output
        };
        assert!(
            !super::render_bounded_with_config(&full, &crate::config::RiprConfig::default())
                .contains("limited_partial_scope")
        );
        Ok(())
    }

    /// A partial-scope report whose scope record has the shape the real
    /// selector produces for `stop_reason` (pinned against the selector by
    /// `analysis::language::rust::tests::partial_stop_reason_shapes_match_the_human_disclosure_fixtures`).
    fn partial_scope_output(
        stop_reason: crate::analysis::PartialDiffStopReason,
        uninspected_files: usize,
        uninspected_lines: usize,
    ) -> CheckOutput {
        use crate::analysis::PartialDiffStopReason;
        // file_budget 1 / line_budget 40: a file-budget stop selected one
        // 30-line file; a line-budget stop (file budget 7) selected 35 lines
        // and the next file would overshoot; a first-file stop selected one
        // 60-line file alone.
        let (file_budget, selected_changed_lines, next_file_changed_lines) = match stop_reason {
            PartialDiffStopReason::FileBudget => (1, 30, Some(30)),
            PartialDiffStopReason::LineBudget => (7, 35, Some(30)),
            PartialDiffStopReason::LineBudgetExceededOnFirstFile => (7, 60, None),
        };
        CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![sample_finding()],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: Some(crate::analysis::PartialDiffScope {
                run_status: crate::analysis::PartialDiffScope::RUN_STATUS.to_string(),
                diff_identity: "sha256:abc".to_string(),
                file_budget,
                line_budget: 40,
                budget_disclosures: Vec::new(),
                selected_files: vec!["src/a.rs".to_string()],
                selected_changed_lines,
                uninspected_files_lower_bound: uninspected_files,
                uninspected_changed_lines_lower_bound: uninspected_lines,
                stop_reason,
                next_file_changed_lines,
                partition_identity: "c".repeat(64),
            }),
        }
    }

    #[test]
    fn partial_scope_names_stopping_budget_size_finding_count_and_override() -> Result<(), String> {
        use crate::analysis::PartialDiffStopReason;
        let cases = [
            (
                PartialDiffStopReason::FileBudget,
                3,
                90,
                vec![
                    "analysis stopped at the file budget of 1 changed file(s) \
                     (RIPR_PARTIAL_DIFF_FILE_BUDGET=1)",
                    "Found 1 finding(s) before stopping.",
                    "NOT inspected: at least 3 changed file(s) and at least 90 changed line(s); \
                     more findings may exist beyond the budget.",
                    "raise RIPR_PARTIAL_DIFF_FILE_BUDGET to at least 2 and \
                     RIPR_PARTIAL_DIFF_LINE_BUDGET to at least 60, then re-run.",
                ],
            ),
            (
                PartialDiffStopReason::LineBudget,
                2,
                30,
                vec![
                    "analysis stopped at the line budget of 40 changed line(s) \
                     (RIPR_PARTIAL_DIFF_LINE_BUDGET=40)",
                    "Found 1 finding(s) before stopping.",
                    "more findings may exist beyond the budget.",
                    "raise RIPR_PARTIAL_DIFF_LINE_BUDGET to at least 65, then re-run.",
                ],
            ),
            (
                PartialDiffStopReason::LineBudgetExceededOnFirstFile,
                0,
                0,
                vec![
                    "analysis stopped at the line budget of 40 changed line(s) \
                     (RIPR_PARTIAL_DIFF_LINE_BUDGET=40); the first selected file alone exceeded \
                     it and was analyzed whole",
                    "Found 1 finding(s) before stopping.",
                    "Every changed file ripr's language adapters read was selected",
                    "this result stays partial and is not a complete-scope claim",
                    "raise RIPR_PARTIAL_DIFF_LINE_BUDGET to at least 60, then re-run.",
                ],
            ),
        ];
        for (stop_reason, files, lines, needles) in cases {
            let output = partial_scope_output(stop_reason, files, lines);
            for rendered in [
                super::render_bounded_with_config(&output, &crate::config::RiprConfig::default()),
                super::render_full_with_config(&output, &crate::config::RiprConfig::default()),
            ] {
                for needle in &needles {
                    if !rendered.contains(needle) {
                        return Err(format!(
                            "{stop_reason:?}: partial disclosure missing `{needle}` in:\n{rendered}"
                        ));
                    }
                }
                if rendered.contains("at least 0") {
                    return Err(format!(
                        "{stop_reason:?}: `at least 0` printed as a count in:\n{rendered}"
                    ));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn human_output_escapes_control_bytes_in_selected_paths() -> Result<(), String> {
        // A crafted diff filename with control bytes must not reach the
        // terminal verbatim (#2142 review): the display is escaped while the
        // raw path stays on the scope record.
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: Vec::new(),
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: Some(crate::analysis::PartialDiffScope {
                run_status: crate::analysis::PartialDiffScope::RUN_STATUS.to_string(),
                diff_identity: "sha256:abc".to_string(),
                file_budget: 2,
                line_budget: 100,
                budget_disclosures: Vec::new(),
                selected_files: vec!["src/evil\u{1b}[2K.rs".to_string()],
                selected_changed_lines: 1,
                uninspected_files_lower_bound: 1,
                uninspected_changed_lines_lower_bound: 1,
                stop_reason: crate::analysis::PartialDiffStopReason::FileBudget,
                next_file_changed_lines: Some(60),
                partition_identity: "c".repeat(64),
            }),
        };

        let rendered =
            super::render_bounded_with_config(&output, &crate::config::RiprConfig::default());
        if rendered.contains('\u{1b}') {
            return Err("raw ESC byte reached the terminal display".to_string());
        }
        if !rendered.contains("selected: src/evil\\u{1b}[2K.rs") {
            return Err(format!("escaped path missing in:\n{rendered}"));
        }
        Ok(())
    }

    #[test]
    fn render_finding_includes_ripr_evidence_related_tests_gap_and_next_step() {
        let finding = sample_finding();
        let location = finding.probe.location.file.display().to_string();
        let related_path = finding.related_tests[0].file.display().to_string();

        let rendered = render_finding(&finding);

        assert!(rendered.contains(&format!("WARNING {location}:7")));
        assert!(rendered.contains("Changed\n"));
        assert!(rendered.contains("before: if enabled"));
        assert!(rendered.contains("after:  if disabled"));
        assert!(rendered.contains("Probe\n"));
        assert!(rendered.contains("family: predicate"));
        assert!(rendered.contains("Static exposure\n"));
        assert!(rendered.contains("weakly_exposed (warning, confidence 0.70)"));
        assert!(rendered.contains("Evidence\n"));
        assert!(rendered.contains("reach yes: reaches test"));
        assert!(rendered.contains("infection weak: weak mutation"));
        assert!(rendered.contains("propagation unknown: propagation unclear"));
        assert!(rendered.contains("observation yes: observed"));
        assert!(rendered.contains("discriminator no: no discriminator"));
        assert!(rendered.contains("local flow reaches returned value: disabled_result (line 8)"));
        assert!(rendered.contains(&format!(
            "{related_path}:22 test_handles_disabled uses strong exact value oracle: assert_eq!(actual, expected)"
        )));
        assert!(rendered.contains("observed function argument value enabled = false at line 22"));
        assert!(rendered.contains("Weakness\n"));
        assert!(rendered.contains("missing strong oracle"));
        assert!(rendered.contains(
            "missing discriminator enabled == false: related tests do not use the changed value"
        ));
        assert!(rendered.contains("Next step\n"));
        assert!(rendered.contains("Add assertion for disabled path result."));
    }

    /// #4320: the 5-related-test window must disclose the total — the number
    /// of reaching tests is core exposure evidence, and an unmarked cap reads
    /// as the whole evidence.
    #[test]
    fn evidence_window_discloses_related_tests_cap() {
        let mut finding = sample_finding();
        for index in 1..9 {
            finding.related_tests.push(RelatedTest {
                name: format!("test_extra_{index}"),
                file: PathBuf::from("tests/sample.rs"),
                line: 100 + index,
                oracle: None,
                oracle_kind: OracleKind::SmokeOnly,
                oracle_strength: OracleStrength::Weak,
                relation_reason: None,
                relation_confidence: None,
            });
        }
        assert_eq!(finding.related_tests.len(), 9);

        let rendered = render_finding(&finding);

        assert_eq!(
            rendered.matches("related test tests/sample.rs:").count(),
            5,
            "only the windowed related tests render:\n{rendered}"
        );
        assert!(
            rendered.contains("related tests (showing 5 of 9; more in --format json)"),
            "expected the related-tests window disclosure; got:\n{rendered}"
        );
    }

    /// #4320: the 8-observed-value window must disclose the total — observed
    /// values are the raw material for writing the missing-discriminator test,
    /// and an unmarked cap can hide the one boundary value the reader needs.
    #[test]
    fn evidence_window_discloses_observed_values_cap() {
        let mut finding = sample_finding();
        // sample_finding already carries one observed value; 13 more make 14.
        for index in 0..13 {
            finding.activation.observed_values.push(ValueFact {
                line: 200 + index,
                text: format!("sample({index})"),
                value: format!("arg{index} = {index}"),
                context: ValueContext::FunctionArgument,
            });
        }
        assert_eq!(finding.activation.observed_values.len(), 14);

        let rendered = render_finding(&finding);

        assert_eq!(
            rendered
                .matches("observed function argument value ")
                .count(),
            8,
            "only the windowed observed values render:\n{rendered}"
        );
        assert!(
            rendered.contains("observed values (showing 8 of 14; full list in --format json)"),
            "expected the observed-values window disclosure; got:\n{rendered}"
        );
    }

    /// #4320 review: beyond JSON's own ranked 32-value cap, both formats are
    /// windows — the human pointer must name JSON's cap instead of promising
    /// a full list no surface carries.
    #[test]
    fn evidence_window_observed_values_pointer_names_json_cap_beyond_it() {
        let mut finding = sample_finding();
        // sample_finding already carries one observed value; 41 more make 42
        // (over the 8-value human window and the 32-value ranked JSON cap).
        for index in 0..41 {
            finding.activation.observed_values.push(ValueFact {
                line: 300 + index,
                text: format!("sample({index})"),
                value: format!("arg{index} = {index}"),
                context: ValueContext::AssertionArgument,
            });
        }
        assert_eq!(finding.activation.observed_values.len(), 42);

        let rendered = render_finding(&finding);

        assert!(
            rendered.contains("observed values (showing 8 of 42; --format json keeps a ranked 32)"),
            "the pointer must disclose JSON's ranked cap; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("full list in --format json"),
            "no surface carries the full list at this size:\n{rendered}"
        );
    }

    /// #4320: the digest shows only the first related test; the line must
    /// carry the total so the reader knows how much reaching-test evidence
    /// exists. A single related test keeps the unmarked form.
    #[test]
    fn digest_related_test_line_carries_the_total() {
        let mut finding = sample_finding();
        finding.related_tests.push(RelatedTest {
            name: "test_second_observer".to_string(),
            file: PathBuf::from("tests/other.rs"),
            line: 9,
            oracle: None,
            oracle_kind: OracleKind::SmokeOnly,
            oracle_strength: OracleStrength::Weak,
            relation_reason: None,
            relation_confidence: None,
        });

        let digest = super::sections::render_finding_digest_with_config(
            &finding,
            &crate::config::RiprConfig::default(),
        );

        assert!(
            digest.contains("  Related test (1 of 2): tests/sample.rs:22 test_handles_disabled\n"),
            "expected the digest related-test total; got:\n{digest}"
        );

        let single = sample_finding();
        let digest = super::sections::render_finding_digest_with_config(
            &single,
            &crate::config::RiprConfig::default(),
        );
        assert!(
            digest.contains("  Related test: tests/sample.rs:22 test_handles_disabled\n"),
            "a single related test keeps the unmarked form; got:\n{digest}"
        );
        assert!(!digest.contains("Related test (1 of"), "{digest}");
    }

    /// #4320: the digest missing-discriminator line discloses its window into
    /// the full Weakness set (acceptance wording: `Missing discriminator (1 of 3)`).
    #[test]
    fn digest_missing_discriminator_discloses_one_of_n_window() {
        let mut finding = sample_finding();
        finding.class = ExposureClass::WeaklyExposed;
        finding.missing = vec![
            format!("{MISSING_DISCRIMINATOR_VALUE_PREFIX}amount >= threshold"),
            "no strong oracle observes the boundary".to_string(),
            "no related test constructs the boundary input".to_string(),
        ];
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "amount >= threshold".to_string(),
            reason: "no related test call uses an amount at the threshold".to_string(),
            flow_sink: None,
        }];

        let digest = super::sections::render_finding_digest_with_config(
            &finding,
            &crate::config::RiprConfig::default(),
        );

        assert!(
            digest.contains("  Missing discriminator (1 of 3): amount >= threshold\n"),
            "expected the 1-of-3 window disclosure; got:\n{digest}"
        );
    }

    /// #2752: the `Changed` block rendered `before`/`after`/`expr` at full source
    /// width, so one long expression printed 400+ characters on a single line.
    ///
    /// It is bounded by *wrapping*, not truncation, and review on the fix
    /// established why: `--format json` serializes only `probe.expression`, so
    /// truncating here would leave a long `before` in no ripr output at all.
    #[test]
    fn render_finding_wraps_long_changed_expressions_without_losing_them() {
        let long_expr = format!(
            "if {}true",
            "decisions.iter().filter(|d| d.decision == \"blocking\").count() == 0 && ".repeat(6)
        );
        assert!(
            long_expr.chars().count() > 180,
            "fixture must exceed the display budget to exercise wrapping, got {}",
            long_expr.chars().count()
        );
        let mut finding = sample_finding();
        finding.probe.before = Some(long_expr.clone());
        finding.probe.after = Some(long_expr.clone());

        let rendered = render_finding(&finding);

        // Every line stays inside the budget...
        for line in rendered.lines() {
            let width = line.chars().count();
            assert!(
                width <= 180,
                "rendered line exceeds the display budget at {width} chars: {line}"
            );
        }
        // ...and no character of the expression is lost. Reassembling the
        // `before` block by stripping the label/indent column must reproduce it
        // exactly.
        // `  before: ` and the continuation indent are both exactly 10 columns.
        const LABEL_WIDTH: usize = 10;
        let indent = " ".repeat(LABEL_WIDTH);
        let reassembled: String = rendered
            .lines()
            .skip_while(|line| !line.starts_with("  before: "))
            .take_while(|line| line.starts_with("  before: ") || line.starts_with(&indent))
            .map(|line| line.get(LABEL_WIDTH..).unwrap_or_default())
            .collect();
        assert_eq!(
            reassembled, long_expr,
            "the wrapped before-block must reproduce the complete expression"
        );
        assert!(
            !rendered.contains('…'),
            "the exhaustive surface must not elide the expression"
        );
    }

    /// A whitespace-only change is a real behavior change when it is inside a
    /// string literal. Collapsing whitespace would render both sides
    /// identically and hide exactly the delta the finding is about.
    #[test]
    fn render_finding_preserves_whitespace_that_distinguishes_before_and_after() {
        let mut finding = sample_finding();
        finding.probe.before = Some("assert_eq!(msg, \"a  b\")".to_string());
        finding.probe.after = Some("assert_eq!(msg, \"a b\")".to_string());

        let rendered = render_finding(&finding);

        assert!(rendered.contains("  before: assert_eq!(msg, \"a  b\")"));
        assert!(rendered.contains("  after:  assert_eq!(msg, \"a b\")"));
    }

    /// The `expr:` fallback is the same rendered fragment reached through a
    /// different branch, so it gets the same treatment: wrapped, complete, and
    /// not whitespace-normalized.
    #[test]
    fn render_finding_wraps_the_expr_fallback() {
        let mut finding = sample_finding();
        finding.probe.before = None;
        finding.probe.after = None;
        finding.probe.expression = format!("enabled && {}", "other.flag() && ".repeat(20));

        let rendered = render_finding(&finding);

        assert!(rendered.contains("  expr:   enabled && other.flag()"));
        for line in rendered.lines() {
            assert!(line.chars().count() <= 180, "line too wide: {line}");
        }
        let expr_lines = rendered
            .lines()
            .filter(|line| line.starts_with("  expr:   ") || line.starts_with("           "))
            .count();
        assert!(
            expr_lines > 1,
            "a long expression should wrap across continuation lines"
        );
    }

    #[test]
    fn render_finding_uses_expr_and_fallback_evidence_when_no_before_after() {
        let mut finding = sample_finding();
        finding.probe.before = None;
        finding.probe.after = None;
        finding.flow_sinks.clear();
        finding.related_tests.clear();
        finding.activation.observed_values.clear();
        finding.evidence = vec!["fallback evidence line".to_string()];

        let rendered = render_finding(&finding);

        assert!(rendered.contains("expr:   enabled"));
        assert!(rendered.contains("  - fallback evidence line"));
    }

    #[test]
    fn render_finding_deduplicates_missing_discriminator_value_line() {
        let mut finding = sample_finding();
        finding.missing = vec![
            "Missing discriminator value: enabled == false".to_string(),
            "another gap".to_string(),
        ];

        let rendered = render_finding(&finding);

        assert_eq!(
            rendered
                .matches("missing discriminator enabled == false")
                .count(),
            1
        );
        assert!(rendered.contains("  - another gap"));
    }

    #[test]
    fn render_finding_includes_language_metadata_when_present() {
        let mut finding = sample_finding();
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);

        let rendered = render_finding(&finding);

        assert!(rendered.contains("Language\n"));
        assert!(rendered.contains("  language: typescript\n"));
        assert!(rendered.contains("  status: preview\n"));
    }

    #[test]
    fn render_finding_includes_preview_actionability_without_raw_string_spam() {
        let mut finding = unknown_finding();
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.owner_kind = Some(crate::domain::OwnerKind::Function);
        finding.evidence = vec![
            "owner: discountedTotal".to_string(),
            "gap_state: advisory".to_string(),
            "actionability_category: incomplete_repair_packet".to_string(),
            "why_not_actionable: TypeScript preview lacks a complete repair packet contract"
                .to_string(),
            "repair_route: project canonical TypeScript repair packet fields later".to_string(),
            "missing_actionability_fields: canonical_gap_id, verify_command".to_string(),
            "evidence_needed_to_promote: canonical gap identity and verify command".to_string(),
            "raw_evidence_ref: file=src/lib.ts;line=2;kind=typescript_preview_probe;source_id=probe:src_lib.ts:2:typescript_preview;owner=discountedTotal".to_string(),
        ];
        finding.missing = vec![
            "TypeScript preview actionability `advisory` / `incomplete_repair_packet`: duplicate summary".to_string(),
        ];

        let rendered = render_finding(&finding);

        assert!(rendered.contains("Preview actionability\n"));
        assert!(rendered.contains("  authority: preview_advisory_only\n"));
        assert!(rendered.contains("  gap state: advisory\n"));
        assert!(rendered.contains("  category: incomplete_repair_packet\n"));
        assert!(rendered.contains("  repair packet ready: false\n"));
        assert!(rendered.contains("  raw evidence: src/lib.ts:2 (typescript_preview_probe)"));
        assert!(rendered.contains("  - owner: discountedTotal\n"));
        assert!(!rendered.contains("  - gap_state: advisory\n"));
        assert!(!rendered.contains("duplicate summary"));
    }

    #[test]
    fn render_finding_includes_bun_cross_language_grip() {
        let mut finding = unknown_finding();
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.owner_kind = Some(crate::domain::OwnerKind::Function);
        finding.evidence = vec![
            "owner: Blob::from_js_without_defer_gc".to_string(),
            "gap_state: static_limitation".to_string(),
            "actionability_category: cross_language_oracle_visibility_unresolved".to_string(),
            "why_not_actionable: TypeScript cross-language preview is a named limitation until the external oracle path is visible".to_string(),
            "repair_route: analysis/cross-language-oracle-visibility".to_string(),
            "missing_graph_legs: boundary_discriminator:resizable_array_buffer".to_string(),
            "unlock_condition: add or inspect the missing external TypeScript discriminator(s) in test/js/web/fetch/blob.test.ts and keep repair-packet projection blocked until verify, receipt, and edit-surface evidence exists".to_string(),
            "evidence_needed_to_promote: bridge calibration and non-preview repair packet contract"
                .to_string(),
            "raw_evidence_ref: leg=rust_seam;file=src/jsc/Blob.rs;line=42;kind=rust_boundary;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=array_buffer.shared || array_buffer.resizable".to_string(),
            "typescript_bun_ub_bridge_hint: confidence=configured_hint rust_file=src/jsc/Blob.rs rust_owner=Blob::from_js_without_defer_gc rust_boundary=\"array_buffer.shared || array_buffer.resizable\" ts_test_file=test/js/web/fetch/blob.test.ts".to_string(),
            "typescript_bun_ub_bridge_verdict: ts_missing_resizable missing_discriminators=resizable_array_buffer action=route_cross_language_oracle_visibility_limitation suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_missing_discriminator rust_grip=ungripped ts_verdict=ts_missing_resizable action=route_cross_language_oracle_visibility_limitation authority=preview_advisory_only suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_test_placement: rank=1 suggested_test_file=test/js/web/fetch/blob.test.ts reason=\"existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable ArrayBuffer\" basis=configured_bridge_suggested_test_file,same_js_surface,same_boundary_vocabulary authority=preview_advisory_only repair_packet_ready=false".to_string(),
            "typescript_bun_ub_bridge_hint: confidence=configured_hint rust_file=src/jsc/array_buffer.rs rust_owner=copy_to_unshared rust_boundary=\"array_buffer.shared || array_buffer.resizable\" ts_test_file=test/js/web/fetch/blob.test.ts".to_string(),
            "typescript_bun_ub_bridge_verdict: ts_missing_shared missing_discriminators=shared_array_buffer action=route_cross_language_oracle_visibility_limitation suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_missing_discriminator rust_grip=ungripped ts_verdict=ts_missing_shared action=route_cross_language_oracle_visibility_limitation authority=preview_advisory_only suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
        ];

        let rendered = render_finding(&finding);

        assert!(rendered.contains("  Bun cross-language grip 1/2:\n"));
        assert!(rendered.contains("    state: rust_ungripped_ts_missing_discriminator\n"));
        assert!(rendered.contains("  Bun cross-language grip 2/2:\n"));
        assert!(rendered.contains("    Rust seam: src/jsc/array_buffer.rs owner=copy_to_unshared"));
        assert!(rendered.contains(
            "    Rust seam: src/jsc/Blob.rs owner=Blob::from_js_without_defer_gc boundary=array_buffer.shared || array_buffer.resizable\n"
        ));
        assert!(rendered.contains(
            "    TypeScript evidence: test/js/web/fetch/blob.test.ts verdict=ts_missing_resizable confidence=configured_hint\n"
        ));
        assert!(rendered.contains("    missing discriminators: resizable_array_buffer\n"));
        assert!(
            rendered.contains(
                "    missing graph legs: boundary_discriminator:resizable_array_buffer\n"
            )
        );
        assert!(rendered.contains(
            "    unlock condition: add or inspect the missing external TypeScript discriminator(s) in test/js/web/fetch/blob.test.ts and keep repair-packet projection blocked until verify, receipt, and edit-surface evidence exists\n"
        ));
        assert!(
            rendered
                .contains("    limitation category: cross_language_oracle_visibility_unresolved\n")
        );
        assert!(rendered.contains("    repair route: analysis/cross-language-oracle-visibility\n"));
        assert!(
            rendered.contains("    action: route_cross_language_oracle_visibility_limitation\n")
        );
        assert!(rendered.contains("    suggested test file: test/js/web/fetch/blob.test.ts\n"));
        assert!(rendered.contains("    placement: rank 1 test/js/web/fetch/blob.test.ts\n"));
        assert!(rendered.contains(
            "    placement reason: existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable ArrayBuffer\n"
        ));
        assert_eq!(
            rendered
                .matches("    placement: rank 1 test/js/web/fetch/blob.test.ts\n")
                .count(),
            1,
            "only the Blob profile may receive placement evidence"
        );
        let copy_profile = rendered
            .split("  Bun cross-language grip 2/2:\n")
            .nth(1)
            .unwrap_or_default();
        assert!(!copy_profile.is_empty(), "expected copy profile rendering");
        assert!(
            !copy_profile.contains("    placement:"),
            "copy_to_unshared must not receive placement evidence"
        );
        assert!(rendered.contains("    proof mode: observable_red_green\n"));
        assert!(rendered.contains(
            "    proof mode reason: The missing TypeScript discriminator belongs in an existing bridged stable-byte observer route; future proof should be a system-Bun red/patched-green witness after the discriminator is added.\n"
        ));
        assert!(rendered.contains(
            "    proof execution: runtime=false mutation=false miri=false proof_claim=false\n"
        ));
        assert!(rendered.contains("    advisory packet:\n"));
        assert!(rendered.contains("      version: bun_cross_language_advisory_packet.v1\n"));
        assert!(
            rendered
                .contains("      next action: add_typescript_discriminator_in_suggested_file\n")
        );
        assert!(rendered.contains("      ts test file: test/js/web/fetch/blob.test.ts\n"));
        assert!(rendered.contains(
            "      suggested shape: Add new ArrayBuffer(..., { maxByteLength: ... }) through Blob/view with a stable-byte byte/text/value assertion.\n"
        ));
        assert!(rendered.contains(
            "      stop condition: Stop if placement evidence disappears or the stable-byte assertion requires production-code, public API, or test-framework changes.\n"
        ));
        assert!(rendered.contains(
            "      must not change: Rust production behavior, public API, test framework shape, generated tests, runtime Bun/TypeScript execution, public repair-packet authority\n"
        ));
        assert!(rendered.contains("      public repair packet: false\n"));
        assert!(rendered.contains("      repair packet ready: false\n"));
        assert!(rendered.contains("    authority: preview_advisory_only\n"));
        assert!(rendered.contains("    repair packet ready: false\n"));
    }

    #[test]
    fn render_finding_includes_perl_preview_card_as_advisory_human_surface() {
        let mut finding = unknown_finding();
        add_perl_preview_card_inputs(&mut finding);

        let rendered = render_finding(&finding);

        assert!(rendered.contains("Perl preview card (advisory)\n"));
        assert!(rendered.contains("  card version: perl_preview_card.v1\n"));
        assert!(rendered.contains("  authority: preview_advisory_only (perl/preview)\n"));
        assert!(
            rendered
                .contains("  surface scope: check_json_human_sarif_github_gap_ledger_markdown\n")
        );
        assert!(rendered.contains("  public projection ready: true\n"));
        assert!(rendered.contains("  public repair packet: false\n"));
        assert!(rendered.contains("  repair packet ready: false\n"));
        assert!(rendered.contains("  agent packet ready: false\n"));
        assert!(rendered.contains("  gate candidate: false\n"));
        assert!(rendered.contains("  badge candidate: false\n"));
        assert!(rendered.contains("  RIPR Zero candidate: false\n"));
        assert!(rendered.contains("  packet id: perl-preview:gap-return\n"));
        assert!(rendered.contains(
            "  canonical gap: gap:perl:lib/My/App.pm:My::App::discount:return_value:exact_return_assertion:return_value\n"
        ));
        assert!(rendered.contains("  changed owner: perl:lib/My/App.pm::My::App::discount\n"));
        assert!(rendered.contains("  repair route: add_exact_return_assertion\n"));
        assert!(rendered.contains("  missing discriminator: return_value\n"));
        assert!(rendered.contains("  target test shape: Test::More exact_return_assertion\n"));
        assert!(rendered.contains("  suggested location: t/app.t::discount_smoke\n"));
        assert!(
            rendered.contains(
                "  suggested assertion: assert the exact returned `return_value` value\n"
            )
        );
        assert!(rendered.contains("  verify: prove t/app.t (preview_fact_only_not_delegated)\n"));
        assert!(rendered.contains("  receipt: preview_available_not_delegated\n"));
        assert!(rendered.contains("  raw evidence: perl_change lib/My/App.pm:8 (perl_change)"));
        assert!(rendered.contains("  stop if:\n"));
        assert!(rendered.contains("    - perl-lsp packet status changes\n"));
        assert!(rendered.contains("  must not change:\n"));
        assert!(rendered.contains("    - do not edit Perl production code\n"));
        assert!(!rendered.contains("ripr agent receipt --root"));
        assert!(!rendered.contains("perl_allowed_edit_boundary"));
        assert!(!rendered.contains("perl_forbidden_edit_boundary"));
        assert!(!rendered.contains("allowed edit"));
        assert!(!rendered.contains("forbidden edit"));
        assert!(!rendered.contains("perl_internal_agent_packet"));
        assert!(!rendered.contains("perl_repair_card"));
    }

    #[test]
    fn render_finding_omits_language_metadata_when_absent() {
        let rendered = render_finding(&sample_finding());

        assert!(!rendered.contains("Language\n"));
        assert!(!rendered.contains("language:"));
        assert!(!rendered.contains("status:"));
    }

    #[test]
    fn render_finding_omits_rust_default_language_metadata() {
        let mut finding = sample_finding();
        finding.language = Some(LanguageId::Rust);
        finding.language_status = Some(LanguageStatus::Stable);

        let rendered = render_finding(&finding);

        assert!(!rendered.contains("Language\n"));
        assert!(!rendered.contains("language: rust"));
        assert!(!rendered.contains("status: stable"));
    }

    #[test]
    fn render_finding_includes_probe_owner_when_present() {
        let mut finding = sample_finding();
        finding.probe.owner = Some(SymbolId("python:src/pricing.py::discount".to_string()));

        let rendered = render_finding(&finding);

        assert!(rendered.contains("  owner:  python:src/pricing.py::discount\n"));
    }

    #[test]
    fn render_finding_includes_canonical_gap_when_present() {
        let mut finding = sample_finding();
        finding.canonical_gap = Some(FindingCanonicalGap {
            id: "gap:python:src/pricing.py:discount:predicate_boundary:predicate:amount>=threshold"
                .to_string(),
            language: "python".to_string(),
            file: "src/pricing.py".to_string(),
            owner: "discount".to_string(),
            behavior_kind: "predicate_boundary".to_string(),
            probe_kind: "predicate".to_string(),
            normalized_discriminator: "amount>=threshold".to_string(),
        });

        let rendered = render_finding(&finding);

        assert!(rendered.contains(
            "  canonical gap: gap:python:src/pricing.py:discount:predicate_boundary:predicate:amount>=threshold\n"
        ));
    }

    #[test]
    fn human_output_includes_effective_stop_reasons_for_unknowns() {
        let output = render_finding(&unknown_finding());

        assert!(output.contains("Stop reasons:"));
        assert!(output.contains("  - static_probe_unknown"));
    }

    /// #4323: a stop reason renders its gloss beside the token, the way the
    /// `Static limitation` section already does, so the reader learns why.
    #[test]
    fn human_output_glosses_stop_reasons() {
        let output = render_finding(&unknown_finding());

        assert!(
            output.contains(
                "  - static_probe_unknown \u{2014} ripr could not model this change well enough to classify it\n"
            ),
            "{output}"
        );
    }

    /// #4323 review: guidance just under the budget must count the label, so
    /// the rendered line never runs past 180 characters.
    #[test]
    fn digest_next_step_line_counts_its_label_against_the_budget() {
        let mut finding = sample_finding();
        let guidance = format!("{} remedy.", "word ".repeat(34));
        assert!(
            (170..=180).contains(&guidance.chars().count()),
            "{guidance}"
        );
        finding.recommended_next_step = Some(guidance);

        let digest = super::sections::render_finding_digest_with_config(
            &finding,
            &crate::config::RiprConfig::default(),
        );

        assert!(
            digest.lines().all(|line| line.chars().count() <= 180),
            "no digest line exceeds the budget; got:\n{digest}"
        );
        assert!(digest.contains("remedy."), "{digest}");
    }

    /// #4323: the default no-path guidance is ~330 characters and ends with
    /// the remedy. The digest used to cut it at 180 characters, so the
    /// imperative ("add a co-located test ...") never rendered.
    #[test]
    fn digest_next_step_keeps_the_remedy_clause() {
        let mut finding = sample_finding();
        finding.class = ExposureClass::NoStaticPath;
        finding.recommended_next_step = Some(crate::domain::NO_STATIC_PATH_NEXT_STEP.to_string());

        let digest = super::sections::render_finding_digest_with_config(
            &finding,
            &crate::config::RiprConfig::default(),
        );
        // The field is its first line plus the four-space continuation lines.
        let mut lines = digest
            .lines()
            .skip_while(|line| !line.starts_with("  Next step: "));
        let mut next_step = lines
            .next()
            .map(|line| line.trim_start_matches("  Next step: ").to_string())
            .unwrap_or_default();
        for line in lines.take_while(|line| line.starts_with("    ")) {
            next_step.push(' ');
            next_step.push_str(line.trim());
        }
        let flattened = next_step.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(
            flattened.ends_with(
                "add a co-located test that reaches and observes the changed behavior so a discriminator exists."
            ),
            "digest must keep the remedy; got:\n{digest}"
        );
        assert!(
            !next_step.contains('\u{2026}'),
            "digest must not truncate; got:\n{digest}"
        );
        assert!(
            digest.lines().all(|line| line.chars().count() <= 180),
            "wrapped lines stay within the display budget; got:\n{digest}"
        );
    }

    /// #4324: evidence ordering is pipeline-ordered, so a positional 2-line
    /// window hid propagation/observation/discriminator behind a bare count.
    /// The digest names all five stage states compactly — every stage always
    /// has a line, so no stage is ever silently dropped — and keeps the
    /// 2-line detail window with an honest remainder disclosure that names
    /// the recovery format.
    #[test]
    fn digest_names_all_five_stage_states_and_keeps_the_remainder_disclosure() {
        let finding = sample_finding();

        let digest = super::sections::render_finding_digest_with_config(
            &finding,
            &crate::config::RiprConfig::default(),
        );

        assert!(
            digest.contains(
                "  Evidence: reach yes · infection weak · propagation unknown · observation yes · discriminator no\n"
            ),
            "digest must name all five stage states; got:\n{digest}"
        );
        // sample_finding carries flow-sink, related-test and observed-value
        // lines beyond the five stage lines, so the window cannot show
        // everything and the remainder disclosure must fire.
        assert!(
            digest.contains("more detail line(s) in --format human-full"),
            "digest must keep an honest remainder disclosure; got:\n{digest}"
        );
        assert!(
            digest.lines().all(|line| line.chars().count() <= 180),
            "the compact stage line stays within the display budget; got:\n{digest}"
        );
    }

    /// #4324 review: the `discriminate` stage grades the strongest related
    /// oracle, so a non-`exposed` finding can carry a `yes` grade while the
    /// digest simultaneously names the missing discriminating input. The
    /// compact token keeps the full evidence line's semantic and must not
    /// read `discriminator yes` in that case.
    #[test]
    fn digest_compact_discriminator_token_mirrors_the_full_evidence_line() {
        let mut finding = sample_finding();
        finding.ripr.reveal.discriminate =
            stage(StageState::Yes, Confidence::High, "strong oracle grade");
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "amount == discount_threshold".to_string(),
            reason: "no related test call uses the boundary value".to_string(),
            flow_sink: None,
        }];

        let digest = super::sections::render_finding_digest_with_config(
            &finding,
            &crate::config::RiprConfig::default(),
        );

        assert!(
            digest.contains(
                "  Evidence: reach yes · infection weak · propagation unknown · observation yes · discriminator missing\n"
            ),
            "a yes oracle grade on a non-exposed finding must not read as a present discriminator; got:\n{digest}"
        );
        assert!(
            !digest.contains("discriminator yes"),
            "the compact line must not contradict the missing-discriminator wording; got:\n{digest}"
        );

        finding.activation.missing_discriminators = Vec::new();
        let digest = super::sections::render_finding_digest_with_config(
            &finding,
            &crate::config::RiprConfig::default(),
        );
        assert!(
            digest.contains("· discriminator not established\n"),
            "without a named missing discriminator the token keeps the full line's wording; got:\n{digest}"
        );
    }

    // RIPR-SPEC-0115: a transitive-reach witness line in `evidence` (recognized
    // by the shared prefix) renders as a concrete "Where to look" pointer.
    #[test]
    fn human_output_surfaces_transitive_reach_witness_as_where_to_look() {
        let mut finding = sample_finding();
        finding.evidence.push(
            "For example, the test `test_uses_outer` (tests/it.rs:12) calls `outer`, an entry \
             point that may lead here. Inspect it to judge whether this change is observed."
                .to_string(),
        );
        let output = render_finding(&finding);
        assert!(output.contains("Where to look\n"));
        assert!(output.contains("the test `test_uses_outer` (tests/it.rs:12) calls `outer`"));
        assert!(output.contains("may lead here"));
    }

    #[test]
    fn human_output_surfaces_static_limitation_detail() {
        let mut finding = sample_finding();
        finding.evidence.extend([
            "limitation_last_established_edge: test `test_uses_outer` (tests/it.rs:12) -> entry `outer`".to_string(),
            "limitation_first_unresolved_edge: entry `outer` -> owner `inner` through a transitive Rust helper path".to_string(),
            "limitation_analyzer_route: analysis/rust-public-api-transitive-reach".to_string(),
            "limitation_non_claim: named limitation only; ripr cannot confirm or deny that this path observes the change".to_string(),
        ]);

        let output = render_finding(&finding);

        assert!(output.contains("Limitation detail\n"));
        assert!(output.contains(
            "  last established edge: test `test_uses_outer` (tests/it.rs:12) -> entry `outer`\n"
        ));
        assert!(output.contains(
            "  first unresolved edge: entry `outer` -> owner `inner` through a transitive Rust helper path\n"
        ));
        assert!(output.contains("  analyzer route: analysis/rust-public-api-transitive-reach\n"));
        assert!(output.contains(
            "  non-claim: named limitation only; ripr cannot confirm or deny that this path observes the change\n"
        ));
    }

    // No witness line -> no "Where to look" section (fail-closed: only render
    // when the limitation actually named a witness).
    #[test]
    fn human_output_omits_where_to_look_without_witness() {
        let output = render_finding(&sample_finding());
        assert!(!output.contains("Where to look"));
    }

    fn add_perl_preview_card_inputs(finding: &mut Finding) {
        finding.id = "probe:lib_My_App_pm:8:perl_return".to_string();
        finding.canonical_gap = Some(FindingCanonicalGap {
            id: "gap:perl:lib/My/App.pm:My::App::discount:return_value:exact_return_assertion:return_value"
                .to_string(),
            language: "perl".to_string(),
            file: "lib/My/App.pm".to_string(),
            owner: "perl:lib/My/App.pm::My::App::discount".to_string(),
            behavior_kind: "return_value".to_string(),
            probe_kind: "exact_return_assertion".to_string(),
            normalized_discriminator: "return_value".to_string(),
        });
        finding.probe = Probe {
            id: ProbeId("probe:lib_My_App_pm:8:perl_return".to_string()),
            location: SourceLocation::new("lib/My/App.pm", 8, 5),
            owner: Some(SymbolId(
                "perl:lib/My/App.pm::My::App::discount".to_string(),
            )),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: Some("return $price".to_string()),
            after: Some("return $discounted".to_string()),
            expression: "return $discounted".to_string(),
            expected_sinks: vec!["return_value".to_string()],
            required_oracles: vec!["exact_return_assertion".to_string()],
        };
        finding.class = ExposureClass::WeaklyExposed;
        finding.ripr = RiprEvidence {
            reach: stage(
                StageState::Yes,
                Confidence::Medium,
                "Perl fact packet links the related test to the changed owner",
            ),
            infect: stage(
                StageState::Yes,
                Confidence::Medium,
                "Changed return value reaches the owner result",
            ),
            propagate: stage(
                StageState::Yes,
                Confidence::Medium,
                "Return value can propagate to Test::More assertion",
            ),
            reveal: RevealEvidence {
                observe: stage(
                    StageState::Yes,
                    Confidence::Medium,
                    "Related test reaches the changed owner",
                ),
                discriminate: stage(
                    StageState::Weak,
                    Confidence::Medium,
                    "Exact return discriminator is missing",
                ),
            },
        };
        finding.confidence = 0.8;
        finding.evidence = vec![
            "perl_packet_id: perl-preview:gap-return".to_string(),
            "perl_repair_kind: add_exact_return_assertion".to_string(),
            "perl_target_test_shape: Test::More exact_return_assertion".to_string(),
            "perl_suggested_test_location: t/app.t::discount_smoke".to_string(),
            "perl_suggested_assertion: assert the exact returned `return_value` value".to_string(),
            "perl_verify_command: prove t/app.t".to_string(),
            "perl_receipt_command: ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id perl-gap --json".to_string(),
            "perl_confidence: medium".to_string(),
            "perl_allowed_edit_boundary: t/app.t".to_string(),
            "perl_forbidden_edit_boundary: lib/My/App.pm, badges/ripr-plus.json".to_string(),
            "perl_stop_if: perl-lsp packet status changes".to_string(),
            "perl_must_not_change: do not edit Perl production code".to_string(),
            "raw_evidence_ref: leg=perl_change;file=lib/My/App.pm;line=8;kind=perl_change;source_id=change:lib/My/App.pm:8:return;owner=perl:lib/My/App.pm::My::App::discount;sample=return $discounted".to_string(),
            "raw_evidence_ref: leg=perl_oracle;file=t/app.t;line=7;kind=perl_oracle;source_id=oracle:t/app.t:7:is;owner=perl:lib/My/App.pm::My::App::discount;sample=is(discount(...), 90)".to_string(),
        ];
        finding.missing = vec!["return_value".to_string()];
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "return_value".to_string(),
            reason: "Related Perl test reaches the owner but lacks an exact return discriminator"
                .to_string(),
            flow_sink: None,
        }];
        finding.related_tests = vec![RelatedTest {
            name: "discount_smoke".to_string(),
            file: PathBuf::from("t/app.t"),
            line: 7,
            oracle: Some("ok(discount(...))".to_string()),
            oracle_kind: OracleKind::SmokeOnly,
            oracle_strength: OracleStrength::Weak,
            relation_reason: None,
            relation_confidence: None,
        }];
        finding.recommended_next_step = Some("Add a focused Perl assertion.".to_string());
        finding.language = Some(LanguageId::Perl);
        finding.language_status = Some(LanguageStatus::Preview);
    }

    #[test]
    fn discriminator_line_never_says_yes_for_a_non_exposed_finding() {
        // CLI parity with the editor hover (#4419): a strong related oracle
        // on a weakly_exposed finding must not read as "discriminator yes".
        use crate::output::discriminator_line::discriminator_evidence_line;
        let mut finding = sample_finding();
        finding.ripr.reveal.discriminate =
            stage(StageState::Yes, Confidence::High, "Strong oracle found");
        assert_eq!(
            discriminator_evidence_line(&finding),
            "discriminator missing: `enabled == false`; related oracle: Strong oracle found"
        );
        finding.activation.missing_discriminators.clear();
        assert_eq!(
            discriminator_evidence_line(&finding),
            "discriminator not established (weakly_exposed); related oracle: Strong oracle found"
        );
        finding.class = ExposureClass::Exposed;
        assert_eq!(
            discriminator_evidence_line(&finding),
            "discriminator yes: Strong oracle found"
        );
    }

    fn bounded_output_with_findings(findings: Vec<Finding>) -> CheckOutput {
        let count = findings.len();
        CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: count,
                findings: count,
                weakly_exposed: count,
                ..Summary::default()
            },
            findings,
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        }
    }

    fn sample_finding() -> Finding {
        Finding {
            id: "probe:sample.rs:7:predicate".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:sample.rs:7:predicate".to_string()),
                location: SourceLocation::new("src/sample.rs", 7, 3),
                owner: None,
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: Some("if enabled".to_string()),
                after: Some("if disabled".to_string()),
                expression: "enabled".to_string(),
                expected_sinks: vec![],
                required_oracles: vec![],
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: stage(StageState::Yes, Confidence::High, "reaches test"),
                infect: stage(StageState::Weak, Confidence::Medium, "weak mutation"),
                propagate: stage(StageState::Unknown, Confidence::Low, "propagation unclear"),
                reveal: RevealEvidence {
                    observe: stage(StageState::Yes, Confidence::High, "observed"),
                    discriminate: stage(StageState::No, Confidence::Medium, "no discriminator"),
                },
            },
            confidence: 0.7,
            evidence: vec![],
            missing: vec!["missing strong oracle".to_string()],
            flow_sinks: vec![FlowSinkFact {
                kind: FlowSinkKind::ReturnValue,
                text: "disabled_result".to_string(),
                line: 8,
                owner: None,
            }],
            activation: ActivationEvidence {
                observed_values: vec![ValueFact {
                    line: 22,
                    text: "sample(false)".to_string(),
                    value: "enabled = false".to_string(),
                    context: ValueContext::FunctionArgument,
                }],
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "enabled == false".to_string(),
                    reason: "related tests do not use the changed value".to_string(),
                    flow_sink: None,
                }],
            },
            stop_reasons: vec![],
            related_tests: vec![RelatedTest {
                name: "test_handles_disabled".to_string(),
                file: PathBuf::from("tests/sample.rs"),
                line: 22,
                oracle: Some("assert_eq!(actual, expected)".to_string()),
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
                relation_reason: None,
                relation_confidence: None,
            }],
            recommended_next_step: Some("Add assertion for disabled path result.".to_string()),
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
            source_currentness: crate::domain::SourceCurrentness::CandidateCurrent,
        }
    }

    fn unknown_finding() -> Finding {
        Finding {
            id: "probe:src_lib_rs:1:static_unknown".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_lib_rs:1:static_unknown".to_string()),
                location: SourceLocation::new("src/lib.rs", 1, 1),
                owner: None,
                family: ProbeFamily::StaticUnknown,
                delta: DeltaKind::Unknown,
                before: None,
                after: None,
                expression: "unknown syntax".to_string(),
                expected_sinks: vec![],
                required_oracles: vec![],
            },
            class: ExposureClass::StaticUnknown,
            ripr: RiprEvidence {
                reach: unknown_stage("No stable syntax owner"),
                infect: unknown_stage("Changed syntax is not mapped to a probe"),
                propagate: unknown_stage("No propagation model is available"),
                reveal: RevealEvidence {
                    observe: unknown_stage("No observation model is available"),
                    discriminate: unknown_stage("No discriminator model is available"),
                },
            },
            confidence: 0.2,
            evidence: vec![],
            missing: vec![],
            flow_sinks: vec![],
            activation: ActivationEvidence::default(),
            stop_reasons: vec![],
            related_tests: vec![],
            recommended_next_step: Some("Escalate to real mutation testing.".to_string()),
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
            source_currentness: crate::domain::SourceCurrentness::CandidateCurrent,
        }
    }

    fn stage(state: StageState, confidence: Confidence, summary: &str) -> StageEvidence {
        StageEvidence::new(state, confidence, summary)
    }

    fn unknown_stage(summary: &str) -> StageEvidence {
        stage(StageState::Unknown, Confidence::Low, summary)
    }

    // RIPR-SPEC-0082 tests: preview-language disclosure honesty
    #[test]
    fn render_emits_preview_disclosure_when_typescript_files_in_scope() {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: vec![PreviewLanguageAdvisory {
                language: "typescript".to_string(),
                file_count: 2,
                sample_paths: vec!["src/discount.ts".to_string(), "src/pricing.ts".to_string()],
                javascript_file_count: 0,
                enabled: true,
            }],
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("2 TypeScript files analyzed under preview support"),
            "expected preview disclosure in output; got:\n{rendered}"
        );
        assert!(
            rendered.contains("preview evidence is advisory"),
            "expected advisory note; got:\n{rendered}"
        );
        assert!(
            rendered.contains("NOT a clean Rust-grade result"),
            "expected honesty note; got:\n{rendered}"
        );
    }

    #[test]
    fn render_emits_preview_disclosure_when_python_files_in_scope() {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: vec![PreviewLanguageAdvisory {
                language: "python".to_string(),
                file_count: 3,
                sample_paths: vec!["app/main.py".to_string()],
                javascript_file_count: 0,
                enabled: true,
            }],
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("3 Python files analyzed under preview support"),
            "expected python preview disclosure; got:\n{rendered}"
        );
        assert!(
            rendered.contains("NOT a clean Rust-grade result"),
            "expected honesty note; got:\n{rendered}"
        );

        // With a finding the result is not empty, so the empty-result caveat
        // would be false; the note keeps the advisory framing instead.
        let with_finding = CheckOutput {
            findings: vec![sample_finding()],
            ..output
        };
        let rendered = render(&with_finding);
        assert!(
            rendered.contains("3 Python files analyzed under preview support")
                && rendered.contains("Treat its findings as advisory, not Rust-grade.")
                && !rendered.contains("An empty result here"),
            "non-empty run must not carry the empty-result caveat; got:\n{rendered}"
        );
    }

    #[test]
    fn render_omits_preview_disclosure_for_pure_rust_scope() {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            !rendered.contains("preview support"),
            "pure-Rust scope must not emit preview note; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("NOT a clean Rust-grade result"),
            "pure-Rust scope must not emit honesty note; got:\n{rendered}"
        );
    }

    #[test]
    fn render_preview_disclosure_count_matches_advisory_file_count() {
        // The file_count in the advisory must appear verbatim in the disclosure line.
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: vec![PreviewLanguageAdvisory {
                language: "typescript".to_string(),
                file_count: 7,
                sample_paths: vec!["src/lib.ts".to_string()],
                javascript_file_count: 0,
                enabled: true,
            }],
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("7 TypeScript files analyzed under preview support"),
            "expected file_count=7 in disclosure; got:\n{rendered}"
        );
    }

    #[test]
    fn render_emits_singular_perl_disclosure_when_adapter_disabled() {
        // The #2104 case: one Perl file in the diff but the adapter
        // is NOT enabled. The empty result must be broken by a disclosure that
        // says the files were not analyzed.
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: vec![PreviewLanguageAdvisory {
                language: "perl".to_string(),
                file_count: 1,
                sample_paths: vec!["lib/Pricing.pm".to_string()],
                javascript_file_count: 0,
                enabled: false,
            }],
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("this diff contains 1 Perl file"),
            "expected not-enabled disclosure; got:\n{rendered}"
        );
        assert!(
            rendered.contains("so these files were not analyzed"),
            "expected not-analyzed wording; got:\n{rendered}"
        );
        assert!(
            rendered.contains("NOT a clean Rust-grade result"),
            "expected honesty note; got:\n{rendered}"
        );
        if cfg!(feature = "lang-perl") {
            // Adapter compiled in: the ripr.toml edit is real, but it is not
            // sufficient on its own — a fact packet/exporter is still needed.
            assert!(
                rendered.contains("[languages]\nenabled = [\"rust\", \"perl\"]"),
                "expected copy-paste TOML block; got:\n{rendered}"
            );
            assert!(
                rendered.contains("Perl also needs a fact packet"),
                "expected exporter prerequisite; got:\n{rendered}"
            );
        } else {
            // Adapter NOT compiled in: following a ripr.toml hint makes
            // `ripr check` exit 2 (config rejects `perl`), so the note must
            // not offer it and must name both real prerequisites.
            assert!(
                rendered.contains("not compiled into this ripr binary"),
                "expected not-compiled disclosure; got:\n{rendered}"
            );
            assert!(
                !rendered.contains("Enable it in ripr.toml")
                    && !rendered.contains("enabled = [\"rust\", \"perl\"]"),
                "must not advise a ripr.toml edit this binary rejects; got:\n{rendered}"
            );
            for required in [
                "cargo install ripr --features lang-perl",
                "`perl-ripr-facts`",
                "not yet published",
            ] {
                assert!(
                    rendered.contains(required),
                    "expected `{required}` in recovery; got:\n{rendered}"
                );
            }
        }
        // Must NOT use the enabled wording.
        assert!(
            !rendered.contains("analyzed under preview support"),
            "not-enabled case must not claim analysis ran; got:\n{rendered}"
        );
    }

    #[test]
    fn render_not_enabled_disclosure_includes_language_specific_toml_block() {
        // Verify the copy-paste block uses the actual language name, not a
        // hardcoded string. This covers the Python path; Perl is covered by
        // render_emits_singular_perl_disclosure_when_adapter_disabled.
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: vec![PreviewLanguageAdvisory {
                language: "python".to_string(),
                file_count: 3,
                sample_paths: vec!["app/models.py".to_string()],
                javascript_file_count: 0,
                enabled: false,
            }],
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            !rendered.contains(r#"enabled = ["rust", "typescript"]"#),
            "python advisory must not mention typescript; got:\n{rendered}"
        );
        if cfg!(feature = "lang-python") {
            assert!(
                rendered.contains(r#"enabled = ["rust", "python"]"#),
                "expected python-specific copy-paste TOML block; got:\n{rendered}"
            );
        } else {
            // Adapter NOT compiled in: config load rejects `python`, so the
            // note names the rebuild instead of a TOML edit (#4252).
            assert!(
                rendered.contains("The Python adapter is not compiled into this ripr binary")
                    && rendered.contains("rebuild ripr with Cargo feature `lang-python`"),
                "expected not-compiled Python disclosure; got:\n{rendered}"
            );
            assert!(
                !rendered.contains(r#"enabled = ["rust", "python"]"#),
                "must not advise a ripr.toml edit this binary rejects; got:\n{rendered}"
            );
        }
    }

    #[test]
    fn render_finding_normalizes_backslash_location_path_to_forward_slash() {
        // Proves sections.rs uses display_path: Windows-style .\\src\\pricing.ts
        // must render as src/pricing.ts in the WARNING line.
        let mut finding = sample_finding();
        finding.probe.location = SourceLocation::new(PathBuf::from(r"src\pricing.ts"), 10, 1);

        let rendered = render_finding(&finding);

        assert!(
            rendered.contains("src/pricing.ts:10"),
            "expected forward-slash location path in human output; got:\n{rendered}"
        );
        assert!(
            !rendered.contains(r"src\pricing.ts"),
            "backslash path must not appear in human output; got:\n{rendered}"
        );
    }

    #[test]
    fn render_finding_normalizes_backslash_related_test_path_to_forward_slash() {
        // Proves evidence_lines.rs uses display_path: related test file with
        // backslashes must appear as forward-slash in the evidence lines.
        let mut finding = sample_finding();
        finding.related_tests[0].file = PathBuf::from(r"tests\sample.rs");

        let rendered = render_finding(&finding);

        assert!(
            rendered.contains("tests/sample.rs:"),
            "expected forward-slash related-test path in human evidence; got:\n{rendered}"
        );
        assert!(
            !rendered.contains(r"tests\sample.rs"),
            "backslash related-test path must not appear in human output; got:\n{rendered}"
        );
    }

    // RIPR-SPEC-0083 tests: no-scope disclosure honesty

    #[test]
    fn render_emits_no_scope_guidance_when_no_scope_provided_and_empty() {
        // The cardinal case: bare `ripr check` produces an empty result.
        // `no_scope_provided: true` must emit the guidance note.
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: true,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("no analysis scope was provided"),
            "expected no-scope guidance; got:\n{rendered}"
        );
        assert!(
            rendered.contains("`ripr check --base BASE`"),
            "expected --base guidance; got:\n{rendered}"
        );
        assert!(!rendered.contains("--base origin/main"));
        assert!(
            rendered.contains("does NOT mean your changed behavior is covered"),
            "expected honesty note; got:\n{rendered}"
        );
        // Bug 2 regression guard: the recommended full-repo-scan command must be
        // --format repo-exposure-md, not --mode fast (which is a speed tier).
        assert!(
            rendered.contains("--format repo-exposure-md"),
            "expected --format repo-exposure-md in guidance; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("--mode fast"),
            "guidance must NOT recommend --mode fast as a full-repo scan; got:\n{rendered}"
        );
    }

    #[test]
    fn render_omits_no_scope_guidance_when_scope_provided_and_empty() {
        // Scope was provided (--diff/--base) but found 0 probes.
        // `no_scope_provided: false` must NOT emit the guidance — the result
        // is honest: that diff really had no probes.
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            !rendered.contains("no analysis scope was provided"),
            "scope-provided empty result must NOT show no-scope guidance; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("does NOT mean your changed behavior is covered"),
            "scope-provided empty result must NOT show honesty note; got:\n{rendered}"
        );
    }

    #[test]
    fn render_no_scope_guidance_uses_conservative_static_language() {
        // Verify the no-scope disclosure text uses only approved static-language
        // vocabulary. The gate bans mutation-testing runtime terms; we verify
        // the disclosure uses the approved phrasing ("does NOT mean your changed
        // behavior is covered") rather than any runtime claim.
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: true,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        // Confirm the actual approved honesty phrase is present.
        assert!(
            rendered.contains("does NOT mean your changed behavior is covered"),
            "expected approved honesty phrase; got:\n{rendered}"
        );
        // The disclosure is a static analysis advisory, not a runtime claim.
        assert!(
            rendered.contains("no analysis scope was provided"),
            "expected scope disclosure lead-in; got:\n{rendered}"
        );
    }

    #[test]
    fn guidance_recommends_format_repo_exposure_md_not_mode_fast() {
        // Bug 2 regression guard: the human guidance string must recommend
        // --format repo-exposure-md for a full-repo scan, NOT --mode fast.
        // --mode is a speed tier on the diff path; it does NOT provide scope.
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: true,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("--format repo-exposure-md"),
            "guidance must recommend --format repo-exposure-md for full-repo scan; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("--mode fast"),
            "guidance must NOT recommend --mode fast as a full-repo-scan command; got:\n{rendered}"
        );
    }

    // RIPR-SPEC-0090 tests: all-no-path disclosure honesty

    #[test]
    fn render_emits_all_no_path_disclosure_when_all_findings_are_no_path() {
        // Primary case: findings exist but none are exposed/weak/reachable.
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                changed_rust_files: 1,
                probes: 2,
                findings: 2,
                no_static_path: 2,
                ..Summary::default()
            },
            findings: vec![unknown_finding(), unknown_finding()],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered
                .contains("ripr found no static test path for any of the 2 changed expression(s)"),
            "expected all-no-path disclosure; got:\n{rendered}"
        );
        assert!(
            rendered.contains("not a coverage assessment"),
            "expected honesty note; got:\n{rendered}"
        );
        assert!(
            rendered.contains("A test may already exercise these changes through")
                && rendered.contains("macros, helper-call chains"),
            "expected honest untraced-test wording; got:\n{rendered}"
        );
        assert!(
            rendered.contains(
                "analyzed: 1 changed Rust file(s), 2 changed expression(s), and 0 statically linked related"
            ),
            "expected scope-count disclosure; got:\n{rendered}"
        );
    }

    #[test]
    fn all_no_path_disclosure_for_bindings_does_not_presume_external_tests_exist() {
        let cross_language = || {
            let mut finding = unknown_finding();
            finding.static_limit_kind =
                Some(crate::domain::StaticLimitKind::CrossLanguageOracleVisibilityUnresolved);
            finding
        };
        let output_with = |findings: Vec<Finding>| CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                changed_rust_files: 1,
                probes: 2,
                findings: 2,
                no_static_path: 2,
                ..Summary::default()
            },
            findings,
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };
        let flat = |output: &CheckOutput| {
            render(output)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };

        let bindings = flat(&output_with(vec![cross_language(), cross_language()]));
        assert!(
            bindings.contains(
                "if none does, add or check tests in the bindings' other language that observe the changed behavior."
            ),
            "got:\n{bindings}"
        );
        assert!(
            !bindings.contains("add co-located tests"),
            "got:\n{bindings}"
        );

        // A mixed diff names both repairs, so neither finding's own next
        // step is contradicted.
        let mixed = flat(&output_with(vec![cross_language(), unknown_finding()]));
        assert!(
            mixed.contains(
                "if none does, add co-located tests that observe the changed behavior, or, for a language binding, tests in the binding's other language."
            ),
            "got:\n{mixed}"
        );

        let plain = flat(&output_with(vec![unknown_finding(), unknown_finding()]));
        assert!(
            plain.contains("if none does, add co-located tests that observe the changed behavior."),
            "got:\n{plain}"
        );
    }

    #[test]
    fn render_emits_all_no_path_disclosure_for_infection_unknown_findings() {
        // Also fires for infection_unknown / propagation_unknown / static_unknown classes.
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 1,
                findings: 1,
                static_unknown: 1,
                ..Summary::default()
            },
            findings: vec![unknown_finding()],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered
                .contains("ripr found no static test path for any of the 1 changed expression(s)"),
            "expected disclosure for static_unknown finding; got:\n{rendered}"
        );
    }

    #[test]
    fn render_all_no_path_disclosure_counts_linked_related_tests() {
        let mut finding = unknown_finding();
        let related_test = RelatedTest {
            name: "test_handles_disabled".to_string(),
            file: PathBuf::from("tests/sample.rs"),
            line: 22,
            oracle: Some("assert_eq!(actual, expected)".to_string()),
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            relation_reason: None,
            relation_confidence: None,
        };
        finding.related_tests.push(related_test.clone());
        let mut duplicate_finding = unknown_finding();
        duplicate_finding.related_tests.push(related_test);
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 1,
                findings: 2,
                static_unknown: 2,
                ..Summary::default()
            },
            findings: vec![finding, duplicate_finding],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("analyzed: 2 changed expression(s) and 1 statically linked related"),
            "expected related-test count disclosure; got:\n{rendered}"
        );
    }

    #[test]
    fn human_advisory_prose_wrap_preserves_words_and_fixed_width() {
        let prose = "ripr saw a test reaching public API that may call toward this change through a transitive path it does not fully trace (pub to pub(crate) helper chains, macros, or generics). This is not a coverage assessment -- ripr cannot confirm or deny that the change is observed.";
        let wrapped = super::wrap_human_prose(prose, "  - ", "    ");

        assert!(wrapped.lines().all(|line| line.chars().count() <= 100));
        assert_eq!(
            wrapped
                .trim_start_matches("  - ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            prose
        );
    }

    #[test]
    fn render_omits_all_no_path_disclosure_when_a_finding_reaches() {
        // Honesty (dogfood: anyhow Chain::len): an unknown-class finding can carry
        // reach=yes (a test DOES reach the change). Claiming "no static test path"
        // then contradicts the finding's own reach evidence, so the all-no-path
        // note must be suppressed when any finding reaches.
        let mut finding = unknown_finding();
        finding.ripr.reach.state = StageState::Yes;
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 1,
                findings: 1,
                static_unknown: 1,
                ..Summary::default()
            },
            findings: vec![finding],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            !rendered.contains("ripr found no static test path for any"),
            "must not claim no-static-path when a finding reaches; got:\n{rendered}"
        );
    }

    #[test]
    fn render_omits_all_no_path_disclosure_when_exposed_finding_exists() {
        // If any finding is exposed, the per-finding output carries the signal.
        // Do NOT emit the all-no-path note.
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 2,
                findings: 2,
                exposed: 1,
                no_static_path: 1,
                ..Summary::default()
            },
            findings: vec![sample_finding(), unknown_finding()],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            !rendered.contains("ripr found no static test path for any of the"),
            "must NOT emit all-no-path disclosure when an exposed finding exists; got:\n{rendered}"
        );
    }

    #[test]
    fn render_omits_all_no_path_disclosure_when_weakly_exposed_finding_exists() {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 1,
                findings: 1,
                weakly_exposed: 1,
                ..Summary::default()
            },
            findings: vec![sample_finding()],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            !rendered.contains("ripr found no static test path for any of the"),
            "must NOT emit all-no-path disclosure when a weakly_exposed finding exists; got:\n{rendered}"
        );
    }

    #[test]
    fn render_omits_all_no_path_disclosure_when_zero_findings() {
        // Zero findings is a different case (handled by no-probes message).
        // The all-no-path disclosure must NOT fire here.
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            !rendered.contains("ripr found no static test path for any of the"),
            "must NOT emit all-no-path disclosure when there are zero findings; got:\n{rendered}"
        );
    }

    #[test]
    fn render_all_no_path_disclosure_uses_finding_count_not_probe_count() {
        // The count shown must be the no-path/unknown total (= findings), not probes.
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 5,
                findings: 3,
                no_static_path: 2,
                static_unknown: 1,
                ..Summary::default()
            },
            findings: vec![unknown_finding(), unknown_finding(), unknown_finding()],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("for any of the 3 changed expression(s)"),
            "expected count=3 (findings), not 5 (probes); got:\n{rendered}"
        );
    }

    #[test]
    fn render_all_no_path_disclosure_uses_conservative_static_language() {
        // Verify the disclosure does not use forbidden mutation-testing vocabulary.
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.2".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary {
                probes: 1,
                findings: 1,
                no_static_path: 1,
                ..Summary::default()
            },
            findings: vec![unknown_finding()],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(!rendered.contains("killed"), "must not use 'killed'"); // ripr-allow: static-language: test guard verifying disclosure does not emit forbidden mutation-testing term
        assert!(!rendered.contains("survived"), "must not use 'survived'"); // ripr-allow: static-language: test guard verifying disclosure does not emit forbidden mutation-testing term
        assert!(!rendered.contains("untested"), "must not use 'untested'"); // ripr-allow: static-language: test guard verifying disclosure does not emit forbidden mutation-testing term
        assert!(!rendered.contains("proven"), "must not use 'proven'"); // ripr-allow: static-language: test guard verifying disclosure does not emit forbidden mutation-testing term
        assert!(!rendered.contains("adequate"), "must not use 'adequate'"); // ripr-allow: static-language: test guard verifying disclosure does not emit forbidden mutation-testing term
        assert!(
            rendered.contains("ripr found no static test path"),
            "expected absence-of-path statement"
        );
    }

    #[test]
    fn preview_disclosure_counts_files_with_language_display_names() {
        let advisory = |language: &str, file_count: usize, enabled: bool| PreviewLanguageAdvisory {
            language: language.to_string(),
            file_count,
            sample_paths: Vec::new(),
            javascript_file_count: 0,
            enabled,
        };
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![],
            preview_language_advisories: vec![
                advisory("typescript", 1, true),
                advisory("python", 1, true),
                advisory("javascript", 2, true),
                advisory("typescript", 1, false),
            ],
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        // The not-enabled TypeScript note names the adapter either way; its
        // wording depends on whether this binary can analyze TypeScript.
        let not_enabled = if cfg!(feature = "lang-typescript") {
            "Note: this diff contains 1 TypeScript file. The TypeScript adapter is preview"
        } else {
            "Note: this diff contains 1 TypeScript file. The TypeScript adapter is not compiled into this ripr binary"
        };
        for expected in [
            "Note: 1 TypeScript file analyzed under preview support",
            "Note: 1 Python file analyzed under preview support",
            "Note: 2 JavaScript files analyzed under preview support",
            not_enabled,
        ] {
            assert!(
                rendered.contains(expected),
                "expected `{expected}`; got:\n{rendered}"
            );
        }
        for forbidden in ["(s) analyzed", "Typescript", "Javascript", "Python(s)"] {
            assert!(
                !rendered.contains(forbidden),
                "must not render `{forbidden}`; got:\n{rendered}"
            );
        }
    }

    /// #4555: the TypeScript adapter also analyzes JavaScript, so a
    /// JavaScript-only advisory says "JavaScript file", a mixed one says
    /// "TypeScript/JavaScript files", and the not-enabled note keeps the
    /// `"typescript"` config value with a gloss that it covers JavaScript.
    #[test]
    fn typescript_advisory_names_javascript_and_mixed_files() {
        let advisory = |file_count: usize, javascript_file_count: usize, enabled: bool| {
            PreviewLanguageAdvisory {
                language: "typescript".to_string(),
                file_count,
                sample_paths: Vec::new(),
                javascript_file_count,
                enabled,
            }
        };
        let render_one = |advisory: PreviewLanguageAdvisory| {
            render(&CheckOutput {
                harness_projections: Vec::new(),
                schema_version: "0.1".to_string(),
                tool: "ripr".to_string(),
                mode: Mode::Draft,
                root: PathBuf::from("repo"),
                base: None,
                summary: Summary::default(),
                findings: vec![],
                preview_language_advisories: vec![advisory],
                language_runs: Vec::new(),
                no_scope_provided: false,
                unanalyzed_working_tree: false,
                suppression: None,
                analysis_outcome: None,
                partial_scope: None,
            })
        };

        let javascript_only = render_one(advisory(1, 1, true));
        assert!(
            javascript_only.contains("Note: 1 JavaScript file analyzed under preview support"),
            "{javascript_only}"
        );
        let mixed = render_one(advisory(3, 1, true));
        assert!(
            mixed.contains("Note: 3 TypeScript/JavaScript files analyzed under preview support"),
            "{mixed}"
        );
        let typescript_only = render_one(advisory(2, 0, true));
        assert!(
            typescript_only.contains("Note: 2 TypeScript files analyzed under preview support"),
            "{typescript_only}"
        );

        let not_enabled = render_one(advisory(1, 1, false));
        let expected = if cfg!(feature = "lang-typescript") {
            "Note: this diff contains 1 JavaScript file. The TypeScript/JavaScript adapter is preview"
        } else {
            "Note: this diff contains 1 JavaScript file. The TypeScript/JavaScript adapter is not compiled into this ripr binary"
        };
        assert!(not_enabled.contains(expected), "{not_enabled}");
        if cfg!(feature = "lang-typescript") {
            assert!(
                not_enabled.contains("enabled = [\"rust\", \"typescript\"]"),
                "{not_enabled}"
            );
            assert!(
                not_enabled
                    .contains("(\"typescript\" enables the adapter for JavaScript files too.)"),
                "{not_enabled}"
            );
        }
        // A TypeScript-only diff counts TypeScript files and gets no
        // JavaScript gloss. The adapter name itself says
        // "TypeScript/JavaScript" in every build.
        let typescript_not_enabled = render_one(advisory(1, 0, false));
        assert!(
            typescript_not_enabled.contains("Note: this diff contains 1 TypeScript file."),
            "{typescript_not_enabled}"
        );
        for forbidden in ["JavaScript file.", "enables the adapter for JavaScript"] {
            assert!(
                !typescript_not_enabled.contains(forbidden),
                "TypeScript-only note must not render `{forbidden}`:\n{typescript_not_enabled}"
            );
        }
    }
}
