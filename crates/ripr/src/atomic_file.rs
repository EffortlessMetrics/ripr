//! Atomic replacement for small on-disk artifacts and cache entries.

use std::io::Write;
use std::path::Path;

static TEMP_FILE_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Clone, Copy)]
enum ErrorPathPolicy {
    Include,
    Omit,
}

impl ErrorPathPolicy {
    fn suffix(self, path: &Path) -> String {
        match self {
            Self::Include => format!(" {}", path.display()),
            Self::Omit => String::new(),
        }
    }
}

/// Write bytes through a same-directory temporary file, flush them to disk,
/// then atomically replace the destination. The short temporary name keeps
/// the operation usable when the destination filename is near a platform's
/// maximum filename length.
pub(crate) fn write(path: &Path, bytes: &[u8], label: &str) -> Result<(), String> {
    write_with_sync(path, bytes, label, true, ErrorPathPolicy::Include)
}

/// Atomically replace an expendable cache entry without forcing a physical
/// flush for every entry. Rename still prevents readers from observing a
/// partial file; the cache may be recomputed after a power loss.
///
/// Cache errors deliberately omit directory, destination, and temporary-file
/// spellings so bounded diagnostics remain portable across checkout/cache
/// roots and safe to retain in public receipts.
pub(crate) fn write_cache(path: &Path, bytes: &[u8], label: &str) -> Result<(), String> {
    write_with_sync(path, bytes, label, false, ErrorPathPolicy::Omit)
}

fn write_with_sync(
    path: &Path,
    bytes: &[u8],
    label: &str,
    sync_before_publish: bool,
    error_path_policy: ErrorPathPolicy,
) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let dir = parent.unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir).map_err(|err| {
        format!(
            "failed to create {label} directory{}: {err}",
            error_path_policy.suffix(dir)
        )
    })?;
    if path.file_name().is_none() {
        return Err(format!(
            "atomic write path{} has no file name",
            error_path_policy.suffix(path)
        ));
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp_path = dir.join(format!(
        ".ripr-atomic-{}-{nanos}-{sequence}.tmp",
        std::process::id()
    ));
    let result = (|| -> Result<(), String> {
        let mut file = std::fs::File::create(&tmp_path).map_err(|err| {
            format!(
                "failed to create {label} temp file{}: {err}",
                error_path_policy.suffix(&tmp_path)
            )
        })?;
        file.write_all(bytes).map_err(|err| {
            format!(
                "failed to write {label} temp file{}: {err}",
                error_path_policy.suffix(&tmp_path)
            )
        })?;
        // Only an existing *file* has permissions to carry over. A directory
        // at the destination makes the rename below fail with the finalize
        // error; copying its attributes first would fail earlier on Windows,
        // where they include FILE_ATTRIBUTE_DIRECTORY (os error 87), and
        // misreport the cause as a permission problem.
        if let Ok(metadata) = std::fs::metadata(path)
            && metadata.is_file()
        {
            file.set_permissions(metadata.permissions())
                .map_err(|err| {
                    format!(
                        "failed to preserve {label} permissions for{}: {err}",
                        error_path_policy.suffix(path)
                    )
                })?;
        }
        if sync_before_publish {
            file.sync_all().map_err(|err| {
                format!(
                    "failed to fsync {label} temp file{}: {err}",
                    error_path_policy.suffix(&tmp_path)
                )
            })?;
        }
        drop(file);
        std::fs::rename(&tmp_path, path).map_err(|err| {
            format!(
                "failed to finalize {label}{}: {err}",
                error_path_policy.suffix(path)
            )
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{TEMP_FILE_SEQUENCE, write, write_cache};
    use std::path::{Path, PathBuf};

    fn isolated_dir(label: &str) -> PathBuf {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "ripr-atomic-file-{label}-{}-{sequence}",
            std::process::id()
        ))
    }

    fn directory_failure(root: &Path) -> Result<String, String> {
        let _ = std::fs::remove_dir_all(root);
        std::fs::create_dir_all(root)
            .map_err(|err| format!("fixture directory setup failed: {err}"))?;
        let blocker = root.join("blocker");
        std::fs::write(&blocker, b"not a directory")
            .map_err(|err| format!("fixture blocker setup failed: {err}"))?;
        let destination = blocker.join("entry.json");
        let Err(error) = write_cache(&destination, b"cache", "test cache") else {
            return Err("a cache path below a regular file unexpectedly succeeded".to_string());
        };
        let _ = std::fs::remove_dir_all(root);
        Ok(error)
    }

    #[test]
    fn cache_directory_error_omits_host_paths() -> Result<(), String> {
        let root = isolated_dir("cache-directory-path-sentinel");
        let error = directory_failure(&root)?;
        assert!(
            error.starts_with("failed to create test cache directory:"),
            "unexpected error: {error}"
        );
        assert!(
            !error.contains(root.to_string_lossy().as_ref()),
            "cache error leaked root {}: {error}",
            root.display()
        );
        assert!(
            !error.contains("blocker"),
            "cache error leaked path: {error}"
        );
        assert!(
            !error.contains("entry.json"),
            "cache error leaked path: {error}"
        );
        Ok(())
    }

    #[test]
    fn cache_finalize_error_omits_host_paths() -> Result<(), String> {
        let root = isolated_dir("cache-finalize-path-sentinel");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)
            .map_err(|err| format!("fixture directory setup failed: {err}"))?;
        let destination = root.join("existing-directory");
        std::fs::create_dir(&destination)
            .map_err(|err| format!("fixture destination setup failed: {err}"))?;
        let Err(error) = write_cache(&destination, b"cache", "test cache") else {
            return Err(
                "publishing a cache file over a directory unexpectedly succeeded".to_string(),
            );
        };
        assert!(
            error.starts_with("failed to finalize test cache:"),
            "unexpected error: {error}"
        );
        assert!(
            !error.contains(root.to_string_lossy().as_ref()),
            "cache error leaked root {}: {error}",
            root.display()
        );
        assert!(
            !error.contains("existing-directory"),
            "cache error leaked destination: {error}"
        );
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn durable_write_finalize_error_retains_path_context() -> Result<(), String> {
        let root = isolated_dir("durable-finalize-path-sentinel");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)
            .map_err(|err| format!("fixture directory setup failed: {err}"))?;
        let destination = root.join("existing-directory");
        std::fs::create_dir(&destination)
            .map_err(|err| format!("fixture destination setup failed: {err}"))?;
        let Err(error) = write(&destination, b"artifact", "test artifact") else {
            return Err(
                "publishing an artifact file over a directory unexpectedly succeeded".to_string(),
            );
        };
        assert!(
            error.starts_with("failed to finalize test artifact "),
            "unexpected error: {error}"
        );
        assert!(
            error.contains(destination.to_string_lossy().as_ref()),
            "durable-write error lost destination {}: {error}",
            destination.display()
        );
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn equivalent_roots_produce_equivalent_cache_directory_errors() -> Result<(), String> {
        let first = isolated_dir("equivalent-root-a");
        let second = isolated_dir("equivalent-root-b");
        let first_error = directory_failure(&first)?;
        let second_error = directory_failure(&second)?;
        assert_eq!(first_error, second_error);
        Ok(())
    }
}
