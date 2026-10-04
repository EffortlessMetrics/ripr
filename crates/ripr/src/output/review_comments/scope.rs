use crate::app::agent_brief::AgentBriefResolvedWorkingSet;
use crate::output::gap_decision_ledger::GapRecord;
use std::path::Path;

use super::{display_paths, normalize_path_text};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ReviewPlacement {
    pub(super) path: String,
    pub(super) line: usize,
    pub(super) mode: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReviewCommentsAnalysisScope {
    pub(crate) scope: &'static str,
    pub(crate) run_status: &'static str,
    pub(crate) basis: &'static str,
    pub(crate) changed_files: Vec<String>,
    pub(crate) changed_lines: usize,
    pub(crate) changed_owner_functions: usize,
    pub(crate) changed_production_files: Vec<String>,
    pub(crate) immediate_caller_files: Vec<String>,
    pub(crate) scoped_production_files: Vec<String>,
    pub(crate) total_rust_files: Option<usize>,
    pub(crate) total_production_files: Option<usize>,
    pub(crate) production_files_considered: usize,
    pub(crate) classified_seams_considered: usize,
    /// Scoped seams skipped because changed-line seams filled every
    /// review slot; zero when the whole scope was classified.
    pub(crate) unevaluated_seams: usize,
    /// Changed source files named by the diff that are not on disk (#4586).
    pub(crate) absent_changed_files: Vec<String>,
    pub(crate) downstream_consumable: bool,
    pub(crate) limitation: &'static str,
    pub(crate) repair_route: &'static str,
}

impl ReviewCommentsAnalysisScope {
    pub(crate) fn limited_diff_scope(
        working_set: &AgentBriefResolvedWorkingSet,
        inventory: &crate::analysis::ScopedClassifiedSeamInventory,
    ) -> Self {
        Self {
            scope: "diff_scoped_changed_files",
            run_status: "limited_diff_scope",
            basis: "changed_production_files_plus_immediate_callers",
            changed_files: display_paths(&working_set.files),
            changed_lines: working_set.changed_lines.len(),
            changed_owner_functions: working_set.changed_owners.len(),
            changed_production_files: display_paths(&inventory.changed_production_files),
            immediate_caller_files: display_paths(&inventory.immediate_caller_files),
            scoped_production_files: display_paths(&inventory.scoped_production_files),
            total_rust_files: Some(inventory.total_rust_files),
            total_production_files: Some(inventory.total_production_files),
            production_files_considered: inventory.scoped_production_files.len(),
            classified_seams_considered: inventory.classified_seams_considered,
            unevaluated_seams: inventory.unevaluated_seams,
            absent_changed_files: display_paths(&inventory.absent_changed_files),
            downstream_consumable: true,
            limitation: "review_comments_diff_scope_only",
            repair_route: "analysis/diff-scoped-large-repo-review-fast-path",
        }
    }

    pub(crate) fn producer_projection(
        working_set: &AgentBriefResolvedWorkingSet,
        reviewed_count: usize,
    ) -> Self {
        // Reuse the producer's changed-file authority, including bounded
        // repository automation and fixture/test exclusions. This projection
        // does not build a repo index or invent target metadata; its empty
        // context is the same bounded layout fallback as the diff producer.
        let role_context = crate::analysis::SourceRoleContext::empty();
        let production_files = working_set
            .files
            .iter()
            .filter(|path| crate::analysis::seeds_diff_probes(path, &role_context))
            .cloned()
            .collect::<Vec<_>>();
        Self {
            scope: "diff_scoped_changed_files",
            run_status: "limited_diff_scope",
            basis: "producer_check_projection",
            changed_files: display_paths(&working_set.files),
            changed_lines: working_set.changed_lines.len(),
            changed_owner_functions: working_set.changed_owners.len(),
            changed_production_files: display_paths(&production_files),
            immediate_caller_files: Vec::new(),
            scoped_production_files: display_paths(&production_files),
            total_rust_files: None,
            total_production_files: None,
            production_files_considered: production_files.len(),
            classified_seams_considered: reviewed_count,
            // Only complete producer output is admitted on this route; it
            // neither stages live seam evaluation nor admits absent-file limits.
            unevaluated_seams: 0,
            absent_changed_files: Vec::new(),
            downstream_consumable: true,
            limitation: "review_comments_producer_projection_only",
            repair_route: "analysis/review-comments-producer-projection",
        }
    }

    #[cfg(test)]
    pub(super) fn from_working_set(
        working_set: &AgentBriefResolvedWorkingSet,
        classified_seams_considered: usize,
    ) -> Self {
        Self {
            scope: "working_set",
            run_status: "scoped",
            basis: working_set.source.as_str(),
            changed_files: display_paths(&working_set.files),
            changed_lines: working_set.changed_lines.len(),
            changed_owner_functions: working_set.changed_owners.len(),
            changed_production_files: Vec::new(),
            immediate_caller_files: Vec::new(),
            scoped_production_files: display_paths(&working_set.files),
            total_rust_files: None,
            total_production_files: None,
            production_files_considered: working_set.files.len(),
            classified_seams_considered,
            unevaluated_seams: 0,
            absent_changed_files: Vec::new(),
            downstream_consumable: true,
            limitation: "review_comments_working_set_scope_only",
            repair_route: "analysis/review-comments-working-set",
        }
    }

    pub(crate) fn gap_ledger_artifact(records: &[GapRecord]) -> Self {
        let mut anchor_files = records
            .iter()
            .filter_map(|record| record.anchor.as_ref())
            .filter_map(|anchor| anchor.file.as_deref())
            .map(str::trim)
            .filter(|file| !file.is_empty())
            .map(|file| normalize_path_text(Path::new(file)))
            .collect::<Vec<_>>();
        anchor_files.sort();
        anchor_files.dedup();
        let anchored_lines = records
            .iter()
            .filter_map(|record| record.anchor.as_ref())
            .filter(|anchor| {
                anchor
                    .file
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|file| !file.is_empty())
                    && anchor.line.is_some()
            })
            .count();

        Self {
            scope: "gap_ledger_artifact",
            run_status: "artifact_scope",
            basis: "supplied_gap_decision_ledger",
            changed_files: anchor_files.clone(),
            changed_lines: anchored_lines,
            changed_owner_functions: 0,
            changed_production_files: anchor_files.clone(),
            immediate_caller_files: Vec::new(),
            scoped_production_files: anchor_files.clone(),
            total_rust_files: None,
            total_production_files: None,
            production_files_considered: anchor_files.len(),
            classified_seams_considered: records.len(),
            unevaluated_seams: 0,
            absent_changed_files: Vec::new(),
            downstream_consumable: true,
            limitation: "review_comments_gap_ledger_artifact_scope_only",
            repair_route: "reports/gap-decision-ledger",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn producer_projection_preserves_changed_file_roles_and_complete_denominator() {
        let working_set = AgentBriefResolvedWorkingSet::files(
            [
                "src/pricing.rs",
                "src/pricing_tests.rs",
                "tests/pricing.rs",
                "xtask/src/main.rs",
                "xtask/tests/control.rs",
                "xtask/fixtures/src/sample.rs",
                "fixtures/xtask/src/sample.rs",
                "build.rs",
            ]
            .into_iter()
            .map(PathBuf::from)
            .collect(),
        );
        // The complete producer's reviewed denominator is independent of
        // the projected file count and any later comment-rendering cap.
        let scope = ReviewCommentsAnalysisScope::producer_projection(&working_set, 23);
        assert_eq!(
            scope.changed_production_files,
            [
                "src/pricing.rs",
                "src/pricing_tests.rs",
                "xtask/src/main.rs"
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
        );
        assert_eq!(
            scope.scoped_production_files,
            scope.changed_production_files
        );
        assert_eq!(scope.production_files_considered, 3);
        assert_eq!(scope.classified_seams_considered, 23);
        assert_eq!(scope.total_rust_files, None);
        assert_eq!(scope.total_production_files, None);
        assert_eq!(scope.unevaluated_seams, 0);
        assert!(scope.absent_changed_files.is_empty());
        assert_eq!(scope.limitation, "review_comments_producer_projection_only");
    }
}
