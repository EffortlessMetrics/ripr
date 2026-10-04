#[cfg(test)]
mod limitations_tests;
mod load;
mod model;
mod parse;
mod path;
pub mod records;

pub use load::{
    load_diff, load_diff_range, load_pr_evidence_diff_range, load_worktree_diff,
    resolve_base_commit, resolve_default_base_commit, resolve_effective_base,
    working_tree_has_tracked_changes,
};
pub(crate) use load::{
    load_diff_range_with_deadline_core, load_diff_with_effective_base_core,
    load_worktree_diff_with_effective_base_core, no_merge_base_diagnosis,
};
#[allow(
    unused_imports,
    reason = "ChangedLine is re-exported for use by probes.rs and other external modules; not used within diff module itself."
)]
pub use model::{ChangedFile, ChangedLine};
pub use parse::parse_unified_diff;
pub(crate) use parse::parse_unified_diff_bounded_with_metadata;
