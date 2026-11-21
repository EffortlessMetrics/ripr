//! Package-root and path normalization helpers.
//!
//! The pre-#3285 production-path predicate (`is_production_rust_path`)
//! is retired: every consumer — diff seeding, repo production sets, the
//! seam inventory, and the LSP scope partition — routes through the
//! producer-owned source-role model in `source_role.rs`, which carries
//! the same exclusions forward plus declared-target and opt-in context
//! (#3283/#3285). The production-contract carry-over pin lives in
//! `source_role.rs::tests::repo_production_contract_carries_over_exactly`.

use std::path::Path;

pub(crate) fn package_root(path: &Path) -> Option<String> {
    let normalized = normalize_path(path);
    if normalized.starts_with("src/")
        || normalized.starts_with("tests/")
        || normalized.starts_with("examples/")
        || normalized.starts_with("benches/")
    {
        return Some(String::new());
    }
    if let Some(rest) = normalized.strip_prefix("crates/")
        && let Some((crate_name, crate_relative)) = rest.split_once('/')
        && (crate_relative.starts_with("src/") || crate_relative.starts_with("tests/"))
    {
        return Some(format!("crates/{crate_name}/"));
    }
    for marker in ["/src/", "/tests/", "/examples/", "/benches/"] {
        if let Some(idx) = normalized.rfind(marker) {
            let prefix = &normalized[..idx];
            if !prefix.is_empty() {
                return Some(format!("{prefix}/"));
            }
        }
    }
    None
}

pub(crate) fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}
