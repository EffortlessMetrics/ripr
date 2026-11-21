#[cfg(test)]
mod limitations_tests;
mod load;
mod model;
mod parse;
mod path;

pub use load::{
    load_diff, load_diff_range, load_worktree_diff, resolve_base_commit,
    resolve_default_base_commit, working_tree_has_tracked_changes,
};
#[allow(
    unused_imports,
    reason = "ChangedLine is re-exported for use by probes.rs and other external modules; not used within diff module itself."
)]
pub use model::{ChangedFile, ChangedLine};
pub use parse::parse_unified_diff;
pub(crate) use parse::parse_unified_diff_bounded_with_metadata;
