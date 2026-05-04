use super::super::syntax::{LexicalRustSyntaxAdapter, RaRustSyntaxAdapter, RustSyntaxAdapter};
use super::model::RustIndex;
use std::path::{Path, PathBuf};

pub fn build_index(root: &Path, files: &[PathBuf]) -> Result<RustIndex, String> {
    let mut index = RustIndex::default();
    let adapter = RaRustSyntaxAdapter;
    let fallback = LexicalRustSyntaxAdapter;
    for file in files {
        let full = root.join(file);
        let text = std::fs::read_to_string(&full)
            .map_err(|err| format!("failed to read {}: {err}", full.display()))?;
        let summary = adapter
            .summarize_file(file, &text)
            .or_else(|_| fallback.summarize_file(file, &text))?;
        index.tests.extend(summary.tests.clone());
        index.functions.extend(summary.functions.clone());
        index.files.insert(file.clone(), summary);
    }
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-{name}-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn build_index_collects_functions_and_tests_from_workspace_files() {
        let root = temp_dir("index_functions");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='test'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[test]
fn test_add() {
    assert_eq!(add(1, 2), 3);
}
"#,
        )
        .unwrap();

        let index = build_index(&root, &[PathBuf::from("src/lib.rs")]).unwrap();
        assert!(!index.functions.is_empty());
        assert!(!index.tests.is_empty());
        assert!(index.files.contains_key(&PathBuf::from("src/lib.rs")));
    }

    #[test]
    fn build_index_collects_calls_returns_literals() {
        let root = temp_dir("index_facts");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='test'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn process() -> Result<i32, String> {
    let value = some_fn();
    Ok(42)
}

fn some_fn() -> i32 {
    100
}
"#,
        )
        .unwrap();

        let index = build_index(&root, &[PathBuf::from("src/lib.rs")]).unwrap();
        let file_facts = index.files.get(&PathBuf::from("src/lib.rs")).unwrap();
        assert!(!file_facts.calls.is_empty());
        assert!(!file_facts.returns.is_empty());
    }

    #[test]
    fn build_index_collects_parser_probe_shapes_for_valid_source() {
        let root = temp_dir("index_probes");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='test'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn check(x: i32) -> bool {
    if x > 0 {
        true
    } else {
        false
    }
}
"#,
        )
        .unwrap();

        let index = build_index(&root, &[PathBuf::from("src/lib.rs")]).unwrap();
        let file_facts = index.files.get(&PathBuf::from("src/lib.rs")).unwrap();
        assert!(!file_facts.probe_shapes.is_empty());
    }

    #[test]
    fn build_index_returns_read_error_for_missing_file() {
        let root = temp_dir("index_missing");
        fs::create_dir_all(root.join("src")).unwrap();

        let result = build_index(&root, &[PathBuf::from("src/nonexistent.rs")]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to read"));
    }
}
