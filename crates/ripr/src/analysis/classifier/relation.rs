use crate::analysis::rust_index::{
    FunctionSummary, RustIndex, TestSummary, extract_identifier_tokens,
};
use crate::domain::Probe;
use std::path::Path;

pub(super) fn find_related_tests<'a>(
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
        let mentions_tokens = probe_tokens
            .iter()
            .any(|token| token.len() > 3 && test.body.contains(token));
        let same_file_or_named = normalize_path(&test.file).contains(file_name)
            || test
                .name
                .to_ascii_lowercase()
                .contains(&owner_name.to_ascii_lowercase())
            || probe_tokens.iter().any(|token| {
                test.name
                    .to_ascii_lowercase()
                    .contains(&token.to_ascii_lowercase())
            });

        if calls_owner || mentions_tokens || same_file_or_named {
            related.push(test);
        }
    }

    related.sort_by(|a, b| a.name.cmp(&b.name));
    related.dedup_by(|a, b| a.name == b.name && a.file == b.file);
    related
}

pub(super) fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

pub(super) fn package_prefix(path: &Path) -> Option<String> {
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
