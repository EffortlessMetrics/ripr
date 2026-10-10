use super::{
    agent_seam_packets, badge, format::OutputFormat, github, human, json, repo_exposure,
    repo_seams, sarif, suppressions,
};
use crate::analysis;
use crate::app::causal_projection::CausalDeltaArtifact;
use crate::app::{AnalysisProgressSink, CheckOutput, FindingDrillIn, repo_inventory_with_progress};
use crate::config::RiprConfig;
use crate::output::repo_exposure::TsFullRepoGuidance;
use std::collections::BTreeMap;

/// Path (relative to the analyzed workspace root) where the
/// test-efficiency report is expected when rendering `ripr+` badge formats.
const TEST_EFFICIENCY_REPORT_RELATIVE: &str = "target/ripr/reports/test-efficiency.json";

/// Repo-relative report path the public `ripr` badge projection names as its
/// source (RIPR-SPEC-0066 `source_report`). The repo-badge render persists
/// the canonical-actionable-gap projection at this path inside the analyzed
/// workspace (#6610), so the pointer the artifact carries always resolves.
const REPO_RIPR_BADGE_SOURCE_REPORT: &str = "target/ripr/reports/repo-ripr-badge.json";

/// Repo-relative report path the public `ripr+` badge projection names as its
/// source (RIPR-SPEC-0066 `source_report`). Persisted by the producing
/// native repo-badge-plus render, like the `ripr` badge path (#6610).
const REPO_RIPR_PLUS_BADGE_SOURCE_REPORT: &str = "target/ripr/reports/repo-ripr-plus-badge.json";

pub(crate) fn render_check_with_config(
    output: &CheckOutput,
    format: &OutputFormat,
    config: &RiprConfig,
) -> Result<String, String> {
    render_check_with_config_and_progress(output, format, config, None)
}

/// Renders a previously computed [`CheckOutput`] in the requested format,
/// reporting repo-scope progress boundaries to `progress` while the
/// full-repo audit-path formats run their seam walks (#4945). Diff-scoped
/// and badge arms ignore the sink; `None` reproduces the silent library
/// rendering.
pub(crate) fn render_check_with_config_and_progress(
    output: &CheckOutput,
    format: &OutputFormat,
    config: &RiprConfig,
    progress: Option<&dyn AnalysisProgressSink>,
) -> Result<String, String> {
    match format {
        OutputFormat::Human => Ok(human::terminal_safe(human::render_bounded_with_config(
            output, config,
        ))),
        OutputFormat::HumanFull => Ok(human::terminal_safe(human::render_full_with_config(
            output, config,
        ))),
        OutputFormat::Json => {
            // Fail-closed budget resolution (#5203): an unparseable
            // `RIPR_CHECK_FINDINGS_BYTES` aborts the run, never rendering.
            let findings_budget = json::check_findings_byte_budget()?;
            stamp_check_json_checked(
                json::render_with_config(output, config, findings_budget),
                &output.root,
            )
        }
        OutputFormat::Github => Ok(github::render_with_config(output, config)),
        OutputFormat::Sarif => {
            let suppressions = load_suppressions(output, config)?;
            Ok(sarif::render_findings_sarif(output, config, &suppressions))
        }
        OutputFormat::BadgeJson => {
            let summary = ripr_summary_with_suppressions(output, config)?;
            Ok(badge::render_native_json(&summary))
        }
        OutputFormat::RepoBadgeJson => repo_inventory_with_progress(
            progress,
            || ripr_repo_canonical_actionable_summary(output, config),
            |(mut summary, limit_info)| {
                Ok(render_native_badge_with_persisted_source_report(
                    &mut summary,
                    &output.root,
                    REPO_RIPR_BADGE_SOURCE_REPORT,
                    |summary, source| {
                        attach_repo_badge_projection(summary, limit_info.as_ref(), source)
                    },
                ))
            },
        ),
        OutputFormat::BadgeShields => {
            let summary = ripr_summary_with_suppressions(output, config)?;
            Ok(badge::render_shields_json(&summary))
        }
        OutputFormat::RepoBadgeShields => repo_inventory_with_progress(
            progress,
            || ripr_repo_canonical_actionable_summary(output, config),
            |(mut summary, limit_info)| {
                // Shields output carries exactly four fields and no
                // `source_report` member, so the pointer needs no persisted
                // artifact here; the count state is the fresh run's.
                attach_repo_badge_projection(
                    &mut summary,
                    limit_info.as_ref(),
                    Some(REPO_RIPR_BADGE_SOURCE_REPORT),
                );
                Ok(badge::render_shields_json(&summary))
            },
        ),
        OutputFormat::BadgePlusJson | OutputFormat::RepoBadgePlusJson => {
            let (mut summary, limit_info) =
                ripr_plus_summary_from_disk(output, format.is_repo_scope(), config)?;
            maybe_attach_repo_plus_projection(&mut summary, output, format, limit_info.as_ref());
            if format.is_repo_scope() && summary.projection.is_some() {
                return Ok(render_native_badge_with_persisted_source_report(
                    &mut summary,
                    &output.root,
                    REPO_RIPR_PLUS_BADGE_SOURCE_REPORT,
                    |summary, source| {
                        maybe_attach_repo_plus_projection_with_source(
                            summary,
                            output,
                            limit_info.as_ref(),
                            source,
                        )
                    },
                ));
            }
            Ok(badge::render_native_json(&summary))
        }
        OutputFormat::BadgePlusShields | OutputFormat::RepoBadgePlusShields => {
            let (mut summary, limit_info) =
                ripr_plus_summary_from_disk(output, format.is_repo_scope(), config)?;
            maybe_attach_repo_plus_projection(&mut summary, output, format, limit_info.as_ref());
            Ok(badge::render_shields_json(&summary))
        }
        OutputFormat::RepoSeamsJson => repo_inventory_with_progress(
            progress,
            || analysis::inventory_seams_at_with_config(&output.root, config),
            |seams| {
                let context = crate::agent::artifact::RepoExposureArtifactContext::for_repo_seams(
                    output.root.clone(),
                    output.mode.as_str().to_string(),
                    output.base.clone(),
                    config,
                )?;
                repo_seams::render_repo_seams_json_with_context(&seams, &context)
            },
        ),
        OutputFormat::RepoSeamsMd => repo_inventory_with_progress(
            progress,
            || analysis::inventory_seams_at_with_config(&output.root, config),
            |seams| Ok(repo_seams::render_repo_seams_md(&seams)),
        ),
        OutputFormat::RepoExposureJson => repo_inventory_with_progress(
            progress,
            || analysis::inventory_classified_seams_report_at_with_config(&output.root, config),
            |report| {
                let ts_guidance = detect_ts_full_repo_guidance(&output.root, &report.classified);
                let python_guidance =
                    detect_python_repo_exposure_guidance(&output.root, &report.classified);
                let generated_skip = repo_exposure::GeneratedRustSkip::from_paths(
                    report.skipped_generated,
                    report.naming_only_skips,
                );
                let artifact_context =
                    crate::agent::artifact::RepoExposureArtifactContext::for_repo_exposure(
                        output.root.clone(),
                        output.mode.as_str().to_string(),
                        output.base.clone(),
                        config,
                    )?;
                repo_exposure::render_repo_exposure_json_with_context(
                    &report.classified,
                    report.limit_info.as_ref(),
                    ts_guidance.as_ref(),
                    python_guidance.as_ref(),
                    generated_skip.as_ref(),
                    &artifact_context,
                )
            },
        ),
        OutputFormat::RepoExposureSummaryJson => repo_inventory_with_progress(
            progress,
            || analysis::inventory_compact_classified_seams_at_with_config(&output.root, config),
            |classified| {
                Ok(repo_exposure::render_repo_exposure_summary_json(
                    &classified,
                    &output.root,
                    output.base.as_deref(),
                    output.mode.as_str(),
                ))
            },
        ),
        OutputFormat::RepoExposureMd => repo_inventory_with_progress(
            progress,
            || analysis::inventory_classified_seams_report_at_with_config(&output.root, config),
            |report| {
                let ts_guidance = detect_ts_full_repo_guidance(&output.root, &report.classified);
                let python_guidance =
                    detect_python_repo_exposure_guidance(&output.root, &report.classified);
                let generated_skip = repo_exposure::GeneratedRustSkip::from_paths(
                    report.skipped_generated,
                    report.naming_only_skips,
                );
                Ok(repo_exposure::render_repo_exposure_md_with_generated_skip(
                    &report.classified,
                    report.limit_info.as_ref(),
                    ts_guidance.as_ref(),
                    python_guidance.as_ref(),
                    generated_skip.as_ref(),
                ))
            },
        ),
        OutputFormat::RepoSarif => repo_inventory_with_progress(
            progress,
            || analysis::inventory_classified_seams_at_with_config(&output.root, config),
            |(classified, limit_info)| {
                Ok(sarif::render_repo_seams_sarif(
                    &classified,
                    limit_info.as_ref(),
                    config,
                ))
            },
        ),
        OutputFormat::AgentSeamPacketsJson => repo_inventory_with_progress(
            progress,
            || analysis::inventory_classified_seams_at_with_config(&output.root, config),
            |(classified, _)| {
                let (causal_projection, causal_projection_warning) =
                    CausalDeltaArtifact::load_optional(&output.root);
                if let Some(warning) = causal_projection_warning {
                    eprintln!(
                        "{}",
                        human::terminal_safe(format!("ripr agent packets: {warning}"))
                    );
                }
                Ok(
                    agent_seam_packets::render_agent_seam_packets_json_with_causal_and_outcome(
                        &classified,
                        None,
                        causal_projection.as_ref(),
                        output.analysis_outcome.as_ref(),
                        output.base.is_some(),
                    ),
                )
            },
        ),
    }
}

/// Unbounded JSON render for in-process consumers (#5203, Codex P1 on #5271).
///
/// `pr-evidence` runs its check in-process and routes from the full finding
/// set; the findings-array byte budget protects external document consumers
/// (agents, editors, CI logs), so it must not truncate a JSON string that
/// never leaves the process. Same stamping as the `Json` arm, budget `None`.
pub(crate) fn render_check_json_unbounded(output: &CheckOutput, config: &RiprConfig) -> String {
    stamp_check_json(json::render_with_config(output, config, None), &output.root)
}

/// Same full renderer for PR evidence, propagating active frozen stamp failures.
/// This does not establish a bounded serialization or aggregate resource claim.
pub(crate) fn render_check_json_for_pr_evidence(
    output: &CheckOutput,
    config: &RiprConfig,
) -> Result<String, String> {
    stamp_check_json_checked(json::render_with_config(output, config, None), &output.root)
}

fn stamp_check_json_checked(
    rendered: String,
    root: &std::path::Path,
) -> Result<String, String> {
    let Some(authority) = super::gap_source_subject::frozen_stamp_context(root)? else {
        return Ok(stamp_check_json(rendered, root));
    };
    let paths = super::gap_decision_ledger::check_output_subject_paths(&rendered, root);
    super::gap_source_subject::require_frozen_stamp_context(root, &authority)?;
    let paths = paths?;
    let stamped =
        super::gap_source_subject::append_source_subject_member_checked(rendered, root, &paths)?;
    super::gap_source_subject::require_frozen_stamp_context(root, &authority)?;
    Ok(stamped)
}

/// #4544: stamp the check JSON with the content digests of every file a gap
/// ledger derived from it would name, read in this analysis run, so the
/// ledger writer can copy them instead of hashing the workspace later.
fn stamp_check_json(rendered: String, root: &std::path::Path) -> String {
    if let Some(authority) = crate::analysis::committed_source::frozen::current() {
        return match stamp_check_json_checked(rendered.clone(), root) {
            Ok(stamped) => stamped,
            Err(error) => {
                authority.refuse_external_effect(&error);
                rendered
            }
        };
    }
    match super::gap_decision_ledger::check_output_subject_paths(&rendered, root) {
        Ok(paths) => {
            super::gap_source_subject::append_source_subject_member(rendered, root, &paths)
        }
        Err(_) => rendered,
    }
}

/// Navigation-aware rendering that also reports repo-scope progress
/// boundaries to `progress` (#4945).
pub(crate) fn render_check_with_config_and_navigation_and_progress(
    output: &CheckOutput,
    format: &OutputFormat,
    config: &RiprConfig,
    drill_in: Option<&FindingDrillIn>,
    progress: Option<&dyn AnalysisProgressSink>,
) -> Result<String, String> {
    match format {
        OutputFormat::Human => Ok(human::terminal_safe(
            human::render_bounded_with_config_and_navigation(output, config, drill_in),
        )),
        OutputFormat::HumanFull => Ok(human::terminal_safe(
            human::render_full_with_config_and_navigation(output, config, drill_in),
        )),
        _ => render_check_with_config_and_progress(output, format, config, progress),
    }
}

/// Public re-export for CLI callers that drive the streaming JSON path directly
/// (bypassing `render_check_with_config`).
pub(crate) fn detect_ts_full_repo_guidance_pub(
    root: &std::path::Path,
    classified: &[crate::analysis::ClassifiedSeam],
) -> Option<TsFullRepoGuidance> {
    detect_ts_full_repo_guidance(root, classified)
}

/// Public re-export for CLI callers that drive the streaming JSON path directly.
pub(crate) fn detect_python_repo_exposure_guidance_pub(
    root: &std::path::Path,
    classified: &[crate::analysis::ClassifiedSeam],
) -> Option<repo_exposure::PythonRepoExposureGuidance> {
    detect_python_repo_exposure_guidance(root, classified)
}

/// Detect whether a TypeScript diff-first guidance disclosure should fire.
///
/// Returns `Some(TsFullRepoGuidance)` when ALL of:
///
/// 1. The classified seam inventory is empty (no Rust seams, so the report
///    would otherwise be a silent empty result).
/// 2. TypeScript or JavaScript files are present in the workspace (detected
///    by path extension via `workspace_preview_language_files`).
/// 3. No Rust source files are found at the root (the workspace has no
///    Rust crate presence that could legitimately produce zero seams).
///
/// Condition 3 is the key guard for the "additive / no Rust regression"
/// invariant: a Rust-only workspace that happens to produce zero seams
/// (e.g., a workspace with only `mod` re-exports and no production shapes)
/// must NOT trigger the TS guidance. Only workspaces where Rust is absent
/// and TS is present get the disclosure.
fn detect_ts_full_repo_guidance(
    root: &std::path::Path,
    classified: &[crate::analysis::ClassifiedSeam],
) -> Option<TsFullRepoGuidance> {
    use crate::domain::LanguageId;

    // Guidance only fires when the repo scan produced no seams.
    if !classified.is_empty() {
        return None;
    }

    // Count TS/JS files in the workspace.
    let preview_files = analysis::workspace_preview_language_files(root);
    let ts_file_count = preview_files
        .iter()
        .filter(|(lang, _)| *lang == LanguageId::TypeScript || *lang == LanguageId::JavaScript)
        .count();

    if ts_file_count == 0 {
        return None;
    }

    // Guard: if there are Rust files, the empty-seam result is a legitimate
    // Rust analysis outcome (e.g. a workspace with no production shapes), not
    // a TS-first scan gap. Only fire when Rust is absent.
    let rust_files = analysis::workspace_rust_files(root);
    if !rust_files.is_empty() {
        return None;
    }

    let readiness = analysis::workspace_typescript_repo_readiness(root)?;

    Some(TsFullRepoGuidance {
        ts_file_count,
        readiness,
    })
}

/// Detect whether a Python diff-first guidance disclosure should fire.
///
/// Same fail-closed guards as the TypeScript disclosure: empty seam
/// inventory, at least one Python file, and no Rust source. A Rust workspace
/// that happens to contain Python and zero seams keeps the Rust result.
fn detect_python_repo_exposure_guidance(
    root: &std::path::Path,
    classified: &[crate::analysis::ClassifiedSeam],
) -> Option<repo_exposure::PythonRepoExposureGuidance> {
    use crate::domain::LanguageId;

    if !classified.is_empty() {
        return None;
    }

    let preview_files = analysis::workspace_preview_language_files(root);
    let python_file_count = preview_files
        .iter()
        .filter(|(lang, _)| *lang == LanguageId::Python)
        .count();
    if python_file_count == 0 {
        return None;
    }
    if !analysis::workspace_rust_files(root).is_empty() {
        return None;
    }

    Some(repo_exposure::PythonRepoExposureGuidance { python_file_count })
}

fn load_suppressions(
    output: &CheckOutput,
    config: &RiprConfig,
) -> Result<Vec<suppressions::SuppressionEntry>, String> {
    suppressions::load_suppressions_for_root_at(&output.root, config.suppressions().path()).map_err(
        |violations| {
            format!(
                "{} validation failed:\n{}",
                config.suppressions().display_path(),
                violations.join("\n")
            )
        },
    )
}

/// Attaches the public projection to a repo-scoped `ripr+` badge, but only
/// when a measured test-efficiency report exists. The neutral
/// "needs test-efficiency" badge (no report on disk) is left unprojected: an
/// unmeasurable `ripr+` must not be projected as a clean `0 actionable`
/// public count. Diff-scoped `ripr+` badges are also left unchanged.
fn maybe_attach_repo_plus_projection(
    summary: &mut badge::BadgeSummary,
    output: &CheckOutput,
    format: &OutputFormat,
    limit_info: Option<&analysis::SeamLimitInfo>,
) {
    if format.is_repo_scope() && output.root.join(TEST_EFFICIENCY_REPORT_RELATIVE).exists() {
        maybe_attach_repo_plus_projection_with_source(
            summary,
            output,
            limit_info,
            Some(REPO_RIPR_PLUS_BADGE_SOURCE_REPORT),
        );
    }
}

/// `maybe_attach_repo_plus_projection` with the caller-resolved
/// `source_report` (#6610): the native repo-badge-plus render reattaches with
/// `None` when the named report could not be persisted.
fn maybe_attach_repo_plus_projection_with_source(
    summary: &mut badge::BadgeSummary,
    output: &CheckOutput,
    limit_info: Option<&analysis::SeamLimitInfo>,
    source_report: Option<&str>,
) {
    if !output.root.join(TEST_EFFICIENCY_REPORT_RELATIVE).exists() {
        return;
    }
    match limit_info {
        Some(limit) => badge::attach_public_projection_with_optional_source(
            summary,
            source_report,
            "limited_seam_cap",
            Some(format!(
                "seam limit applied: analyzed {} of {} seams; unscanned seams are not counted",
                limit.analyzed, limit.total
            )),
        ),
        None => match source_report {
            Some(source) => badge::attach_public_projection(summary, source),
            None => {
                badge::attach_public_projection_with_optional_source(summary, None, "full", None)
            }
        },
    }
}

fn ripr_summary_with_suppressions(
    output: &CheckOutput,
    config: &RiprConfig,
) -> Result<badge::BadgeSummary, String> {
    let suppressions = load_suppressions(output, config)?;
    let today = suppressions::current_iso_date();
    let policy = badge::BadgePolicy {
        suppressions_path: config.suppressions().display_path(),
        ..badge::BadgePolicy::default()
    };
    Ok(badge::ripr_badge_summary_with_suppressions(
        output,
        &suppressions,
        &today,
        policy,
    ))
}

/// The repo badge's `canonical_actionable_gap` basis derives from the same
/// full classified seam inventory `repo-exposure-json` renders (#5261). The
/// compact classified walk zeroes the related-test and observed-value
/// evidence payload and approximates activation for several seam kinds, so
/// evidence records projected from it reclassify actionable gaps as
/// `unknown` and the badge rendered a clean `0 actionable` beside an
/// actionable repo-exposure record on the same tree. Agreement with
/// repo-exposure outranks the compact walk's cost saving: a wrong clean
/// signal on the public projection is the same harm as a wrong repair
/// signal on the other side, and the full classified inventory is the
/// cache-backed walk the audit-path disclosure already names.
fn ripr_repo_canonical_actionable_summary(
    output: &CheckOutput,
    config: &RiprConfig,
) -> Result<(badge::BadgeSummary, Option<analysis::SeamLimitInfo>), String> {
    let report = analysis::inventory_classified_seams_report_at_with_config(&output.root, config)?;
    let policy = badge::BadgePolicy {
        suppressions_path: config.suppressions().display_path(),
        ..badge::BadgePolicy::default()
    };
    let summary = badge::ripr_canonical_actionable_gap_badge_summary(&report.classified, policy);
    // A seam-capped inventory must not project its partial count as a clean
    // full-run badge (review round 1, #5261): the limit travels with the
    // summary so the public projection resolves to the limited state.
    Ok((summary, report.limit_info))
}

/// The seam-native repo badge's public projection, honoring the producing
/// walk's completeness: a capped inventory projects `limited` (with the cap
/// named) instead of a count that would read as a full-scan result. The
/// caller resolves `source_report` (#6610): the canonical report path when
/// this run persists it, `None` (projection fails closed to `unknown`) when
/// it could not.
fn attach_repo_badge_projection(
    summary: &mut badge::BadgeSummary,
    limit_info: Option<&analysis::SeamLimitInfo>,
    source_report: Option<&str>,
) {
    match limit_info {
        Some(limit) => badge::attach_public_projection_with_optional_source(
            summary,
            source_report,
            "limited_seam_cap",
            Some(format!(
                "seam limit applied: analyzed {} of {} seams; unscanned seams are not counted",
                limit.analyzed, limit.total
            )),
        ),
        None => match source_report {
            Some(source) => badge::attach_public_projection(summary, source),
            None => {
                badge::attach_public_projection_with_optional_source(summary, None, "full", None)
            }
        },
    }
}

/// Renders the native repo badge and makes its `public_projection.source_report`
/// pointer real (#6610): the exact rendered bytes are persisted at the
/// canonical repo-relative report path inside the analyzed workspace, so the
/// provenance the artifact claims always resolves — including when a CI
/// pipeline redirects stdout elsewhere. When the workspace cannot be
/// written, the projection is reattached with no source and the artifact
/// fails closed to the RIPR-SPEC-0066 `unknown` state instead of naming a
/// report the run did not produce.
fn render_native_badge_with_persisted_source_report(
    summary: &mut badge::BadgeSummary,
    root: &std::path::Path,
    source_report: &str,
    attach: impl Fn(&mut badge::BadgeSummary, Option<&str>),
) -> String {
    attach(summary, Some(source_report));
    let rendered = badge::render_native_json(summary);
    if write_workspace_report(root, source_report, &rendered) {
        return rendered;
    }
    attach(summary, None);
    badge::render_native_json(summary)
}

/// Persists `text` at `relative` (root-relative, forward slashes) inside the
/// analyzed workspace, creating parent directories. `false` when the write
/// fails; the caller decides the honest degraded rendering.
fn write_workspace_report(root: &std::path::Path, relative: &str, text: &str) -> bool {
    let path = root.join(relative);
    let Some(parent) = path.parent() else {
        return false;
    };
    if let Err(err) = std::fs::create_dir_all(parent) {
        eprintln!(
            "ripr: could not create report directory {}: {err}",
            parent.display()
        );
        return false;
    }
    // The house atomic writer (review on #6788): a failed or interrupted
    // write leaves the previous report untouched, and a destination that is
    // not a regular file (for example a symlinked report leaf) is refused
    // rather than written through — the degraded projection then discloses
    // `unknown` instead of claiming the unrefreshed file.
    if let Err(err) = crate::output::file_write::write(&path, text.as_bytes()) {
        eprintln!("ripr: could not persist report {}: {err}", path.display());
        return false;
    }
    true
}

fn ripr_plus_summary_from_disk(
    output: &CheckOutput,
    repo_scope: bool,
    config: &RiprConfig,
) -> Result<(badge::BadgeSummary, Option<analysis::SeamLimitInfo>), String> {
    let report_path = output.root.join(TEST_EFFICIENCY_REPORT_RELATIVE);
    if !report_path.exists() {
        let warning = format!(
            "missing {}; provide test-efficiency JSON before requesting a measured ripr+ badge; see docs/BADGE_ADOPTION.md",
            report_path.display()
        );
        eprintln!(
            "{}",
            human::terminal_safe(format!("ripr: {warning}; rendering neutral ripr+ badge"))
        );
        return Ok((
            missing_test_efficiency_badge_summary(repo_scope, config, warning),
            None,
        ));
    }
    let text = std::fs::read_to_string(&report_path)
        .map_err(|err| format!("failed to read {}: {err}", report_path.display()))?;
    let test_efficiency = badge::parse_test_efficiency_badge_summary(&text)?;
    let suppressions = load_suppressions(output, config)?;
    let today = suppressions::current_iso_date();
    // `cargo xtask test-efficiency-report` is repo-wide as a fact source.
    // Diff-scoped `ripr+` filters that ledger to entries related to the
    // changed code (via `Finding.related_tests` names + `Finding.probe.owner`
    // intersected with each entry's `reached_owners`); repo-scoped
    // `ripr+` aggregates the repo-wide ledger directly.
    let diff_filter = if repo_scope {
        None
    } else {
        Some(badge::DiffRelatedTests::from_check_output(output))
    };
    let scope = match &diff_filter {
        Some(filter) => badge::TestEfficiencyAggregationScope::Diff(filter),
        None => badge::TestEfficiencyAggregationScope::Repo,
    };
    let policy = badge::BadgePolicy {
        suppressions_path: config.suppressions().display_path(),
        ..badge::BadgePolicy::default()
    };
    if repo_scope {
        let (exposure, limit_info) = ripr_repo_canonical_actionable_summary(output, config)?;
        let summary = badge::ripr_plus_canonical_actionable_gap_badge_summary(
            exposure,
            test_efficiency,
            policy,
        );
        Ok((summary, limit_info))
    } else {
        Ok((
            badge::ripr_plus_badge_summary_with_suppressions(
                output,
                test_efficiency,
                &suppressions,
                &today,
                policy,
                scope,
            ),
            None,
        ))
    }
}

fn missing_test_efficiency_badge_summary(
    repo_scope: bool,
    config: &RiprConfig,
    warning: String,
) -> badge::BadgeSummary {
    badge::BadgeSummary {
        kind: badge::BadgeKind::RiprPlus,
        scope: if repo_scope {
            badge::BadgeScope::Repo
        } else {
            badge::BadgeScope::Diff
        },
        basis: if repo_scope {
            badge::BadgeBasis::CanonicalActionableGap
        } else {
            badge::BadgeBasis::FindingExposure
        },
        message: "needs test-efficiency".to_string(),
        status: badge::BadgeStatus::Warn,
        color: "lightgrey",
        counts: badge::BadgeCounts {
            unsuppressed_exposure_gaps: 0,
            unsuppressed_test_efficiency_findings: 0,
            intentional_test_efficiency_findings: 0,
            suppressed_exposure_gaps: 0,
            suppressed_test_efficiency_findings: 0,
            unknowns: 0,
            unknowns_test_efficiency: 0,
            analyzed_findings: 0,
            analyzed_seams: 0,
            analyzed_gap_records: 0,
            analyzed_tests: 0,
        },
        reason_counts: BTreeMap::new(),
        policy: badge::BadgePolicy {
            suppressions_path: config.suppressions().display_path(),
            ..badge::BadgePolicy::default()
        },
        warnings: vec![warning],
        preview_skipped: Vec::new(),
        projection: None,
        analysis_outcome: None,
    }
}

#[cfg(test)]
mod tests {
    use super::render_check_with_config;
    use crate::app::{CheckOutput, Mode};
    use crate::config::RiprConfig;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, OracleKind,
        OracleStrength, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence,
        SourceLocation, StageEvidence, StageState, StopReason, Summary, SymbolId,
    };
    use crate::output::format::OutputFormat;
    use std::path::{Path, PathBuf};

    #[test]
    fn render_dispatch_renders_diff_sarif() -> Result<(), String> {
        let output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
        let rendered =
            render_check_with_config(&output, &OutputFormat::Sarif, &RiprConfig::default())?;

        assert!(rendered.contains("\"version\": \"2.1.0\""));
        assert!(rendered.contains("ripr.finding.weakly_exposed"));
        Ok(())
    }

    #[test]
    fn render_dispatch_renders_repo_seam_formats() -> Result<(), String> {
        let output = check_output_with_temp_seam_workspace(Vec::new())?;

        let seams_json = render_check_with_config(
            &output,
            &OutputFormat::RepoSeamsJson,
            &RiprConfig::default(),
        )?;
        let seams_md =
            render_check_with_config(&output, &OutputFormat::RepoSeamsMd, &RiprConfig::default())?;

        assert!(seams_json.contains("\"schema_version\": \"0.1\""));
        assert!(seams_json.contains("over_threshold"));
        assert!(seams_md.contains("over_threshold"));
        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn repo_exposure_names_python_diff_first_only_without_rust_files() -> Result<(), String> {
        let python_only = temp_root("ripr-render-python-only")?;
        std::fs::create_dir_all(python_only.join("src"))
            .map_err(|err| format!("create python src: {err}"))?;
        std::fs::create_dir_all(python_only.join("tests"))
            .map_err(|err| format!("create python tests: {err}"))?;
        std::fs::write(python_only.join("src/app.py"), "def run():\n    return 1\n")
            .map_err(|err| format!("write app.py: {err}"))?;
        std::fs::write(
            python_only.join("tests/test_app.py"),
            "def test_run():\n    assert True\n",
        )
        .map_err(|err| format!("write test: {err}"))?;

        let detected = super::detect_python_repo_exposure_guidance(&python_only, &[]);
        let count = detected
            .as_ref()
            .map(|guidance| guidance.python_file_count)
            .ok_or("python-only workspace must disclose python_diff_first")?;
        assert!(count >= 1, "expected at least one python file, got {count}");

        let mut output = check_output_with(Vec::new());
        output.root = python_only.clone();
        let json = render_check_with_config(
            &output,
            &OutputFormat::RepoExposureJson,
            &RiprConfig::default(),
        )?;
        assert!(
            json.contains("\"category\": \"python_diff_first\""),
            "{json}"
        );
        assert!(
            json.contains("\"seams\": []") || json.contains("\"seams\":[\n]"),
            "{json}"
        );
        let md = render_check_with_config(
            &output,
            &OutputFormat::RepoExposureMd,
            &RiprConfig::default(),
        )?;
        assert!(md.contains("python_diff_first"), "{md}");
        remove_temp_root(&python_only)?;

        let mixed = temp_root("ripr-render-python-with-rust")?;
        std::fs::write(mixed.join("lib.rs"), "// rust marker, no behavior\n")
            .map_err(|err| format!("write lib.rs: {err}"))?;
        std::fs::write(mixed.join("app.py"), "def run():\n    return 1\n")
            .map_err(|err| format!("write mixed app.py: {err}"))?;
        assert!(
            super::detect_python_repo_exposure_guidance(&mixed, &[]).is_none(),
            "a workspace that still has Rust files must not get the Python empty-seam disclosure"
        );
        remove_temp_root(&mixed)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_renders_repo_exposure_and_sarif_formats() -> Result<(), String> {
        let output = check_output_with_temp_seam_workspace(Vec::new())?;

        let exposure_json = render_check_with_config(
            &output,
            &OutputFormat::RepoExposureJson,
            &RiprConfig::default(),
        )?;
        let exposure_md = render_check_with_config(
            &output,
            &OutputFormat::RepoExposureMd,
            &RiprConfig::default(),
        )?;
        let sarif =
            render_check_with_config(&output, &OutputFormat::RepoSarif, &RiprConfig::default())?;

        assert!(exposure_json.contains("\"schema_version\": \"0.3\""));
        assert!(exposure_json.contains("over_threshold"));
        let exposure_summary = render_check_with_config(
            &output,
            &OutputFormat::RepoExposureSummaryJson,
            &RiprConfig::default(),
        )?;
        assert!(exposure_summary.contains("\"format\": \"repo-exposure-summary-json\""));
        assert!(exposure_summary.contains("\"basis\": \"canonical_actionable_gap\""));
        assert!(!exposure_summary.contains("\"evidence_record\""));
        assert!(exposure_md.contains("over_threshold"));
        assert!(sarif.contains("\"version\": \"2.1.0\""));
        assert!(sarif.contains("ripr.seam."));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_renders_agent_seam_packets() -> Result<(), String> {
        let output = check_output_with_temp_seam_workspace(Vec::new())?;

        let rendered = render_check_with_config(
            &output,
            &OutputFormat::AgentSeamPacketsJson,
            &RiprConfig::default(),
        )?;

        assert!(rendered.contains("\"schema_version\": \"0.5\""));
        assert!(rendered.contains("\"packets\""));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_reads_diff_badge_plus_report() -> Result<(), String> {
        let output =
            check_output_with_temp_report_workspace(vec![sample_finding("src/lib.rs", 1)])?;

        let native = render_check_with_config(
            &output,
            &OutputFormat::BadgePlusJson,
            &RiprConfig::default(),
        )?;
        let shields = render_check_with_config(
            &output,
            &OutputFormat::BadgePlusShields,
            &RiprConfig::default(),
        )?;

        assert!(native.contains("\"kind\": \"ripr_plus\""));
        assert!(native.contains("\"scope\": \"diff\""));
        assert!(shields.contains("\"schemaVersion\": 1"));
        assert!(!shields.contains("\"scope\""));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_reads_repo_badge_plus_report() -> Result<(), String> {
        let output = check_output_with_temp_seam_workspace(Vec::new())?;
        write_test_efficiency_report(&output.root)?;

        let native = render_check_with_config(
            &output,
            &OutputFormat::RepoBadgePlusJson,
            &RiprConfig::default(),
        )?;
        let shields = render_check_with_config(
            &output,
            &OutputFormat::RepoBadgePlusShields,
            &RiprConfig::default(),
        )?;

        assert!(native.contains("\"kind\": \"ripr_plus\""));
        assert!(native.contains("\"scope\": \"repo\""));
        assert!(native.contains("\"basis\": \"canonical_actionable_gap\""));
        assert!(shields.contains("\"schemaVersion\": 1"));
        assert!(!shields.contains("\"scope\""));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[cfg(all(target_os = "linux", feature = "lang-rust"))]
    #[test]
    fn frozen_check_rendering_stamps_named_head_and_propagates_snapshot_failures()
    -> Result<(), String> {
        use crate::analysis::committed_source::frozen;
        let root = temp_root("ripr-render-frozen-stamp")?;
        std::fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(root.join("tests")).map_err(|error| error.to_string())?;
        std::fs::write(root.join("src/lib.rs"), "pub fn named_head() {}\n")
            .map_err(|error| error.to_string())?;
        std::fs::write(root.join("tests/check.rs"), "fn named_test() {}\n")
            .map_err(|error| error.to_string())?;
        for args in [
            &["-c", "init.templateDir=", "init", "-q"][..],
            &["config", "user.email", "render-stamp@example.invalid"][..],
            &["config", "user.name", "render stamp fixture"][..],
            &["add", "."][..],
            &["commit", "-qm", "named rendering head"][..],
        ] {
            crate::testing::fixture_git::fixture_git_ok(&root, args)?;
        }
        let prepared = crate::analysis::git_candidate_execution::prepare_named_tree(
            &root, "HEAD", None,
        ).map_err(|error| error.to_string())?;
        let physical = prepared.physical_root().to_path_buf();
        let authority = prepared.frozen_source_authority(&root)
            .map_err(|error| error.to_string())?;
        let mut finding = sample_finding("src/lib.rs", 1);
        finding.related_tests[0].file = "tests/check.rs".into();
        let mut output = check_output_with(vec![finding]);
        output.root = root.clone();
        let config = RiprConfig::default();
        let ordinary = super::render_check_json_unbounded(&output, &config);
        assert_eq!(super::render_check_json_for_pr_evidence(&output, &config)?, ordinary);
        let ordinary_value: serde_json::Value = serde_json::from_str(&ordinary)
            .map_err(|error| error.to_string())?;
        let stamped = ordinary_value["source_subject"]["files"].as_array()
            .ok_or("ordinary render lacks source subject files")?;
        assert!(stamped.iter().any(|file| file["path"] == "src/lib.rs"));
        std::fs::write(root.join("src/lib.rs"), "dirty live replacement\n")
            .map_err(|error| error.to_string())?;
        std::fs::remove_file(root.join("tests/check.rs")).map_err(|error| error.to_string())?;
        frozen::with_context(Some(authority.clone()), || {
            assert_eq!(super::render_check_json_for_pr_evidence(&output, &config)?, ordinary);
            let external = render_check_with_config(&output, &OutputFormat::Json, &config)?;
            let external: serde_json::Value = serde_json::from_str(&external)
                .map_err(|error| error.to_string())?;
            assert_eq!(external["source_subject"], ordinary_value["source_subject"]);
            std::fs::remove_file(physical.join("src/lib.rs")).map_err(|error| error.to_string())?;
            let failure = expect_err(super::render_check_json_for_pr_evidence(&output, &config))?;
            assert!(failure.contains("src/lib.rs"), "{failure}");
            let public_failure = expect_err(render_check_with_config(&output, &OutputFormat::Json, &config))?;
            assert_eq!(public_failure, failure);
            Ok::<_, String>(())
        })?;
        drop(authority);
        remove_temp_root(&root)
    }

    #[cfg(all(target_os = "linux", feature = "lang-rust"))]
    #[test]
    fn frozen_checked_json_stamp_refuses_projection_errors_even_without_named_paths()
    -> Result<(), String> {
        use crate::analysis::committed_source::frozen;
        let root = temp_root("ripr-render-empty-frozen-stamp")?;
        std::fs::write(root.join("empty.rs"), "named\n").map_err(|error| error.to_string())?;
        for args in [
            &["-c", "init.templateDir=", "init", "-q"][..],
            &["config", "user.email", "render-stamp@example.invalid"][..],
            &["config", "user.name", "render stamp fixture"][..],
            &["add", "."][..],
            &["commit", "-qm", "named rendering head"][..],
        ] {
            crate::testing::fixture_git::fixture_git_ok(&root, args)?;
        }
        let prepared = crate::analysis::git_candidate_execution::prepare_named_tree(
            &root, "HEAD", None,
        ).map_err(|error| error.to_string())?;
        let authority = prepared.frozen_source_authority(&root)
            .map_err(|error| error.to_string())?;
        frozen::with_context(Some(authority.clone()), || {
            let parse = expect_err(super::stamp_check_json_checked("not JSON".into(), &root))?;
            assert!(parse.contains("invalid JSON"), "{parse}");
            let nonobject = expect_err(super::stamp_check_json_checked("[]".into(), &root))?;
            assert!(nonobject.contains("expected check output object"), "{nonobject}");
            authority.ensure_clean().map_err(|error| error.to_string())?;
            let empty = std::collections::BTreeSet::<String>::new();
            let failure = super::super::gap_source_subject::append_source_subject_member_checked(
                "{}".into(), &root.join("empty.rs"), &empty,
            );
            let failure = expect_err(failure)?;
            assert!(failure.contains("exact logical root"), "{failure}");
            Ok::<_, String>(())
        })?;
        drop(authority);
        remove_temp_root(&root)
    }

    #[test]
    fn render_dispatch_renders_human_json_github_with_default_config() -> Result<(), String> {
        let output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
        let config = RiprConfig::default();

        let human = render_check_with_config(&output, &OutputFormat::Human, &config)?;
        let json = render_check_with_config(&output, &OutputFormat::Json, &config)?;
        let github = render_check_with_config(&output, &OutputFormat::Github, &config)?;

        assert!(!human.is_empty());
        assert!(json.contains("\"schema_version\""));
        assert!(github.contains("ripr"));
        Ok(())
    }

    #[test]
    fn render_dispatch_renders_diff_badge_formats() -> Result<(), String> {
        let output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
        let config = RiprConfig::default();

        let native = render_check_with_config(&output, &OutputFormat::BadgeJson, &config)?;
        let shields = render_check_with_config(&output, &OutputFormat::BadgeShields, &config)?;

        assert!(native.contains("\"kind\""));
        assert!(shields.contains("\"schemaVersion\": 1"));
        Ok(())
    }

    /// #5261: the repo badge's `canonical_actionable_gap` count must agree
    /// with `repo-exposure-json` on the same tree. The compact classified
    /// walk this badge used to consume drops the related-test and
    /// observed-value payload and approximates activation for several seam
    /// kinds, so actionable gaps reclassified as `unknown` rendered a clean
    /// `0 actionable` beside an actionable exposure record. A workspace
    /// whose only tests reach the boundary but never exercise it pins the
    /// agreement: both surfaces must count the same canonical actionable
    /// gaps, and a zero badge beside a nonzero exposure count fails.
    #[test]
    fn repo_badge_actionable_count_agrees_with_repo_exposure_on_the_same_tree() -> Result<(), String>
    {
        let root = temp_root("ripr-render-badge-agreement")?;
        std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
        std::fs::create_dir_all(root.join("tests"))
            .map_err(|err| format!("create tests dir: {err}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]
name=\"ripr-badge-agreement\"
version=\"0.1.0\"
edition=\"2021\"
",
        )
        .map_err(|err| format!("write Cargo.toml: {err}"))?;
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn over_threshold(amount: i32, threshold: i32) -> bool {
    amount >= threshold
}

pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}
",
        )
        .map_err(|err| format!("write lib.rs: {err}"))?;
        std::fs::write(
            root.join("tests/pricing.rs"),
            "use ripr_badge_agreement::{discounted_total, over_threshold};

#[test]
fn below_threshold_has_no_discount() {
    assert_eq!(discounted_total(50, 100), 50);
}

#[test]
fn happy_path_passes() {
    assert!(over_threshold(5, 3));
}
",
        )
        .map_err(|err| format!("write tests/pricing.rs: {err}"))?;

        let mut output = check_output_with(Vec::new());
        output.root = root.clone();
        let config = RiprConfig::default();

        let exposure = render_check_with_config(&output, &OutputFormat::RepoExposureJson, &config)?;
        let exposure_value: serde_json::Value = serde_json::from_str(&exposure)
            .map_err(|err| format!("repo-exposure JSON should parse: {err}"))?;
        let exposure_actionable: Vec<String> = exposure_value["seams"]
            .as_array()
            .ok_or_else(|| "repo-exposure JSON should carry a seams array".to_string())?
            .iter()
            .filter(|seam| {
                let item = &seam["evidence_record"]["canonical_item"];
                item["gap_state"] == "actionable"
                    && !item["repair_route"].is_null()
                    && !item["verify_command"].is_null()
            })
            .filter_map(|seam| {
                seam["evidence_record"]["canonical_item"]["canonical_gap_id"]
                    .as_str()
                    .map(str::to_string)
            })
            .collect();

        let badge = render_check_with_config(&output, &OutputFormat::RepoBadgeJson, &config)?;
        let badge_value: serde_json::Value = serde_json::from_str(&badge)
            .map_err(|err| format!("repo-badge JSON should parse: {err}"))?;
        let badge_count = badge_value["counts"]["unsuppressed_exposure_gaps"]
            .as_u64()
            .ok_or_else(|| "badge should carry unsuppressed_exposure_gaps".to_string())?;

        assert!(
            !exposure_actionable.is_empty(),
            "fixture must produce at least one actionable canonical gap with a complete              repair route on the exposure path, or the agreement proves nothing"
        );
        assert_eq!(
            badge_count as usize,
            exposure_actionable.len(),
            "badge actionable count {badge_count} must equal the repo-exposure canonical              actionable gap count {} on the same tree",
            exposure_actionable.len()
        );

        remove_temp_root(&root)?;
        Ok(())
    }

    use super::REPO_RIPR_BADGE_SOURCE_REPORT;
    use super::attach_repo_badge_projection;
    use super::render_native_badge_with_persisted_source_report;
    use crate::output::badge;

    /// #5263 review: a seam-capped classified inventory must not project a
    /// partial count as a clean full-run badge. The public projection
    /// resolves to the `limited` state with the cap named, and the badge
    /// message can no longer read `0 actionable` for a truncated scan.
    #[test]
    fn repo_badge_projects_limited_when_the_seam_cap_truncates_the_scan() -> Result<(), String> {
        let classified: Vec<crate::analysis::ClassifiedSeam> = Vec::new();
        let mut summary = badge::ripr_canonical_actionable_gap_badge_summary(
            &classified,
            badge::BadgePolicy::default(),
        );
        let limit = crate::analysis::SeamLimitInfo {
            analyzed: 500,
            total: 501,
            source: crate::analysis::SeamLimitSource::Default,
        };
        attach_repo_badge_projection(
            &mut summary,
            Some(&limit),
            Some(REPO_RIPR_BADGE_SOURCE_REPORT),
        );
        let projection = summary
            .projection
            .as_ref()
            .ok_or("the limited run must still carry a public projection")?;
        assert_eq!(
            projection.state.as_str(),
            "limited",
            "{:?}",
            projection.state
        );
        assert_eq!(summary.message, "limited", "{}", summary.message);
        assert_eq!(
            projection.run_status, "limited_seam_cap",
            "{}",
            projection.run_status
        );
        assert!(
            projection
                .limited_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("analyzed 500 of 501 seams")),
            "the limited reason must name the cap: {:?}",
            projection.limited_reason
        );
        Ok(())
    }

    /// #6610: the repo badge's `public_projection.source_report` names
    /// `target/ripr/reports/repo-ripr-badge.json`, so the producing stdout
    /// run must persist that artifact inside the analyzed workspace. A bare
    /// stdout run that named the file without writing it made every
    /// dereferencing consumer hit ENOENT with no way to tell "stale" from
    /// "never written".
    #[test]
    fn repo_badge_stdout_run_persists_the_source_report_it_names() -> Result<(), String> {
        let output = check_output_with_temp_seam_workspace(Vec::new())?;

        let rendered = render_check_with_config(
            &output,
            &OutputFormat::RepoBadgeJson,
            &RiprConfig::default(),
        )?;

        let report_path = output.root.join("target/ripr/reports/repo-ripr-badge.json");
        let persisted = std::fs::read_to_string(&report_path)
            .map_err(|err| format!("the named source report must exist: {err}"))?;
        assert_eq!(
            persisted, rendered,
            "the persisted source report must be the exact rendered bytes"
        );

        remove_temp_root(&output.root)?;
        Ok(())
    }

    /// #6610: when the analyzed workspace cannot take the named report (here
    /// `target` is a regular file, so the report directory cannot be created),
    /// the projection must not claim it. The fail-closed machinery resolves
    /// `source_report: null` to the `unknown` state (RIPR-SPEC-0066
    /// reject-list) instead of an actionable count whose provenance pointer
    /// does not resolve.
    #[test]
    fn repo_badge_projection_degrades_to_unknown_when_the_report_cannot_be_persisted()
    -> Result<(), String> {
        let output = check_output_with_temp_seam_workspace(Vec::new())?;
        // A regular file at `target` blocks `create_dir_all(target/ripr/reports)`.
        std::fs::write(output.root.join("target"), b"not a directory")
            .map_err(|err| format!("write target sentinel: {err}"))?;

        let rendered = render_check_with_config(
            &output,
            &OutputFormat::RepoBadgeJson,
            &RiprConfig::default(),
        )?;

        assert!(
            rendered.contains("\"source_report\": null"),
            "an unpersistable pointer must degrade to null: {rendered}"
        );
        assert!(
            rendered.contains("\"state\": \"unknown\""),
            "a missing source report resolves to the unknown state: {rendered}"
        );
        assert!(
            rendered.contains("\"actionable_count\": null"),
            "the unknown state never carries a count: {rendered}"
        );
        assert!(
            !rendered.contains("repo-ripr-badge.json"),
            "no report path may be claimed when none was written: {rendered}"
        );

        let _ = std::fs::remove_file(output.root.join("target"));
        remove_temp_root(&output.root)?;
        Ok(())
    }

    /// #6610: the repo-scoped `ripr+` badge names
    /// `target/ripr/reports/repo-ripr-plus-badge.json`; the producing native
    /// run persists it the same way.
    #[test]
    fn repo_badge_plus_stdout_run_persists_the_source_report_it_names() -> Result<(), String> {
        let output = check_output_with_temp_report_workspace(Vec::new())?;

        let rendered = render_check_with_config(
            &output,
            &OutputFormat::RepoBadgePlusJson,
            &RiprConfig::default(),
        )?;

        assert!(rendered.contains("\"public_projection\""), "{rendered}");
        let report_path = output
            .root
            .join("target/ripr/reports/repo-ripr-plus-badge.json");
        let persisted = std::fs::read_to_string(&report_path)
            .map_err(|err| format!("the named ripr+ source report must exist: {err}"))?;
        assert_eq!(persisted, rendered, "persisted bytes must equal stdout");

        remove_temp_root(&output.root)?;
        Ok(())
    }

    /// #6610: the persisted native badge helper keeps its pointer claim only
    /// when the write succeeded; a failed write flips the same summary to the
    /// unknown state without re-deriving the walk.
    #[test]
    fn persisted_source_report_helper_degrades_on_write_failure() -> Result<(), String> {
        let mut summary =
            badge::ripr_canonical_actionable_gap_badge_summary(&[], badge::BadgePolicy::default());
        let blocker = std::env::temp_dir().join(format!(
            "ripr-badge-source-blocker-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::write(&blocker, b"not a directory")
            .map_err(|err| format!("write blocker file: {err}"))?;

        let rendered = render_native_badge_with_persisted_source_report(
            &mut summary,
            &blocker,
            "reports/repo-ripr-badge.json",
            |summary, source| {
                attach_repo_badge_projection(summary, None, source);
            },
        );

        assert!(
            rendered.contains("\"source_report\": null"),
            "write failure must degrade to a null pointer: {rendered}"
        );
        assert!(rendered.contains("\"state\": \"unknown\""), "{rendered}");
        let _ = std::fs::remove_file(&blocker);
        Ok(())
    }

    #[test]
    fn render_dispatch_renders_repo_badge_formats_with_seam_workspace() -> Result<(), String> {
        let output = check_output_with_temp_seam_workspace(Vec::new())?;
        let config = RiprConfig::default();

        let native = render_check_with_config(&output, &OutputFormat::RepoBadgeJson, &config)?;
        let shields = render_check_with_config(&output, &OutputFormat::RepoBadgeShields, &config)?;

        assert!(native.contains("\"scope\": \"repo\""));
        assert!(native.contains("\"basis\": \"canonical_actionable_gap\""));
        assert!(shields.contains("\"schemaVersion\": 1"));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_sarif_surfaces_malformed_suppressions_as_error() -> Result<(), String> {
        let output = check_output_with_temp_malformed_suppressions()?;
        let result =
            render_check_with_config(&output, &OutputFormat::Sarif, &RiprConfig::default());

        let err = expect_err(result)?;
        assert!(
            err.contains("validation failed"),
            "expected suppressions validation failure, got: {err}"
        );
        assert!(err.contains(".ripr/suppressions.toml"));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_badge_json_surfaces_malformed_suppressions_as_error() -> Result<(), String> {
        let output = check_output_with_temp_malformed_suppressions()?;
        let result =
            render_check_with_config(&output, &OutputFormat::BadgeJson, &RiprConfig::default());

        let err = expect_err(result)?;
        assert!(err.contains("validation failed"));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_badge_shields_surfaces_malformed_suppressions_as_error() -> Result<(), String>
    {
        let output = check_output_with_temp_malformed_suppressions()?;
        let result =
            render_check_with_config(&output, &OutputFormat::BadgeShields, &RiprConfig::default());

        let err = expect_err(result)?;
        assert!(err.contains("validation failed"));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_badge_json_surfaces_missing_workspace_as_error() -> Result<(), String> {
        let output = check_output_with_nonexistent_root();
        let result = render_check_with_config(
            &output,
            &OutputFormat::RepoBadgeJson,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_badge_shields_surfaces_missing_workspace_as_error() -> Result<(), String>
    {
        let output = check_output_with_nonexistent_root();
        let result = render_check_with_config(
            &output,
            &OutputFormat::RepoBadgeShields,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_seams_json_surfaces_missing_workspace_as_error() -> Result<(), String> {
        let output = check_output_with_nonexistent_root();
        let result = render_check_with_config(
            &output,
            &OutputFormat::RepoSeamsJson,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_seams_md_surfaces_missing_workspace_as_error() -> Result<(), String> {
        let output = check_output_with_nonexistent_root();
        let result =
            render_check_with_config(&output, &OutputFormat::RepoSeamsMd, &RiprConfig::default());

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_exposure_json_surfaces_missing_workspace_as_error() -> Result<(), String>
    {
        let output = check_output_with_nonexistent_root();
        let result = render_check_with_config(
            &output,
            &OutputFormat::RepoExposureJson,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_exposure_summary_json_surfaces_missing_workspace_as_error()
    -> Result<(), String> {
        let output = check_output_with_nonexistent_root();
        let result = render_check_with_config(
            &output,
            &OutputFormat::RepoExposureSummaryJson,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_exposure_md_surfaces_missing_workspace_as_error() -> Result<(), String>
    {
        let output = check_output_with_nonexistent_root();
        let result = render_check_with_config(
            &output,
            &OutputFormat::RepoExposureMd,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_sarif_surfaces_missing_workspace_as_error() -> Result<(), String> {
        let output = check_output_with_nonexistent_root();
        let result =
            render_check_with_config(&output, &OutputFormat::RepoSarif, &RiprConfig::default());

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_agent_seam_packets_surfaces_missing_workspace_as_error() -> Result<(), String>
    {
        let output = check_output_with_nonexistent_root();
        let result = render_check_with_config(
            &output,
            &OutputFormat::AgentSeamPacketsJson,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_badge_plus_shields_missing_report_is_neutral() -> Result<(), String> {
        let root = temp_root("ripr-render-badge-plus-shields-missing")?;
        let mut output = check_output_with(Vec::new());
        output.root = root;

        let rendered = render_check_with_config(
            &output,
            &OutputFormat::BadgePlusShields,
            &RiprConfig::default(),
        )?;

        assert!(rendered.contains(r#""schemaVersion": 1"#));
        assert!(rendered.contains(r#""label": "ripr+""#));
        assert!(rendered.contains(r#""message": "needs test-efficiency""#));
        assert!(rendered.contains(r#""color": "lightgrey""#));
        assert!(!rendered.contains("cargo xtask test-efficiency-report"));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_badge_plus_json_missing_report_is_neutral() -> Result<(), String> {
        let root = temp_root("ripr-render-repo-badge-plus-shields-missing")?;
        let mut output = check_output_with(Vec::new());
        output.root = root;

        let rendered = render_check_with_config(
            &output,
            &OutputFormat::RepoBadgePlusJson,
            &RiprConfig::default(),
        )?;

        assert!(rendered.contains(r#""kind": "ripr_plus""#));
        assert!(rendered.contains(r#""scope": "repo""#));
        assert!(rendered.contains(r#""basis": "canonical_actionable_gap""#));
        assert!(rendered.contains(r#""message": "needs test-efficiency""#));
        assert!(rendered.contains(r#""color": "lightgrey""#));
        assert!(rendered.contains("test-efficiency.json"));
        assert!(rendered.contains("docs/BADGE_ADOPTION.md"));
        assert!(!rendered.contains("cargo xtask test-efficiency-report"));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_badge_plus_surfaces_invalid_test_efficiency_json_as_error()
    -> Result<(), String> {
        let root = temp_root("ripr-render-badge-plus-invalid-json")?;
        let report_dir = root.join("target/ripr/reports");
        std::fs::create_dir_all(&report_dir)
            .map_err(|err| format!("create test-efficiency report dir: {err}"))?;
        std::fs::write(report_dir.join("test-efficiency.json"), "this is not json")
            .map_err(|err| format!("write malformed test-efficiency: {err}"))?;
        let mut output = check_output_with(Vec::new());
        output.root = root;

        let result = render_check_with_config(
            &output,
            &OutputFormat::BadgePlusJson,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(
            err.contains("test-efficiency.json is not valid JSON"),
            "expected JSON parse failure, got: {err}"
        );

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_badge_plus_surfaces_malformed_suppressions_as_error() -> Result<(), String> {
        let root = temp_root("ripr-render-badge-plus-bad-suppressions")?;
        // Valid efficiency report so parse succeeds and execution reaches the
        // suppressions load.
        write_test_efficiency_report(&root)?;
        write_malformed_suppressions(&root)?;
        let mut output = check_output_with(Vec::new());
        output.root = root;

        let result = render_check_with_config(
            &output,
            &OutputFormat::BadgePlusJson,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(
            err.contains("validation failed"),
            "expected suppressions validation failure, got: {err}"
        );

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn remove_temp_root_treats_missing_dir_as_success() -> Result<(), String> {
        let path = std::env::temp_dir().join(format!(
            "ripr-render-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        // Path does not exist; helper should return Ok without error.
        remove_temp_root(&path)?;
        Ok(())
    }

    #[test]
    fn remove_temp_root_returns_error_for_other_io_failures() -> Result<(), String> {
        // Passing a regular file (not a directory) makes `remove_dir_all`
        // fail with an error kind other than NotFound, exercising the
        // catch-all `Err(err) => Err(...)` arm.
        let root = temp_root("ripr-render-remove-error")?;
        let file_path = root.join("not-a-dir");
        std::fs::write(&file_path, b"").map_err(|err| format!("write sentinel file: {err}"))?;

        let result = remove_temp_root(&file_path);
        match result {
            Ok(()) => {
                // Some platforms allow `remove_dir_all` on a regular file
                // (it just unlinks it). In that case we cannot exercise
                // the error arm here, so accept the success path. Clean up
                // and return.
                remove_temp_root(&root)?;
                Ok(())
            }
            Err(message) => {
                assert!(message.contains("remove temp root"));
                let _ = std::fs::remove_file(&file_path);
                remove_temp_root(&root)?;
                Ok(())
            }
        }
    }

    fn check_output_with_nonexistent_root() -> CheckOutput {
        let mut output = check_output_with(Vec::new());
        output.root = PathBuf::from("/this/path/does/not/exist/ripr-render-test");
        output
    }

    fn check_output_with_temp_malformed_suppressions() -> Result<CheckOutput, String> {
        let root = temp_root("ripr-render-bad-suppressions")?;
        write_malformed_suppressions(&root)?;
        let mut output = check_output_with(Vec::new());
        output.root = root;
        Ok(output)
    }

    fn write_malformed_suppressions(root: &Path) -> Result<(), String> {
        let dir = root.join(".ripr");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create .ripr dir: {err}"))?;
        // Missing `schema_version = 1` and an unsupported top-level field
        // both produce validation violations from `parse_suppressions_manifest`.
        std::fs::write(
            dir.join("suppressions.toml"),
            "unsupported_field = \"value\"\n",
        )
        .map_err(|err| format!("write malformed suppressions: {err}"))?;
        Ok(())
    }

    fn expect_err<T: std::fmt::Debug>(result: Result<T, String>) -> Result<String, String> {
        match result {
            Ok(value) => Err(format!("expected error, got Ok({value:?})")),
            Err(err) => Ok(err),
        }
    }

    fn check_output_with(findings: Vec<Finding>) -> CheckOutput {
        CheckOutput {
            harness_projections: Vec::new(),
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("."),
            base: Some("origin/main".to_string()),
            summary: Summary::default(),
            findings,
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            untracked_working_tree_source_paths: Vec::new(),
            unlinked_python_tests: None,
            suppression: None,
            analysis_outcome: None,
            partial_scope: None,
        }
    }

    fn check_output_with_temp_report_workspace(
        findings: Vec<Finding>,
    ) -> Result<CheckOutput, String> {
        let root = temp_root("ripr-render-report")?;
        write_test_efficiency_report(&root)?;
        let mut output = check_output_with(findings);
        output.root = root;
        Ok(output)
    }

    fn check_output_with_temp_seam_workspace(
        findings: Vec<Finding>,
    ) -> Result<CheckOutput, String> {
        let root = temp_root("ripr-render-seams")?;
        std::fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src dir: {err}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname=\"ripr-render-seams\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
        )
        .map_err(|err| format!("write temp Cargo.toml: {err}"))?;
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}\n",
        )
        .map_err(|err| format!("write temp src/lib.rs: {err}"))?;

        let mut output = check_output_with(findings);
        output.root = root;
        Ok(output)
    }

    fn temp_root(prefix: &str) -> Result<PathBuf, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("{prefix}-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&root).map_err(|err| format!("create temp root: {err}"))?;
        Ok(root)
    }

    fn write_test_efficiency_report(root: &Path) -> Result<(), String> {
        let report_dir = root.join("target/ripr/reports");
        std::fs::create_dir_all(&report_dir)
            .map_err(|err| format!("create test-efficiency report dir: {err}"))?;
        std::fs::write(
            report_dir.join("test-efficiency.json"),
            r#"{
  "schema_version": "0.1",
  "tests": [
    {
      "class": "smoke_only",
      "name": "sample_test",
      "reached_owners": ["sample_owner"]
    }
  ],
  "metrics": {
    "tests_scanned": 1,
    "reason_counts": {
      "smoke_oracle_only": 1
    }
  }
}"#,
        )
        .map_err(|err| format!("write test-efficiency report: {err}"))?;
        Ok(())
    }

    fn remove_temp_root(root: &Path) -> Result<(), String> {
        match std::fs::remove_dir_all(root) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(format!("remove temp root: {err}")),
        }
    }

    fn sample_finding(file: &str, line: usize) -> Finding {
        Finding {
            id: "probe:src_lib_rs:42:error_path".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_lib_rs:42:error_path".to_string()),
                family: ProbeFamily::ErrorPath,
                location: SourceLocation::new(file, line, 1),
                owner: Some(SymbolId("sample_owner".to_string())),
                delta: DeltaKind::Control,
                before: None,
                after: None,
                expression: "sample_expr".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: StageEvidence::new(StageState::Yes, Confidence::Medium, "reached"),
                infect: StageEvidence::new(StageState::Weak, Confidence::Low, "infected"),
                propagate: StageEvidence::new(StageState::No, Confidence::Medium, "not propagated"),
                reveal: RevealEvidence {
                    observe: StageEvidence::new(StageState::Weak, Confidence::Low, "observed"),
                    discriminate: StageEvidence::new(
                        StageState::No,
                        Confidence::Medium,
                        "no discriminator",
                    ),
                },
            },
            confidence: 0.5,
            evidence: vec!["changed test".to_string()],
            missing: vec!["strong oracle".to_string()],
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: vec![StopReason::NoChangedRustLine],
            related_tests_matched_total: None,
            related_tests: vec![RelatedTest {
                name: "sample_test".to_string(),
                file: "tests/sample.rs".into(),
                line: 10,
                oracle: None,
                oracle_kind: OracleKind::Unknown,
                oracle_strength: OracleStrength::Weak,
                relation_reason: None,
                relation_confidence: None,
                miss: None,
            }],
            recommended_next_step: Some("add stronger assertion".to_string()),
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
}
