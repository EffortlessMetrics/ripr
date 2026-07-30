use crate::app::{CheckOutput, FindingNavigation};
use crate::config::RiprConfig;
use crate::domain::Finding;
use std::collections::BTreeSet;

/// Render the bounded triage report in the default human-readable CLI format.
pub fn render(output: &CheckOutput) -> String {
    render_bounded_with_config(output, &RiprConfig::default())
}

#[cfg(test)]
pub(crate) fn render_with_config(output: &CheckOutput, config: &RiprConfig) -> String {
    render_bounded_with_config(output, config)
}

pub(crate) fn render_bounded_with_config(output: &CheckOutput, config: &RiprConfig) -> String {
    let navigation = FindingNavigation::legacy();
    render_bounded_with_config_and_navigation(output, config, Some(&navigation))
}

pub(crate) fn render_bounded_with_config_and_navigation(
    output: &CheckOutput,
    config: &RiprConfig,
    navigation: Option<&FindingNavigation>,
) -> String {
    let mut out = render_header_summary(output);
    render_suppression_policy_block(&mut out, output);
    render_partial_scope_disclosure(&mut out, output);

    if output.findings.is_empty() {
        out.push_str("No diff-derived static exposure probes found.\n");
        if output.no_scope_provided {
            let triage = triage::select_human_triage(output, config);
            triage::render_human_triage(&mut out, &triage, output, config, navigation);
        }
        if output.no_scope_provided {
            out.push_str(
                "\nNote: no analysis scope was provided — `ripr check` is diff-first. \
Run `ripr check --base origin/main` to analyze your changes, or \
`ripr check --root . --format repo-exposure-md` for a full-repo scan. \
An empty result here does NOT mean your changed behavior is covered.\n",
            );
        }
        if output.unanalyzed_working_tree {
            out.push_str(
                "\nNote: uncommitted changes to tracked source were not analyzed. \
`--base` compares committed history only — commit or stage these changes and re-run, \
or analyze a committed branch with `ripr check --base origin/main`.\n",
            );
        }
        render_preview_language_advisories(&mut out, output);
        render_language_runs(&mut out, output);
        return out;
    }

    let triage = triage::select_human_triage(output, config);
    triage::render_human_triage(&mut out, &triage, output, config, navigation);
    render_all_no_path_disclosure(&mut out, output);
    if output.unanalyzed_working_tree {
        out.push_str(
            "\nNote: uncommitted changes to tracked source were not analyzed. \
`--base` compares committed history only; run `ripr check` (no --base) to analyze \
your working tree.\n",
        );
    }
    render_preview_language_advisories(&mut out, output);
    render_language_runs(&mut out, output);
    out
}

pub(crate) fn render_full_with_config(output: &CheckOutput, config: &RiprConfig) -> String {
    let mut out = render_header_summary(output);

    render_suppression_policy_block(&mut out, output);
    render_partial_scope_disclosure(&mut out, output);

    if output.findings.is_empty() {
        out.push_str("No diff-derived static exposure probes found.\n");
        // RIPR-SPEC-0083: disclose when no analysis scope was provided.
        // This fires only when the caller passed no --diff/--base/--mode, so
        // an empty result here means "nothing was analyzed", not "tests pass".
        if output.no_scope_provided {
            out.push_str(
                "\nNote: no analysis scope was provided — `ripr check` is diff-first. \
Run `ripr check --base origin/main` to analyze your changes, or \
`ripr check --root . --format repo-exposure-md` for a full-repo scan. \
An empty result here does NOT mean your changed behavior is covered.\n",
            );
        }
        // RIPR-SPEC-0112: disclose when --base was used but uncommitted working-tree
        // changes were NOT analyzed. An empty result here does NOT mean those changes
        // are covered — they were excluded from the committed-history diff.
        if output.unanalyzed_working_tree {
            out.push_str(
                "\nNote: uncommitted changes to tracked source were not analyzed. \
`--base` compares committed history only — commit or stage these changes and re-run, \
or analyze a committed branch with `ripr check --base origin/main`.\n",
            );
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
    for finding in &output.findings {
        if suppressed_ids.contains(finding.id.as_str()) {
            continue;
        }
        out.push_str(&render_finding_with_config(finding, config));
        out.push('\n');
    }
    render_all_no_path_disclosure(&mut out, output);
    // RIPR-SPEC-0112: disclose when --base was used but uncommitted working-tree
    // changes were NOT analyzed. Fires whether or not the committed diff had findings —
    // those uncommitted edits are still unanalyzed regardless.
    if output.unanalyzed_working_tree {
        out.push_str(
            "\nNote: uncommitted changes to tracked source were not analyzed. \
`--base` compares committed history only; run `ripr check` (no --base) to analyze \
your working tree.\n",
        );
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
    out.push_str(&format!(
        "Summary: {} probe(s), {} exposed, {} weak, {} unrevealed, {} no path, {} unknown\n\n",
        output.summary.probes,
        output.summary.exposed,
        output.summary.weakly_exposed,
        output.summary.reachable_unrevealed,
        output.summary.no_static_path,
        output.summary.static_unknown
            + output.summary.infection_unknown
            + output.summary.propagation_unknown
    ));
    render_language_file_breakdown(&mut out, output);
    out
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
    let parts = counts
        .iter()
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
/// names the exact selected partition, the lower-bound uninspected scope, the
/// stop reason, gate ineligibility, and the only continuation route (raising
/// the explicit budget overrides).
fn render_partial_scope_disclosure(out: &mut String, output: &CheckOutput) {
    let Some(scope) = &output.partial_scope else {
        return;
    };
    out.push_str(&format!(
        "Partial scope: run state {} — analyzed {} changed file(s) ({} changed line(s)) of the diff; \
         stop reason: {}.\n",
        scope.run_status,
        scope.selected_files.len(),
        scope.selected_changed_lines,
        scope.stop_reason.as_str(),
    ));
    out.push_str(&format!(
        "  NOT inspected: at least {} changed file(s) and at least {} changed line(s); \
         the uninspected scope may contain additional findings.\n",
        scope.uninspected_files_lower_bound, scope.uninspected_changed_lines_lower_bound,
    ));
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
    out.push_str(&format!(
        "  {}\n  partition_identity: {}\n\n",
        crate::analysis::PartialDiffScope::CONTINUATION_DISCLOSURE,
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
    let note = format!(
        "Note: ripr found no static test path for any of the {} changed expression(s) in this diff. {} This is not a coverage assessment. A test may already exercise these changes through macros, helper-call chains, or integration tests that ripr's static model does not yet trace; if none does, add co-located tests that observe the changed behavior.",
        all_no_path_count, scope_summary
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
/// Two wordings per the `enabled` flag:
///
/// - `enabled` — the preview adapter ran; the empty/partial result is advisory
///   and may be incomplete, not Rust-grade clean.
/// - not enabled — the files were detected but not analyzed because the
///   preview adapter is not enabled in `ripr.toml`; the empty result must not
///   be read as clean. A copy-paste-ready TOML block is appended so enabling
///   the adapter is a single edit.
fn render_preview_language_advisories(out: &mut String, output: &CheckOutput) {
    for advisory in &output.preview_language_advisories {
        let language = capitalize_first(&advisory.language);
        let file_label = if advisory.language == "perl" && advisory.file_count == 1 {
            format!("{language} file")
        } else {
            format!("{language}(s)")
        };
        if advisory.enabled {
            out.push_str(&format!(
                "\nNote: {} {} analyzed under preview support — preview evidence is advisory and may be incomplete. An empty result here is NOT a clean Rust-grade result.\n",
                advisory.file_count, file_label,
            ));
        } else {
            let language_lowercase = advisory.language.to_lowercase();
            out.push_str(&format!(
                "\nNote: this diff contains {} {}. The {} adapter is preview and not enabled, so these files were not analyzed — this is NOT a clean Rust-grade result. Enable it in ripr.toml [languages] to analyze them.\n\nTo enable, add to ripr.toml:\n\n[languages]\nenabled = [\"rust\", \"{language_lowercase}\"]\n",
                advisory.file_count, file_label, language,
            ));
        }
    }
}

/// Render per-language run-status lines for languages that did not complete
/// successfully (non-abort contract, Campaign 31 PR 10, #1403). Silent when
/// every language ran to completion.
fn render_language_runs(out: &mut String, output: &CheckOutput) {
    for run in &output.language_runs {
        let language = capitalize_first(&run.language);
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
        MissingDiscriminatorFact, OracleKind, OracleStrength, Probe, ProbeFamily, ProbeId,
        RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
        Summary, SymbolId, ValueContext, ValueFact,
    };
    use std::path::PathBuf;

    #[test]
    fn render_includes_summary_counts_and_empty_findings_message() {
        let output = CheckOutput {
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
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("mode: draft"));
        assert!(rendered.contains(
            "Summary: 8 probe(s), 1 exposed, 2 weak, 1 unrevealed, 1 no path, 3 unknown"
        ));
        assert!(rendered.contains("No diff-derived static exposure probes found."));
        assert!(!rendered.contains("Next:"));
    }

    #[test]
    fn bounded_human_output_suggests_explain_and_context_for_top_finding() {
        let finding = sample_finding();
        let finding_id = finding.id.clone();
        let output = CheckOutput {
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
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("Next: drill into the top finding:"));
        assert!(rendered.contains(&format!("  ripr explain {finding_id}\n")));
        assert!(rendered.contains(&format!("  ripr context --at {finding_id}\n")));
    }

    /// #2567: nothing was omitted, so the render must not advertise a hidden
    /// remainder. The format pointers stay under `More:`.
    #[test]
    fn render_replaces_hidden_block_with_more_when_nothing_is_omitted() {
        let output = CheckOutput {
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
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("\nHidden:\n"));
        assert!(rendered.contains("2 lower-priority finding(s) omitted"));
        assert!(!rendered.contains("More:"));
    }

    /// #2103: a Rust-only run emits no per-language breakdown line, so
    /// Rust-only human output stays byte-identical.
    #[test]
    fn render_omits_language_breakdown_for_rust_only_run() {
        let output = CheckOutput {
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
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("Changed file(s) by language: python: 5, rust: 1."),
            "expected per-language breakdown line; got:\n{rendered}"
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
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("Start here:"));
        assert!(rendered.contains("State: top_gap"));
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
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("State: top_gap"));
        assert!(rendered.contains("File: src/actionable.rs:9"));
        assert!(rendered.contains("Static exposure: reachable_unrevealed"));
        assert!(!rendered.contains("File: src/exposed.rs:1"));
    }

    #[test]
    fn bounded_human_output_reports_missing_scope_as_start_here_state() {
        let output = CheckOutput {
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
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("Start here:"));
        assert!(rendered.contains("State: missing_scope"));
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
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("State: preview_limited"));
        assert!(rendered.contains("preview-language evidence is advisory"));
        assert!(!rendered.contains("State: top_gap"));
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
                digest.contains("  Missing discriminator: missing strong oracle"),
                "expected missing-discriminator label for {:?}; got:\n{digest}",
                finding.class
            );
            assert!(
                !digest.contains("Discriminator (observed, advisory)"),
                "non-exposed digest must not use the observed-advisory label; got:\n{digest}"
            );
        }
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

    // #2273: the preview_limited safe next action distinguishes a
    // complete-but-advisory repair packet (shared validator authority) from
    // one with missing fields.
    #[test]
    fn preview_limited_safe_action_keeps_missing_fields_line_for_incomplete_packet() {
        let finding = typescript_preview_finding(false);
        let output = single_finding_output(finding);

        let rendered = render(&output);

        assert!(rendered.contains("State: preview_limited"));
        assert!(rendered.contains(
            "  Safe next action: preview-language evidence is advisory; complete the missing repair-packet fields before acting.\n"
        ));
        assert!(!rendered.contains("the repair packet is complete but remains advisory"));
    }

    #[test]
    fn preview_limited_safe_action_names_complete_but_advisory_packet() {
        let finding = typescript_preview_finding(true);
        let output = single_finding_output(finding);

        let rendered = render(&output);

        assert!(rendered.contains("State: preview_limited"));
        assert!(rendered.contains(
            "  Safe next action: preview-language evidence is advisory; the repair packet is complete but remains advisory, so verify independently before acting.\n"
        ));
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

        assert!(rendered.contains("State: preview_limited"));
        assert!(rendered.contains(
            "  Safe next action: preview-language evidence is advisory; the repair packet is blocked by the named static limitation, not by missing fields; resolve the limitation and rerun preview evidence before acting.\n"
        ));
        assert!(!rendered.contains("complete the missing repair-packet fields before acting"));
        assert!(!rendered.contains("the repair packet is complete but remains advisory"));
    }

    // Guard against over-crediting the limitation arm: the same blocked
    // packet WITHOUT a structured static-limit kind keeps the generic
    // missing-fields line (e.g. a strong-oracle preview finding whose packet
    // simply lacks projected contract fields).
    #[test]
    fn preview_limited_safe_action_keeps_missing_fields_line_without_static_limit_kind() {
        let mut finding = typescript_preview_finding(false);
        finding
            .evidence
            .retain(|line| !line.starts_with("missing_actionability_fields: "));
        let output = single_finding_output(finding);

        let rendered = render(&output);

        assert!(rendered.contains("State: preview_limited"));
        assert!(rendered.contains(
            "  Safe next action: preview-language evidence is advisory; complete the missing repair-packet fields before acting.\n"
        ));
        assert!(!rendered.contains("blocked by the named static limitation"));
    }

    fn single_finding_output(finding: Finding) -> CheckOutput {
        CheckOutput {
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
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("State: top_gap"));
        assert!(rendered.contains("File: src/stable.rs:10"));
        assert!(!rendered.contains("File: src/preview.ts:1"));
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
            partial_scope: None,
        };

        let rendered =
            super::render_full_with_config(&output, &crate::config::RiprConfig::default());

        assert_eq!(rendered.matches("Changed\n").count(), 2);
        assert_eq!(rendered.matches("Probe\n").count(), 2);
        assert!(!rendered.contains("lower-priority finding(s) omitted"));
    }

    #[test]
    fn render_lists_policy_suppressed_findings_compactly_with_warnings() {
        use crate::output::suppressions::{CheckSuppressionOutcome, SuppressedCheckFinding};
        let finding = sample_finding();
        let finding_id = finding.id.clone();
        let location = finding.probe.location.file.display().to_string();
        let output = CheckOutput {
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
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("State: no_actionable_gap"));
        assert!(rendered.contains("all findings are suppressed by policy"));
        assert!(!rendered.contains("inspect the named static limitation"));
        assert!(!rendered.contains("Next: drill into the top finding:"));
    }

    #[test]
    fn human_output_discloses_limited_partial_scope_run_state() -> Result<(), String> {
        let output = CheckOutput {
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
            partial_scope: None,
            ..output
        };
        assert!(
            !super::render_bounded_with_config(&full, &crate::config::RiprConfig::default())
                .contains("limited_partial_scope")
        );
        Ok(())
    }

    #[test]
    fn human_output_escapes_control_bytes_in_selected_paths() -> Result<(), String> {
        // A crafted diff filename with control bytes must not reach the
        // terminal verbatim (#2142 review): the display is escaped while the
        // raw path stays on the scope record.
        let output = CheckOutput {
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
        assert!(rendered.contains("  verify: prove t/app.t (fact_only_not_delegated)\n"));
        assert!(rendered.contains("  receipt: available_not_delegated\n"));
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
                enabled: true,
            }],
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("2 Typescript(s) analyzed under preview support"),
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
                enabled: true,
            }],
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("3 Python(s) analyzed under preview support"),
            "expected python preview disclosure; got:\n{rendered}"
        );
        assert!(
            rendered.contains("NOT a clean Rust-grade result"),
            "expected honesty note; got:\n{rendered}"
        );
    }

    #[test]
    fn render_omits_preview_disclosure_for_pure_rust_scope() {
        let output = CheckOutput {
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
                enabled: true,
            }],
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("7 Typescript(s) analyzed under preview support"),
            "expected file_count=7 in disclosure; got:\n{rendered}"
        );
    }

    #[test]
    fn render_emits_singular_perl_disclosure_when_adapter_disabled() {
        // The #2104 case: one Perl file in the diff but the adapter
        // is NOT enabled. The empty result must be broken by a disclosure that
        // says the files were not analyzed.
        let output = CheckOutput {
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
                enabled: false,
            }],
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("this diff contains 1 Perl file"),
            "expected not-enabled disclosure; got:\n{rendered}"
        );
        assert!(
            rendered.contains("not enabled, so these files were not analyzed"),
            "expected not-analyzed wording; got:\n{rendered}"
        );
        assert!(
            rendered.contains("NOT a clean Rust-grade result"),
            "expected honesty note; got:\n{rendered}"
        );
        assert!(
            rendered.contains("Enable it in ripr.toml"),
            "expected enable hint; got:\n{rendered}"
        );
        // Must include the copy-paste TOML block.
        assert!(
            rendered.contains("[languages]\nenabled = [\"rust\", \"perl\"]"),
            "expected copy-paste TOML block; got:\n{rendered}"
        );
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
                enabled: false,
            }],
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains(r#"enabled = ["rust", "python"]"#),
            "expected python-specific copy-paste TOML block; got:\n{rendered}"
        );
        assert!(
            !rendered.contains(r#"enabled = ["rust", "typescript"]"#),
            "python advisory must not mention typescript; got:\n{rendered}"
        );
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
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(
            rendered.contains("no analysis scope was provided"),
            "expected no-scope guidance; got:\n{rendered}"
        );
        assert!(
            rendered.contains("`ripr check --base origin/main`"),
            "expected --base guidance; got:\n{rendered}"
        );
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
    fn render_emits_all_no_path_disclosure_for_infection_unknown_findings() {
        // Also fires for infection_unknown / propagation_unknown / static_unknown classes.
        let output = CheckOutput {
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
}
