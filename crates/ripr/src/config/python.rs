//! Python project auto-detection for configuration defaults.

use std::path::Path;

pub(super) const PYTHON_PROJECT_MARKERS: &[&str] = &[
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "requirements.txt",
    "pytest.ini",
    "tox.ini",
    "noxfile.py",
];
pub(super) const PYTHON_SOURCE_DIR_MARKERS: &[&str] = &["src", "tests"];
pub(super) const PYTHON_PROJECT_EXCLUDED_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    ".ripr",
    ".direnv",
    "__pycache__",
    ".venv",
    "venv",
    "env",
    ".tox",
    ".nox",
    "site-packages",
    ".pytest_cache",
    ".mypy_cache",
    "dist",
    "build",
];

pub(crate) fn detect_python_project(root: &Path) -> bool {
    PYTHON_PROJECT_MARKERS
        .iter()
        .any(|marker| root.join(marker).is_file())
        || PYTHON_SOURCE_DIR_MARKERS
            .iter()
            .any(|marker| dir_contains_python_source(&root.join(marker)))
}

fn dir_contains_python_source(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if is_python_project_excluded_dir(name) {
                continue;
            }
            if dir_contains_python_source(&path) {
                return true;
            }
        } else if file_type.is_file()
            && is_python_source_file(&path)
            && !is_detectable_generated_python_file(&path)
        {
            return true;
        }
    }
    false
}

fn is_python_project_excluded_dir(name: &str) -> bool {
    PYTHON_PROJECT_EXCLUDED_DIRS.contains(&name)
}

fn is_python_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "py")
}

fn is_detectable_generated_python_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name.ends_with("_pb2.py")
        || name.ends_with("_pb2_grpc.py")
        || name.ends_with(".generated.py")
        || name.ends_with("_generated.py")
        || name.starts_with("generated_")
}
