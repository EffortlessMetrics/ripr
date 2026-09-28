#[cfg(test)]
mod limitations_tests;
mod load;
mod model;
mod parse;
mod path;
pub mod records;

pub use load::{
    committed_diff_files_with_uncommitted_edits, load_diff, load_diff_range,
    load_diff_with_effective_base, load_pr_evidence_diff_range, load_worktree_diff,
    load_worktree_diff_with_effective_base, resolve_base_commit, resolve_default_base_commit,
    resolve_effective_base, working_tree_has_tracked_changes,
};
#[allow(
    unused_imports,
    reason = "ChangedLine is re-exported for use by probes.rs and other external modules; not used within diff module itself."
)]
pub use model::{ChangedFile, ChangedLine};
pub use parse::parse_unified_diff;
pub(crate) use parse::parse_unified_diff_bounded_with_metadata;
