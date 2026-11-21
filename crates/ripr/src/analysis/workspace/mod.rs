mod cargo_targets;
mod classify;
mod discover;
mod path_dependencies;
mod select;
mod source_role;

pub(crate) use cargo_targets::{
    CargoHarnessVerdict, ManifestInventory, context_for_files,
    declared_crate_root_paths_from_manifest,
};
pub(crate) use path_dependencies::{
    PathDependencyAdjacency, PathDependencyGraphStatus, reverse_dependent_scope_expansion,
};
pub(crate) use source_role::{
    SourceRoleContext, classify_with, is_repo_automation_path, is_test_surface_path,
};

pub(crate) use classify::{normalize_path, package_root};
pub(crate) use discover::discover_preview_language_files;
pub use discover::discover_rust_files;
pub(crate) use select::select_rust_files_for_mode_with_dependent_packages;
