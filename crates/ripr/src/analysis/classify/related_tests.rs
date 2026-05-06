use super::super::rust_index::{
    FunctionSummary, RustIndex, TestSummary, extract_identifier_tokens,
};
use crate::domain::Probe;
use std::path::Path;

pub(in crate::analysis) fn find_related_tests<'a>(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    index: &'a RustIndex,
) -> Vec<&'a TestSummary> {
    let mut related = Vec::new();
    let owner_name = owner_fn.map(|f| f.name.as_str()).unwrap_or("");
    let probe_tokens = extract_identifier_tokens(&probe.expression);
    let file_name = probe
        .location
        .file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let package_prefix = owner_fn.and_then(|owner| package_prefix(&owner.file));

    for test in &index.tests {
        if let Some(prefix) = &package_prefix
            && !normalize_path(&test.file).starts_with(prefix)
        {
            continue;
        }
        let calls_owner = !owner_name.is_empty()
            && (test.calls.iter().any(|call| call.name == owner_name)
                || test.body.contains(owner_name));
        let test_name = test.name.to_ascii_lowercase();
        let owner_name = owner_name.to_ascii_lowercase();
        let same_file_or_named = normalize_path(&test.file).contains(file_name)
            || (!owner_name.is_empty() && test_name.contains(&owner_name))
            || probe_tokens
                .iter()
                .any(|token| token.len() > 2 && test_name.contains(&token.to_ascii_lowercase()));

        if calls_owner || same_file_or_named {
            related.push(test);
        }
    }

    related.sort_by(|a, b| a.name.cmp(&b.name));
    related.dedup_by(|a, b| a.name == b.name && a.file == b.file);
    related
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

fn package_prefix(path: &Path) -> Option<String> {
    let normalized = normalize_path(path);
    if let Some(rest) = normalized.strip_prefix("crates/")
        && let Some((crate_name, crate_relative)) = rest.split_once('/')
        && (crate_relative.starts_with("src/") || crate_relative.starts_with("tests/"))
    {
        return Some(format!("crates/{crate_name}/"));
    }
    for marker in ["/src/", "/tests/"] {
        if let Some(idx) = normalized.rfind(marker) {
            let prefix = &normalized[..idx];
            if prefix.is_empty() {
                return None;
            }
            return Some(format!("{prefix}/"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_workspace_paths_when_extracting_package_prefix_then_handles_nested_markers() {
        assert_eq!(
            package_prefix(Path::new("crates/foo/src/support/src/lib.rs")).as_deref(),
            Some("crates/foo/")
        );
        assert_eq!(
            package_prefix(Path::new("crates/foo/tests/support/tests/cases.rs")).as_deref(),
            Some("crates/foo/")
        );
        assert_eq!(
            package_prefix(Path::new("vendor/foo/src/support/src/lib.rs")).as_deref(),
            Some("vendor/foo/src/support/")
        );
        assert_eq!(
            package_prefix(Path::new("crates/ripr/examples/sample/src/lib.rs")).as_deref(),
            Some("crates/ripr/examples/sample/")
        );
    }

    #[test]
    fn given_non_workspace_paths_when_extracting_package_prefix_then_returns_none() {
        assert_eq!(package_prefix(Path::new("src/lib.rs")), None);
        assert_eq!(package_prefix(Path::new("tests/basic.rs")), None);
        assert_eq!(package_prefix(Path::new("README.md")), None);
    }

    #[test]
    fn given_mixed_separator_path_when_normalizing_then_uses_workspace_relative_form() {
        let normalized = normalize_path(Path::new("./crates\\ripr\\src\\lib.rs"));
        assert_eq!(normalized, "crates/ripr/src/lib.rs");
    }
}
