use crate::app::CheckOutput;
use crate::config::RiprConfig;
use crate::domain::{ExposureClass, Finding, LanguageId, LanguageStatus};
use crate::output::next_step::reconcile_next_step;
use crate::output::path::repository_display_path;
use crate::output::perl_preview_card::perl_preview_card;
use crate::output::preview_actionability::preview_actionability_for;
use crate::output::python_repair_card::python_repair_card;
use crate::output::typescript_preview_card::typescript_preview_card;
use crate::output::workflow_escape::{escape_data, escape_property, escape_property_pre_encoded};

/// Render findings as GitHub Actions workflow command annotations.
///
/// Each finding is emitted as one escaped annotation line so newlines and
/// punctuation survive GitHub's workflow-command parser.
pub fn render(output: &CheckOutput) -> String {
    render_with_config(output, &RiprConfig::default())
}

pub(crate) fn render_with_config(output: &CheckOutput, config: &RiprConfig) -> String {
    let mut annotations = String::new();
    let mut per_level = std::collections::BTreeMap::<&'static str, usize>::new();
    // Findings suppressed by an explicit `--suppression-policy` (#1441) are
    // not annotated: filtering PR-annotation noise on accepted surfaces is
    // the purpose of the policy. The JSON surface keeps them visible. When
    // every finding is omitted this way, emit a denominator notice (#4393)
    // rather than an empty stream.
    let suppressed_ids: std::collections::BTreeSet<&str> = output
        .suppression
        .iter()
        .flat_map(|outcome| {
            outcome
                .suppressed
                .iter()
                .map(|entry| entry.finding_id.as_str())
        })
        .collect();
    let mut suppressed = 0usize;
    let mut not_current = 0usize;
    for finding in &output.findings {
        if suppressed_ids.contains(finding.id.as_str()) {
            suppressed += 1;
            continue;
        }
        // Candidate-actionable eligibility (#3281): annotations are current
        // PR obligations; base-side evidence and unresolved subjects remain
        // visible on the check JSON and human surfaces.
        if !finding.is_candidate_actionable() {
            not_current += 1;
            continue;
        }
        let Some(annotation_level) = config
            .severity()
            .for_exposure(&finding.class)
            .github_annotation_level()
        else {
            continue;
        };
        let title = format!("ripr {}", finding.class.as_str());
        let reconciled = reconcile_next_step(finding);
        let mut message = if reconciled.is_empty() {
            "Static RIPR exposure finding".to_string()
        } else {
            reconciled
        };
        let stop_reasons = finding.effective_stop_reasons();
        if !stop_reasons.is_empty() {
            let reasons = stop_reasons
                .iter()
                .map(|reason| reason.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            message.push_str(" Stop reason: ");
            message.push_str(&reasons);
        }
        if let Some(gap) = &finding.canonical_gap {
            message.push_str(" Canonical gap: ");
            message.push_str(&gap.id);
        }
        if let Some(actionability) = preview_actionability_for(finding) {
            message.push_str(" Preview actionability: ");
            message.push_str(&actionability.gap_state);
            message.push('/');
            message.push_str(&actionability.actionability_category);
            message.push_str(advisory_packet_suffix(actionability.repair_packet_ready));
        }
        if let Some(card) = python_repair_card(finding) {
            message.push_str(" Python repair card: missing discriminator `");
            message.push_str(&card.missing_discriminator);
            message.push_str("`; add or strengthen `");
            message.push_str(&card.suggested_test_name);
            message.push_str("` in `");
            message.push_str(&card.suggested_test_file);
            message.push_str("`; verify `");
            message.push_str(&card.verify_command);
            message.push_str("` (preview advisory).");
        } else if let Some(no_action) = python_no_action_annotation(finding) {
            message.push_str(&no_action);
        }
        if let Some(card) = typescript_preview_card(finding) {
            message.push_str(" TypeScript preview card: owner `");
            message.push_str(&card.owner);
            message.push_str("`, oracle ");
            message.push_str(&card.oracle_kind);
            message.push('/');
            message.push_str(&card.oracle_strength);
            message.push_str(", suggested shape `");
            message.push_str(&card.suggested_assertion_shape);
            message.push('`');
            message.push_str(advisory_packet_suffix(card.repair_packet_ready));
            for (index, grip) in card.bun_cross_language_grips.iter().enumerate() {
                if card.bun_cross_language_grips.len() == 1 {
                    message.push_str(" Bun cross-language grip: ");
                } else {
                    message.push_str(" Bun cross-language grip ");
                    message.push_str(&(index + 1).to_string());
                    message.push('/');
                    message.push_str(&card.bun_cross_language_grips.len().to_string());
                    message.push_str(": ");
                }
                message.push_str(&grip.state);
                message.push_str("; Rust seam `");
                message.push_str(&grip.rust_file);
                message.push(':');
                message.push_str(&grip.rust_owner);
                message.push('`');
                message.push_str("; action `");
                message.push_str(&grip.action);
                message.push_str("`; suggested test file `");
                message.push_str(&grip.suggested_test_file);
                message.push_str("` (preview advisory).");
                if let Some(placement) = &grip.placement {
                    message.push_str(" TypeScript placement: rank ");
                    message.push_str(&placement.rank.to_string());
                    message.push_str(" `");
                    message.push_str(&placement.suggested_test_file);
                    message.push_str("`; reason `");
                    message.push_str(&placement.reason);
                    message.push_str("` (preview advisory).");
                }
            }
        }
        // ADR-0019 §83-86 bespoke path (Campaign 31 item 6 formal scope-down):
        // this `perl_preview_card` call is a bespoke renderer that violates
        // ADR-0019's "no bespoke packet renderer" rule. It remains because the
        // shared path (`perl_gap_record_for` → `validate_agent_gap_record_packet`)
        // returns `None` for real Perl findings until perl-lsp-swarm Phase B.
        // It is hard-pinned to advisory-only (`advisory_only_readiness()`); the
        // regression test `perl_preview_card_advisory_only_readiness_never_flips_authority`
        // proves no authority flag can flip true. Full decommissioning is the
        // post-Phase-B PR.
        if let Some(card) = perl_preview_card(finding) {
            message.push_str(" Perl preview card: missing discriminator `");
            message.push_str(&card.missing_discriminator);
            message.push_str("`; add `");
            message.push_str(&card.suggested_assertion);
            message.push_str("` at `");
            message.push_str(&card.suggested_test_location);
            message.push_str("`; verify `");
            message.push_str(&card.verify_command);
            message.push_str("` (preview advisory; no repair packet).");
        }
        *per_level.entry(annotation_level).or_default() += 1;
        // GitHub workflow-command `line` is 1-indexed (documented default 1).
        // A line-0 probe is unlocated — the same class SARIF already omits
        // `region` for (#2407). Omit `,line=` so the command does not emit
        // out-of-contract `line=0` (#5089). This does not claim GitHub UI
        // file-level placement; omitted `line` may still default to 1.
        let line_property = match finding.probe.location.line {
            0 => String::new(),
            line => format!(",line={line}"),
        };
        annotations.push_str(&format!(
            "::{annotation_level} file={}{line_property},title={}::{}\n",
            // `file` arrives via `repository_display_path` (stable text, `%`
            // pre-encoded); `title` is raw text.
            escape_property_pre_encoded(&repository_display_path(
                &output.root,
                &finding.probe.location.file
            )),
            escape_property(&title),
            escape_data(&message)
        ));
    }
    // Disclosures lead the stream: GitHub keeps only the first
    // `GITHUB_ANNOTATIONS_PER_LEVEL` annotations of each level per step, so
    // a trailing notice is the first line dropped on a busy run.
    let mut out = String::new();
    let incomplete = output
        .analysis_outcome
        .as_ref()
        .filter(|outcome| !outcome.kind.is_complete());
    // #5011: an empty stream is only "clean" when nothing was left
    // unanalyzed. Run states the human and JSON surfaces disclose as NOT
    // clean (uncommitted working-tree edits, missing analysis scope,
    // preview-language files detected but not analyzed) downgrade the
    // empty-stream notice to warnings naming the unanalyzed surface and
    // the remedy route. These lead the stream like the other disclosures.
    let state_warnings = unanalyzed_state_warnings(output);
    if let Some(outcome) = incomplete {
        *per_level.entry("warning").or_default() += 1;
        out.push_str(&incomplete_outcome_warning(
            outcome,
            output.findings.is_empty(),
        ));
    } else if output.findings.is_empty() && state_warnings.is_empty() {
        out.push_str("::notice title=ripr::No static exposure findings found\n");
    }
    for warning in &state_warnings {
        *per_level.entry("warning").or_default() += 1;
        out.push_str(warning);
    }
    if !output.findings.is_empty() && (suppressed > 0 || not_current > 0) {
        out.push_str(&unannotated_denominator_notice(
            output,
            suppressed,
            not_current,
        ));
    }
    if let Some(notice) = display_limit_notice(&per_level) {
        out.push_str(&notice);
    }
    out.push_str(&annotations);
    out
}

/// An incomplete analysis must not read as a clean one in a PR check: the
/// annotation stream is often the only thing a reviewer sees, so it names
/// the outcome and each limitation's recovery, as the human report does.
fn incomplete_outcome_warning(
    outcome: &crate::analysis_outcome::AnalysisOutcome,
    no_findings: bool,
) -> String {
    let scope = if no_findings {
        "Zero findings is not a clean result because the analyzed scope is incomplete."
    } else {
        "The findings cover only the analyzed scope; behavior outside it has no finding."
    };
    let mut message = format!(
        "Analysis outcome: {} (analysis incomplete). {scope}",
        outcome.kind.as_str()
    );
    for limitation in &outcome.limitations {
        message.push_str(&format!(
            " Limitation: {}: {}",
            limitation.kind.as_str(),
            limitation.recovery.detail
        ));
    }
    format!(
        "::warning title=ripr analysis incomplete::{}\n",
        escape_data(&message)
    )
}

/// The preview suffix must track the shared validator's own verdict, as the
/// human and JSON surfaces do (#5014): an approved packet is complete (the
/// preview stays advisory), a blocked one has no repair packet. Claiming
/// "no repair packet" for a validator-approved packet would route the
/// operator to a manual step the packet was built to avoid.
fn advisory_packet_suffix(repair_packet_ready: bool) -> &'static str {
    if repair_packet_ready {
        " (advisory preview; complete repair packet)."
    } else {
        " (advisory preview; no repair packet)."
    }
}

/// Run states the human and JSON surfaces disclose as NOT clean, so an
/// empty GitHub stream must not read as all-clear either (#5011). Each
/// warning names the unanalyzed surface and the remedy route, mirroring
/// the human notes; the preview-advisory arm reuses the JSON surface's
/// shared `why` strings so the machine surfaces cannot drift.
fn unanalyzed_state_warnings(output: &CheckOutput) -> Vec<String> {
    let mut warnings = Vec::new();
    if output.unanalyzed_working_tree {
        warnings.push(
            "::warning title=ripr unanalyzed working tree::Uncommitted source and test changes were not analyzed; `ripr check` reads each file as committed at HEAD. An empty result here does NOT mean those changes are covered; add `--worktree` to include staged and unstaged edits (for example `ripr check --worktree`).\n"
                .to_string(),
        );
    }
    if output.no_scope_provided {
        let message = if let Some(base) = output.base.as_deref() {
            format!(
                "`{base}...HEAD` contains no changed files, so there was nothing to analyze; the compared base was `{base}`. An empty result means no behavior changed against it — it does NOT mean your changes are covered."
            )
        } else {
            "No analysis scope was provided — `ripr check` is diff-first. Run `ripr check --base BASE` with BASE set to an existing ref to analyze your changes. An empty result here does NOT mean your changed behavior is covered."
                .to_string()
        };
        warnings.push(format!(
            "::warning title=ripr no analysis scope::{}\n",
            escape_data(&message)
        ));
    }
    for advisory in &output.preview_language_advisories {
        if advisory.analyzed(&output.language_runs) {
            continue;
        }
        let why = advisory.unaudited_why(&output.language_runs);
        let (_language, file_language) = crate::output::human::advisory_language_names(advisory);
        let file_label = if advisory.file_count == 1 {
            "file"
        } else {
            "files"
        };
        let message = format!(
            "{} {} {} in scope: {}",
            advisory.file_count, file_language, file_label, why
        );
        warnings.push(format!(
            "::warning title=ripr preview advisory::{}\n",
            escape_data(&message)
        ));
    }
    warnings
}

/// GitHub Actions displays at most this many annotations of each level
/// (error, warning, notice) per step and drops the rest without a trace.
/// See actions/toolkit docs/problem-matchers.md "Limitations".
const GITHUB_ANNOTATIONS_PER_LEVEL: usize = 10;

/// Name the annotations GitHub will not display, so a truncated run does
/// not read as a complete one. Counted against the display budget itself,
/// the limit applies to this notice too, which is why it is emitted first.
fn display_limit_notice(
    per_level: &std::collections::BTreeMap<&'static str, usize>,
) -> Option<String> {
    let over = per_level
        .iter()
        .filter(|(_, count)| **count > GITHUB_ANNOTATIONS_PER_LEVEL)
        .map(|(level, count)| format!("{count} {level}"))
        .collect::<Vec<_>>();
    if over.is_empty() {
        return None;
    }
    let message = format!(
        "Emitted {} annotations; GitHub displays at most {GITHUB_ANNOTATIONS_PER_LEVEL} per level in one step, so some are not shown. Run `ripr check --format json` to list every finding.",
        over.join(" and ")
    );
    Some(format!("::notice title=ripr::{}\n", escape_data(&message)))
}

/// Denominator for findings the annotation stream deliberately omits (#4393).
///
/// Without it, a run whose findings are all policy-suppressed or all
/// base-side prints nothing, which a reader cannot tell apart from a clean
/// run. The omitted findings stay unannotated; this line only counts them.
fn unannotated_denominator_notice(
    output: &CheckOutput,
    suppressed: usize,
    not_current: usize,
) -> String {
    let total = output.findings.len();
    let annotated = total - suppressed - not_current;
    let mut parts = Vec::new();
    if suppressed > 0 {
        let policy = output
            .suppression
            .as_ref()
            .map(|outcome| outcome.policy_path.as_str())
            .unwrap_or_default();
        parts.push(format!("{suppressed} suppressed by policy {policy}"));
    }
    if not_current > 0 {
        parts.push(format!(
            "{not_current} not current in this change (base-side or unresolved evidence)"
        ));
    }
    let message = format!(
        "Annotated {annotated} of {total} static exposure finding(s); {}. Run `ripr check --format json` to list every finding.",
        parts.join("; ")
    );
    format!("::notice title=ripr::{}\n", escape_data(&message))
}

fn python_no_action_annotation(finding: &Finding) -> Option<String> {
    if finding.language != Some(LanguageId::Python)
        || finding.language_status != Some(LanguageStatus::Preview)
    {
        return None;
    }
    if let Some(no_action_kind) = python_ordinary_no_action_kind(finding) {
        return Some(format!(
            " Python no-action: {no_action_kind}; {} No repair card or agent packet emitted (preview advisory).",
            python_ordinary_no_action_reason(no_action_kind)
        ));
    }

    let static_limit_kind = finding.static_limit_kind?;
    let static_limit_kind = static_limit_kind.as_str();
    Some(format!(
        " Python no-action: static_limit `{static_limit_kind}`; {} No repair card or agent packet emitted (preview advisory).",
        python_static_limit_detail(finding, static_limit_kind)
    ))
}

fn python_ordinary_no_action_kind(finding: &Finding) -> Option<&'static str> {
    match &finding.class {
        ExposureClass::Exposed => Some("already_observed"),
        ExposureClass::NoStaticPath => Some("no_related_test"),
        ExposureClass::WeaklyExposed if python_finding_is_heuristic_only(finding) => {
            Some("heuristic_only")
        }
        _ => None,
    }
}

fn python_finding_is_heuristic_only(finding: &Finding) -> bool {
    finding
        .evidence
        .iter()
        .any(|item| item.starts_with("related_test_uncertain:"))
        || finding
            .ripr
            .reach
            .summary
            .contains("heuristic Python test link")
}

fn python_ordinary_no_action_reason(no_action_kind: &str) -> &'static str {
    match no_action_kind {
        "already_observed" => {
            "Current Python test evidence already observes the changed behavior; no missing proof was routed."
        }
        "no_related_test" => {
            "No related Python test was statically linked, so RIPR cannot choose a safe edit target."
        }
        "heuristic_only" => {
            "Only heuristic Python related-test proximity was found, so bounded repair routing would overclaim."
        }
        _ => "Python preview did not find a bounded repair route.",
    }
}

fn python_static_limit_detail(finding: &Finding, static_limit_kind: &str) -> String {
    finding
        .missing
        .iter()
        .find_map(|detail| non_empty(detail).map(ToString::to_string))
        .or_else(|| {
            finding
                .evidence
                .iter()
                .find(|detail| {
                    detail.contains("static_limit") || detail.contains(static_limit_kind)
                })
                .cloned()
        })
        .unwrap_or_else(|| {
            format!(
                "Python preview reported static limit `{static_limit_kind}` without a bounded repair route."
            )
        })
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::app::{CheckOutput, Mode};
    use crate::domain::{
        Confidence, DeltaKind, ExposureClass, Finding, FindingCanonicalGap, LanguageId,
        LanguageStatus, MissingDiscriminatorFact, OracleKind, OracleStrength, Probe, ProbeFamily,
        ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence,
        StageState, StaticLimitKind, StopReason, Summary, SymbolId,
    };
    use std::path::PathBuf;

    #[test]
    fn render_reports_empty_findings_as_notice() {
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

        assert_eq!(
            rendered,
            "::notice title=ripr::No static exposure findings found\n"
        );
    }

    fn partial_outcome() -> Result<crate::analysis_outcome::AnalysisOutcome, String> {
        use crate::analysis_outcome::{
            AnalysisIdentity, AnalysisLimitation, AnalysisLimitationKind, AnalysisOutcome,
            AnalysisOutcomeCounts, AnalysisOutcomeKind, AnalysisRecovery, AnalysisRecoveryKind,
            AnalysisStage,
        };
        AnalysisOutcome::new(
            AnalysisOutcomeKind::PartialWithLimitations,
            AnalysisIdentity::default(),
            AnalysisOutcomeCounts {
                changed_file_count: 1,
                changed_line_count: 2,
                ..AnalysisOutcomeCounts::default()
            },
            vec![AnalysisLimitation::new(
                AnalysisLimitationKind::LanguageScopeUnsupported,
                AnalysisStage::LanguageAdapter,
                AnalysisRecovery::new(
                    AnalysisRecoveryKind::InspectFailure,
                    "Not analyzed (Go: 1): pkg/calc.go.",
                )?,
            )],
        )
    }

    #[test]
    fn render_never_calls_an_incomplete_analysis_clean() -> Result<(), String> {
        // A Perl- or Go-only diff is `partial_with_limitations`; the GitHub
        // stream printed only "No static exposure findings found".
        let mut output = output_with_unknown_finding();
        output.findings.clear();
        output.analysis_outcome = Some(partial_outcome()?);

        let rendered = render(&output);

        assert!(
            !rendered.contains("No static exposure findings found"),
            "{rendered}"
        );
        assert!(
            rendered.starts_with("::warning title=ripr analysis incomplete::Analysis outcome: partial_with_limitations"),
            "{rendered}"
        );
        assert!(
            rendered.contains("Zero findings is not a clean result"),
            "{rendered}"
        );
        assert!(
            rendered.contains(
                "Limitation: language_scope_unsupported: Not analyzed (Go: 1): pkg/calc.go."
            ),
            "{rendered}"
        );
        Ok(())
    }

    #[test]
    fn render_scopes_findings_of_an_incomplete_analysis() -> Result<(), String> {
        let mut output = output_with_unknown_finding();
        output.analysis_outcome = Some(partial_outcome()?);

        let rendered = render(&output);

        let first = rendered.lines().next().unwrap_or_default();
        assert!(
            first.contains("findings cover only the analyzed scope"),
            "{rendered}"
        );
        assert!(
            rendered.contains("title=ripr static_unknown::"),
            "{rendered}"
        );
        Ok(())
    }

    #[test]
    fn render_escapes_annotations_and_includes_effective_stop_reason_for_unknowns() {
        let rendered = render(&output_with_unknown_finding());

        assert!(rendered.contains("::notice file=src/lib.rs,line=13,title=ripr static_unknown::"));
        // Message (data) encoding: comma/colon stay literal, percent/CR/LF
        // escaped. The previous assertions pinned %3A/%2C in the message,
        // which the workflow-command parser does not decode in data —
        // visible scars (#4065).
        assert!(rendered.contains("Add: case, with 100%25 coverage%0Athen verify%0Doutcome"));
        assert!(rendered.contains("Stop reason: static_probe_unknown"));
    }

    /// Property section of a workflow-command line (`file=...,title=...`),
    /// excluding the message so a message containing `line=` cannot pass.
    fn annotation_properties(command: &str) -> &str {
        let rest = command.strip_prefix("::").unwrap_or(command);
        let Some((level_and_props, _)) = rest.split_once("::") else {
            return "";
        };
        level_and_props
            .split_once(' ')
            .map(|(_, props)| props)
            .unwrap_or("")
    }

    fn finding_annotation_commands(rendered: &str) -> Vec<&str> {
        rendered
            .lines()
            .filter(|line| {
                line.starts_with("::notice file=")
                    || line.starts_with("::warning file=")
                    || line.starts_with("::error file=")
            })
            .collect()
    }

    #[test]
    fn github_annotations_omit_line_property_for_line_zero_probe() {
        let mut output = output_with_unknown_finding();
        output.findings[0].probe.location.line = 0;

        let rendered = render(&output);
        let commands = finding_annotation_commands(&rendered);
        assert_eq!(
            commands.len(),
            1,
            "expected one finding annotation: {rendered}"
        );
        let properties = annotation_properties(commands[0]);
        assert!(
            commands[0].starts_with("::notice file=src/lib.rs,title=ripr static_unknown::"),
            "line-0 probe must omit `,line=` from the command: {rendered}"
        );
        assert!(
            !properties.contains("line="),
            "line-0 command properties must not contain `line=` (neither `line=0` nor `,line=1`): {rendered}"
        );
        assert_eq!(
            properties, "file=src/lib.rs,title=ripr static_unknown",
            "file and title stay; line is absent: {rendered}"
        );
    }

    #[test]
    fn github_annotations_include_line_property_for_located_probe() {
        let rendered = render(&output_with_unknown_finding());
        let commands = finding_annotation_commands(&rendered);
        assert_eq!(commands.len(), 1, "{rendered}");
        assert!(
            commands[0].starts_with("::notice file=src/lib.rs,line=13,title=ripr static_unknown::"),
            "located probe must keep `,line=13`: {rendered}"
        );
        assert!(
            annotation_properties(commands[0]).contains(",line=13,"),
            "{rendered}"
        );
    }

    #[test]
    fn github_annotations_keep_line_one_as_located() {
        let mut output = output_with_unknown_finding();
        output.findings[0].probe.location.line = 1;

        let rendered = render(&output);
        let commands = finding_annotation_commands(&rendered);
        assert_eq!(commands.len(), 1, "{rendered}");
        assert!(
            annotation_properties(commands[0]).contains(",line=1,"),
            "line 1 is in-contract and must not be omitted as if it were 0: {rendered}"
        );
    }

    #[test]
    fn github_annotations_omit_line_only_for_the_zero_probe_in_a_mixed_stream() {
        let mut output = output_with_unknown_finding();
        let mut unlocated = output.findings[0].clone();
        unlocated.id = "probe:src_lib_rs:0:static_unknown".to_string();
        unlocated.probe.id = ProbeId(unlocated.id.clone());
        unlocated.probe.location.line = 0;
        output.findings.push(unlocated);

        let rendered = render(&output);
        let commands = finding_annotation_commands(&rendered);
        assert_eq!(commands.len(), 2, "{rendered}");
        assert!(
            annotation_properties(commands[0]).contains(",line=13,"),
            "located finding stays located: {rendered}"
        );
        assert!(
            !annotation_properties(commands[1]).contains("line="),
            "unlocated finding omits `line=` from the command: {rendered}"
        );
    }

    #[test]
    fn render_message_keeps_data_punctuation_literal() {
        // GitHub's workflow-command parser decodes data (the message) with
        // percent/CR/LF only: comma and colon arrive literally. Encoding
        // them as %2C/%3A leaves visible percent-escapes in the rendered
        // annotation (#4065).
        let mut output = output_with_unknown_finding();
        output.findings[0].recommended_next_step = Some(
            "Check Result::Err from assert_eq!(actual, expected) at 100% — naïve ünïcode"
                .to_string(),
        );

        let rendered = render(&output);

        assert!(
            rendered.contains("Check Result::Err from assert_eq!(actual, expected) at 100%25"),
            "message punctuation must stay literal with only percent escaped: {rendered}"
        );
        assert!(
            rendered.contains("naïve ünïcode"),
            "unicode message bytes must pass through unescaped: {rendered}"
        );
        assert!(
            !rendered.contains("%2C") && !rendered.contains("%3A"),
            "old data encoding must fail: {rendered}"
        );
        assert_eq!(
            rendered.lines().count(),
            1,
            "one annotation must stay one physical line: {rendered}"
        );
    }

    #[test]
    fn render_file_property_escapes_comma_colon_percent() {
        // Properties (file, title) decode comma/colon/percent in addition
        // to CR/LF: an unescaped comma splits the filename into metadata
        // (#4065). Unicode passes through raw — the escape map has no
        // UTF-8 branch.
        let mut output = output_with_unknown_finding();
        output.findings[0].probe.location.file = PathBuf::from("src/a,b:c%dé.rs");

        let rendered = render(&output);

        assert!(
            rendered.contains("file=src/a%2Cb%3Ac%25dé.rs,line=13"),
            "file property must escape comma/colon/percent, keep unicode literal: {rendered}"
        );
    }

    #[test]
    fn render_literal_percent_sequences_survive_one_decode() {
        // A literal `%0A` in the source text must arrive as `%250A`: the
        // percent itself is escaped, so one parser decode yields the
        // original `%0A` instead of a newline. Blanket URL-decoding the
        // input would be the wrong repair (#4065).
        let mut output = output_with_unknown_finding();
        output.findings[0].recommended_next_step =
            Some("Expect literal %0A and %2C tokens".to_string());

        let rendered = render(&output);

        assert!(
            rendered.contains("Expect literal %250A and %252C tokens"),
            "literal percent sequences must be percent-escaped once: {rendered}"
        );
    }

    #[test]
    fn render_github_paths_are_repo_relative_without_dot_prefix() {
        // Use a platform-correct absolute root so strip_prefix works on both
        // Unix (where /workspace/repo is absolute) and Windows (where it is
        // drive-relative and is_absolute() returns false).
        let abs_root = std::env::temp_dir().join("ripr_github_annotation_test");
        let mut output = output_with_unknown_finding();
        output.root = abs_root.clone();
        output.findings[0].probe.location.file = abs_root.join(".").join("crates/ripr/src/lib.rs");

        let rendered = render(&output);

        assert!(rendered.contains("file=crates/ripr/src/lib.rs,line=13"));
        assert!(!rendered.contains("file=./"));
        assert!(
            !rendered.contains("file=") || !rendered.contains(&abs_root.display().to_string()),
            "absolute root must not appear in the annotation file path: {rendered}"
        );

        let mut nested = output_with_unknown_finding();
        nested.root = PathBuf::from("fixtures/boundary_gap/input");
        nested.findings[0].probe.location.file =
            PathBuf::from("fixtures/boundary_gap/input/src/lib.rs");

        let nested_rendered = render(&nested);

        assert!(nested_rendered.contains("file=fixtures/boundary_gap/input/src/lib.rs,line=13"));
    }

    #[test]
    fn render_uses_notice_for_exposed_and_default_message_without_stop_reason() {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![Finding {
                id: "probe:src_lib_rs:21:error_path".to_string(),
                canonical_gap: None,
                probe: Probe {
                    id: ProbeId("probe:src_lib_rs:21:error_path".to_string()),
                    location: SourceLocation::new("src/lib.rs", 21, 1),
                    owner: None,
                    family: ProbeFamily::ErrorPath,
                    delta: DeltaKind::Control,
                    before: Some("ok".to_string()),
                    after: Some("err".to_string()),
                    expression: "result".to_string(),
                    expected_sinks: vec![],
                    required_oracles: vec![],
                },
                class: ExposureClass::Exposed,
                ripr: RiprEvidence {
                    reach: stage(StageState::Yes, "reachable"),
                    infect: stage(StageState::Yes, "infected"),
                    propagate: stage(StageState::Yes, "propagated"),
                    reveal: RevealEvidence {
                        observe: stage(StageState::Yes, "observed"),
                        discriminate: stage(StageState::Yes, "discriminated"),
                    },
                },
                confidence: 1.0,
                evidence: vec![],
                missing: vec![],
                flow_sinks: vec![],
                activation: crate::domain::ActivationEvidence::default(),
                stop_reasons: vec![],
                related_tests_matched_total: None,
                related_tests: vec![],
                recommended_next_step: None,
                language: None,
                language_status: None,
                owner_kind: None,
                static_limit_kind: None,
                changed_sink: None,
                observed_sink: None,
                oracle_alignment: None,
                alignment_reason: None,
                source_currentness: crate::domain::SourceCurrentness::CandidateCurrent,
            }],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        // An exposed finding is already discriminated; it annotates as a
        // notice, not a warning, by default.
        assert!(rendered.contains("::notice file=src/lib.rs,line=21,title=ripr exposed::"));
        assert!(!rendered.contains("::warning "));
        assert!(rendered.contains("Static RIPR exposure finding"));
        assert!(!rendered.contains("Stop reason"));
    }

    #[test]
    fn render_uses_warning_annotation_for_warning_severity_findings() {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![Finding {
                id: "probe:src_lib_rs:34:weak_signal".to_string(),
                canonical_gap: None,
                probe: Probe {
                    id: ProbeId("probe:src_lib_rs:34:weak_signal".to_string()),
                    location: SourceLocation::new("src/lib.rs", 34, 1),
                    owner: None,
                    family: ProbeFamily::Predicate,
                    delta: DeltaKind::Control,
                    before: Some("x > 0".to_string()),
                    after: Some("x >= 0".to_string()),
                    expression: "x > 0".to_string(),
                    expected_sinks: vec![],
                    required_oracles: vec![],
                },
                class: ExposureClass::WeaklyExposed,
                ripr: RiprEvidence {
                    reach: stage(StageState::Yes, "reachable"),
                    infect: stage(StageState::Yes, "infected"),
                    propagate: stage(StageState::Yes, "propagated"),
                    reveal: RevealEvidence {
                        observe: stage(StageState::Yes, "observed"),
                        discriminate: stage(StageState::No, "not discriminated"),
                    },
                },
                confidence: 0.7,
                evidence: vec![],
                missing: vec![],
                flow_sinks: vec![],
                activation: crate::domain::ActivationEvidence::default(),
                stop_reasons: vec![],
                related_tests_matched_total: None,
                related_tests: vec![],
                recommended_next_step: Some("Add discriminator assertion".to_string()),
                language: None,
                language_status: None,
                owner_kind: None,
                static_limit_kind: None,
                changed_sink: None,
                observed_sink: None,
                oracle_alignment: None,
                alignment_reason: None,
                source_currentness: crate::domain::SourceCurrentness::CandidateCurrent,
            }],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        };

        let rendered = render(&output);

        assert!(rendered.contains("::warning file=src/lib.rs,line=34,title=ripr weakly_exposed::"));
        assert!(rendered.contains("Add discriminator assertion"));
    }

    #[test]
    fn render_includes_canonical_gap_id_when_present() {
        let mut output = output_with_unknown_finding();
        output.findings[0].canonical_gap = Some(FindingCanonicalGap {
            id: "gap:python:src/pricing.py:discount:predicate_boundary:predicate:amount>=threshold"
                .to_string(),
            language: "python".to_string(),
            file: "src/pricing.py".to_string(),
            owner: "discount".to_string(),
            behavior_kind: "predicate_boundary".to_string(),
            probe_kind: "predicate".to_string(),
            normalized_discriminator: "amount>=threshold".to_string(),
        });

        let rendered = render(&output);

        assert!(rendered.contains(
            "Canonical gap: gap:python:src/pricing.py:discount:predicate_boundary:predicate:amount>=threshold"
        ));
    }

    #[test]
    fn render_includes_preview_actionability_boundary() {
        let mut output = output_with_unknown_finding();
        let finding = &mut output.findings[0];
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.evidence = vec![
            "gap_state: advisory".to_string(),
            "actionability_category: incomplete_repair_packet".to_string(),
            "why_not_actionable: TypeScript preview lacks a complete repair packet contract"
                .to_string(),
            "repair_route: project canonical TypeScript repair packet fields later".to_string(),
            "evidence_needed_to_promote: canonical gap identity and verify command".to_string(),
            "raw_evidence_ref: file=src/lib.ts;line=2;kind=typescript_preview_probe;source_id=probe:src_lib.ts:2:typescript_preview;owner=discountedTotal".to_string(),
        ];

        let rendered = render(&output);

        assert!(rendered.contains(
            "Preview actionability: advisory/incomplete_repair_packet (advisory preview; no repair packet)."
        ));
    }

    #[test]
    fn render_discloses_unanalyzed_working_tree_instead_of_clean_notice() {
        // #5011: the human surface prints UNANALYZED_WORKING_TREE_NOTE and
        // JSON emits "unanalyzed_working_tree": true; the GitHub stream must
        // not read as clean for the same CheckOutput. The full stream is
        // pinned by the fixtures/github_unanalyzed_states golden.
        let mut output = output_with_unknown_finding();
        output.findings.clear();
        output.unanalyzed_working_tree = true;

        let rendered = render(&output);

        assert!(
            !rendered.contains("No static exposure findings found"),
            "{rendered}"
        );
        assert_eq!(
            rendered,
            include_str!(
                "../../../../fixtures/github_unanalyzed_states/expected/unanalyzed_working_tree.txt"
            )
        );
    }

    #[test]
    fn render_discloses_no_scope_provided_instead_of_clean_notice() {
        // #5011: mirror the human no-scope note; the empty-range variant
        // (#4012) names the compared base instead. The full stream for the
        // no-base case is pinned by the fixtures/github_unanalyzed_states
        // golden.
        let mut output = output_with_unknown_finding();
        output.findings.clear();
        output.no_scope_provided = true;

        let rendered = render(&output);

        assert!(
            !rendered.contains("No static exposure findings found"),
            "{rendered}"
        );
        assert_eq!(
            rendered,
            include_str!(
                "../../../../fixtures/github_unanalyzed_states/expected/no_scope_provided.txt"
            )
        );

        let mut ranged = output_with_unknown_finding();
        ranged.findings.clear();
        ranged.no_scope_provided = true;
        ranged.base = Some("main".to_string());

        let rendered = render(&ranged);

        assert!(
            !rendered.contains("No static exposure findings found"),
            "{rendered}"
        );
        assert!(
            rendered.contains("`main...HEAD` contains no changed files"),
            "{rendered}"
        );
        assert!(rendered.contains("compared base was `main`"), "{rendered}");
    }

    #[test]
    fn render_discloses_not_analyzed_preview_advisory_instead_of_clean_notice() {
        // #5011: a detected-but-not-analyzed preview advisory records no
        // successful LanguageRun, so the outcome stays "complete" — but the
        // files were not analyzed and the human/JSON/badge surfaces all say
        // the result is NOT clean. The wire string is deliberately
        // unregistered so the recovery text is identical in every
        // feature-lane build (real adapters vary with compile-time
        // features); the full stream is pinned by the
        // fixtures/github_unanalyzed_states golden.
        let mut output = output_with_unknown_finding();
        output.findings.clear();
        output.preview_language_advisories = vec![crate::analysis::PreviewLanguageAdvisory {
            language: "cobol".to_string(),
            file_count: 2,
            sample_paths: vec!["src/a.cobol".to_string(), "src/b.cobol".to_string()],
            javascript_file_count: 0,
            enabled: false,
        }];

        let rendered = render(&output);

        assert!(
            !rendered.contains("No static exposure findings found"),
            "{rendered}"
        );
        assert_eq!(
            rendered,
            include_str!(
                "../../../../fixtures/github_unanalyzed_states/expected/preview_advisory.txt"
            )
        );

        // Real-language arms: file labels follow the human surface's prose
        // names (#4555), and the recovery text is feature-dependent, so these
        // assert feature-independent fragments only.
        let mut typescript = output_with_unknown_finding();
        typescript.findings.clear();
        typescript.preview_language_advisories = vec![crate::analysis::PreviewLanguageAdvisory {
            language: "typescript".to_string(),
            file_count: 2,
            sample_paths: vec!["src/a.ts".to_string(), "src/b.ts".to_string()],
            javascript_file_count: 0,
            enabled: false,
        }];

        let rendered = render(&typescript);

        assert!(
            !rendered.contains("No static exposure findings found"),
            "{rendered}"
        );
        assert!(
            rendered.starts_with("::warning title=ripr preview advisory::"),
            "{rendered}"
        );
        assert!(rendered.contains("2 TypeScript files"), "{rendered}");
        assert!(
            rendered.contains("files detected but not analyzed"),
            "{rendered}"
        );
        assert!(rendered.contains("not Rust-grade clean"), "{rendered}");

        let mut javascript = output_with_unknown_finding();
        javascript.findings.clear();
        javascript.preview_language_advisories = vec![crate::analysis::PreviewLanguageAdvisory {
            language: "typescript".to_string(),
            file_count: 1,
            sample_paths: vec!["src/a.js".to_string()],
            javascript_file_count: 1,
            enabled: false,
        }];

        let rendered = render(&javascript);

        assert!(
            rendered.contains("1 JavaScript file in scope"),
            "JavaScript-only files take the JavaScript label: {rendered}"
        );
    }

    #[test]
    fn render_preview_actionability_suffix_tracks_repair_packet_readiness() {
        // #5014: the suffix tracks the shared validator's verdict — an
        // approved packet is complete (still advisory); a blocked packet has
        // no repair packet.
        let mut ready = output_with_unknown_finding();
        ready.findings[0] = typescript_preview_finding(true);

        let rendered = render(&ready);

        assert!(
            rendered.contains(
                "Preview actionability: actionable/complete_repair_packet (advisory preview; complete repair packet)."
            ),
            "{rendered}"
        );
        assert!(
            !rendered.contains("no repair packet"),
            "a validator-approved packet must not be called absent: {rendered}"
        );

        let mut blocked = output_with_unknown_finding();
        blocked.findings[0] = typescript_preview_finding(false);

        let rendered = render(&blocked);

        assert!(
            rendered.contains(
                "Preview actionability: advisory/incomplete_repair_packet (advisory preview; no repair packet)."
            ),
            "{rendered}"
        );
    }

    #[test]
    fn render_typescript_card_suffix_tracks_repair_packet_readiness() {
        let mut ready = output_with_unknown_finding();
        ready.findings[0] = typescript_preview_finding(true);

        let rendered = render(&ready);

        assert!(
            rendered.contains("TypeScript preview card: owner `applyDiscount`"),
            "{rendered}"
        );
        assert!(
            rendered.contains("(advisory preview; complete repair packet)."),
            "{rendered}"
        );
        assert!(
            !rendered.contains("no repair packet"),
            "a validator-approved packet must not be called absent: {rendered}"
        );

        let mut blocked = output_with_unknown_finding();
        blocked.findings[0] = typescript_preview_finding(false);

        let rendered = render(&blocked);

        assert!(
            rendered.contains("(advisory preview; no repair packet)."),
            "a blocked packet keeps the pinned suffix: {rendered}"
        );
    }

    #[test]
    fn render_includes_bun_cross_language_grip_annotation() {
        let mut output = output_with_unknown_finding();
        let finding = &mut output.findings[0];
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.evidence = vec![
            "owner: Blob::from_js_without_defer_gc".to_string(),
            "gap_state: static_limitation".to_string(),
            "actionability_category: cross_language_oracle_visibility_unresolved".to_string(),
            "why_not_actionable: TypeScript cross-language preview is a named limitation until the external oracle path is visible".to_string(),
            "repair_route: analysis/cross-language-oracle-visibility".to_string(),
            "evidence_needed_to_promote: bridge calibration and non-preview repair packet contract"
                .to_string(),
            "typescript_bun_ub_bridge_hint: confidence=configured_hint rust_file=src/jsc/Blob.rs rust_owner=Blob::from_js_without_defer_gc rust_boundary=\"array_buffer.shared || array_buffer.resizable\" ts_test_file=test/js/web/fetch/blob.test.ts".to_string(),
            "typescript_bun_ub_bridge_verdict: ts_missing_resizable missing_discriminators=resizable_array_buffer action=route_cross_language_oracle_visibility_limitation suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_missing_discriminator rust_grip=ungripped ts_verdict=ts_missing_resizable action=route_cross_language_oracle_visibility_limitation authority=preview_advisory_only suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_test_placement: rank=1 suggested_test_file=test/js/web/fetch/blob.test.ts reason=\"existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable ArrayBuffer\" basis=configured_bridge_suggested_test_file,same_js_surface,same_boundary_vocabulary authority=preview_advisory_only repair_packet_ready=false".to_string(),
            "typescript_bun_ub_bridge_hint: confidence=configured_hint rust_file=src/jsc/array_buffer.rs rust_owner=copy_to_unshared rust_boundary=\"array_buffer.shared || array_buffer.resizable\" ts_test_file=test/js/web/fetch/blob.test.ts".to_string(),
            "typescript_bun_ub_bridge_verdict: ts_missing_shared missing_discriminators=shared_array_buffer action=route_cross_language_oracle_visibility_limitation suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_missing_discriminator rust_grip=ungripped ts_verdict=ts_missing_shared action=route_cross_language_oracle_visibility_limitation authority=preview_advisory_only suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
        ];

        let rendered = render(&output);

        assert!(
            rendered
                .contains("Bun cross-language grip 1/2: rust_ungripped_ts_missing_discriminator")
        );
        assert!(
            rendered
                .contains("Bun cross-language grip 2/2: rust_ungripped_ts_missing_discriminator")
        );
        assert!(rendered.contains("copy_to_unshared"));
        assert!(rendered.contains("action `route_cross_language_oracle_visibility_limitation`"));
        assert!(rendered.contains("suggested test file `test/js/web/fetch/blob.test.ts`"));
        assert!(rendered.contains("TypeScript placement: rank 1"));
        assert!(rendered.contains("missing discriminator is resizable ArrayBuffer"));
        assert_eq!(
            rendered.matches("TypeScript placement: rank 1").count(),
            1,
            "only the Blob profile may receive placement evidence"
        );
        let copy_profile = rendered
            .split("Bun cross-language grip 2/2:")
            .nth(1)
            .unwrap_or_default();
        assert!(!copy_profile.is_empty(), "expected copy profile annotation");
        assert!(
            !copy_profile.contains("TypeScript placement:"),
            "copy_to_unshared must not receive placement evidence"
        );
        assert!(rendered.contains("(preview advisory)."));
    }

    #[test]
    fn render_includes_python_repair_card_guidance() {
        let rendered = render(&output_with_python_repair_card());

        assert!(
            rendered.contains("Python repair card: missing discriminator `amount == threshold`")
        );
        assert!(rendered.contains("add or strengthen `test_calculate_discount_threshold_boundary` in `tests/test_pricing.py`"));
        assert!(rendered.contains(
            "verify `pytest tests/test_pricing.py::test_calculate_discount_threshold_boundary` (preview advisory)."
        ));
        assert!(!rendered.contains("Python no-action"));
    }

    #[test]
    fn render_includes_perl_preview_card_guidance() {
        let rendered = render(&output_with_perl_preview_card());

        assert!(rendered.contains("Perl preview card: missing discriminator `return_value`"));
        assert!(rendered.contains(
            "add `assert the exact returned `return_value` value` at `t/app.t::discount_smoke`"
        ));
        assert!(rendered.contains("verify `prove t/app.t` (preview advisory; no repair packet)."));
        assert!(!rendered.contains("ripr agent receipt --root"));
        assert!(!rendered.contains("perl_allowed_edit_boundary"));
        assert!(!rendered.contains("perl_forbidden_edit_boundary"));
        assert!(!rendered.contains("allowed edit"));
        assert!(!rendered.contains("forbidden edit"));
        assert!(!rendered.contains("perl_repair_card"));
        assert!(!rendered.contains("perl_internal_agent_packet"));
    }

    #[test]
    fn render_includes_python_ordinary_no_action_guidance() {
        let rendered = render(&output_with_python_no_action_findings());

        assert!(rendered.contains(
            "Python no-action: already_observed; Current Python test evidence already observes"
        ));
        assert!(rendered.contains(
            "Python no-action: no_related_test; No related Python test was statically linked"
        ));
        assert!(rendered.contains(
            "Python no-action: heuristic_only; Only heuristic Python related-test proximity was found"
        ));
        assert_eq!(
            rendered
                .matches("No repair card or agent packet emitted")
                .count(),
            3
        );
        assert!(!rendered.contains("Python repair card"));
    }

    #[test]
    fn render_includes_python_static_limit_no_action_guidance() {
        let rendered = render(&output_with_python_static_limit());

        assert!(rendered.contains(
            "Python no-action: static_limit `dynamic_dispatch`; Static limit `dynamic_dispatch` prevents bounded repair routing"
        ));
        assert!(rendered.contains("Stop reason: dynamic_dispatch_unresolved"));
        assert!(rendered.contains("No repair card or agent packet emitted (preview advisory)."));
        assert!(!rendered.contains("Python repair card"));
    }

    #[test]
    fn render_includes_python_static_limit_fallback_no_action_guidance() {
        let mut output = output_with_python_static_limit();
        let finding = &mut output.findings[0];
        finding.missing.clear();
        finding.evidence.clear();

        let rendered = render(&output);

        assert!(rendered.contains(
            "Python no-action: static_limit `dynamic_dispatch`; Python preview reported static limit `dynamic_dispatch` without a bounded repair route."
        ));
        assert!(rendered.contains("No repair card or agent packet emitted (preview advisory)."));
        assert!(!rendered.contains("Python repair card"));
    }

    #[test]
    fn render_skips_policy_suppressed_findings() {
        use crate::output::suppressions::{CheckSuppressionOutcome, SuppressedCheckFinding};
        let mut output = output_with_unknown_finding();
        let finding_id = output.findings[0].id.clone();
        output.suppression = Some(CheckSuppressionOutcome {
            policy_path: "policy/ripr-suppressions.toml".to_string(),
            suppressed: vec![SuppressedCheckFinding {
                finding_id,
                selector: "src/**".to_string(),
            }],
            warnings: Vec::new(),
        });

        let rendered = render(&output);

        // Deliberate re-pin of the former empty-string assertion: suppressed
        // findings stay unannotated, but all-suppressed is not silent (#4393).
        assert_eq!(
            rendered,
            "::notice title=ripr::Annotated 0 of 1 static exposure finding(s); 1 suppressed by policy policy/ripr-suppressions.toml. Run `ripr check --format json` to list every finding.\n",
        );
        assert!(!rendered.contains("file=src/lib.rs"));
        assert!(
            !rendered.contains("No static exposure findings found"),
            "all-suppressed is not an empty run: {rendered}"
        );
    }

    #[test]
    fn render_over_broad_src_policy_keeps_unmatched_annotation() {
        // Over-broad `src/**` hides every src finding; a tests/ finding
        // outside that glob must still annotate. This rejects both silence
        // and "any suppression blanks the whole stream" (#4393).
        use crate::output::suppressions::{CheckSuppressionOutcome, SuppressedCheckFinding};
        let mut output = output_with_unknown_finding();
        let template = output.findings[0].clone();
        let mut src_other = template.clone();
        src_other.id = "probe:src_other_rs:4:static_unknown".to_string();
        src_other.probe.location = SourceLocation::new("src/other.rs", 4, 1);
        let mut tests_other = template;
        tests_other.id = "probe:tests_other_rs:8:static_unknown".to_string();
        tests_other.probe.location = SourceLocation::new("tests/other.rs", 8, 1);
        let src_lib_id = output.findings[0].id.clone();
        output.findings = vec![output.findings[0].clone(), src_other, tests_other];
        output.suppression = Some(CheckSuppressionOutcome {
            policy_path: "policy/over-broad.toml".to_string(),
            suppressed: vec![
                SuppressedCheckFinding {
                    finding_id: src_lib_id,
                    selector: "src/**".to_string(),
                },
                SuppressedCheckFinding {
                    finding_id: "probe:src_other_rs:4:static_unknown".to_string(),
                    selector: "src/**".to_string(),
                },
            ],
            warnings: Vec::new(),
        });

        let rendered = render(&output);

        assert_eq!(
            rendered.lines().next(),
            Some(
                "::notice title=ripr::Annotated 1 of 3 static exposure finding(s); 2 suppressed by policy policy/over-broad.toml. Run `ripr check --format json` to list every finding."
            ),
            "{rendered}"
        );
        assert!(
            !rendered.contains("file=src/lib.rs") && !rendered.contains("file=src/other.rs"),
            "over-broad src/** must not annotate src findings: {rendered}"
        );
        assert!(
            rendered.contains("file=tests/other.rs"),
            "a finding outside the over-broad glob must stay annotated: {rendered}"
        );
        assert!(
            !rendered.contains("No static exposure findings found"),
            "{rendered}"
        );
    }

    #[test]
    fn render_discloses_annotations_past_github_display_limit_first() {
        let mut output = output_with_unknown_finding();
        let template = output.findings[0].clone();
        output.findings = (0..12)
            .map(|index| {
                let mut finding = template.clone();
                finding.probe.location.line = 100 + index;
                finding
            })
            .collect();

        let rendered = render(&output);
        let lines = rendered.lines().collect::<Vec<_>>();

        // The disclosure must precede the per-finding notices: GitHub keeps
        // the first ten notices of a step, so a trailing one is never shown.
        assert_eq!(
            lines.first().copied(),
            Some(
                "::notice title=ripr::Emitted 12 notice annotations; GitHub displays at most 10 per level in one step, so some are not shown. Run `ripr check --format json` to list every finding."
            ),
            "{rendered}"
        );
        assert_eq!(lines.len(), 13, "{rendered}");
    }

    #[test]
    fn render_limit_notice_fires_at_eleven_and_follows_the_denominator() {
        use crate::output::suppressions::{CheckSuppressionOutcome, SuppressedCheckFinding};
        let mut output = output_with_unknown_finding();
        let template = output.findings[0].clone();
        // Twelve findings, one suppressed: eleven notices remain, one past
        // the display limit, and the denominator notice is also due.
        output.findings = (0..12)
            .map(|index| {
                let mut finding = template.clone();
                finding.id = format!("finding-{index}");
                finding.probe.location.line = 100 + index;
                finding
            })
            .collect();
        output.suppression = Some(CheckSuppressionOutcome {
            policy_path: "policy/ripr-suppressions.toml".to_string(),
            suppressed: vec![SuppressedCheckFinding {
                finding_id: "finding-0".to_string(),
                selector: "src/**".to_string(),
            }],
            warnings: Vec::new(),
        });

        let rendered = render(&output);
        let lines = rendered.lines().collect::<Vec<_>>();

        assert_eq!(
            lines.get(..2),
            Some(
                &[
                    "::notice title=ripr::Annotated 11 of 12 static exposure finding(s); 1 suppressed by policy policy/ripr-suppressions.toml. Run `ripr check --format json` to list every finding.",
                    "::notice title=ripr::Emitted 11 notice annotations; GitHub displays at most 10 per level in one step, so some are not shown. Run `ripr check --format json` to list every finding.",
                ][..]
            ),
            "{rendered}"
        );
        assert_eq!(lines.len(), 13, "{rendered}");
    }

    #[test]
    fn render_within_github_display_limit_emits_no_limit_notice() {
        let mut output = output_with_unknown_finding();
        let template = output.findings[0].clone();
        output.findings = (0..10)
            .map(|index| {
                let mut finding = template.clone();
                finding.probe.location.line = 100 + index;
                finding
            })
            .collect();

        let rendered = render(&output);

        assert!(!rendered.contains("GitHub displays at most"), "{rendered}");
        assert_eq!(rendered.lines().count(), 10, "{rendered}");
    }

    #[test]
    fn render_without_suppression_policy_emits_no_denominator_notice() {
        let rendered = render(&output_with_unknown_finding());

        assert!(rendered.contains("file=src/lib.rs"));
        assert!(
            !rendered.contains("Annotated "),
            "a fully annotated run needs no denominator: {rendered}"
        );
    }

    fn output_with_unknown_finding() -> CheckOutput {
        CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![Finding {
                id: "probe:src_lib_rs:13:static_unknown".to_string(),
                canonical_gap: None,
                probe: Probe {
                    id: ProbeId("probe:src_lib_rs:13:static_unknown".to_string()),
                    location: SourceLocation::new("src/lib.rs", 13, 1),
                    owner: None,
                    family: ProbeFamily::StaticUnknown,
                    delta: DeltaKind::Unknown,
                    before: None,
                    after: None,
                    expression: "opaque".to_string(),
                    expected_sinks: vec![],
                    required_oracles: vec![],
                },
                class: ExposureClass::StaticUnknown,
                ripr: RiprEvidence {
                    reach: stage(StageState::Unknown, "reach unknown"),
                    infect: stage(StageState::Unknown, "infection unknown"),
                    propagate: stage(StageState::Unknown, "propagation unknown"),
                    reveal: RevealEvidence {
                        observe: stage(StageState::Unknown, "observe unknown"),
                        discriminate: stage(StageState::Unknown, "discriminate unknown"),
                    },
                },
                confidence: 0.2,
                evidence: vec![],
                missing: vec![],
                flow_sinks: vec![],
                activation: crate::domain::ActivationEvidence::default(),
                stop_reasons: vec![],
                related_tests_matched_total: None,
                related_tests: vec![],
                recommended_next_step: Some(
                    "Add: case, with 100% coverage\nthen verify\routcome".to_string(),
                ),
                language: None,
                language_status: None,
                owner_kind: None,
                static_limit_kind: None,
                changed_sink: None,
                observed_sink: None,
                oracle_alignment: None,
                alignment_reason: None,
                source_currentness: crate::domain::SourceCurrentness::CandidateCurrent,
            }],
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        }
    }

    /// Build a TypeScript preview finding. With `complete_packet`, the
    /// evidence satisfies the shared repair-packet validator so
    /// `preview_actionability_for` reports `repair_packet_ready: true`
    /// (same recipe as the human surface's complete-packet fixture).
    fn typescript_preview_finding(complete_packet: bool) -> Finding {
        let mut finding = output_with_unknown_finding().findings[0].clone();
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

    fn output_with_python_repair_card() -> CheckOutput {
        let mut output = output_with_unknown_finding();
        let finding = &mut output.findings[0];
        finding.id = "probe:src_pricing.py:2:python_preview".to_string();
        finding.canonical_gap = Some(FindingCanonicalGap {
            id: "gap:python:src/pricing.py:calculate_discount:predicate_boundary:predicate:amount>=threshold"
                .to_string(),
            language: "python".to_string(),
            file: "src/pricing.py".to_string(),
            owner: "calculate_discount".to_string(),
            behavior_kind: "predicate_boundary".to_string(),
            probe_kind: "predicate".to_string(),
            normalized_discriminator: "amount>=threshold".to_string(),
        });
        finding.probe = Probe {
            id: ProbeId("probe:src_pricing.py:2:python_preview".to_string()),
            location: SourceLocation::new("src/pricing.py", 2, 5),
            owner: None,
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: Some("amount > threshold".to_string()),
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec!["return_value".to_string()],
            required_oracles: vec!["exact boundary assertion".to_string()],
        };
        finding.class = ExposureClass::WeaklyExposed;
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "amount == threshold".to_string(),
            reason: "no related test calls the equality boundary".to_string(),
            flow_sink: None,
        }];
        finding.evidence = vec![
            "suggested_test_file: tests/test_pricing.py".to_string(),
            "suggested_test_name: test_calculate_discount_threshold_boundary".to_string(),
            "suggested_test_node_id: tests/test_pricing.py::test_calculate_discount_threshold_boundary"
                .to_string(),
            "suggested_verify_command: pytest tests/test_pricing.py::test_calculate_discount_threshold_boundary"
                .to_string(),
            "suggested_verify_command_confidence: high".to_string(),
        ];
        finding.related_tests = vec![RelatedTest {
            name: "test_calculate_discount_above_threshold".to_string(),
            file: PathBuf::from("tests/test_pricing.py"),
            line: 6,
            oracle: Some("assert result".to_string()),
            oracle_kind: OracleKind::SmokeOnly,
            oracle_strength: OracleStrength::Weak,
            relation_reason: None,
            relation_confidence: None,
        }];
        finding.language = Some(LanguageId::Python);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.recommended_next_step = Some("Add a Python boundary assertion".to_string());
        output
    }

    fn output_with_perl_preview_card() -> CheckOutput {
        let mut output = output_with_unknown_finding();
        let finding = &mut output.findings[0];
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
                "Perl fact packet links the related test to the changed owner",
            ),
            infect: stage(
                StageState::Yes,
                "Changed return value reaches the owner result",
            ),
            propagate: stage(
                StageState::Yes,
                "Return value can propagate to Test::More assertion",
            ),
            reveal: RevealEvidence {
                observe: stage(StageState::Yes, "Related test reaches the changed owner"),
                discriminate: stage(StageState::Weak, "Exact return discriminator is missing"),
            },
        };
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
        finding.language = Some(LanguageId::Perl);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.recommended_next_step = Some("Add a focused Perl assertion.".to_string());
        output
    }

    fn output_with_python_no_action_findings() -> CheckOutput {
        let base = output_with_unknown_finding();
        CheckOutput {
            findings: vec![
                python_no_action_finding(
                    "probe:src_pricing.py:2:already_observed",
                    ExposureClass::Exposed,
                    2,
                ),
                python_no_action_finding(
                    "probe:src_pricing.py:4:no_related",
                    ExposureClass::NoStaticPath,
                    4,
                ),
                python_heuristic_only_finding(),
            ],
            ..base
        }
    }

    fn python_no_action_finding(id: &str, class: ExposureClass, line: usize) -> Finding {
        let mut finding = output_with_unknown_finding().findings[0].clone();
        finding.id = id.to_string();
        finding.probe.id = ProbeId(id.to_string());
        finding.probe.location = SourceLocation::new("src/pricing.py", line, 1);
        finding.probe.owner = Some(SymbolId("python:src/pricing.py::discount".to_string()));
        finding.probe.family = ProbeFamily::Predicate;
        finding.probe.delta = DeltaKind::Control;
        finding.probe.expression = "amount >= threshold".to_string();
        finding.class = class;
        finding.evidence.clear();
        finding.missing.clear();
        finding.related_tests.clear();
        finding.recommended_next_step = None;
        finding.language = Some(LanguageId::Python);
        finding.language_status = Some(LanguageStatus::Preview);
        finding
    }

    fn python_heuristic_only_finding() -> Finding {
        let mut finding = python_no_action_finding(
            "probe:src_pricing.py:6:heuristic",
            ExposureClass::WeaklyExposed,
            6,
        );
        finding.evidence = vec!["related_test_uncertain: test_name_similarity".to_string()];
        finding.ripr.reach = stage(StageState::Weak, "heuristic Python test link by file name");
        finding
    }

    fn output_with_python_static_limit() -> CheckOutput {
        let mut output = output_with_unknown_finding();
        let finding = &mut output.findings[0];
        finding.id = "probe:src_runtime.py:2:python_static_limit".to_string();
        finding.probe.id = ProbeId("probe:src_runtime.py:2:python_static_limit".to_string());
        finding.probe.location = SourceLocation::new("src/runtime.py", 2, 1);
        finding.probe.owner = Some(SymbolId("python:src/runtime.py::dispatch".to_string()));
        finding.probe.expression = "return getattr(handler, name)(payload)".to_string();
        finding.class = ExposureClass::StaticUnknown;
        finding.language = Some(LanguageId::Python);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.static_limit_kind = Some(StaticLimitKind::DynamicDispatch);
        finding.missing = vec![
            "Static limit `dynamic_dispatch` prevents bounded repair routing because syntax alone cannot resolve runtime getattr dispatch.".to_string(),
        ];
        finding.stop_reasons = vec![StopReason::DynamicDispatchUnresolved];
        output
    }

    fn stage(state: StageState, reason: &str) -> StageEvidence {
        StageEvidence {
            state,
            confidence: Confidence::Low,
            summary: reason.to_string(),
        }
    }

    #[test]
    fn render_normalizes_backslash_location_path_to_forward_slash() {
        let output = CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: None,
            summary: Summary::default(),
            findings: vec![Finding {
                id: "probe:src_pricing_ts:5:predicate".to_string(),
                canonical_gap: None,
                probe: Probe {
                    id: ProbeId("probe:src_pricing_ts:5:predicate".to_string()),
                    location: SourceLocation::new(PathBuf::from(r"src\pricing.ts"), 5, 1),
                    owner: None,
                    family: ProbeFamily::Predicate,
                    delta: DeltaKind::Control,
                    before: None,
                    after: None,
                    expression: "x > 0".to_string(),
                    expected_sinks: vec![],
                    required_oracles: vec![],
                },
                class: ExposureClass::WeaklyExposed,
                ripr: RiprEvidence {
                    reach: stage(StageState::Yes, "reachable"),
                    infect: stage(StageState::Yes, "infected"),
                    propagate: stage(StageState::Yes, "propagated"),
                    reveal: RevealEvidence {
                        observe: stage(StageState::Yes, "observed"),
                        discriminate: stage(StageState::No, "not discriminated"),
                    },
                },
                confidence: 0.7,
                evidence: vec![],
                missing: vec![],
                flow_sinks: vec![],
                activation: crate::domain::ActivationEvidence::default(),
                stop_reasons: vec![],
                related_tests_matched_total: None,
                related_tests: vec![],
                recommended_next_step: None,
                language: None,
                language_status: None,
                owner_kind: None,
                static_limit_kind: None,
                changed_sink: None,
                observed_sink: None,
                oracle_alignment: None,
                alignment_reason: None,
                source_currentness: crate::domain::SourceCurrentness::CandidateCurrent,
            }],
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
            rendered.contains("file=src/pricing.ts,"),
            "expected forward-slash path in github annotation; got:\n{rendered}"
        );
        assert!(
            !rendered.contains(r"src\pricing.ts"),
            "backslash path must not appear in github annotation; got:\n{rendered}"
        );
    }

    #[test]
    fn base_deleted_findings_emit_no_github_annotations() -> Result<(), String> {
        // RIPR-SPEC-0152: annotations are current PR obligations; the
        // candidate-current twin keeps its annotation.
        let mut base = output_with_unknown_finding();
        let Some(finding) = base.findings.first_mut() else {
            return Err("fixture finding expected".to_string());
        };
        finding.source_currentness = crate::domain::SourceCurrentness::BaseDeleted;
        let annotations = super::render_with_config(&base, &crate::config::RiprConfig::default());
        assert!(
            !annotations.contains("::notice file=src/lib.rs")
                && !annotations.contains("::warning file=src/lib.rs"),
            "base-deleted finding must not annotate: {annotations}"
        );
        assert!(
            annotations.contains(
                "::notice title=ripr::Annotated 0 of 1 static exposure finding(s); 1 not current in this change"
            ),
            "an all-base-side run still carries a denominator (#4393): {annotations}"
        );

        let mut current = output_with_unknown_finding();
        if let Some(finding) = current.findings.first_mut() {
            finding.source_currentness = crate::domain::SourceCurrentness::CandidateCurrent;
        }
        let annotations =
            super::render_with_config(&current, &crate::config::RiprConfig::default());
        assert!(
            annotations.contains("::notice file=src/lib.rs"),
            "candidate-current twin keeps its annotation"
        );
        Ok(())
    }
}
