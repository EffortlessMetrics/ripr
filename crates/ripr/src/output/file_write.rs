//! Advisory output leaf acquisition. This is not ancestor or hard-link confinement.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

pub(crate) fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut file = create(path)?;
    file.write_all(bytes)
}

/// Open `path` for a fresh write without following a leaf symlink. Parent
/// directories must already exist.
pub(crate) fn create(path: &Path) -> io::Result<File> {
    // Never truncate during acquisition: validate the opened object first.
    let file = open(path, false)?;
    file.set_len(0)?;
    Ok(file)
}

pub(crate) fn create_exclusive(path: &Path) -> io::Result<File> {
    open(path, true)
}

/// Rename does not follow the destination leaf, but refuse existing nonregular
/// destinations rather than silently replacing them. The open handle, not this
/// path snapshot, owns validation for direct writes.
pub(crate) fn validate_destination(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "output destination is not a regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn open(path: &Path, exclusive: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true);
    if exclusive {
        options.create_new(true);
    } else {
        options.create(true);
    }
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
    #[cfg(not(any(
        windows,
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(
            target_os = "macos",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )
    )))]
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "safe no-follow output writes are unsupported on this target",
        ));
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
