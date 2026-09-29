//! Advisory output leaf acquisition. This is not ancestor or hard-link confinement.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// Replace `path` with `bytes` so a reader sees the old file or the complete
/// new one, never a truncated or half-written file (see
/// [`crate::atomic_file::replace_streamed`]). An interrupted run, a full disk
/// or a failed write leaves the previous file untouched. An existing
/// destination that is not a regular file is refused, as before, and so is a
/// read-only one: a direct write was refused by its permissions, and a rename
/// would replace it regardless of them.
pub(crate) fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_with(path, |file| file.write_all(bytes))
}

/// [`write`] for output produced incrementally: `fill` streams into the
/// temporary file, which is published only when `fill` succeeds.
pub(crate) fn write_with(
    path: &Path,
    fill: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    validate_destination(path)?;
    crate::atomic_file::replace_streamed(path, fill)
}

pub(crate) fn create_exclusive(path: &Path) -> io::Result<File> {
    open_new(path)
}

/// Rename does not follow the destination leaf, but refuse existing nonregular
/// destinations rather than silently replacing them.
pub(crate) fn validate_destination(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() && metadata.permissions().readonly() => {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "output destination is read-only",
            ))
        }
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "output destination is not a regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// `create_new` (O_CREAT|O_EXCL, CREATE_NEW) never opens an existing path,
/// symlink or otherwise, so every platform can create the temporary file
/// safely. The platform flags below are hardening where their values are
/// known, not the safety boundary.
fn open_new(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        // O_NOFOLLOW | O_NONBLOCK: reject links; do not wait for a FIFO reader.
        options.custom_flags(0x0002_0000 | 0x0000_0800);
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        // aarch64 uses the asm-generic values: O_NOFOLLOW is 0o100000 there,
        // and x86_64's 0x20000 would be O_LARGEFILE, which follows links.
        options.custom_flags(0x0000_8000 | 0x0000_0800);
    }
    #[cfg(all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(0x0000_0100 | 0x0000_0004);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        // OPEN_REPARSE_POINT; deny write/delete sharing while validating/writing.
        options.custom_flags(0x0020_0000).share_mode(0x0000_0001);
    }
    let file = options.open(path)?;
    if !file.metadata()?.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "opened output is not a regular file",
        ));
    }
    Ok(file)
}

/// Create a command output directory. When the tree is not writable, name the
/// flag that relocates the write so the raw OS error is not the only clue
/// (#4774).
pub(crate) fn create_output_dir(path: &Path, relocate_flag: &str) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|err| create_output_dir_error(path, &err, relocate_flag))
}

fn create_output_dir_error(path: &Path, err: &io::Error, relocate_flag: &str) -> String {
    let message = format!("create {} failed: {err}", path.display());
    if is_unwritable_output_dir(err) {
        format!("{message}; write elsewhere with {relocate_flag} PATH")
    } else {
        message
    }
}

fn is_unwritable_output_dir(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem
    )
}

#[cfg(test)]
mod tests {
    use super::write;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    fn fresh_dir(label: &str) -> Result<PathBuf, String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-file-write-{label}-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).map_err(|err| format!("create {}: {err}", dir.display()))?;
        Ok(dir)
    }

    fn leftovers(dir: &Path) -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        for entry in fs::read_dir(dir).map_err(|err| format!("read_dir: {err}"))? {
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
    fn replaces_a_longer_file_completely_and_leaves_no_temp() -> Result<(), String> {
        let dir = fresh_dir("replace")?;
        let path = dir.join("report.json");
        fs::write(&path, "x".repeat(4096)).map_err(|err| format!("seed: {err}"))?;
        write(&path, b"{}\n").map_err(|err| format!("write: {err}"))?;
        assert_eq!(
            fs::read(&path).map_err(|err| format!("read: {err}"))?,
            b"{}\n"
        );
        assert_eq!(leftovers(&dir)?, Vec::<String>::new());
        write(&dir.join("nested/dir/out.md"), b"# ok\n")
            .map_err(|err| format!("nested write: {err}"))?;
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn a_destination_name_near_the_filename_limit_is_still_written() -> Result<(), String> {
        // A temporary name derived from the destination's name would pass the
        // 255-byte filename limit here and turn a valid `--out` into an error.
        let dir = fresh_dir("long-name")?;
        let path = dir.join(format!("{}.json", "r".repeat(245)));
        write(&path, b"{}\n").map_err(|err| format!("write: {err}"))?;
        assert_eq!(
            fs::read(&path).map_err(|err| format!("read: {err}"))?,
            b"{}\n"
        );
        assert_eq!(leftovers(&dir)?, Vec::<String>::new());
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn a_read_only_destination_is_refused_and_left_untouched() -> Result<(), String> {
        let dir = fresh_dir("read-only")?;
        let path = dir.join("report.json");
        fs::write(&path, b"previous").map_err(|err| format!("seed: {err}"))?;
        let writable = fs::metadata(&path)
            .map_err(|err| format!("metadata: {err}"))?
            .permissions();
        let mut read_only = writable.clone();
        read_only.set_readonly(true);
        fs::set_permissions(&path, read_only).map_err(|err| format!("chmod: {err}"))?;
        let outcome = write(&path, b"replacement");
        let contents = fs::read(&path).map_err(|err| format!("read: {err}"));
        let leftover = leftovers(&dir);
        let _ = fs::set_permissions(&path, writable);
        let _ = fs::remove_dir_all(&dir);
        let Err(error) = outcome else {
            return Err("a read-only destination was replaced".to_string());
        };
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(contents?, b"previous");
        assert_eq!(leftover?, Vec::<String>::new());
        Ok(())
    }

    #[test]
    fn a_refused_destination_is_left_untouched() -> Result<(), String> {
        let dir = fresh_dir("refused")?;
        let blocker = dir.join("report.json");
        fs::create_dir(&blocker).map_err(|err| format!("seed dir: {err}"))?;
        fs::write(blocker.join("keep"), b"keep").map_err(|err| format!("seed keep: {err}"))?;
        let Err(error) = write(&blocker, b"new") else {
            return Err("a directory destination was replaced".to_string());
        };
        assert!(
            error.to_string().contains("not a regular file"),
            "unexpected error: {error}"
        );
        assert_eq!(
            fs::read(blocker.join("keep")).map_err(|err| format!("read keep: {err}"))?,
            b"keep"
        );
        assert_eq!(leftovers(&dir)?, Vec::<String>::new());
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_destination_is_refused_and_its_target_kept() -> Result<(), String> {
        let dir = fresh_dir("symlink")?;
        let target = dir.join("target.json");
        fs::write(&target, b"original").map_err(|err| format!("seed: {err}"))?;
        let link = dir.join("report.json");
        std::os::unix::fs::symlink(&target, &link).map_err(|err| format!("symlink: {err}"))?;
        if write(&link, b"new").is_ok() {
            return Err("a symlinked destination was written".to_string());
        }
        assert_eq!(
            fs::read(&target).map_err(|err| format!("read: {err}"))?,
            b"original"
        );
        assert_eq!(leftovers(&dir)?, Vec::<String>::new());
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn replacing_a_file_keeps_its_permissions() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = fresh_dir("mode")?;
        let path = dir.join("report.md");
        fs::write(&path, b"old").map_err(|err| format!("seed: {err}"))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640))
            .map_err(|err| format!("chmod: {err}"))?;
        write(&path, b"new").map_err(|err| format!("write: {err}"))?;
        let mode = fs::metadata(&path)
            .map_err(|err| format!("stat: {err}"))?
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o640);
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}

#[cfg(test)]
mod output_dir_tests {
    use super::*;
    use crate::testing::unwritable_output::OutputDirFixture;

    const PILOT_HINT: &str = "write elsewhere with --out PATH";
    const FIRST_PR_HINT: &str = "write elsewhere with --out-dir PATH";

    #[test]
    fn permission_denied_names_the_relocate_flag() {
        let err = io::Error::new(io::ErrorKind::PermissionDenied, "permission denied");
        let message = create_output_dir_error(Path::new("target/ripr/pilot"), &err, "--out");
        assert!(
            message.contains("create target/ripr/pilot failed:"),
            "{message}"
        );
        assert!(message.contains(PILOT_HINT), "{message}");
        assert!(!message.contains(FIRST_PR_HINT), "{message}");
    }

    #[test]
    fn read_only_filesystem_names_the_relocate_flag() {
        let err = io::Error::new(io::ErrorKind::ReadOnlyFilesystem, "Read-only file system");
        let message = create_output_dir_error(Path::new("target/ripr/reports"), &err, "--out-dir");
        assert!(message.contains(FIRST_PR_HINT), "{message}");
        assert!(!message.contains(PILOT_HINT), "{message}");
    }

    #[test]
    fn already_exists_does_not_name_the_relocate_flag() {
        let err = io::Error::new(io::ErrorKind::AlreadyExists, "File exists");
        let message = create_output_dir_error(Path::new("target/ripr/pilot"), &err, "--out");
        assert!(
            message.contains("create target/ripr/pilot failed: File exists"),
            "{message}"
        );
        assert!(!message.contains("write elsewhere"), "{message}");
    }

    #[test]
    fn not_a_directory_does_not_name_the_relocate_flag() {
        let err = io::Error::new(io::ErrorKind::NotADirectory, "Not a directory");
        let message = create_output_dir_error(Path::new("target/ripr/reports"), &err, "--out-dir");
        assert!(!message.contains("write elsewhere"), "{message}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_erofs_keeps_the_os_error_and_names_pilot_out() {
        let err = io::Error::from_raw_os_error(30);
        assert_eq!(err.kind(), io::ErrorKind::ReadOnlyFilesystem);
        let message = create_output_dir_error(Path::new("target/ripr/pilot"), &err, "--out");
        assert!(message.contains("(os error 30)"), "{message}");
        assert!(message.contains(PILOT_HINT), "{message}");
    }

    #[cfg(unix)]
    #[test]
    fn unwritable_parent_create_names_the_flag() -> Result<(), String> {
        let env = OutputDirFixture::unwritable("helper", "pilot")?;
        let error = match create_output_dir(&env.target, "--out") {
            Err(error) => error,
            Ok(()) => return Err("unwritable parent must fail".to_string()),
        };
        assert!(
            error.contains(&format!("create {} failed:", env.target.display())),
            "{error}"
        );
        assert!(error.contains(PILOT_HINT), "{error}");
        assert!(!error.contains(FIRST_PR_HINT), "{error}");
        Ok(())
    }

    #[test]
    fn occupying_file_create_does_not_name_the_flag() -> Result<(), String> {
        let env = OutputDirFixture::occupying_file("helper-file", "pilot")?;
        let error = match create_output_dir(&env.target, "--out") {
            Err(error) => error,
            Ok(()) => return Err("file path must fail".to_string()),
        };
        assert!(
            error.contains(&format!("create {} failed:", env.target.display())),
            "{error}"
        );
        assert!(
            !error.contains("write elsewhere"),
            "a file occupying the path is not a not-writable tree: {error}"
        );
        Ok(())
    }

    #[test]
    fn writable_path_creates_the_directory() -> Result<(), String> {
        let env = OutputDirFixture::writable("helper-ok", "pilot")?;
        create_output_dir(&env.target, "--out")?;
        if !env.target.is_dir() {
            return Err(format!(
                "writable create_output_dir must create {}",
                env.target.display()
            ));
        }
        Ok(())
    }
}
