//! Shared fixtures for output-directory create failures (#4774).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A temporary `--root` plus an output path that cannot be created as a directory.
pub(crate) struct OutputDirFixture {
    pub(crate) root: PathBuf,
    pub(crate) target: PathBuf,
    parent: PathBuf,
    cleanup: PathBuf,
}

impl OutputDirFixture {
    /// `--out` / `--out-dir` sits under a directory with no write bits.
    #[cfg(unix)]
    pub(crate) fn unwritable(label: &str, child: &str) -> Result<Self, String> {
        let cleanup = temp_root(label)?;
        let root = cleanup.join("repo");
        fs::create_dir(&root).map_err(|err| format!("create fixture root: {err}"))?;
        let parent = cleanup.join("ro");
        fs::create_dir(&parent).map_err(|err| format!("create unwritable parent: {err}"))?;
        chmod_dir(&parent, 0o555)?;
        Ok(Self {
            target: parent.join(child),
            parent,
            root,
            cleanup,
        })
    }

    /// `--out` / `--out-dir` is a missing directory under a writable parent.
    pub(crate) fn writable(label: &str, child: &str) -> Result<Self, String> {
        let cleanup = temp_root(label)?;
        let root = cleanup.join("repo");
        fs::create_dir(&root).map_err(|err| format!("create fixture root: {err}"))?;
        let parent = cleanup.join("parent");
        fs::create_dir(&parent).map_err(|err| format!("create writable parent: {err}"))?;
        Ok(Self {
            target: parent.join(child),
            parent,
            root,
            cleanup,
        })
    }

    /// `--out` / `--out-dir` names an existing regular file.
    pub(crate) fn occupying_file(label: &str, child: &str) -> Result<Self, String> {
        let cleanup = temp_root(label)?;
        let root = cleanup.join("repo");
        fs::create_dir(&root).map_err(|err| format!("create fixture root: {err}"))?;
        let parent = cleanup.join("parent");
        fs::create_dir(&parent).map_err(|err| format!("create occupying parent: {err}"))?;
        let target = parent.join(child);
        fs::write(&target, b"not a directory")
            .map_err(|err| format!("write occupying file: {err}"))?;
        Ok(Self {
            root,
            target,
            parent,
            cleanup,
        })
    }

    pub(crate) fn path_arg(path: &Path) -> Result<&str, String> {
        path.to_str()
            .ok_or_else(|| format!("path is not UTF-8: {}", path.display()))
    }
}

impl Drop for OutputDirFixture {
    fn drop(&mut self) {
        let _ = chmod_dir(&self.parent, 0o755);
        let _ = fs::remove_dir_all(&self.cleanup);
    }
}

fn temp_root(label: &str) -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("ripr-4774-{label}-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&root).map_err(|err| format!("create temp root: {err}"))?;
    Ok(root)
}

fn chmod_dir(path: &Path, mode: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|err| format!("chmod {} to {mode:#o}: {err}", path.display()))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}
