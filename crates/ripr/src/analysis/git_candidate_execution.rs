//! Object-backed execution of a [`GitCandidateSubject`] (#3237 / #3277,
//! R2).
//!
//! R1 (#3276) bound the subject's identity; this module resolves that
//! identity through Git object plumbing and produces the two inputs the
//! existing analyzers need — the exact base→candidate unified diff and a
//! materialized candidate root — without consulting the worktree or the
//! index:
//!
//! - identities are validated with `git rev-parse` / `git cat-file -e`
//!   (read-only plumbing; no ref, index, or worktree mutation);
//! - the diff is `git diff-tree` between the two **trees**, preserving
//!   add/delete/rename/type-change information;
//! - the candidate root is materialized from the candidate tree alone
//!   through one streaming `git cat-file --batch` process (two git spawns
//!   total with the `ls-tree` listing, regardless of tree size; #5015)
//!   into a fresh temp directory, so every byte comes from the bound tree;
//! - any failure (missing base/candidate, unsupported object mode,
//!   traversal, materialization error) fails closed naming the exact
//!   identity — never an empty analysis.

#[cfg(test)]
pub(crate) mod staged;

use crate::domain::{
    GitCandidateBase, GitCandidateSubject, GitCandidateSubjectError as SubjectError, GitObjectId,
};
use sha2::Digest;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Bounded invocation deadline for each plumbing call.
const GIT_DEADLINE: Option<Duration> = Some(Duration::from_mins(1));

/// Upper bound on the total materialized candidate tree size. A tree whose
/// materialized bytes exceed it fails closed with a named limit instead of
/// an unbounded disk write.
const MAX_ARCHIVE_BYTES: usize = 512 * 1024 * 1024;
/// The resolved, analyzed form of one immutable subject: the derived
/// unified diff, the materialized candidate root (owned temp directory
/// — dropping the guard removes it), and the identities the diff was
/// derived from (recorded for R3 output projection).
pub(crate) fn subject_identity(resolved: &ResolvedGitCandidate) -> String {
    format!(
        "base_tree={} candidate_tree={}",
        resolved.base_tree, resolved.candidate_tree
    )
}

/// The resolved, analyzed form of one immutable subject: the derived
/// unified diff, the materialized candidate root (owned temp directory
/// — dropping the guard removes it), and the identities the diff was
/// derived from (recorded for R3 output projection).
pub(crate) struct ResolvedGitCandidate {
    pub(crate) base_tree: String,
    pub(crate) candidate_tree: String,
    pub(crate) diff: String,
    pub(crate) root: PathBuf,
    /// Removes the materialized root on drop; keep alive for the run.
    pub(crate) _cleanup: TempRootGuard,
}

pub(crate) struct TempRootGuard(PathBuf);

impl TempRootGuard {
    pub(crate) fn checked_cleanup(&self) -> std::io::Result<()> {
        remove_temp_root(&self.0)
    }

    #[cfg(test)]
    pub(crate) fn for_test(path: PathBuf) -> Self {
        Self(path)
    }

    /// Remove the root and, if that fails, report it to `sink`.
    ///
    /// `Drop` delegates here in full so the warning is assertable. Asserting
    /// it any other way is not possible: `Drop` can neither return the error
    /// nor be handed a channel, and a test that re-derives the message by
    /// calling the helpers itself proves nothing about what `Drop` wrote —
    /// deleting the write would leave such a test green.
    fn clean_up_reporting_to(&self, sink: &mut dyn std::io::Write) {
        // A cleanup failure leaves an extracted copy of the candidate tree on
        // disk. Discarding it would make an unbounded, invisible disk leak
        // indistinguishable from a clean run, so name the path an operator
        // has to remove.
        if let Err(error) = remove_temp_root(&self.0) {
            // Report fallibly, discarding the write result. `eprintln!` panics
            // when the stderr write fails (a closed descriptor, a non-blocking
            // pipe), and this runs in `Drop` — possibly while a panic is
            // already unwinding, where a second panic aborts the process.
            // Losing a warning is strictly better than turning a disk-cleanup
            // problem into an abort.
            let _ = writeln!(sink, "{}", cleanup_failure_report(&self.0, &error));
        }
    }
}

impl Drop for TempRootGuard {
    fn drop(&mut self) {
        self.clean_up_reporting_to(&mut std::io::stderr());
    }
}

/// The operator-facing text for a cleanup that could not be performed.
fn cleanup_failure_report(path: &Path, error: &std::io::Error) -> String {
    format!(
        "ripr: candidate materialization root could not be removed: {} ({error}); \
         remove it manually to reclaim the space",
        path.display()
    )
}

/// Remove one materialization root, retrying once.
///
/// One retry absorbs a transient hold (an antivirus scan or a still-closing
/// handle on Windows). A second failure is real and is returned to the caller.
fn remove_temp_root(path: &Path) -> std::io::Result<()> {
    match remove_temp_root_once(path) {
        Ok(()) => Ok(()),
        Err(_) => remove_temp_root_once(path),
    }
}

/// One removal attempt. A path that is already gone is success: what must not
/// survive is the root, not this particular call — a guard dropped after a
/// manual cleanup has nothing to report.
fn remove_temp_root_once(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

fn failed(detail: String) -> SubjectError {
    SubjectError::ExecutionFailed { detail }
}

fn git(root: &Path, args: &[&str], deadline: Option<Duration>) -> Result<String, SubjectError> {
    let named = |detail: String| SubjectError::ExecutionFailed { detail };
    let output = crate::git::run_git_output_with_deadline(root, args, deadline)
        .map_err(|error| named(error.to_string()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.lines().next().unwrap_or("unknown git error").trim();
        return Err(named(format!(
            "git {} failed with {}: {detail}",
            args.first().unwrap_or(&""),
            output.status
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Resolve the base side to one exact tree object ID, using the
/// repository's real empty-tree semantics when no base is requested.
fn resolve_base_tree(
    subject: &GitCandidateSubject,
    deadline: Option<Duration>,
) -> Result<String, SubjectError> {
    match &subject.base {
        GitCandidateBase::EmptyTree => {
            // The empty tree's object ID is fixed per hash format; ask
            // the repository which format it uses rather than writing
            // an object or passing a literal empty filename (the old
            // `hash-object -t tree ""` fallback always failed with
            // "could not open ''").
            let format = git(
                &subject.repository_root,
                &["rev-parse", "--show-object-format"],
                deadline,
            )?;
            let empty_tree_id = match format.trim() {
                "sha256" => "6ef19b41225c5369f1c104d45d8d85efa9d058d53bc6434cd0f5d23e5dc71d12",
                _ => "4b825dc642cb6eb9a060e54bf8d69288fbee4904",
            };
            git(
                &subject.repository_root,
                &[
                    "rev-parse",
                    "--verify",
                    &format!("{empty_tree_id}^{{tree}}"),
                ],
                deadline,
            )
        }
        // One-step peel: a treeish may name a commit, tag, or tree;
        // `^{tree}` resolves all three without rejecting the model's
        // own documented tree-OID shape.
        GitCandidateBase::Treeish(treeish) => git(
            &subject.repository_root,
            &[
                "rev-parse",
                "--verify",
                &format!("{}^{{tree}}", treeish.as_str()),
            ],
            deadline,
        ),
    }
}

/// Validate that the candidate names one existing tree object.
fn resolve_candidate_tree(
    subject: &GitCandidateSubject,
    deadline: Option<Duration>,
) -> Result<String, SubjectError> {
    let treeish = subject.candidate_tree.as_str();
    // One-step peel: the model documents a tree object ID; `^{tree}`
    // accepts a tree directly and still resolves commits and tags.
    git(
        &subject.repository_root,
        &["rev-parse", "--verify", &format!("{treeish}^{{tree}}")],
        deadline,
    )
}

/// Derive the base→candidate unified diff from the trees alone.
fn derive_diff(
    root: &Path,
    base: &str,
    candidate: &str,
    deadline: Option<Duration>,
) -> Result<String, SubjectError> {
    // -M preserves rename information so the pipeline's pinned
    // rename semantics (pure-rename paths produce no probes) fire for
    // subject runs too (#3296 review: without -M a pure rename became
    // delete+add and probed unchanged content).
    git(
        root,
        &[
            "diff-tree",
            "--unified=0",
            "--no-ext-diff",
            "--root",
            "-M",
            base,
            candidate,
        ],
        deadline,
    )
}

#[derive(Clone, Copy)]
enum ConfigurationCapture {
    NotRequested,
    Requested { limit: u64 },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CapturedConfiguration {
    NotRequested,
    Absent,
    Present { blob_oid: GitObjectId, text: String },
}

pub(crate) struct PreparedNamedTree {
    pub(crate) tree: GitObjectId,
    pub(crate) configuration: CapturedConfiguration,
    _root: PathBuf,
    _cleanup: Arc<TempRootGuard>,
    inventory: Arc<super::committed_source::frozen::FrozenInventory>,
}

impl PreparedNamedTree {
    #[cfg(test)]
    pub(crate) fn physical_root(&self) -> &Path {
        &self._root
    }

    /// Carry authenticated source inventory and cleanup into scoped workers.
    /// This alone does not install a source context or complete admission.
    pub(crate) fn frozen_source_authority(
        self,
        logical_root: &Path,
    ) -> std::io::Result<Arc<super::committed_source::frozen::FrozenSourceAuthority>> {
        super::committed_source::frozen::FrozenSourceAuthority::new(
            logical_root,
            &self._root,
            self.tree,
            self.configuration,
            self.inventory,
            self._cleanup,
        )
    }
}

/// Prepare one named tree and its configuration through the same object stream.
/// This does not change the analyzer's source root or freeze effective defaults.
pub(crate) fn prepare_named_tree(
    root: &Path,
    head: &str,
    deadline: Option<Duration>,
) -> Result<PreparedNamedTree, SubjectError> {
    let output = crate::git::run_git_output_with_deadline_and_limit_strict(
        root,
        &["rev-parse", "--verify", &format!("{head}^{{tree}}")],
        deadline.or(GIT_DEADLINE).unwrap_or(Duration::from_mins(1)),
        16 * 1024,
    )
    .map_err(|error| failed(format!("named-tree identity capture failed: {error}")))?;
    if !output.status.success() {
        return Err(failed("named-tree identity did not resolve".into()));
    }
    let tree = std::str::from_utf8(&output.stdout)
        .map_err(|error| failed(format!("named-tree identity is not UTF-8: {error}")))?;
    let tree = GitObjectId::parse(tree.trim()).map_err(|error| failed(error.to_string()))?;
    let (materialized, cleanup, configuration, inventory) = materialize_with_configuration(
        root,
        tree.as_str(),
        deadline,
        ConfigurationCapture::Requested {
            limit: crate::bounded_input::MAX_CLI_INPUT_BYTES,
        },
    )?;
    Ok(PreparedNamedTree {
        tree,
        configuration,
        _root: materialized,
        _cleanup: Arc::new(cleanup),
        inventory: Arc::new(
            inventory.ok_or_else(|| failed("named-tree source inventory is missing".into()))?,
        ),
    })
}

fn validate_configuration_inventory(listing: &[u8]) -> Result<(), SubjectError> {
    validate_configuration_inventory_with(listing, |_, _, _, _| Ok(()))
}

/// The existing inventory grammar, with a synchronous caller admission before
/// retaining each borrowed path in the duplicate detector. Ordinary capture
/// supplies a no-op observer and keeps its error ordering and messages.
fn validate_configuration_inventory_with<'a>(
    listing: &'a [u8],
    mut observe: impl FnMut(&'a str, &'a str, &'a str, &'a str) -> Result<(), SubjectError>,
) -> Result<(), SubjectError> {
    if !listing.is_empty() && listing.last() != Some(&0) {
        return Err(failed(
            "configuration inventory is not NUL-terminated".into(),
        ));
    }
    let mut paths = std::collections::BTreeSet::new();
    if listing.is_empty() {
        return Ok(());
    }
    for record in listing[..listing.len() - 1].split(|byte| *byte == 0) {
        if record.is_empty() {
            return Err(failed(
                "configuration inventory contains an empty record".into(),
            ));
        }
        let text = std::str::from_utf8(record)
            .map_err(|error| failed(format!("configuration inventory is not UTF-8: {error}")))?;
        let (metadata, path) = text
            .split_once('\t')
            .ok_or_else(|| failed("configuration inventory entry has no TAB".into()))?;
        let mut fields = metadata.split_whitespace();
        let (Some(mode), Some(kind), Some(object)) = (fields.next(), fields.next(), fields.next())
        else {
            return Err(failed(
                "configuration inventory metadata is malformed".into(),
            ));
        };
        if fields.next().is_some() {
            return Err(failed(
                "configuration inventory metadata is malformed".into(),
            ));
        }
        GitObjectId::parse(object)
            .map_err(|error| failed(format!("configuration inventory object ID: {error}")))?;
        if path.is_empty()
            || Path::new(path)
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err(failed("configuration inventory path is malformed".into()));
        }
        observe(mode, kind, object, path)?;
        if !paths.insert(path) {
            return Err(failed(format!(
                "configuration inventory duplicates path {path}"
            )));
        }
        if (path == "ripr.toml" && kind == "tree") || path.starts_with("ripr.toml/") {
            return Err(failed(
                "configuration inventory ripr.toml is a directory".into(),
            ));
        }
    }
    Ok(())
}

fn append_captured_configuration(
    bytes: &mut Vec<u8>,
    chunk: &[u8],
    limit: u64,
) -> Result<(), SubjectError> {
    let next = (bytes.len() as u64)
        .checked_add(chunk.len() as u64)
        .ok_or_else(|| failed("configuration capture length overflowed".into()))?;
    if next > limit {
        return Err(failed(format!(
            "configuration capture exceeds the {limit}-byte input limit"
        )));
    }
    bytes
        .try_reserve_exact(chunk.len())
        .map_err(|error| failed(format!("configuration capture reservation failed: {error}")))?;
    bytes.extend_from_slice(chunk);
    Ok(())
}

/// Materialize the candidate tree into a fresh temp directory through ONE
/// streaming `git cat-file --batch` process (#5015; before this, one
/// sequential `git cat-file` subprocess per file, each carrying the full
/// per-invocation deadline, multiplied the worst case to N × git_timeout and
/// paid a process spawn per file). Every byte still comes from the bound
/// blobs; the worktree and index are never consulted.
fn materialize(
    root: &Path,
    candidate_tree: &str,
    deadline: Option<Duration>,
) -> Result<(PathBuf, TempRootGuard), SubjectError> {
    let (root, cleanup, _, _) = materialize_with_configuration(
        root,
        candidate_tree,
        deadline,
        ConfigurationCapture::NotRequested,
    )?;
    Ok((root, cleanup))
}

fn materialize_with_configuration(
    root: &Path,
    candidate_tree: &str,
    deadline: Option<Duration>,
    capture: ConfigurationCapture,
) -> Result<
    (
        PathBuf,
        TempRootGuard,
        CapturedConfiguration,
        Option<super::committed_source::frozen::FrozenInventory>,
    ),
    SubjectError,
> {
    // Unique per invocation: concurrent runs (or racing tests) never
    // share a materialization directory, and a stale directory from a
    // crashed run can never be silently reused.
    let unique = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_nanos())
            .unwrap_or(0)
    );
    let base_dir = std::env::temp_dir().join("ripr-git-candidate").join(unique);
    let target = base_dir.join(candidate_tree);
    // Arm cleanup BEFORE any fallible work. Every `?` below returns early,
    // and until this guard exists those paths leave `base_dir` — which by
    // then holds an extracted copy of the candidate tree — on disk forever.
    // A fail-closed subject (an unsupported entry mode, a git failure, a
    // bounded-read overrun) must not cost a permanent temp directory; only
    // the success path hands the guard to the caller, who holds it for as
    // long as the materialization is in use.
    let cleanup = TempRootGuard(base_dir.clone());
    if target.exists() {
        // A stale directory from a crashed run must not be reused: its
        // bytes are unverified. Remove and re-create deterministically.
        std::fs::remove_dir_all(&target).map_err(|error| {
            failed(format!(
                "stale materialization could not be removed: {error}"
            ))
        })?;
    }
    std::fs::create_dir_all(&target)
        .map_err(|error| failed(format!("materialization dir failed: {error}")))?;
    // Materialize straight from blob identities. `git archive` honors
    // `.gitattributes` (e.g. `*.rs text eol=crlf`) even with
    // `core.autocrlf=false`, so extracted bytes could silently differ from
    // the bound blob identity (#3548 review); `ls-tree` + `cat-file --batch`
    // emit raw blob bytes only.
    // ONE overall deadline bounds the whole materialization phase, both git
    // processes included: the clock starts here, `ls-tree` runs inside
    // whatever remains, and the batch session receives only what is left
    // after listing and validation — so a slow listing followed by a slow
    // batch stream still cannot exceed one deadline.
    let budget = deadline.unwrap_or(Duration::from_mins(1));
    let budget_started = std::time::Instant::now();
    let listing = match capture {
        ConfigurationCapture::NotRequested => crate::git::run_git_output_with_deadline_and_limit(
            root,
            &["ls-tree", "-r", "-z", candidate_tree],
            budget.saturating_sub(budget_started.elapsed()),
            MAX_ARCHIVE_BYTES,
        ),
        ConfigurationCapture::Requested { .. } => {
            crate::git::run_git_output_with_deadline_and_limit_strict(
                root,
                &["ls-tree", "-r", "-t", "-z", candidate_tree],
                budget.saturating_sub(budget_started.elapsed()),
                MAX_ARCHIVE_BYTES,
            )
        }
    }
    .map_err(|error| failed(format!("git ls-tree failed: {error}")))?;
    if !listing.status.success() {
        return Err(failed(
            "git ls-tree of the candidate tree failed".to_string(),
        ));
    }
    // Validate every entry up front — non-UTF-8 paths, unsupported modes,
    // and traversal attempts fail closed with the existing named errors
    // before any byte is materialized, so a partially written tree can
    // never be mistaken for a complete one.
    if matches!(capture, ConfigurationCapture::Requested { .. }) {
        validate_configuration_inventory(&listing.stdout)?;
    }
    let mut configuration = match capture {
        ConfigurationCapture::NotRequested => CapturedConfiguration::NotRequested,
        ConfigurationCapture::Requested { .. } => CapturedConfiguration::Absent,
    };
    let mut inventory = matches!(capture, ConfigurationCapture::Requested { .. })
        .then(super::committed_source::frozen::FrozenInventory::new);
    let mut directories = Vec::new();
    let mut pending_configuration: Option<(GitObjectId, Vec<u8>)> = None;
    let mut entries: Vec<(String, String)> = Vec::new();
    // Only Requested capture retains one closed mode value per admitted blob.
    // Ordinary entries and blob requests keep their original representation.
    let mut original_modes = inventory
        .as_ref()
        .map(|_| Vec::<super::committed_source::frozen::FrozenFileMode>::new());
    for entry in listing.stdout.split(|byte| *byte == 0) {
        if entry.is_empty() {
            continue;
        }
        // Tree paths are repo-relative and must survive identity intact:
        // a non-UTF-8 path fails closed instead of lossily collapsing
        // distinct names (#3545 family).
        let text = std::str::from_utf8(entry).map_err(|_utf8_error| {
            failed(
                "candidate tree entry is not valid UTF-8; refusing lossy materialization"
                    .to_string(),
            )
        })?;
        let Some((meta, path)) = text.split_once('\t') else {
            return Err(failed(format!(
                "malformed ls-tree entry without a TAB separator: {text}"
            )));
        };
        let mut meta_parts = meta.split_whitespace();
        let mode = meta_parts.next().unwrap_or_default();
        let kind = meta_parts.next().unwrap_or_default();
        let object = meta_parts.next().unwrap_or_default();
        if matches!(capture, ConfigurationCapture::Requested { .. })
            && kind == "tree"
            && mode == "040000"
        {
            let destination = safe_join(&target, path)?;
            if let Some(inventory) = &mut inventory {
                inventory.insert_directory(Path::new(path));
            }
            directories.push(destination);
            continue;
        }
        if kind != "blob" || !(mode == "100644" || mode == "100755") {
            return Err(failed(format!(
                "unsupported tree entry mode `{mode}` (`{kind}`) for `{path}`: the candidate tree contains a non-file object ripr cannot faithfully materialize"
            )));
        }
        // Validate the join up front so a traversal attempt fails before
        // any byte is materialized.
        let _destination = safe_join(&target, path)?;
        if let Some(modes) = &mut original_modes {
            use super::committed_source::frozen::FrozenFileMode;
            let original_mode = match mode {
                "100644" => FrozenFileMode::Regular,
                "100755" => FrozenFileMode::Executable,
                _ => return Err(failed("named-tree original mode is unsupported".into())),
            };
            modes.try_reserve(1).map_err(|error| {
                failed(format!(
                    "named-tree mode inventory allocation failed: {error}"
                ))
            })?;
            modes.push(original_mode);
        }
        entries.push((path.to_string(), object.to_string()));
    }
    // Requested inventory includes empty directories; the ordinary -r listing
    // and materialization path remain unchanged.
    for directory in directories {
        std::fs::create_dir_all(directory)
            .map_err(|error| failed(format!("materialization mkdir failed: {error}")))?;
    }
    if entries.is_empty() {
        return Ok((target.clone(), cleanup, configuration, inventory));
    }
    // The batch session gets only the budget left after listing and
    // validation.
    let session_budget = budget.saturating_sub(budget_started.elapsed());
    let mut session = crate::git::CatFileBatch::spawn(root, session_budget)
        .map_err(|error| failed(format!("git cat-file --batch failed: {error}")))?;
    let mut total_bytes: u64 = 0;
    let mut chunk = vec![0_u8; 64 * 1024];
    for (entry_index, (path, object)) in entries.iter().enumerate() {
        let destination = safe_join(&target, path)?;
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| failed(format!("materialization mkdir failed: {error}")))?;
        }
        let Some(size) = session
            .request_blob(object)
            .map_err(|error| failed(format!("git cat-file blob {object} failed: {error}")))?
        else {
            return Err(failed(format!("git cat-file blob {object} is missing")));
        };
        total_bytes = total_bytes.checked_add(size).ok_or_else(|| {
            failed("candidate tree materialization byte total overflowed".to_string())
        })?;
        if total_bytes > MAX_ARCHIVE_BYTES as u64 {
            return Err(failed(format!(
                "candidate tree materialization exceeded the {MAX_ARCHIVE_BYTES}-byte total limit"
            )));
        }
        let capture_limit = match capture {
            ConfigurationCapture::Requested { limit } if path == "ripr.toml" => {
                if size > limit {
                    return Err(failed(format!(
                        "configuration capture exceeds the {limit}-byte input limit"
                    )));
                }
                pending_configuration = Some((
                    GitObjectId::parse(object).map_err(|error| failed(error.to_string()))?,
                    Vec::new(),
                ));
                Some(limit)
            }
            _ => None,
        };
        let mut file = std::fs::File::create(&destination)
            .map_err(|error| failed(format!("materialization write failed: {error}")))?;
        let mut source_hash = inventory.as_ref().map(|_| sha2::Sha256::new());
        let mut remaining = size;
        while remaining > 0 {
            let take = (remaining as usize).min(chunk.len()) as u64;
            session
                .read_blob_bytes(&mut chunk[..take as usize])
                .map_err(|error| failed(format!("git cat-file blob {object} failed: {error}")))?;
            if let Some(hash) = &mut source_hash {
                hash.update(&chunk[..take as usize]);
            }
            if let Some(limit) = capture_limit {
                let (_, bytes) = pending_configuration
                    .as_mut()
                    .ok_or_else(|| failed("configuration capture state is missing".into()))?;
                append_captured_configuration(bytes, &chunk[..take as usize], limit)?;
            }
            // The budget is enforced on every git stream read above and by
            // `finish` below. One residual limitation, unchanged from the
            // per-blob path: a single stalled OS-level `write_all` is
            // outside the deadline's reach (std file I/O has no timed
            // wait); the next stream read fails closed once the budget is
            // spent.
            file.write_all(&chunk[..take as usize])
                .map_err(|error| failed(format!("materialization write failed: {error}")))?;
            remaining -= take;
        }
        session
            .end_blob()
            .map_err(|error| failed(format!("git cat-file blob {object} failed: {error}")))?;
        if let (Some(inventory), Some(hash)) = (&mut inventory, source_hash) {
            inventory.insert_file(
                PathBuf::from(path),
                super::committed_source::frozen::FrozenFile {
                    mode: original_modes
                        .as_ref()
                        .and_then(|modes| modes.get(entry_index))
                        .copied()
                        .ok_or_else(|| failed("named-tree mode inventory is missing".into()))?,
                    blob_oid: GitObjectId::parse(object)
                        .map_err(|error| failed(error.to_string()))?,
                    size,
                    sha256: hash.finalize().into(),
                },
            );
        }
    }
    session
        .finish()
        .map_err(|error| failed(format!("git cat-file --batch failed: {error}")))?;
    if let Some((blob_oid, bytes)) = pending_configuration {
        let text = String::from_utf8(bytes)
            .map_err(|error| failed(format!("captured configuration is not UTF-8: {error}")))?;
        configuration = CapturedConfiguration::Present { blob_oid, text };
    }
    Ok((target.clone(), cleanup, configuration, inventory))
}

/// Join a tree entry path under the target, rejecting traversal.
fn safe_join(target: &Path, name: &str) -> Result<PathBuf, SubjectError> {
    let relative = Path::new(name);
    if relative.is_absolute() || name.contains("..") || name.contains('\\') || name.starts_with('/')
    {
        return Err(failed(format!(
            "candidate tree entry `{name}` escapes the materialization root"
        )));
    }
    Ok(target.join(relative))
}

/// Resolve and execute one subject: validate identities, derive the
/// diff, and materialize the candidate root.
pub(crate) fn resolve(
    subject: &GitCandidateSubject,
    git_timeout: Option<Duration>,
) -> Result<ResolvedGitCandidate, SubjectError> {
    // The caller's timeout wins; the internal default only covers
    // library callers that pass None (#3294 review: the candidate path
    // previously dropped the user's git_timeout).
    let deadline = git_timeout.or(GIT_DEADLINE);
    // Git owns the "is this a repository" decision; a hand-rolled
    // `.git`/`HEAD` check both misses GIT_DIR setups and accepts
    // lookalike directories (#3294 review).
    if git(
        &subject.repository_root,
        &["rev-parse", "--absolute-git-dir"],
        deadline,
    )
    .is_err()
    {
        return Err(failed(format!(
            "repository root `{}` does not own a Git object database",
            subject.repository_root.display()
        )));
    }
    let base_tree = resolve_base_tree(subject, deadline)?;
    let candidate_tree = resolve_candidate_tree(subject, deadline)?;
    if base_tree == candidate_tree {
        return Err(failed(
            "base tree and candidate tree are identical: no diff to analyze".to_string(),
        ));
    }
    let diff = derive_diff(
        &subject.repository_root,
        &base_tree,
        &candidate_tree,
        deadline,
    )?;
    let (root, cleanup) = materialize(&subject.repository_root, &candidate_tree, deadline)?;
    Ok(ResolvedGitCandidate {
        base_tree,
        candidate_tree,
        diff,
        root,
        _cleanup: cleanup,
    })
}

/// The candidate tree's `ripr.toml` bytes, when the tree carries one
/// (#3279 R4: a worktree ripr.toml must not configure a subject run).
/// Read from the tree object alone; the worktree file is never opened.
pub(crate) fn candidate_config_bytes(
    subject: &GitCandidateSubject,
    deadline: Option<Duration>,
) -> Result<Option<String>, SubjectError> {
    let treeish = subject.candidate_tree.as_str();
    let output = crate::git::run_git_output_with_deadline(
        &subject.repository_root,
        &["show", &format!("{treeish}:ripr.toml")],
        deadline,
    )
    .map_err(|error| SubjectError::ExecutionFailed {
        detail: format!("reading candidate ripr.toml failed: {error}"),
    })?;
    if !output.status.success() {
        // A tree without a ripr.toml uses the default config.
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{GitCandidateDiffSemantics, GitObjectId, GitTreeish};

    fn subject(
        root: &Path,
        base: GitCandidateBase,
        treeish: &str,
    ) -> Result<GitCandidateSubject, String> {
        Ok(GitCandidateSubject {
            repository_root: root.to_path_buf(),
            base,
            candidate_tree: GitObjectId::parse(treeish).map_err(|error| error.to_string())?,
            diff_semantics: GitCandidateDiffSemantics::DirectTreeToTree,
        })
    }

    fn candidate_blob(root: &Path, treeish: &str, path: &str) -> Result<Vec<u8>, String> {
        let output = crate::git::run_git_output_with_deadline(
            root,
            &["show", &format!("{treeish}:{path}")],
            GIT_DEADLINE,
        )
        .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "git show {treeish}:{path} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(output.stdout)
    }

    struct RepoGuard(PathBuf);
    impl Drop for RepoGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A real two-commit fixture repository with distinct base/candidate
    /// content, a rename, and a deletion.
    fn fixture_repo(name: &str) -> Result<(RepoGuard, String, String), String> {
        let root = std::env::temp_dir().join(format!("ripr-3277-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
        let run = |args: &[&str]| -> Result<String, String> {
            let out = crate::git::run_git_output_with_deadline(&root, args, GIT_DEADLINE)
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(format!(
                    "git {} failed: {}",
                    args[0],
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        run(&["init", "--initial-branch=main"])?;
        run(&["config", "user.email", "ripr@example.invalid"])?;
        run(&["config", "user.name", "ripr test"])?;
        std::fs::write(root.join("src/lib.rs"), "pub fn base() -> u8 { 1 }\n")
            .map_err(|e| e.to_string())?;
        std::fs::write(root.join("src/old.rs"), "pub fn gone() {}\n").map_err(|e| e.to_string())?;
        std::fs::write(root.join("ripr.toml"), "[analysis]\nmode = \"draft\"\n")
            .map_err(|e| e.to_string())?;
        run(&["add", "."])?;
        run(&["commit", "-m", "base"])?;
        let base = run(&["rev-parse", "HEAD"])?;
        std::fs::write(root.join("src/lib.rs"), "pub fn candidate() -> u8 { 2 }\n")
            .map_err(|e| e.to_string())?;
        std::fs::rename(root.join("src/old.rs"), root.join("src/renamed.rs"))
            .map_err(|e| e.to_string())?;
        std::fs::create_dir_all(root.join("tests")).map_err(|e| e.to_string())?;
        std::fs::write(root.join("tests/it.rs"), "use crate::candidate;\n")
            .map_err(|e| e.to_string())?;
        run(&["add", "."])?;
        run(&["commit", "-m", "candidate"])?;
        let candidate = run(&["rev-parse", "HEAD"])?;
        Ok((RepoGuard(root), base, candidate))
    }

    #[test]
    fn named_inventory_retains_original_modes_and_order_with_ordinary_blob_parity()
    -> Result<(), String> {
        use super::super::committed_source::frozen::{self, FrozenFileMode};
        let (guard, _, _) = fixture_repo("frozen-original-modes")?;
        let bytes = b"pub fn shared() -> u8 { 7 }\n";
        for name in ["regular.rs", "executable.rs"] {
            std::fs::write(guard.0.join(name), bytes).map_err(|error| error.to_string())?;
        }
        for args in [
            &["add", "regular.rs", "executable.rs"][..],
            &["update-index", "--chmod=-x", "regular.rs"][..],
            &["update-index", "--chmod=+x", "executable.rs"][..],
        ] {
            crate::testing::fixture_git::fixture_git_ok(&guard.0, args)?;
        }
        let tree =
            git(&guard.0, &["write-tree"], GIT_DEADLINE).map_err(|error| error.to_string())?;
        let oid = git(
            &guard.0,
            &["rev-parse", &format!("{tree}:regular.rs")],
            GIT_DEADLINE,
        )
        .map_err(|error| error.to_string())?;
        let executable_oid = git(
            &guard.0,
            &["rev-parse", &format!("{tree}:executable.rs")],
            GIT_DEADLINE,
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(
            oid, executable_oid,
            "the mode discriminator must share one blob"
        );
        let listing = crate::git::run_git_output_with_deadline(
            &guard.0,
            &["ls-tree", "-r", "-z", &tree],
            GIT_DEADLINE,
        )
        .map_err(|error| error.to_string())?;
        assert!(listing.status.success(), "actual mode listing failed");
        for (name, mode) in [("regular.rs", "100644"), ("executable.rs", "100755")] {
            let expected = format!("{mode} blob {oid}\t{name}");
            assert!(
                listing
                    .stdout
                    .split(|byte| *byte == 0)
                    .any(|row| row == expected.as_bytes()),
                "actual Git tree did not contain {expected}"
            );
        }
        let prepared =
            prepare_named_tree(&guard.0, &tree, None).map_err(|error| error.to_string())?;
        let authority = prepared
            .frozen_source_authority(&guard.0)
            .map_err(|error| error.to_string())?;
        assert_eq!(authority.head_tree().as_str(), tree);
        for (name, expected, git_mode) in [
            ("regular.rs", FrozenFileMode::Regular, "100644"),
            ("executable.rs", FrozenFileMode::Executable, "100755"),
        ] {
            let (_, file) = authority
                .inventory()
                .files()
                .find(|(path, _)| *path == Path::new(name))
                .ok_or_else(|| format!("mode inventory lost {name}"))?;
            assert_eq!(file.mode, expected, "original Git mode was lost for {name}");
            assert_eq!(file.mode.git_mode(), git_mode);
            assert_eq!(file.blob_oid.as_str(), oid);
            assert_eq!(file.size, bytes.len() as u64);
        }
        let records = |authority: &frozen::FrozenSourceAuthority| {
            authority
                .inventory()
                .files()
                .map(|(path, file)| {
                    (
                        path.to_path_buf(),
                        file.mode,
                        file.blob_oid.clone(),
                        file.size,
                        file.sha256,
                    )
                })
                .collect::<Vec<_>>()
        };
        let first = records(&authority);
        assert!(
            !first.is_empty(),
            "fixture must yield authenticated file records"
        );
        assert!(first.windows(2).all(|pair| pair[0].0 < pair[1].0));
        let directories = authority
            .inventory()
            .directories()
            .map(Path::to_path_buf)
            .collect::<Vec<_>>();
        assert!(directories.windows(2).all(|pair| pair[0] < pair[1]));
        let repeated = prepare_named_tree(&guard.0, &tree, None)
            .map_err(|error| error.to_string())?
            .frozen_source_authority(&guard.0)
            .map_err(|error| error.to_string())?;
        assert_eq!(
            records(&repeated),
            first,
            "physical roots must not enter records"
        );
        assert_eq!(
            repeated
                .inventory()
                .directories()
                .map(Path::to_path_buf)
                .collect::<Vec<_>>(),
            directories
        );
        std::fs::write(guard.0.join("regular.rs"), b"live decoy")
            .map_err(|error| error.to_string())?;
        let (ordinary, _cleanup) =
            materialize(&guard.0, &tree, None).map_err(|error| error.to_string())?;
        frozen::with_context(Some(authority.clone()), || -> Result<(), String> {
            for (path, file) in authority.inventory().files() {
                let snapshot =
                    frozen::fs::read(guard.0.join(path)).map_err(|error| error.to_string())?;
                let legacy =
                    std::fs::read(ordinary.join(path)).map_err(|error| error.to_string())?;
                assert_eq!(
                    snapshot,
                    legacy,
                    "ordinary bytes changed for {}",
                    path.display()
                );
                assert_eq!(snapshot.len() as u64, file.size);
                let digest: [u8; 32] = sha2::Sha256::digest(&snapshot).into();
                assert_eq!(digest, file.sha256);
            }
            authority.ensure_clean().map_err(|error| error.to_string())
        })?;
        Ok(())
    }

    #[test]
    fn named_tree_authority_reads_bound_blobs_and_owns_snapshot_after_preparation_drop()
    -> Result<(), String> {
        use super::super::committed_source::frozen;
        let (guard, _, candidate) = fixture_repo("frozen-source-authority")?;
        let expected = candidate_blob(&guard.0, &candidate, "src/lib.rs")?;
        let prepared =
            prepare_named_tree(&guard.0, &candidate, None).map_err(|error| error.to_string())?;
        let physical = prepared.physical_root().to_path_buf();
        let authority = prepared
            .frozen_source_authority(&guard.0)
            .map_err(|error| error.to_string())?;
        std::fs::remove_file(guard.0.join("src/lib.rs")).map_err(|error| error.to_string())?;
        std::fs::write(guard.0.join("live-only.rs"), b"untracked")
            .map_err(|error| error.to_string())?;
        assert!(
            physical.is_dir(),
            "the worker Arc must retain materialization cleanup"
        );
        frozen::with_context(Some(authority.clone()), || -> Result<(), String> {
            assert_eq!(
                frozen::fs::read(guard.0.join("src/lib.rs")).map_err(|error| error.to_string())?,
                expected
            );
            assert!(!frozen::fs::exists(guard.0.join("live-only.rs")));
            authority
                .ensure_clean()
                .map_err(|error| error.to_string())?;
            let entries =
                frozen::fs::read_dir(guard.0.join("src")).map_err(|error| error.to_string())?;
            for entry in entries {
                let entry = entry.map_err(|error| error.to_string())?;
                assert!(entry.path().starts_with(&guard.0));
                assert!(!entry.path().starts_with(&physical));
            }
            // A same-length physical replacement must fail even though its
            // metadata still matches the materialized blob.
            let mut changed = expected.clone();
            let first = changed.first_mut().ok_or("fixture source is empty")?;
            *first ^= 1;
            std::fs::write(physical.join("src/lib.rs"), &changed)
                .map_err(|error| error.to_string())?;
            let failure = frozen::fs::read(guard.0.join("src/lib.rs"))
                .err()
                .ok_or("a same-length snapshot replacement must fail")?;
            assert_eq!(failure.kind(), std::io::ErrorKind::InvalidData);
            let failure = authority
                .ensure_clean()
                .err()
                .ok_or("snapshot replacement must remain fatal")?;
            assert_eq!(failure.kind(), std::io::ErrorKind::InvalidData);
            Ok(())
        })?;
        drop(authority);
        assert!(
            !physical.exists(),
            "the last authority Arc must remove the owned tree"
        );
        Ok(())
    }

    #[test]
    fn named_configuration_is_captured_from_the_materialized_blob() -> Result<(), String> {
        let (guard, _, candidate) = fixture_repo("capture-config")?;
        let expected = candidate_blob(&guard.0, &candidate, "ripr.toml")?;
        let expected_oid = git(
            &guard.0,
            &["rev-parse", &format!("{candidate}:ripr.toml")],
            GIT_DEADLINE,
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(guard.0.join("ripr.toml"), "[analysis]\nmode = \"fast\"\n")
            .map_err(|error| error.to_string())?;
        let prepared =
            prepare_named_tree(&guard.0, &candidate, None).map_err(|error| error.to_string())?;
        let CapturedConfiguration::Present { blob_oid, text } = &prepared.configuration else {
            return Err("committed configuration was not captured".into());
        };
        assert_eq!(blob_oid.as_str(), expected_oid);
        assert_eq!(text.as_bytes(), expected);
        assert_eq!(
            std::fs::read(prepared._root.join("ripr.toml")).map_err(|error| error.to_string())?,
            expected
        );
        Ok(())
    }

    #[test]
    fn captured_empty_and_absent_configuration_remain_distinct() -> Result<(), String> {
        let (guard, _, _) = fixture_repo("capture-empty-absent")?;
        std::fs::write(guard.0.join("ripr.toml"), "").map_err(|error| error.to_string())?;
        crate::testing::fixture_git::fixture_git_ok(&guard.0, &["add", "-A"])?;
        crate::testing::fixture_git::fixture_git_ok(&guard.0, &["commit", "-qm", "empty config"])?;
        let empty =
            prepare_named_tree(&guard.0, "HEAD", None).map_err(|error| error.to_string())?;
        assert!(matches!(
            &empty.configuration,
            CapturedConfiguration::Present { text, .. } if text.is_empty()
        ));
        std::fs::remove_file(guard.0.join("ripr.toml")).map_err(|error| error.to_string())?;
        crate::testing::fixture_git::fixture_git_ok(&guard.0, &["add", "-A"])?;
        crate::testing::fixture_git::fixture_git_ok(&guard.0, &["commit", "-qm", "absent config"])?;
        let absent =
            prepare_named_tree(&guard.0, "HEAD", None).map_err(|error| error.to_string())?;
        assert_eq!(absent.configuration, CapturedConfiguration::Absent);
        assert!(!absent._root.join("ripr.toml").exists());
        Ok(())
    }

    #[test]
    fn capture_rejects_invalid_utf8_without_changing_ordinary_blob_materialization()
    -> Result<(), String> {
        let (guard, _, _) = fixture_repo("capture-invalid-config")?;
        std::fs::write(guard.0.join("ripr.toml"), [0xff_u8, 0xfe])
            .map_err(|error| error.to_string())?;
        crate::testing::fixture_git::fixture_git_ok(&guard.0, &["add", "-A"])?;
        crate::testing::fixture_git::fixture_git_ok(
            &guard.0,
            &["commit", "-qm", "invalid config"],
        )?;
        let tree = git(&guard.0, &["rev-parse", "HEAD^{tree}"], GIT_DEADLINE)
            .map_err(|error| error.to_string())?;
        let failure = prepare_named_tree(&guard.0, "HEAD", None)
            .err()
            .ok_or("invalid UTF-8 config was accepted")?;
        assert!(failure.to_string().contains("not UTF-8"), "{failure}");
        let (root, _cleanup) =
            materialize(&guard.0, &tree, None).map_err(|error| error.to_string())?;
        assert_eq!(
            std::fs::read(root.join("ripr.toml")).map_err(|error| error.to_string())?,
            [0xff_u8, 0xfe]
        );
        Ok(())
    }

    #[test]
    fn capture_refuses_a_config_directory_and_missing_config_object() -> Result<(), String> {
        let (guard, _, candidate) = fixture_repo("capture-missing-object")?;
        let oid = git(
            &guard.0,
            &["rev-parse", &format!("{candidate}:ripr.toml")],
            GIT_DEADLINE,
        )
        .map_err(|error| error.to_string())?;
        let parsed = GitObjectId::parse(&oid).map_err(|error| error.to_string())?;
        let (prefix, suffix) = parsed.as_str().split_at(2);
        std::fs::remove_file(guard.0.join(".git/objects").join(prefix).join(suffix))
            .map_err(|error| format!("remove fixture's loose config blob: {error}"))?;
        let failure = prepare_named_tree(&guard.0, &candidate, None)
            .err()
            .ok_or("missing config blob became absent")?;
        assert!(failure.to_string().contains("git cat-file"), "{failure}");

        let (directory_guard, _, _) = fixture_repo("capture-config-directory")?;
        std::fs::remove_file(directory_guard.0.join("ripr.toml"))
            .map_err(|error| error.to_string())?;
        std::fs::create_dir(directory_guard.0.join("ripr.toml"))
            .map_err(|error| error.to_string())?;
        std::fs::write(directory_guard.0.join("ripr.toml/nested"), b"alias")
            .map_err(|error| error.to_string())?;
        crate::testing::fixture_git::fixture_git_ok(&directory_guard.0, &["add", "-A"])?;
        crate::testing::fixture_git::fixture_git_ok(
            &directory_guard.0,
            &["commit", "-qm", "config directory"],
        )?;
        let failure = prepare_named_tree(&directory_guard.0, "HEAD", None)
            .err()
            .ok_or("config directory alias became absent")?;
        assert!(
            failure.to_string().contains("ripr.toml is a directory"),
            "{failure}"
        );
        let tree = git(
            &directory_guard.0,
            &["rev-parse", "HEAD^{tree}"],
            GIT_DEADLINE,
        )
        .map_err(|error| error.to_string())?;
        let (ordinary, _cleanup) =
            materialize(&directory_guard.0, &tree, None).map_err(|error| error.to_string())?;
        assert!(ordinary.join("ripr.toml/nested").is_file());
        Ok(())
    }

    #[test]
    fn frozen_inventory_keeps_an_actual_empty_nonconfiguration_subtree() -> Result<(), String> {
        use super::super::committed_source::frozen;
        let (guard, _, _) = fixture_repo("frozen-empty-directory")?;
        let empty_tree =
            git(&guard.0, &["mktree"], GIT_DEADLINE).map_err(|error| error.to_string())?;
        let empty_tree = GitObjectId::parse(&empty_tree).map_err(|error| error.to_string())?;
        let mut record = b"40000 empty\0".to_vec();
        for pair in empty_tree.as_str().as_bytes().chunks_exact(2) {
            let hex = std::str::from_utf8(pair).map_err(|error| error.to_string())?;
            record.push(u8::from_str_radix(hex, 16).map_err(|error| error.to_string())?);
        }
        let input = guard.0.join("empty-source-tree-input");
        std::fs::write(&input, record).map_err(|error| error.to_string())?;
        let input_name = input
            .to_str()
            .ok_or("fixture tree-input path is not UTF-8")?;
        let tree = git(
            &guard.0,
            &["hash-object", "-w", "-t", "tree", input_name],
            GIT_DEADLINE,
        )
        .map_err(|error| error.to_string())?;
        let prepared =
            prepare_named_tree(&guard.0, &tree, None).map_err(|error| error.to_string())?;
        let authority = prepared
            .frozen_source_authority(&guard.0)
            .map_err(|error| error.to_string())?;
        assert_eq!(authority.inventory().files().len(), 0);
        assert_eq!(
            authority
                .inventory()
                .directories()
                .map(Path::to_path_buf)
                .collect::<Vec<_>>(),
            vec![PathBuf::new(), PathBuf::from("empty")]
        );
        frozen::with_context(Some(authority.clone()), || -> Result<(), String> {
            assert!(frozen::fs::is_dir(guard.0.join("empty")));
            assert_eq!(
                frozen::fs::read_dir(guard.0.join("empty"))
                    .map_err(|error| error.to_string())?
                    .count(),
                0
            );
            authority.ensure_clean().map_err(|error| error.to_string())
        })?;
        let (ordinary, _cleanup) =
            materialize(&guard.0, &tree, None).map_err(|error| error.to_string())?;
        assert!(
            !ordinary.join("empty").exists(),
            "ordinary -r behavior remains unchanged"
        );
        Ok(())
    }

    #[test]
    fn capture_refuses_an_actual_empty_configuration_subtree() -> Result<(), String> {
        let (guard, _, _) = fixture_repo("capture-empty-config-tree")?;
        let empty_tree =
            git(&guard.0, &["mktree"], GIT_DEADLINE).map_err(|error| error.to_string())?;
        let empty_tree = GitObjectId::parse(&empty_tree).map_err(|error| error.to_string())?;
        let mut record = b"40000 ripr.toml\0".to_vec();
        for pair in empty_tree.as_str().as_bytes().chunks_exact(2) {
            let hex = std::str::from_utf8(pair).map_err(|error| error.to_string())?;
            record.push(u8::from_str_radix(hex, 16).map_err(|error| error.to_string())?);
        }
        let input_path = guard.0.join("empty-config-tree-input");
        std::fs::write(&input_path, &record).map_err(|error| error.to_string())?;
        let input_name = input_path
            .to_str()
            .ok_or("fixture tree-input path is not UTF-8")?;
        let tree = git(
            &guard.0,
            &["hash-object", "-w", "-t", "tree", input_name],
            GIT_DEADLINE,
        )
        .map_err(|error| error.to_string())?;
        let recursive = crate::git::run_git_output_with_deadline(
            &guard.0,
            &["ls-tree", "-r", "-z", &tree],
            GIT_DEADLINE,
        )
        .map_err(|error| error.to_string())?;
        assert!(recursive.status.success());
        assert!(
            recursive.stdout.is_empty(),
            "fixture is not an empty subtree"
        );
        let failure = prepare_named_tree(&guard.0, &tree, None)
            .err()
            .ok_or("empty config subtree became Absent")?;
        assert!(
            failure.to_string().contains("ripr.toml is a directory"),
            "{failure}"
        );
        let (ordinary, _cleanup) =
            materialize(&guard.0, &tree, None).map_err(|error| error.to_string())?;
        assert!(!ordinary.join("ripr.toml").exists());
        Ok(())
    }

    #[test]
    fn capture_inventory_refuses_truncation_duplicates_and_malformed_object_ids()
    -> Result<(), String> {
        let (guard, _, candidate) = fixture_repo("capture-inventory")?;
        let listing = crate::git::run_git_output_with_deadline(
            &guard.0,
            &["ls-tree", "-r", "-z", &candidate],
            GIT_DEADLINE,
        )
        .map_err(|error| error.to_string())?;
        assert!(listing.status.success());
        validate_configuration_inventory(&listing.stdout).map_err(|error| error.to_string())?;
        let truncated = &listing.stdout[..listing.stdout.len() - 1];
        assert!(validate_configuration_inventory(truncated).is_err());
        let mut duplicate = listing.stdout.clone();
        let first = listing
            .stdout
            .split(|byte| *byte == 0)
            .next()
            .ok_or("no tree record")?;
        duplicate.extend_from_slice(first);
        duplicate.push(0);
        let failure = validate_configuration_inventory(&duplicate)
            .err()
            .ok_or("duplicate path was accepted")?;
        assert!(failure.to_string().contains("duplicates path"), "{failure}");
        let malformed = b"100644 blob invalid\tripr.toml\0";
        assert!(validate_configuration_inventory(malformed).is_err());
        let oid = "a".repeat(40);
        let extra = format!("100644 blob {oid} extra\tripr.toml\0");
        assert!(validate_configuration_inventory(extra.as_bytes()).is_err());
        let empty_record = [listing.stdout.as_slice(), &[0]].concat();
        assert!(validate_configuration_inventory(&empty_record).is_err());
        Ok(())
    }

    #[test]
    fn configuration_capture_cap_refuses_before_growth() -> Result<(), String> {
        let (guard, _, candidate) = fixture_repo("capture-cap")?;
        let tree = git(
            &guard.0,
            &["rev-parse", &format!("{candidate}^{{tree}}")],
            GIT_DEADLINE,
        )
        .map_err(|error| error.to_string())?;
        let failure = materialize_with_configuration(
            &guard.0,
            &tree,
            None,
            ConfigurationCapture::Requested { limit: 8 },
        )
        .err()
        .ok_or("oversize configuration was captured")?;
        assert!(
            failure.to_string().contains("8-byte input limit"),
            "{failure}"
        );
        let mut bytes = b"12".to_vec();
        assert!(append_captured_configuration(&mut bytes, b"3", 2).is_err());
        assert_eq!(bytes, b"12");
        append_captured_configuration(&mut bytes, b"3", 3).map_err(|error| error.to_string())?;
        assert_eq!(bytes, b"123");
        assert_eq!(crate::bounded_input::MAX_CLI_INPUT_BYTES, 256 * 1024 * 1024);
        Ok(())
    }

    #[test]
    fn resolves_trees_derives_diff_and_materializes_bytes() -> Result<(), String> {
        let (guard, base, candidate) = fixture_repo("resolve")?;
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let resolved = resolve(&s, None).map_err(|e| e.to_string())?;
        assert!(resolved.diff.contains("src/lib.rs"), "{}", resolved.diff);
        assert!(resolved.diff.contains("src/renamed.rs") || resolved.diff.contains("src/old.rs"));
        // Candidate bytes, not worktree bytes:
        assert_eq!(
            std::fs::read(resolved.root.join("src/lib.rs")).map_err(|e| e.to_string())?,
            candidate_blob(&guard.0, &candidate, "src/lib.rs")?
        );
        assert!(resolved.root.join("ripr.toml").exists());
        Ok(())
    }

    #[test]
    fn worktree_mutations_cannot_change_the_analysis_input() -> Result<(), String> {
        let (guard, base, candidate) = fixture_repo("mutate")?;
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let first = resolve(&s, None).map_err(|e| e.to_string())?;
        // The three mutations from the issue's reproduction.
        std::fs::write(guard.0.join("src/lib.rs"), "pub fn dirty() -> u8 { 9 }\n")
            .map_err(|e| e.to_string())?;
        std::fs::write(guard.0.join("ripr.toml"), "[analysis]\nmode = \"deep\"\n")
            .map_err(|e| e.to_string())?;
        std::fs::write(guard.0.join("src/staged.rs"), "pub fn staged() {}\n")
            .map_err(|e| e.to_string())?;
        crate::git::run_git_output_with_deadline(&guard.0, &["add", "."], GIT_DEADLINE)
            .map_err(|e| e.to_string())?;
        let second = resolve(&s, None).map_err(|e| e.to_string())?;
        assert_eq!(
            first.diff, second.diff,
            "diff must follow objects, not the worktree"
        );
        assert_eq!(
            std::fs::read(second.root.join("src/lib.rs")).map_err(|e| e.to_string())?,
            candidate_blob(&guard.0, &candidate, "src/lib.rs")?,
            "materialized bytes must follow the candidate tree"
        );
        assert!(
            !second.root.join("src/staged.rs").exists(),
            "a blob staged after binding must not appear in the candidate"
        );
        Ok(())
    }

    // #3296 review finding 4: a pure rename stays a rename (delete+add
    // would probe unchanged content).
    #[test]
    fn rename_information_is_preserved_in_the_derived_diff() -> Result<(), String> {
        let (guard, base, candidate) = fixture_repo("rename")?;
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let resolved = resolve(&s, None).map_err(|e| e.to_string())?;
        assert!(
            resolved.diff.contains("rename from") || resolved.diff.contains("src/renamed.rs"),
            "rename must survive derivation: {}",
            resolved.diff
        );
        Ok(())
    }

    // #3296 review finding 3: paths longer than the 100-char tar name
    // field arrive as pax extended headers and must materialize.
    #[test]
    fn long_paths_materialize_through_pax_headers() -> Result<(), String> {
        let (guard, base, _fixture_candidate) = fixture_repo("longpath")?;
        let deep = format!("metrics/{}", "x".repeat(60));
        let nested = guard.0.join(&deep);
        std::fs::create_dir_all(&nested).map_err(|e| e.to_string())?;
        let long_name = "y".repeat(60);
        std::fs::write(
            nested.join(&long_name),
            "long path content
",
        )
        .map_err(|e| e.to_string())?;
        crate::git::run_git_output_with_deadline(&guard.0, &["add", "."], GIT_DEADLINE)
            .map_err(|e| e.to_string())?;
        crate::git::run_git_output_with_deadline(
            &guard.0,
            &["commit", "-m", "long path"],
            GIT_DEADLINE,
        )
        .map_err(|e| e.to_string())?;
        let candidate = String::from_utf8_lossy(
            &crate::git::run_git_output_with_deadline(
                &guard.0,
                &["rev-parse", "HEAD"],
                GIT_DEADLINE,
            )
            .map_err(|e| e.to_string())?
            .stdout,
        )
        .trim()
        .to_string();
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let resolved = resolve(&s, None).map_err(|e| e.to_string())?;
        assert!(
            resolved.root.join(&deep).join(&long_name).exists(),
            "long path must materialize from the pax header"
        );
        assert_eq!(
            std::fs::read(resolved.root.join(&deep).join(&long_name)).map_err(|e| e.to_string())?,
            candidate_blob(&guard.0, &candidate, &format!("{deep}/{long_name}"))?
        );
        Ok(())
    }

    // #3548 review: a `.gitattributes` `text eol=crlf` attribute must not
    // convert materialized bytes. `git archive` honors the attribute even
    // with core.autocrlf=false, so the blob-wise materialization is pinned
    // against the `git show <tree>:<path>` oracle bytes.
    #[test]
    fn materialization_ignores_attribute_driven_conversion() -> Result<(), String> {
        let (guard, _base, _pre_attribute_head) = fixture_repo("attributes")?;
        let run = |args: &[&str]| -> Result<String, String> {
            let out = crate::git::run_git_output_with_deadline(&guard.0, args, GIT_DEADLINE)
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(format!(
                    "git {} failed: {}",
                    args[0],
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        std::fs::write(
            guard.0.join(".gitattributes"),
            "*.rs text eol=crlf
",
        )
        .map_err(|e| e.to_string())?;
        run(&["add", ".gitattributes"])?;
        run(&["commit", "-m", "attributes"])?;
        // A fresh commit AFTER the attribute exists: the candidate tree's
        // blob is pure LF, but archive would deliver CRLF.
        std::fs::write(
            guard.0.join("src/lib.rs"),
            "pub fn attr() -> u8 { 3 }
",
        )
        .map_err(|e| e.to_string())?;
        run(&["add", "."])?;
        run(&["commit", "-m", "candidate under attribute"])?;
        let candidate = run(&["rev-parse", "HEAD"])?;
        let base = run(&["rev-parse", "HEAD~1"])?;
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let resolved = resolve(&s, None).map_err(|e| e.to_string())?;
        let materialized =
            std::fs::read(resolved.root.join("src/lib.rs")).map_err(|e| e.to_string())?;
        let oracle = candidate_blob(&guard.0, &candidate, "src/lib.rs")?;
        assert_eq!(
            materialized, oracle,
            "materialized bytes must equal the bound blob bytes under eol=crlf"
        );
        assert!(
            !materialized.contains(&b'\r'),
            "an LF blob must stay LF under an eol=crlf attribute"
        );
        Ok(())
    }

    // #3296 review blocker 1: worktree and repo modes fail closed on a
    // bound subject instead of silently analyzing the live tree.
    #[test]
    fn worktree_and_repo_modes_reject_a_bound_subject() -> Result<(), String> {
        let (guard, base, candidate) = fixture_repo("modes")?;
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let options = crate::analysis::AnalysisOptions {
            root: guard.0.clone(),
            base: None,
            diff_file: None,
            mode: crate::analysis::AnalysisMode::Draft,
            include_unchanged_tests: false,
            resolve_tsconfig_paths: false,
            perl_facts_path: None,
            git_timeout: None,
            git_candidate: Some(s.clone()),
            production_like_targets: Default::default(),
            test_harnesses: Vec::new(),
            resolved_subject_identity: None,
            open_rust_index_paths: Default::default(),
        };
        let error = crate::analysis::run_worktree_analysis_with_oracle_policy_and_rust_config(
            &options,
            &crate::config::OraclePolicy::default(),
            &[crate::analysis::language::LanguageId::Rust],
            &crate::config::RustLanguageConfig::default(),
        )
        .err()
        .ok_or("worktree mode must fail closed on a subject")?;
        assert!(
            error.to_string().contains("git candidate subject"),
            "worktree rejection must name the subject: {error}"
        );
        let repo_error = crate::analysis::run_repo_analysis_with_oracle_policy(
            &options,
            &crate::config::OraclePolicy::default(),
            &[crate::analysis::language::LanguageId::Rust],
        )
        .err()
        .ok_or("repo mode must fail closed on a subject")?;
        assert!(
            repo_error.contains("git candidate subject"),
            "repo rejection must name the subject: {repo_error}"
        );
        Ok(())
    }

    #[test]
    fn pipeline_executes_the_subject_against_candidate_bytes() -> Result<(), String> {
        let (guard, base, candidate) = fixture_repo("pipeline")?;
        // Dirty the worktree AND stage a different blob: the analysis
        // input must still follow the bound objects.
        std::fs::write(
            guard.0.join("src/lib.rs"),
            "pub fn dirty() -> u8 { 9 }
",
        )
        .map_err(|e| e.to_string())?;
        crate::git::run_git_output_with_deadline(&guard.0, &["add", "."], GIT_DEADLINE)
            .map_err(|e| e.to_string())?;
        let subject = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let options = crate::analysis::AnalysisOptions {
            root: guard.0.clone(),
            base: None,
            diff_file: None,
            mode: crate::analysis::AnalysisMode::Draft,
            include_unchanged_tests: false,
            resolve_tsconfig_paths: false,
            perl_facts_path: None,
            git_timeout: None,
            git_candidate: Some(subject),
            production_like_targets: Default::default(),
            test_harnesses: Vec::new(),
            resolved_subject_identity: None,
            open_rust_index_paths: Default::default(),
        };
        let result = crate::analysis::run_analysis_with_oracle_policy(
            &options,
            &crate::config::OraclePolicy::default(),
            &[crate::analysis::language::LanguageId::Rust],
        )?;
        // The derived diff contains the candidate's src/lib.rs change;
        // the dirty worktree bytes never entered.
        // A successful Rust run records no LanguageRun failure entry;
        // the completed outcome with the candidate's diff counts is the
        // observable proof the materialized root was analyzed.
        assert!(
            result
                .language_runs
                .iter()
                .all(|run| run.language != "rust"),
            "the Rust adapter must not fail against the candidate root: {:?}",
            result.language_runs
        );
        assert!(
            result.summary.changed_rust_files >= 1,
            "the candidate diff must be consumed: {:?}",
            result.summary
        );
        Ok(())
    }

    #[test]
    fn missing_candidate_fails_closed_naming_the_identity() -> Result<(), String> {
        let (guard, base, _candidate) = fixture_repo("missing")?;
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            "0123456789012345678901234567890123456789",
        )?;
        let error = match resolve(&s, None) {
            Err(error) => error,
            Ok(_) => return Err("unknown candidate unexpectedly resolved".to_string()),
        };
        assert!(
            error.to_string().contains("git rev-parse"),
            "failure must name the resolver: {error}"
        );
        Ok(())
    }

    #[test]
    fn empty_base_uses_real_empty_tree_semantics() -> Result<(), String> {
        let (guard, _base, candidate) = fixture_repo("empty")?;
        let s = subject(&guard.0, GitCandidateBase::EmptyTree, &candidate)?;
        let resolved = resolve(&s, None).map_err(|e| e.to_string())?;
        assert!(
            resolved.diff.contains("src/lib.rs"),
            "empty→candidate must show the added files"
        );
        Ok(())
    }

    /// The guard reports a cleanup it could not perform instead of
    /// discarding the error. `Drop` cannot return one, so the decision of
    /// what counts as a failure lives here, where it can be asserted.
    #[test]
    fn remove_temp_root_separates_removal_success_from_real_failure() -> Result<(), String> {
        let base = std::env::temp_dir().join(format!(
            "ripr-temp-root-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos()
        ));
        let root = base.join("tree");
        std::fs::create_dir_all(root.join("nested")).map_err(|error| error.to_string())?;
        std::fs::write(root.join("nested/file.rs"), "pub fn one() {}\n")
            .map_err(|error| error.to_string())?;

        // A populated root is removed, and the removal is observable.
        remove_temp_root(&root)
            .map_err(|error| format!("populated root must be removed: {error}"))?;
        assert!(!root.exists(), "root must not survive a successful removal");

        // An already-absent root is success: the postcondition is that the
        // root is gone, not that this call is the one that removed it. A
        // guard dropped after a manual cleanup must stay quiet.
        remove_temp_root(&root).map_err(|error| format!("absent root must be success: {error}"))?;

        // A real failure is returned, not swallowed. A regular file is not a
        // directory on every platform this runs on, and — unlike a
        // permission denial — it fails for root too, so the negative control
        // holds in a container as well as on a developer machine.
        let not_a_directory = base.join("regular-file");
        std::fs::write(&not_a_directory, "not a directory\n").map_err(|error| error.to_string())?;
        let error = remove_temp_root(&not_a_directory)
            .err()
            .ok_or_else(|| "removing a non-directory must report the failure".to_string())?;
        assert_ne!(
            error.kind(),
            std::io::ErrorKind::NotFound,
            "a present-but-unremovable path must not be reported as already gone"
        );
        assert!(
            not_a_directory.exists(),
            "the failing path must still be there for the operator the warning names"
        );

        let _ = std::fs::remove_file(&not_a_directory);
        let _ = std::fs::remove_dir_all(&base);
        Ok(())
    }

    /// The guard's whole reason to exist on the failure path is that it says
    /// something. Capture what it actually writes.
    ///
    /// An earlier version of this test dropped the guard and then re-derived
    /// the message by calling `remove_temp_root` and `cleanup_failure_report`
    /// itself. Those two halves were disconnected: deleting the write from the
    /// guard left every assertion green, so the promised operator warning was
    /// not bound at all. `clean_up_reporting_to` is what `Drop` delegates to in
    /// full, so driving it against a buffer observes the real emission.
    #[test]
    fn cleanup_failure_writes_the_operator_warning_and_success_writes_nothing() -> Result<(), String>
    {
        let base = std::env::temp_dir().join(format!(
            "ripr-guard-drop-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).map_err(|error| error.to_string())?;

        // A regular file is not a directory on every platform, and — unlike a
        // permission denial — it fails for root too, so this control holds in
        // a container as well as on a developer machine.
        let unremovable = base.join("regular-file");
        std::fs::write(&unremovable, "not a directory\n").map_err(|error| error.to_string())?;

        let mut reported = Vec::new();
        TempRootGuard(unremovable.clone()).clean_up_reporting_to(&mut reported);
        let reported = String::from_utf8(reported).map_err(|error| error.to_string())?;
        assert!(
            reported.contains(&unremovable.display().to_string()),
            "the warning must name the path an operator has to remove: {reported:?}"
        );
        assert!(
            reported.contains("could not be removed"),
            "the warning must say what went wrong: {reported:?}"
        );
        assert!(
            unremovable.exists(),
            "the failing path must survive, or this proves nothing about the failure branch"
        );

        // A removable root is removed, and says nothing. Without this half a
        // guard that warned on every drop would still pass.
        let removable = base.join("tree");
        std::fs::create_dir_all(removable.join("nested")).map_err(|error| error.to_string())?;
        let mut quiet = Vec::new();
        TempRootGuard(removable.clone()).clean_up_reporting_to(&mut quiet);
        assert!(
            quiet.is_empty(),
            "a successful cleanup must not warn: {:?}",
            String::from_utf8_lossy(&quiet)
        );
        assert!(
            !removable.exists(),
            "a removable root must be gone after cleanup"
        );

        let _ = std::fs::remove_file(&unremovable);
        let _ = std::fs::remove_dir_all(&base);
        Ok(())
    }

    // #5015: one streaming `cat-file --batch` process materializes a whole
    // large tree. Every file — nested paths, an empty file, and a multi-chunk
    // binary blob whose framing bytes (NULs, interior newlines) could corrupt
    // a naive stream parser — must arrive byte-identical to the `git show`
    // oracle, which is exactly what the previous per-blob `cat-file` path was
    // pinned to.
    #[test]
    fn batched_materialization_preserves_bytes_at_scale() -> Result<(), String> {
        let (guard, base, _fixture_candidate) = fixture_repo("batchscale")?;
        let run = |args: &[&str]| -> Result<String, String> {
            let out = crate::git::run_git_output_with_deadline(&guard.0, args, GIT_DEADLINE)
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(format!(
                    "git {} failed: {}",
                    args[0],
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        let mut expected: Vec<(String, Vec<u8>)> = Vec::new();
        // 300 small files spread over nested directories: enough sequential
        // responses to exercise the batch protocol far beyond one pipe buffer
        // of framing, at a test-acceptable cost.
        for index in 0..300 {
            let deep = format!("wide/deep-{}/file-{}.rs", index % 17, index);
            let content = format!("pub fn f{index}() -> u8 {{ {index} }}\n").into_bytes();
            let path = guard.0.join(&deep);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&path, &content).map_err(|e| e.to_string())?;
            expected.push((deep, content));
        }
        // A binary blob larger than the 64 KiB read chunk: the stream parser
        // must not confuse content bytes with framing bytes. Every byte
        // value cycles through (NULs and interior newlines included).
        let binary: Vec<u8> = (0u8..=255).cycle().take(1024 * 1024 + 13).collect();
        std::fs::write(guard.0.join("binary.bin"), &binary).map_err(|e| e.to_string())?;
        expected.push(("binary.bin".to_string(), binary));
        // An empty file: zero content bytes followed only by the framing
        // newline.
        std::fs::write(guard.0.join("empty.rs"), b"").map_err(|e| e.to_string())?;
        expected.push(("empty.rs".to_string(), Vec::new()));
        run(&["add", "."])?;
        run(&["commit", "-m", "wide tree"])?;
        let candidate = run(&["rev-parse", "HEAD"])?;
        let s = subject(
            &guard.0,
            GitCandidateBase::Treeish(GitTreeish::new(&base).map_err(|e| e.to_string())?),
            &candidate,
        )?;
        let resolved = resolve(&s, None).map_err(|e| e.to_string())?;
        for (path, oracle) in &expected {
            let materialized =
                std::fs::read(resolved.root.join(path)).map_err(|e| e.to_string())?;
            let blob_oracle = candidate_blob(&guard.0, &candidate, path)?;
            assert_eq!(
                materialized, blob_oracle,
                "batch materialization must equal the bound blob bytes for {path}"
            );
            assert_eq!(
                materialized, *oracle,
                "batch materialization must equal the written fixture bytes for {path}"
            );
        }
        Ok(())
    }
}
