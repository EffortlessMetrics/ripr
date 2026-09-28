use std::path::Path;

pub(crate) fn display_path(path: &Path) -> String {
    normalize_report_path(&path.display().to_string())
}

pub(super) fn normalize_report_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    match normalized.strip_prefix("./") {
        Some(stripped) => stripped.to_string(),
        None => normalized,
    }
}
