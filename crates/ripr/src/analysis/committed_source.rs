//! Committed-content source reads for committed-history diffs.
//!
//! A committed-history diff (`git diff <base>...HEAD`, the default and
//! `--base` path) names line numbers in the files as they exist at `HEAD`.
//! The language adapters read source bytes from the working tree, so a
//! tracked file with uncommitted edits would pair committed diff lines with
//! uncommitted bytes and bind probes to the wrong owners and expressions.
//!
//! This module is the one shared source-read seam that closes that gap. The
//! diff pipeline probes the working tree once for paths that differ from
//! `HEAD` (tracked edits, plus untracked files an adapter would read) and,
//! for each of them, loads the `HEAD` blob. While the
//! overlay is installed (see [`with_overlay`]), every adapter read routed
//! through [`lookup`] sees committed bytes for those paths and treats paths
//! that do not exist at `HEAD` (a staged or untracked new file) as absent,
//! so source and test evidence alike reflect `HEAD`.
//! Clean paths still read the working tree, which equals `HEAD` for them.
//!
//! The overlay is scoped to the calling thread, mirroring
//! [`super::cancellation`]: reads on other threads (for example rayon
//! workers) do not see it, so every overlay-aware read site runs on the
//! pipeline thread.

use crate::core_error::CoreError;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Upper bound on the bytes kept from one probe or blob invocation. A larger
/// output fails closed with a named limit instead of an unbounded allocation.
const MAX_GIT_OUTPUT_BYTES: usize = 256 * 1024 * 1024;

/// Paths per `git ls-tree` invocation, keeping argument lists bounded.
const LS_TREE_CHUNK: usize = 256;

/// Committed content for the tracked paths that differ from `HEAD`.
#[derive(Debug, Default)]
pub(crate) struct CommittedSourceOverlay {
    root: PathBuf,
    /// `root` canonicalized, so a caller holding the canonical workspace root
    /// (the workspace authority) resolves the same entries.
    canonical_root: Option<PathBuf>,
    /// Root-relative, `/`-separated path → `HEAD` bytes, or `None` when the
    /// path is not a regular file at `HEAD` (absent, symlink, or gitlink).
    entries: BTreeMap<String, Option<Vec<u8>>>,
    /// Git-ignored paths an adapter routes, and git-ignored directories.
    /// Neither exists at `HEAD`, but discovery walks the disk without
    /// `.gitignore`, so both read as absent unless tracked.
    ignored: IgnoredPaths,
}

#[derive(Debug, Default)]
struct IgnoredPaths {
    files: BTreeSet<String>,
    directories: Vec<String>,
    /// Tracked files inside an ignored directory (force-added); these keep
    /// their working-tree reads. Git reports a directory as one ignored
    /// record only when it holds no tracked file, so this set is normally
    /// empty; it guards that assumption rather than a case seen in practice.
    tracked_within: BTreeSet<String>,
}

impl IgnoredPaths {
    fn contains(&self, key: &str) -> bool {
        self.files.contains(key)
            || (!self.tracked_within.contains(key)
                && self.directories.iter().any(|directory| {
                    key.strip_prefix(directory.as_str())
                        .is_some_and(|rest| rest.starts_with('/'))
                }))
    }
}

/// What a source read at one path should observe.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CommittedSourceRead {
    /// No overlay entry applies: read the working tree.
    Worktree,
    /// The path is dirty: these are its committed bytes.
    Committed(Vec<u8>),
    /// The path is dirty and has no regular-file content at `HEAD`.
    AbsentAtHead,
}

impl CommittedSourceOverlay {
    /// Dirty tracked paths, root-relative and `/`-separated, in order.
    pub(crate) fn dirty_paths(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// Dirty paths that exist at `HEAD` but are missing from the working
    /// tree. Discovery walks the working tree, so these cannot be found and
    /// are disclosed instead of silently dropped.
    pub(crate) fn committed_paths_missing_on_disk(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(_, bytes)| bytes.is_some())
            .filter(|(path, _)| !self.root.join(path.as_str()).exists())
            .map(|(path, _)| path.clone())
            .collect()
    }

    /// Dirty paths a language adapter routes (source and test files alike),
    /// in order. Only these make a `--worktree` run read different bytes, so
    /// an edited README or workflow file does not raise the
    /// uncommitted-edits note.
    pub(crate) fn dirty_source_paths(&self) -> Vec<String> {
        self.dirty_paths()
            .filter(|path| super::language::route(Path::new(path)).is_some())
            .map(str::to_string)
            .collect()
    }

    /// Test-only constructor for adapter read-seam tests that need an
    /// overlay without a git fixture.
    #[cfg(test)]
    pub(crate) fn from_entries(
        root: &Path,
        entries: impl IntoIterator<Item = (&'static str, Option<&'static [u8]>)>,
    ) -> Self {
        Self {
            root: root.to_path_buf(),
            canonical_root: root.canonicalize().ok(),
            ignored: IgnoredPaths::default(),
            entries: entries
                .into_iter()
                .map(|(path, bytes)| (path.to_string(), bytes.map(<[u8]>::to_vec)))
                .collect(),
        }
    }

    fn lookup(&self, root: &Path, relative: &Path) -> CommittedSourceRead {
        if root != self.root && self.canonical_root.as_deref() != Some(root) {
            return CommittedSourceRead::Worktree;
        }
        let Some(key) = normalized_key(relative) else {
            return CommittedSourceRead::Worktree;
        };
        match self.entries.get(&key) {
            None if self.ignored.contains(&key) => CommittedSourceRead::AbsentAtHead,
            None => CommittedSourceRead::Worktree,
            Some(Some(bytes)) => CommittedSourceRead::Committed(bytes.clone()),
            Some(None) => CommittedSourceRead::AbsentAtHead,
        }
    }
}

/// Root-relative `/`-separated key for `relative`, or `None` when the path
/// is absolute or escapes the root (no overlay entry can apply to it).
fn normalized_key(relative: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?.to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

thread_local! {
    static CURRENT_OVERLAY: RefCell<Option<Arc<CommittedSourceOverlay>>> =
        const { RefCell::new(None) };
}

struct OverlayGuard(Option<Arc<CommittedSourceOverlay>>);

impl Drop for OverlayGuard {
    fn drop(&mut self) {
        let previous = self.0.take();
        CURRENT_OVERLAY.with(|slot| {
            *slot.borrow_mut() = previous;
        });
    }
}

/// Run `work` with `overlay` installed for this thread; the previous overlay
/// is restored afterwards, including on early return.
pub(crate) fn with_overlay<T>(
    overlay: Option<Arc<CommittedSourceOverlay>>,
    work: impl FnOnce() -> T,
) -> T {
    let previous = CURRENT_OVERLAY.with(|slot| slot.replace(overlay));
    let _guard = OverlayGuard(previous);
    work()
}

/// Root-relative paths the installed overlay serves from `HEAD` although the
/// working tree lacks them; empty without an overlay for `root`. A caller
/// that lists the working tree adds these to see the committed file set.
pub(crate) fn committed_paths_missing_on_disk(root: &Path) -> Vec<String> {
    CURRENT_OVERLAY.with(|slot| {
        slot.borrow()
            .as_ref()
            .filter(|overlay| {
                overlay.root == root || overlay.canonical_root.as_deref() == Some(root)
            })
            .map(|overlay| overlay.committed_paths_missing_on_disk())
            .unwrap_or_default()
    })
}

/// What a read of `root.join(relative)` should observe under the installed
/// overlay. Without an overlay every path reads the working tree.
pub(crate) fn lookup(root: &Path, relative: &Path) -> CommittedSourceRead {
    CURRENT_OVERLAY.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(CommittedSourceRead::Worktree, |overlay| {
                overlay.lookup(root, relative)
            })
    })
}

/// Read `root.join(relative)` as source bytes: committed bytes for a dirty
/// path, `Ok(None)` for a dirty path with no content at `HEAD`, and the
/// working-tree bytes otherwise.
pub(crate) fn read_source_bytes(root: &Path, relative: &Path) -> std::io::Result<Option<Vec<u8>>> {
    match lookup(root, relative) {
        CommittedSourceRead::Worktree => std::fs::read(root.join(relative)).map(Some),
        CommittedSourceRead::Committed(bytes) => Ok(Some(bytes)),
        CommittedSourceRead::AbsentAtHead => Ok(None),
    }
}

/// Probe `root` for paths that differ from `HEAD` (tracked edits and
/// untracked adapter-routed files) and load their committed content. Returns
/// `Ok(None)` when the tree matches `HEAD` for every such path.
///
/// Fails closed: when the dirty set or a committed blob cannot be
/// established, the error names the failing step, because running the
/// analysis anyway would mix committed diff lines with working-tree bytes.
pub(crate) fn probe(
    root: &Path,
    git_timeout: Option<Duration>,
) -> Result<Option<CommittedSourceOverlay>, CoreError> {
    // `None` is the caller's "no deadline" (`--git-timeout 0`), as for the
    // diff loader; output stays bounded either way.
    let deadline = git_timeout;
    let status = git_bytes(
        root,
        &[
            "status",
            "--porcelain",
            "-z",
            "--untracked-files=all",
            // Ignored files are listed too (an ignored directory as one
            // `dir/` record): discovery does not apply `.gitignore`.
            "--ignored=matching",
            "--",
            ".",
        ],
        deadline,
    )?;
    let records = parse_porcelain_z(&status)?;
    if records.dirty.is_empty()
        && records.ignored_files.is_empty()
        && records.ignored_directories.is_empty()
    {
        return Ok(None);
    }
    let prefix = String::from_utf8(git_bytes(root, &["rev-parse", "--show-prefix"], deadline)?)
        .map_err(|_utf8_error| {
            "committed-source probe: `git rev-parse --show-prefix` returned a non-UTF-8 prefix"
                .to_string()
        })?;
    let prefix = prefix.trim_end_matches(['\n', '\r']);
    let below_root = |paths: Vec<String>| {
        paths
            .into_iter()
            .filter_map(|path| path.strip_prefix(prefix).map(str::to_string))
            .filter(|path| !path.is_empty())
            .collect::<BTreeSet<_>>()
    };
    let dirty = below_root(records.dirty);
    let mut ignored = IgnoredPaths {
        files: below_root(records.ignored_files),
        directories: below_root(records.ignored_directories)
            .into_iter()
            .collect(),
        tracked_within: BTreeSet::new(),
    };
    if dirty.is_empty() && ignored.files.is_empty() && ignored.directories.is_empty() {
        return Ok(None);
    }
    let directories = ignored
        .directories
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    for chunk in directories.chunks(LS_TREE_CHUNK) {
        let mut args = vec!["--literal-pathspecs", "ls-files", "-z", "--"];
        args.extend(chunk.iter().copied());
        for record in git_bytes(root, &args, deadline)?.split(|byte| *byte == 0) {
            if let Ok(path) = utf8_path(record) {
                ignored.tracked_within.insert(path);
            }
        }
    }
    let mut head_blobs = BTreeMap::new();
    let dirty_list = dirty.iter().map(String::as_str).collect::<Vec<_>>();
    for chunk in dirty_list.chunks(LS_TREE_CHUNK) {
        // Literal pathspecs: a file named `:x` or `*.rs` is a path, not magic.
        let mut args = vec!["--literal-pathspecs", "ls-tree", "-r", "-z", "HEAD", "--"];
        args.extend(chunk.iter().copied());
        let listing = git_bytes(root, &args, deadline)?;
        for (path, entry) in parse_ls_tree_z(&listing)? {
            head_blobs.insert(path, entry);
        }
    }
    let mut entries = BTreeMap::new();
    for path in dirty {
        let content =
            match head_blobs.get(&path) {
                Some(LsTreeEntry { mode, object }) if is_regular_file_mode(mode) => Some(
                    git_bytes(root, &["cat-file", "blob", object.as_str()], deadline)?,
                ),
                _ => None,
            };
        entries.insert(path, content);
    }
    Ok(Some(CommittedSourceOverlay {
        root: root.to_path_buf(),
        canonical_root: root.canonicalize().ok(),
        entries,
        ignored,
    }))
}

fn is_regular_file_mode(mode: &str) -> bool {
    mode == "100644" || mode == "100755"
}

fn git_bytes(root: &Path, args: &[&str], deadline: Option<Duration>) -> Result<Vec<u8>, CoreError> {
    let describe = args.iter().take(2).copied().collect::<Vec<_>>().join(" ");
    let output = crate::git::run_git_output_with_optional_deadline_and_limit(
        root,
        args,
        deadline,
        MAX_GIT_OUTPUT_BYTES,
    )
    .map_err(|error| {
        error.with_context(format!("committed-source probe: `git {describe}` failed"))
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.lines().next().unwrap_or("unknown git error").trim();
        return Err(format!(
            "committed-source probe: `git {describe}` exited with {}: {detail}",
            output.status
        )
        .into());
    }
    Ok(output.stdout)
}

/// Repo-relative paths from `git status --porcelain -z`, limited to files a
/// language adapter routes: nothing else is read through the overlay, so an
/// unrelated binary, README or non-UTF-8 name never costs a blob load or
/// fails the probe.
#[derive(Debug, Default, PartialEq, Eq)]
struct StatusRecords {
    /// Tracked paths that differ from `HEAD`, and untracked routed files.
    /// Rename and copy records carry a second (original) path; both sides
    /// are dirty.
    dirty: Vec<String>,
    ignored_files: Vec<String>,
    /// Ignored directories, without the trailing `/`.
    ignored_directories: Vec<String>,
}

fn routed(path: &[u8]) -> bool {
    super::language::route(Path::new(String::from_utf8_lossy(path).as_ref())).is_some()
}

fn parse_porcelain_z(bytes: &[u8]) -> Result<StatusRecords, String> {
    let mut out = StatusRecords::default();
    let mut records = bytes.split(|byte| *byte == 0);
    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        let (Some(index_status), Some(path)) = (record.first(), record.get(3..)) else {
            return Err(format!(
                "committed-source probe: malformed status record `{}`",
                String::from_utf8_lossy(record)
            ));
        };
        let worktree_status = record.get(1).copied().unwrap_or(b' ');
        let has_original =
            matches!(index_status, b'R' | b'C') || matches!(worktree_status, b'R' | b'C');
        let original = if has_original { records.next() } else { None };
        if *index_status == b'!' {
            if let Some(directory) = path.strip_suffix(b"/") {
                // A non-UTF-8 directory name keys no overlay lookup anyway.
                if let Ok(directory) = utf8_path(directory) {
                    out.ignored_directories.push(directory);
                }
            } else if routed(path) {
                out.ignored_files.push(utf8_path(path)?);
            }
            continue;
        }
        if routed(path) {
            out.dirty.push(utf8_path(path)?);
        }
        if let Some(original) = original.filter(|original| routed(original)) {
            out.dirty.push(utf8_path(original)?);
        }
    }
    Ok(out)
}

#[derive(Debug)]
struct LsTreeEntry {
    mode: String,
    object: String,
}

/// Parse `git ls-tree -z` output into (cwd-relative path, entry) pairs.
fn parse_ls_tree_z(bytes: &[u8]) -> Result<Vec<(String, LsTreeEntry)>, String> {
    let mut out = Vec::new();
    for record in bytes.split(|byte| *byte == 0) {
        if record.is_empty() {
            continue;
        }
        let text = utf8_path(record)?;
        let Some((meta, path)) = text.split_once('\t') else {
            return Err(format!(
                "committed-source probe: malformed ls-tree record `{text}`"
            ));
        };
        let mut parts = meta.split_whitespace();
        let (Some(mode), Some(_kind), Some(object)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(format!(
                "committed-source probe: malformed ls-tree record `{text}`"
            ));
        };
        out.push((
            path.to_string(),
            LsTreeEntry {
                mode: mode.to_string(),
                object: object.to_string(),
            },
        ));
    }
    Ok(out)
}

fn utf8_path(bytes: &[u8]) -> Result<String, String> {
    String::from_utf8(bytes.to_vec()).map_err(|_utf8_error| {
        format!(
            "committed-source probe: tracked path `{}` is not valid UTF-8; refusing a lossy committed-content lookup",
            String::from_utf8_lossy(bytes)
        )
    })
}

#[cfg(test)]
mod tests;
