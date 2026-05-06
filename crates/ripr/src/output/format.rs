/// Output renderer selection for `ripr` reports.
///
/// Most automation should prefer [`OutputFormat::Json`] for stable
/// machine-readable data. Badge and repo-inventory formats exist for specific
/// downstream integrations and may require additional artifacts.
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Repo-scoped native `ripr` badge JSON. Renders seam-native repo
    /// counts rather than diff-scoped `Finding` counts. Carries
    /// `scope: "repo"` and `basis: "seam_native"` so README/store
    /// endpoints can distinguish public repo signal from PR/diff artifacts.
    RepoBadgeJson,
    /// Repo-scoped Shields projection for the `ripr` badge. Same four
    /// fields as the diff-scoped Shields shape; native-only fields like
    /// `scope` and `basis` do not leak into Shields.
    RepoBadgeShields,
    /// Repo-scoped native `ripr+` badge JSON. Same disk requirement as
    /// `BadgePlusJson` (the test-efficiency report) - `cargo xtask
    /// test-efficiency-report` already scans the full test suite, so
    /// the report is already repo-scoped.
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

impl OutputFormat {
    /// Returns `true` when the format targets full-repo scope rather than
    /// diff scope.
    ///
    /// Repo-scope formats use full-repo inputs. Native repo badge JSON carries
    /// `scope: "repo"` and seam-native badge formats carry
    /// `basis: "seam_native"`. The Shields projection stays four-field for
    /// both scopes.
    pub fn is_repo_scope(&self) -> bool {
        matches!(
            self,
            OutputFormat::RepoBadgeJson
                | OutputFormat::RepoBadgeShields
                | OutputFormat::RepoBadgePlusJson
                | OutputFormat::RepoBadgePlusShields
                | OutputFormat::RepoSeamsJson
                | OutputFormat::RepoSeamsMd
                | OutputFormat::RepoExposureJson
                | OutputFormat::RepoExposureMd
                | OutputFormat::RepoSarif
                | OutputFormat::AgentSeamPacketsJson
        )
    }

    /// Returns `true` when the format renders repo seam-driven artifacts
    /// that do not consume legacy repo `Finding` output.
    ///
    /// These formats short-circuit legacy repo Finding analysis because they
    /// either walk/classify repo seams directly or render badge summaries from
    /// classified seams. Running legacy repo Finding analysis first would add
    /// cost and then be discarded.
    pub fn is_repo_seam_inventory(&self) -> bool {
        matches!(
            self,
            OutputFormat::RepoBadgeJson
                | OutputFormat::RepoBadgeShields
                | OutputFormat::RepoBadgePlusJson
                | OutputFormat::RepoBadgePlusShields
                | OutputFormat::RepoSeamsJson
                | OutputFormat::RepoSeamsMd
                | OutputFormat::RepoExposureJson
                | OutputFormat::RepoExposureMd
                | OutputFormat::RepoSarif
                | OutputFormat::AgentSeamPacketsJson
        )
    }
}

#[cfg(test)]
mod tests {
    use super::OutputFormat;

    #[test]
    fn output_format_is_repo_scope_only_for_repo_variants() {
        for repo in [
            OutputFormat::RepoBadgeJson,
            OutputFormat::RepoBadgeShields,
            OutputFormat::RepoBadgePlusJson,
            OutputFormat::RepoBadgePlusShields,
            OutputFormat::RepoSeamsJson,
            OutputFormat::RepoSeamsMd,
            OutputFormat::RepoExposureJson,
            OutputFormat::RepoExposureMd,
            OutputFormat::RepoSarif,
            OutputFormat::AgentSeamPacketsJson,
        ] {
            assert!(
                repo.is_repo_scope(),
                "expected {:?} to report repo scope",
                repo
            );
        }
        for diff in [
            OutputFormat::Human,
            OutputFormat::Json,
            OutputFormat::Github,
            OutputFormat::Sarif,
            OutputFormat::BadgeJson,
            OutputFormat::BadgeShields,
            OutputFormat::BadgePlusJson,
            OutputFormat::BadgePlusShields,
        ] {
            assert!(
                !diff.is_repo_scope(),
                "expected {:?} to report diff scope",
                diff
            );
        }
    }

    #[test]
    fn repo_artifact_formats_use_repo_seam_short_circuit() {
        for format in [
            OutputFormat::RepoBadgeJson,
            OutputFormat::RepoBadgeShields,
            OutputFormat::RepoBadgePlusJson,
            OutputFormat::RepoBadgePlusShields,
            OutputFormat::RepoSeamsJson,
            OutputFormat::RepoSeamsMd,
            OutputFormat::RepoExposureJson,
            OutputFormat::RepoExposureMd,
            OutputFormat::RepoSarif,
            OutputFormat::AgentSeamPacketsJson,
        ] {
            assert!(
                format.is_repo_seam_inventory(),
                "expected {:?} to skip legacy repo Finding analysis",
                format
            );
        }
        assert!(!OutputFormat::Human.is_repo_seam_inventory());
        assert!(!OutputFormat::Json.is_repo_seam_inventory());
        assert!(!OutputFormat::BadgeJson.is_repo_seam_inventory());
        assert!(!OutputFormat::BadgePlusJson.is_repo_seam_inventory());
    }
}
