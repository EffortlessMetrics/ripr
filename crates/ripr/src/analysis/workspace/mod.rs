mod cargo_targets;
mod classify;
mod discover;
mod module_graph;
mod path_dependencies;
mod select;
mod source_role;

pub(crate) use cargo_targets::{
    CargoHarnessVerdict, ManifestInventory, context_for_files,
    declared_crate_root_paths_from_manifest,
};
pub(crate) use module_graph::apply_module_graph_evidence;
pub(crate) use path_dependencies::{
    PathDependencyAdjacency, PathDependencyGraphStatus, reverse_dependent_scope_expansion,
};
pub(crate) use source_role::{
    SourceRole, SourceRoleContext, classify_with, is_test_surface_path, seeds_diff_probes,
};

pub(crate) use classify::{normalize_path, package_root};
pub use discover::discover_rust_files;
pub(crate) use discover::{
    changed_source_files_absent_from_worktree, discover_preview_language_files,
    discover_unanalyzed_source_files, limitations_for_absent_changed_files,
};
pub(crate) use select::select_rust_files_for_mode_with_dependent_packages;
