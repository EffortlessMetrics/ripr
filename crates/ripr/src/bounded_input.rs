//! Bounded reads for user-named CLI inputs (#4480).
//!
//! The CLI reads `--diff`, `--diff -` (stdin), JSON artifact flags such as
//! `outcome --before/--after`, `--suppression-policy`, and `ripr.toml` fully
//! into memory. An unbounded `read_to_string` on a character device
//! (`--diff /dev/zero`) or an accidentally huge file (a multi-GB log passed as
//! `--diff`) grows memory until the process is terminated instead of failing.
//!
//! These helpers are drop-in replacements for `std::fs::read_to_string` /
//! `std::fs::read` and return `std::io::Result`, so every call site keeps its
//! existing error wording (which names the flag or path) and its exit class:
//! the CLI maps a returned `Err(String)` to exit 2 (docs/EXIT_CODES.md).
//!
//! The bound is enforced while reading (`take(limit + 1)`), not from
//! metadata: a character device or FIFO reports length 0, and a file that
//! grows between a metadata check and the read would bypass a pre-check.
//! Non-regular files are deliberately not refused: `--diff <(git diff ...)`
//! passes a FIFO, and stdin (`-`) is a pipe; both are legitimate and are
//! bounded by the same cap instead.

use std::io::Read;
use std::path::Path;

/// Byte cap for a single user-named CLI input.
///
/// Real diffs and repo-exposure artifacts for large repositories reach tens of
/// megabytes; 256 MiB is far above any legitimate input while still failing
/// closed on an unbounded one. It matches the LSP's `MAX_LSP_ARTIFACT_BYTES`
/// and the CLI's `MAX_AGENT_VERIFY_SNAPSHOT_BYTES` (#2921), so the same file
/// is accepted or refused consistently across surfaces.
pub(crate) const MAX_CLI_INPUT_BYTES: u64 = 256 * 1024 * 1024;

/// Bounded replacement for `std::fs::read_to_string` on a user-named input.
pub(crate) fn read_to_string(path: impl AsRef<Path>) -> std::io::Result<String> {
    read_to_string_with_limit(path, MAX_CLI_INPUT_BYTES)
}

/// Bounded replacement for `std::fs::read` on a user-named input.
pub(crate) fn read(path: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
    read_with_limit(path, MAX_CLI_INPUT_BYTES)
}

/// Bounded byte read of an already-open stream, for callers that decode
/// non-UTF-8 input themselves (`--diff -`, #4584).
pub(crate) fn read_reader(reader: impl Read) -> std::io::Result<Vec<u8>> {
    read_reader_with_limit(reader, MAX_CLI_INPUT_BYTES)
}

/// `limit` is a parameter so tests can exercise the cap without
/// materializing 256 MiB; production callers use [`read_to_string`].
pub(crate) fn read_to_string_with_limit(
    path: impl AsRef<Path>,
    limit: u64,
) -> std::io::Result<String> {
    read_reader_to_string_with_limit(open_within_limit(path.as_ref(), limit)?, limit)
}

pub(crate) fn read_with_limit(path: impl AsRef<Path>, limit: u64) -> std::io::Result<Vec<u8>> {
    read_reader_with_limit(open_within_limit(path.as_ref(), limit)?, limit)
}

/// Open `path`, rejecting a regular file whose reported length already
/// exceeds `limit` without reading it. This is only a fast path: devices and
/// FIFOs report length 0, so the read itself stays capped.
fn open_within_limit(path: &Path, limit: u64) -> std::io::Result<std::fs::File> {
    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.is_file() && metadata.len() > limit {
        return Err(oversize_error(limit));
    }
    Ok(file)
}

pub(crate) fn read_reader_to_string_with_limit(
    reader: impl Read,
    limit: u64,
) -> std::io::Result<String> {
    let bytes = read_reader_with_limit(reader, limit)?;
    // Same wording as `std::fs::read_to_string`, plus the offending offset.
    String::from_utf8(bytes).map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("stream did not contain valid UTF-8 ({err})"),
        )
    })
}

fn read_reader_with_limit(reader: impl Read, limit: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(oversize_error(limit));
    }
    Ok(bytes)
}

fn oversize_error(limit: u64) -> std::io::Error {
    const MIB: u64 = 1024 * 1024;
    let readable = if limit >= MIB && limit.is_multiple_of(MIB) {
        format!(" ({} MiB)", limit / MIB)
    } else {
        String::new()
    };
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!(
            "input exceeds the {limit} byte input limit{readable}; ripr reads file inputs fully into memory, so pass a finite input under the limit (device paths such as /dev/zero never end)"
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, contents: &[u8]) -> Result<std::path::PathBuf, String> {
        let dir =
            std::env::temp_dir().join(format!("ripr-bounded-input-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|err| format!("create dir: {err}"))?;
        let path = dir.join("input");
        std::fs::write(&path, contents).map_err(|err| format!("write input: {err}"))?;
        Ok(path)
    }

    #[test]
    fn reads_input_at_limit_and_rejects_one_byte_over() -> Result<(), String> {
        let path = temp_file("limit", b"12345")?;
        let text = read_to_string_with_limit(&path, 5).map_err(|err| err.to_string())?;
        assert_eq!(text, "12345");
        let bytes = read_with_limit(&path, 5).map_err(|err| err.to_string())?;
        assert_eq!(bytes, b"12345");

        let err = read_to_string_with_limit(&path, 4)
            .err()
            .ok_or("5-byte file must exceed a 4-byte limit")?;
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(
            err.to_string()
                .contains("input exceeds the 4 byte input limit;"),
            "{err}"
        );
        let err = read_with_limit(&path, 4)
            .err()
            .ok_or("5-byte file must exceed a 4-byte limit")?;
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(
            err.to_string()
                .contains("input exceeds the 4 byte input limit;"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn stream_read_is_bounded_and_reports_mib_limit() -> Result<(), String> {
        // An endless reader stands in for stdin fed from /dev/zero: an
        // unbounded read never returns.
        let err = read_reader_to_string_with_limit(std::io::repeat(b'a'), 16)
            .err()
            .ok_or("endless stream must be refused")?;
        assert!(err.to_string().contains("16 byte input limit"), "{err}");
        assert!(
            oversize_error(MAX_CLI_INPUT_BYTES)
                .to_string()
                .contains("268435456 byte input limit (256 MiB)"),
        );
        Ok(())
    }

    #[test]
    fn stream_byte_read_keeps_non_utf8_bytes() -> Result<(), String> {
        // `--diff -` decodes non-UTF-8 itself (#4584), so the byte read
        // must not reject or rewrite them.
        let bytes = read_reader(&b"+caf\xe9\n"[..]).map_err(|err| err.to_string())?;
        assert_eq!(bytes, b"+caf\xe9\n");
        Ok(())
    }

    #[test]
    fn missing_file_keeps_not_found_kind() -> Result<(), String> {
        let path = std::env::temp_dir().join("ripr-bounded-input-definitely-missing/input");
        let err = read_to_string(&path)
            .err()
            .ok_or("missing file must fail")?;
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn character_device_is_refused_instead_of_read_forever() -> Result<(), String> {
        let err = read_to_string_with_limit(Path::new("/dev/zero"), 1024)
            .err()
            .ok_or("/dev/zero must be refused")?;
        assert!(err.to_string().contains("1024 byte input limit"), "{err}");
        Ok(())
    }
}
