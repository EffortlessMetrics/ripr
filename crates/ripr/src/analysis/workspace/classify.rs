use std::path::Path;

pub fn is_production_rust_path(path: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) != Some("rs") {
        return false;
    }

    let normalized = normalize_path(path);
    let components = normalized.split('/').collect::<Vec<_>>();

    if !components.iter().any(|c| c == &"src") {
        return false;
    }

    let exclude_components = [
        "tests",
        "examples",
        "benches",
        "target",
        "fixtures",
        "editors",
        "node_modules",
        "xtask",
    ];
    if components.iter().any(|c| exclude_components.contains(c)) {
        return false;
    }

    let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if file_stem == "tests" {
        return false;
    }

    true
}

pub fn package_root(path: &Path) -> Option<String> {
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

pub fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn classify_functions_are_callable() {
        let path = PathBuf::from("src/lib.rs");
        assert!(is_production_rust_path(&path));
        assert_eq!(package_root(&path), Some(String::new()));
        let normalized = normalize_path(&path);
        assert_eq!(normalized, "src/lib.rs");
    }
}
