//! Capped file reads for the TypeScript preview adapter.
//!
//! Mirrors the repo-standard bounded-read pattern from `edit_cage.rs`
//! (`symlink_metadata` check → regular-file open → `take(limit + 1)` → reject
//! on excess) so a pathological workspace file can never be slurped fully into
//! memory. Two bounds apply per analysis run:
//!
//! - a per-file cap ([`DEFAULT_TS_MAX_FILE_READ_BYTES`], env
//!   `RIPR_TS_MAX_FILE_READ_BYTES`), and
//! - an aggregate per-run byte budget ([`DEFAULT_TS_MAX_WORKSPACE_READ_BYTES`],
//!   env `RIPR_TS_MAX_WORKSPACE_READ_BYTES`).
//!
//! Files that exceed either bound are surfaced as named limitations by the
//! adapter — never silently skipped — with the per-run disclosure sampled
//! (#5022, `read_limit_disclosure.rs`). Plain IO failures (unreadable
//! files) keep the pre-existing silent-continue behaviour; lane
//! `ts-d-silent-gaps` owns disclosure for those, and this lane deliberately
//! leaves that channel alone.

use std::collections::HashMap;
use std::io::Read as _;
use std::path::Path;

/// Env override for [`DEFAULT_TS_MAX_FILE_READ_BYTES`].
pub(crate) const TS_MAX_FILE_READ_BYTES_ENV: &str = "RIPR_TS_MAX_FILE_READ_BYTES";
/// Per-file read cap, mirroring `edit_cage.rs::MAX_CAPTURE_FILE_BYTES`.
pub(crate) const DEFAULT_TS_MAX_FILE_READ_BYTES: u64 = 16 * 1024 * 1024;
/// Env override for [`DEFAULT_TS_MAX_WORKSPACE_READ_BYTES`].
pub(crate) const TS_MAX_WORKSPACE_READ_BYTES_ENV: &str = "RIPR_TS_MAX_WORKSPACE_READ_BYTES";
/// Aggregate per-run byte budget across every workspace source read.
pub(crate) const DEFAULT_TS_MAX_WORKSPACE_READ_BYTES: u64 = 64 * 1024 * 1024;

/// Why a capped read did not produce source text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CappedReadError {
    /// Metadata/open/decode failure. Lane `ts-d-silent-gaps` owns disclosure
    /// for unreadable files; this lane keeps the silent-continue there.
    Io(String),
    /// The file's size exceeds the per-file cap.
    OverFileLimit {
        /// The per-file cap that was exceeded, in bytes.
        limit: u64,
    },
    /// The per-run aggregate byte budget is (or would be) exhausted.
    OverWorkspaceBudget {
        /// Bytes remaining in the aggregate budget when the file was rejected.
        remaining: u64,
    },
}

impl CappedReadError {
    /// Distinguishable, human-readable reason for limitation disclosure.
    pub(crate) fn reason(&self) -> String {
        match self {
            Self::Io(err) => format!("read error: {err}"),
            Self::OverFileLimit { limit } => format!(
                "file_read_capped: file exceeds the {limit}-byte capped read limit ({TS_MAX_FILE_READ_BYTES_ENV})"
            ),
            Self::OverWorkspaceBudget { remaining } => format!(
                "workspace_read_budget_exhausted: per-run read budget exhausted with only {remaining} bytes remaining ({TS_MAX_WORKSPACE_READ_BYTES_ENV})"
            ),
        }
    }

    /// Whether this error is a size bound (surfaced as a named limitation)
    /// rather than a plain IO failure (left to the read-error lane).
    pub(crate) fn is_size_limit(&self) -> bool {
        matches!(
            self,
            Self::OverFileLimit { .. } | Self::OverWorkspaceBudget { .. }
        )
    }
}

/// Parse a positive byte limit from an env override, failing closed to the
/// error string on invalid input (mirrors `rust/mod.rs::positive_limit_from_env`).
pub(crate) fn ts_byte_limit_from_env(
    env_name: &str,
    default: u64,
    value: Result<String, std::env::VarError>,
) -> Result<u64, String> {
    match value {
        Ok(raw) => {
            let parsed = raw
                .trim()
                .parse::<u64>()
                .map_err(|err| format!("{env_name} must be a positive integer: {err}"))?;
            if parsed == 0 {
                return Err(format!("{env_name} must be a positive integer"));
            }
            Ok(parsed)
        }
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(format!("{env_name} must be a positive integer"))
        }
    }
}

/// Per-file cap resolved from the environment, failing closed to the default
/// so a malformed operator override cannot abort analysis.
pub(crate) fn ts_file_read_limit() -> u64 {
    ts_byte_limit_from_env(
        TS_MAX_FILE_READ_BYTES_ENV,
        DEFAULT_TS_MAX_FILE_READ_BYTES,
        std::env::var(TS_MAX_FILE_READ_BYTES_ENV),
    )
    .unwrap_or(DEFAULT_TS_MAX_FILE_READ_BYTES)
}

/// Aggregate per-run byte budget resolved from the environment.
pub(crate) fn ts_workspace_read_budget() -> u64 {
    ts_byte_limit_from_env(
        TS_MAX_WORKSPACE_READ_BYTES_ENV,
        DEFAULT_TS_MAX_WORKSPACE_READ_BYTES,
        std::env::var(TS_MAX_WORKSPACE_READ_BYTES_ENV),
    )
    .unwrap_or(DEFAULT_TS_MAX_WORKSPACE_READ_BYTES)
}

/// Result of reading every discovered workspace source under the caps.
pub(crate) struct CappedWorkspaceSources {
    /// Files read successfully, keyed by workspace-relative path. Also serves
    /// as the Phase-1 source cache: every consumer (parse check, owner/test
    /// extraction, re-export index) reads from here instead of re-reading.
    pub(crate) sources: HashMap<std::path::PathBuf, String>,
    /// Named read-limit entries: one per over-limit file, plus at most one
    /// workspace-budget entry (the first file that tripped it).
    pub(crate) limits: Vec<(std::path::PathBuf, CappedReadError)>,
    /// Plain IO failures (missing file, permission, non-regular file,
    /// decode error), one per failed file. These are NOT read-limit
    /// entries: disclosure is owned by the read-failure lane, which counts
    /// them as skipped files and names unreadable changed paths.
    pub(crate) io_failures: Vec<(std::path::PathBuf, String)>,
}

/// Read every `files` entry under `root` with a per-file cap and an aggregate
/// byte budget.
///
/// - Files over the per-file cap each produce an `OverFileLimit` entry.
/// - The first file that does not fit the remaining aggregate budget produces
///   a single `OverWorkspaceBudget` entry; subsequent files are skipped
///   silently because that one entry already discloses the bound.
/// - The adapter's disclosure of these entries is sampled per run (#5022,
///   `read_limit_disclosure.rs`): a stable-sorted sample of refused paths
///   plus one summary entry carrying the true refused count, so a capped
///   monorepo (up to 20,000 workspace files) cannot emit one limitation
///   object per refused file.
/// - Plain IO failures are returned in `io_failures` for the read-failure
///   disclosure lane; they never produce a read-limit entry.
pub(crate) fn read_workspace_sources_capped(
    root: &Path,
    files: &[std::path::PathBuf],
    file_limit: u64,
    workspace_budget: u64,
) -> CappedWorkspaceSources {
    let mut sources = HashMap::new();
    let mut limits = Vec::new();
    let mut io_failures = Vec::new();
    let mut budget_reported = false;
    let mut consumed = 0u64;
    for relative in files {
        let mut remaining = workspace_budget.saturating_sub(consumed);
        let outcome = match crate::analysis::committed_source::lookup(root, relative) {
            crate::analysis::committed_source::CommittedSourceRead::Worktree => {
                read_source_capped(&root.join(relative), file_limit, Some(&mut remaining))
            }
            // A committed-history diff reads HEAD content for a dirty tracked
            // file under the same caps; a path absent at HEAD is not source.
            crate::analysis::committed_source::CommittedSourceRead::Committed(bytes) => {
                committed_source_capped(&root.join(relative), bytes, file_limit, remaining)
            }
            crate::analysis::committed_source::CommittedSourceRead::AbsentAtHead => continue,
        };
        if let Ok(source) = &outcome {
            consumed = consumed.saturating_add(source.len() as u64);
        }
        match outcome {
            Ok(source) => {
                sources.insert(relative.clone(), source);
            }
            Err(err) => match err {
                CappedReadError::OverFileLimit { .. } => {
                    limits.push((relative.clone(), err));
                }
                CappedReadError::OverWorkspaceBudget { .. } => {
                    if !budget_reported {
                        limits.push((relative.clone(), err));
                        budget_reported = true;
                    }
                }
                CappedReadError::Io(message) => {
                    io_failures.push((relative.clone(), message));
                }
            },
        }
    }
    CappedWorkspaceSources {
        sources,
        limits,
        io_failures,
    }
}

/// Read a single config-style file (`tsconfig.json`, `package.json`) under the
/// per-file cap, without touching the workspace aggregate budget.
pub(crate) fn read_config_capped(path: &Path) -> Result<String, CappedReadError> {
    read_source_capped(path, ts_file_read_limit(), None)
}

/// Apply the per-file cap, the remaining aggregate budget, and UTF-8
/// decoding to committed bytes, exactly as a working-tree read would.
fn committed_source_capped(
    path: &Path,
    bytes: Vec<u8>,
    file_limit: u64,
    remaining: u64,
) -> Result<String, CappedReadError> {
    let len = bytes.len() as u64;
    if len > file_limit {
        return Err(CappedReadError::OverFileLimit { limit: file_limit });
    }
    if len > remaining {
        return Err(CappedReadError::OverWorkspaceBudget { remaining });
    }
    String::from_utf8(bytes)
        .map_err(|err| CappedReadError::Io(format!("decode {}: {err}", path.display())))
}

/// Bounded read mirroring `edit_cage.rs`: metadata check → regular-file open
/// → `take(limit + 1)` → reject on excess.
fn read_source_capped(
    path: &Path,
    file_limit: u64,
    budget: Option<&mut u64>,
) -> Result<String, CappedReadError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|err| CappedReadError::Io(format!("inspect {}: {err}", path.display())))?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() || !file_type.is_file() {
        return Err(CappedReadError::Io(format!(
            "not a regular file: {}",
            path.display()
        )));
    }
    if metadata.len() > file_limit {
        return Err(CappedReadError::OverFileLimit { limit: file_limit });
    }
    let over_budget = budget
        .as_ref()
        .is_some_and(|remaining| metadata.len() > **remaining);
    if over_budget {
        let remaining = budget.as_ref().map(|remaining| **remaining).unwrap_or(0);
        return Err(CappedReadError::OverWorkspaceBudget { remaining });
    }
    let file = open_source_read_no_follow(path)?;
    let mut bytes = Vec::new();
    file.take(file_limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|err| CappedReadError::Io(format!("read {}: {err}", path.display())))?;
    if bytes.len() as u64 > file_limit {
        return Err(CappedReadError::OverFileLimit { limit: file_limit });
    }
    let text = String::from_utf8(bytes)
        .map_err(|err| CappedReadError::Io(format!("decode {}: {err}", path.display())))?;
    if let Some(remaining) = budget {
        *remaining = remaining.saturating_sub(text.len() as u64);
    }
    Ok(text)
}

/// Open `path` for reading without following symlinks and without blocking on
/// FIFO or device replacement. Mirrors the edit_cage capture-open flags so a
/// concurrent path swap cannot stall the bounded read between the metadata
/// check above and the open; the opened handle is then validated as a regular
/// file before any bytes are read.
fn open_source_read_no_follow(path: &Path) -> Result<std::fs::File, CappedReadError> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        // Linux O_NOFOLLOW | O_NONBLOCK. The nonblocking bit prevents FIFO or
        // device replacement from stalling the read before handle validation.
        options.custom_flags(0x0002_0000 | 0x0000_0800);
    }
    #[cfg(all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        // Darwin O_NOFOLLOW | O_NONBLOCK.
        options.custom_flags(0x0000_0100 | 0x0000_0004);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        // FILE_FLAG_OPEN_REPARSE_POINT keeps a replacement symlink from being
        // followed; the opened-handle metadata check below rejects it.
        options.custom_flags(0x0020_0000);
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
        return Err(CappedReadError::Io(format!(
            "safe no-follow open unsupported on this target: {}",
            path.display()
        )));
    }
    let file = options
        .open(path)
        .map_err(|err| CappedReadError::Io(format!("open {}: {err}", path.display())))?;
    // `File::metadata` describes the opened handle itself (fstat on the fd),
    // so a reparse point or swapped non-regular file is rejected here even
    // when the pre-open path check raced.
    let opened = file
        .metadata()
        .map_err(|err| CappedReadError::Io(format!("inspect opened {}: {err}", path.display())))?;
    if !opened.file_type().is_file() {
        return Err(CappedReadError::Io(format!(
            "not a regular file after open: {}",
            path.display()
        )));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0);
            let root = std::env::temp_dir().join(format!(
                "ripr-ts-bounded-read-{label}-{}-{stamp}",
                std::process::id()
            ));
            let created = fs::create_dir_all(&root);
            assert!(
                created.is_ok(),
                "create temp dir {}: {:?}",
                root.display(),
                created.err()
            );
            Self(root)
        }

        fn write(&self, name: &str, bytes: &[u8]) {
            let written = fs::write(self.0.join(name), bytes);
            assert!(
                written.is_ok(),
                "write temp file {name}: {:?}",
                written.err()
            );
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(any(
        windows,
        all(
            any(target_os = "linux", target_os = "macos"),
            any(target_arch = "x86_64", target_arch = "aarch64")
        )
    ))]
    #[test]
    fn no_follow_open_rejects_symlink_and_accepts_regular_file() -> Result<(), String> {
        let dir = TempDir::new("no-follow-open");
        dir.write("real.ts", b"export const x: number = 1;\n");
        // A regular file opens cleanly through the guarded helper.
        let regular = open_source_read_no_follow(&dir.0.join("real.ts"));
        assert!(
            regular.is_ok(),
            "regular file must open, got {:?}",
            regular.err()
        );
        // A symlink must not be followed even when the pre-open path check
        // raced (issue #4356: nonblocking, no-follow open flags).
        #[cfg(unix)]
        {
            let link_target = dir.0.join("real.ts");
            let link = dir.0.join("linked.ts");
            let created = std::os::unix::fs::symlink(&link_target, &link);
            assert!(created.is_ok(), "create symlink: {:?}", created.err());
            let outcome = open_source_read_no_follow(&link);
            let Err(err) = &outcome else {
                return Err(format!("symlink open must fail, got {outcome:?}"));
            };
            assert!(
                err.reason().contains("not a regular file") || err.reason().contains("open"),
                "symlink refusal must name the cause: {}",
                err.reason()
            );
        }
        Ok(())
    }

    // Exercise the same open authority used after the pre-open path inspection.
    // A separate process makes removal of O_NONBLOCK a bounded test failure,
    // rather than leaving the test runner blocked on a FIFO without a writer.
    #[cfg(all(
        any(target_os = "linux", target_os = "macos"),
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn no_follow_open_rejects_fifo_without_waiting_for_writer() -> Result<(), String> {
        use std::os::unix::fs::FileTypeExt as _;

        let dir = TempDir::new("fifo-open");
        let fifo = dir.0.join("replacement.ts");
        let mut create = std::process::Command::new("mkfifo");
        create.arg(&fifo);
        wait_fifo_test_process(create, "create FIFO")?;
        assert!(
            fs::symlink_metadata(&fifo)
                .map_err(|err| format!("inspect FIFO fixture: {err}"))?
                .file_type()
                .is_fifo(),
            "fixture must be a FIFO"
        );
        let executable =
            std::env::current_exe().map_err(|err| format!("locate test executable: {err}"))?;
        let module = module_path!()
            .split_once("::")
            .map(|(_, module)| module)
            .ok_or_else(|| "test module has no crate prefix".to_string())?;
        let mut child = std::process::Command::new(executable);
        let acknowledgement = dir.0.join("fifo-rejected.txt");
        child
            .args([
                "--exact",
                &format!("{module}::fifo_open_child"),
                "--nocapture",
            ])
            .env("RIPR_TS_FIFO_OPEN_CHILD", &fifo)
            .env("RIPR_TS_FIFO_OPEN_ACK", &acknowledgement);
        wait_fifo_test_process(child, "FIFO open without writer")?;
        assert_eq!(
            fs::read_to_string(&acknowledgement)
                .map_err(|err| format!("read child execution acknowledgement: {err}"))?,
            "FIFO refused by opened-handle inspection",
            "a zero-subject child run must not satisfy the nonblocking proof"
        );
        Ok(())
    }

    #[cfg(all(
        any(target_os = "linux", target_os = "macos"),
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    fn wait_fifo_test_process(command: std::process::Command, label: &str) -> Result<(), String> {
        let mut child = crate::process_owner::OwnedProcess::spawn(command)
            .map_err(|err| format!("spawn {label}: {err}"))?;
        let started = std::time::Instant::now();
        loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|err| format!("observe {label}: {err}"))?
            {
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!("{label} failed: {status}"))
                };
            }
            if started.elapsed() >= std::time::Duration::from_secs(5) {
                child
                    .terminate_tree()
                    .map_err(|err| format!("terminate {label}: {err}"))?;
                return Err(format!("{label} exceeded five seconds"));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[cfg(all(
        any(target_os = "linux", target_os = "macos"),
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn fifo_open_child() -> Result<(), String> {
        let Some(path) = std::env::var_os("RIPR_TS_FIFO_OPEN_CHILD") else {
            return Ok(());
        };
        let acknowledgement = std::env::var_os("RIPR_TS_FIFO_OPEN_ACK").ok_or_else(|| {
            "FIFO child is missing its execution acknowledgement path".to_string()
        })?;
        let outcome = open_source_read_no_follow(Path::new(&path));
        match outcome {
            Err(CappedReadError::Io(message))
                if message.contains("not a regular file after open") =>
            {
                fs::write(acknowledgement, "FIFO refused by opened-handle inspection")
                    .map_err(|err| format!("write child execution acknowledgement: {err}"))
            }
            other => Err(format!(
                "FIFO must be refused after handle inspection: {other:?}"
            )),
        }
    }

    #[test]
    fn over_limit_file_is_rejected_with_named_reason() -> Result<(), String> {
        let dir = TempDir::new("over-limit");
        dir.write("big.ts", &[b'a'; 100]);
        let outcome = read_source_capped(&dir.0.join("big.ts"), 50, None);
        let Err(err) = &outcome else {
            return Err(format!("over-limit read must fail, got {outcome:?}"));
        };
        assert_eq!(err, &CappedReadError::OverFileLimit { limit: 50 });
        assert!(err.is_size_limit());
        assert!(err.reason().contains("file_read_capped"));
        assert!(err.reason().contains("50"));
        Ok(())
    }

    #[test]
    fn under_limit_file_reads_and_reports_budget() {
        let dir = TempDir::new("under-limit");
        dir.write("ok.ts", b"export const value = 1;\n");
        let mut remaining = 100u64;
        let outcome = read_source_capped(&dir.0.join("ok.ts"), 1024, Some(&mut remaining));
        assert!(outcome.is_ok(), "under-limit read must succeed");
        let text = outcome.unwrap_or_default();
        assert!(text.contains("value = 1"));
        assert_eq!(remaining, 100 - text.len() as u64);
    }

    #[test]
    fn workspace_budget_exhaustion_is_distinguishable() -> Result<(), String> {
        let dir = TempDir::new("budget");
        dir.write("a.ts", &[b'a'; 40]);
        dir.write("b.ts", &[b'b'; 40]);
        let mut remaining = 50u64;
        let first_outcome = read_source_capped(&dir.0.join("a.ts"), 1024, Some(&mut remaining));
        assert!(first_outcome.is_ok(), "first read must succeed");
        let first = first_outcome.unwrap_or_default();
        assert_eq!(remaining, 50 - first.len() as u64);
        let outcome = read_source_capped(&dir.0.join("b.ts"), 1024, Some(&mut remaining));
        let Err(err) = &outcome else {
            return Err(format!("budget-exhausting read must fail, got {outcome:?}"));
        };
        assert!(matches!(err, CappedReadError::OverWorkspaceBudget { .. }));
        assert!(err.reason().contains("workspace_read_budget_exhausted"));
        Ok(())
    }

    #[test]
    fn capped_workspace_read_collects_limit_once_per_trigger() {
        let dir = TempDir::new("workspace");
        dir.write("small.ts", b"export const a = 1;\n");
        dir.write("big.ts", &[b'x'; 200]);
        dir.write("also_big.ts", &[b'y'; 200]);
        let files = vec![
            PathBuf::from("small.ts"),
            PathBuf::from("big.ts"),
            PathBuf::from("also_big.ts"),
        ];
        let outcome = read_workspace_sources_capped(&dir.0, &files, 100, 1024);
        assert_eq!(outcome.sources.len(), 1, "only the small file is cached");
        assert_eq!(outcome.limits.len(), 2, "one entry per over-limit file");
        assert!(
            outcome
                .limits
                .iter()
                .all(|(_, err)| matches!(err, CappedReadError::OverFileLimit { .. }))
        );
    }

    #[test]
    fn capped_workspace_read_reports_budget_exhaustion_once() {
        let dir = TempDir::new("workspace-budget");
        dir.write("a.ts", &[b'a'; 60]);
        dir.write("b.ts", &[b'b'; 60]);
        dir.write("c.ts", &[b'c'; 60]);
        let files = vec![
            PathBuf::from("a.ts"),
            PathBuf::from("b.ts"),
            PathBuf::from("c.ts"),
        ];
        let outcome = read_workspace_sources_capped(&dir.0, &files, 1024, 100);
        assert_eq!(outcome.sources.len(), 1);
        assert_eq!(outcome.limits.len(), 1, "budget exhaustion reported once");
        assert!(
            matches!(
                outcome.limits[0].1,
                CappedReadError::OverWorkspaceBudget { .. }
            ),
            "expected budget error, got {:?}",
            outcome.limits[0].1
        );
    }

    #[test]
    fn byte_limit_env_parsing_matches_repo_conventions() {
        assert_eq!(
            ts_byte_limit_from_env("RIPR_TEST_X", 16, Err(std::env::VarError::NotPresent)),
            Ok(16)
        );
        assert_eq!(
            ts_byte_limit_from_env("RIPR_TEST_X", 16, Ok(" 64 ".to_string())),
            Ok(64)
        );
        assert!(
            ts_byte_limit_from_env("RIPR_TEST_X", 16, Ok("0".to_string())).is_err(),
            "zero limit must be rejected"
        );
        assert!(
            ts_byte_limit_from_env("RIPR_TEST_X", 16, Ok("nope".to_string())).is_err(),
            "non-numeric limit must be rejected"
        );
    }

    #[test]
    fn committed_history_overlay_supplies_head_bytes_under_the_same_caps() {
        use crate::analysis::committed_source::{CommittedSourceOverlay, with_overlay};
        let dir = TempDir::new("committed-overlay");
        dir.write("dirty.ts", b"worktree bytes\n");
        dir.write("staged.ts", b"staged only\n");
        dir.write("clean.ts", b"clean bytes\n");
        dir.write("grown.ts", b"small\n");
        let overlay = CommittedSourceOverlay::from_entries(
            &dir.0,
            [
                ("dirty.ts", Some(&b"committed bytes\n"[..])),
                ("staged.ts", None),
                ("grown.ts", Some(&[b'g'; 200][..])),
            ],
        );
        let files = vec![
            PathBuf::from("dirty.ts"),
            PathBuf::from("staged.ts"),
            PathBuf::from("clean.ts"),
            PathBuf::from("grown.ts"),
        ];
        let outcome = with_overlay(Some(std::sync::Arc::new(overlay)), || {
            read_workspace_sources_capped(&dir.0, &files, 100, 1024)
        });
        assert_eq!(
            outcome
                .sources
                .get(&PathBuf::from("dirty.ts"))
                .map(String::as_str),
            Some("committed bytes\n"),
            "a dirty typescript file reads its committed bytes"
        );
        assert!(
            !outcome.sources.contains_key(&PathBuf::from("staged.ts")),
            "a path absent at HEAD is not committed source"
        );
        assert_eq!(
            outcome
                .sources
                .get(&PathBuf::from("clean.ts"))
                .map(String::as_str),
            Some("clean bytes\n")
        );
        assert!(
            outcome
                .limits
                .iter()
                .any(|(path, err)| path == &PathBuf::from("grown.ts")
                    && matches!(err, CappedReadError::OverFileLimit { .. })),
            "committed bytes obey the per-file cap: {:?}",
            outcome.limits
        );
        assert!(outcome.io_failures.is_empty(), "{:?}", outcome.io_failures);
    }
}
