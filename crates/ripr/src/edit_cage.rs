//! Internal repository observation and edit-cage evaluation for a future
//! durable `RepairAttempt`.
//!
//! This slice captures a bounded Git/worktree baseline and feeds its derived
//! delta into the pure verdict kernel. Binding that evidence into the durable
//! attempt manifest remains a separate #2927/#3163 slice.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

const MAX_CAPTURE_PATHS: usize = 10_000;
const MAX_CAPTURE_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CAPTURE_TOTAL_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CAPTURE_TOTAL_WORK_BYTES: u64 = 384 * 1024 * 1024;
const IGNORED_FILE_SAMPLE_BYTES: u64 = 4 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AttemptPathChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AttemptPathChangeWire")]
pub(crate) struct AttemptPathChange {
    path: PathBuf,
    kind: AttemptPathChangeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_path: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttemptPathChangeWire {
    path: PathBuf,
    kind: AttemptPathChangeKind,
    previous_path: Option<PathBuf>,
}

impl TryFrom<AttemptPathChangeWire> for AttemptPathChange {
    type Error = String;

    fn try_from(wire: AttemptPathChangeWire) -> Result<Self, Self::Error> {
        match (wire.kind, wire.previous_path) {
            (AttemptPathChangeKind::Renamed, Some(previous_path)) => {
                Ok(Self::renamed(previous_path, wire.path))
            }
            (AttemptPathChangeKind::Renamed, None) => {
                Err("a renamed path change requires previous_path".to_string())
            }
            (kind, Some(_)) => Err(format!(
                "a {} path change must not carry previous_path",
                path_change_kind_name(kind)
            )),
            (kind, None) => Ok(Self::new(wire.path, kind)),
        }
    }
}

impl AttemptPathChange {
    pub(crate) fn added(path: impl Into<PathBuf>) -> Self {
        Self::new(path, AttemptPathChangeKind::Added)
    }

    pub(crate) fn modified(path: impl Into<PathBuf>) -> Self {
        Self::new(path, AttemptPathChangeKind::Modified)
    }

    pub(crate) fn deleted(path: impl Into<PathBuf>) -> Self {
        Self::new(path, AttemptPathChangeKind::Deleted)
    }

    pub(crate) fn renamed(from: impl Into<PathBuf>, to: impl Into<PathBuf>) -> Self {
        Self {
            path: to.into(),
            kind: AttemptPathChangeKind::Renamed,
            previous_path: Some(from.into()),
        }
    }

    fn new(path: impl Into<PathBuf>, kind: AttemptPathChangeKind) -> Self {
        Self {
            path: path.into(),
            kind,
            previous_path: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct AttemptDelta {
    /// False when HEAD/worktree/baseline movement makes attribution unsafe.
    pub(crate) comparable: bool,
    pub(crate) changes: Vec<AttemptPathChange>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct AttemptBaseline {
    root: PathBuf,
    head: String,
    policy: EditCagePolicy,
    paths: BTreeMap<String, RepositoryPathState>,
    ambiguous: bool,
    #[cfg(windows)]
    #[serde(skip, default)]
    _unknown_ignored_write_guards: Vec<fs::File>,
    #[cfg(windows)]
    #[serde(skip, default)]
    _expected_write_authorities: Vec<winsafe::guard::CloseHandleGuard<winsafe::HFILE>>,
}

impl AttemptBaseline {
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// The declared cage policy the baseline was captured under, for consumers
    /// that restate the retained edit surface (repair-attempt binding records).
    pub(crate) fn policy(&self) -> &EditCagePolicy {
        &self.policy
    }

    /// The baseline index entry metadata (`git ls-files --stage` shape) for
    /// one path, for consumers that must verify the index copy a checkout
    /// would restore is still the baseline's index copy.
    pub(crate) fn index_entry(&self, path: &str) -> Option<&str> {
        self.paths
            .get(path)
            .and_then(|state| state.index_entry.as_deref())
    }

    /// The baseline worktree content digest (sha256 hex) for one path when the
    /// baseline captured it as a regular file.
    pub(crate) fn worktree_digest(&self, path: &str) -> Option<&str> {
        self.paths
            .get(path)
            .and_then(|state| match &state.worktree_identity {
                WorktreeIdentity::File { digest, .. } => Some(digest.as_str()),
                _ => None,
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct RepositoryPathState {
    index_entry: Option<String>,
    worktree_identity: WorktreeIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum WorktreeIdentity {
    Missing,
    File {
        digest: String,
        executable: bool,
    },
    IgnoredFileProbe {
        sample_digest: String,
        snapshot: BoundedFileSnapshot,
    },
    Symlink(PathBuf),
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct BoundedFileSnapshot {
    len: u64,
    modified: Duration,
    executable: bool,
    #[cfg(unix)]
    change_seconds: i64,
    #[cfg(unix)]
    change_nanoseconds: i64,
    #[cfg(windows)]
    creation_time: u64,
    #[cfg(windows)]
    file_attributes: u32,
}

pub(crate) fn capture_attempt_baseline(
    root: &Path,
    policy: &EditCagePolicy,
) -> Result<AttemptBaseline, String> {
    let root = canonical_repository_root(root)?;
    capture_repository_state(root, policy.clone())
}

pub(crate) fn evaluate_repository_edit_cage(
    baseline: &AttemptBaseline,
) -> Result<EditCageVerdict, String> {
    Ok(evaluate_repository_edit_cage_with_delta(baseline)?.1)
}

pub(crate) fn evaluate_repository_edit_cage_with_delta(
    baseline: &AttemptBaseline,
) -> Result<(AttemptDelta, EditCageVerdict), String> {
    let after = capture_repository_state(baseline.root.clone(), baseline.policy.clone())?;
    let delta = delta_from_repository_states(baseline, &after);
    let verdict = evaluate_edit_cage(&baseline.policy, &delta);
    Ok((delta, verdict))
}

fn capture_repository_state(
    root: PathBuf,
    policy: EditCagePolicy,
) -> Result<AttemptBaseline, String> {
    let head = git_text(&root, &["rev-parse", "--verify", "HEAD"])?;
    let index = git_bytes(&root, &["ls-files", "--stage", "-z"])?;
    let tracked = git_bytes(&root, &["ls-files", "-z"])?;
    let untracked = git_bytes(&root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    let ignored = git_bytes(
        &root,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "-z",
        ],
    )?;

    let (inventory, mut ambiguous) =
        inventory_paths(&tracked, &untracked, &ignored, MAX_CAPTURE_PATHS)?;

    let mut paths = BTreeMap::new();
    let mut remaining_file_bytes = MAX_CAPTURE_TOTAL_FILE_BYTES;
    let mut remaining_work_bytes = MAX_CAPTURE_TOTAL_WORK_BYTES;
    #[cfg(windows)]
    let mut unknown_ignored_write_guards = Vec::new();
    #[cfg(windows)]
    let mut expected_write_authorities = Vec::new();
    for (path, ignored_path) in inventory {
        // The before-phase serialization lock is an authority artifact, not
        // an agent edit. It is necessarily held while the baseline is
        // captured, so probing its metadata would make every real repair
        // incomparable on Windows.
        if path == "target/ripr/repair-attempts/.before.lock" {
            continue;
        }
        let expected_write = policy
            .expected_operational_writes
            .iter()
            .any(|rule| rule.matches(&path));
        let writable_surface = expected_write
            || policy
                .allowed_edit_surface
                .iter()
                .any(|rule| rule.matches(&path));
        if writable_surface && !writable_path_components_are_safe(&root, &path)? {
            ambiguous = true;
        }
        #[cfg(windows)]
        let expected_write_authority = if expected_write {
            match writable_regular_file_authority(&root.join(&path)) {
                Ok(authority) => authority,
                Err(_) => {
                    ambiguous = true;
                    None
                }
            }
        } else {
            None
        };
        #[cfg(windows)]
        if writable_surface
            && !expected_write
            && writable_regular_file_authority(&root.join(&path)).is_err()
        {
            ambiguous = true;
        }
        #[cfg(windows)]
        let unknown_ignored_write_guard = if ignored_path && !expected_write {
            // Windows metadata and sparse samples are only a bounded probe, not
            // content identity. Acquire the no-write/no-delete authority before
            // the first read and retain it for the whole attempt so an ignored
            // file cannot move through an unsampled, metadata-restored edit.
            match open_regular_file_no_follow_with_write_sharing(&root.join(&path), false) {
                Ok(guard) => Some(guard),
                Err(_) => {
                    ambiguous = true;
                    None
                }
            }
        } else {
            None
        };
        let worktree_identity = worktree_identity(
            &root,
            &path,
            &mut remaining_file_bytes,
            &mut remaining_work_bytes,
            ignored_path,
            ignored_path && expected_write,
        )?;
        ambiguous |= worktree_identity == WorktreeIdentity::Other;
        #[cfg(windows)]
        if let Some(guard) = unknown_ignored_write_guard {
            unknown_ignored_write_guards.push(guard);
        }
        #[cfg(windows)]
        if let Some(authority) = expected_write_authority {
            expected_write_authorities.push(authority);
        }
        paths.insert(
            path,
            RepositoryPathState {
                index_entry: None,
                worktree_identity,
            },
        );
    }
    apply_index_records(&mut paths, &mut ambiguous, &index)?;

    // A capture is usable only when its repository identity stayed stable
    // while paths and content identities were read.
    let stable_head = git_text(&root, &["rev-parse", "--verify", "HEAD"])?;
    let stable_index = git_bytes(&root, &["ls-files", "--stage", "-z"])?;
    let stable_tracked = git_bytes(&root, &["ls-files", "-z"])?;
    let stable_untracked = git_bytes(&root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    let stable_ignored = git_bytes(
        &root,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "-z",
        ],
    )?;
    let ignored_paths = nul_records(&stable_ignored).collect::<Result<BTreeSet<_>, _>>()?;
    let mut stable_file_bytes = MAX_CAPTURE_TOTAL_FILE_BYTES;
    let mut stable_worktree = true;
    for (path, state) in &paths {
        let identity = worktree_identity(
            &root,
            path,
            &mut stable_file_bytes,
            &mut remaining_work_bytes,
            ignored_paths.contains(path.as_str()),
            ignored_paths.contains(path.as_str())
                && policy
                    .expected_operational_writes
                    .iter()
                    .any(|rule| rule.matches(path)),
        );
        if !matches!(identity, Ok(identity) if identity == state.worktree_identity) {
            stable_worktree = false;
            break;
        }
    }
    if stable_head != head
        || stable_index != index
        || stable_tracked != tracked
        || stable_untracked != untracked
        || stable_ignored != ignored
        || !stable_worktree
    {
        ambiguous = true;
    }

    Ok(AttemptBaseline {
        root,
        head,
        policy,
        paths,
        ambiguous,
        #[cfg(windows)]
        _unknown_ignored_write_guards: unknown_ignored_write_guards,
        #[cfg(windows)]
        _expected_write_authorities: expected_write_authorities,
    })
}

fn inventory_paths(
    tracked: &[u8],
    untracked: &[u8],
    ignored: &[u8],
    max_paths: usize,
) -> Result<(BTreeMap<String, bool>, bool), String> {
    let mut inventory = BTreeMap::new();
    let mut case_keys = BTreeSet::new();
    let mut ambiguous = false;
    for (path_index, (raw_path, ignored_path)) in nul_records(tracked)
        .map(|path| (path, false))
        .chain(nul_records(untracked).map(|path| (path, false)))
        .chain(nul_records(ignored).map(|path| (path, true)))
        .enumerate()
    {
        if path_index >= max_paths {
            ambiguous = true;
            break;
        }
        let raw_path = raw_path?;
        let path = normalize_repo_relative_path(Path::new(&raw_path))?;
        if !case_keys.insert(path.to_lowercase()) {
            ambiguous = true;
        }
        if inventory.insert(path, ignored_path).is_some() {
            ambiguous = true;
        }
    }
    Ok((inventory, ambiguous))
}

fn apply_index_records(
    paths: &mut BTreeMap<String, RepositoryPathState>,
    ambiguous: &mut bool,
    index: &[u8],
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for (record_index, record) in nul_records(index).enumerate() {
        if record_index >= MAX_CAPTURE_PATHS {
            *ambiguous = true;
            break;
        }
        let record = record?;
        let Some((metadata, raw_path)) = record.split_once('\t') else {
            return Err(format!(
                "git ls-files emitted malformed index record `{record}`"
            ));
        };
        let path = normalize_repo_relative_path(Path::new(raw_path))?;
        let mut fields = metadata.split_ascii_whitespace();
        let mode = fields.next();
        let object = fields.next();
        let stage = fields.next();
        if mode.is_none() || object.is_none() || stage.is_none() || fields.next().is_some() {
            return Err(format!(
                "git ls-files emitted malformed stage metadata `{metadata}`"
            ));
        }
        if stage != Some("0") || !seen.insert(path.clone()) {
            *ambiguous = true;
        }
        let entry = paths.entry(path).or_insert(RepositoryPathState {
            index_entry: None,
            worktree_identity: WorktreeIdentity::Missing,
        });
        if mode == Some("120000")
            && !matches!(&entry.worktree_identity, WorktreeIdentity::Symlink(_))
        {
            *ambiguous = true;
        }
        entry.index_entry = Some(metadata.to_string());
    }
    Ok(())
}

fn delta_from_repository_states(before: &AttemptBaseline, after: &AttemptBaseline) -> AttemptDelta {
    let mut changes = Vec::new();
    let paths = before
        .paths
        .keys()
        .chain(after.paths.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut changed_symlink = false;
    for path in paths {
        let old = before.paths.get(&path);
        let new = after.paths.get(&path);
        if old == new {
            continue;
        }
        changed_symlink |= old
            .map(|state| matches!(state.worktree_identity, WorktreeIdentity::Symlink(_)))
            .unwrap_or(false)
            || new
                .map(|state| matches!(state.worktree_identity, WorktreeIdentity::Symlink(_)))
                .unwrap_or(false);
        changes.push(match (old, new) {
            (None, Some(_)) => AttemptPathChange::added(path),
            (Some(_), None) => AttemptPathChange::deleted(path),
            (Some(_), Some(_)) => AttemptPathChange::modified(path),
            (None, None) => continue,
        });
    }
    AttemptDelta {
        comparable: before.root == after.root
            && before.head == after.head
            && !before.ambiguous
            && !after.ambiguous
            && !changed_symlink,
        changes,
    }
}

fn canonical_repository_root(root: &Path) -> Result<PathBuf, String> {
    let requested = fs::canonicalize(root)
        .map_err(|err| format!("canonicalize repository root {}: {err}", root.display()))?;
    let top = git_text(&requested, &["rev-parse", "--show-toplevel"])?;
    let top =
        fs::canonicalize(&top).map_err(|err| format!("canonicalize Git top-level {top}: {err}"))?;
    if requested != top {
        return Err(format!(
            "attempt root {} is not the exact Git top-level {}",
            requested.display(),
            top.display()
        ));
    }
    Ok(requested)
}

fn writable_path_components_are_safe(root: &Path, relative: &str) -> Result<bool, String> {
    let mut current = root.to_path_buf();
    for component in relative.split('/') {
        current.push(component);
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(true),
            Err(err) => {
                return Err(format!(
                    "inspect writable path {}: {err}",
                    current.display()
                ));
            }
        };
        if metadata.file_type().is_symlink() || is_windows_reparse_point(&metadata) {
            return Ok(false);
        }
        #[cfg(unix)]
        if metadata.is_file() {
            use std::os::unix::fs::MetadataExt as _;
            if metadata.nlink() != 1 {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

#[cfg(windows)]
fn is_windows_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(windows)]
fn writable_regular_file_authority(
    path: &Path,
) -> Result<Option<winsafe::guard::CloseHandleGuard<winsafe::HFILE>>, String> {
    use winsafe::{HFILE, co};

    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("inspect writable path {}: {err}", path.display())),
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || is_windows_reparse_point(&metadata)
    {
        return Err(format!(
            "writable path {} is not a regular non-reparse file",
            path.display()
        ));
    }
    let path = path
        .to_str()
        .ok_or_else(|| "writable Windows path is not valid UTF-8".to_string())?;
    let (handle, _) = HFILE::CreateFile(
        path,
        co::GENERIC::READ,
        Some(co::FILE_SHARE::READ | co::FILE_SHARE::WRITE),
        None,
        co::DISPOSITION::OPEN_EXISTING,
        co::FILE_ATTRIBUTE::NORMAL,
        Some(co::FILE_FLAG::OPEN_REPARSE_POINT),
        None,
        None,
    )
    .map_err(|err| format!("open writable file authority {path}: {err}"))?;
    let information = handle
        .GetFileInformationByHandle()
        .map_err(|err| format!("inspect writable file authority {path}: {err}"))?;
    if information.nNumberOfLinks != 1 {
        return Err(format!(
            "writable path {path} has {} hard links",
            information.nNumberOfLinks
        ));
    }
    Ok(Some(handle))
}

fn git_text(root: &Path, args: &[&str]) -> Result<String, String> {
    let bytes = git_bytes(root, args)?;
    String::from_utf8(bytes)
        .map(|value| value.trim().to_string())
        .map_err(|err| format!("git {args:?} emitted non-UTF-8 output: {err}"))
}

fn git_bytes(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = crate::git::run_git_output_with_deadline_and_limit(
        root,
        args,
        Duration::from_secs(10),
        4 * 1024 * 1024,
    )?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    Err(format!(
        "git {args:?} failed in {}: {}",
        root.display(),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn nul_records(bytes: &[u8]) -> impl Iterator<Item = Result<&str, String>> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .map(|record| {
            std::str::from_utf8(record)
                .map_err(|err| format!("Git path record is not UTF-8: {err}"))
        })
}

fn worktree_identity(
    root: &Path,
    relative: &str,
    remaining_file_bytes: &mut u64,
    remaining_work_bytes: &mut u64,
    ignored_path: bool,
    exact_ignored_path: bool,
) -> Result<WorktreeIdentity, String> {
    let path = root.join(relative);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(WorktreeIdentity::Missing);
        }
        Err(err) => return Err(format!("inspect {}: {err}", path.display())),
    };
    if metadata.file_type().is_symlink() {
        let target = fs::read_link(&path)
            .map_err(|err| format!("read symlink {}: {err}", path.display()))?;
        return Ok(WorktreeIdentity::Symlink(target));
    }
    if metadata.is_file() {
        if ignored_path && !exact_ignored_path {
            return bounded_ignored_file_identity(&path, &metadata, remaining_work_bytes);
        }
        if metadata.len() > MAX_CAPTURE_FILE_BYTES
            || metadata.len() > *remaining_file_bytes
            || metadata.len() > *remaining_work_bytes
        {
            return Ok(WorktreeIdentity::Other);
        }
        let file = match open_regular_file_no_follow(&path) {
            Ok(file) => file,
            Err(_) => return Ok(WorktreeIdentity::Other),
        };
        let opened_metadata = file
            .metadata()
            .map_err(|err| format!("inspect opened file {}: {err}", path.display()))?;
        if !opened_metadata.is_file()
            || opened_metadata.len() > MAX_CAPTURE_FILE_BYTES
            || !same_file_object(&metadata, &opened_metadata)
        {
            return Ok(WorktreeIdentity::Other);
        }
        let mut bytes = Vec::new();
        file.take(MAX_CAPTURE_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|err| format!("read bounded content from {}: {err}", path.display()))?;
        if bytes.len() as u64 > MAX_CAPTURE_FILE_BYTES {
            return Ok(WorktreeIdentity::Other);
        }
        *remaining_file_bytes = remaining_file_bytes.saturating_sub(bytes.len() as u64);
        *remaining_work_bytes = remaining_work_bytes.saturating_sub(bytes.len() as u64);
        let rechecked = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => return Ok(WorktreeIdentity::Other),
        };
        if !rechecked.is_file() || !same_file_object(&opened_metadata, &rechecked) {
            return Ok(WorktreeIdentity::Other);
        }
        return Ok(WorktreeIdentity::File {
            digest: format!("{:x}", Sha256::digest(bytes)),
            executable: is_executable(&opened_metadata),
        });
    }
    Ok(WorktreeIdentity::Other)
}

fn bounded_ignored_file_identity(
    path: &Path,
    initial_metadata: &fs::Metadata,
    remaining_work_bytes: &mut u64,
) -> Result<WorktreeIdentity, String> {
    let mut file = match open_regular_file_no_follow(path) {
        Ok(file) => file,
        Err(_) => return Ok(WorktreeIdentity::Other),
    };
    let opened_metadata = file
        .metadata()
        .map_err(|err| format!("inspect opened ignored file {}: {err}", path.display()))?;
    let initial_snapshot = match bounded_file_snapshot(initial_metadata) {
        Ok(snapshot) => snapshot,
        Err(_) => return Ok(WorktreeIdentity::Other),
    };
    let opened_snapshot = match bounded_file_snapshot(&opened_metadata) {
        Ok(snapshot) => snapshot,
        Err(_) => return Ok(WorktreeIdentity::Other),
    };
    if !opened_metadata.is_file()
        || !same_file_object(initial_metadata, &opened_metadata)
        || opened_snapshot != initial_snapshot
    {
        return Ok(WorktreeIdentity::Other);
    }

    let len = opened_snapshot.len;
    let mut sample_offsets = BTreeSet::new();
    sample_offsets.insert(0);
    sample_offsets.insert(len / 4);
    sample_offsets.insert(len / 2);
    sample_offsets.insert(len.saturating_mul(3) / 4);
    sample_offsets.insert(len.saturating_sub(IGNORED_FILE_SAMPLE_BYTES));
    let sample_work = sample_offsets
        .iter()
        .map(|offset| IGNORED_FILE_SAMPLE_BYTES.min(len.saturating_sub(*offset)))
        .sum::<u64>();
    if sample_work > *remaining_work_bytes {
        return Ok(WorktreeIdentity::Other);
    }
    let mut hasher = Sha256::new();
    hasher.update(len.to_le_bytes());
    for offset in sample_offsets {
        let sample_len = IGNORED_FILE_SAMPLE_BYTES.min(len.saturating_sub(offset));
        hasher.update(offset.to_le_bytes());
        hasher.update(sample_len.to_le_bytes());
        if sample_len == 0 {
            continue;
        }
        file.seek(SeekFrom::Start(offset))
            .map_err(|err| format!("seek bounded ignored file {}: {err}", path.display()))?;
        let mut sample = vec![0_u8; sample_len as usize];
        file.read_exact(&mut sample)
            .map_err(|err| format!("read bounded ignored file {}: {err}", path.display()))?;
        hasher.update(sample);
    }
    *remaining_work_bytes = remaining_work_bytes.saturating_sub(sample_work);

    let final_handle_metadata = file
        .metadata()
        .map_err(|err| format!("reinspect opened ignored file {}: {err}", path.display()))?;
    let final_path_metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(WorktreeIdentity::Other),
    };
    let final_handle_snapshot = match bounded_file_snapshot(&final_handle_metadata) {
        Ok(snapshot) => snapshot,
        Err(_) => return Ok(WorktreeIdentity::Other),
    };
    let final_path_snapshot = match bounded_file_snapshot(&final_path_metadata) {
        Ok(snapshot) => snapshot,
        Err(_) => return Ok(WorktreeIdentity::Other),
    };
    if !final_path_metadata.is_file()
        || !same_file_object(&opened_metadata, &final_handle_metadata)
        || !same_file_object(&opened_metadata, &final_path_metadata)
        || final_handle_snapshot != opened_snapshot
        || final_path_snapshot != opened_snapshot
    {
        return Ok(WorktreeIdentity::Other);
    }

    Ok(WorktreeIdentity::IgnoredFileProbe {
        sample_digest: format!("{:x}", hasher.finalize()),
        snapshot: opened_snapshot,
    })
}

fn bounded_file_snapshot(metadata: &fs::Metadata) -> Result<BoundedFileSnapshot, String> {
    let modified = metadata
        .modified()
        .map_err(|err| format!("read file modification time: {err}"))?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| format!("file modification time predates UNIX epoch: {err}"))?;
    Ok(BoundedFileSnapshot {
        len: metadata.len(),
        modified,
        executable: is_executable(metadata),
        #[cfg(unix)]
        change_seconds: {
            use std::os::unix::fs::MetadataExt as _;
            metadata.ctime()
        },
        #[cfg(unix)]
        change_nanoseconds: {
            use std::os::unix::fs::MetadataExt as _;
            metadata.ctime_nsec()
        },
        #[cfg(windows)]
        creation_time: {
            use std::os::windows::fs::MetadataExt as _;
            metadata.creation_time()
        },
        #[cfg(windows)]
        file_attributes: {
            use std::os::windows::fs::MetadataExt as _;
            metadata.file_attributes()
        },
    })
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    false
}

fn open_regular_file_no_follow(path: &Path) -> Result<fs::File, std::io::Error> {
    open_regular_file_no_follow_with_write_sharing(path, true)
}

fn open_regular_file_no_follow_with_write_sharing(
    path: &Path,
    share_write: bool,
) -> Result<fs::File, std::io::Error> {
    #[cfg(not(windows))]
    let _ = share_write;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        // Linux O_NOFOLLOW | O_NONBLOCK. The nonblocking bit prevents FIFO or
        // device replacement from stalling capture before handle validation.
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
        // followed; opened-handle metadata below then rejects the reparse point.
        // Sharing read/write but not delete prevents rename/replacement while
        // the capture handle is alive.
        let share_mode = if share_write {
            0x0000_0001 | 0x0000_0002
        } else {
            0x0000_0001
        };
        options.custom_flags(0x0020_0000).share_mode(share_mode);
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
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "safe no-follow capture is unsupported on this target",
        ));
    }
    options.open(path)
}

#[cfg(unix)]
fn same_file_object(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(windows)]
fn same_file_object(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    // Stable Rust does not expose Windows file IDs. OPEN_REPARSE_POINT keeps
    // link replacement from being followed; compare the strongest stable
    // handle/path snapshot fields before and after the bounded read.
    left.file_attributes() == right.file_attributes()
        && left.creation_time() == right.creation_time()
        && left.last_write_time() == right.last_write_time()
        && left.file_size() == right.file_size()
}

#[cfg(not(any(unix, windows)))]
fn same_file_object(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CagePathScope {
    Exact,
    Subtree,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CagePathRuleWire")]
pub(crate) struct CagePathRule {
    path: String,
    scope: CagePathScope,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CagePathRuleWire {
    path: String,
    scope: CagePathScope,
}

impl TryFrom<CagePathRuleWire> for CagePathRule {
    type Error = String;

    fn try_from(wire: CagePathRuleWire) -> Result<Self, Self::Error> {
        Self::new(Path::new(&wire.path), wire.scope)
    }
}

impl CagePathRule {
    pub(crate) fn exact(path: impl AsRef<Path>) -> Result<Self, String> {
        Self::new(path.as_ref(), CagePathScope::Exact)
    }

    pub(crate) fn subtree(path: impl AsRef<Path>) -> Result<Self, String> {
        Self::new(path.as_ref(), CagePathScope::Subtree)
    }

    fn new(path: &Path, scope: CagePathScope) -> Result<Self, String> {
        Ok(Self {
            path: normalize_repo_relative_path(path)?,
            scope,
        })
    }

    /// The normalized repository-relative rule path, for consumers that
    /// restate the retained edit surface in binding records.
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    fn matches(&self, candidate: &str) -> bool {
        match self.scope {
            CagePathScope::Exact => candidate == self.path,
            CagePathScope::Subtree => {
                candidate == self.path
                    || candidate
                        .strip_prefix(&self.path)
                        .is_some_and(|tail| tail.starts_with('/'))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct EditCagePolicy {
    pub(crate) selected_target: CagePathRule,
    pub(crate) allowed_edit_surface: Vec<CagePathRule>,
    pub(crate) forbidden_paths: Vec<CagePathRule>,
    /// Command-declared generated or receipt writes that may occur alongside
    /// the authored test edit. They never satisfy `selected_target` movement.
    pub(crate) expected_operational_writes: Vec<CagePathRule>,
}

impl EditCagePolicy {
    pub(crate) fn allows_path(&self, path: &str) -> bool {
        self.allowed_edit_surface
            .iter()
            .any(|rule| rule.matches(path))
            || self
                .expected_operational_writes
                .iter()
                .any(|rule| rule.matches(path))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EditCageVerdictStatus {
    Compliant,
    Violated,
    Incomparable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EditCageViolationKind {
    InvalidOrEscapingPath,
    InvalidDelta,
    InvalidPolicy,
    ForbiddenPath,
    OutsideAllowedSurface,
    SelectedTargetNotChanged,
    UnexpectedDeletionOrRename,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub(crate) struct EditCageViolation {
    pub(crate) kind: EditCageViolationKind,
    pub(crate) path: String,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct EditCageVerdict {
    pub(crate) status: EditCageVerdictStatus,
    pub(crate) changed_paths: Vec<String>,
    pub(crate) violations: Vec<EditCageViolation>,
}

pub(crate) fn evaluate_edit_cage(policy: &EditCagePolicy, delta: &AttemptDelta) -> EditCageVerdict {
    let delta_violations = validate_delta(delta);
    if !delta.comparable || !delta_violations.is_empty() {
        let (changed_paths, mut violations) = normalized_audit_paths(delta);
        violations.extend(delta_violations);
        violations.extend(validate_policy(policy));
        violations.sort();
        violations.dedup();
        return EditCageVerdict {
            status: EditCageVerdictStatus::Incomparable,
            changed_paths,
            violations,
        };
    }

    let policy_violations = validate_policy(policy);
    if !policy_violations.is_empty() {
        let (changed_paths, mut violations) = normalized_audit_paths(delta);
        violations.extend(policy_violations);
        violations.sort();
        violations.dedup();
        return EditCageVerdict {
            status: EditCageVerdictStatus::Incomparable,
            changed_paths,
            violations,
        };
    }

    let mut changed_paths = Vec::new();
    let mut violations = Vec::new();
    let mut selected_target_changed = false;

    for change in &delta.changes {
        evaluate_change(
            policy,
            change,
            &mut selected_target_changed,
            &mut changed_paths,
            &mut violations,
        );
    }

    if !selected_target_changed {
        violations.push(EditCageViolation {
            kind: EditCageViolationKind::SelectedTargetNotChanged,
            path: policy.selected_target.path.clone(),
            reason: "the attempt did not change its selected test target".to_string(),
        });
    }

    changed_paths.sort();
    changed_paths.dedup();
    violations.sort();
    violations.dedup();

    EditCageVerdict {
        status: if violations.is_empty() {
            EditCageVerdictStatus::Compliant
        } else {
            EditCageVerdictStatus::Violated
        },
        changed_paths,
        violations,
    }
}

fn validate_delta(delta: &AttemptDelta) -> Vec<EditCageViolation> {
    delta
        .changes
        .iter()
        .filter_map(|change| {
            let shape_is_valid = matches!(
                (change.kind, change.previous_path.is_some()),
                (AttemptPathChangeKind::Renamed, true)
                    | (
                        AttemptPathChangeKind::Added
                            | AttemptPathChangeKind::Modified
                            | AttemptPathChangeKind::Deleted,
                        false
                    )
            );
            (!shape_is_valid).then(|| EditCageViolation {
                kind: EditCageViolationKind::InvalidDelta,
                path: change.path.to_string_lossy().to_string(),
                reason: if change.kind == AttemptPathChangeKind::Renamed {
                    "a renamed path change requires previous_path".to_string()
                } else {
                    format!(
                        "a {} path change must not carry previous_path",
                        path_change_kind_name(change.kind)
                    )
                },
            })
        })
        .collect()
}

fn evaluate_change(
    policy: &EditCagePolicy,
    change: &AttemptPathChange,
    selected_target_changed: &mut bool,
    changed_paths: &mut Vec<String>,
    violations: &mut Vec<EditCageViolation>,
) {
    let paths = change
        .previous_path
        .iter()
        .map(|path| {
            (
                path.as_path(),
                change.kind == AttemptPathChangeKind::Renamed,
                change.kind == AttemptPathChangeKind::Renamed,
            )
        })
        .chain(std::iter::once((
            change.path.as_path(),
            change.kind == AttemptPathChangeKind::Deleted,
            change.kind != AttemptPathChangeKind::Renamed,
        )));
    for (raw_path, is_removed_side, is_target_movement) in paths {
        let path = match normalize_repo_relative_path(raw_path) {
            Ok(path) => path,
            Err(reason) => {
                violations.push(EditCageViolation {
                    kind: EditCageViolationKind::InvalidOrEscapingPath,
                    path: raw_path.to_string_lossy().to_string(),
                    reason,
                });
                continue;
            }
        };
        changed_paths.push(path.clone());
        evaluate_normalized_path(
            policy,
            path,
            is_removed_side,
            is_target_movement,
            selected_target_changed,
            violations,
        );
    }
}

fn evaluate_normalized_path(
    policy: &EditCagePolicy,
    path: String,
    is_removed_side: bool,
    is_target_movement: bool,
    selected_target_changed: &mut bool,
    violations: &mut Vec<EditCageViolation>,
) {
    if policy.selected_target.matches(&path) {
        *selected_target_changed |= is_target_movement;
        if is_removed_side {
            violations.push(EditCageViolation {
                kind: EditCageViolationKind::UnexpectedDeletionOrRename,
                path: path.clone(),
                reason: "the selected repair target was deleted or renamed".to_string(),
            });
        }
    }

    if policy
        .forbidden_paths
        .iter()
        .any(|rule| rule.matches(&path))
    {
        violations.push(EditCageViolation {
            kind: EditCageViolationKind::ForbiddenPath,
            path,
            reason: "the changed path matches an explicit forbidden rule".to_string(),
        });
        return;
    }

    let authored_edit_allowed = policy
        .allowed_edit_surface
        .iter()
        .any(|rule| rule.matches(&path));
    let operational_write_allowed = policy
        .expected_operational_writes
        .iter()
        .any(|rule| rule.matches(&path));
    if !authored_edit_allowed && !operational_write_allowed {
        violations.push(EditCageViolation {
            kind: EditCageViolationKind::OutsideAllowedSurface,
            path,
            reason: "the changed path is outside the allowed edit surface and expected operational writes"
                .to_string(),
        });
    }
}

fn validate_policy(policy: &EditCagePolicy) -> Vec<EditCageViolation> {
    let mut violations = Vec::new();
    let selected_path = policy.selected_target.path.clone();
    if policy.selected_target.scope != CagePathScope::Exact {
        violations.push(invalid_policy_violation(
            &selected_path,
            "the selected target must be one exact repository-relative path",
        ));
    }
    if !policy
        .allowed_edit_surface
        .iter()
        .any(|rule| rule.matches(&selected_path))
    {
        violations.push(invalid_policy_violation(
            &selected_path,
            "the selected target is outside the authored edit surface",
        ));
    }
    if policy
        .expected_operational_writes
        .iter()
        .any(|rule| rule.matches(&selected_path))
    {
        violations.push(invalid_policy_violation(
            &selected_path,
            "the selected target overlaps an expected operational write",
        ));
    }
    if policy
        .forbidden_paths
        .iter()
        .any(|rule| rule.matches(&selected_path))
    {
        violations.push(invalid_policy_violation(
            &selected_path,
            "the selected target overlaps an explicit forbidden path",
        ));
    }
    violations
}

fn invalid_policy_violation(path: &str, reason: &str) -> EditCageViolation {
    EditCageViolation {
        kind: EditCageViolationKind::InvalidPolicy,
        path: path.to_string(),
        reason: reason.to_string(),
    }
}

fn normalized_audit_paths(delta: &AttemptDelta) -> (Vec<String>, Vec<EditCageViolation>) {
    let mut changed_paths = Vec::new();
    let mut violations = Vec::new();
    for raw_path in delta.changes.iter().flat_map(|change| {
        change
            .previous_path
            .iter()
            .map(PathBuf::as_path)
            .chain(std::iter::once(change.path.as_path()))
    }) {
        match normalize_repo_relative_path(raw_path) {
            Ok(path) => changed_paths.push(path),
            Err(reason) => violations.push(EditCageViolation {
                kind: EditCageViolationKind::InvalidOrEscapingPath,
                path: raw_path.to_string_lossy().to_string(),
                reason,
            }),
        }
    }
    changed_paths.sort();
    changed_paths.dedup();
    violations.sort();
    violations.dedup();
    (changed_paths, violations)
}

fn path_change_kind_name(kind: AttemptPathChangeKind) -> &'static str {
    match kind {
        AttemptPathChangeKind::Added => "added",
        AttemptPathChangeKind::Modified => "modified",
        AttemptPathChangeKind::Deleted => "deleted",
        AttemptPathChangeKind::Renamed => "renamed",
    }
}

fn normalize_repo_relative_path(path: &Path) -> Result<String, String> {
    let raw = path
        .to_str()
        .ok_or_else(|| "path is not valid UTF-8".to_string())?
        .replace('\\', "/");
    if raw.trim().is_empty() {
        return Err("path is empty".to_string());
    }
    if raw.starts_with('/')
        || raw
            .as_bytes()
            .get(1)
            .is_some_and(|separator| *separator == b':')
    {
        return Err("path is rooted or carries a drive/UNC prefix".to_string());
    }

    let mut parts = Vec::new();
    for component in Path::new(&raw).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => {
                let part = part
                    .to_str()
                    .ok_or_else(|| "path component is not valid UTF-8".to_string())?;
                if !part.is_empty() {
                    parts.push(part.to_string());
                }
            }
            Component::ParentDir => return Err("path contains parent traversal".to_string()),
            Component::RootDir | Component::Prefix(_) => {
                return Err("path is not repository-relative".to_string());
            }
        }
    }
    if parts.is_empty() {
        return Err("path contains no repository-relative component".to_string());
    }
    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct GitFixture {
        root: PathBuf,
    }

    struct ExternalFixture {
        root: PathBuf,
    }

    impl Drop for GitFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    impl Drop for ExternalFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn git_fixture(name: &str) -> Result<GitFixture, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-edit-cage-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("tests"))
            .map_err(|err| format!("create fixture tests: {err}"))?;
        fs::create_dir_all(root.join("src")).map_err(|err| format!("create fixture src: {err}"))?;
        fs::write(root.join("tests/pricing.rs"), "fn boundary() {}\n")
            .map_err(|err| format!("write selected test: {err}"))?;
        fs::write(root.join("src/pricing.rs"), "pub fn price() {}\n")
            .map_err(|err| format!("write production file: {err}"))?;
        fs::write(root.join("Cargo.toml"), "[workspace]\n")
            .map_err(|err| format!("write config: {err}"))?;
        git_ok(&root, &["-c", "init.templateDir=", "init", "-q"])?;
        git_ok(&root, &["config", "user.email", "ripr@example.invalid"])?;
        git_ok(&root, &["config", "user.name", "RIPR Test"])?;
        git_ok(&root, &["config", "commit.gpgSign", "false"])?;
        git_ok(&root, &["config", "core.fileMode", "true"])?;
        git_ok(&root, &["config", "core.autocrlf", "false"])?;
        git_ok(&root, &["config", "core.symlinks", "true"])?;
        git_ok(&root, &["config", "core.hooksPath", ".no-hooks"])?;
        git_ok(&root, &["add", "."])?;
        git_ok(&root, &["commit", "-qm", "baseline"])?;
        Ok(GitFixture { root })
    }

    fn external_fixture(name: &str) -> Result<ExternalFixture, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-edit-cage-external-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&root)
            .map_err(|err| format!("create external fixture {}: {err}", root.display()))?;
        Ok(ExternalFixture { root })
    }

    /// Per-invocation fixture deadline. Host git slowness (Defender scans
    /// of fresh `.git` directories on Windows) routinely pushes a single
    /// `git add`/`commit` past shorter windows, so the deadline is generous
    /// and idempotent commands are retried once on a timeout.
    const FIXTURE_GIT_DEADLINE: Duration = Duration::from_secs(30);

    /// Fixture git commands whose re-execution cannot change fixture state.
    /// `commit` is deliberately absent: a timed-out commit may still have
    /// landed, so it is reconciled by checking whether HEAD exists instead
    /// of blindly re-running (a re-run would fail with "nothing to
    /// commit"), (#3597 review).
    const RETRYABLE_FIXTURE_GIT: &[&str] = &[
        "init",
        "config",
        "add",
        "checkout",
        "update-ref",
        "rev-parse",
        "status",
    ];

    fn git_ok(root: &Path, args: &[&str]) -> Result<(), String> {
        // A `commit` is reconciled against the revision it started from: a
        // timed-out commit that moved HEAD landed; one that left HEAD
        // unchanged did not and must not be re-run blindly ("nothing to
        // commit" would mask the real failure).
        let head_before = if args.first() == Some(&"commit") {
            current_head(root)
        } else {
            None
        };

        let first = match crate::git::run_git_output_with_deadline_and_limit_isolated(
            root,
            args,
            FIXTURE_GIT_DEADLINE,
            4 * 1024 * 1024,
        ) {
            Ok(output) if output.status.success() => return Ok(()),
            Ok(output) => {
                return Err(format!(
                    "isolated fixture git {args:?} failed in {}: {}",
                    root.display(),
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            // The timeout is the retryable/reconcilable outcome; every other
            // runner error propagates untouched.
            Err(error) if crate::git::is_git_invocation_timeout(&error) => error,
            Err(error) => return Err(error),
        };

        // Reconcile a timed-out commit: HEAD moved past the pre-commit
        // revision, so the commit landed despite the deadline.
        if args.first() == Some(&"commit") {
            let head_after = current_head(root);
            let landed = head_after.is_some() && head_after != head_before;
            if landed {
                return Ok(());
            }
            return Err(first);
        }

        // Only provably idempotent commands are re-executed.
        if !args
            .first()
            .is_some_and(|command| RETRYABLE_FIXTURE_GIT.contains(command))
        {
            return Err(first);
        }
        match crate::git::run_git_output_with_deadline_and_limit_isolated(
            root,
            args,
            FIXTURE_GIT_DEADLINE,
            4 * 1024 * 1024,
        ) {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => Err(format!(
                "isolated fixture git {args:?} failed again in {}: {}",
                root.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            )),
            Err(error) if crate::git::is_git_invocation_timeout(&error) => {
                Err(format!("retry timed out: {error}"))
            }
            Err(error) => Err(error),
        }
    }

    /// Current HEAD revision of the fixture repository, or `None` when the
    /// repository has no commits (fresh `git init`).
    fn current_head(root: &Path) -> Option<String> {
        let output = crate::git::run_git_output_with_deadline_and_limit_isolated(
            root,
            &["rev-parse", "-q", "--verify", "HEAD"],
            FIXTURE_GIT_DEADLINE,
            4 * 1024 * 1024,
        )
        .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn policy() -> Result<EditCagePolicy, String> {
        Ok(EditCagePolicy {
            selected_target: CagePathRule::exact("tests/pricing.rs")?,
            allowed_edit_surface: vec![CagePathRule::exact("tests/pricing.rs")?],
            forbidden_paths: vec![
                CagePathRule::subtree("src")?,
                CagePathRule::exact("Cargo.toml")?,
            ],
            expected_operational_writes: vec![CagePathRule::subtree("target/ripr")?],
        })
    }

    fn commit_target_ignore(fixture: &GitFixture) -> Result<(), String> {
        fs::write(fixture.root.join(".gitignore"), "target/\n")
            .map_err(|err| format!("write target ignore: {err}"))?;
        git_ok(&fixture.root, &["add", ".gitignore"])?;
        git_ok(&fixture.root, &["commit", "-qm", "ignore target"])
    }

    fn create_oversized_ignored_file(fixture: &GitFixture, relative: &str) -> Result<(), String> {
        let path = fixture.root.join(relative);
        let parent = path
            .parent()
            .ok_or_else(|| format!("ignored fixture path has no parent: {}", path.display()))?;
        fs::create_dir_all(parent)
            .map_err(|err| format!("create ignored fixture parent {}: {err}", parent.display()))?;
        let file = fs::File::create(&path)
            .map_err(|err| format!("create ignored fixture {}: {err}", path.display()))?;
        file.set_len(MAX_CAPTURE_FILE_BYTES + 1)
            .map_err(|err| format!("size ignored fixture {}: {err}", path.display()))
    }

    fn mutate_oversized_file(fixture: &GitFixture, relative: &str) -> Result<(), String> {
        use std::io::Write as _;

        let path = fixture.root.join(relative);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|err| format!("open ignored fixture {}: {err}", path.display()))?;
        // Stay between the bounded probe offsets so this helper can challenge
        // any accidental claim that sparse samples establish content identity.
        file.seek(SeekFrom::Start(1024 * 1024))
            .map_err(|err| format!("seek ignored fixture {}: {err}", path.display()))?;
        file.write_all(b"attempt-created movement")
            .map_err(|err| format!("mutate ignored fixture {}: {err}", path.display()))
    }

    fn require_invalid_json<T: DeserializeOwned>(raw: &str) -> Result<(), String> {
        match serde_json::from_str::<T>(raw) {
            Ok(_) => Err(format!("expected invalid JSON input to be rejected: {raw}")),
            Err(_) => Ok(()),
        }
    }

    #[test]
    fn repository_delta_attributes_only_movement_after_the_exact_baseline() -> Result<(), String> {
        for staged in [false, true] {
            let fixture = git_fixture(if staged { "staged" } else { "unstaged" })?;
            fs::write(
                fixture.root.join("src/pricing.rs"),
                "pub fn price() { /* pre-existing user edit */ }\n",
            )
            .map_err(|err| format!("write pre-existing dirty source: {err}"))?;
            fs::write(
                fixture.root.join("tests/pricing.rs"),
                "fn boundary() { /* pre-existing selected-test edit */ }\n",
            )
            .map_err(|err| format!("write pre-existing dirty selected test: {err}"))?;
            let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
            fs::write(
                fixture.root.join("tests/pricing.rs"),
                "fn boundary() { assert_eq!(2 + 2, 4); }\n",
            )
            .map_err(|err| format!("write selected test edit: {err}"))?;
            if staged {
                git_ok(&fixture.root, &["add", "tests/pricing.rs"])?;
            }

            let verdict = evaluate_repository_edit_cage(&baseline)?;
            assert_eq!(verdict.status, EditCageVerdictStatus::Compliant);
            assert_eq!(verdict.changed_paths, vec!["tests/pricing.rs".to_string()]);
        }
        Ok(())
    }

    #[test]
    fn repository_delta_allows_expected_ignored_output_but_not_as_target_movement()
    -> Result<(), String> {
        let fixture = git_fixture("ignored-output")?;
        fs::write(fixture.root.join(".gitignore"), "target/\n")
            .map_err(|err| format!("write gitignore: {err}"))?;
        git_ok(&fixture.root, &["add", ".gitignore"])?;
        git_ok(&fixture.root, &["commit", "-qm", "ignore target"])?;
        fs::create_dir_all(fixture.root.join("target/ripr/reports"))
            .map_err(|err| format!("create ignored reports: {err}"))?;
        fs::write(
            fixture.root.join("target/ripr/reports/agent-receipt.json"),
            "before\n",
        )
        .map_err(|err| format!("write baseline ignored output: {err}"))?;
        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn repaired() {}\n")
            .map_err(|err| format!("write selected test: {err}"))?;
        fs::write(
            fixture.root.join("target/ripr/reports/agent-receipt.json"),
            "after\n",
        )
        .map_err(|err| format!("write changed ignored output: {err}"))?;

        let verdict = evaluate_repository_edit_cage(&baseline)?;
        assert_eq!(verdict.status, EditCageVerdictStatus::Compliant);
        assert_eq!(
            verdict.changed_paths,
            vec![
                "target/ripr/reports/agent-receipt.json".to_string(),
                "tests/pricing.rs".to_string()
            ]
        );
        Ok(())
    }

    #[test]
    fn unchanged_oversized_ignored_cache_does_not_poison_selected_test_edit() -> Result<(), String>
    {
        let fixture = git_fixture("ignored-oversized-unchanged")?;
        commit_target_ignore(&fixture)?;
        create_oversized_ignored_file(&fixture, "target/cache/preexisting.bin")?;

        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn repaired() {}\n")
            .map_err(|err| format!("write selected test: {err}"))?;

        let verdict = evaluate_repository_edit_cage(&baseline)?;
        assert_eq!(verdict.status, EditCageVerdictStatus::Compliant);
        assert_eq!(verdict.changed_paths, vec!["tests/pricing.rs".to_string()]);
        Ok(())
    }

    #[test]
    fn oversized_ignored_cache_does_not_hide_source_or_config_edits() -> Result<(), String> {
        let fixture = git_fixture("ignored-oversized-forbidden")?;
        commit_target_ignore(&fixture)?;
        create_oversized_ignored_file(&fixture, "target/cache/preexisting.bin")?;

        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn repaired() {}\n")
            .map_err(|err| format!("write selected test: {err}"))?;
        fs::write(fixture.root.join("src/pricing.rs"), "pub fn changed() {}\n")
            .map_err(|err| format!("write forbidden source: {err}"))?;
        fs::write(fixture.root.join("Cargo.toml"), "[workspace]\nmembers=[]\n")
            .map_err(|err| format!("write forbidden config: {err}"))?;

        let verdict = evaluate_repository_edit_cage(&baseline)?;
        assert_eq!(verdict.status, EditCageVerdictStatus::Violated);
        assert_eq!(
            verdict.changed_paths,
            vec![
                "Cargo.toml".to_string(),
                "src/pricing.rs".to_string(),
                "tests/pricing.rs".to_string()
            ]
        );
        assert_eq!(
            verdict
                .violations
                .iter()
                .filter(|violation| violation.kind == EditCageViolationKind::ForbiddenPath)
                .count(),
            2
        );
        Ok(())
    }

    #[test]
    fn changed_oversized_unknown_ignored_output_fails_closed() -> Result<(), String> {
        let fixture = git_fixture("ignored-oversized-unknown-change")?;
        commit_target_ignore(&fixture)?;
        create_oversized_ignored_file(&fixture, "target/cache/unknown.bin")?;

        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn repaired() {}\n")
            .map_err(|err| format!("write selected test: {err}"))?;

        #[cfg(windows)]
        {
            let path = fixture.root.join("target/cache/unknown.bin");
            let before_metadata = fs::metadata(&path)
                .map_err(|err| format!("read ignored fixture timestamp: {err}"))?;
            let before_modified = before_metadata
                .modified()
                .map_err(|err| format!("read ignored fixture timestamp: {err}"))?;
            let mut before_work = MAX_CAPTURE_TOTAL_WORK_BYTES;
            let before_probe =
                bounded_ignored_file_identity(&path, &before_metadata, &mut before_work)?;
            let before_full = Sha256::digest(
                fs::read(&path).map_err(|err| format!("read ignored fixture before: {err}"))?,
            );
            let write = fs::OpenOptions::new().write(true).open(&path);
            let error = write.err().ok_or_else(|| {
                "unknown ignored file remained writable during attempt".to_string()
            })?;
            if error.kind() != std::io::ErrorKind::PermissionDenied
                && error.raw_os_error() != Some(32)
            {
                return Err(format!(
                    "unknown ignored write failed with unexpected error: {error}"
                ));
            }

            drop(baseline);
            mutate_oversized_file(&fixture, "target/cache/unknown.bin")?;
            let file = fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .map_err(|err| format!("reopen ignored fixture after guard drop: {err}"))?;
            file.set_times(fs::FileTimes::new().set_modified(before_modified))
                .map_err(|err| format!("restore ignored fixture timestamp: {err}"))?;
            let after_metadata =
                fs::metadata(&path).map_err(|err| format!("reinspect ignored fixture: {err}"))?;
            let mut after_work = MAX_CAPTURE_TOTAL_WORK_BYTES;
            let after_probe =
                bounded_ignored_file_identity(&path, &after_metadata, &mut after_work)?;
            let after_full = Sha256::digest(
                fs::read(&path).map_err(|err| format!("read ignored fixture after: {err}"))?,
            );
            assert_eq!(before_probe, after_probe);
            assert_ne!(before_full, after_full);
            Ok(())
        }

        #[cfg(not(windows))]
        {
            mutate_oversized_file(&fixture, "target/cache/unknown.bin")?;
            let verdict = evaluate_repository_edit_cage(&baseline)?;
            assert_eq!(verdict.status, EditCageVerdictStatus::Violated);
            assert_eq!(
                verdict.changed_paths,
                vec![
                    "target/cache/unknown.bin".to_string(),
                    "tests/pricing.rs".to_string()
                ]
            );
            assert!(verdict.violations.iter().any(|violation| {
                violation.kind == EditCageViolationKind::OutsideAllowedSurface
                    && violation.path == "target/cache/unknown.bin"
            }));
            Ok(())
        }
    }

    #[test]
    fn ignored_inventory_path_limit_fails_closed_for_ignored_entries() -> Result<(), String> {
        let ignored = b"target/a\0target/b\0target/c\0";
        let (inventory, ambiguous) = inventory_paths(&[], &[], ignored, 2)?;
        assert!(ambiguous);
        assert_eq!(inventory.len(), 2);
        assert!(inventory.values().all(|ignored_path| *ignored_path));
        Ok(())
    }

    #[test]
    fn ignored_sample_work_limit_fails_closed_before_sampling() -> Result<(), String> {
        let fixture = git_fixture("ignored-sample-work-budget")?;
        commit_target_ignore(&fixture)?;
        create_oversized_ignored_file(&fixture, "target/cache/preexisting.bin")?;
        let path = fixture.root.join("target/cache/preexisting.bin");
        let metadata =
            fs::symlink_metadata(&path).map_err(|err| format!("inspect ignored fixture: {err}"))?;
        let mut remaining_work = 1;
        let identity = bounded_ignored_file_identity(&path, &metadata, &mut remaining_work)?;
        assert_eq!(identity, WorktreeIdentity::Other);
        assert_eq!(remaining_work, 1);
        Ok(())
    }

    #[test]
    fn ignored_sample_work_budget_is_cumulative_across_stability_passes() -> Result<(), String> {
        let fixture = git_fixture("ignored-cumulative-sample-work-budget")?;
        commit_target_ignore(&fixture)?;
        create_oversized_ignored_file(&fixture, "target/cache/preexisting.bin")?;
        let path = fixture.root.join("target/cache/preexisting.bin");
        let metadata =
            fs::symlink_metadata(&path).map_err(|err| format!("inspect ignored fixture: {err}"))?;

        let mut measuring_budget = u64::MAX;
        let expected = bounded_ignored_file_identity(&path, &metadata, &mut measuring_budget)?;
        assert_ne!(expected, WorktreeIdentity::Other);
        let one_pass_work = u64::MAX - measuring_budget;
        assert!(one_pass_work > 0);

        let mut cumulative_budget = one_pass_work * 2 - 1;
        assert_eq!(
            bounded_ignored_file_identity(&path, &metadata, &mut cumulative_budget)?,
            expected
        );
        assert_eq!(cumulative_budget, one_pass_work - 1);
        assert_eq!(
            bounded_ignored_file_identity(&path, &metadata, &mut cumulative_budget)?,
            WorktreeIdentity::Other
        );
        assert_eq!(cumulative_budget, one_pass_work - 1);
        Ok(())
    }

    #[test]
    fn changed_expected_output_uses_exact_identity_and_remains_compliant() -> Result<(), String> {
        let fixture = git_fixture("ignored-exact-expected-change")?;
        commit_target_ignore(&fixture)?;
        let path = fixture.root.join("target/ripr/cache.bin");
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| "missing cache parent".to_string())?,
        )
        .map_err(|err| format!("create expected cache parent: {err}"))?;
        let file =
            fs::File::create(&path).map_err(|err| format!("create expected cache: {err}"))?;
        // The 1 MiB mutation below sits between the five legacy sparse probe
        // offsets for this exact 16 MiB file.
        file.set_len(MAX_CAPTURE_FILE_BYTES)
            .map_err(|err| format!("size expected cache: {err}"))?;
        let original_modified = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .map_err(|err| format!("read expected cache timestamp: {err}"))?;

        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn repaired() {}\n")
            .map_err(|err| format!("write selected test: {err}"))?;
        mutate_oversized_file(&fixture, "target/ripr/cache.bin")?;
        let file = fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|err| format!("reopen expected cache: {err}"))?;
        file.set_times(fs::FileTimes::new().set_modified(original_modified))
            .map_err(|err| format!("restore expected cache timestamp: {err}"))?;

        let verdict = evaluate_repository_edit_cage(&baseline)?;
        assert_eq!(verdict.status, EditCageVerdictStatus::Compliant);
        assert_eq!(
            verdict.changed_paths,
            vec![
                "target/ripr/cache.bin".to_string(),
                "tests/pricing.rs".to_string()
            ]
        );
        Ok(())
    }

    #[test]
    fn oversized_expected_output_is_incomparable_without_exact_identity() -> Result<(), String> {
        let fixture = git_fixture("ignored-oversized-expected-incomparable")?;
        commit_target_ignore(&fixture)?;
        create_oversized_ignored_file(&fixture, "target/ripr/cache.bin")?;
        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn repaired() {}\n")
            .map_err(|err| format!("write selected test: {err}"))?;
        assert_eq!(
            evaluate_repository_edit_cage(&baseline)?.status,
            EditCageVerdictStatus::Incomparable
        );
        Ok(())
    }

    #[test]
    fn expected_write_hardlink_escape_is_incomparable() -> Result<(), String> {
        let fixture = git_fixture("expected-hardlink-escape")?;
        let external = external_fixture("expected-hardlink-escape")?;
        commit_target_ignore(&fixture)?;
        let external_file = external.root.join("outside.bin");
        fs::write(&external_file, "outside-before\n")
            .map_err(|err| format!("write external hardlink target: {err}"))?;
        let link = fixture.root.join("target/ripr/cache.bin");
        fs::create_dir_all(
            link.parent()
                .ok_or_else(|| "missing cache parent".to_string())?,
        )
        .map_err(|err| format!("create expected cache parent: {err}"))?;
        fs::hard_link(&external_file, &link)
            .map_err(|err| format!("create escaping hardlink: {err}"))?;

        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn repaired() {}\n")
            .map_err(|err| format!("write selected test: {err}"))?;
        fs::write(&link, "outside-after\n")
            .map_err(|err| format!("write through escaping hardlink: {err}"))?;
        assert_eq!(
            evaluate_repository_edit_cage(&baseline)?.status,
            EditCageVerdictStatus::Incomparable
        );
        assert_eq!(
            fs::read_to_string(&external_file)
                .map_err(|err| format!("read external hardlink target: {err}"))?,
            "outside-after\n"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unchanged_expected_write_symlink_escape_is_incomparable() -> Result<(), String> {
        use std::os::unix::fs::symlink;

        let fixture = git_fixture("expected-symlink-escape")?;
        let external = external_fixture("expected-symlink-escape")?;
        commit_target_ignore(&fixture)?;
        let external_file = external.root.join("outside.bin");
        fs::write(&external_file, "outside-before\n")
            .map_err(|err| format!("write external symlink target: {err}"))?;
        let link = fixture.root.join("target/ripr/escape");
        fs::create_dir_all(
            link.parent()
                .ok_or_else(|| "missing link parent".to_string())?,
        )
        .map_err(|err| format!("create expected link parent: {err}"))?;
        symlink(&external.root, &link)
            .map_err(|err| format!("create escaping expected symlink: {err}"))?;

        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn repaired() {}\n")
            .map_err(|err| format!("write selected test: {err}"))?;
        fs::write(link.join("outside.bin"), "outside-after\n")
            .map_err(|err| format!("write through escaping symlink: {err}"))?;
        assert_eq!(
            evaluate_repository_edit_cage(&baseline)?.status,
            EditCageVerdictStatus::Incomparable
        );
        assert_eq!(
            fs::read_to_string(&external_file)
                .map_err(|err| format!("read external symlink target: {err}"))?,
            "outside-after\n"
        );
        Ok(())
    }

    #[test]
    fn unbounded_repository_content_cannot_earn_compliance() -> Result<(), String> {
        let fixture = git_fixture("unbounded-content")?;
        let oversized = fixture.root.join("oversized.bin");
        let file = fs::File::create(&oversized)
            .map_err(|err| format!("create oversized fixture: {err}"))?;
        file.set_len(MAX_CAPTURE_FILE_BYTES + 1)
            .map_err(|err| format!("size oversized fixture: {err}"))?;
        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn repaired() {}\n")
            .map_err(|err| format!("write selected test: {err}"))?;

        assert_eq!(
            evaluate_repository_edit_cage(&baseline)?.status,
            EditCageVerdictStatus::Incomparable
        );
        Ok(())
    }

    #[test]
    fn aggregate_file_budget_fails_closed_before_reading() -> Result<(), String> {
        let fixture = git_fixture("aggregate-budget")?;
        let mut remaining = 1;
        let mut remaining_work = MAX_CAPTURE_TOTAL_WORK_BYTES;
        let identity = worktree_identity(
            &fixture.root,
            "tests/pricing.rs",
            &mut remaining,
            &mut remaining_work,
            false,
            false,
        )?;
        if identity != WorktreeIdentity::Other {
            return Err(format!(
                "file larger than remaining aggregate budget was accepted: {identity:?}"
            ));
        }
        if remaining != 1 {
            return Err("rejected file consumed aggregate budget".to_string());
        }
        Ok(())
    }

    #[test]
    fn aggregate_file_budget_restarts_for_the_stability_pass() -> Result<(), String> {
        let fixture = git_fixture("aggregate-budget-stability")?;
        fs::create_dir_all(fixture.root.join("bulk"))
            .map_err(|err| format!("create aggregate fixture directory: {err}"))?;
        for index in 0..3 {
            let path = fixture.root.join(format!("bulk/{index}.bin"));
            let file = fs::File::create(&path)
                .map_err(|err| format!("create aggregate fixture {}: {err}", path.display()))?;
            file.set_len(11 * 1024 * 1024)
                .map_err(|err| format!("size aggregate fixture {}: {err}", path.display()))?;
        }
        git_ok(&fixture.root, &["add", "bulk"])?;
        git_ok(
            &fixture.root,
            &["commit", "-qm", "add bounded tracked content"],
        )?;

        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn repaired() {}\n")
            .map_err(|err| format!("write selected test: {err}"))?;

        let verdict = evaluate_repository_edit_cage(&baseline)?;
        assert_eq!(verdict.status, EditCageVerdictStatus::Compliant);
        assert_eq!(verdict.changed_paths, vec!["tests/pricing.rs".to_string()]);
        Ok(())
    }

    #[test]
    fn repository_delta_retains_every_forbidden_attempt_edit() -> Result<(), String> {
        let fixture = git_fixture("forbidden")?;
        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn repaired() {}\n")
            .map_err(|err| format!("write selected test: {err}"))?;
        fs::write(fixture.root.join("src/pricing.rs"), "pub fn changed() {}\n")
            .map_err(|err| format!("write production edit: {err}"))?;
        fs::write(fixture.root.join("Cargo.toml"), "[workspace]\nmembers=[]\n")
            .map_err(|err| format!("write config edit: {err}"))?;

        let verdict = evaluate_repository_edit_cage(&baseline)?;
        assert_eq!(verdict.status, EditCageVerdictStatus::Violated);
        assert_eq!(
            verdict.changed_paths,
            vec![
                "Cargo.toml".to_string(),
                "src/pricing.rs".to_string(),
                "tests/pricing.rs".to_string()
            ]
        );
        assert_eq!(
            verdict
                .violations
                .iter()
                .filter(|violation| violation.kind == EditCageViolationKind::ForbiddenPath)
                .count(),
            2
        );
        Ok(())
    }

    #[test]
    fn repository_delta_fails_closed_for_head_rename_case_and_symlink_ambiguity()
    -> Result<(), String> {
        let head = git_fixture("head-movement")?;
        let head_baseline = capture_attempt_baseline(&head.root, &policy()?)?;
        fs::write(head.root.join("tests/pricing.rs"), "fn concurrent() {}\n")
            .map_err(|err| format!("write concurrent commit: {err}"))?;
        git_ok(&head.root, &["add", "tests/pricing.rs"])?;
        git_ok(&head.root, &["commit", "-qm", "concurrent head movement"])?;
        assert_eq!(
            evaluate_repository_edit_cage(&head_baseline)?.status,
            EditCageVerdictStatus::Incomparable
        );

        let renamed = git_fixture("rename")?;
        let rename_baseline = capture_attempt_baseline(&renamed.root, &policy()?)?;
        git_ok(
            &renamed.root,
            &["mv", "tests/pricing.rs", "tests/pricing_new.rs"],
        )?;
        assert_ne!(
            evaluate_repository_edit_cage(&rename_baseline)?.status,
            EditCageVerdictStatus::Compliant
        );

        let case = git_fixture("case-collision")?;
        let oid = git_text(&case.root, &["rev-parse", "HEAD:tests/pricing.rs"])?;
        git_ok(
            &case.root,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("100644,{oid},tests/Case.rs"),
            ],
        )?;
        git_ok(
            &case.root,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("100644,{oid},tests/case.rs"),
            ],
        )?;
        let case_baseline = capture_attempt_baseline(&case.root, &policy()?)?;
        fs::write(case.root.join("tests/pricing.rs"), "fn changed() {}\n")
            .map_err(|err| format!("write case fixture selected test: {err}"))?;
        assert_eq!(
            evaluate_repository_edit_cage(&case_baseline)?.status,
            EditCageVerdictStatus::Incomparable
        );

        let symlink = git_fixture("symlink")?;
        let symlink_baseline = capture_attempt_baseline(&symlink.root, &policy()?)?;
        fs::write(symlink.root.join("link-target.txt"), "../outside.rs\n")
            .map_err(|err| format!("write symlink blob source: {err}"))?;
        let link_oid = git_text(&symlink.root, &["hash-object", "-w", "link-target.txt"])?;
        git_ok(
            &symlink.root,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("120000,{link_oid},tests/pricing.rs"),
            ],
        )?;
        assert_eq!(
            evaluate_repository_edit_cage(&symlink_baseline)?.status,
            EditCageVerdictStatus::Incomparable
        );
        Ok(())
    }

    #[test]
    fn nonzero_or_duplicate_index_stages_are_explicitly_ambiguous() -> Result<(), String> {
        let oid = "0".repeat(40);
        let nonzero = format!("100644 {oid} 2\ttests/pricing.rs\0");
        let mut paths = BTreeMap::new();
        let mut ambiguous = false;
        apply_index_records(&mut paths, &mut ambiguous, nonzero.as_bytes())?;
        if !ambiguous {
            return Err("nonzero index stage was accepted as comparable".to_string());
        }

        let duplicate =
            format!("100644 {oid} 0\ttests/pricing.rs\0100644 {oid} 0\ttests/pricing.rs\0");
        paths.clear();
        ambiguous = false;
        apply_index_records(&mut paths, &mut ambiguous, duplicate.as_bytes())?;
        if !ambiguous {
            return Err("duplicate index path was accepted as comparable".to_string());
        }
        Ok(())
    }

    #[test]
    fn real_merge_conflict_index_cannot_earn_compliance() -> Result<(), String> {
        let fixture = git_fixture("index-conflict")?;
        git_ok(&fixture.root, &["checkout", "-qb", "other"])?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn other() {}\n")
            .map_err(|err| format!("write other branch: {err}"))?;
        git_ok(&fixture.root, &["commit", "-qam", "other branch"])?;
        git_ok(&fixture.root, &["checkout", "-q", "-"])?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn primary() {}\n")
            .map_err(|err| format!("write primary branch: {err}"))?;
        git_ok(&fixture.root, &["commit", "-qam", "primary branch"])?;
        let merge_result = git_bytes(&fixture.root, &["merge", "other"]);
        if merge_result.is_ok() {
            return Err("fixture merge unexpectedly avoided a conflict".to_string());
        }
        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
        if !baseline.ambiguous {
            return Err("real unmerged index was accepted as comparable".to_string());
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn no_follow_open_rejects_replacement_symlink_before_reading() -> Result<(), String> {
        use std::os::unix::fs::symlink;

        let fixture = git_fixture("no-follow-symlink")?;
        let path = fixture.root.join("replacement");
        symlink("tests/pricing.rs", &path)
            .map_err(|err| format!("create replacement symlink: {err}"))?;
        if open_regular_file_no_follow(&path).is_ok() {
            return Err("no-follow adapter opened a replacement symlink".to_string());
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn nonblocking_open_rejects_fifo_without_waiting_for_a_writer() -> Result<(), String> {
        let fixture = git_fixture("nonblocking-fifo")?;
        git_ok(
            &fixture.root,
            &[
                "-c",
                "alias.make-fifo=!mkfifo",
                "make-fifo",
                "replacement-fifo",
            ],
        )?;
        let started = std::time::Instant::now();
        let file = open_regular_file_no_follow(&fixture.root.join("replacement-fifo"))
            .map_err(|err| format!("nonblocking FIFO open failed unexpectedly: {err}"))?;
        let metadata = file
            .metadata()
            .map_err(|err| format!("inspect opened FIFO: {err}"))?;
        if metadata.is_file() {
            return Err("opened FIFO was misclassified as a regular file".to_string());
        }
        if started.elapsed() >= Duration::from_secs(2) {
            return Err("FIFO adapter open blocked waiting for a writer".to_string());
        }
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn capture_handle_denies_windows_rename_until_closed() -> Result<(), String> {
        let fixture = git_fixture("deny-replacement-share")?;
        let source = fixture.root.join("tests/pricing.rs");
        let destination = fixture.root.join("tests/renamed.rs");
        let handle = open_regular_file_no_follow(&source)
            .map_err(|err| format!("open capture handle: {err}"))?;
        if fs::rename(&source, &destination).is_ok() {
            return Err("Windows capture handle allowed pathname replacement".to_string());
        }
        drop(handle);
        fs::rename(&source, &destination)
            .map_err(|err| format!("rename should succeed after capture closes: {err}"))?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unstaged_tracked_symlink_target_change_cannot_hide_beside_allowed_edit() -> Result<(), String>
    {
        use std::os::unix::fs::symlink;

        let fixture = git_fixture("symlink-target-change")?;
        let link = fixture.root.join("tests/tracked-link");
        symlink("pricing.rs", &link)
            .map_err(|err| format!("create tracked symlink fixture: {err}"))?;
        git_ok(&fixture.root, &["add", "tests/tracked-link"])?;
        git_ok(&fixture.root, &["commit", "-qm", "track symlink"])?;
        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;

        fs::remove_file(&link).map_err(|err| format!("remove baseline symlink: {err}"))?;
        symlink("../Cargo.toml", &link)
            .map_err(|err| format!("retarget tracked symlink: {err}"))?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn repaired() {}\n")
            .map_err(|err| format!("write simultaneous selected edit: {err}"))?;

        let after = capture_attempt_baseline(&fixture.root, &policy()?)?;
        let before_link = baseline
            .paths
            .get("tests/tracked-link")
            .ok_or_else(|| "baseline omitted tracked symlink".to_string())?;
        let after_link = after
            .paths
            .get("tests/tracked-link")
            .ok_or_else(|| "after-state omitted tracked symlink".to_string())?;
        if before_link.worktree_identity == after_link.worktree_identity {
            return Err("distinct worktree symlink targets collapsed to one identity".to_string());
        }
        let verdict = evaluate_edit_cage(
            &baseline.policy,
            &delta_from_repository_states(&baseline, &after),
        );
        if verdict.status != EditCageVerdictStatus::Incomparable {
            return Err(format!(
                "retargeted tracked symlink plus an allowed edit must fail closed, got {:?}",
                verdict.status
            ));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn forbidden_chmod_cannot_hide_beside_allowed_edit() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt as _;

        let fixture = git_fixture("forbidden-chmod")?;
        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
        let production = fixture.root.join("src/pricing.rs");
        let mut permissions = fs::metadata(&production)
            .map_err(|err| format!("inspect production permissions: {err}"))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&production, permissions)
            .map_err(|err| format!("chmod production fixture: {err}"))?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn repaired() {}\n")
            .map_err(|err| format!("write simultaneous selected edit: {err}"))?;
        let verdict = evaluate_repository_edit_cage(&baseline)?;
        if verdict.status != EditCageVerdictStatus::Violated {
            return Err(format!(
                "forbidden chmod plus allowed edit must violate, got {:?}",
                verdict.status
            ));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn distinct_non_utf8_symlink_targets_remain_distinct() -> Result<(), String> {
        use std::os::unix::ffi::OsStringExt as _;
        use std::os::unix::fs::symlink;

        let fixture = git_fixture("non-utf8-symlink")?;
        let link = fixture.root.join("tests/tracked-link");
        symlink(std::ffi::OsString::from_vec(vec![b'a', 0x80]), &link)
            .map_err(|err| format!("create first non-UTF8 symlink: {err}"))?;
        git_ok(&fixture.root, &["add", "tests/tracked-link"])?;
        git_ok(&fixture.root, &["commit", "-qm", "track non-UTF8 symlink"])?;
        let baseline = capture_attempt_baseline(&fixture.root, &policy()?)?;
        fs::remove_file(&link).map_err(|err| format!("remove first symlink: {err}"))?;
        symlink(std::ffi::OsString::from_vec(vec![b'a', 0x81]), &link)
            .map_err(|err| format!("create second non-UTF8 symlink: {err}"))?;
        fs::write(fixture.root.join("tests/pricing.rs"), "fn repaired() {}\n")
            .map_err(|err| format!("write simultaneous selected edit: {err}"))?;
        let verdict = evaluate_repository_edit_cage(&baseline)?;
        if verdict.status != EditCageVerdictStatus::Incomparable {
            return Err(format!(
                "distinct non-UTF8 symlink target must fail closed, got {:?}",
                verdict.status
            ));
        }
        Ok(())
    }

    #[test]
    fn selected_test_plus_expected_receipt_write_is_compliant() -> Result<(), String> {
        let verdict = evaluate_edit_cage(
            &policy()?,
            &AttemptDelta {
                comparable: true,
                changes: vec![
                    AttemptPathChange::modified("tests\\pricing.rs"),
                    AttemptPathChange::added("target/ripr/reports/agent-receipt.json"),
                ],
            },
        );
        assert_eq!(verdict.status, EditCageVerdictStatus::Compliant);
        assert!(verdict.violations.is_empty());
        assert_eq!(
            verdict.changed_paths,
            vec![
                "target/ripr/reports/agent-receipt.json".to_string(),
                "tests/pricing.rs".to_string()
            ]
        );
        Ok(())
    }

    #[test]
    fn forbidden_production_edit_wins_over_other_rules() -> Result<(), String> {
        let verdict = evaluate_edit_cage(
            &policy()?,
            &AttemptDelta {
                comparable: true,
                changes: vec![
                    AttemptPathChange::modified("tests/pricing.rs"),
                    AttemptPathChange::modified("src/pricing.rs"),
                ],
            },
        );
        assert_eq!(verdict.status, EditCageVerdictStatus::Violated);
        assert!(verdict.violations.iter().any(|violation| {
            violation.kind == EditCageViolationKind::ForbiddenPath
                && violation.path == "src/pricing.rs"
        }));
        Ok(())
    }

    #[test]
    fn operational_write_does_not_substitute_for_the_selected_test() -> Result<(), String> {
        let verdict = evaluate_edit_cage(
            &policy()?,
            &AttemptDelta {
                comparable: true,
                changes: vec![AttemptPathChange::added(
                    "target/ripr/reports/agent-receipt.json",
                )],
            },
        );
        assert_eq!(verdict.status, EditCageVerdictStatus::Violated);
        assert!(verdict.violations.iter().any(|violation| {
            violation.kind == EditCageViolationKind::SelectedTargetNotChanged
        }));
        Ok(())
    }

    #[test]
    fn selected_target_delete_or_rename_is_not_a_compliant_repair() -> Result<(), String> {
        for change in [
            AttemptPathChange::deleted("tests/pricing.rs"),
            AttemptPathChange::renamed("tests/pricing.rs", "tests/pricing_new.rs"),
        ] {
            let verdict = evaluate_edit_cage(
                &policy()?,
                &AttemptDelta {
                    comparable: true,
                    changes: vec![change],
                },
            );
            assert_eq!(verdict.status, EditCageVerdictStatus::Violated);
            assert!(verdict.violations.iter().any(|violation| {
                violation.kind == EditCageViolationKind::UnexpectedDeletionOrRename
            }));
        }
        Ok(())
    }

    #[test]
    fn rename_into_selected_target_cannot_substitute_without_baseline_authority()
    -> Result<(), String> {
        let mut rename_policy = policy()?;
        rename_policy.allowed_edit_surface = vec![CagePathRule::subtree("tests")?];
        let verdict = evaluate_edit_cage(
            &rename_policy,
            &AttemptDelta {
                comparable: true,
                changes: vec![AttemptPathChange::renamed(
                    "tests/other.rs",
                    "tests/pricing.rs",
                )],
            },
        );

        assert_eq!(verdict.status, EditCageVerdictStatus::Violated);
        assert_eq!(
            verdict.changed_paths,
            vec!["tests/other.rs".to_string(), "tests/pricing.rs".to_string()]
        );
        assert!(verdict.violations.iter().any(|violation| {
            violation.kind == EditCageViolationKind::SelectedTargetNotChanged
        }));
        assert!(!verdict.violations.iter().any(|violation| {
            violation.kind == EditCageViolationKind::UnexpectedDeletionOrRename
                && violation.path == "tests/pricing.rs"
        }));
        Ok(())
    }

    #[test]
    fn subtree_selected_target_is_rejected_as_an_inexact_policy() -> Result<(), String> {
        let subtree_policy = EditCagePolicy {
            selected_target: CagePathRule::subtree("tests/pricing")?,
            allowed_edit_surface: vec![CagePathRule::subtree("tests/pricing")?],
            forbidden_paths: Vec::new(),
            expected_operational_writes: Vec::new(),
        };
        let verdict = evaluate_edit_cage(
            &subtree_policy,
            &AttemptDelta {
                comparable: true,
                changes: vec![AttemptPathChange::modified("tests/pricing/boundary.rs")],
            },
        );

        assert_eq!(verdict.status, EditCageVerdictStatus::Incomparable);
        assert_eq!(
            verdict.changed_paths,
            vec!["tests/pricing/boundary.rs".to_string()]
        );
        assert!(verdict.violations.iter().any(|violation| {
            violation.kind == EditCageViolationKind::InvalidPolicy
                && violation.reason.contains("one exact")
        }));
        Ok(())
    }

    #[test]
    fn selected_target_must_be_authored_and_disjoint_from_other_roles() -> Result<(), String> {
        let mut outside_authored_surface = policy()?;
        outside_authored_surface.allowed_edit_surface =
            vec![CagePathRule::exact("tests/other.rs")?];

        let mut operational_target = policy()?;
        operational_target.expected_operational_writes =
            vec![CagePathRule::exact("tests/pricing.rs")?];

        let mut forbidden_target = policy()?;
        forbidden_target
            .forbidden_paths
            .push(CagePathRule::exact("tests/pricing.rs")?);

        for invalid_policy in [
            outside_authored_surface,
            operational_target,
            forbidden_target,
        ] {
            let verdict = evaluate_edit_cage(
                &invalid_policy,
                &AttemptDelta {
                    comparable: true,
                    changes: vec![AttemptPathChange::modified("tests/pricing.rs")],
                },
            );
            assert_eq!(verdict.status, EditCageVerdictStatus::Incomparable);
            assert_eq!(verdict.changed_paths, vec!["tests/pricing.rs".to_string()]);
            assert!(
                verdict
                    .violations
                    .iter()
                    .any(|violation| violation.kind == EditCageViolationKind::InvalidPolicy)
            );
        }
        Ok(())
    }

    #[test]
    fn deserialized_path_rules_reuse_the_repository_path_validator() -> Result<(), String> {
        let normalized =
            serde_json::from_str::<CagePathRule>(r#"{"path":"tests\\pricing.rs","scope":"exact"}"#)
                .map_err(|error| error.to_string())?;
        assert_eq!(normalized.path, "tests/pricing.rs");
        assert_eq!(normalized.scope, CagePathScope::Exact);

        for invalid in [
            r#"{"path":"../tests/pricing.rs","scope":"exact"}"#,
            r#"{"path":"/tests/pricing.rs","scope":"exact"}"#,
        ] {
            require_invalid_json::<CagePathRule>(invalid)?;
        }
        let drive_path = format!("{}:\\tests\\pricing.rs", 'C');
        let drive_rule = serde_json::json!({"path": drive_path, "scope": "exact"});
        let drive_rule = serde_json::to_string(&drive_rule).map_err(|error| error.to_string())?;
        require_invalid_json::<CagePathRule>(&drive_rule)?;
        Ok(())
    }

    #[test]
    fn deserialized_path_changes_reject_incoherent_rename_shapes() -> Result<(), String> {
        let renamed = serde_json::from_str::<AttemptPathChange>(
            r#"{"path":"tests/new.rs","kind":"renamed","previous_path":"tests/old.rs"}"#,
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(
            renamed,
            AttemptPathChange::renamed("tests/old.rs", "tests/new.rs")
        );

        for invalid in [
            r#"{"path":"tests/new.rs","kind":"renamed"}"#,
            r#"{"path":"tests/new.rs","kind":"modified","previous_path":"tests/old.rs"}"#,
        ] {
            require_invalid_json::<AttemptPathChange>(invalid)?;
        }
        Ok(())
    }

    #[test]
    fn malformed_in_memory_change_shape_cannot_fabricate_target_movement() -> Result<(), String> {
        let mut authored_tests_policy = policy()?;
        authored_tests_policy.allowed_edit_surface = vec![CagePathRule::subtree("tests")?];
        let malformed_change = AttemptPathChange {
            path: PathBuf::from("tests/other.rs"),
            kind: AttemptPathChangeKind::Modified,
            previous_path: Some(PathBuf::from("tests/pricing.rs")),
        };

        let verdict = evaluate_edit_cage(
            &authored_tests_policy,
            &AttemptDelta {
                comparable: true,
                changes: vec![malformed_change],
            },
        );

        assert_eq!(verdict.status, EditCageVerdictStatus::Incomparable);
        assert_eq!(
            verdict.changed_paths,
            vec!["tests/other.rs".to_string(), "tests/pricing.rs".to_string()]
        );
        assert!(verdict.violations.iter().any(|violation| {
            violation.kind == EditCageViolationKind::InvalidDelta
                && violation.path == "tests/other.rs"
        }));
        Ok(())
    }

    #[test]
    fn parent_drive_and_unc_paths_fail_closed() -> Result<(), String> {
        let drive_path = format!("{}:\\tests\\pricing.rs", 'C');
        let unc_path = ["", "", "server", "share", "test.rs"].join("\\");
        for path in ["../tests/pricing.rs".to_string(), drive_path, unc_path] {
            let verdict = evaluate_edit_cage(
                &policy()?,
                &AttemptDelta {
                    comparable: true,
                    changes: vec![AttemptPathChange::modified(path)],
                },
            );
            assert_eq!(verdict.status, EditCageVerdictStatus::Violated);
            assert!(verdict.violations.iter().any(|violation| {
                violation.kind == EditCageViolationKind::InvalidOrEscapingPath
            }));
        }
        Ok(())
    }

    #[test]
    fn incomparable_delta_never_manufactures_compliance() -> Result<(), String> {
        let verdict = evaluate_edit_cage(
            &policy()?,
            &AttemptDelta {
                comparable: false,
                changes: vec![AttemptPathChange::modified("tests/pricing.rs")],
            },
        );
        assert_eq!(verdict.status, EditCageVerdictStatus::Incomparable);
        assert_eq!(verdict.changed_paths, vec!["tests/pricing.rs".to_string()]);
        assert!(verdict.violations.is_empty());
        Ok(())
    }
}
