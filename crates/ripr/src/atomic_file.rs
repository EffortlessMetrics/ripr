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
    replace_with(path, sync_before_publish, |file| file.write_all(bytes)).map_err(|failure| {
        let subject = error_path_policy.suffix(&failure.subject);
        let err = failure.error;
        match failure.stage {
            Stage::CreateDirectory => format!("failed to create {label} directory{subject}: {err}"),
            Stage::NoFileName => format!("atomic write path{subject} has no file name"),
            Stage::CreateTemp => format!("failed to create {label} temp file{subject}: {err}"),
            Stage::Fill => format!("failed to write {label} temp file{subject}: {err}"),
            Stage::Permissions => {
                format!("failed to preserve {label} permissions for{subject}: {err}")
            }
            Stage::Sync => format!("failed to fsync {label} temp file{subject}: {err}"),
            Stage::Finalize => format!("failed to finalize {label}{subject}: {err}"),
        }
    })
}

/// Stream `fill` into a same-directory temporary file, then atomically
/// replace `path` with it, so a reader sees the old file or the complete new
/// one and an interrupted or failed write leaves the old file in place. The
/// temporary file is created exclusively under a short name that does not
/// grow with the destination's name, and is removed on failure.
pub(crate) fn replace_streamed(
    path: &Path,
    fill: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    replace_with(path, true, fill).map_err(|failure| failure.error)
}

#[derive(Clone, Copy)]
enum Stage {
    CreateDirectory,
    NoFileName,
    CreateTemp,
    Fill,
    Permissions,
    Sync,
    Finalize,
}

struct ReplaceFailure {
    stage: Stage,
    subject: std::path::PathBuf,
    error: std::io::Error,
}

fn replace_with(
    path: &Path,
    sync_before_publish: bool,
    fill: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<(), ReplaceFailure> {
    let fail = |stage, subject: &Path, error| ReplaceFailure {
        stage,
        subject: subject.to_path_buf(),
        error,
    };
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let dir = parent.unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir).map_err(|err| fail(Stage::CreateDirectory, dir, err))?;
    if path.file_name().is_none() {
        let err = std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no file name");
        return Err(fail(Stage::NoFileName, path, err));
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
    let result = (|| {
        // `create_new` never opens an existing path, so a file or link
        // planted at the temporary name cannot redirect the write.
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .map_err(|err| fail(Stage::CreateTemp, &tmp_path, err))?;
        fill(&mut file).map_err(|err| fail(Stage::Fill, &tmp_path, err))?;
        // Only an existing *file* has permissions to carry over. A directory
        // at the destination makes the rename below fail with the finalize
        // error; copying its attributes first would fail earlier on Windows,
        // where they include FILE_ATTRIBUTE_DIRECTORY (os error 87), and
        // misreport the cause as a permission problem.
        if let Ok(metadata) = std::fs::metadata(path)
            && metadata.is_file()
        {
            file.set_permissions(metadata.permissions())
                .map_err(|err| fail(Stage::Permissions, path, err))?;
        }
        if sync_before_publish {
            file.sync_all()
                .map_err(|err| fail(Stage::Sync, &tmp_path, err))?;
        }
        drop(file);
        std::fs::rename(&tmp_path, path).map_err(|err| fail(Stage::Finalize, path, err))
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
