mod classify;
mod config;
mod diff;
mod repo;

pub use diff::probes_for_file;
pub use repo::probes_for_repo_file;

use std::path::Path;

fn sanitize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").replace('/', "_")
}

#[cfg(test)]
mod tests {
    use super::sanitize_path;
    use std::path::PathBuf;

    #[test]
    fn sanitize_path_converts_separators() {
        let path = PathBuf::from("src/lib.rs");
        let sanitized = sanitize_path(&path);
        assert_eq!(sanitized, "src_lib.rs");
    }
}
