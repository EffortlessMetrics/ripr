/// Output renderer selection for `ripr` reports.
///
/// Most automation should prefer [`OutputFormat::Json`] for stable
/// machine-readable data. Badge and repo-inventory formats exist for specific
/// downstream integrations and may require additional artifacts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable plain text report.
    Human,
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
    /// declared intent. Requires `target/ripr/reports/test-efficiency.json`
    /// produced by `cargo xtask test-efficiency-report`.
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
    /// Repo-scoped native `ripr+` badge JSON. Same disk requirement as
    /// `BadgePlusJson` (the test-efficiency report), but raw test-efficiency
    /// debt does not move the repo headline until it is lifted into the same
    /// actionable repair / verify / receipt model as canonical gaps.
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
}

const FORMAT_SPECS: &[OutputFormatSpec] = &[
    OutputFormatSpec {
        format: OutputFormat::Human,
        cli_names: &["human", "text"],
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::Json,
        cli_names: &["json"],
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::Github,
        cli_names: &["github"],
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::Sarif,
        cli_names: &["sarif"],
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::BadgeJson,
        cli_names: &["badge-json"],
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::BadgeShields,
        cli_names: &["badge-shields"],
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::BadgePlusJson,
        cli_names: &["badge-plus-json"],
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::BadgePlusShields,
        cli_names: &["badge-plus-shields"],
        is_repo_seam_inventory: false,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoBadgeJson,
        cli_names: &["repo-badge-json"],
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoBadgeShields,
        cli_names: &["repo-badge-shields"],
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoBadgePlusJson,
        cli_names: &["repo-badge-plus-json"],
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoBadgePlusShields,
        cli_names: &["repo-badge-plus-shields"],
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoSeamsJson,
        cli_names: &["repo-seams-json"],
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoSeamsMd,
        cli_names: &["repo-seams-md"],
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoExposureJson,
        cli_names: &["repo-exposure-json"],
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoExposureMd,
        cli_names: &["repo-exposure-md"],
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::RepoSarif,
        cli_names: &["repo-sarif"],
        is_repo_seam_inventory: true,
    },
    OutputFormatSpec {
        format: OutputFormat::AgentSeamPacketsJson,
        cli_names: &["agent-seam-packets-json"],
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
}

#[cfg(test)]
mod tests {
    use super::{FORMAT_SPECS, OutputFormat};

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
