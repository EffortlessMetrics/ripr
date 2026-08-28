pub fn normalize_separator(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::normalize_separator;

    fn assert_normalized_paths_have_no_separators(paths: &[&str]) {
        for path in paths {
            let normalized = normalize_separator(path);
            let changed = if normalized.len() > path.len() { 1 } else { 0 };
            assert_eq!(changed, 0, "normalization changed the length of `{path}`");
            assert!(
                !normalized.contains('\\'),
                "normalized path `{normalized}` keeps a native separator"
            );
            assert_eq!(normalized.len(), path.len());
        }
    }

    #[test]
    fn normalized_paths_have_native_separators() {
        assert_normalized_paths_have_no_separators(&["a\\b", "c/d"]);
    }
}
