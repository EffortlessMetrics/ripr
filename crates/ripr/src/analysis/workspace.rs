use std::path::{Path, PathBuf};

pub fn discover_rust_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    visit(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

fn visit(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|err| format!("failed to read {}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("failed to read dir entry: {err}"))?;
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if matches!(
                name,
                ".git" | "target" | ".ripr" | ".direnv" | "node_modules"
            ) {
                continue;
            }
            visit(root, &path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push(relative);
        }
    }
    Ok(())
}
