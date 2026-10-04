/// Output renderer selection for `ripr` reports.
///
/// Most automation should prefer [`OutputFormat::Json`] for stable
/// machine-readable data. Badge and repo-inventory formats exist for specific
/// downstream integrations and may require additional artifacts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// Bounded human-readable triage report.
    Human,
    /// Exhaustive human-readable report with every finding body.
    HumanFull,
    /// Versioned JSON report for automation.
    Json,
    /// GitHub annotation output suitable for CI logs.
    Github,
    /// SARIF 2.1.0 report for diff-scoped static exposure Findings.
    Sarif,
    /// Native `ripr` badge JSON (snake_case wire shape with full counts,
    /// reason counts, and policy). Consumed by tools and CI artifacts.
    BadgeJson,
    /// Shields-compatible projection for the `ripr` badge: exactly four
    /// top-level fields (`schemaVersion`, `label`, `message`, `color`).
    BadgeShields,
    /// Native `ripr+` badge JSON. Sums unsuppressed exposure gaps and
    /// unsuppressed actionable test-efficiency findings, excluding
    /// declared intent. When `target/ripr/reports/test-efficiency.json`
    /// is missing, renders a neutral badge-generator-safe response.
    BadgePlusJson,
    /// Shields-compatible projection for the `ripr+` badge.
    BadgePlusShields,
    /// Repo-scoped native `ripr` badge JSON. Renders unresolved actionable
    /// canonical repair items rather than diff-scoped `Finding` counts or
    /// seam-native inventory. Carries `scope: "repo"` and
    /// `basis: "canonical_actionable_gap"` so README/store endpoints can
    /// distinguish public repair signal from PR/diff and inventory artifacts.
    RepoBadgeJson,
    /// Repo-scoped Shields projection for the `ripr` badge. Same four
    /// fields as the diff-scoped Shields shape; native-only fields like
    /// `scope` and `basis` do not leak into Shields.
    RepoBadgeShields,
    /// Repo-scoped native `ripr+` badge JSON. Uses the same
    /// test-efficiency report as `BadgePlusJson` when present, but raw
    /// test-efficiency debt does not move the repo headline until it is
    /// lifted into the same actionable repair / verify / receipt model as
    /// canonical gaps.
    RepoBadgePlusJson,
    /// Repo-scoped Shields projection for the `ripr+` badge.
    RepoBadgePlusShields,
    /// Repo seam inventory rendered as JSON. Walks production Rust
    /// files and emits `RepoSeam` records per RIPR-SPEC-0005. Schema
    /// version is documented in `docs/OUTPUT_SCHEMA.md` under
    /// `repo-seams.json`. Independent of the diff-scoped `Findings`
    /// pipeline.
    RepoSeamsJson,
    /// Repo seam inventory rendered as Markdown for human review.
    RepoSeamsMd,
    /// Classified seam inventory rendered as a repo exposure JSON
    /// report. Adds per-seam grip class and per-class metrics on top
    /// of the seam inventory. Schema in `docs/OUTPUT_SCHEMA.md` under
    /// `repo-exposure.json`.
    RepoExposureJson,
    /// Bounded repo exposure summary rendered as JSON. Emits aggregate
    /// canonical actionable gap counts, reason breakdowns, and a capped
    /// top-file summary without per-seam evidence payloads.
    RepoExposureSummaryJson,
    /// Repo exposure report rendered as Markdown for human review.
    RepoExposureMd,
    /// SARIF 2.1.0 report for repo-scoped classified seam evidence.
    RepoSarif,
    /// Agent-ready seam packets per RIPR-SPEC-0005 - one
    /// `write_targeted_test` packet per headline-eligible classified
    /// seam, plus conservative `inspect_static_limitation` packets for
    /// opaque seams. Schema 0.3 in `docs/OUTPUT_SCHEMA.md` section "Agent
    /// Seam Packets". Strongly-gripped, intentional, and suppressed
    /// seams emit no packet.
    AgentSeamPacketsJson,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OutputFormatSpec {
    format: OutputFormat,
    cli_names: &'static [&'static str],
    is_repo_seam_inventory: bool,
    /// `true` for the seven full-repo audit-path formats (the help's
    /// "Repo-scope (full-repo analysis)" group plus
    /// `agent-seam-packets-json`): every invocation walks and classifies the
    /// whole Rust corpus, so the CLI discloses the expected cost class at
    /// invocation time (#4945). Repo badge formats render disk reports plus
    /// the compact seam summary and stay outside this class.
    is_full_repo_analysis: bool,
    /// `true` only when warm reruns of the format's walk can hit a
    /// seam-facts cache: the classified and compact-classified inventory
    /// paths (`seam_inventory.rs` `RepoSeamFactCache::at` /
    /// `::at_compact_classified`). The raw `repo-seams-*` walks rebuild the
    /// corpus index on every invocation and read no cache, so their
    /// disclosure must not promise cache-backed warm reruns (#4945 review).
    is_seam_fact_cache_backed: bool,
}

const FORMAT_SPECS: &[OutputFormatSpec] = &[
    OutputFormatSpec {
        format: OutputFormat::Human,
        cli_names: &["human", "text"],
        is_full_repo_analysis: false,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::HumanFull,
        cli_names: &["human-full", "text-full"],
        is_full_repo_analysis: false,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::Json,
        cli_names: &["json"],
        is_full_repo_analysis: false,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::Github,
        cli_names: &["github"],
        is_full_repo_analysis: false,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::Sarif,
        cli_names: &["sarif"],
        is_full_repo_analysis: false,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::BadgeJson,
        cli_names: &["badge-json"],
        is_full_repo_analysis: false,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::BadgeShields,
        cli_names: &["badge-shields"],
        is_full_repo_analysis: false,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::BadgePlusJson,
        cli_names: &["badge-plus-json"],
        is_full_repo_analysis: false,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::BadgePlusShields,
        cli_names: &["badge-plus-shields"],
        is_full_repo_analysis: false,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoBadgeJson,
        cli_names: &["repo-badge-json"],
        is_full_repo_analysis: false,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoBadgeShields,
        cli_names: &["repo-badge-shields"],
        is_full_repo_analysis: false,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoBadgePlusJson,
        cli_names: &["repo-badge-plus-json"],
        is_full_repo_analysis: false,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoBadgePlusShields,
        cli_names: &["repo-badge-plus-shields"],
        is_full_repo_analysis: false,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoSeamsJson,
        cli_names: &["repo-seams-json"],
        is_full_repo_analysis: true,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoSeamsMd,
        cli_names: &["repo-seams-md"],
        is_full_repo_analysis: true,
        is_seam_fact_cache_backed: false,
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoExposureJson,
        cli_names: &["repo-exposure-json"],
        is_full_repo_analysis: true,
        is_seam_fact_cache_backed: true,
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoExposureSummaryJson,
        cli_names: &["repo-exposure-summary-json"],
        is_full_repo_analysis: true,
        is_seam_fact_cache_backed: true,
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoExposureMd,
        cli_names: &["repo-exposure-md"],
        is_full_repo_analysis: true,
        is_seam_fact_cache_backed: true,
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoSarif,
        cli_names: &["repo-sarif"],
        is_full_repo_analysis: true,
        is_seam_fact_cache_backed: true,
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::AgentSeamPacketsJson,
        cli_names: &["agent-seam-packets-json"],
        is_full_repo_analysis: true,
        is_seam_fact_cache_backed: true,
        is_repo_seam_inventory: true,
    },
];

impl OutputFormat {
    /// Parses a CLI output format name or alias.
    pub(crate) fn parse_cli_name(value: &str) -> Option<Self> {
        FORMAT_SPECS
            .iter()
            .find_map(|spec| spec.cli_names.contains(&value).then_some(spec.format))
    }

    /// Returns the preferred CLI spelling for this output format.
    pub(crate) fn primary_cli_name(&self) -> &'static str {
        FORMAT_SPECS
            .iter()
            .find(|spec| spec.format == *self)
            .and_then(|spec| spec.cli_names.first().copied())
            .unwrap_or("unknown")
    }

    /// Every CLI name this build accepts for `--format`, in declaration
    /// order.
    ///
    /// Derived from [`FORMAT_SPECS`] so an error message can enumerate the
    /// accepted set without a second list that could drift from the parser.
    pub(crate) fn accepted_cli_names() -> Vec<&'static str> {
        FORMAT_SPECS
            .iter()
            .flat_map(|spec| spec.cli_names.iter().copied())
            .collect()
    }

    /// Returns `true` when the format targets full-repo scope rather than
    /// diff scope.
    ///
    /// Repo-scope formats use full-repo inputs. Native repo badge JSON carries
    /// `scope: "repo"` and public repo badge formats carry
    /// `basis: "canonical_actionable_gap"`. The Shields projection stays
    /// four-field for both scopes.
    pub fn is_repo_scope(&self) -> bool {
        self.is_repo_seam_inventory()
    }

    /// Returns `true` when the format renders repo seam-driven artifacts
    /// that do not consume legacy repo `Finding` output.
    ///
    /// These formats short-circuit legacy repo Finding analysis because they
    /// either walk/classify repo seams directly or render badge summaries from
    /// classified seams. Running legacy repo Finding analysis first would add
    /// cost and then be discarded.
    pub fn is_repo_seam_inventory(&self) -> bool {
        FORMAT_SPECS
            .iter()
            .find(|spec| spec.format == *self)
            .is_some_and(|spec| spec.is_repo_seam_inventory)
    }

    /// Returns `true` when the format runs the full-repo seam analysis walk:
    /// the help's "Repo-scope (full-repo analysis)" group plus
    /// `agent-seam-packets-json`. Every such invocation walks and classifies
    /// the whole analyzable Rust corpus, so these are the audit-path surfaces
    /// the CLI must disclose invocation-time cost for (#4945). Repo badge
    /// formats render disk reports plus the compact seam summary and stay
    /// outside this class.
    pub(crate) fn is_full_repo_analysis(&self) -> bool {
        FORMAT_SPECS
            .iter()
            .find(|spec| spec.format == *self)
            .is_some_and(|spec| spec.is_full_repo_analysis)
    }

    /// Returns `true` only when warm reruns of the format's walk can hit a
    /// seam-facts cache: the classified and compact-classified inventory
    /// paths. The raw `repo-seams-*` walks rebuild the corpus index on every
    /// invocation and read no cache, so only this group may claim faster
    /// warm reruns (#4945 review).
    pub(crate) fn is_seam_fact_cache_backed(&self) -> bool {
        FORMAT_SPECS
            .iter()
            .find(|spec| spec.format == *self)
            .is_some_and(|spec| spec.is_seam_fact_cache_backed)
    }

    /// Invocation-time stderr disclosure naming the expected cost class for
    /// full-repo audit-path formats (#4945). One line, emitted before the run
    /// begins; honest to the measured magnitude (release notes: a cold
    /// `repo-exposure-json` run took about 76 minutes on 4 vCPUs, a warm run
    /// with the classified cache 42 seconds), without promising a wall clock.
    /// The warm-rerun clause is honest per format: only the
    /// cache-backed classified/compact-classified walks claim seam-facts
    /// cache reuse; the raw `repo-seams-*` walks disclose that every run
    /// pays the full walk (#4945 review).
    pub(crate) fn repo_audit_path_disclosure(&self) -> Option<String> {
        if !self.is_full_repo_analysis() {
            return None;
        }
        let warm_rerun_clause = if self.is_seam_fact_cache_backed() {
            "warm reruns reuse the seam-facts cache and are much faster."
        } else {
            "raw seam inventory does not read the seam-facts cache, so every run pays the full walk."
        };
        Some(format!(
            "ripr: {} is the full-repo audit path: a cold run analyzes every seam in the \
             workspace and can take minutes on large repositories; {warm_rerun_clause}",
            self.primary_cli_name()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{FORMAT_SPECS, OutputFormat};

    #[test]
    fn parses_human_full_aliases() {
        assert_eq!(
            OutputFormat::parse_cli_name("human-full"),
            Some(OutputFormat::HumanFull)
        );
        assert_eq!(
            OutputFormat::parse_cli_name("text-full"),
            Some(OutputFormat::HumanFull)
        );
    }

    #[test]
    fn human_full_is_not_repo_scope() {
        assert!(!OutputFormat::HumanFull.is_repo_scope());
        assert!(!OutputFormat::HumanFull.is_repo_seam_inventory());
    }

    #[test]
    fn output_format_is_repo_scope_only_for_repo_variants() {
        for spec in FORMAT_SPECS {
            assert_eq!(
                spec.format.is_repo_scope(),
                spec.is_repo_seam_inventory,
                "repo scope should match metadata for {:?}",
                spec.format
            );
        }
    }

    #[test]
    fn repo_artifact_formats_use_repo_seam_short_circuit() {
        for spec in FORMAT_SPECS {
            assert_eq!(
                spec.format.is_repo_seam_inventory(),
                spec.is_repo_seam_inventory,
                "repo seam short-circuit should match metadata for {:?}",
                spec.format
            );
        }
    }

    /// #4945: the invocation-time audit-path cost disclosure covers exactly
    /// the seven full-repo analysis formats — the help's "Repo-scope
    /// (full-repo analysis)" group plus `agent-seam-packets-json` — and no
    /// diff-scoped, badge, or gap-ledger surface claims minutes it does not
    /// charge. The warm-rerun clause is honest per format: only the
    /// cache-backed classified/compact-classified walks claim seam-facts
    /// cache reuse; the raw `repo-seams-*` walks disclose that every run
    /// pays the full walk (#4945 review).
    #[test]
    fn repo_format_audit_path_disclosure_covers_exactly_the_full_repo_analysis_group()
    -> Result<(), String> {
        for spec in FORMAT_SPECS {
            let disclosure = spec.format.repo_audit_path_disclosure();
            assert_eq!(
                spec.format.is_full_repo_analysis(),
                spec.is_full_repo_analysis,
                "full-repo-analysis predicate should match metadata for {:?}",
                spec.format
            );
            assert_eq!(
                spec.format.is_seam_fact_cache_backed(),
                spec.is_seam_fact_cache_backed,
                "cache-backed predicate should match metadata for {:?}",
                spec.format
            );
            if spec.is_full_repo_analysis {
                let disclosure = disclosure
                    .ok_or_else(|| format!("missing disclosure for {:?}", spec.format))?;
                assert!(
                    disclosure.contains("full-repo audit path"),
                    "disclosure must name the audit path: {disclosure}"
                );
                assert!(
                    disclosure.contains("minutes"),
                    "disclosure must name the cold-run cost class: {disclosure}"
                );
                assert!(
                    disclosure.contains(spec.format.primary_cli_name()),
                    "disclosure must name the format: {disclosure}"
                );
                if spec.is_seam_fact_cache_backed {
                    assert!(
                        disclosure.contains("warm reruns reuse the seam-facts cache"),
                        "cache-backed format must claim warm cache reruns: {disclosure}"
                    );
                    assert!(
                        !disclosure.contains("every run pays the full walk"),
                        "cache-backed format must not claim an uncached walk: {disclosure}"
                    );
                } else {
                    assert!(
                        disclosure.contains("every run pays the full walk"),
                        "raw seam format must disclose the uncached walk: {disclosure}"
                    );
                    assert!(
                        !disclosure.contains("warm reruns reuse"),
                        "raw seam format must not claim cache-backed warm reruns: {disclosure}"
                    );
                }
            } else {
                assert!(
                    disclosure.is_none(),
                    "non-audit format {:?} must not claim audit-path cost: {disclosure:?}",
                    spec.format
                );
                assert!(
                    !spec.is_seam_fact_cache_backed,
                    "non-audit format {:?} must not claim cache-backed warm reruns",
                    spec.format
                );
            }
        }
        Ok(())
    }

    #[test]
    fn repo_format_audit_path_group_is_the_seven_measured_formats() {
        let expected: Vec<&str> = vec![
            "repo-seams-json",
            "repo-seams-md",
            "repo-exposure-json",
            "repo-exposure-summary-json",
            "repo-exposure-md",
            "repo-sarif",
            "agent-seam-packets-json",
        ];
        let mut observed: Vec<&str> = FORMAT_SPECS
            .iter()
            .filter(|spec| spec.is_full_repo_analysis)
            .map(|spec| spec.format.primary_cli_name())
            .collect();
        observed.sort_unstable();
        let mut sorted_expected = expected.clone();
        sorted_expected.sort_unstable();
        assert_eq!(
            observed, sorted_expected,
            "the audit-path disclosure group must be exactly the seven measured formats"
        );
    }

    /// #4945 review: only the classified/compact-classified audit walks read
    /// a seam-facts cache. The raw `repo-seams-*` walks rebuild the corpus
    /// index every run (`inventory_seams_at_with_config` never touches
    /// `RepoSeamFactCache`), so exactly these five may claim warm reruns.
    #[test]
    fn repo_format_audit_path_cache_backed_group_is_exactly_the_classified_walks() {
        let expected: Vec<&str> = vec![
            "repo-exposure-json",
            "repo-exposure-summary-json",
            "repo-exposure-md",
            "repo-sarif",
            "agent-seam-packets-json",
        ];
        let mut observed: Vec<&str> = FORMAT_SPECS
            .iter()
            .filter(|spec| spec.is_seam_fact_cache_backed)
            .map(|spec| spec.format.primary_cli_name())
            .collect();
        observed.sort_unstable();
        let mut sorted_expected = expected.clone();
        sorted_expected.sort_unstable();
        assert_eq!(
            observed, sorted_expected,
            "the warm-rerun cache claim must cover exactly the classified walks"
        );
    }

    #[test]
    fn output_format_parse_cli_name_uses_declared_names() {
        for spec in FORMAT_SPECS {
            for cli_name in spec.cli_names {
                assert_eq!(
                    OutputFormat::parse_cli_name(cli_name),
                    Some(spec.format),
                    "CLI name {:?} should parse to {:?}",
                    cli_name,
                    spec.format
                );
            }
        }
        assert_eq!(OutputFormat::parse_cli_name("xml"), None);
    }
}
