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

/// Publish `bytes` at `path` only when nothing is there yet. The bytes are
/// written and fsynced in a same-directory temporary file first, so a failed
/// write never leaves a partial file at `path`, and the final hard link fails
/// with `AlreadyExists` rather than replace an entry that appeared meanwhile.
pub(crate) fn create_new(path: &Path, bytes: &[u8]) -> Result<(), CreateNewError> {
    publish_with(path, true, Publish::NoReplace, |file| file.write_all(bytes)).map_err(|failure| {
        match failure.stage {
            Stage::Finalize => CreateNewError::Link(failure.error),
            _ => CreateNewError::Staging(failure.error),
        }
    })
}

/// Why [`create_new`] published nothing.
#[derive(Debug)]
pub(crate) enum CreateNewError {
    /// Preparing the complete temporary file failed (for example a full
    /// disk); nothing was written at the destination.
    Staging(std::io::Error),
    /// The hard link into place failed: the destination exists
    /// (`AlreadyExists`) or the filesystem has no hard links.
    Link(std::io::Error),
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
    publish_with(path, sync_before_publish, Publish::Replace, fill)
}

/// How a complete temporary file reaches its destination.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Publish {
    /// Rename over whatever entry is at the destination.
    Replace,
    /// Hard-link into place, failing with `AlreadyExists` if any entry
    /// (including a dangling symlink) appeared at the destination.
    NoReplace,
}

fn publish_with(
    path: &Path,
    sync_before_publish: bool,
    publish: Publish,
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
        //
        // `symlink_metadata` does not follow a symlink at the destination:
        // the rename replaces the link itself, so the new file must not take
        // on the permissions of whatever the link pointed at.
        if publish == Publish::Replace
            && let Ok(metadata) = std::fs::symlink_metadata(path)
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
        match publish {
            Publish::Replace => {
                std::fs::rename(&tmp_path, path).map_err(|err| fail(Stage::Finalize, path, err))
            }
            Publish::NoReplace => {
                std::fs::hard_link(&tmp_path, path)
                    .map_err(|err| fail(Stage::Finalize, path, err))?;
                let _ = std::fs::remove_file(&tmp_path);
                Ok(())
            }
        }
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{CreateNewError, TEMP_FILE_SEQUENCE, create_new, write, write_cache};
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

    fn temp_leftovers(dir: &Path) -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(|err| format!("read_dir: {err}"))? {
            let name = entry
                .map_err(|err| format!("dir entry: {err}"))?
                .file_name()
                .to_string_lossy()
                .into_owned();
            if name.ends_with(".tmp") {
                names.push(name);
            }
        }
        Ok(names)
    }

    #[test]
    fn create_new_publishes_once_and_never_replaces() -> Result<(), String> {
        let dir = isolated_dir("create-new");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|err| format!("setup: {err}"))?;
        let path = dir.join("ripr.toml");
        create_new(&path, b"first\n").map_err(|err| format!("first create: {err:?}"))?;
        let second = create_new(&path, b"second\n");
        let contents = std::fs::read(&path).map_err(|err| format!("read: {err}"));
        let leftovers = temp_leftovers(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        match second {
            Err(CreateNewError::Link(err)) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            other => return Err(format!("an existing file must refuse the link: {other:?}")),
        }
        assert_eq!(contents?, b"first\n");
        assert_eq!(leftovers?, Vec::<String>::new());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn create_new_refuses_a_dangling_symlink() -> Result<(), String> {
        let dir = isolated_dir("create-new-dangling");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|err| format!("setup: {err}"))?;
        let target = dir.join("missing-target");
        let link = dir.join("ripr.toml");
        std::os::unix::fs::symlink(&target, &link).map_err(|err| format!("symlink: {err}"))?;
        let outcome = create_new(&link, b"body\n");
        let target_created = target.exists();
        let _ = std::fs::remove_dir_all(&dir);
        match outcome {
            Err(CreateNewError::Link(err)) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            other => {
                return Err(format!(
                    "a dangling symlink must refuse the link: {other:?}"
                ));
            }
        }
        assert!(
            !target_created,
            "create_new wrote through a dangling symlink"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn replacing_a_symlink_does_not_copy_its_target_permissions() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = isolated_dir("replace-symlink-mode");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|err| format!("setup: {err}"))?;
        let target = dir.join("target");
        std::fs::write(&target, b"keep").map_err(|err| format!("seed: {err}"))?;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600))
            .map_err(|err| format!("chmod: {err}"))?;
        let link = dir.join("report.json");
        std::os::unix::fs::symlink(&target, &link).map_err(|err| format!("symlink: {err}"))?;
        let probe = dir.join("probe");
        std::fs::write(&probe, b"").map_err(|err| format!("probe: {err}"))?;
        let default_mode = std::fs::metadata(&probe)
            .map_err(|err| format!("stat probe: {err}"))?
            .permissions()
            .mode()
            & 0o777;
        let outcome = write(&link, b"new", "test");
        let mode = std::fs::symlink_metadata(&link)
            .map(|metadata| (metadata.is_file(), metadata.permissions().mode() & 0o777));
        let kept = std::fs::read(&target);
        let _ = std::fs::remove_dir_all(&dir);
        outcome?;
        let (is_file, mode) = mode.map_err(|err| format!("stat: {err}"))?;
        assert!(is_file, "the link must be replaced by a regular file");
        assert_eq!(
            mode, default_mode,
            "the replacement took the link target's mode"
        );
        assert_eq!(kept.map_err(|err| format!("read target: {err}"))?, b"keep");
        Ok(())
    }
}
