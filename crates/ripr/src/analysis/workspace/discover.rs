use crate::analysis::language::{LanguageAdapter, LanguageId, RustAdapter, route};
use std::path::{Path, PathBuf};

const DEFAULT_IGNORED_DIRS: &[&str] = &[
    ".git",
    "target",
    ".ripr",
    ".direnv",
    "fixtures",
    "node_modules",
];

pub fn discover_rust_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let adapter = RustAdapter;
    visit(root, root, &adapter, &mut out)?;
    out.sort();
    Ok(out)
}

/// Discover production files in the workspace that route to a preview-language
/// adapter (TypeScript/JavaScript or Python), by path extension only.
///
/// Routing is `analysis::language::route`, the same predicate adapter dispatch
/// uses. This does not require the adapter to be enabled; it is used so the
/// repo pipeline can disclose preview-language files in scope even when the
/// adapter is not enabled (RIPR-SPEC-0082, #1111). Test files and excluded
/// directories are skipped the same way as Rust discovery.
pub(crate) fn discover_preview_language_files(root: &Path) -> Vec<(LanguageId, PathBuf)> {
    let mut out: Vec<(LanguageId, PathBuf)> = Vec::new();
    visit_preview(root, root, &mut out);
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

fn visit_preview(root: &Path, dir: &Path, out: &mut Vec<(LanguageId, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if DEFAULT_IGNORED_DIRS.contains(&name) {
                continue;
            }
            visit_preview(root, &path, out);
        } else if let Some(language) = route(&path)
            && matches!(
                language,
                LanguageId::TypeScript | LanguageId::JavaScript | LanguageId::Python
            )
        {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push((language, relative));
        }
    }
}

fn visit(
    root: &Path,
    dir: &Path,
    adapter: &RustAdapter,
    out: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|err| format!("failed to read {}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("failed to read dir entry: {err}"))?;
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if DEFAULT_IGNORED_DIRS.contains(&name) {
                continue;
            }
            visit(root, &path, adapter, out)?;
        } else if adapter.accepts_path(&path) {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push(relative);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn discover_rust_files_is_callable() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-discover-test-{:?}",
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        fs::create_dir(dir.join("src"))?;
        fs::write(dir.join("src/lib.rs"), "")?;

        let result = discover_rust_files(&dir)?;
        assert!(result.iter().any(|p| p.ends_with("src/lib.rs")));

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn discover_skips_default_excluded_directories() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-discover-default-exclusions-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src"))?;
        fs::write(dir.join("src/lib.rs"), "")?;

        for ignored in DEFAULT_IGNORED_DIRS {
            let ignored_src = dir.join(ignored).join("src");
            fs::create_dir_all(&ignored_src)?;
            fs::write(ignored_src.join("lib.rs"), "")?;
        }

        let result = discover_rust_files(&dir)?;
        assert_eq!(result, vec![PathBuf::from("src/lib.rs")]);

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
