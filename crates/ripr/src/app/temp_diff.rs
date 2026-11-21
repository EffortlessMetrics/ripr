//! Hardened temporary diff-file creation for the `diff` command (#2102).
//!
//! The diff text is staged in a per-invocation private directory directly
//! under the OS temp dir so a crafted local environment cannot win a
//! symlink race against a predictable path, and so no fixed shared name
//! lets one local account deny the feature to another.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Write `diff_text` to a fresh private temporary file and return its path.
///
/// The caller owns cleanup: remove the file and its parent directory when
/// the analysis completes.
pub(crate) fn write_temporary_diff_file(diff_text: &str) -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    // Per-invocation private directory directly under the OS temp dir
    // (#2102). A fixed shared name like `/tmp/ripr-diff` lets the first local
    // account own the directory and deny every other account (cross-user
    // DoS), and lets an attacker pre-plant a symlink at the shared parent to
    // redirect the write. The unpredictable nanosecond stamp plus mkdir
    // EEXIST semantics prevent both: the directory is created fresh with
    // owner-only permissions, and a pre-existing name (including a symlink)
    // is never followed.
    for attempt in 0..16u32 {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let dir =
            std::env::temp_dir().join(format!("ripr-diff-{}-{stamp}{suffix}", std::process::id()));
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = std::fs::DirBuilder::new();
            builder.mode(0o700);
            builder
        };
        #[cfg(not(unix))]
        let builder = std::fs::DirBuilder::new();
        match builder.create(&dir) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(format!(
                    "create temporary diff dir {} failed: {err}",
                    dir.display()
                ));
            }
        }
        let path = dir.join("diff.patch");
        // create_new fails if the file exists, including as a symlink, so a
        // pre-planted symlink cannot win a TOCTOU overwrite (same race class
        // as #1948). Owner-only file permissions keep the diff unreadable by
        // other local users even if the parent is somehow reachable.
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => {
                use std::io::Write;
                if let Err(err) = file.write_all(diff_text.as_bytes()) {
                    // Do not leak the partial file (or the private dir) when
                    // the write fails: the caller never receives the path and
                    // cannot clean up.
                    let _ = std::fs::remove_file(&path);
                    let _ = std::fs::remove_dir(&dir);
                    return Err(format!(
                        "write temporary diff file {} failed: {err}",
                        path.display()
                    ));
                }
                return Ok(path);
            }
            Err(err) => {
                let _ = std::fs::remove_dir(&dir);
                return Err(format!(
                    "create temporary diff file {} failed: {err}",
                    path.display()
                ));
            }
        }
    }
    Err("create temporary diff dir failed: all name candidates exist".to_string())
}

#[cfg(test)]
mod tests {
    use super::write_temporary_diff_file;

    #[test]
    fn write_temporary_diff_file_writes_content_in_restricted_dir() -> Result<(), String> {
        let diff_text =
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n";
        let path = write_temporary_diff_file(diff_text)?;

        let content = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
        assert_eq!(content, diff_text);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let parent = path
                .parent()
                .ok_or_else(|| "temporary diff path has no parent".to_string())?;
            // Per-invocation private dir is owner-only (#2102).
            let dir_mode = std::fs::metadata(parent)
                .map_err(|err| err.to_string())?
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(dir_mode, 0o700);
            // The diff file itself is owner-read/write only.
            let file_mode = std::fs::metadata(&path)
                .map_err(|err| err.to_string())?
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(file_mode, 0o600);
        }
        std::fs::remove_file(&path).map_err(|err| err.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::remove_dir(parent).map_err(|err| err.to_string())?;
        }
        Ok(())
    }
}
