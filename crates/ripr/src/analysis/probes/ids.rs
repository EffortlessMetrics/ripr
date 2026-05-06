use crate::domain::{ProbeFamily, ProbeId};
use std::path::Path;

pub fn diff_probe_id(path: &Path, line: usize, family: &ProbeFamily) -> ProbeId {
    ProbeId(format!(
        "probe:{}:{}:{}",
        sanitize_path(path),
        line,
        family.as_str()
    ))
}

pub fn repo_probe_id(path: &Path, line: usize, family: &ProbeFamily) -> ProbeId {
    ProbeId(format!(
        "repo-probe:{}:{}:{}",
        sanitize_path(path),
        line,
        family.as_str()
    ))
}

fn sanitize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace(['/', '\\', ':'], "_")
        .trim_matches('_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ProbeFamily;
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

    #[test]
    fn diff_probe_id_preserves_legacy_format() {
        let id = diff_probe_id(&PathBuf::from("src/lib.rs"), 3, &ProbeFamily::Predicate);

        assert_eq!(id.0, "probe:src_lib.rs:3:predicate");
    }

    #[test]
    fn repo_probe_id_preserves_legacy_format() {
        let id = repo_probe_id(&PathBuf::from("src/lib.rs"), 7, &ProbeFamily::ErrorPath);

        assert_eq!(id.0, "repo-probe:src_lib.rs:7:error_path");
    }
}
