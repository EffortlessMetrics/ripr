use crate::analysis::language::{LanguageAdapter, RustAdapter};
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
