use super::super::syntax::{LexicalRustSyntaxAdapter, RaRustSyntaxAdapter, RustSyntaxAdapter};
use super::model::RustIndex;
use std::path::{Path, PathBuf};

pub fn build_index(root: &Path, files: &[PathBuf]) -> Result<RustIndex, String> {
    build_index_with_adapters(root, files, &RaRustSyntaxAdapter, &LexicalRustSyntaxAdapter)
}

fn build_index_with_adapters(
    root: &Path,
    files: &[PathBuf],
    adapter: &dyn RustSyntaxAdapter,
    fallback: &dyn RustSyntaxAdapter,
) -> Result<RustIndex, String> {
    let mut index = RustIndex::default();
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
    use crate::analysis::syntax::{SyntaxNodeFact, TextRange};
    use std::error::Error;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> Result<PathBuf, Box<dyn Error>> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-{name}-{stamp}"));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    fn write_manifest(root: &Path) -> Result<(), Box<dyn Error>> {
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='test'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        Ok(())
    }

    #[test]
    fn build_index_collects_functions_and_tests_from_workspace_files() -> Result<(), Box<dyn Error>>
    {
        let root = temp_dir("index_functions")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
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
        )?;

        let index = build_index(&root, &[PathBuf::from("src/lib.rs")])?;
        assert!(!index.functions.is_empty());
        assert!(!index.tests.is_empty());
        assert!(index.files.contains_key(&PathBuf::from("src/lib.rs")));
        Ok(())
    }

    #[test]
    fn build_index_collects_calls_returns_literals() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_facts")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
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
        )?;

        let index = build_index(&root, &[PathBuf::from("src/lib.rs")])?;
        let file_facts = index.files.get(&PathBuf::from("src/lib.rs"));
        assert!(file_facts.is_some());
        assert!(file_facts.is_some_and(|facts| !facts.calls.is_empty()));
        assert!(
            index
                .files
                .get(&PathBuf::from("src/lib.rs"))
                .is_some_and(|facts| !facts.returns.is_empty())
        );
        Ok(())
    }

    #[test]
    fn build_index_collects_parser_probe_shapes_for_valid_source() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_probes")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
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
        )?;

        let index = build_index(&root, &[PathBuf::from("src/lib.rs")])?;
        assert!(
            index
                .files
                .get(&PathBuf::from("src/lib.rs"))
                .is_some_and(|facts| !facts.probe_shapes.is_empty())
        );
        Ok(())
    }

    #[test]
    fn build_index_returns_read_error_for_missing_file() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_missing")?;
        fs::create_dir_all(root.join("src"))?;

        let result = build_index(&root, &[PathBuf::from("src/nonexistent.rs")]);
        assert!(matches!(result, Err(ref err) if err.contains("failed to read")));
        Ok(())
    }

    #[derive(Clone, Debug, Default)]
    struct FailingSyntaxAdapter;

    impl RustSyntaxAdapter for FailingSyntaxAdapter {
        fn summarize_file(
            &self,
            _path: &Path,
            _text: &str,
        ) -> Result<super::super::FileFacts, String> {
            Err("synthetic parser failure".to_string())
        }

        fn changed_nodes(
            &self,
            _facts: &super::super::FileFacts,
            _ranges: &[TextRange],
        ) -> Vec<SyntaxNodeFact> {
            Vec::new()
        }
    }

    #[derive(Clone, Debug, Default)]
    struct StubSyntaxAdapter;

    impl RustSyntaxAdapter for StubSyntaxAdapter {
        fn summarize_file(
            &self,
            path: &Path,
            text: &str,
        ) -> Result<super::super::FileFacts, String> {
            Ok(super::super::FileFacts {
                path: path.to_path_buf(),
                source: text.to_string(),
                ..super::super::FileFacts::default()
            })
        }

        fn changed_nodes(
            &self,
            _facts: &super::super::FileFacts,
            _ranges: &[TextRange],
        ) -> Vec<SyntaxNodeFact> {
            Vec::new()
        }
    }

    #[test]
    fn build_index_falls_back_when_primary_adapter_errors() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("index_fallback")?;
        fs::create_dir_all(root.join("src"))?;
        fs::write(root.join("src/lib.rs"), "pub fn fallback() {}\n")?;

        let index = build_index_with_adapters(
            &root,
            &[PathBuf::from("src/lib.rs")],
            &FailingSyntaxAdapter,
            &StubSyntaxAdapter,
        )?;
        assert_eq!(
            index
                .files
                .get(&PathBuf::from("src/lib.rs"))
                .map_or("", |facts| facts.source.as_str()),
            "pub fn fallback() {}\n"
        );
        assert!(
            FailingSyntaxAdapter
                .changed_nodes(&super::super::FileFacts::default(), &[])
                .is_empty()
        );
        assert!(
            StubSyntaxAdapter
                .changed_nodes(&super::super::FileFacts::default(), &[])
                .is_empty()
        );
        Ok(())
    }
}
