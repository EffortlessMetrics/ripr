mod classify;
mod diff;
mod expectations;
mod family;
mod repo;

pub use diff::probes_for_file;
pub use repo::probes_for_repo_file;

use std::path::Path;

fn sanitize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace(['/', '\\', ':'], "_")
        .trim_matches('_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::sanitize_path;
    use std::path::PathBuf;

    #[test]
    fn sanitize_path_converts_separators_and_colons() {
        let path = PathBuf::from("src/lib.rs");
        let sanitized = sanitize_path(&path);
        assert_eq!(sanitized, "src_lib.rs");
    }

    #[test]
    fn sanitize_path_handles_windows_paths() {
        let path = PathBuf::from("workspace\\src\\lib.rs");
        let sanitized = sanitize_path(&path);
        assert_eq!(sanitized, "workspace_src_lib.rs");
    }

    #[test]
    fn sanitize_path_trims_underscores() {
        let path = PathBuf::from(":src/lib:");
        let sanitized = sanitize_path(&path);
        assert_eq!(sanitized, "src_lib");
    }
}
